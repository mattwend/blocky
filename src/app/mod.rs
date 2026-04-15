/// Printable demo helpers used by the `demo` binary.
pub mod demo;
/// Interactive terminal REPL implementation.
pub mod repl;

use crate::{Address, address_to_hex};

/// Formats an address as a short lowercase hexadecimal prefix for UI display.
///
/// # Arguments
/// - `address`: Address to shorten.
///
/// # Returns
/// The first eight lowercase hexadecimal characters of `address`.
pub fn short_address(address: &Address) -> String {
    address_to_hex(address).chars().take(8).collect()
}

/// Formats a hash as a short lowercase hexadecimal prefix for UI display.
///
/// # Arguments
/// - `hash`: Hash to shorten.
///
/// # Returns
/// The first eight lowercase hexadecimal characters of `hash`.
pub fn short_hash(hash: &[u8; 32]) -> String {
    hex::encode(hash).chars().take(8).collect()
}
