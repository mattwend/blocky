//! Minimal counter contract example for the blocky REPL.
//!
//! The contract stores a single `u64` under the `count` key. Call `increment`
//! to increase the counter and emit a log, then call `get` to print the current
//! value without changing state.

use blocky_sdk::{log, storage};

const COUNT_KEY: &str = "count";

/// Increments the stored counter and logs the new value.
#[unsafe(no_mangle)]
pub extern "C" fn increment() {
    let current: u64 = storage::read(COUNT_KEY).unwrap_or(0);
    let next = current + 1;
    storage::write(COUNT_KEY, &next);
    log(&format!("count = {}", next));
}

/// Logs the current counter value.
#[unsafe(no_mangle)]
pub extern "C" fn get() {
    let current: u64 = storage::read(COUNT_KEY).unwrap_or(0);
    log(&format!("count = {}", current));
}
