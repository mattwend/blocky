/// Base gas charged for every transaction before payload-specific work.
pub const BASE_TX_COST: u64 = 1_000;
/// Gas charged for one storage read host call.
pub const STORAGE_READ_COST: u64 = 500;
/// Gas charged for one storage write host call.
pub const STORAGE_WRITE_COST: u64 = 2_000;
/// Gas charged for one storage remove host call.
pub const STORAGE_REMOVE_COST: u64 = 500;
/// Gas charged for one balance transfer host call.
pub const TRANSFER_COST: u64 = 1_000;
/// Gas charged per byte of deployed Wasm code.
pub const DEPLOY_COST_PER_BYTE: u64 = 10;
/// Default gas limit assigned to contract calls.
pub const DEFAULT_GAS_LIMIT: u64 = 1_000_000;

/// Computes the gas cost of deploying a Wasm module.
///
/// # Arguments
/// - `code_len`: Length of the deployed Wasm bytecode in bytes.
///
/// # Returns
/// The total deployment gas cost for the provided byte length.
pub fn deploy_cost(code_len: usize) -> u64 {
    (code_len as u64).saturating_mul(DEPLOY_COST_PER_BYTE)
}
