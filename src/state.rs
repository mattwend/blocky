use std::collections::{BTreeMap, HashMap};

use thiserror::Error;

use crate::{
    Address, Block, CallEnvelope, ExecutionContext,
    transaction::{Payload, Transaction},
    vm::{
        CallRequest, GasReport, VmError,
        gas::{BASE_TX_COST, DEFAULT_GAS_LIMIT, deploy_cost},
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AccountState {
    pub balance: u64,
    pub nonce: u64,
    pub code: Option<Vec<u8>>,
    pub storage: BTreeMap<Vec<u8>, Vec<u8>>,
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error("insufficient balance: available {available}, required {required}")]
    InsufficientBalance { available: u64, required: u64 },
    #[error("invalid nonce for account: expected {expected}, got {got}")]
    InvalidNonce { expected: u64, got: u64 },
    #[error("contract deployment already exists at derived address")]
    ContractAlreadyExists,
    #[error("contract call target has no code")]
    ContractCodeMissing,
    #[error(transparent)]
    Vm(#[from] VmError),
}

#[derive(Debug, Clone, Default)]
pub struct WorldState {
    accounts: HashMap<Address, AccountState>,
}

impl WorldState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_account(&self, addr: &Address) -> Option<&AccountState> {
        self.accounts.get(addr)
    }

    pub fn get_or_create(&mut self, addr: &Address) -> &mut AccountState {
        self.accounts.entry(*addr).or_default()
    }

    pub fn get_balance(&self, addr: &Address) -> u64 {
        self.get_account(addr)
            .map(|account| account.balance)
            .unwrap_or(0)
    }

    pub fn set_balance(&mut self, addr: &Address, balance: u64) {
        self.get_or_create(addr).balance = balance;
    }

    pub fn transfer(
        &mut self,
        from: &Address,
        to: &Address,
        amount: u64,
    ) -> Result<(), StateError> {
        if from == to || amount == 0 {
            return Ok(());
        }

        let available = self.get_balance(from);
        if available < amount {
            return Err(StateError::InsufficientBalance {
                available,
                required: amount,
            });
        }

        self.get_or_create(from).balance -= amount;
        self.get_or_create(to).balance += amount;
        Ok(())
    }

    pub fn apply_block(&mut self, block: &Block) -> Result<(), StateError> {
        for transaction in &block.transactions {
            self.apply_transaction(transaction)?;
        }
        Ok(())
    }

    pub fn apply_transaction(&mut self, transaction: &Transaction) -> Result<(), StateError> {
        self.apply_transaction_with_vm(transaction, None)
            .map(|_| ())
    }

    pub fn apply_transaction_with_vm(
        &mut self,
        transaction: &Transaction,
        vm: Option<&mut crate::VmEngine>,
    ) -> Result<(ExecutionContext, GasReport), StateError> {
        let sender = transaction.sender;
        let expected_nonce = self
            .get_account(&sender)
            .map(|account| account.nonce)
            .unwrap_or(0);
        if transaction.nonce != expected_nonce {
            return Err(StateError::InvalidNonce {
                expected: expected_nonce,
                got: transaction.nonce,
            });
        }

        let mut context = ExecutionContext::new(sender, sender, 0, Vec::new());
        let mut gas_report = GasReport {
            gas_limit: DEFAULT_GAS_LIMIT,
            gas_used: BASE_TX_COST,
        };

        match &transaction.payload {
            Payload::Transfer { receiver, amount } => {
                self.transfer(&sender, receiver, *amount)?;
            }
            Payload::Deploy { code } => {
                let contract = transaction.derived_contract_address();
                if self.get_account(&contract).is_some() {
                    return Err(StateError::ContractAlreadyExists);
                }
                self.get_or_create(&contract).code = Some(code.clone());
                gas_report.gas_used = gas_report.gas_used.saturating_add(deploy_cost(code.len()));
            }
            Payload::Call {
                contract,
                method,
                args,
                deposit,
            } => {
                let code = self
                    .get_account(contract)
                    .and_then(|account| account.code.as_ref())
                    .cloned()
                    .ok_or(StateError::ContractCodeMissing)?;

                let envelope = CallEnvelope::new(method.clone(), args.clone());
                let input = envelope.encode();
                let mut working_state = self.clone();
                working_state.transfer(&sender, contract, *deposit)?;

                if let Some(vm) = vm {
                    let (next_state, vm_context, vm_gas_report) =
                        vm.execute_call_with_state(CallRequest {
                            state: working_state,
                            caller: sender,
                            contract: *contract,
                            deposit: *deposit,
                            method,
                            args: &input,
                            code: &code,
                            gas_limit: DEFAULT_GAS_LIMIT.saturating_sub(BASE_TX_COST),
                        })?;
                    *self = next_state;
                    context = vm_context;
                    gas_report.gas_used =
                        gas_report.gas_used.saturating_add(vm_gas_report.gas_used);
                } else {
                    *self = working_state;
                    context = ExecutionContext::new(sender, *contract, *deposit, input);
                }
            }
        }

        self.get_or_create(&sender).nonce += 1;
        Ok((context, gas_report))
    }
}

#[cfg(test)]
mod tests {
    use super::{StateError, WorldState};
    use crate::transaction::{Payload, Transaction, address_from_name};

    #[test]
    fn transfer_moves_balance() {
        let alice = address_from_name("alice");
        let bob = address_from_name("bob");
        let mut state = WorldState::new();
        state.set_balance(&alice, 50);

        state.transfer(&alice, &bob, 20).unwrap();

        assert_eq!(state.get_balance(&alice), 30);
        assert_eq!(state.get_balance(&bob), 20);
    }

    #[test]
    fn apply_transaction_checks_nonce() {
        let alice = address_from_name("alice");
        let bob = address_from_name("bob");
        let mut state = WorldState::new();
        state.set_balance(&alice, 50);

        let error = state
            .apply_transaction(&Transaction {
                sender: alice,
                nonce: 7,
                payload: Payload::Transfer {
                    receiver: bob,
                    amount: 10,
                },
                timestamp: 1,
            })
            .unwrap_err();

        assert!(matches!(
            error,
            StateError::InvalidNonce {
                expected: 0,
                got: 7,
            }
        ));
    }

    #[test]
    fn deploy_stores_contract_code() {
        let alice = address_from_name("alice");
        let mut state = WorldState::new();

        let tx = Transaction {
            sender: alice,
            nonce: 0,
            payload: Payload::Deploy {
                code: vec![0, 97, 115, 109],
            },
            timestamp: 1,
        };
        let contract = tx.derived_contract_address();

        state.apply_transaction(&tx).unwrap();

        assert_eq!(
            state.get_account(&contract).unwrap().code,
            Some(vec![0, 97, 115, 109])
        );
        assert_eq!(state.get_account(&alice).unwrap().nonce, 1);
    }
}
