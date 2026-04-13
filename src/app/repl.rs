use std::io;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use thiserror::Error;
use tracing::{debug, info};

use crate::{
    Blockchain, BlockyError, Transaction, address_from_name, address_to_hex,
    app::demo::render_chain,
};

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
    #[error("unterminated quoted string")]
    UnterminatedQuote,
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

const MAX_OUTPUT_LINES: usize = 500;
const MAX_HISTORY_ENTRIES: usize = 100;

pub struct Repl {
    blockchain: Blockchain,
    input: String,
    output: Vec<String>,
    scroll_offset: u16,
    history: Vec<String>,
    history_index: Option<usize>,
}

impl Repl {
    pub fn try_new(difficulty: u32) -> Result<Self, ReplError> {
        Ok(Self {
            blockchain: Blockchain::try_new(difficulty)?,
            input: String::new(),
            output: vec![
                "Welcome to Blocky REPL".to_string(),
                Self::help_text().to_string(),
            ],
            scroll_offset: 0,
            history: Vec::new(),
            history_index: None,
        })
    }

    pub fn run(&mut self) -> Result<(), ReplError> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_loop(&mut terminal);

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    pub fn execute_line(&mut self, line: &str) -> Result<bool, ReplError> {
        self.push_output(format!("> {line}"));
        info!(command = line, "executing repl command");

        match parse_command(line)? {
            ReplCommand::Add {
                sender,
                receiver,
                amount,
            } => {
                let sender_address = address_from_name(&sender);
                let receiver_address = address_from_name(&receiver);
                let nonce = self
                    .blockchain
                    .state
                    .get_account(&sender_address)
                    .map(|account| account.nonce)
                    .unwrap_or(0)
                    + self
                        .blockchain
                        .pending_transactions
                        .iter()
                        .filter(|pending| pending.sender == sender_address)
                        .count() as u64;
                self.blockchain.add_transaction(Transaction::new_transfer(
                    sender_address,
                    nonce,
                    receiver_address,
                    amount,
                ))?;
                info!(sender = %sender, receiver = %receiver, amount, "queued transaction");
                self.push_output("Transaction queued.".to_string());
                Ok(false)
            }
            ReplCommand::Mine => {
                let block = self.blockchain.mine_pending()?;
                info!(nonce = block.nonce, "mined pending transactions");
                self.push_output(format!("Mined block with nonce {}.", block.nonce));
                if let Some(receipts) = self.blockchain.receipts.last().cloned() {
                    for receipt in receipts {
                        self.push_output(format!(
                            "Receipt {} success={} gas={}",
                            short_hash(&receipt.tx_hash),
                            receipt.success,
                            receipt.gas_used
                        ));
                        for log in receipt.logs {
                            self.push_output(format!("  log: {log}"));
                        }
                        if let Some(error) = receipt.error {
                            self.push_output(format!("  error: {error}"));
                        }
                    }
                }
                Ok(false)
            }
            ReplCommand::Print => {
                debug!(
                    blocks = self.blockchain.chain.len(),
                    "rendering blockchain state in repl"
                );
                for line in render_chain(&self.blockchain).lines() {
                    self.push_output(line.to_string());
                }
                Ok(false)
            }
            ReplCommand::Validate => {
                let is_valid = self.blockchain.is_valid();
                info!(is_valid, "validated blockchain state");
                self.push_output(format!("Chain valid: {is_valid}"));
                Ok(false)
            }
            ReplCommand::Help => {
                self.push_output(Self::help_text().to_string());
                Ok(false)
            }
            ReplCommand::Quit => Ok(true),
        }
    }

    fn run_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<(), ReplError> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            if let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Char(c) => {
                        self.input.push(c);
                        self.history_index = None;
                    }
                    KeyCode::Backspace => {
                        self.input.pop();
                        self.history_index = None;
                    }
                    KeyCode::Enter => {
                        let line = self.input.trim().to_string();
                        self.input.clear();
                        self.history_index = None;

                        if line.is_empty() {
                            continue;
                        }

                        self.record_history(line.clone());

                        match self.execute_line(&line) {
                            Ok(true) => return Ok(()),
                            Ok(false) => {}
                            Err(error) => self.push_output(format!("Error: {error}")),
                        }
                    }
                    KeyCode::Up => {
                        if self.input.is_empty() {
                            self.scroll_offset = self.scroll_offset.saturating_add(1);
                        } else {
                            self.navigate_history_older();
                        }
                    }
                    KeyCode::Down => {
                        if self.input.is_empty() {
                            self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        } else {
                            self.navigate_history_newer();
                        }
                    }
                    KeyCode::PageUp => {
                        self.scroll_offset = self.scroll_offset.saturating_add(10);
                    }
                    KeyCode::PageDown => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(10);
                    }
                    KeyCode::Esc => return Ok(()),
                    _ => {}
                }
            }
        }
    }

    fn draw(&self, frame: &mut ratatui::Frame<'_>) {
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(3)])
            .split(frame.area());
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
            .split(vertical[0]);
        let sidebar = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),
                Constraint::Min(8),
                Constraint::Length(5),
            ])
            .split(horizontal[1]);

        let output_lines: Vec<Line<'_>> = self
            .output
            .iter()
            .map(|line| Line::from(line.as_str()))
            .collect();
        let output = Paragraph::new(Text::from(output_lines))
            .block(
                Block::default()
                    .title("Output")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset, 0));
        frame.render_widget(output, horizontal[0]);

        let status = Paragraph::new(self.status_lines())
            .block(
                Block::default()
                    .title("Status")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(status, sidebar[0]);

        let pending = Paragraph::new(self.pending_lines())
            .block(
                Block::default()
                    .title("Pending Transactions")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(pending, sidebar[1]);

        let help = Paragraph::new(self.help_lines())
            .style(Style::default().fg(Color::DarkGray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Help")
                    .border_style(Style::default().fg(Color::Magenta)),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(help, sidebar[2]);

        let input = Paragraph::new(self.input.as_str()).block(
            Block::default()
                .title("Input")
                .borders(Borders::ALL)
                .border_style(
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
        );
        frame.render_widget(input, vertical[1]);

        frame.set_cursor_position((
            vertical[1].x + self.input.len() as u16 + 1,
            vertical[1].y + 1,
        ));
    }

    fn push_output(&mut self, line: String) {
        self.output.push(line);
        if self.output.len() > MAX_OUTPUT_LINES {
            let excess = self.output.len() - MAX_OUTPUT_LINES;
            self.output.drain(0..excess);
        }
        self.scroll_offset = 0;
    }

    fn record_history(&mut self, line: String) {
        if self.history.last() == Some(&line) {
            return;
        }

        self.history.push(line);
        if self.history.len() > MAX_HISTORY_ENTRIES {
            let excess = self.history.len() - MAX_HISTORY_ENTRIES;
            self.history.drain(0..excess);
        }
    }

    fn navigate_history_older(&mut self) {
        if self.history.is_empty() {
            return;
        }

        let next_index = self
            .history_index
            .map(|index| index.saturating_sub(1))
            .unwrap_or(self.history.len().saturating_sub(1));
        self.history_index = Some(next_index);
        self.input = self.history[next_index].clone();
    }

    fn navigate_history_newer(&mut self) {
        let Some(current_index) = self.history_index else {
            return;
        };

        let next_index = current_index.saturating_add(1);
        if next_index >= self.history.len() {
            self.history_index = None;
            self.input.clear();
            return;
        }

        self.history_index = Some(next_index);
        self.input = self.history[next_index].clone();
    }

    fn status_lines(&self) -> Text<'_> {
        Text::from(vec![
            Line::from(format!("Blocks: {}", self.blockchain.chain.len())),
            Line::from(format!(
                "Pending: {}",
                self.blockchain.pending_transactions.len()
            )),
            Line::from(format!("Difficulty: {}", self.blockchain.difficulty)),
            Line::from(format!("Valid: {}", self.blockchain.is_valid())),
            Line::from(format!("History: {}", self.history.len())),
            Line::from(format!(
                "Alice: {}",
                self.blockchain
                    .state
                    .get_balance(&address_from_name("alice"))
            )),
            Line::from(format!(
                "Bob: {}",
                self.blockchain.state.get_balance(&address_from_name("bob"))
            )),
        ])
    }

    fn pending_lines(&self) -> Text<'_> {
        if self.blockchain.pending_transactions.is_empty() {
            return Text::from("(none)");
        }

        let lines = self
            .blockchain
            .pending_transactions
            .iter()
            .map(|transaction| {
                let description = match &transaction.payload {
                    crate::Payload::Transfer { receiver, amount } => format!(
                        "{} -> {}: {}",
                        short_address(&transaction.sender),
                        short_address(receiver),
                        amount
                    ),
                    crate::Payload::Deploy { .. } => {
                        format!("{} deploy", short_address(&transaction.sender))
                    }
                    crate::Payload::Call {
                        contract,
                        method,
                        deposit,
                        ..
                    } => format!(
                        "{} call {}.{} ({})",
                        short_address(&transaction.sender),
                        short_address(contract),
                        method,
                        deposit
                    ),
                };
                Line::from(description)
            })
            .collect::<Vec<_>>();

        Text::from(lines)
    }

    fn help_lines(&self) -> Text<'_> {
        Text::from(vec![
            Line::from("Enter: run command"),
            Line::from("Up/Down: scroll or history"),
            Line::from("PgUp/PgDn: fast scroll"),
            Line::from("Quotes: add \"alice a\" bob 4"),
            Line::from("Esc: quit"),
        ])
    }

    fn help_text() -> &'static str {
        "Commands: add <sender> <receiver> <amount> | mine | print | validate | help | quit"
    }
}

pub fn parse_command(line: &str) -> Result<ReplCommand, ReplError> {
    let parts = tokenize(line)?;
    let Some(command) = parts.first().map(String::as_str) else {
        return Err(ReplError::UnknownCommand(String::new()));
    };

    match command {
        "add" => {
            if parts.len() != 4 {
                return Err(ReplError::InvalidAddUsage);
            }

            let amount = parts[3]
                .parse::<u64>()
                .map_err(|_| ReplError::InvalidAmount(parts[3].clone()))?;

            Ok(ReplCommand::Add {
                sender: parts[1].clone(),
                receiver: parts[2].clone(),
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

fn short_address(address: &crate::Address) -> String {
    let hex = address_to_hex(address);
    hex.chars().take(8).collect()
}

fn short_hash(hash: &[u8; 32]) -> String {
    hex::encode(hash).chars().take(8).collect()
}

fn tokenize(line: &str) -> Result<Vec<String>, ReplError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for character in line.chars() {
        match character {
            '"' => {
                in_quotes = !in_quotes;
            }
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }

    if in_quotes {
        return Err(ReplError::UnterminatedQuote);
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::{Repl, ReplCommand, parse_command, tokenize};

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
    fn parses_quoted_add_command() {
        let command = parse_command("add \"alice smith\" \"bob jones\" 42").unwrap();
        assert_eq!(
            command,
            ReplCommand::Add {
                sender: "alice smith".into(),
                receiver: "bob jones".into(),
                amount: 42,
            }
        );
    }

    #[test]
    fn rejects_invalid_add_command() {
        assert!(parse_command("add alice bob").is_err());
    }

    #[test]
    fn detects_unterminated_quote() {
        assert!(parse_command("add \"alice bob 10").is_err());
    }

    #[test]
    fn tokenizes_quoted_segments() {
        let tokens = tokenize("add \"alice smith\" bob 5").unwrap();
        assert_eq!(tokens, vec!["add", "alice smith", "bob", "5"]);
    }

    #[test]
    fn repl_can_execute_happy_path() {
        let mut repl = Repl::try_new(4).unwrap();
        repl.blockchain
            .credit_balance(crate::address_from_name("alice"), 5);

        assert!(!repl.execute_line("add alice bob 5").unwrap());
        assert!(!repl.execute_line("mine").unwrap());
        assert!(!repl.execute_line("validate").unwrap());
        assert!(repl.execute_line("quit").unwrap());
    }
}
