use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::transaction::Transaction;

pub type Hash = [u8; 32];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Block {
    pub timestamp: u64,
    pub prev_hash: Hash,
    pub nonce: u64,
    pub transactions: Vec<Transaction>,
}

impl Block {
    pub fn new(transactions: Vec<Transaction>, prev_hash: Hash) -> Self {
        Self {
            timestamp: Utc::now().timestamp() as u64,
            prev_hash,
            nonce: 0,
            transactions,
        }
    }

    pub fn compute_hash(&self) -> Hash {
        let serialized_transactions =
            serde_json::to_string(&self.transactions).unwrap_or_else(|_| "[]".to_string());
        let input = format!(
            "{}{}{}{}",
            self.timestamp,
            hex::encode(self.prev_hash),
            self.nonce,
            serialized_transactions
        );

        let digest = Sha256::digest(input.as_bytes());
        let mut hash = [0_u8; 32];
        hash.copy_from_slice(&digest);
        hash
    }

    pub fn mine(&mut self, difficulty: u32) {
        while !hash_meets_difficulty(&self.compute_hash(), difficulty) {
            self.nonce = self.nonce.saturating_add(1);
        }
    }
}

pub fn hash_meets_difficulty(hash: &Hash, difficulty: u32) -> bool {
    let full_zero_bytes = (difficulty / 8) as usize;
    let remaining_bits = (difficulty % 8) as usize;

    if hash.iter().take(full_zero_bytes).any(|byte| *byte != 0) {
        return false;
    }

    if remaining_bits == 0 {
        return true;
    }

    let mask = 0xFF_u8 << (8 - remaining_bits);
    hash.get(full_zero_bytes)
        .map(|byte| byte & mask == 0)
        .unwrap_or(false)
}

pub fn hash_to_hex(hash: &Hash) -> String {
    hex::encode(hash)
}

#[cfg(test)]
mod tests {
    use super::{Block, hash_meets_difficulty};
    use crate::transaction::Transaction;

    #[test]
    fn hash_is_deterministic() {
        let tx = Transaction {
            sender: "alice".into(),
            receiver: "bob".into(),
            amount: 10,
            timestamp: 1,
        };
        let block = Block {
            timestamp: 2,
            prev_hash: [7; 32],
            nonce: 3,
            transactions: vec![tx],
        };

        assert_eq!(block.compute_hash(), block.compute_hash());
    }

    #[test]
    fn mining_produces_valid_hash() {
        let mut block = Block {
            timestamp: 2,
            prev_hash: [0; 32],
            nonce: 0,
            transactions: vec![Transaction {
                sender: "alice".into(),
                receiver: "bob".into(),
                amount: 10,
                timestamp: 1,
            }],
        };

        block.mine(8);

        assert!(hash_meets_difficulty(&block.compute_hash(), 8));
    }
}
