//! Interactive terminal UI binary for blocky.
//!
//! This target starts the teaching REPL so you can queue transactions, mine
//! blocks, deploy contracts, and inspect receipts from a text interface.

use blocky::app::repl::{Repl, ReplError};
use tracing::error;

/// Starts the REPL binary, initializes tracing, and exits with a non-zero code
/// when interactive setup or execution fails.
fn main() {
    init_tracing();

    match run() {
        Ok(()) => {}
        Err(error) => {
            error!(error = %error, "repl execution failed");
            std::process::exit(1);
        }
    }
}

/// Creates the REPL with the default demo difficulty and runs the UI loop.
///
/// # Returns
/// `Ok(())` when the REPL exits cleanly, or a [`ReplError`] if startup or
/// runtime execution fails.
fn run() -> Result<(), ReplError> {
    let mut repl = Repl::try_new(8)?;
    repl.run()
}

/// Configures default tracing output for the REPL binary.
fn init_tracing() {
    tracing_subscriber::fmt()
        .with_target(false)
        .without_time()
        .init();
}
