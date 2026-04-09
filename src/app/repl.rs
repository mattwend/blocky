use std::io::{self, Write};

use thiserror::Error;

use crate::{Blockchain, BlockyError, Transaction, app::demo::render_chain};

#[derive(Debug, Error)]
pub enum ReplError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Blocky(#[from] BlockyError),
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("usage: add <sender> <receiver> <amount>")]
    InvalidAddUsage,
    #[error("invalid amount: {0}")]
    InvalidAmount(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplCommand {
    Add {
        sender: String,
        receiver: String,
        amount: u64,
    },
    Mine,
    Print,
    Validate,
    Help,
    Quit,
}

pub struct Repl {
    blockchain: Blockchain,
}

impl Repl {
    pub fn new(difficulty: u32) -> Self {
        Self {
            blockchain: Blockchain::new(difficulty),
        }
    }

    pub fn run(&mut self) -> Result<(), ReplError> {
        self.print_help();

        loop {
            print!("blocky> ");
            io::stdout().flush()?;

            let mut input = String::new();
            let bytes_read = io::stdin().read_line(&mut input)?;
            if bytes_read == 0 {
                return Ok(());
            }

            let trimmed = input.trim();
            if trimmed.is_empty() {
                continue;
            }

            match self.execute_line(trimmed) {
                Ok(true) => return Ok(()),
                Ok(false) => {}
                Err(error) => println!("Error: {error}"),
            }
        }
    }

    pub fn execute_line(&mut self, line: &str) -> Result<bool, ReplError> {
        match parse_command(line)? {
            ReplCommand::Add {
                sender,
                receiver,
                amount,
            } => {
                self.blockchain
                    .add_transaction(Transaction::new(sender, receiver, amount))?;
                println!("Transaction queued.");
                Ok(false)
            }
            ReplCommand::Mine => {
                let block = self.blockchain.mine_pending()?;
                println!("Mined block with nonce {}.", block.nonce);
                Ok(false)
            }
            ReplCommand::Print => {
                print!("{}", render_chain(&self.blockchain));
                Ok(false)
            }
            ReplCommand::Validate => {
                println!("Chain valid: {}", self.blockchain.is_valid());
                Ok(false)
            }
            ReplCommand::Help => {
                self.print_help();
                Ok(false)
            }
            ReplCommand::Quit => Ok(true),
        }
    }

    fn print_help(&self) {
        println!("Commands:");
        println!("  add <sender> <receiver> <amount>");
        println!("  mine");
        println!("  print");
        println!("  validate");
        println!("  help");
        println!("  quit");
    }
}

pub fn parse_command(line: &str) -> Result<ReplCommand, ReplError> {
    let mut parts = line.split_whitespace();
    let Some(command) = parts.next() else {
        return Err(ReplError::UnknownCommand(String::new()));
    };

    match command {
        "add" => {
            let sender = parts.next().ok_or(ReplError::InvalidAddUsage)?.to_string();
            let receiver = parts.next().ok_or(ReplError::InvalidAddUsage)?.to_string();
            let amount_raw = parts.next().ok_or(ReplError::InvalidAddUsage)?;

            if parts.next().is_some() {
                return Err(ReplError::InvalidAddUsage);
            }

            let amount = amount_raw
                .parse::<u64>()
                .map_err(|_| ReplError::InvalidAmount(amount_raw.to_string()))?;

            Ok(ReplCommand::Add {
                sender,
                receiver,
                amount,
            })
        }
        "mine" => Ok(ReplCommand::Mine),
        "print" => Ok(ReplCommand::Print),
        "validate" => Ok(ReplCommand::Validate),
        "help" => Ok(ReplCommand::Help),
        "quit" | "exit" => Ok(ReplCommand::Quit),
        other => Err(ReplError::UnknownCommand(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::{Repl, ReplCommand, parse_command};

    #[test]
    fn parses_add_command() {
        let command = parse_command("add alice bob 42").unwrap();
        assert_eq!(
            command,
            ReplCommand::Add {
                sender: "alice".into(),
                receiver: "bob".into(),
                amount: 42,
            }
        );
    }

    #[test]
    fn rejects_invalid_add_command() {
        assert!(parse_command("add alice bob").is_err());
    }

    #[test]
    fn repl_can_execute_happy_path() {
        let mut repl = Repl::new(4);

        assert!(!repl.execute_line("add alice bob 5").unwrap());
        assert!(!repl.execute_line("mine").unwrap());
        assert!(!repl.execute_line("validate").unwrap());
        assert!(repl.execute_line("quit").unwrap());
    }
}
