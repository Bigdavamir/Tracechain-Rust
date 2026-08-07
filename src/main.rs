use std::collections::HashMap;
use tracechain_rust::{
    AccountState, BankState, Runtime, RuntimeCall, RuntimeState, Transaction, VaultState,
    VAULT_ACCOUNT,
};

fn main() {
    println!("=== TraceChain Rust Educational Runtime Demo ===");

    // Initialize RuntimeState with specified values:
    // Bank balances:
    // amir = 500
    // ali = 200
    // attacker = 50
    // vault = 0
    let mut balances = HashMap::new();
    balances.insert("amir".to_string(), 500);
    balances.insert("ali".to_string(), 200);
    balances.insert("attacker".to_string(), 50);
    balances.insert(VAULT_ACCOUNT.to_string(), 0);

    let bank_state = BankState { balances };

    // Vault shares:
    // amir = 0
    // ali = 0
    // attacker = 0
    // total_shares = 0
    let mut shares = HashMap::new();
    shares.insert("amir".to_string(), 0);
    shares.insert("ali".to_string(), 0);
    shares.insert("attacker".to_string(), 0);

    let vault_state = VaultState {
        shares,
        total_shares: 0,
    };

    // Account nonces:
    // amir = 0
    // ali = 0
    // attacker = 0
    let mut nonces = HashMap::new();
    nonces.insert("amir".to_string(), 0);
    nonces.insert("ali".to_string(), 0);
    nonces.insert("attacker".to_string(), 0);

    let account_state = AccountState { nonces };

    let initial_state = RuntimeState {
        bank: bank_state,
        vault: vault_state,
        accounts: account_state,
    };

    let mut runtime = Runtime::new(initial_state);

    // ------------------------------------------------------------
    // SCENARIO A: FAILED ATOMIC TRANSACTION
    // ------------------------------------------------------------
    println!("\n=== Scenario A: Failed Atomic Transaction ===");
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
                shares: 200,
            },
        ],
    };

    let res_fail = runtime.execute_transaction(tx_fail);
    match res_fail {
        Ok(results) => println!("result: Succeeded unexpectedly with {:?}", results),
        Err(err) => println!("result: Failed as expected with Error: \"{}\"", err),
    }

    println!("committed state after failure:");
    print_state(&runtime.state);

    // ------------------------------------------------------------
    // SCENARIO B: SUCCESSFUL ATOMIC TRANSACTION
    // ------------------------------------------------------------
    println!("\n=== Scenario B: Successful Atomic Transaction ===");
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

    let res_success = runtime.execute_transaction(tx_success);
    match res_success {
        Ok(results) => println!("results: {:?}", results),
        Err(err) => println!("result: Failed unexpectedly with Error: \"{}\"", err),
    }

    println!("committed state after success:");
    print_state(&runtime.state);
}

fn print_state(state: &RuntimeState) {
    println!(
        "  amir assets: {:?}",
        state.bank.balances.get("amir").unwrap_or(&0)
    );
    println!(
        "  vault assets: {:?}",
        state.bank.balances.get(VAULT_ACCOUNT).unwrap_or(&0)
    );
    println!(
        "  amir shares: {:?}",
        state.vault.shares.get("amir").unwrap_or(&0)
    );
    println!("  total shares: {:?}", state.vault.total_shares);
    println!(
        "  amir nonce: {:?}",
        state.accounts.nonces.get("amir").unwrap_or(&0)
    );
}
