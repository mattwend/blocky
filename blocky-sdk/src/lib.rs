use borsh::{BorshDeserialize, BorshSerialize};
use std::{string::String, vec, vec::Vec};

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct CallEnvelope {
    pub method: String,
    pub args: Vec<u8>,
}

impl CallEnvelope {
    pub fn new(method: impl Into<String>, args: Vec<u8>) -> Self {
        Self {
            method: method.into(),
            args,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        borsh::to_vec(self).expect("call envelope serialization should succeed")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        borsh::from_slice(bytes).map_err(|error| error.to_string())
    }
}

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

pub fn call_envelope() -> Result<CallEnvelope, String> {
    CallEnvelope::decode(&input())
}

pub fn decode_args<T: BorshDeserialize>() -> Result<T, String> {
    let envelope = call_envelope()?;
    borsh::from_slice(&envelope.args).map_err(|error| error.to_string())
}

pub fn log(message: &str) {
    unsafe { env::log(message.as_ptr(), message.len() as i64) }
}

pub fn balance() -> u64 {
    unsafe { env::get_balance() as u64 }
}

pub fn deposit() -> u64 {
    unsafe { env::get_deposit() as u64 }
}

pub fn caller() -> [u8; 32] {
    let mut address = [0_u8; 32];
    unsafe { env::get_caller(address.as_mut_ptr()) };
    address
}

pub fn transfer(to: &[u8; 32], amount: u64) -> bool {
    unsafe { env::env_transfer(to.as_ptr(), amount as i64) == 0 }
}

pub mod storage {
    use super::env;
    use borsh::{BorshDeserialize, BorshSerialize};
    use std::vec;

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

    pub fn write<K, V>(key: K, value: &V)
    where
        K: AsRef<[u8]>,
        V: BorshSerialize,
    {
        let key = key.as_ref();
        let bytes = borsh::to_vec(value)
            .unwrap_or_else(|_| super::panic_abort("failed to serialize storage value"));
        unsafe {
            env::storage_write(
                key.as_ptr(),
                key.len() as i32,
                bytes.as_ptr(),
                bytes.len() as i32,
            )
        }
    }

    pub fn remove<K>(key: K) -> bool
    where
        K: AsRef<[u8]>,
    {
        let key = key.as_ref();
        unsafe { env::storage_remove(key.as_ptr(), key.len() as i32) == 1 }
    }
}

pub(crate) fn panic_abort(message: &str) -> ! {
    unsafe { env::abort(message.as_ptr(), message.len() as i32) }
}

pub mod env {
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
        pub fn log(msg_ptr: *const u8, msg_len: i64);
        pub fn abort(msg_ptr: *const u8, msg_len: i32) -> !;
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::{CallEnvelope, balance, caller, decode_args, deposit, storage};
    use borsh::{BorshDeserialize, BorshSerialize};
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
    extern "C" fn log(msg_ptr: *const u8, msg_len: i64) {
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
    fn decodes_envelope() {
        let envelope = CallEnvelope {
            method: "set".to_string(),
            args: borsh::to_vec(&SetArgs { value: 7 }).unwrap(),
        };

        let decoded = CallEnvelope::decode(&borsh::to_vec(&envelope).unwrap()).unwrap();

        assert_eq!(decoded.method, "set");
    }

    #[test]
    fn decodes_typed_args_from_envelope_payload() {
        let envelope = CallEnvelope {
            method: "set".to_string(),
            args: borsh::to_vec(&SetArgs { value: 11 }).unwrap(),
        };
        let encoded = borsh::to_vec(&envelope).unwrap();
        let decoded = CallEnvelope::decode(&encoded).unwrap();
        let args: SetArgs = borsh::from_slice(&decoded.args).unwrap();

        assert_eq!(args, SetArgs { value: 11 });
        let _ = decode_args::<SetArgs>;
    }

    #[test]
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
    fn typed_storage_round_trip_and_remove() {
        reset_host();

        storage::write("total", &11_u64);
        assert_eq!(storage::read::<_, u64>("total"), Some(11));
        assert!(storage::remove("total"));
        assert_eq!(storage::read::<_, u64>("total"), None);
    }
}
