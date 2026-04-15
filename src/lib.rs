#![deny(missing_docs)]

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

/// Application-facing helpers for the demo binary and interactive REPL.
pub mod app;
/// Block structure, hashing, and proof-of-work utilities.
pub mod block;
/// Shared contract call ABI utilities.
pub mod call_abi;
/// Blockchain orchestration across blocks, state, receipts, and mining.
pub mod chain;
/// Transaction receipt types and hashing helpers.
pub mod receipt;
/// In-memory world-state types and transition logic.
pub mod state;
/// Transaction, payload, and address types.
pub mod transaction;
/// Wasm virtual machine integration, gas accounting, and host bindings.
pub mod vm;

pub use block::{Block, Hash, hash_to_hex};
pub use call_abi::{CallAbiError, CallEnvelope};
pub use chain::{Blockchain, BlockyError};
pub use receipt::{Receipt, transaction_hash};
pub use state::{AccountState, StateError, WorldState};
pub use transaction::{Address, Payload, Transaction, address_from_name, address_to_hex};
pub use vm::{ExecutionContext, HostError, VmEngine, VmError, VmHostState};
