use thiserror::Error;

use crate::{
    Address, Receipt, Transaction, VmEngine, WorldState,
    block::{Block, Hash, hash_meets_difficulty},
    state::StateError,
    transaction::Payload,
    vm::{
        VmError,
        gas::{BASE_TX_COST, DEFAULT_GAS_LIMIT, deploy_cost},
    },
};

#[derive(Debug, Error)]
pub enum BlockyError {
    #[error("cannot mine a block without pending transactions")]
    NoPendingTransactions,
    #[error("failed to serialize block transactions while hashing: {0}")]
    BlockHashSerialization(#[source] serde_json::Error),
    #[error("sender has insufficient balance: available {available}, required {required}")]
    InsufficientBalance { available: u64, required: u64 },
    #[error("sender nonce mismatch: expected {expected}, got {got}")]
    InvalidNonce { expected: u64, got: u64 },
    #[error(transparent)]
    State(#[from] StateError),
    #[error(transparent)]
    Vm(#[from] VmError),
}

pub struct Blockchain {
    pub chain: Vec<Block>,
    pub pending_transactions: Vec<Transaction>,
    pub difficulty: u32,
    pub state: WorldState,
    pub vm: VmEngine,
    pub receipts: Vec<Vec<Receipt>>,
}

impl Blockchain {
    pub fn new(difficulty: u32) -> Self {
        match Self::try_new(difficulty) {
            Ok(blockchain) => blockchain,
            Err(error) => panic!("failed to initialize blockchain: {error}"),
        }
    }

    pub fn try_new(difficulty: u32) -> Result<Self, BlockyError> {
        let mut genesis = Block::new(Vec::new(), [0_u8; 32]);
        genesis.mine(difficulty)?;

        Ok(Self {
            chain: vec![genesis],
            pending_transactions: Vec::new(),
            difficulty,
            state: WorldState::new(),
            vm: VmEngine::new()?,
            receipts: Vec::new(),
        })
    }

    pub fn credit_balance(&mut self, address: Address, amount: u64) {
        let current = self.state.get_balance(&address);
        self.state
            .set_balance(&address, current.saturating_add(amount));
    }

    pub fn add_transaction(&mut self, tx: Transaction) -> Result<(), BlockyError> {
        let expected_nonce = self
            .state
            .get_account(&tx.sender)
            .map(|account| account.nonce)
            .unwrap_or(0)
            + self
                .pending_transactions
                .iter()
                .filter(|pending| pending.sender == tx.sender)
                .count() as u64;
        if tx.nonce != expected_nonce {
            return Err(BlockyError::InvalidNonce {
                expected: expected_nonce,
                got: tx.nonce,
            });
        }

        let required = match &tx.payload {
            Payload::Transfer { amount, .. } => *amount,
            Payload::Deploy { code } => deploy_cost(code.len()).saturating_add(BASE_TX_COST),
            Payload::Call { deposit, .. } => deposit.saturating_add(DEFAULT_GAS_LIMIT),
        };
        let reserved = self
            .pending_transactions
            .iter()
            .filter(|pending| pending.sender == tx.sender)
            .map(|pending| match &pending.payload {
                Payload::Transfer { amount, .. } => *amount,
                Payload::Deploy { code } => deploy_cost(code.len()).saturating_add(BASE_TX_COST),
                Payload::Call { deposit, .. } => deposit.saturating_add(DEFAULT_GAS_LIMIT),
            })
            .sum::<u64>();
        let available = self.state.get_balance(&tx.sender);
        if available < reserved.saturating_add(required) {
            return Err(BlockyError::InsufficientBalance {
                available: available.saturating_sub(reserved),
                required,
            });
        }

        self.pending_transactions.push(tx);
        Ok(())
    }

    pub fn mine_pending(&mut self) -> Result<Block, BlockyError> {
        if self.pending_transactions.is_empty() {
            return Err(BlockyError::NoPendingTransactions);
        }

        let prev_hash: Hash = match self.chain.last() {
            Some(block) => block.compute_hash()?,
            None => [0_u8; 32],
        };
        let transactions = std::mem::take(&mut self.pending_transactions);
        let mut block = Block::new(transactions.clone(), prev_hash);
        block.mine(self.difficulty)?;

        let mut working_state = self.state.clone();
        let mut receipts = Vec::with_capacity(block.transactions.len());
        for transaction in &block.transactions {
            match working_state.apply_transaction_with_vm(transaction, Some(&mut self.vm)) {
                Ok((context, gas_report)) => receipts.push(Receipt::success(
                    transaction,
                    gas_report.gas_used,
                    context.logs,
                )),
                Err(error) => {
                    let gas_used = match &transaction.payload {
                        Payload::Deploy { code } => {
                            BASE_TX_COST.saturating_add(deploy_cost(code.len()))
                        }
                        _ => BASE_TX_COST,
                    };
                    receipts.push(Receipt::failure(transaction, gas_used, error.to_string()));
                    self.receipts.push(receipts);
                    self.pending_transactions = transactions;
                    return Err(error.into());
                }
            }
        }

        self.state = working_state;
        self.chain.push(block.clone());
        self.receipts.push(receipts);
        Ok(block)
    }

    pub fn is_valid(&self) -> Result<bool, BlockyError> {
        let Some(genesis) = self.chain.first() else {
            return Ok(false);
        };

        if genesis.prev_hash != [0_u8; 32] {
            return Ok(false);
        }

        if !hash_meets_difficulty(&genesis.compute_hash()?, self.difficulty) {
            return Ok(false);
        }

        for (index, block) in self.chain.iter().enumerate() {
            if !hash_meets_difficulty(&block.compute_hash()?, self.difficulty) {
                return Ok(false);
            }

            if index > 0 {
                let prev = &self.chain[index - 1];
                if block.prev_hash != prev.compute_hash()? {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::{Blockchain, BlockyError};
    use crate::{
        transaction::{Payload, Transaction, address_from_name},
        vm::gas::BASE_TX_COST,
    };

    const EMPTY_MODULE: &[u8] = &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    const NOOP_MODULE: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00, 0x03,
        0x02, 0x01, 0x00, 0x07, 0x08, 0x01, 0x04, 0x6e, 0x6f, 0x6f, 0x70, 0x00, 0x00, 0x0a, 0x04,
        0x01, 0x02, 0x00, 0x0b,
    ];

    #[test]
    fn genesis_block_has_zero_prev_hash() {
        let chain = Blockchain::new(4);
        assert_eq!(chain.chain[0].prev_hash, [0_u8; 32]);
    }

    #[test]
    fn valid_chain_stays_valid() {
        let mut chain = Blockchain::new(4);
        let alice = address_from_name("alice");
        let bob = address_from_name("bob");
        chain.credit_balance(alice, 25);
        chain
            .add_transaction(Transaction::new_transfer(alice, 0, bob, 25))
            .unwrap();
        chain.mine_pending().unwrap();

        assert!(chain.is_valid().unwrap());
        assert_eq!(chain.state.get_balance(&alice), 0);
        assert_eq!(chain.state.get_balance(&bob), 25);
    }

    #[test]
    fn rejects_transaction_when_balance_is_too_low() {
        let mut chain = Blockchain::new(4);
        let alice = address_from_name("alice");
        let bob = address_from_name("bob");

        let error = chain
            .add_transaction(Transaction::new_transfer(alice, 0, bob, 25))
            .unwrap_err();

        assert!(matches!(
            error,
            super::BlockyError::InsufficientBalance { .. }
        ));
    }

    #[test]
    fn tampering_is_detected() {
        let mut chain = Blockchain::new(12);
        let alice = address_from_name("alice");
        let bob = address_from_name("bob");
        chain.credit_balance(alice, 25);
        chain
            .add_transaction(Transaction::new_transfer(alice, 0, bob, 25))
            .unwrap();
        chain.mine_pending().unwrap();

        if let crate::transaction::Payload::Transfer { amount, .. } =
            &mut chain.chain[1].transactions[0].payload
        {
            *amount = 999;
        }

        assert!(!chain.is_valid().unwrap());
    }

    #[test]
    fn mine_pending_records_success_receipts() {
        let mut chain = Blockchain::new(4);
        let alice = address_from_name("alice");
        let deploy = Transaction::new_deploy(alice, 0, NOOP_MODULE.to_vec());
        let contract = deploy.derived_contract_address();

        chain.credit_balance(alice, 2_000_000);
        chain.add_transaction(deploy).unwrap();
        chain
            .add_transaction(Transaction::new_call(
                alice,
                1,
                contract,
                "noop",
                Vec::new(),
                3,
            ))
            .unwrap();

        chain.mine_pending().unwrap();

        assert_eq!(chain.receipts.len(), 1);
        assert_eq!(chain.receipts[0].len(), 2);
        assert!(chain.receipts[0].iter().all(|receipt| receipt.success));
        assert!(
            chain.receipts[0]
                .iter()
                .all(|receipt| receipt.gas_used >= BASE_TX_COST)
        );
        assert_eq!(chain.state.get_balance(&contract), 3);
    }

    #[test]
    fn mine_pending_records_failure_receipt_for_vm_errors() {
        let mut chain = Blockchain::new(4);
        let alice = address_from_name("alice");
        let deploy = Transaction::new_deploy(alice, 0, EMPTY_MODULE.to_vec());
        let contract = deploy.derived_contract_address();

        chain.credit_balance(alice, 2_000_000);
        chain.add_transaction(deploy.clone()).unwrap();
        let failing_call = Transaction {
            sender: alice,
            nonce: 1,
            payload: Payload::Call {
                contract,
                method: "missing".to_string(),
                args: Vec::new(),
                deposit: 2,
            },
            timestamp: 1,
        };
        chain.add_transaction(failing_call.clone()).unwrap();

        let error = chain.mine_pending().unwrap_err();

        assert!(matches!(
            error,
            BlockyError::State(crate::StateError::Vm(_))
        ));
        assert_eq!(chain.receipts.len(), 1);
        assert_eq!(chain.receipts[0].len(), 2);
        assert!(chain.receipts[0][0].success);
        assert!(!chain.receipts[0][1].success);
        assert_eq!(chain.chain.len(), 1);
        assert!(chain.state.get_account(&contract).is_none());
        assert_eq!(chain.pending_transactions, vec![deploy, failing_call]);
    }
}
