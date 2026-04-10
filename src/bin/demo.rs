use blocky::{
    BlockyError,
    app::demo::{build_demo_blockchain, render_chain},
};
use tracing::{error, info};

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

fn run() -> Result<(), BlockyError> {
    let difficulty = 8;
    let mut blockchain = build_demo_blockchain(difficulty)?;

    info!(
        blocks = blockchain.chain.len(),
        "rendering demo blockchain state"
    );
    println!("== Blocky Demo ==");
    print!("{}", render_chain(&blockchain));
    println!("Chain valid: {}", blockchain.is_valid());

    info!("tampering with first mined block to demonstrate validation failure");
    println!("\nTampering with the first mined block...");
    if let Some(block) = blockchain.chain.get_mut(1)
        && let Some(transaction) = block.transactions.get_mut(0)
    {
        transaction.amount += 1;
    }
    println!("Chain valid after tampering: {}", blockchain.is_valid());

    Ok(())
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_target(false)
        .without_time()
        .with_writer(std::io::stderr)
        .init();
}
