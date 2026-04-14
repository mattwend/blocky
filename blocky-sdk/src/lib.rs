//! Contract-side SDK for blocky's Wasm smart contracts.
//!
//! This crate provides structured call input via [`CallEnvelope`], safe wrappers
//! around runtime host functions such as [`log`], [`balance`], [`deposit`],
//! [`caller`], and [`transfer`], typed storage helpers in [`storage`], and the
//! raw imported functions in [`mod@env`] for low-level integrations.
//!
//! Most contracts will decode call input with [`call_envelope`] or
//! [`decode_args`], then interact with storage and the host through the safe
//! wrappers while compiling to `wasm32-unknown-unknown`.

use borsh::{BorshDeserialize, BorshSerialize};
use std::{string::String, vec, vec::Vec};

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// Structured input passed from the host into a contract call.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct CallEnvelope {
    /// Exported guest method to invoke.
    pub method: String,
    /// Method-specific encoded arguments.
    pub args: Vec<u8>,
}

impl CallEnvelope {
    /// Creates a new call envelope.
    ///
    /// # Arguments
    /// - `method`: Exported guest method to invoke.
    /// - `args`: Encoded method-specific arguments.
    ///
    /// # Returns
    /// A new call envelope.
    pub fn new(method: impl Into<String>, args: Vec<u8>) -> Self {
        Self {
            method: method.into(),
            args,
        }
    }

    /// Encodes the envelope using Borsh.
    ///
    /// # Returns
    /// The Borsh-encoded envelope bytes, or a stringified serialization error.
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        borsh::to_vec(self).map_err(|error| error.to_string())
    }

    /// Decodes a Borsh-encoded call envelope.
    ///
    /// # Arguments
    /// - `bytes`: Raw bytes to decode.
    ///
    /// # Returns
    /// The decoded envelope, or a stringified decode error.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        borsh::from_slice(bytes).map_err(|error| error.to_string())
    }
}

/// Reads the raw input bytes for the current contract call.
///
/// # Returns
/// The current call input, or an empty vector if no input is available.
pub fn input() -> Vec<u8> {
    let len = unsafe { env::input_len() };
    if len <= 0 {
        return Vec::new();
    }

    let mut bytes = vec![0_u8; len as usize];
    let written = unsafe { env::read_input(bytes.as_mut_ptr()) };
    if written <= 0 {
        return Vec::new();
    }

    bytes.truncate(written as usize);
    bytes
}

/// Reads and decodes the current call input as a [`CallEnvelope`].
///
/// # Returns
/// The decoded envelope, or a decode error string.
pub fn call_envelope() -> Result<CallEnvelope, String> {
    CallEnvelope::decode(&input())
}

/// Decodes typed arguments from the current call envelope.
///
/// # Returns
/// The decoded argument value, or a decode error string.
pub fn decode_args<T: BorshDeserialize>() -> Result<T, String> {
    let envelope = call_envelope()?;
    borsh::from_slice(&envelope.args).map_err(|error| error.to_string())
}

/// Emits a log line through the host.
///
/// # Arguments
/// - `message`: UTF-8 message to append to the receipt logs.
pub fn log(message: &str) {
    unsafe { env::host_log(message.as_ptr(), message.len() as i32) }
}

/// Returns the executing contract's balance.
///
/// # Returns
/// The current contract balance.
pub fn balance() -> u64 {
    unsafe { env::get_balance() as u64 }
}

/// Returns the deposit attached to the current call.
///
/// # Returns
/// The amount transferred into the contract before execution.
pub fn deposit() -> u64 {
    unsafe { env::get_deposit() as u64 }
}

/// Returns the external caller address for the current execution.
///
/// # Returns
/// The 32-byte caller address.
pub fn caller() -> [u8; 32] {
    let mut address = [0_u8; 32];
    unsafe { env::get_caller(address.as_mut_ptr()) };
    address
}

/// Transfers balance from the executing contract to another address.
///
/// # Arguments
/// - `to`: Destination address.
/// - `amount`: Amount of balance to transfer.
///
/// # Returns
/// `true` when the transfer succeeds, otherwise `false`.
pub fn transfer(to: &[u8; 32], amount: u64) -> bool {
    unsafe { env::env_transfer(to.as_ptr(), amount as i64) == 0 }
}

/// Typed helpers for reading and writing contract storage.
pub mod storage {
    use super::env;
    use borsh::{BorshDeserialize, BorshSerialize};
    use std::vec;

    /// Reads and decodes a typed value from contract storage.
    ///
    /// # Arguments
    /// - `key`: Storage key to read.
    ///
    /// # Returns
    /// The decoded value if present and decodable, otherwise `None`.
    pub fn read<K, V>(key: K) -> Option<V>
    where
        K: AsRef<[u8]>,
        V: BorshDeserialize,
    {
        let key = key.as_ref();
        let len =
            unsafe { env::storage_read(key.as_ptr(), key.len() as i32, core::ptr::null_mut()) };
        if len < 0 {
            return None;
        }

        let mut bytes = vec![0_u8; len as usize];
        let written =
            unsafe { env::storage_read(key.as_ptr(), key.len() as i32, bytes.as_mut_ptr()) };
        if written < 0 {
            return None;
        }

        bytes.truncate(written as usize);
        borsh::from_slice(&bytes).ok()
    }

    /// Encodes and writes a typed value into contract storage.
    ///
    /// # Arguments
    /// - `key`: Storage key to write.
    /// - `value`: Typed value to encode and persist.
    pub fn write<K, V>(key: K, value: &V)
    where
        K: AsRef<[u8]>,
        V: BorshSerialize,
    {
        let key = key.as_ref();
        let bytes = match borsh::to_vec(value) {
            Ok(bytes) => bytes,
            Err(error) => {
                let message = format!("failed to serialize storage value: {error}");
                super::panic_abort(&message)
            }
        };
        unsafe {
            env::storage_write(
                key.as_ptr(),
                key.len() as i32,
                bytes.as_ptr(),
                bytes.len() as i32,
            )
        }
    }

    /// Removes a value from contract storage.
    ///
    /// # Arguments
    /// - `key`: Storage key to delete.
    ///
    /// # Returns
    /// `true` if a value existed and was removed, otherwise `false`.
    pub fn remove<K>(key: K) -> bool
    where
        K: AsRef<[u8]>,
    {
        let key = key.as_ref();
        unsafe { env::storage_remove(key.as_ptr(), key.len() as i32) == 1 }
    }
}

pub(crate) fn panic_abort(message: &str) -> ! {
    unsafe { env::host_abort(message.as_ptr(), message.len() as i32) }
}

/// Raw host imports exposed by the runtime.
pub mod env {
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        pub fn input_len() -> i32;
        pub fn read_input(out_ptr: *mut u8) -> i32;
        pub fn storage_read(key_ptr: *const u8, key_len: i32, val_ptr: *mut u8) -> i32;
        pub fn storage_write(key_ptr: *const u8, key_len: i32, val_ptr: *const u8, val_len: i32);
        pub fn storage_remove(key_ptr: *const u8, key_len: i32) -> i32;
        pub fn get_balance() -> i64;
        pub fn get_caller(out_ptr: *mut u8);
        pub fn get_deposit() -> i64;
        #[link_name = "transfer"]
        pub fn env_transfer(to_ptr: *const u8, amount: i64) -> i32;
        #[link_name = "log"]
        pub fn host_log(msg_ptr: *const u8, msg_len: i32);
        #[link_name = "abort"]
        pub fn host_abort(msg_ptr: *const u8, msg_len: i32) -> !;
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::{CallEnvelope, balance, call_envelope, caller, decode_args, deposit, storage};
    use borsh::{BorshDeserialize, BorshSerialize};
    use serial_test::serial;
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};

    #[derive(Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
    struct SetArgs {
        value: u64,
    }

    #[derive(Default)]
    struct TestHost {
        input: Vec<u8>,
        balance: u64,
        deposit: u64,
        caller: [u8; 32],
        transfers: Vec<([u8; 32], u64)>,
        logs: Vec<String>,
        storage: BTreeMap<Vec<u8>, Vec<u8>>,
    }

    fn host() -> &'static Mutex<TestHost> {
        static HOST: OnceLock<Mutex<TestHost>> = OnceLock::new();
        HOST.get_or_init(|| Mutex::new(TestHost::default()))
    }

    fn reset_host() {
        *host().lock().unwrap() = TestHost::default();
    }

    #[unsafe(no_mangle)]
    extern "C" fn input_len() -> i32 {
        host().lock().unwrap().input.len() as i32
    }

    #[unsafe(no_mangle)]
    extern "C" fn read_input(out_ptr: *mut u8) -> i32 {
        let host = host().lock().unwrap();
        let len = host.input.len();
        unsafe { std::ptr::copy_nonoverlapping(host.input.as_ptr(), out_ptr, len) };
        len as i32
    }

    #[unsafe(no_mangle)]
    extern "C" fn storage_read(key_ptr: *const u8, key_len: i32, val_ptr: *mut u8) -> i32 {
        let key = unsafe { std::slice::from_raw_parts(key_ptr, key_len.max(0) as usize) };
        let host = host().lock().unwrap();
        let Some(value) = host.storage.get(key) else {
            return -1;
        };
        if !val_ptr.is_null() {
            unsafe { std::ptr::copy_nonoverlapping(value.as_ptr(), val_ptr, value.len()) };
        }
        value.len() as i32
    }

    #[unsafe(no_mangle)]
    extern "C" fn storage_write(
        key_ptr: *const u8,
        key_len: i32,
        val_ptr: *const u8,
        val_len: i32,
    ) {
        let key = unsafe { std::slice::from_raw_parts(key_ptr, key_len.max(0) as usize) }.to_vec();
        let value =
            unsafe { std::slice::from_raw_parts(val_ptr, val_len.max(0) as usize) }.to_vec();
        host().lock().unwrap().storage.insert(key, value);
    }

    #[unsafe(no_mangle)]
    extern "C" fn storage_remove(key_ptr: *const u8, key_len: i32) -> i32 {
        let key = unsafe { std::slice::from_raw_parts(key_ptr, key_len.max(0) as usize) };
        host().lock().unwrap().storage.remove(key).is_some() as i32
    }

    #[unsafe(no_mangle)]
    extern "C" fn get_balance() -> i64 {
        host().lock().unwrap().balance as i64
    }

    #[unsafe(no_mangle)]
    extern "C" fn get_caller(out_ptr: *mut u8) {
        let caller = host().lock().unwrap().caller;
        unsafe { std::ptr::copy_nonoverlapping(caller.as_ptr(), out_ptr, caller.len()) };
    }

    #[unsafe(no_mangle)]
    extern "C" fn get_deposit() -> i64 {
        host().lock().unwrap().deposit as i64
    }

    #[unsafe(export_name = "transfer")]
    extern "C" fn env_transfer(to_ptr: *const u8, amount: i64) -> i32 {
        let mut to = [0_u8; 32];
        unsafe { std::ptr::copy_nonoverlapping(to_ptr, to.as_mut_ptr(), to.len()) };
        host().lock().unwrap().transfers.push((to, amount as u64));
        0
    }

    #[unsafe(no_mangle)]
    extern "C" fn log(msg_ptr: *const u8, msg_len: i32) {
        let bytes = unsafe { std::slice::from_raw_parts(msg_ptr, msg_len.max(0) as usize) };
        host()
            .lock()
            .unwrap()
            .logs
            .push(String::from_utf8(bytes.to_vec()).unwrap());
    }

    #[unsafe(no_mangle)]
    extern "C" fn abort(_msg_ptr: *const u8, _msg_len: i32) -> ! {
        panic!("abort called")
    }

    #[test]
    #[serial]
    fn decodes_envelope() {
        let envelope = CallEnvelope {
            method: "set".to_string(),
            args: borsh::to_vec(&SetArgs { value: 7 }).unwrap(),
        };

        let decoded = CallEnvelope::decode(&borsh::to_vec(&envelope).unwrap()).unwrap();

        assert_eq!(decoded.method, "set");
    }

    #[test]
    #[serial]
    fn decodes_typed_args_from_envelope_payload() {
        reset_host();
        let envelope = CallEnvelope {
            method: "set".to_string(),
            args: borsh::to_vec(&SetArgs { value: 11 }).unwrap(),
        };
        host().lock().unwrap().input = envelope.encode().unwrap();

        let decoded_envelope = call_envelope().unwrap();
        let args: SetArgs = decode_args().unwrap();

        assert_eq!(decoded_envelope.method, "set");
        assert_eq!(args, SetArgs { value: 11 });
    }

    #[test]
    #[serial]
    fn host_wrappers_delegate_to_env() {
        reset_host();
        let mut guard = host().lock().unwrap();
        guard.balance = 42;
        guard.deposit = 9;
        guard.caller = [7_u8; 32];
        drop(guard);

        assert_eq!(balance(), 42);
        assert_eq!(deposit(), 9);
        assert_eq!(caller(), [7_u8; 32]);

        assert!(super::transfer(&[3_u8; 32], 5));
        super::log("hello");

        let guard = host().lock().unwrap();
        assert_eq!(guard.transfers, vec![([3_u8; 32], 5)]);
        assert_eq!(guard.logs, vec!["hello".to_string()]);
    }

    #[test]
    #[serial]
    fn typed_storage_round_trip_and_remove() {
        reset_host();

        storage::write("total", &11_u64);
        assert_eq!(storage::read::<_, u64>("total"), Some(11));
        assert!(storage::remove("total"));
        assert_eq!(storage::read::<_, u64>("total"), None);
    }
}
