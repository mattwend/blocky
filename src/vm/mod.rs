use std::{collections::HashMap, sync::Arc};

use sha2::{Digest, Sha256};
use thiserror::Error;
use wasmtime::{Config, Engine, Linker, Module, Store};

use crate::{Address, Hash};

/// Gas schedule constants and helpers for VM execution.
pub mod gas;
/// Host bindings exposed to guest Wasm contracts.
pub mod host;

pub use host::{ExecutionContext, HostError, VmHostState};

struct PreparedCall {
    store: Store<VmHostState>,
    method: String,
}

/// Wasmtime-based execution engine with a cache of compiled contract modules.
pub struct VmEngine {
    engine: Engine,
    linker: Linker<VmHostState>,
    module_cache: HashMap<Hash, Arc<Module>>,
}

/// Gas accounting summary for a contract call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GasReport {
    /// Maximum gas made available to the call.
    pub gas_limit: u64,
    /// Gas actually consumed by the call.
    pub gas_used: u64,
}

/// All inputs required to execute a single contract call.
#[derive(Debug, Clone)]
pub struct CallRequest<'a> {
    /// World state snapshot to execute against.
    pub state: crate::WorldState,
    /// External account that initiated the call.
    pub caller: Address,
    /// Contract address being executed.
    pub contract: Address,
    /// Balance transferred into the contract before execution.
    pub deposit: u64,
    /// Exported guest method to invoke.
    pub method: &'a str,
    /// Encoded input bytes passed to the guest.
    pub args: &'a [u8],
    /// Wasm bytecode for the target contract.
    pub code: &'a [u8],
    /// Maximum gas available to the call.
    pub gas_limit: u64,
}

/// Errors produced while preparing or executing a Wasm contract call.
#[derive(Debug, Error)]
pub enum VmError {
    /// Wasmtime engine setup, module compilation, instantiation, or execution failed.
    #[error("failed to configure wasmtime engine: {0}")]
    EngineConfig(#[from] wasmtime::Error),
    /// A host function failed while the guest was executing.
    #[error(transparent)]
    Host(#[from] host::HostError),
    /// The requested exported guest method does not exist.
    #[error("contract method not found: {0}")]
    MissingMethod(String),
    /// The guest aborted execution without a message.
    #[error("contract execution aborted")]
    Aborted,
    /// The guest aborted execution and provided a message.
    #[error("contract execution aborted: {0}")]
    AbortedWithMessage(String),
}

impl VmEngine {
    /// Creates a new VM engine with deterministic Wasmtime settings and linked host functions.
    ///
    /// # Returns
    /// A ready-to-use VM engine, or an error if initialization fails.
    pub fn new() -> Result<Self, VmError> {
        let engine = Engine::new(&deterministic_config()?)?;
        let mut linker = Linker::new(&engine);
        host::link_host_functions(&mut linker)?;

        Ok(Self {
            engine,
            linker,
            module_cache: HashMap::new(),
        })
    }

    /// Returns the underlying Wasmtime engine.
    ///
    /// # Returns
    /// A shared reference to the configured engine.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Returns the linker containing all registered host functions.
    ///
    /// # Returns
    /// A shared reference to the VM linker.
    pub fn linker(&self) -> &Linker<VmHostState> {
        &self.linker
    }

    /// Returns the number of compiled modules currently cached.
    ///
    /// # Returns
    /// The current module cache size.
    pub fn module_cache_len(&self) -> usize {
        self.module_cache.len()
    }

    /// Compiles and caches a module if it is not already present.
    ///
    /// # Arguments
    /// - `code`: Raw Wasm bytecode to compile.
    ///
    /// # Returns
    /// The code hash used as the cache key, or an error if compilation fails.
    pub fn cache_module(&mut self, code: &[u8]) -> Result<Hash, VmError> {
        let code_hash = code_hash(code);
        if self.module_cache.contains_key(&code_hash) {
            return Ok(code_hash);
        }

        let module = Module::new(&self.engine, code)?;
        self.module_cache.insert(code_hash, Arc::new(module));
        Ok(code_hash)
    }

    /// Returns a previously cached compiled module.
    ///
    /// # Arguments
    /// - `code_hash`: Hash of the module bytecode.
    ///
    /// # Returns
    /// The cached module if present.
    pub fn get_cached_module(&self, code_hash: &Hash) -> Option<Arc<Module>> {
        self.module_cache.get(code_hash).cloned()
    }

    /// Ensures a module is cached and returns the compiled instance template.
    ///
    /// # Arguments
    /// - `code`: Raw Wasm bytecode to prepare.
    ///
    /// # Returns
    /// The compiled module, or an error if compilation or cache lookup fails.
    pub fn prepare_module(&mut self, code: &[u8]) -> Result<Arc<Module>, VmError> {
        let code_hash = self.cache_module(code)?;
        self.get_cached_module(&code_hash)
            .ok_or_else(|| VmError::EngineConfig(wasmtime::Error::msg("cached module missing")))
    }

    /// Executes a contract call against the provided world state snapshot.
    ///
    /// # Arguments
    /// - `request`: Complete description of the call to execute.
    ///
    /// # Returns
    /// The post-call world state, execution context, and gas report, or a VM error.
    pub fn execute_call_with_state(
        &mut self,
        request: CallRequest<'_>,
    ) -> Result<(crate::WorldState, ExecutionContext, GasReport), VmError> {
        let gas_limit = request.gas_limit;
        let PreparedCall { mut store, method } = self.prepare_call(request.clone())?;
        let instance = {
            let module = self.prepare_module(request.code)?;
            self.linker.instantiate(&mut store, &module)?
        };
        let function = instance
            .get_func(&mut store, &method)
            .ok_or_else(|| VmError::MissingMethod(method.clone()))?;
        if let Err(error) = function.call(&mut store, &[], &mut []) {
            return Err(execution_error(&store, error));
        }

        let remaining_fuel = store.get_fuel()?;
        let host_state = store.into_data();
        let context = finalize_context(host_state.context)?;
        let gas_report = GasReport {
            gas_limit,
            gas_used: gas_limit.saturating_sub(remaining_fuel),
        };

        Ok((host_state.state, context, gas_report))
    }

    fn prepare_call(&mut self, request: CallRequest<'_>) -> Result<PreparedCall, VmError> {
        let host_state = VmHostState {
            state: request.state,
            context: Some(ExecutionContext::new(
                request.caller,
                request.contract,
                request.deposit,
                request.args.to_vec(),
            )),
        };
        let mut store = Store::new(&self.engine, host_state);
        store.set_fuel(request.gas_limit)?;
        Ok(PreparedCall {
            store,
            method: request.method.to_string(),
        })
    }
}

fn finalize_context(context: Option<ExecutionContext>) -> Result<ExecutionContext, VmError> {
    let context = context.ok_or(VmError::Host(HostError::MissingContext))?;
    if context.reverted {
        return match context.revert_message {
            Some(message) => Err(VmError::AbortedWithMessage(message)),
            None => Err(VmError::Aborted),
        };
    }
    Ok(context)
}

fn execution_error(store: &Store<VmHostState>, error: wasmtime::Error) -> VmError {
    let message = store
        .data()
        .context
        .as_ref()
        .and_then(|context| context.revert_message.clone())
        .or_else(|| extract_error_message(&error));
    match message {
        Some(message) => VmError::AbortedWithMessage(message),
        None => VmError::EngineConfig(error),
    }
}

fn deterministic_config() -> Result<Config, wasmtime::Error> {
    let mut config = Config::new();
    config.wasm_multi_memory(false);
    config.wasm_multi_value(true);
    config.wasm_reference_types(true);
    config.wasm_simd(false);
    config.wasm_tail_call(false);
    config.wasm_threads(false);
    config.wasm_relaxed_simd(false);
    config.wasm_bulk_memory(true);
    config.wasm_function_references(true);
    config.consume_fuel(true);
    config.cranelift_nan_canonicalization(true);
    Ok(config)
}

fn extract_error_message(error: &wasmtime::Error) -> Option<String> {
    Some(error.to_string()).filter(|message| !message.is_empty())
}

/// Computes the code hash used to cache compiled Wasm modules.
///
/// # Arguments
/// - `code`: Raw Wasm bytecode to hash.
///
/// # Returns
/// A 32-byte SHA-256 digest of the code.
pub fn code_hash(code: &[u8]) -> Hash {
    let digest = Sha256::digest(code);
    let mut hash = [0_u8; 32];
    hash.copy_from_slice(&digest);
    hash
}

#[cfg(test)]
mod tests {
    use super::{CallRequest, VmEngine, code_hash};
    use crate::{transaction::address_from_name, vm::gas::DEFAULT_GAS_LIMIT};

    const EMPTY_MODULE: &[u8] = &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    const NOOP_MODULE: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00, 0x03,
        0x02, 0x01, 0x00, 0x07, 0x08, 0x01, 0x04, 0x6e, 0x6f, 0x6f, 0x70, 0x00, 0x00, 0x0a, 0x04,
        0x01, 0x02, 0x00, 0x0b,
    ];

    #[test]
    fn equal_code_bytes_hash_to_same_value() {
        let bytes_a = EMPTY_MODULE.to_vec();
        let bytes_b = EMPTY_MODULE.to_vec();

        assert_eq!(code_hash(&bytes_a), code_hash(&bytes_b));
    }

    #[test]
    fn caches_modules_by_code_hash() {
        let mut vm = VmEngine::new().unwrap();

        let first = vm.cache_module(EMPTY_MODULE).unwrap();
        let second = vm.cache_module(EMPTY_MODULE).unwrap();

        assert_eq!(first, second);
        assert_eq!(vm.module_cache_len(), 1);
        assert!(vm.get_cached_module(&first).is_some());
    }

    #[test]
    fn executes_exported_method() {
        let mut vm = VmEngine::new().unwrap();
        let caller = address_from_name("alice");
        let contract = address_from_name("contract");

        let (_, context, gas_report) = vm
            .execute_call_with_state(CallRequest {
                state: crate::WorldState::new(),
                caller,
                contract,
                deposit: 0,
                method: "noop",
                args: &[],
                code: NOOP_MODULE,
                gas_limit: DEFAULT_GAS_LIMIT,
            })
            .unwrap();

        assert_eq!(context.caller, caller);
        assert_eq!(context.contract, contract);
        assert!(context.args.is_empty());
        assert!(context.logs.is_empty());
        assert!(gas_report.gas_used <= gas_report.gas_limit);
    }
}
