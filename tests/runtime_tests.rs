use std::collections::HashMap;
use tracechain_rust::{
    AccountState, BankState, Runtime, RuntimeCall, RuntimeError, RuntimeState, Transaction,
    VaultState, VAULT_ACCOUNT,
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

#[test]
fn test_cloning_runtime_state_is_independent() {
    let original = initial_test_state();
    let mut cloned = original.clone();

    // Mutate cloned
    cloned.bank.balances.insert("amir".to_string(), 999);
    cloned.vault.shares.insert("amir".to_string(), 123);
    cloned.vault.total_shares = 123;
    cloned.accounts.nonces.insert("amir".to_string(), 5);

    // Original must remain unchanged
    assert_ne!(original, cloned);
    assert_eq!(*original.bank.balances.get("amir").unwrap(), 500);
    assert_eq!(*original.vault.shares.get("amir").unwrap(), 0);
    assert_eq!(original.vault.total_shares, 0);
    assert_eq!(*original.accounts.nonces.get("amir").unwrap(), 0);
}

#[test]
fn test_failed_multicall_transaction_rolls_back_all_state() {
    let mut runtime = Runtime::new(initial_test_state());

    // Transaction that has a successful deposit followed by an invalid withdraw (too many shares)
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
                shares: 500, // amir will only have 100 shares from deposit, so 500 should trigger InsufficientShares
            },
        ],
    };

    let res = runtime.execute_transaction(tx);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), RuntimeError::InsufficientShares);

    // Verify full state rollback
    assert_eq!(*runtime.state.bank.balances.get("amir").unwrap(), 500);
    assert_eq!(*runtime.state.bank.balances.get(VAULT_ACCOUNT).unwrap(), 0);
    assert_eq!(*runtime.state.vault.shares.get("amir").unwrap(), 0);
    assert_eq!(runtime.state.vault.total_shares, 0);
    assert_eq!(*runtime.state.accounts.nonces.get("amir").unwrap(), 0);
}

#[test]
fn test_successful_multicall_transaction_commits_all_state() {
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
    let outputs = res.unwrap();

    // Correct results ordering (1st deposit gives 100 shares, withdraw burns 40 shares and returns 40 assets)
    assert_eq!(outputs, vec![100, 40]);

    // Verify state commits
    assert_eq!(*runtime.state.bank.balances.get("amir").unwrap(), 440);
    assert_eq!(*runtime.state.bank.balances.get(VAULT_ACCOUNT).unwrap(), 60);
    assert_eq!(*runtime.state.vault.shares.get("amir").unwrap(), 60);
    assert_eq!(runtime.state.vault.total_shares, 60);
    assert_eq!(*runtime.state.accounts.nonces.get("amir").unwrap(), 1);
}

#[test]
fn test_replayed_nonce_is_rejected() {
    let mut runtime = Runtime::new(initial_test_state());

    // Perform one successful TX
    let tx1 = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 100,
        }],
    };
    assert!(runtime.execute_transaction(tx1).is_ok());
    assert_eq!(*runtime.state.accounts.nonces.get("amir").unwrap(), 1);

    // Try to replay with nonce = 0
    let tx_replay = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 50,
        }],
    };
    let res = runtime.execute_transaction(tx_replay);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err(),
        RuntimeError::InvalidNonce {
            expected: 1,
            received: 0
        }
    );
}

#[test]
fn test_unauthorized_signer_is_rejected() {
    let runtime = Runtime::new(initial_test_state());

    // tx signer is amir, but call sender is ali
    let tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "ali".to_string(),
            amount: 50,
        }],
    };

    let validation_res = tracechain_rust::TransactionValidator::validate(&tx, &runtime.state);
    assert!(validation_res.is_err());
    assert_eq!(
        validation_res.unwrap_err(),
        RuntimeError::UnauthorizedSigner { call_index: 0 }
    );
}

#[test]
fn test_failed_transaction_does_not_increment_nonce() {
    let mut runtime = Runtime::new(initial_test_state());

    let tx_invalid = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 999999, // way more than amir has
        }],
    };

    let res = runtime.execute_transaction(tx_invalid);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), RuntimeError::InsufficientBalance);

    // Nonce must remain 0
    assert_eq!(*runtime.state.accounts.nonces.get("amir").unwrap(), 0);
}
