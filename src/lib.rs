use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::collections::HashMap;

// ============================================================
// 1. TYPE ALIASES
// ============================================================

pub type AccountId = String;
pub type Balance = u64;
pub type Shares = u64;
pub type Nonce = u64;

pub const VAULT_ACCOUNT: &str = "vault";

// ============================================================
// EDUCATIONAL HELPER
// ============================================================

pub fn write_string(bytes: &mut Vec<u8>, value: &str) {
    let length = value.len() as u64;
    let length_bytes = length.to_be_bytes();
    bytes.extend_from_slice(&length_bytes);
    bytes.extend_from_slice(value.as_bytes());
}

// ============================================================
// 2. BANK STATE
// ============================================================

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BankState {
    pub balances: HashMap<AccountId, Balance>,
}

// ============================================================
// 3. BANK MODULE
// ============================================================

pub struct BankModule<'a> {
    state: &'a mut BankState,
}

impl<'a> BankModule<'a> {
    pub fn new(state: &'a mut BankState) -> Self {
        Self { state }
    }

    pub fn balance(&self, account: &str) -> Balance {
        let found = self.state.balances.get(account);
        let balance_ref = found.unwrap_or(&0);
        let balance = *balance_ref;
        balance
    }

    pub fn send(&mut self, from: &str, to: &str, amount: Balance) -> Result<(), String> {
        let sender_balance = self.balance(from);
        if sender_balance < amount {
            return Err("insufficient balance".to_string());
        }

        let receiver_balance = self.balance(to);

        // Update sender using HashMap::insert
        self.state
            .balances
            .insert(from.to_string(), sender_balance - amount);

        // Update receiver using HashMap::insert
        self.state
            .balances
            .insert(to.to_string(), receiver_balance + amount);

        Ok(())
    }
}

// ============================================================
// 4. VAULT STATE
// ============================================================

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultState {
    pub shares: HashMap<AccountId, Shares>,
    pub total_shares: Shares,
}

// ============================================================
// 5. VAULT MODULE
// ============================================================

pub struct VaultModule<'a> {
    pub vault_state: &'a mut VaultState,
    pub bank_state: &'a mut BankState,
}

impl<'a> VaultModule<'a> {
    pub fn new(vault_state: &'a mut VaultState, bank_state: &'a mut BankState) -> Self {
        Self {
            vault_state,
            bank_state,
        }
    }

    pub fn total_assets(&self) -> Balance {
        let found = self.bank_state.balances.get(VAULT_ACCOUNT);
        let balance_ref = found.unwrap_or(&0);
        let balance = *balance_ref;
        balance
    }

    pub fn preview_deposit(&self, amount: Balance) -> Result<Shares, String> {
        if amount == 0 {
            return Err("amount cannot be zero".to_string());
        }

        let total_assets = self.total_assets();
        let total_shares = self.vault_state.total_shares;

        if total_shares == 0 {
            // first deposit is 1:1
            return Ok(amount);
        }

        if total_assets == 0 {
            return Err("invalid vault state".to_string());
        }

        let shares_to_add = amount * total_shares / total_assets;

        if shares_to_add == 0 {
            return Err("deposit too small".to_string());
        }

        Ok(shares_to_add)
    }

    pub fn deposit(&mut self, sender: &str, amount: Balance) -> Result<Shares, String> {
        // 1. Calculate shares before moving assets
        let shares_to_add = self.preview_deposit(amount)?;

        // 2. Create a short-lived BankModule scope
        {
            let mut bank = BankModule::new(self.bank_state);
            bank.send(sender, VAULT_ACCOUNT, amount)?;
        }

        // 3. Read sender current shares using get + unwrap_or(&0) using expanded syntax
        let found = self.vault_state.shares.get(sender);
        let current_shares_ref = found.unwrap_or(&0);
        let current_shares = *current_shares_ref;

        // 4. Calculate new_shares
        let new_shares = current_shares + shares_to_add;

        // 5. Store sender shares using insert
        self.vault_state
            .shares
            .insert(sender.to_string(), new_shares);

        // 6. Increase total_shares
        self.vault_state.total_shares += shares_to_add;

        // 7. Return Ok
        Ok(shares_to_add)
    }

    pub fn preview_withdraw(&self, shares: Shares) -> Result<Balance, String> {
        if shares == 0 {
            return Err("shares cannot be zero".to_string());
        }

        let total_shares = self.vault_state.total_shares;
        if total_shares == 0 {
            return Err("invalid vault state".to_string());
        }

        let total_assets = self.total_assets();
        let assets_out = shares * total_assets / total_shares;

        if assets_out == 0 {
            return Err("withdraw too small".to_string());
        }

        Ok(assets_out)
    }

    pub fn withdraw(&mut self, owner: &str, shares_to_burn: Shares) -> Result<Balance, String> {
        // 1. Read owner shares using get + unwrap_or(&0)
        let found = self.vault_state.shares.get(owner);
        let current_shares_ref = found.unwrap_or(&0);
        let current_shares = *current_shares_ref;

        // 2. If owner_shares < shares_to_burn
        if current_shares < shares_to_burn {
            return Err("insufficient shares".to_string());
        }

        // 3. Calculate assets_out using preview_withdraw
        let assets_out = self.preview_withdraw(shares_to_burn)?;

        // 4. Create short-lived BankModule
        {
            let mut bank = BankModule::new(self.bank_state);
            bank.send(VAULT_ACCOUNT, owner, assets_out)?;
        }

        // 5. Calculate new owner shares
        let new_shares = current_shares - shares_to_burn;

        // 6. Update HashMap
        self.vault_state
            .shares
            .insert(owner.to_string(), new_shares);

        // 7. Decrease total_shares
        self.vault_state.total_shares -= shares_to_burn;

        // 8. Return Ok
        Ok(assets_out)
    }
}

// ============================================================
// 8. ACCOUNT STATE
// ============================================================

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountState {
    pub nonces: HashMap<AccountId, Nonce>,
    pub public_keys: HashMap<AccountId, [u8; 32]>,
}

// ============================================================
// 9. ACCOUNT MODULE
// ============================================================

pub struct AccountModule<'a> {
    state: &'a mut AccountState,
}

impl<'a> AccountModule<'a> {
    pub fn new(state: &'a mut AccountState) -> Self {
        Self { state }
    }

    pub fn nonce(&self, account: &str) -> Nonce {
        let found = self.state.nonces.get(account);
        let nonce_ref = found.unwrap_or(&0);
        let nonce = *nonce_ref;
        nonce
    }

    pub fn increment_nonce(&mut self, account: &str) {
        let current = self.nonce(account);
        self.state.nonces.insert(account.to_string(), current + 1);
    }
}

// ============================================================
// 10. RUNTIME CALL
// ============================================================

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeCall {
    Deposit { sender: AccountId, amount: Balance },
    Withdraw { owner: AccountId, shares: Shares },
}

impl RuntimeCall {
    pub fn required_signer(&self) -> &str {
        match self {
            RuntimeCall::Deposit { sender, .. } => sender,
            RuntimeCall::Withdraw { owner, .. } => owner,
        }
    }
}

// ============================================================
// 11. TRANSACTION
// ============================================================

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transaction {
    pub signer: AccountId,
    pub nonce: Nonce,
    pub calls: Vec<RuntimeCall>,
    pub signature: Vec<u8>,
}

impl Transaction {
    pub fn sign_bytes(&self, chain_id: &str) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::new();

        // 1. chain_id
        write_string(&mut bytes, chain_id);

        // 2. signer
        write_string(&mut bytes, &self.signer);

        // 3. nonce
        bytes.extend_from_slice(&self.nonce.to_be_bytes());

        // 4. number of calls
        let num_calls = self.calls.len() as u64;
        bytes.extend_from_slice(&num_calls.to_be_bytes());

        // 5. every call in transaction order
        for call in &self.calls {
            match call {
                RuntimeCall::Deposit { sender, amount } => {
                    write_string(&mut bytes, "deposit");
                    write_string(&mut bytes, sender);
                    bytes.extend_from_slice(&amount.to_be_bytes());
                }
                RuntimeCall::Withdraw { owner, shares } => {
                    write_string(&mut bytes, "withdraw");
                    write_string(&mut bytes, owner);
                    bytes.extend_from_slice(&shares.to_be_bytes());
                }
            }
        }

        bytes
    }
}

// ============================================================
// SIGNATURE VERIFICATION HELPER
// ============================================================

pub fn verify_signature(
    tx: &Transaction,
    public_key_bytes: &[u8; 32],
    sign_bytes: &[u8],
) -> Result<(), String> {
    // 1. Convert public-key bytes into an Ed25519 VerifyingKey.
    let verifying_key_res = VerifyingKey::from_bytes(public_key_bytes);
    let verifying_key = match verifying_key_res {
        Ok(key) => key,
        Err(_) => {
            return Err("invalid public key".to_string());
        }
    };

    // 2. Convert the raw signature bytes into the library Signature type.
    let raw_signature = tx.signature.as_slice();
    let signature_res = Signature::from_slice(raw_signature);
    let signature = match signature_res {
        Ok(sig) => sig,
        Err(_) => {
            return Err("invalid signature".to_string());
        }
    };

    // 3. Cryptographically verify public key + sign bytes + signature.
    let verification_res = verifying_key.verify(sign_bytes, &signature);
    match verification_res {
        Ok(()) => Ok(()),
        Err(_) => Err("invalid signature".to_string()),
    }
}

// ============================================================
// 12. TRANSACTION VALIDATOR
// ============================================================

pub struct TransactionValidator;

impl TransactionValidator {
    pub fn validate(
        tx: &Transaction,
        account_state: &AccountState,
        chain_id: &str,
    ) -> Result<(), String> {
        // STEP 1 — Registered Public Key
        let found_pubkey = account_state.public_keys.get(&tx.signer);
        let public_key = match found_pubkey {
            Some(key) => key,
            None => {
                return Err("unknown signer".to_string());
            }
        };

        // STEP 2 — Rebuild SignBytes
        let sign_bytes = tx.sign_bytes(chain_id);

        // STEP 3 — Authentication
        verify_signature(tx, public_key, &sign_bytes)?;

        // STEP 4 — Authorization
        for call in &tx.calls {
            let required_signer = call.required_signer();
            if required_signer != tx.signer {
                return Err("unauthorized signer".to_string());
            }
        }

        // STEP 5 — Replay Protection
        let found_nonce = account_state.nonces.get(&tx.signer);
        let expected_nonce_ref = found_nonce.unwrap_or(&0);
        let expected_nonce = *expected_nonce_ref;

        if tx.nonce != expected_nonce {
            return Err("invalid nonce".to_string());
        }

        Ok(())
    }
}

// ============================================================
// 13. SIMPLE CALL ROUTER
// ============================================================

pub fn execute_call(vault: &mut VaultModule<'_>, call: RuntimeCall) -> Result<u64, String> {
    match call {
        RuntimeCall::Deposit { sender, amount } => vault.deposit(&sender, amount),
        RuntimeCall::Withdraw { owner, shares } => vault.withdraw(&owner, shares),
    }
}

// ============================================================
// 14. RUNTIME STATE
// ============================================================

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeState {
    pub bank: BankState,
    pub vault: VaultState,
    pub accounts: AccountState,
}

// ============================================================
// 15. RUNTIME
// ============================================================

pub struct Runtime {
    pub state: RuntimeState,
    pub chain_id: String,
}

impl Runtime {
    pub fn new(state: RuntimeState, chain_id: String) -> Self {
        Self { state, chain_id }
    }

    // ============================================================
    // 16. ATOMIC EXECUTION & 17. CLONE EXPLANATION
    // ============================================================
    //
    // CLONE EXPLANATION:
    // Deriving `Clone` on our state structs allows us to easily duplicate the entire committed state:
    // `let mut cached_state = self.state.clone();`
    // Because `HashMap` in Rust owns its keys and values, the `clone()` operation performs a complete,
    // independent deep copy of all maps. Any mutation made to `cached_state` will not affect the original
    // `self.state` until/unless we explicitly commit it by overwriting `self.state = cached_state;`.
    //
    // This is conceptually identical to the deep-copy/cache-wrap model we used in our Go/Cosmos learning
    // implementation. It is an educational atomicity model to teach how transactional states roll back on errors,
    // and should not be mistaken for production Polkadot/Substrate where state is managed by highly optimized,
    // sparse Merkle Trie databases that write to disk on block finalization.
    //
    pub fn execute_transaction(&mut self, tx: Transaction) -> Result<Vec<u64>, String> {
        let mut cached_state = self.state.clone();

        TransactionValidator::validate(&tx, &cached_state.accounts, &self.chain_id)?;

        let signer = tx.signer.clone();
        let mut results: Vec<u64> = Vec::new();

        {
            let mut vault = VaultModule::new(&mut cached_state.vault, &mut cached_state.bank);

            for call in tx.calls {
                let result = execute_call(&mut vault, call)?;
                results.push(result);
            }
        }

        {
            let mut accounts = AccountModule::new(&mut cached_state.accounts);
            accounts.increment_nonce(&signer);
        }

        self.state = cached_state;

        Ok(results)
    }
}
