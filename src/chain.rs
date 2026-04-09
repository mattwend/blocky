use thiserror::Error;

use crate::{
    Transaction,
    block::{Block, Hash, hash_meets_difficulty},
};

#[derive(Debug, Error)]
pub enum BlockyError {
    #[error("cannot mine a block without pending transactions")]
    NoPendingTransactions,
}

#[derive(Debug, Clone)]
pub struct Blockchain {
    pub chain: Vec<Block>,
    pub pending_transactions: Vec<Transaction>,
    pub difficulty: u32,
}

impl Blockchain {
    pub fn new(difficulty: u32) -> Self {
        let mut genesis = Block::new(Vec::new(), [0_u8; 32]);
        genesis.mine(difficulty);

        Self {
            chain: vec![genesis],
            pending_transactions: Vec::new(),
            difficulty,
        }
    }

    pub fn add_transaction(&mut self, tx: Transaction) -> Result<(), BlockyError> {
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

            if index == 0 {
                continue;
            }

            let prev = &self.chain[index - 1];
            if block.prev_hash != prev.compute_hash() {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::Blockchain;
    use crate::transaction::Transaction;

    #[test]
    fn genesis_block_has_zero_prev_hash() {
        let chain = Blockchain::new(4);
        assert_eq!(chain.chain[0].prev_hash, [0_u8; 32]);
    }

    #[test]
    fn valid_chain_stays_valid() {
        let mut chain = Blockchain::new(4);
        chain
            .add_transaction(Transaction {
                sender: "alice".into(),
                receiver: "bob".into(),
                amount: 25,
                timestamp: 1,
            })
            .unwrap();
        chain.mine_pending().unwrap();

        assert!(chain.is_valid());
    }

    #[test]
    fn tampering_is_detected() {
        let mut chain = Blockchain::new(4);
        chain
            .add_transaction(Transaction {
                sender: "alice".into(),
                receiver: "bob".into(),
                amount: 25,
                timestamp: 1,
            })
            .unwrap();
        chain.mine_pending().unwrap();

        chain.chain[1].transactions[0].amount = 999;

        assert!(!chain.is_valid());
    }
}
