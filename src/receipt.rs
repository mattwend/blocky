use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{chain::BlockyError, transaction::Transaction};

/// Outcome recorded for a mined transaction.
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct Receipt {
    /// Deterministic hash of the transaction this receipt belongs to.
    pub tx_hash: [u8; 32],
    /// Whether the transaction completed successfully.
    pub success: bool,
    /// Total gas charged for the transaction.
    pub gas_used: u64,
    /// Log lines emitted during execution.
    pub logs: Vec<String>,
    /// Human-readable error message for failed transactions.
    pub error: Option<String>,
}

impl Receipt {
    /// Creates a successful receipt for a transaction.
    ///
    /// # Arguments
    /// - `transaction`: Transaction the receipt corresponds to.
    /// - `gas_used`: Total gas charged during execution.
    /// - `logs`: Log lines emitted during execution.
    ///
    /// # Returns
    /// A receipt marked as successful.
    pub fn success(
        transaction: &Transaction,
        gas_used: u64,
        logs: Vec<String>,
    ) -> Result<Self, BlockyError> {
        Ok(Self {
            tx_hash: transaction_hash(transaction)?,
            success: true,
            gas_used,
            logs,
            error: None,
        })
    }

    /// Creates a failed receipt for a transaction.
    ///
    /// # Arguments
    /// - `transaction`: Transaction the receipt corresponds to.
    /// - `gas_used`: Total gas charged before the failure was reported.
    /// - `error`: Human-readable failure description.
    ///
    /// # Returns
    /// A receipt marked as failed.
    pub fn failure(
        transaction: &Transaction,
        gas_used: u64,
        error: impl Into<String>,
    ) -> Result<Self, BlockyError> {
        Ok(Self {
            tx_hash: transaction_hash(transaction)?,
            success: false,
            gas_used,
            logs: Vec::new(),
            error: Some(error.into()),
        })
    }
}

/// Computes a deterministic hash for a transaction.
///
/// # Arguments
/// - `transaction`: Transaction to hash.
///
/// # Returns
/// A 32-byte SHA-256 digest of the transaction's Borsh encoding.
pub fn transaction_hash(transaction: &Transaction) -> Result<[u8; 32], BlockyError> {
    let bytes = borsh::to_vec(transaction).map_err(BlockyError::HashSerialization)?;
    let digest = Sha256::digest(bytes);
    let mut hash = [0_u8; 32];
    hash.copy_from_slice(&digest);
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::{Receipt, transaction_hash};
    use crate::transaction::{Transaction, address_from_name};

    #[test]
    fn transaction_hash_matches_equal_transactions() {
        let tx_a =
            Transaction::new_transfer(address_from_name("alice"), 0, address_from_name("bob"), 5);
        let tx_b =
            Transaction::new_transfer(address_from_name("alice"), 0, address_from_name("bob"), 5);

        assert_eq!(
            transaction_hash(&tx_a).unwrap(),
            transaction_hash(&tx_b).unwrap()
        );
    }

    #[test]
    fn success_receipt_captures_logs() {
        let tx =
            Transaction::new_transfer(address_from_name("alice"), 0, address_from_name("bob"), 5);
        let receipt = Receipt::success(&tx, 77, vec!["hello".to_string()]).unwrap();

        assert!(receipt.success);
        assert_eq!(receipt.gas_used, 77);
        assert_eq!(receipt.logs, vec!["hello".to_string()]);
        assert!(receipt.error.is_none());
    }
}
