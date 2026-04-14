//! Command-line walkthrough binary for blocky.
//!
//! This target builds a small demonstration chain, renders it, then mutates one
//! mined transaction to show how validation detects tampering. It is the
//! quickest way to see the core blockchain flow without using the interactive
//! REPL.

use blocky::{
    BlockyError, Payload,
    app::demo::{build_demo_blockchain, render_chain},
};
use tracing::{error, info};

/// Starts the demo binary, initializes tracing, and exits with a non-zero code
/// when the walkthrough fails.
fn main() {
    init_tracing();

    match run() {
        Ok(()) => {}
        Err(error) => {
            error!(error = %error, "demo execution failed");
            std::process::exit(1);
        }
    }
}

/// Runs the printable demo workflow.
///
/// # Returns
/// `Ok(())` when the demo renders and the tampering check completes
/// successfully, or a [`BlockyError`] when blockchain setup or validation
/// fails.
fn run() -> Result<(), BlockyError> {
    let difficulty = 8;
    let mut blockchain = build_demo_blockchain(difficulty)?;

    let rendered_chain = render_chain(&blockchain)?;
    let chain_valid = blockchain.is_valid()?;
    info!(
        blocks = blockchain.chain.len(),
        rendered_chain = %rendered_chain,
        chain_valid,
        "rendering demo blockchain state"
    );
    info!("== Blocky Demo ==");

    info!("tampering with first mined block to demonstrate validation failure");
    if let Some(block) = blockchain.chain.get_mut(1)
        && let Some(transaction) = block.transactions.get_mut(0)
        && let Payload::Transfer { amount, .. } = &mut transaction.payload
    {
        *amount += 1;
    }
    let chain_valid_after_tampering = blockchain.is_valid()?;
    info!(chain_valid_after_tampering, "demo state after tampering");

    Ok(())
}

/// Configures stderr logging for the demo binary.
fn init_tracing() {
    tracing_subscriber::fmt()
        .with_target(false)
        .without_time()
        .with_writer(std::io::stderr)
        .init();
}
