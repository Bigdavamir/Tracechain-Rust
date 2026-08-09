use ed25519_dalek::{Signer, SigningKey};
use std::collections::HashMap;
use tracechain_rust::{
    AccountState, BankModule, BankState, Runtime, RuntimeCall, RuntimeState, Transaction,
    TransactionValidator, VaultModule, VaultState, VAULT_ACCOUNT,
};

// ============================================================
// TEST WALLET HELPER
// ============================================================

struct TestWallet {
    pub signing_key: SigningKey,
    pub public_key: [u8; 32],
}

impl TestWallet {
    fn new(seed: u8) -> Self {
        let mut bytes = [0u8; 32];
        bytes[0] = seed;
        let signing_key = SigningKey::from_bytes(&bytes);
        let public_key = signing_key.verifying_key().to_bytes();
        Self {
            signing_key,
            public_key,
        }
    }
}

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

    let amir_wallet = TestWallet::new(1);
    let ali_wallet = TestWallet::new(2);

    let mut public_keys = HashMap::new();
    public_keys.insert("amir".to_string(), amir_wallet.public_key);
    public_keys.insert("ali".to_string(), ali_wallet.public_key);

    let accounts = AccountState {
        nonces,
        public_keys,
    };

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
    let amir_wallet = TestWallet::new(1);

    // Signer is amir, but call is from ali
    let mut tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "ali".to_string(),
            amount: 50,
        }],
        signature: Vec::new(),
    };

    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = amir_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

    let res = TransactionValidator::validate(&tx, &state.accounts, "tracechain-1");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "unauthorized signer".to_string());
}

// 10. validator_rejects_invalid_nonce
#[test]
fn test_validator_rejects_invalid_nonce() {
    let state = initial_test_state();
    let amir_wallet = TestWallet::new(1);

    // Expected nonce is 0, tx uses 1
    let mut tx = Transaction {
        signer: "amir".to_string(),
        nonce: 1,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 50,
        }],
        signature: Vec::new(),
    };

    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = amir_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

    let res = TransactionValidator::validate(&tx, &state.accounts, "tracechain-1");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "invalid nonce".to_string());
}

// 11. failed_multi_call_transaction_rolls_back_all_state
#[test]
fn test_failed_multi_call_transaction_rolls_back_all_state() {
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());
    let amir_wallet = TestWallet::new(1);

    let mut tx = Transaction {
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
        signature: Vec::new(),
    };

    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = amir_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

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
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());
    let amir_wallet = TestWallet::new(1);

    let mut tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 9999, // triggers insufficient balance
        }],
        signature: Vec::new(),
    };

    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = amir_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

    let res = runtime.execute_transaction(tx);
    assert!(res.is_err());
    assert_eq!(*runtime.state.accounts.nonces.get("amir").unwrap(), 0);
}

// 13. successful_multi_call_transaction_commits_all_state
#[test]
fn test_successful_multi_call_transaction_commits_all_state() {
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());
    let amir_wallet = TestWallet::new(1);

    let mut tx = Transaction {
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
        signature: Vec::new(),
    };

    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = amir_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

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
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());
    let amir_wallet = TestWallet::new(1);

    let mut tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 100,
        }],
        signature: Vec::new(),
    };

    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = amir_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

    let res = runtime.execute_transaction(tx);
    assert!(res.is_ok());
    assert_eq!(*runtime.state.accounts.nonces.get("amir").unwrap(), 1);
}

// 15. call_results_preserve_order
#[test]
fn test_call_results_preserve_order() {
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());
    let amir_wallet = TestWallet::new(1);

    let mut tx = Transaction {
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
        signature: Vec::new(),
    };

    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = amir_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

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

// ============================================================
// NEW AUTHENTICATION TESTS
// ============================================================

// 1. valid signed transaction succeeds
#[test]
fn test_valid_signed_transaction_succeeds() {
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());
    let amir_wallet = TestWallet::new(1);

    let mut tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 50,
        }],
        signature: Vec::new(),
    };

    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = amir_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

    let res = runtime.execute_transaction(tx);
    assert!(res.is_ok());
}

// 2. attacker claims signer = "amir" but signs with attacker's private key
#[test]
fn test_attacker_signature_rejected() {
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());
    let attacker_wallet = TestWallet::new(3); // attacker's private key

    let mut tx = Transaction {
        signer: "amir".to_string(), // claiming to be amir
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 50,
        }],
        signature: Vec::new(),
    };

    let sign_bytes = tx.sign_bytes("tracechain-1");
    // attacker signs it
    let signature = attacker_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

    let res = runtime.execute_transaction(tx);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "invalid signature".to_string());
}

// 3. sign Deposit then mutate amount without signing again
#[test]
fn test_mutate_amount_invalidates_signature() {
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());
    let amir_wallet = TestWallet::new(1);

    let mut tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 100,
        }],
        signature: Vec::new(),
    };

    // Sign the original tx with 100
    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = amir_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

    // Mutate amount to 500 without signing again
    tx.calls = vec![RuntimeCall::Deposit {
        sender: "amir".to_string(),
        amount: 500,
    }];

    let res = runtime.execute_transaction(tx);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "invalid signature".to_string());
}

// 4. mutate nonce after signing
#[test]
fn test_mutate_nonce_invalidates_signature() {
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());
    let amir_wallet = TestWallet::new(1);

    let mut tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 100,
        }],
        signature: Vec::new(),
    };

    // Sign with nonce 0
    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = amir_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

    // Mutate nonce to 1
    tx.nonce = 1;

    let res = runtime.execute_transaction(tx);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "invalid signature".to_string());
}

// 5. mutate signer after signing
#[test]
fn test_mutate_signer_invalidates_signature() {
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());
    let amir_wallet = TestWallet::new(1);

    let mut tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 100,
        }],
        signature: Vec::new(),
    };

    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = amir_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

    // Mutate signer to ali
    tx.signer = "ali".to_string();

    let res = runtime.execute_transaction(tx);
    assert!(res.is_err());
    // Since we mutated signer, public key looked up will be Ali's, but signed by Amir -> invalid signature
    assert_eq!(res.unwrap_err(), "invalid signature".to_string());
}

// 6. sign for chain ID "tracechain-1", validate against "tracechain-2"
#[test]
fn test_different_chain_id_invalidates_signature() {
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-2".to_string()); // validator uses tracechain-2
    let amir_wallet = TestWallet::new(1);

    let mut tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 100,
        }],
        signature: Vec::new(),
    };

    // Signed for tracechain-1
    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = amir_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

    let res = runtime.execute_transaction(tx);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "invalid signature".to_string());
}

// 7. change call order after signing
#[test]
fn test_change_call_order_invalidates_signature() {
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());
    let amir_wallet = TestWallet::new(1);

    let mut tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![
            RuntimeCall::Deposit {
                sender: "amir".to_string(),
                amount: 100,
            },
            RuntimeCall::Withdraw {
                owner: "amir".to_string(),
                shares: 50,
            },
        ],
        signature: Vec::new(),
    };

    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = amir_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

    // Swap the calls order
    tx.calls.reverse();

    let res = runtime.execute_transaction(tx);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "invalid signature".to_string());
}

// 8. unknown signer with no registered public key
#[test]
fn test_unknown_signer_rejected() {
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());
    let unknown_wallet = TestWallet::new(99); // unregistered key

    let mut tx = Transaction {
        signer: "someone_else".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "someone_else".to_string(),
            amount: 100,
        }],
        signature: Vec::new(),
    };

    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = unknown_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

    let res = runtime.execute_transaction(tx);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "unknown signer".to_string());
}

// 9. correctly signed transaction from Amir containing Withdraw { owner: "ali", ... }
#[test]
fn test_authentication_vs_authorization() {
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());
    let amir_wallet = TestWallet::new(1);

    // Genuinely signed by Amir, but trying to withdraw from Ali's shares
    let mut tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Withdraw {
            owner: "ali".to_string(),
            shares: 50,
        }],
        signature: Vec::new(),
    };

    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = amir_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

    let res = runtime.execute_transaction(tx);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "unauthorized signer".to_string());
}

// 10. correctly signed transaction with incorrect nonce
#[test]
fn test_correctly_signed_with_incorrect_nonce() {
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());
    let amir_wallet = TestWallet::new(1);

    let mut tx = Transaction {
        signer: "amir".to_string(),
        nonce: 5, // expected is 0
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 100,
        }],
        signature: Vec::new(),
    };

    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = amir_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

    let res = runtime.execute_transaction(tx);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "invalid nonce".to_string());
}

// 11. replay a previously successful transaction after nonce increment
#[test]
fn test_replay_of_successful_transaction_rejected() {
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());
    let amir_wallet = TestWallet::new(1);

    let mut tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 100,
        }],
        signature: Vec::new(),
    };

    let sign_bytes = tx.sign_bytes("tracechain-1");
    let signature = amir_wallet.signing_key.sign(&sign_bytes);
    tx.signature = signature.to_bytes().to_vec();

    // First execution succeeds and increments nonce to 1
    let res1 = runtime.execute_transaction(tx.clone());
    assert!(res1.is_ok());
    assert_eq!(*runtime.state.accounts.nonces.get("amir").unwrap(), 1);

    // Replay the exact same transaction (still has nonce: 0)
    let res2 = runtime.execute_transaction(tx);
    assert!(res2.is_err());
    assert_eq!(res2.unwrap_err(), "invalid nonce".to_string());
}

// 12. invalid authentication leaves all committed state unchanged
#[test]
fn test_invalid_auth_leaves_state_unchanged() {
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());

    let tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 100,
        }],
        signature: vec![0u8; 64], // invalid dummy signature
    };

    let res = runtime.execute_transaction(tx);
    assert!(res.is_err());

    // State remains unchanged
    assert_eq!(*runtime.state.bank.balances.get("amir").unwrap(), 500);
    assert_eq!(*runtime.state.bank.balances.get(VAULT_ACCOUNT).unwrap(), 0);
}

// 13. invalid authentication does not increment nonce
#[test]
fn test_invalid_auth_does_not_increment_nonce() {
    let mut runtime = Runtime::new(initial_test_state(), "tracechain-1".to_string());

    let tx = Transaction {
        signer: "amir".to_string(),
        nonce: 0,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 100,
        }],
        signature: vec![0u8; 64], // invalid dummy signature
    };

    let res = runtime.execute_transaction(tx);
    assert!(res.is_err());
    assert_eq!(*runtime.state.accounts.nonces.get("amir").unwrap(), 0);
}

// 16. SignBytes are deterministic: same transaction + same chain ID => same bytes
#[test]
fn test_sign_bytes_deterministic() {
    let tx1 = Transaction {
        signer: "amir".to_string(),
        nonce: 3,
        calls: vec![
            RuntimeCall::Deposit {
                sender: "amir".to_string(),
                amount: 100,
            },
            RuntimeCall::Withdraw {
                owner: "amir".to_string(),
                shares: 50,
            },
        ],
        signature: Vec::new(),
    };

    let tx2 = tx1.clone();

    let bytes1 = tx1.sign_bytes("tracechain-1");
    let bytes2 = tx2.sign_bytes("tracechain-1");

    assert_eq!(bytes1, bytes2);
}

// 17. changing any signed transaction field changes SignBytes
#[test]
fn test_changing_fields_changes_sign_bytes() {
    let tx_base = Transaction {
        signer: "amir".to_string(),
        nonce: 3,
        calls: vec![RuntimeCall::Deposit {
            sender: "amir".to_string(),
            amount: 100,
        }],
        signature: Vec::new(),
    };

    let base_bytes = tx_base.sign_bytes("tracechain-1");

    // 1. change chain_id
    let bytes_diff_chain = tx_base.sign_bytes("tracechain-2");
    assert_ne!(base_bytes, bytes_diff_chain);

    // 2. change signer
    let mut tx_diff_signer = tx_base.clone();
    tx_diff_signer.signer = "ali".to_string();
    assert_ne!(base_bytes, tx_diff_signer.sign_bytes("tracechain-1"));

    // 3. change nonce
    let mut tx_diff_nonce = tx_base.clone();
    tx_diff_nonce.nonce = 4;
    assert_ne!(base_bytes, tx_diff_nonce.sign_bytes("tracechain-1"));

    // 4. change call amount
    let mut tx_diff_amount = tx_base.clone();
    tx_diff_amount.calls = vec![RuntimeCall::Deposit {
        sender: "amir".to_string(),
        amount: 200,
    }];
    assert_ne!(base_bytes, tx_diff_amount.sign_bytes("tracechain-1"));
}
