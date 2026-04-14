use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{BlockyError, transaction::Transaction};

/// A 32-byte SHA-256 digest used for block and code hashes.
pub type Hash = [u8; 32];

/// A mined block containing transactions and proof-of-work metadata.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Block {
    /// Unix timestamp captured when the block was created.
    pub timestamp: u64,
    /// Hash of the previous block, or all zeros for genesis.
    pub prev_hash: Hash,
    /// Nonce adjusted during mining until the hash meets difficulty.
    pub nonce: u64,
    /// Transactions included in this block.
    pub transactions: Vec<Transaction>,
}

impl Block {
    /// Creates a new block with the current UTC timestamp and a zero nonce.
    ///
    /// # Arguments
    /// - `transactions`: Transactions to include in the block.
    /// - `prev_hash`: Hash of the previous block in the chain.
    ///
    /// # Returns
    /// A new block ready to be mined.
    pub fn new(transactions: Vec<Transaction>, prev_hash: Hash) -> Self {
        Self {
            timestamp: Utc::now().timestamp() as u64,
            prev_hash,
            nonce: 0,
            transactions,
        }
    }

    /// Computes the block hash from its header fields and serialized transactions.
    ///
    /// # Returns
    /// The SHA-256 digest for this block, or an error if transaction serialization fails.
    pub fn compute_hash(&self) -> Result<Hash, BlockyError> {
        let serialized_transactions = serde_json::to_string(&self.transactions)
            .map_err(std::io::Error::other)
            .map_err(BlockyError::HashSerialization)?;
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
        Ok(hash)
    }

    /// Increments the nonce until the block hash satisfies the given difficulty.
    ///
    /// # Arguments
    /// - `difficulty`: Required number of leading zero bits in the mined hash.
    ///
    /// # Returns
    /// `Ok(())` once a valid nonce is found, or an error if hashing fails.
    pub fn mine(&mut self, difficulty: u32) -> Result<(), BlockyError> {
        while !hash_meets_difficulty(&self.compute_hash()?, difficulty) {
            self.nonce = self.nonce.saturating_add(1);
        }
        Ok(())
    }
}

/// Returns whether a hash satisfies a leading-zero-bit proof-of-work target.
///
/// # Arguments
/// - `hash`: Hash to test.
/// - `difficulty`: Required number of leading zero bits.
///
/// # Returns
/// `true` if the hash meets the difficulty target, otherwise `false`.
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

/// Encodes a hash as lowercase hexadecimal.
///
/// # Arguments
/// - `hash`: Hash to encode.
///
/// # Returns
/// A lowercase hexadecimal string representation of the hash.
pub fn hash_to_hex(hash: &Hash) -> String {
    hex::encode(hash)
}

#[cfg(test)]
mod tests {
    use super::{Block, hash_meets_difficulty};
    use crate::transaction::{Transaction, address_from_name};

    #[test]
    fn equal_blocks_hash_to_same_value() {
        let tx_a =
            Transaction::new_transfer(address_from_name("alice"), 0, address_from_name("bob"), 10);
        let tx_b =
            Transaction::new_transfer(address_from_name("alice"), 0, address_from_name("bob"), 10);
        let block_a = Block {
            timestamp: 2,
            prev_hash: [7; 32],
            nonce: 3,
            transactions: vec![tx_a],
        };
        let block_b = Block {
            timestamp: 2,
            prev_hash: [7; 32],
            nonce: 3,
            transactions: vec![tx_b],
        };

        assert_eq!(
            block_a.compute_hash().unwrap(),
            block_b.compute_hash().unwrap()
        );
    }

    #[test]
    fn mining_produces_valid_hash() {
        let mut block = Block {
            timestamp: 2,
            prev_hash: [0; 32],
            nonce: 0,
            transactions: vec![Transaction::new_transfer(
                address_from_name("alice"),
                0,
                address_from_name("bob"),
                10,
            )],
        };

        block.mine(8).unwrap();

        assert!(hash_meets_difficulty(&block.compute_hash().unwrap(), 8));
    }
}
