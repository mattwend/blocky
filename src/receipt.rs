use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::transaction::Transaction;

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct Receipt {
    pub tx_hash: [u8; 32],
    pub success: bool,
    pub gas_used: u64,
    pub logs: Vec<String>,
    pub error: Option<String>,
}

impl Receipt {
    pub fn success(transaction: &Transaction, gas_used: u64, logs: Vec<String>) -> Self {
        Self {
            tx_hash: transaction_hash(transaction),
            success: true,
            gas_used,
            logs,
            error: None,
        }
    }

    pub fn failure(transaction: &Transaction, gas_used: u64, error: impl Into<String>) -> Self {
        Self {
            tx_hash: transaction_hash(transaction),
            success: false,
            gas_used,
            logs: Vec::new(),
            error: Some(error.into()),
        }
    }
}

pub fn transaction_hash(transaction: &Transaction) -> [u8; 32] {
    let bytes = borsh::to_vec(transaction).unwrap_or_default();
    let digest = Sha256::digest(bytes);
    let mut hash = [0_u8; 32];
    hash.copy_from_slice(&digest);
    hash
}

#[cfg(test)]
mod tests {
    use super::{Receipt, transaction_hash};
    use crate::transaction::{Transaction, address_from_name};

    #[test]
    fn transaction_hash_is_deterministic() {
        let tx =
            Transaction::new_transfer(address_from_name("alice"), 0, address_from_name("bob"), 5);
        assert_eq!(transaction_hash(&tx), transaction_hash(&tx));
    }

    #[test]
    fn success_receipt_captures_logs() {
        let tx =
            Transaction::new_transfer(address_from_name("alice"), 0, address_from_name("bob"), 5);
        let receipt = Receipt::success(&tx, 77, vec!["hello".to_string()]);

        assert!(receipt.success);
        assert_eq!(receipt.gas_used, 77);
        assert_eq!(receipt.logs, vec!["hello".to_string()]);
        assert!(receipt.error.is_none());
    }
}
