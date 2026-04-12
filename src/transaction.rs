use borsh::{BorshDeserialize, BorshSerialize};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub type Address = [u8; 32];

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct Transaction {
    pub sender: Address,
    pub nonce: u64,
    pub payload: Payload,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub enum Payload {
    Transfer {
        receiver: Address,
        amount: u64,
    },
    Deploy {
        code: Vec<u8>,
    },
    Call {
        contract: Address,
        method: String,
        args: Vec<u8>,
        deposit: u64,
    },
}

impl Transaction {
    pub fn new_transfer(sender: Address, nonce: u64, receiver: Address, amount: u64) -> Self {
        Self {
            sender,
            nonce,
            payload: Payload::Transfer { receiver, amount },
            timestamp: Utc::now().timestamp() as u64,
        }
    }

    pub fn new_deploy(sender: Address, nonce: u64, code: Vec<u8>) -> Self {
        Self {
            sender,
            nonce,
            payload: Payload::Deploy { code },
            timestamp: Utc::now().timestamp() as u64,
        }
    }

    pub fn new_call(
        sender: Address,
        nonce: u64,
        contract: Address,
        method: impl Into<String>,
        args: Vec<u8>,
        deposit: u64,
    ) -> Self {
        Self {
            sender,
            nonce,
            payload: Payload::Call {
                contract,
                method: method.into(),
                args,
                deposit,
            },
            timestamp: Utc::now().timestamp() as u64,
        }
    }

    pub fn derived_contract_address(&self) -> Address {
        let mut hasher = Sha256::new();
        hasher.update(self.sender);
        hasher.update(self.nonce.to_le_bytes());
        let digest = hasher.finalize();
        let mut address = [0_u8; 32];
        address.copy_from_slice(&digest);
        address
    }
}

pub fn address_from_name(name: &str) -> Address {
    let digest = Sha256::digest(name.as_bytes());
    let mut address = [0_u8; 32];
    address.copy_from_slice(&digest);
    address
}

pub fn address_to_hex(address: &Address) -> String {
    hex::encode(address)
}

#[cfg(test)]
mod tests {
    use super::{Payload, Transaction, address_from_name};

    #[test]
    fn derived_contract_address_is_deterministic() {
        let sender = address_from_name("alice");
        let tx = Transaction {
            sender,
            nonce: 3,
            payload: Payload::Deploy {
                code: vec![1, 2, 3],
            },
            timestamp: 1,
        };

        assert_eq!(tx.derived_contract_address(), tx.derived_contract_address());
    }

    #[test]
    fn different_names_map_to_different_addresses() {
        assert_ne!(address_from_name("alice"), address_from_name("bob"));
    }
}
