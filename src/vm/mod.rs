use std::{collections::HashMap, sync::Arc};

use sha2::{Digest, Sha256};
use thiserror::Error;
use wasmtime::{Config, Engine, Linker, Module};

use crate::Hash;

pub mod host;

pub use host::{ExecutionContext, HostError, VmHostState};

pub struct VmEngine {
    engine: Engine,
    linker: Linker<VmHostState>,
    module_cache: HashMap<Hash, Arc<Module>>,
}

#[derive(Debug, Error)]
pub enum VmError {
    #[error("failed to configure wasmtime engine: {0}")]
    EngineConfig(#[from] wasmtime::Error),
    #[error(transparent)]
    Host(#[from] host::HostError),
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
}

fn deterministic_config() -> Result<Config, wasmtime::Error> {
    let mut config = Config::new();
    config.wasm_multi_memory(false);
    config.wasm_multi_value(true);
    config.wasm_reference_types(false);
    config.wasm_simd(false);
    config.wasm_tail_call(false);
    config.wasm_threads(false);
    config.wasm_relaxed_simd(false);
    config.wasm_bulk_memory(true);
    config.wasm_function_references(false);
    config.consume_fuel(true);
    config.cranelift_nan_canonicalization(true);
    Ok(config)
}

pub fn code_hash(code: &[u8]) -> Hash {
    let digest = Sha256::digest(code);
    let mut hash = [0_u8; 32];
    hash.copy_from_slice(&digest);
    hash
}

#[cfg(test)]
mod tests {
    use super::{VmEngine, code_hash};

    const EMPTY_MODULE: &[u8] = &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];

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
}
