use std::collections::HashMap;
use tracechain_rust::{
    AccountState, BankModule, BankState, Runtime, RuntimeCall, RuntimeState, Transaction,
    TransactionValidator, VaultModule, VaultState, VAULT_ACCOUNT,
};

fn initial_test_state() -> RuntimeState {
    let mut balances = HashMap::new();
    balances.insert("amir".to_string(), 500);
    balances.insert("ali".to_string(), 200);
    balances.insert(VAULT_ACCOUNT.to_string(), 0);

    let bank = BankState { balances };

    let mut shares = HashMap::new();
    shares.insert("amir".to_string(), 0);
    shares.insert("ali".to_string(), 0);

    let vault = VaultState {
        shares,
        total_shares: 0,
    };

    let mut nonces = HashMap::new();
    nonces.insert("amir".to_string(), 0);
    nonces.insert("ali".to_string(), 0);

    let accounts = AccountState { nonces };

    RuntimeState {
        bank,
        vault,
        accounts,
    }
}

// 1. bank_balance_reads_zero_for_missing_account
#[test]
fn test_bank_balance_reads_zero_for_missing_account() {
    let mut state = initial_test_state();
    let bank = BankModule::new(&mut state.bank);
    assert_eq!(bank.balance("missing_user"), 0);
}

// 2. bank_send_moves_balance
#[test]
fn test_bank_send_moves_balance() {
    let mut state = initial_test_state();
    let mut bank = BankModule::new(&mut state.bank);
    let res = bank.send("amir", "ali", 100);
    assert!(res.is_ok());
    assert_eq!(bank.balance("amir"), 400);
    assert_eq!(bank.balance("ali"), 300);
}

// 3. bank_send_rejects_insufficient_balance
#[test]
fn test_bank_send_rejects_insufficient_balance() {
    let mut state = initial_test_state();
    let mut bank = BankModule::new(&mut state.bank);
    let res = bank.send("amir", "ali", 600);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "insufficient balance".to_string());
    assert_eq!(bank.balance("amir"), 500);
    assert_eq!(bank.balance("ali"), 200);
}

// 4. first_deposit_is_one_to_one
#[test]
fn test_first_deposit_is_one_to_one() {
    let mut state = initial_test_state();
    let mut vault = VaultModule::new(&mut state.vault, &mut state.bank);
    let shares = vault.deposit("amir", 100);
    assert!(shares.is_ok());
    assert_eq!(shares.unwrap(), 100);
    assert_eq!(vault.vault_state.total_shares, 100);
    assert_eq!(vault.total_assets(), 100);
}

// 5. deposit_updates_assets_and_shares
#[test]
fn test_deposit_updates_assets_and_shares() {
    let mut state = initial_test_state();
    let mut vault = VaultModule::new(&mut state.vault, &mut state.bank);

    // First deposit
    let s1 = vault.deposit("amir", 100).unwrap();
    assert_eq!(s1, 100);

    // Second deposit with ali (amount = 50). Total shares is 100, total assets is 100.
    // Expected shares: 50 * 100 / 100 = 50.
    let s2 = vault.deposit("ali", 50);
    assert!(s2.is_ok());
    assert_eq!(s2.unwrap(), 50);

    assert_eq!(vault.vault_state.total_shares, 150);
    assert_eq!(vault.total_assets(), 150);
}

// 6. withdraw_updates_assets_and_shares
#[test]
fn test_withdraw_updates_assets_and_shares() {
    let mut state = initial_test_state();
    let mut vault = VaultModule::new(&mut state.vault, &mut state.bank);

    // Deposit 100
    vault.deposit("amir", 100).unwrap();

    // Withdraw 40 shares. Expected assets out: 40 * 100 / 100 = 40.
    let assets = vault.withdraw("amir", 40);
    assert!(assets.is_ok());
    assert_eq!(assets.unwrap(), 40);

    assert_eq!(vault.vault_state.total_shares, 60);
    assert_eq!(vault.total_assets(), 60);
}

// 7. required_signer_for_deposit
#[test]
fn test_required_signer_for_deposit() {
    let call = RuntimeCall::Deposit {
        sender: "amir".to_string(),
        amount: 100,
    };
    assert_eq!(call.required_signer(), "amir");
}

// 8. required_signer_for_withdraw
#[test]
fn test_required_signer_for_withdraw() {
    let call = RuntimeCall::Withdraw {
        owner: "ali".to_string(),
        shares: 50,
    };
    assert_eq!(call.required_signer(), "ali");
}

// 9. validator_rejects_unauthorized_signer
#[test]
fn test_validator_rejects_unauthorized_signer() {
    let state = initial_test_state();

    // Signer is amir, but call is from ali
    let tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "ali".to_string(),
            amount: 50,
        }],
    };

    let res = TransactionValidator::validate(&tx, &state.accounts);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "unauthorized signer".to_string());
}

// 10. validator_rejects_invalid_nonce
#[test]
fn test_validator_rejects_invalid_nonce() {
    let state = initial_test_state();

    // Expected nonce is 0, tx uses 1
    let tx = Transaction {
        signer: "amir".to_string(),
        nonce: 1,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 50,
        }],
    };

    let res = TransactionValidator::validate(&tx, &state.accounts);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "invalid nonce".to_string());
}

// 11. failed_multi_call_transaction_rolls_back_all_state
#[test]
fn test_failed_multi_call_transaction_rolls_back_all_state() {
    let mut runtime = Runtime::new(initial_test_state());

    let tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![
            RuntimeCall::Deposit {
                sender: "amir".to_string(),
                amount: 100,
            },
            RuntimeCall::Withdraw {
                owner: "amir".to_string(),
                shares: 200, // triggers insufficient shares
            },
        ],
    };

    let res = runtime.execute_transaction(tx);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "insufficient shares".to_string());

    // Original state must remain unchanged (rollback)
    assert_eq!(*runtime.state.bank.balances.get("amir").unwrap(), 500);
    assert_eq!(*runtime.state.bank.balances.get(VAULT_ACCOUNT).unwrap(), 0);
    assert_eq!(*runtime.state.vault.shares.get("amir").unwrap(), 0);
    assert_eq!(runtime.state.vault.total_shares, 0);
    assert_eq!(*runtime.state.accounts.nonces.get("amir").unwrap(), 0);
}

// 12. failed_transaction_does_not_increment_nonce
#[test]
fn test_failed_transaction_does_not_increment_nonce() {
    let mut runtime = Runtime::new(initial_test_state());

    let tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 9999, // triggers insufficient balance
        }],
    };

    let res = runtime.execute_transaction(tx);
    assert!(res.is_err());
    assert_eq!(*runtime.state.accounts.nonces.get("amir").unwrap(), 0);
}

// 13. successful_multi_call_transaction_commits_all_state
#[test]
fn test_successful_multi_call_transaction_commits_all_state() {
    let mut runtime = Runtime::new(initial_test_state());

    let tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![
            RuntimeCall::Deposit {
                sender: "amir".to_string(),
                amount: 100,
            },
            RuntimeCall::Withdraw {
                owner: "amir".to_string(),
                shares: 40,
            },
        ],
    };

    let res = runtime.execute_transaction(tx);
    assert!(res.is_ok());

    // Check state commits (amir assets 500 - 100 + 40 = 440, vault 60, shares 60, total 60)
    assert_eq!(*runtime.state.bank.balances.get("amir").unwrap(), 440);
    assert_eq!(*runtime.state.bank.balances.get(VAULT_ACCOUNT).unwrap(), 60);
    assert_eq!(*runtime.state.vault.shares.get("amir").unwrap(), 60);
    assert_eq!(runtime.state.vault.total_shares, 60);
}

// 14. successful_transaction_increments_nonce_once
#[test]
fn test_successful_transaction_increments_nonce_once() {
    let mut runtime = Runtime::new(initial_test_state());

    let tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 100,
        }],
    };

    let res = runtime.execute_transaction(tx);
    assert!(res.is_ok());
    assert_eq!(*runtime.state.accounts.nonces.get("amir").unwrap(), 1);
}

// 15. call_results_preserve_order
#[test]
fn test_call_results_preserve_order() {
    let mut runtime = Runtime::new(initial_test_state());

    let tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![
            RuntimeCall::Deposit {
                sender: "amir".to_string(),
                amount: 100,
            },
            RuntimeCall::Withdraw {
                owner: "amir".to_string(),
                shares: 40,
            },
        ],
    };

    let res = runtime.execute_transaction(tx);
    assert!(res.is_ok());
    let results = res.unwrap();
    assert_eq!(results, vec![100, 40]);
}

// 16. cloned_runtime_state_is_independent
#[test]
fn test_cloned_runtime_state_is_independent() {
    let original = initial_test_state();
    let mut cloned = original.clone();

    cloned.bank.balances.insert("amir".to_string(), 9999);
    cloned.vault.shares.insert("amir".to_string(), 8888);
    cloned.vault.total_shares = 8888;
    cloned.accounts.nonces.insert("amir".to_string(), 7777);

    assert_eq!(*original.bank.balances.get("amir").unwrap(), 500);
    assert_eq!(*original.vault.shares.get("amir").unwrap(), 0);
    assert_eq!(original.vault.total_shares, 0);
    assert_eq!(*original.accounts.nonces.get("amir").unwrap(), 0);
}
