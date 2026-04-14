use tracing::info;

use crate::{
    Blockchain, BlockyError, Payload, Transaction, address_from_name, address_to_hex, hash_to_hex,
    transaction_hash,
};

pub fn build_demo_blockchain(difficulty: u32) -> Result<Blockchain, BlockyError> {
    let mut blockchain = Blockchain::try_new(difficulty)?;
    info!(difficulty, "initialized demo blockchain");

    let alice = address_from_name("alice");
    let bob = address_from_name("bob");
    let carol = address_from_name("carol");
    let dave = address_from_name("dave");

    blockchain.credit_balance(alice, 25);
    blockchain.credit_balance(bob, 10);
    blockchain.credit_balance(carol, 5);

    let sample_transactions = [
        Transaction::new_transfer(alice, 0, bob, 25),
        Transaction::new_transfer(bob, 0, carol, 10),
        Transaction::new_transfer(carol, 0, dave, 5),
    ];

    for transaction in sample_transactions {
        info!(sender = %address_to_hex(&transaction.sender), nonce = transaction.nonce, "queueing demo transaction");
        blockchain.add_transaction(transaction)?;
    }

    let block = blockchain.mine_pending()?;
    info!(nonce = block.nonce, "mined demo block");

    Ok(blockchain)
}

pub fn render_chain(blockchain: &Blockchain) -> Result<String, BlockyError> {
    let mut output = String::new();

    for (index, block) in blockchain.chain.iter().enumerate() {
        output.push_str(&format!("\nBlock #{index}\n"));
        output.push_str(&format!("  Timestamp: {}\n", block.timestamp));
        output.push_str(&format!("  Prev hash: {}\n", hash_to_hex(&block.prev_hash)));
        output.push_str(&format!(
            "  Hash:      {}\n",
            hash_to_hex(&block.compute_hash()?)
        ));
        output.push_str(&format!("  Nonce:     {}\n", block.nonce));
        output.push_str("  Transactions:\n");

        if block.transactions.is_empty() {
            output.push_str("    (none)\n");
            continue;
        }

        for (tx_index, transaction) in block.transactions.iter().enumerate() {
            let sender = short_address(&transaction.sender);
            let details = match &transaction.payload {
                Payload::Transfer { receiver, amount } => {
                    format!("{sender} -> {}: {}", short_address(receiver), amount)
                }
                Payload::Deploy { code } => {
                    format!("{sender} deploy {} bytes", code.len())
                }
                Payload::Call {
                    contract,
                    method,
                    deposit,
                    ..
                } => format!(
                    "{sender} call {}.{} deposit {}",
                    short_address(contract),
                    method,
                    deposit
                ),
            };
            output.push_str(&format!("    {details} @ {}\n", transaction.timestamp));

            if let Some(block_receipts) = blockchain.receipts.get(index.saturating_sub(1))
                && let Some(receipt) = block_receipts.get(tx_index)
            {
                output.push_str(&format!(
                    "      receipt {} success={} gas={}\n",
                    short_hash(&receipt.tx_hash),
                    receipt.success,
                    receipt.gas_used
                ));
                for log in &receipt.logs {
                    output.push_str(&format!("      log: {log}\n"));
                }
                if let Some(error) = &receipt.error {
                    output.push_str(&format!("      error: {error}\n"));
                }
            } else {
                let tx_hash = transaction_hash(transaction);
                output.push_str(&format!("      receipt {} pending\n", short_hash(&tx_hash)));
            }
        }
    }

    Ok(output)
}

fn short_hash(hash: &[u8; 32]) -> String {
    hex::encode(hash).chars().take(8).collect()
}

fn short_address(address: &crate::Address) -> String {
    address_to_hex(address).chars().take(8).collect()
}
