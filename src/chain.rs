use thiserror::Error;

use crate::{
    Address, Transaction, WorldState,
    block::{Block, Hash, hash_meets_difficulty},
    state::StateError,
    transaction::Payload,
};

#[derive(Debug, Error)]
pub enum BlockyError {
    #[error("cannot mine a block without pending transactions")]
    NoPendingTransactions,
    #[error("sender has insufficient balance: available {available}, required {required}")]
    InsufficientBalance { available: u64, required: u64 },
    #[error("sender nonce mismatch: expected {expected}, got {got}")]
    InvalidNonce { expected: u64, got: u64 },
    #[error(transparent)]
    State(#[from] StateError),
}

#[derive(Debug, Clone)]
pub struct Blockchain {
    pub chain: Vec<Block>,
    pub pending_transactions: Vec<Transaction>,
    pub difficulty: u32,
    pub state: WorldState,
}

impl Blockchain {
    pub fn new(difficulty: u32) -> Self {
        let mut genesis = Block::new(Vec::new(), [0_u8; 32]);
        genesis.mine(difficulty);

        Self {
            chain: vec![genesis],
            pending_transactions: Vec::new(),
            difficulty,
            state: WorldState::new(),
        }
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
            Payload::Deploy { .. } => 0,
            Payload::Call { deposit, .. } => *deposit,
        };
        let reserved = self
            .pending_transactions
            .iter()
            .filter(|pending| pending.sender == tx.sender)
            .map(|pending| match &pending.payload {
                Payload::Transfer { amount, .. } => *amount,
                Payload::Deploy { .. } => 0,
                Payload::Call { deposit, .. } => *deposit,
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

        let prev_hash: Hash = self
            .chain
            .last()
            .map(Block::compute_hash)
            .unwrap_or([0_u8; 32]);
        let transactions = std::mem::take(&mut self.pending_transactions);
        let mut block = Block::new(transactions, prev_hash);
        block.mine(self.difficulty);
        self.state.apply_block(&block)?;
        self.chain.push(block.clone());
        Ok(block)
    }

    pub fn is_valid(&self) -> bool {
        let Some(genesis) = self.chain.first() else {
            return false;
        };

        if genesis.prev_hash != [0_u8; 32] {
            return false;
        }

        if !hash_meets_difficulty(&genesis.compute_hash(), self.difficulty) {
            return false;
        }

        for (index, block) in self.chain.iter().enumerate() {
            if !hash_meets_difficulty(&block.compute_hash(), self.difficulty) {
                return false;
            }

            if index > 0 {
                let prev = &self.chain[index - 1];
                if block.prev_hash != prev.compute_hash() {
                    return false;
                }
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::Blockchain;
    use crate::transaction::{Transaction, address_from_name};

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

        assert!(chain.is_valid());
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

        assert!(!chain.is_valid());
    }
}
