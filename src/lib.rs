//! Blocky is a small educational blockchain and smart-contract playground written in Rust.
//!
//! The crate is organized into a few learner-friendly layers:
//! - [`transaction`] defines addresses and transaction payloads.
//! - [`block`] defines mined blocks and proof-of-work hashing helpers.
//! - [`state`] applies transactions to an in-memory world state.
//! - [`chain`] ties blocks, pending transactions, receipts, and validation together.
//! - [`vm`] executes Wasm smart contracts against the world state.
//! - [`call_abi`] exposes the Borsh call envelope shared with `blocky-sdk`.
//!
//! If you are exploring the project for the first time, a good path is:
//! [`Transaction`] -> [`Block`] -> [`WorldState`] -> [`Blockchain`] -> [`VmEngine`].

pub mod app;
pub mod block;
pub mod call_abi;
pub mod chain;
pub mod receipt;
pub mod state;
pub mod transaction;
pub mod vm;

pub use block::{Block, Hash, hash_to_hex};
pub use call_abi::{CallAbiError, CallEnvelope};
pub use chain::{Blockchain, BlockyError};
pub use receipt::{Receipt, transaction_hash};
pub use state::{AccountState, StateError, WorldState};
pub use transaction::{Address, Payload, Transaction, address_from_name, address_to_hex};
pub use vm::{ExecutionContext, HostError, VmEngine, VmError, VmHostState};
