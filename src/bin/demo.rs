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

    info!(
        blocks = blockchain.chain.len(),
        "rendering demo blockchain state"
    );
    println!("== Blocky Demo ==");
    print!("{}", render_chain(&blockchain)?);
    println!("Chain valid: {}", blockchain.is_valid()?);

    info!("tampering with first mined block to demonstrate validation failure");
    println!("\nTampering with the first mined block...");
    if let Some(block) = blockchain.chain.get_mut(1)
        && let Some(transaction) = block.transactions.get_mut(0)
        && let Payload::Transfer { amount, .. } = &mut transaction.payload
    {
        *amount += 1;
    }
    println!("Chain valid after tampering: {}", blockchain.is_valid()?);

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
