use chrono::Utc;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Transaction {
    pub sender: String,
    pub receiver: String,
    pub amount: u64,
    pub timestamp: u64,
}

impl Transaction {
    pub fn new(sender: impl Into<String>, receiver: impl Into<String>, amount: u64) -> Self {
        Self {
            sender: sender.into(),
            receiver: receiver.into(),
            amount,
            timestamp: Utc::now().timestamp() as u64,
        }
    }
}
