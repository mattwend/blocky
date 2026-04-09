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
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
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
    input: String,
    output: Vec<String>,
}

impl Repl {
    pub fn new(difficulty: u32) -> Self {
        Self {
            blockchain: Blockchain::new(difficulty),
            input: String::new(),
            output: vec![
                "Welcome to Blocky REPL".to_string(),
                Self::help_text().to_string(),
            ],
        }
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

        match parse_command(line)? {
            ReplCommand::Add {
                sender,
                receiver,
                amount,
            } => {
                self.blockchain
                    .add_transaction(Transaction::new(sender, receiver, amount))?;
                self.push_output("Transaction queued.".to_string());
                Ok(false)
            }
            ReplCommand::Mine => {
                let block = self.blockchain.mine_pending()?;
                self.push_output(format!("Mined block with nonce {}.", block.nonce));
                Ok(false)
            }
            ReplCommand::Print => {
                for line in render_chain(&self.blockchain).lines() {
                    self.push_output(line.to_string());
                }
                Ok(false)
            }
            ReplCommand::Validate => {
                self.push_output(format!("Chain valid: {}", self.blockchain.is_valid()));
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
                    }
                    KeyCode::Backspace => {
                        self.input.pop();
                    }
                    KeyCode::Enter => {
                        let line = self.input.trim().to_string();
                        self.input.clear();

                        if line.is_empty() {
                            continue;
                        }

                        match self.execute_line(&line) {
                            Ok(true) => return Ok(()),
                            Ok(false) => {}
                            Err(error) => self.push_output(format!("Error: {error}")),
                        }
                    }
                    KeyCode::Esc => return Ok(()),
                    _ => {}
                }
            }
        }
    }

    fn draw(&self, frame: &mut ratatui::Frame<'_>) {
        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),
                Constraint::Length(3),
                Constraint::Length(2),
            ])
            .split(frame.area());

        let output_lines: Vec<Line<'_>> = self
            .output
            .iter()
            .map(|line| Line::from(line.as_str()))
            .collect();
        let output = Paragraph::new(Text::from(output_lines))
            .block(Block::default().title("Output").borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        frame.render_widget(output, areas[0]);

        let input = Paragraph::new(self.input.as_str())
            .block(Block::default().title("Input").borders(Borders::ALL));
        frame.render_widget(input, areas[1]);

        let help = Paragraph::new("Enter command, Esc quits")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title("Help"));
        frame.render_widget(help, areas[2]);

        frame.set_cursor_position((areas[1].x + self.input.len() as u16 + 1, areas[1].y + 1));
    }

    fn push_output(&mut self, line: String) {
        self.output.push(line);
    }

    fn help_text() -> &'static str {
        "Commands: add <sender> <receiver> <amount> | mine | print | validate | help | quit"
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
