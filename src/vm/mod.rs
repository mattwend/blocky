use std::{collections::HashMap, sync::Arc};

use anyhow::Error as AnyhowError;
use sha2::{Digest, Sha256};
use thiserror::Error;
use wasmtime::{Config, Engine, Linker, Module, Store};

use crate::{Address, Hash};

pub mod gas;
pub mod host;

pub use host::{ExecutionContext, HostError, VmHostState};

struct PreparedCall {
    store: Store<VmHostState>,
    method: String,
}

pub struct VmEngine {
    engine: Engine,
    linker: Linker<VmHostState>,
    module_cache: HashMap<Hash, Arc<Module>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GasReport {
    pub gas_limit: u64,
    pub gas_used: u64,
}

#[derive(Debug, Clone)]
pub struct CallRequest<'a> {
    pub state: crate::WorldState,
    pub caller: Address,
    pub contract: Address,
    pub deposit: u64,
    pub method: &'a str,
    pub args: &'a [u8],
    pub code: &'a [u8],
    pub gas_limit: u64,
}

#[derive(Debug, Error)]
pub enum VmError {
    #[error("failed to configure wasmtime engine: {0}")]
    EngineConfig(#[from] wasmtime::Error),
    #[error(transparent)]
    Host(#[from] host::HostError),
    #[error("contract method not found: {0}")]
    MissingMethod(String),
    #[error("contract execution aborted")]
    Aborted,
    #[error("contract execution aborted: {0}")]
    AbortedWithMessage(String),
}

impl VmEngine {
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

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn linker(&self) -> &Linker<VmHostState> {
        &self.linker
    }

    pub fn module_cache_len(&self) -> usize {
        self.module_cache.len()
    }

    pub fn cache_module(&mut self, code: &[u8]) -> Result<Hash, VmError> {
        let code_hash = code_hash(code);
        if self.module_cache.contains_key(&code_hash) {
            return Ok(code_hash);
        }

        let module = Module::new(&self.engine, code)?;
        self.module_cache.insert(code_hash, Arc::new(module));
        Ok(code_hash)
    }

    pub fn get_cached_module(&self, code_hash: &Hash) -> Option<Arc<Module>> {
        self.module_cache.get(code_hash).cloned()
    }

    pub fn prepare_module(&mut self, code: &[u8]) -> Result<Arc<Module>, VmError> {
        let code_hash = self.cache_module(code)?;
        self.get_cached_module(&code_hash)
            .ok_or_else(|| VmError::EngineConfig(wasmtime::Error::msg("cached module missing")))
    }

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
    error.downcast_ref::<AnyhowError>().map(ToString::to_string)
}

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
    fn code_hash_is_deterministic() {
        assert_eq!(code_hash(EMPTY_MODULE), code_hash(EMPTY_MODULE));
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
