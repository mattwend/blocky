# RFC-002: Smart Contracts via WebAssembly

**Status:** Draft
**Date:** 2026-04-11
**Depends on:** RFC-001

## Goal

Add smart contract support to Blocky. Contracts are written in Rust, compiled to Wasm, and executed in a sandboxed runtime. This RFC also introduces world state (account balances and contract storage) as a prerequisite.

## Design Principles

1. **Determinism** — All nodes must produce identical results. Wasm is deterministic by default; we ban floats in contracts.
2. **Sandboxing** — Contracts cannot access the filesystem, network, or system clock. All external interaction goes through host functions.
3. **Metering** — Every contract call has a fuel (gas) budget. Execution halts if fuel runs out.
4. **Simplicity** — Minimal viable contract system. No cross-contract calls in v1.

## World State

State is the missing layer between transactions and the chain. We introduce it here because contracts need something to read from and write to.

### Account Model

```rust
pub struct AccountState {
    pub balance: u64,
    pub nonce: u64,                        // replay protection
    pub code: Option<Vec<u8>>,             // wasm bytecode (None for regular accounts)
    pub storage: BTreeMap<Vec<u8>, Vec<u8>>, // contract key-value store
}
```

### State Store

```rust
pub struct WorldState {
    accounts: HashMap<Address, AccountState>,
}

pub type Address = [u8; 32];

impl WorldState {
    pub fn get_account(&self, addr: &Address) -> Option<&AccountState>;
    pub fn get_or_create(&mut self, addr: &Address) -> &mut AccountState;
    pub fn get_balance(&self, addr: &Address) -> u64;
    pub fn transfer(&mut self, from: &Address, to: &Address, amount: u64) -> Result<()>;
    pub fn apply_block(&mut self, block: &Block) -> Result<()>;
}
```

`WorldState` lives on `Blockchain` and is updated when a block is mined. It is the authoritative record of all balances and contract storage.

### Balance Enforcement

`add_transaction` now validates that the sender has sufficient balance. This fulfills the RFC-001 decision to defer balance tracking to a later RFC.

## Transaction Types

The current `Transaction` struct is extended with a payload enum:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct Transaction {
    pub sender: Address,
    pub nonce: u64,
    pub payload: Payload,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub enum Payload {
    Transfer {
        receiver: Address,
        amount: u64,
    },
    Deploy {
        code: Vec<u8>,
    },
    Call {
        contract: Address,
        method: String,
        args: Vec<u8>,       // borsh-encoded arguments
        deposit: u64,        // tokens sent with the call
    },
}
```

### Address Derivation

- **User accounts:** SHA-256 of public key (public-key crypto is out of scope; for now, SHA-256 of the name string).
- **Contract accounts:** SHA-256 of `(deployer_address, deployer_nonce)` at deploy time. Deterministic and collision-free.

## Wasm Runtime

### Engine: wasmtime

[wasmtime](https://crates.io/crates/wasmtime) is the Bytecode Alliance reference Wasm runtime. Chosen for:

- First-class Rust API
- Cranelift JIT/AOT compilation
- Built-in fuel metering (`Store::set_fuel`)
- Production-proven (Fastly, Fermyon, NEAR)
- Deterministic execution guarantees

### Execution Flow

```
Call transaction received
  │
  ▼
WorldState.get_account(contract) → load wasm bytecode
  │
  ▼
wasmtime::Module::new(engine, &bytecode)
  │
  ▼
Create Store with:
  - fuel limit (from gas budget)
  - host state (caller, deposit, contract address)
  │
  ▼
Link host functions into the module
  │
  ▼
instance.get_func(method)?.call(args)
  │
  ▼
On success: commit state changes
On failure/out-of-fuel: revert all changes
```

### Module Caching

Compiling Wasm on every call is expensive. We cache compiled `wasmtime::Module` objects keyed by code hash. The cache is invalidated only if the bytecode changes (it won't — deployed code is immutable).

## Host Functions

Host functions are the contract's only interface to the outside world. They are linked into the Wasm instance via wasmtime's `Linker`.

### Namespace: `env`

| Function | Signature | Description |
|---|---|---|
| `storage_read` | `(key_ptr, key_len, val_ptr) -> i32` | Read from contract storage. Returns value length, or -1 if not found. |
| `storage_write` | `(key_ptr, key_len, val_ptr, val_len)` | Write to contract storage. |
| `storage_remove` | `(key_ptr, key_len) -> i32` | Delete key. Returns 1 if existed, 0 otherwise. |
| `get_balance` | `() -> u64` | Balance of the current contract. |
| `get_caller` | `(out_ptr)` | Write caller's 32-byte address to memory. |
| `get_deposit` | `() -> u64` | Tokens sent with this call. |
| `transfer` | `(to_ptr, amount) -> i32` | Transfer from contract balance. Returns 0 on success, 1 on insufficient funds. |
| `log` | `(msg_ptr, msg_len)` | Emit a log entry (stored in transaction receipt). |
| `abort` | `(msg_ptr, msg_len)` | Abort execution, revert all state changes. |

All pointer-based functions operate on the contract's linear memory. The host reads/writes directly via `wasmtime::Memory`.

## Serialization: Borsh

[Borsh](https://crates.io/crates/borsh) (Binary Object Representation Serializer for Hashing) is used for all contract-facing serialization:

- **Transaction args** — Borsh-encoded by the caller, Borsh-decoded by the contract.
- **Contract storage values** — Contracts Borsh-encode before calling `storage_write`.
- **Return values** — Borsh-encoded in Wasm memory, pointer+length returned to host.

Why Borsh over other formats:

| Property | Borsh | serde_json | MessagePack |
|---|---|---|---|
| Deterministic | Yes | No (key order) | No (map order) |
| Compact | Yes | No | Yes |
| Schema-driven | Yes | No | No |
| Rust-native | Yes | Yes | Yes |
| Hash-safe | Yes | No | No |

Deterministic serialization is critical — the same data must always produce the same bytes, because serialized data feeds into hashes.

**Note:** Internal chain serialization (block hashing, etc.) will migrate from `serde_json` to Borsh in a future RFC. This RFC only uses Borsh at the contract boundary.

## Gas Metering

wasmtime's built-in fuel system maps directly to gas:

```rust
store.set_fuel(gas_limit)?;
// ... execute ...
let remaining = store.get_fuel()?;
let gas_used = gas_limit - remaining;
```

### Gas Costs (v1, subject to tuning)

| Operation | Cost |
|---|---|
| Base transaction | 1,000 |
| Wasm instruction | 1 (wasmtime default) |
| `storage_read` | 500 |
| `storage_write` | 2,000 |
| `storage_remove` | 500 |
| `transfer` | 1,000 |
| Deploy (per byte) | 10 |

Default gas limit per transaction: **1,000,000**.

## Contract SDK

A small Rust crate (`blocky-sdk`) that contract authors depend on. It wraps the raw host function FFI in safe Rust:

```rust
// Example contract using the SDK
use blocky_sdk::{storage, caller, deposit, transfer, log};

#[no_mangle]
pub fn donate() {
    let donor = caller();
    let amount = deposit();
    let mut total: u64 = storage::read("total").unwrap_or(0);
    total += amount;
    storage::write("total", &total);
    log(&format!("received {} from {:?}", amount, donor));
}

#[no_mangle]
pub fn withdraw() {
    let owner: [u8; 32] = storage::read("owner").unwrap();
    assert_eq!(caller(), owner, "only owner can withdraw");
    let balance = blocky_sdk::balance();
    transfer(&owner, balance);
}
```

The SDK compiles to `wasm32-unknown-unknown`. It provides:

- `#[no_mangle] pub fn` — the entry point convention
- Borsh serde behind safe wrapper functions
- Memory allocator (`dlmalloc` or `wee_alloc`)
- Panic handler that calls `env::abort`

## Execution Receipts

Each executed transaction produces a receipt stored alongside the block:

```rust
pub struct Receipt {
    pub tx_hash: Hash,
    pub success: bool,
    pub gas_used: u64,
    pub logs: Vec<String>,
    pub error: Option<String>,   // set on revert
}
```

Receipts are not part of the block hash in v1 (no receipt trie). They are informational.

## Changes to Existing Types

### Blockchain

```rust
pub struct Blockchain {
    pub chain: Vec<Block>,
    pub pending_transactions: Vec<Transaction>,
    pub difficulty: u32,
    pub state: WorldState,           // new
    pub receipts: Vec<Vec<Receipt>>, // new: per-block receipts
    vm: VmEngine,                    // new: shared wasmtime engine + cache
}
```

### Block

No structural changes. Blocks still contain `Vec<Transaction>`. The transaction struct changes (see above) but the block format is unchanged.

### Validation

`is_valid` gains a new check: re-execute all transactions against a fresh state and verify the resulting state root matches. (State root hashing deferred to a future RFC — v1 just re-validates execution doesn't error.)

## Project Layout (additions)

```
blocky/
  src/
    state.rs          # WorldState, AccountState
    vm/
      mod.rs          # VmEngine: wasmtime setup, module cache
      host.rs         # Host function implementations
      gas.rs          # Gas cost constants
    transaction.rs    # Updated: Payload enum, Address type
  blocky-sdk/         # Separate crate (or workspace member)
    Cargo.toml        # targets wasm32-unknown-unknown
    src/
      lib.rs          # Safe wrappers around host functions
      storage.rs      # Typed storage helpers
```

## New Dependencies

| Crate | Purpose |
|---|---|
| `wasmtime` | Wasm execution engine |
| `borsh` | Deterministic binary serialization |

## Out of Scope (v1 contracts)

- Cross-contract calls
- Contract upgrades / proxy patterns
- Events/indexing beyond simple logs
- Public-key signatures on transactions
- State root / Merkle proof
- Persistent storage (chain is in-memory)
- Contract standards (ERC-20 equivalents)
- Wasm validation beyond wasmtime's built-in checks

## Implementation Order

1. **World state + balance tracking** — `state.rs`, update `Blockchain` to maintain state
2. **Transaction type refactor** — `Payload` enum, `Address` type, Borsh derives
3. **VM scaffold** — `vm/mod.rs`, wasmtime `Engine` + `Linker` setup
4. **Host functions** — `vm/host.rs`, wire into linker
5. **Deploy + Call execution** — integrate VM into `mine_pending` / `apply_block`
6. **Gas metering** — fuel limits, cost table
7. **Receipts** — capture results per transaction
8. **SDK crate** — `blocky-sdk` with safe wrappers
9. **REPL commands** — `deploy <path>`, `call <addr> <method> [args]`

## Open Questions

1. **Memory limits** — Should we cap Wasm linear memory? (Probably yes, e.g. 1 MB.)
2. **Code size limits** — Max bytecode size per deploy? (Suggested: 256 KB.)
3. **Storage limits** — Per-contract storage cap, or just gas-metered writes?
4. **Address format** — Stay with raw `[u8; 32]` or introduce a human-readable encoding (bech32, base58)?
