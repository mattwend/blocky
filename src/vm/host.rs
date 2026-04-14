use anyhow::anyhow;
use thiserror::Error;
use wasmtime::{Caller, Linker, Memory};

use crate::{Address, WorldState};

use super::gas::{STORAGE_READ_COST, STORAGE_REMOVE_COST, STORAGE_WRITE_COST, TRANSFER_COST};

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub caller: Address,
    pub contract: Address,
    pub deposit: u64,
    pub args: Vec<u8>,
    pub reverted: bool,
    pub revert_message: Option<String>,
    pub logs: Vec<String>,
}

impl ExecutionContext {
    pub fn new(caller: Address, contract: Address, deposit: u64, args: Vec<u8>) -> Self {
        Self {
            caller,
            contract,
            deposit,
            args,
            reverted: false,
            revert_message: None,
            logs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VmHostState {
    pub state: WorldState,
    pub context: Option<ExecutionContext>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HostError {
    #[error("guest memory export \"memory\" is missing")]
    MissingMemory,
    #[error("negative pointer or length")]
    NegativeOffset,
    #[error("guest memory access out of bounds")]
    OutOfBounds,
    #[error("execution context is missing")]
    MissingContext,
    #[error("failed to charge gas: {0}")]
    Gas(String),
}

pub fn link_host_functions(linker: &mut Linker<VmHostState>) -> Result<(), wasmtime::Error> {
    linker.func_wrap(
        "env",
        "storage_read",
        |mut caller: Caller<'_, VmHostState>, key_ptr: i32, key_len: i32, val_ptr: i32| -> i32 {
            match storage_read(&mut caller, key_ptr, key_len, val_ptr) {
                Ok(Some(length)) => length as i32,
                Ok(None) => -1,
                Err(_) => -1,
            }
        },
    )?;
    linker.func_wrap(
        "env",
        "storage_write",
        |mut caller: Caller<'_, VmHostState>,
         key_ptr: i32,
         key_len: i32,
         val_ptr: i32,
         val_len: i32| {
            let _ = storage_write(&mut caller, key_ptr, key_len, val_ptr, val_len);
        },
    )?;
    linker.func_wrap(
        "env",
        "storage_remove",
        |mut caller: Caller<'_, VmHostState>, key_ptr: i32, key_len: i32| -> i32 {
            match storage_remove(&mut caller, key_ptr, key_len) {
                Ok(true) => 1,
                Ok(false) => 0,
                Err(_) => 0,
            }
        },
    )?;
    linker.func_wrap(
        "env",
        "get_balance",
        |caller: Caller<'_, VmHostState>| -> i64 {
            get_balance(&caller).unwrap_or_default() as i64
        },
    )?;
    linker.func_wrap(
        "env",
        "get_caller",
        |mut caller: Caller<'_, VmHostState>, out_ptr: i32| {
            let _ = get_caller(&mut caller, out_ptr);
        },
    )?;
    linker.func_wrap(
        "env",
        "get_deposit",
        |caller: Caller<'_, VmHostState>| -> i64 {
            get_deposit(&caller).unwrap_or_default() as i64
        },
    )?;
    linker.func_wrap(
        "env",
        "transfer",
        |mut caller: Caller<'_, VmHostState>, to_ptr: i32, amount: i64| -> i32 {
            match transfer(&mut caller, to_ptr, amount as u64) {
                Ok(()) => 0,
                Err(_) => 1,
            }
        },
    )?;
    linker.func_wrap(
        "env",
        "input_len",
        |caller: Caller<'_, VmHostState>| -> i32 { input_len(&caller).unwrap_or(-1) },
    )?;
    linker.func_wrap(
        "env",
        "read_input",
        |mut caller: Caller<'_, VmHostState>, out_ptr: i32| -> i32 {
            match read_input(&mut caller, out_ptr) {
                Ok(length) => length as i32,
                Err(_) => -1,
            }
        },
    )?;
    linker.func_wrap(
        "env",
        "log",
        |mut caller: Caller<'_, VmHostState>, msg_ptr: i32, msg_len: i32| {
            if let Ok(message_bytes) = read_memory(&mut caller, msg_ptr, msg_len) {
                let message = String::from_utf8_lossy(&message_bytes).into_owned();
                if let Some(context) = caller.data_mut().context.as_mut() {
                    context.logs.push(message);
                }
            }
        },
    )?;
    linker.func_wrap(
        "env",
        "abort",
        |mut caller: Caller<'_, VmHostState>,
         msg_ptr: i32,
         msg_len: i32|
         -> Result<(), wasmtime::Error> {
            let message = match read_memory(&mut caller, msg_ptr, msg_len) {
                Ok(message_bytes) => String::from_utf8_lossy(&message_bytes).into_owned(),
                Err(error) => error.to_string(),
            };

            if let Some(context) = caller.data_mut().context.as_mut() {
                context.reverted = true;
                context.revert_message = Some(message.clone());
            }

            Err(anyhow!(message))
        },
    )?;
    Ok(())
}

pub fn storage_read(
    caller: &mut Caller<'_, VmHostState>,
    key_ptr: i32,
    key_len: i32,
    val_ptr: i32,
) -> Result<Option<usize>, HostError> {
    charge_gas(caller, STORAGE_READ_COST)?;
    let key = read_memory(caller, key_ptr, key_len)?;
    let contract = execution_context(caller)?.contract;
    let value: Option<Vec<u8>> = caller
        .data()
        .state
        .get_account(&contract)
        .and_then(|account| account.storage.get(&key))
        .cloned();

    match value {
        Some(bytes) => {
            write_memory(caller, val_ptr, &bytes)?;
            Ok(Some(bytes.len()))
        }
        None => Ok(None),
    }
}

pub fn storage_write(
    caller: &mut Caller<'_, VmHostState>,
    key_ptr: i32,
    key_len: i32,
    val_ptr: i32,
    val_len: i32,
) -> Result<(), HostError> {
    charge_gas(caller, STORAGE_WRITE_COST)?;
    let key = read_memory(caller, key_ptr, key_len)?;
    let value = read_memory(caller, val_ptr, val_len)?;
    let contract = execution_context(caller)?.contract;
    caller
        .data_mut()
        .state
        .get_or_create(&contract)
        .storage
        .insert(key, value);
    Ok(())
}

pub fn storage_remove(
    caller: &mut Caller<'_, VmHostState>,
    key_ptr: i32,
    key_len: i32,
) -> Result<bool, HostError> {
    charge_gas(caller, STORAGE_REMOVE_COST)?;
    let key = read_memory(caller, key_ptr, key_len)?;
    let contract = execution_context(caller)?.contract;
    let removed = caller
        .data_mut()
        .state
        .get_or_create(&contract)
        .storage
        .remove(&key)
        .is_some();
    Ok(removed)
}

pub fn get_balance(caller: &Caller<'_, VmHostState>) -> Result<u64, HostError> {
    let contract = execution_context(caller)?.contract;
    Ok(caller.data().state.get_balance(&contract))
}

pub fn get_caller(caller: &mut Caller<'_, VmHostState>, out_ptr: i32) -> Result<(), HostError> {
    let address = execution_context(caller)?.caller;
    write_memory(caller, out_ptr, &address)
}

pub fn get_deposit(caller: &Caller<'_, VmHostState>) -> Result<u64, HostError> {
    Ok(execution_context(caller)?.deposit)
}

pub fn transfer(
    caller: &mut Caller<'_, VmHostState>,
    to_ptr: i32,
    amount: u64,
) -> Result<(), HostError> {
    charge_gas(caller, TRANSFER_COST)?;
    let to = read_address(caller, to_ptr)?;
    let from = execution_context(caller)?.contract;
    caller
        .data_mut()
        .state
        .transfer(&from, &to, amount)
        .map_err(|error| HostError::Gas(error.to_string()))?;
    Ok(())
}

pub fn input_len(caller: &Caller<'_, VmHostState>) -> Result<i32, HostError> {
    let len = execution_context(caller)?.args.len();
    i32::try_from(len).map_err(|_| HostError::OutOfBounds)
}

pub fn read_input(caller: &mut Caller<'_, VmHostState>, out_ptr: i32) -> Result<usize, HostError> {
    let args = execution_context(caller)?.args.clone();
    write_memory(caller, out_ptr, &args)?;
    Ok(args.len())
}

fn execution_context<'a>(
    caller: &'a Caller<'a, VmHostState>,
) -> Result<&'a ExecutionContext, HostError> {
    caller
        .data()
        .context
        .as_ref()
        .ok_or(HostError::MissingContext)
}

fn charge_gas(caller: &mut Caller<'_, VmHostState>, amount: u64) -> Result<(), HostError> {
    caller
        .set_fuel(
            caller
                .get_fuel()
                .map_err(|error| HostError::Gas(error.to_string()))?
                .checked_sub(amount)
                .ok_or_else(|| HostError::Gas("out of fuel".to_string()))?,
        )
        .map_err(|error| HostError::Gas(error.to_string()))
}

fn read_address(caller: &mut Caller<'_, VmHostState>, ptr: i32) -> Result<Address, HostError> {
    let bytes = read_memory(caller, ptr, Address::default().len() as i32)?;
    let mut address = [0_u8; 32];
    address.copy_from_slice(&bytes);
    Ok(address)
}

fn memory(caller: &mut Caller<'_, VmHostState>) -> Result<Memory, HostError> {
    caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
        .ok_or(HostError::MissingMemory)
}

fn read_memory(
    caller: &mut Caller<'_, VmHostState>,
    ptr: i32,
    len: i32,
) -> Result<Vec<u8>, HostError> {
    let offset = checked_offset(ptr, len)?;
    let memory = memory(caller)?;
    let data = memory.data(caller);
    Ok(data
        .get(offset.clone())
        .ok_or(HostError::OutOfBounds)?
        .to_vec())
}

fn write_memory(
    caller: &mut Caller<'_, VmHostState>,
    ptr: i32,
    bytes: &[u8],
) -> Result<(), HostError> {
    let offset = checked_offset(ptr, bytes.len() as i32)?;
    let memory = memory(caller)?;
    let data = memory.data_mut(caller);
    let destination = data.get_mut(offset).ok_or(HostError::OutOfBounds)?;
    destination.copy_from_slice(bytes);
    Ok(())
}

fn checked_offset(ptr: i32, len: i32) -> Result<std::ops::Range<usize>, HostError> {
    if ptr < 0 || len < 0 {
        return Err(HostError::NegativeOffset);
    }

    let start = ptr as usize;
    let length = len as usize;
    let end = start.checked_add(length).ok_or(HostError::OutOfBounds)?;
    Ok(start..end)
}

#[cfg(test)]
mod tests {
    use super::{ExecutionContext, HostError, VmHostState, checked_offset};
    use crate::transaction::address_from_name;

    #[test]
    fn checked_offset_rejects_negative_values() {
        assert_eq!(
            checked_offset(-1, 4).unwrap_err(),
            HostError::NegativeOffset
        );
        assert_eq!(
            checked_offset(0, -1).unwrap_err(),
            HostError::NegativeOffset
        );
    }

    #[test]
    fn execution_context_initializes_cleanly() {
        let caller = address_from_name("alice");
        let contract = address_from_name("contract");
        let context = ExecutionContext::new(caller, contract, 7, vec![1, 2, 3]);

        assert_eq!(context.caller, caller);
        assert_eq!(context.contract, contract);
        assert_eq!(context.deposit, 7);
        assert_eq!(context.args, vec![1, 2, 3]);
        assert!(!context.reverted);
        assert!(context.revert_message.is_none());
        assert!(context.logs.is_empty());
    }

    #[test]
    fn host_state_defaults_to_empty_world_state() {
        let state = VmHostState::default();
        assert_eq!(state.state.get_balance(&address_from_name("nobody")), 0);
        assert!(state.context.is_none());
    }
}
