use crate::{Blockchain, BlockyError, Transaction, hash_to_hex};

pub fn build_demo_blockchain(difficulty: u32) -> Result<Blockchain, BlockyError> {
    let mut blockchain = Blockchain::new(difficulty);

    let sample_transactions = [
        Transaction::new("alice", "bob", 25),
        Transaction::new("bob", "carol", 10),
        Transaction::new("carol", "dave", 5),
    ];

    for transaction in sample_transactions {
        blockchain.add_transaction(transaction)?;
    }

    blockchain.mine_pending()?;

    Ok(blockchain)
}

pub fn render_chain(blockchain: &Blockchain) -> String {
    let mut output = String::new();

    for (index, block) in blockchain.chain.iter().enumerate() {
        output.push_str(&format!("\nBlock #{index}\n"));
        output.push_str(&format!("  Timestamp: {}\n", block.timestamp));
        output.push_str(&format!("  Prev hash: {}\n", hash_to_hex(&block.prev_hash)));
        output.push_str(&format!(
            "  Hash:      {}\n",
            hash_to_hex(&block.compute_hash())
        ));
        output.push_str(&format!("  Nonce:     {}\n", block.nonce));
        output.push_str("  Transactions:\n");

        if block.transactions.is_empty() {
            output.push_str("    (none)\n");
            continue;
        }

        for transaction in &block.transactions {
            output.push_str(&format!(
                "    {} -> {}: {} @ {}\n",
                transaction.sender, transaction.receiver, transaction.amount, transaction.timestamp
            ));
        }
    }

    output
}
