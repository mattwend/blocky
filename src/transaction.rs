use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A 32-byte account or contract address.
pub type Address = [u8; 32];

/// A transaction submitted by an account.
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct Transaction {
    /// Address paying for and authoring the transaction.
    pub sender: Address,
    /// Sender-local nonce used for replay protection and ordering.
    pub nonce: u64,
    /// Operation performed by this transaction.
    pub payload: Payload,
    /// Unix timestamp attached by the creator.
    pub timestamp: u64,
}

/// The operation carried by a transaction.
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub enum Payload {
    /// Transfers native balance from the sender to another account.
    Transfer {
        /// Recipient of the transfer.
        receiver: Address,
        /// Amount of native balance to move.
        amount: u64,
    },
    /// Deploys Wasm code to the contract address derived from the sender and nonce.
    Deploy {
        /// Raw Wasm bytecode stored at the derived contract address.
        code: Vec<u8>,
    },
    /// Calls a method exported by a deployed Wasm contract.
    Call {
        /// Target contract address.
        contract: Address,
        /// Exported method name to invoke.
        method: String,
        /// Method arguments, typically wrapped in a [`crate::CallEnvelope`].
        args: Vec<u8>,
        /// Balance transferred to the contract before execution.
        deposit: u64,
    },
}

impl Transaction {
    /// Creates a transfer transaction with a default timestamp of `0`.
    ///
    /// # Arguments
    /// - `sender`: Address creating and paying for the transaction.
    /// - `nonce`: Sender nonce for ordering and replay protection.
    /// - `receiver`: Recipient of the transferred balance.
    /// - `amount`: Amount of native balance to transfer.
    ///
    /// # Returns
    /// A transfer transaction with timestamp `0`.
    pub fn new_transfer(sender: Address, nonce: u64, receiver: Address, amount: u64) -> Self {
        Self::new_transfer_at(sender, nonce, receiver, amount, 0)
    }

    /// Creates a transfer transaction with an explicit timestamp.
    ///
    /// # Arguments
    /// - `sender`: Address creating and paying for the transaction.
    /// - `nonce`: Sender nonce for ordering and replay protection.
    /// - `receiver`: Recipient of the transferred balance.
    /// - `amount`: Amount of native balance to transfer.
    /// - `timestamp`: Unix timestamp to attach to the transaction.
    ///
    /// # Returns
    /// A transfer transaction using the provided timestamp.
    pub fn new_transfer_at(
        sender: Address,
        nonce: u64,
        receiver: Address,
        amount: u64,
        timestamp: u64,
    ) -> Self {
        Self {
            sender,
            nonce,
            payload: Payload::Transfer { receiver, amount },
            timestamp,
        }
    }

    /// Creates a deployment transaction with a default timestamp of `0`.
    ///
    /// # Arguments
    /// - `sender`: Address creating and paying for the transaction.
    /// - `nonce`: Sender nonce for ordering and replay protection.
    /// - `code`: Wasm bytecode to deploy.
    ///
    /// # Returns
    /// A deployment transaction with timestamp `0`.
    pub fn new_deploy(sender: Address, nonce: u64, code: Vec<u8>) -> Self {
        Self::new_deploy_at(sender, nonce, code, 0)
    }

    /// Creates a deployment transaction with an explicit timestamp.
    ///
    /// # Arguments
    /// - `sender`: Address creating and paying for the transaction.
    /// - `nonce`: Sender nonce for ordering and replay protection.
    /// - `code`: Wasm bytecode to deploy.
    /// - `timestamp`: Unix timestamp to attach to the transaction.
    ///
    /// # Returns
    /// A deployment transaction using the provided timestamp.
    pub fn new_deploy_at(sender: Address, nonce: u64, code: Vec<u8>, timestamp: u64) -> Self {
        Self {
            sender,
            nonce,
            payload: Payload::Deploy { code },
            timestamp,
        }
    }

    /// Creates a contract-call transaction with a default timestamp of `0`.
    ///
    /// # Arguments
    /// - `sender`: Address creating and paying for the transaction.
    /// - `nonce`: Sender nonce for ordering and replay protection.
    /// - `contract`: Target contract address.
    /// - `method`: Exported contract method to call.
    /// - `args`: Encoded method arguments.
    /// - `deposit`: Balance transferred to the contract before execution.
    ///
    /// # Returns
    /// A contract-call transaction with timestamp `0`.
    pub fn new_call(
        sender: Address,
        nonce: u64,
        contract: Address,
        method: impl Into<String>,
        args: Vec<u8>,
        deposit: u64,
    ) -> Self {
        Self::new_call_at(sender, nonce, contract, method, args, deposit, 0)
    }

    /// Creates a contract-call transaction with an explicit timestamp.
    ///
    /// # Arguments
    /// - `sender`: Address creating and paying for the transaction.
    /// - `nonce`: Sender nonce for ordering and replay protection.
    /// - `contract`: Target contract address.
    /// - `method`: Exported contract method to call.
    /// - `args`: Encoded method arguments.
    /// - `deposit`: Balance transferred to the contract before execution.
    /// - `timestamp`: Unix timestamp to attach to the transaction.
    ///
    /// # Returns
    /// A contract-call transaction using the provided timestamp.
    pub fn new_call_at(
        sender: Address,
        nonce: u64,
        contract: Address,
        method: impl Into<String>,
        args: Vec<u8>,
        deposit: u64,
        timestamp: u64,
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
            timestamp,
        }
    }

    /// Derives the deployment address for this transaction from the sender and nonce.
    ///
    /// # Returns
    /// The deterministic contract address associated with this transaction.
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

/// Derives a deterministic address from a human-readable name using SHA-256.
///
/// # Arguments
/// - `name`: Human-readable identifier to hash into an address.
///
/// # Returns
/// A deterministic 32-byte address for the provided name.
pub fn address_from_name(name: &str) -> Address {
    let digest = Sha256::digest(name.as_bytes());
    let mut address = [0_u8; 32];
    address.copy_from_slice(&digest);
    address
}

/// Encodes an address as lowercase hexadecimal.
///
/// # Arguments
/// - `address`: Address to encode.
///
/// # Returns
/// A lowercase hexadecimal string representation of the address.
pub fn address_to_hex(address: &Address) -> String {
    hex::encode(address)
}

#[cfg(test)]
mod tests {
    use super::{Payload, Transaction, address_from_name};
    use crate::CallEnvelope;

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

    #[test]
    fn call_payload_args_can_be_framed_with_envelope() {
        let tx = Transaction::new_call(
            address_from_name("alice"),
            0,
            address_from_name("contract"),
            "set",
            CallEnvelope::new("set", vec![1, 2, 3]).encode(),
            0,
        );

        match tx.payload {
            Payload::Call { args, .. } => {
                let envelope = CallEnvelope::decode(&args).unwrap();
                assert_eq!(envelope.method, "set");
                assert_eq!(envelope.args, vec![1, 2, 3]);
            }
            payload => panic!("unexpected payload: {payload:?}"),
        }
    }
}
