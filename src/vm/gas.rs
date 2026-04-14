pub const BASE_TX_COST: u64 = 1_000;
pub const STORAGE_READ_COST: u64 = 500;
pub const STORAGE_WRITE_COST: u64 = 2_000;
pub const STORAGE_REMOVE_COST: u64 = 500;
pub const TRANSFER_COST: u64 = 1_000;
pub const DEPLOY_COST_PER_BYTE: u64 = 10;
pub const DEFAULT_GAS_LIMIT: u64 = 1_000_000;

pub fn deploy_cost(code_len: usize) -> u64 {
    (code_len as u64).saturating_mul(DEPLOY_COST_PER_BYTE)
}
