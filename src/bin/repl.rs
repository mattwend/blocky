use blocky::app::repl::{Repl, ReplError};
use tracing::error;

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

fn run() -> Result<(), ReplError> {
    let mut repl = Repl::try_new(8)?;
    repl.run()
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_target(false)
        .without_time()
        .init();
}
