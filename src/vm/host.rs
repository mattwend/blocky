use thiserror::Error;
use tracing::warn;
use wasmtime::{Caller, Linker, Memory};

use crate::{Address, WorldState};

use super::gas::{STORAGE_READ_COST, STORAGE_REMOVE_COST, STORAGE_WRITE_COST, TRANSFER_COST};

/// Runtime context captured for a contract execution.
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// External account that initiated the call.
    pub caller: Address,
    /// Contract currently being executed.
    pub contract: Address,
    /// Balance transferred into the contract before execution.
    pub deposit: u64,
    /// Raw input bytes visible to the guest.
    pub args: Vec<u8>,
    /// Whether execution requested a revert/abort.
    pub reverted: bool,
    /// Optional revert message captured from the guest.
    pub revert_message: Option<String>,
    /// Log lines emitted by the guest.
    pub logs: Vec<String>,
}

impl ExecutionContext {
    /// Creates a fresh execution context for a contract call.
    ///
    /// # Arguments
    /// - `caller`: External account that initiated the call.
    /// - `contract`: Contract address being executed.
    /// - `deposit`: Balance transferred into the contract before execution.
    /// - `args`: Raw input bytes passed to the guest.
    ///
    /// # Returns
    /// A clean execution context with no logs or revert status.
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

/// Mutable host state stored inside a Wasmtime store during execution.
#[derive(Debug, Clone, Default)]
pub struct VmHostState {
    /// World state being mutated by the guest call.
    pub state: WorldState,
    /// Current execution context, if one has been initialized.
    pub context: Option<ExecutionContext>,
}

/// Errors raised by VM host functions when interacting with guest memory or state.
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
    #[error("state operation failed: {0}")]
    State(String),
    #[error("failed to charge gas: {0}")]
    Gas(String),
}

/// Registers all host functions exposed to guest Wasm contracts.
///
/// # Arguments
/// - `linker`: Linker to populate with `env` imports.
///
/// # Returns
/// `Ok(())` if all functions were registered successfully, or a Wasmtime error otherwise.
pub fn link_host_functions(linker: &mut Linker<VmHostState>) -> Result<(), wasmtime::Error> {
    linker.func_wrap(
        "env",
        "storage_read",
        |mut caller: Caller<'_, VmHostState>,
         key_ptr: i32,
         key_len: i32,
         val_ptr: i32|
         -> Result<i32, wasmtime::Error> {
            storage_read(&mut caller, key_ptr, key_len, val_ptr)
                .map(|result| result.map_or(-1, |length| length as i32))
                .map_err(host_trap)
        },
    )?;
    linker.func_wrap(
        "env",
        "storage_write",
        |mut caller: Caller<'_, VmHostState>,
         key_ptr: i32,
         key_len: i32,
         val_ptr: i32,
         val_len: i32|
         -> Result<(), wasmtime::Error> {
            storage_write(&mut caller, key_ptr, key_len, val_ptr, val_len).map_err(host_trap)
        },
    )?;
    linker.func_wrap(
        "env",
        "storage_remove",
        |mut caller: Caller<'_, VmHostState>,
         key_ptr: i32,
         key_len: i32|
         -> Result<i32, wasmtime::Error> {
            storage_remove(&mut caller, key_ptr, key_len)
                .map(|removed| if removed { 1 } else { 0 })
                .map_err(host_trap)
        },
    )?;
    linker.func_wrap(
        "env",
        "get_balance",
        |caller: Caller<'_, VmHostState>| -> Result<i64, wasmtime::Error> {
            get_balance(&caller)
                .map(|balance| balance as i64)
                .map_err(host_trap)
        },
    )?;
    linker.func_wrap(
        "env",
        "get_caller",
        |mut caller: Caller<'_, VmHostState>, out_ptr: i32| -> Result<(), wasmtime::Error> {
            get_caller(&mut caller, out_ptr).map_err(host_trap)
        },
    )?;
    linker.func_wrap(
        "env",
        "get_deposit",
        |caller: Caller<'_, VmHostState>| -> Result<i64, wasmtime::Error> {
            get_deposit(&caller)
                .map(|deposit| deposit as i64)
                .map_err(host_trap)
        },
    )?;
    linker.func_wrap(
        "env",
        "transfer",
        |mut caller: Caller<'_, VmHostState>,
         to_ptr: i32,
         amount: i64|
         -> Result<i32, wasmtime::Error> {
            if amount < 0 {
                return Err(host_trap(HostError::State(
                    "transfer amount must be non-negative".to_string(),
                )));
            }
            transfer(&mut caller, to_ptr, amount as u64)
                .map(|()| 0)
                .map_err(host_trap)
        },
    )?;
    linker.func_wrap(
        "env",
        "input_len",
        |caller: Caller<'_, VmHostState>| -> Result<i32, wasmtime::Error> {
            input_len(&caller).map_err(host_trap)
        },
    )?;
    linker.func_wrap(
        "env",
        "read_input",
        |mut caller: Caller<'_, VmHostState>, out_ptr: i32| -> Result<i32, wasmtime::Error> {
            read_input(&mut caller, out_ptr)
                .map(|length| length as i32)
                .map_err(host_trap)
        },
    )?;
    linker.func_wrap(
        "env",
        "log",
        |mut caller: Caller<'_, VmHostState>,
         msg_ptr: i32,
         msg_len: i32|
         -> Result<(), wasmtime::Error> {
            let message_bytes = read_memory(&mut caller, msg_ptr, msg_len).map_err(host_trap)?;
            let message = String::from_utf8_lossy(&message_bytes).into_owned();
            let context = caller
                .data_mut()
                .context
                .as_mut()
                .ok_or_else(|| host_trap(HostError::MissingContext))?;
            context.logs.push(message);
            Ok(())
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

            Err(wasmtime::Error::msg(message))
        },
    )?;
    Ok(())
}

/// Reads a contract storage value into guest memory.
///
/// # Arguments
/// - `caller`: Active Wasmtime caller for the guest instance.
/// - `key_ptr`: Pointer to the storage key in guest memory.
/// - `key_len`: Length of the storage key in bytes.
/// - `val_ptr`: Pointer where the storage value should be written.
///
/// # Returns
/// The value length when the key exists, `None` when absent, or a host error.
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

/// Writes a contract storage value from guest memory.
///
/// # Arguments
/// - `caller`: Active Wasmtime caller for the guest instance.
/// - `key_ptr`: Pointer to the storage key in guest memory.
/// - `key_len`: Length of the storage key in bytes.
/// - `val_ptr`: Pointer to the value bytes in guest memory.
/// - `val_len`: Length of the value in bytes.
///
/// # Returns
/// `Ok(())` if the value was written successfully, or a host error.
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

/// Removes a contract storage key.
///
/// # Arguments
/// - `caller`: Active Wasmtime caller for the guest instance.
/// - `key_ptr`: Pointer to the storage key in guest memory.
/// - `key_len`: Length of the storage key in bytes.
///
/// # Returns
/// `true` if a value was removed, `false` if the key was absent, or a host error.
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

/// Returns the executing contract's balance.
///
/// # Arguments
/// - `caller`: Active Wasmtime caller for the guest instance.
///
/// # Returns
/// The current contract balance, or a host error.
pub fn get_balance(caller: &Caller<'_, VmHostState>) -> Result<u64, HostError> {
    let contract = execution_context(caller)?.contract;
    Ok(caller.data().state.get_balance(&contract))
}

/// Writes the external caller address into guest memory.
///
/// # Arguments
/// - `caller`: Active Wasmtime caller for the guest instance.
/// - `out_ptr`: Pointer where the 32-byte caller address should be written.
///
/// # Returns
/// `Ok(())` if the address was written successfully, or a host error.
pub fn get_caller(caller: &mut Caller<'_, VmHostState>, out_ptr: i32) -> Result<(), HostError> {
    let address = execution_context(caller)?.caller;
    write_memory(caller, out_ptr, &address)
}

/// Returns the deposit transferred into the current contract call.
///
/// # Arguments
/// - `caller`: Active Wasmtime caller for the guest instance.
///
/// # Returns
/// The current call deposit, or a host error.
pub fn get_deposit(caller: &Caller<'_, VmHostState>) -> Result<u64, HostError> {
    Ok(execution_context(caller)?.deposit)
}

/// Transfers balance from the executing contract to another address.
///
/// # Arguments
/// - `caller`: Active Wasmtime caller for the guest instance.
/// - `to_ptr`: Pointer to the 32-byte destination address in guest memory.
/// - `amount`: Amount of balance to transfer.
///
/// # Returns
/// `Ok(())` if the transfer succeeds, or a host error.
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
        .map_err(|error| HostError::State(error.to_string()))?;
    Ok(())
}

/// Returns the length of the current call input.
///
/// # Arguments
/// - `caller`: Active Wasmtime caller for the guest instance.
///
/// # Returns
/// The input length in bytes, or a host error.
pub fn input_len(caller: &Caller<'_, VmHostState>) -> Result<i32, HostError> {
    let len = execution_context(caller)?.args.len();
    i32::try_from(len).map_err(|_| HostError::OutOfBounds)
}

/// Writes the current call input into guest memory.
///
/// # Arguments
/// - `caller`: Active Wasmtime caller for the guest instance.
/// - `out_ptr`: Pointer where the input bytes should be written.
///
/// # Returns
/// The number of bytes written, or a host error.
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

fn host_trap(error: HostError) -> wasmtime::Error {
    warn!(error = %error, "vm host function failed");
    wasmtime::Error::msg(error.to_string())
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
    let bytes = read_memory(caller, ptr, std::mem::size_of::<Address>() as i32)?;
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
    fn execution_context_starts_non_reverted_with_no_logs() {
        let caller = address_from_name("alice");
        let contract = address_from_name("contract");
        let context = ExecutionContext::new(caller, contract, 7, vec![1, 2, 3]);

        assert!(!context.reverted);
        assert!(context.revert_message.is_none());
        assert!(context.logs.is_empty());
    }

    #[test]
    fn host_state_can_store_context() {
        let caller = address_from_name("alice");
        let contract = address_from_name("contract");
        let state = VmHostState {
            context: Some(ExecutionContext::new(caller, contract, 7, vec![1, 2, 3])),
            ..VmHostState::default()
        };

        let context = state.context.as_ref().unwrap();
        assert_eq!(context.caller, caller);
        assert_eq!(context.contract, contract);
        assert_eq!(context.deposit, 7);
        assert_eq!(context.args, vec![1, 2, 3]);
    }
}
