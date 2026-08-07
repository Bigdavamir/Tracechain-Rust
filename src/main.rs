use std::collections::HashMap;
use tracechain_rust::{
    AccountState, BankState, Runtime, RuntimeCall, RuntimeState, Transaction, VaultState,
    VAULT_ACCOUNT,
};

fn main() {
    println!("--- Tracechain-rust: Educational Blockchain Runtime Model ---");

    // Initialize state
    let mut balances = HashMap::new();
    balances.insert("amir".to_string(), 500);
    balances.insert("ali".to_string(), 200);
    balances.insert("attacker".to_string(), 50);
    balances.insert(VAULT_ACCOUNT.to_string(), 0);

    let bank_state = BankState { balances };

    let mut shares = HashMap::new();
    shares.insert("amir".to_string(), 0);
    shares.insert("ali".to_string(), 0);
    shares.insert("attacker".to_string(), 0);

    let vault_state = VaultState {
        shares,
        total_shares: 0,
    };

    let mut nonces = HashMap::new();
    nonces.insert("amir".to_string(), 0);
    nonces.insert("ali".to_string(), 0);
    nonces.insert("attacker".to_string(), 0);

    let account_state = AccountState { nonces };

    let runtime_state = RuntimeState {
        bank: bank_state,
        vault: vault_state,
        accounts: account_state,
    };

    let mut runtime = Runtime::new(runtime_state);

    println!("\n[Initial State]");
    print_state(&runtime.state);

    // -------------------------------------------------------------------------
    // 1. Failing transaction
    // -------------------------------------------------------------------------
    println!("\n--- Executing FAILING Transaction (Multi-call with rollback expectation) ---");
    let tx_fail = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![
            RuntimeCall::Deposit {
                sender: "amir".to_string(),
                amount: 100,
            },
            RuntimeCall::Withdraw {
                owner: "amir".to_string(),
                shares: 200, // This exceeds available shares (amir only gets 100 shares from deposit)
            },
        ],
    };

    match runtime.execute_transaction(tx_fail) {
        Ok(results) => println!(
            "ERROR: Expected transaction to fail, but succeeded with: {:?}",
            results
        ),
        Err(e) => println!("Transaction failed as expected: {}", e),
    }

    println!("\n[State After Failing Transaction - Must be identical to Initial State]");
    print_state(&runtime.state);

    // Verify state has not changed
    assert_eq!(*runtime.state.bank.balances.get("amir").unwrap(), 500);
    assert_eq!(*runtime.state.bank.balances.get(VAULT_ACCOUNT).unwrap(), 0);
    assert_eq!(*runtime.state.vault.shares.get("amir").unwrap(), 0);
    assert_eq!(runtime.state.vault.total_shares, 0);
    assert_eq!(*runtime.state.accounts.nonces.get("amir").unwrap(), 0);

    // -------------------------------------------------------------------------
    // 2. Successful transaction
    // -------------------------------------------------------------------------
    println!("\n--- Executing SUCCESSFUL Transaction ---");
    let tx_success = Transaction {
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

    match runtime.execute_transaction(tx_success) {
        Ok(results) => {
            println!("Transaction succeeded!");
            println!(
                "Returned Results Vector (shares minted, assets returned): {:?}",
                results
            );
            assert_eq!(results, vec![100, 40]);
        }
        Err(e) => println!("ERROR: Expected transaction to succeed, but failed: {}", e),
    }

    println!("\n[State After Successful Transaction]");
    print_state(&runtime.state);

    // Verify successful mutations
    assert_eq!(*runtime.state.bank.balances.get("amir").unwrap(), 440);
    assert_eq!(*runtime.state.bank.balances.get(VAULT_ACCOUNT).unwrap(), 60);
    assert_eq!(*runtime.state.vault.shares.get("amir").unwrap(), 60);
    assert_eq!(runtime.state.vault.total_shares, 60);
    assert_eq!(*runtime.state.accounts.nonces.get("amir").unwrap(), 1);
}

fn print_state(state: &RuntimeState) {
    println!("  Bank Balances:");
    for (acc, bal) in &state.bank.balances {
        println!("    {}: {}", acc, bal);
    }
    println!("  Vault Shares (Total: {}):", state.vault.total_shares);
    for (acc, sh) in &state.vault.shares {
        println!("    {}: {}", acc, sh);
    }
    println!("  Account Nonces:");
    for (acc, nonce) in &state.accounts.nonces {
        println!("    {}: {}", acc, nonce);
    }
}
