use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

use blocky::{Address, Blockchain, Transaction, address_from_name};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn unique_contract_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "blocky-sdk-e2e-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

fn build_contract_source(source: &str) -> Vec<u8> {
    let dir = unique_contract_dir();
    fs::create_dir_all(dir.join("src")).unwrap();

    fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"sdk-e2e-contract\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n[profile.release]\npanic = \"abort\"\n\n[dependencies]\nblocky-sdk = {{ path = {:?} }}\nborsh = {{ version = \"1\", features = [\"derive\"] }}\n",
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("blocky-sdk")
        ),
    )
    .unwrap();
    fs::write(dir.join("src/lib.rs"), source).unwrap();

    let status = Command::new("cargo")
        .args(["build", "--target", "wasm32-unknown-unknown", "--release"])
        .env("RUSTFLAGS", "-C debuginfo=2")
        .current_dir(&dir)
        .status()
        .unwrap();
    assert!(status.success(), "contract build failed");

    let wasm =
        fs::read(dir.join("target/wasm32-unknown-unknown/release/sdk_e2e_contract.wasm")).unwrap();
    strip_custom_sections(&wasm)
}

fn strip_custom_sections(wasm: &[u8]) -> Vec<u8> {
    fn read_uleb(bytes: &[u8], offset: &mut usize) -> usize {
        let mut result = 0usize;
        let mut shift = 0usize;
        loop {
            let byte = bytes[*offset];
            *offset += 1;
            result |= ((byte & 0x7f) as usize) << shift;
            if byte & 0x80 == 0 {
                return result;
            }
            shift += 7;
        }
    }

    fn write_uleb(mut value: usize, out: &mut Vec<u8>) {
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if value == 0 {
                break;
            }
        }
    }

    let mut result = wasm[..8].to_vec();
    let mut offset = 8usize;
    while offset < wasm.len() {
        let section_id = wasm[offset];
        offset += 1;
        let section_size = read_uleb(wasm, &mut offset);
        let section_start = offset;
        let section_end = section_start + section_size;

        let keep = if section_id == 0 {
            let mut name_offset = section_start;
            let name_len = read_uleb(wasm, &mut name_offset);
            let name_end = name_offset + name_len;
            let name = std::str::from_utf8(&wasm[name_offset..name_end]).unwrap_or_default();
            !matches!(name, "name" | "producers" | "target_features")
        } else {
            true
        };

        if keep {
            result.push(section_id);
            write_uleb(section_size, &mut result);
            result.extend_from_slice(&wasm[section_start..section_end]);
        }

        offset = section_end;
    }

    result
}

fn queue_and_mine(chain: &mut Blockchain, tx: Transaction) {
    chain.add_transaction(tx).unwrap();
    chain.mine_pending().unwrap();
}

fn deploy_contract(chain: &mut Blockchain, sender: Address, nonce: u64, wasm: Vec<u8>) -> Address {
    let deploy = Transaction::new_deploy(sender, nonce, wasm);
    let contract = deploy.derived_contract_address();
    queue_and_mine(chain, deploy);
    contract
}

fn build_and_deploy_contract(
    chain: &mut Blockchain,
    sender: Address,
    nonce: u64,
    source: &str,
) -> Address {
    let wasm = build_contract_source(source);
    deploy_contract(chain, sender, nonce, wasm)
}

#[test]
fn sdk_noop_contract_succeeds() {
    let source = r#"
#[unsafe(no_mangle)]
pub extern "C" fn noop() {}
"#;

    let mut chain = Blockchain::new(1);
    let alice = address_from_name("alice");
    chain.credit_balance(alice, 5_000_000);

    let contract = build_and_deploy_contract(&mut chain, alice, 0, source);
    queue_and_mine(
        &mut chain,
        Transaction::new_call(alice, 1, contract, "noop", Vec::new(), 0),
    );

    let receipt = chain.receipts.last().unwrap().last().unwrap();
    assert!(receipt.success);
}

#[test]
fn sdk_log_only_contract_succeeds() {
    let source = r#"
use blocky_sdk::log;

#[unsafe(no_mangle)]
pub extern "C" fn emit() {
    log("hello");
}
"#;

    let mut chain = Blockchain::new(1);
    let alice = address_from_name("alice");
    chain.credit_balance(alice, 5_000_000);

    let contract = build_and_deploy_contract(&mut chain, alice, 0, source);
    queue_and_mine(
        &mut chain,
        Transaction::new_call(alice, 1, contract, "emit", Vec::new(), 0),
    );

    let receipt = chain.receipts.last().unwrap().last().unwrap();
    assert!(receipt.success);
    assert_eq!(receipt.logs, vec!["hello".to_string()]);
}

#[test]
fn sdk_panic_handler_aborts_and_reverts_state_changes() {
    let source = r#"
use blocky_sdk::{log, storage};

#[unsafe(no_mangle)]
pub extern "C" fn explode() {
    storage::write("key", &1_u64);
    log("before panic");
    panic!("boom");
}
"#;

    let mut chain = Blockchain::new(1);
    let alice = address_from_name("alice");
    chain.credit_balance(alice, 5_000_000);

    let wasm = build_contract_source(source);
    assert!(wasm.starts_with(b"\0asm"));
    let contract = deploy_contract(&mut chain, alice, 0, wasm);

    let call = Transaction::new_call(alice, 1, contract, "explode", Vec::new(), 13);
    chain.add_transaction(call.clone()).unwrap();

    let error = chain.mine_pending().unwrap_err();
    let message = error.to_string();
    assert!(!message.is_empty());

    assert_eq!(chain.chain.len(), 2);
    assert_eq!(chain.pending_transactions, vec![call]);
    let contract_account = chain.state.get_account(&contract).unwrap();
    assert!(contract_account.storage.is_empty());
    assert_eq!(chain.state.get_balance(&contract), 0);

    let receipts = chain.receipts.last().unwrap();
    let receipt = receipts.last().unwrap();
    assert!(!receipt.success);
    assert!(
        receipt
            .error
            .as_ref()
            .is_some_and(|error| !error.is_empty())
    );
    assert!(receipt.logs.is_empty());
}

#[test]
fn sdk_wrappers_log_and_query_context() {
    let source = r#"
use blocky_sdk::log;

#[unsafe(no_mangle)]
pub extern "C" fn inspect() {
    log("inspect ok");
}
"#;

    let mut chain = Blockchain::new(1);
    let alice = [43_u8; 32];
    chain.credit_balance(alice, 5_000_000);

    let contract = build_and_deploy_contract(&mut chain, alice, 0, source);
    queue_and_mine(
        &mut chain,
        Transaction::new_call(alice, 1, contract, "inspect", Vec::new(), 13),
    );

    let receipt = chain.receipts.last().unwrap().last().unwrap();
    assert!(receipt.success);
    assert_eq!(receipt.logs, vec!["inspect ok".to_string()]);
    assert_eq!(chain.state.get_balance(&contract), 13);
}

#[test]
fn sdk_typed_storage_round_trips_plain_value() {
    let source = r#"
use blocky_sdk::{decode_args, log, storage};
use borsh::BorshDeserialize;

#[derive(BorshDeserialize)]
struct SetArgs {
    value: u64,
}

#[unsafe(no_mangle)]
pub extern "C" fn set() {
    let args: SetArgs = decode_args().unwrap();
    storage::write("value", &args.value);
    log("set ok");
}

#[unsafe(no_mangle)]
pub extern "C" fn clear() {
    let value: u64 = storage::read("value").unwrap();
    assert_eq!(value, 42);
    assert!(storage::remove("value"));
    log("clear ok");
}
"#;

    let mut chain = Blockchain::new(1);
    let alice = address_from_name("alice");
    chain.credit_balance(alice, 5_000_000);

    let contract = build_and_deploy_contract(&mut chain, alice, 0, source);
    queue_and_mine(
        &mut chain,
        Transaction::new_call(
            alice,
            1,
            contract,
            "set",
            borsh::to_vec(&42_u64).unwrap(),
            0,
        ),
    );

    let contract_account = chain.state.get_account(&contract).unwrap();
    assert!(contract_account.storage.contains_key(b"value" as &[u8]));
    let set_receipt = chain.receipts.last().unwrap().last().unwrap();
    assert!(set_receipt.success);
    assert_eq!(set_receipt.logs, vec!["set ok".to_string()]);

    queue_and_mine(
        &mut chain,
        Transaction::new_call(alice, 2, contract, "clear", Vec::new(), 0),
    );

    let contract_account = chain.state.get_account(&contract).unwrap();
    assert!(!contract_account.storage.contains_key(b"value" as &[u8]));
    let clear_receipt = chain.receipts.last().unwrap().last().unwrap();
    assert!(clear_receipt.success);
    assert_eq!(clear_receipt.logs, vec!["clear ok".to_string()]);
}

#[test]
fn sdk_transfer_wrapper_moves_balance() {
    let source = r#"
use blocky_sdk::{caller, log, transfer};

#[unsafe(no_mangle)]
pub extern "C" fn payout() {
    let recipient = caller();
    assert!(transfer(&recipient, 7));
    log("payout ok");
}
"#;

    let mut chain = Blockchain::new(1);
    let alice = address_from_name("alice");
    chain.credit_balance(alice, 5_000_000);

    let contract = build_and_deploy_contract(&mut chain, alice, 0, source);
    let before = chain.state.get_balance(&alice);

    queue_and_mine(
        &mut chain,
        Transaction::new_call(alice, 1, contract, "payout", Vec::new(), 13),
    );

    let receipt = chain.receipts.last().unwrap().last().unwrap();
    assert!(receipt.success);
    assert_eq!(receipt.logs, vec!["payout ok".to_string()]);
    assert_eq!(chain.state.get_balance(&contract), 6);
    assert_eq!(chain.state.get_balance(&alice), before - 13 + 7);
}
