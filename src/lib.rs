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
}

// ============================================================
// 12. TRANSACTION VALIDATOR
// ============================================================

pub struct TransactionValidator;

impl TransactionValidator {
    pub fn validate(tx: &Transaction, account_state: &AccountState) -> Result<(), String> {
        // 1. For every call
        for call in &tx.calls {
            // 2. Get required_signer
            let required_signer = call.required_signer();

            // 3. If required_signer != tx.signer
            if required_signer != tx.signer {
                return Err("unauthorized signer".to_string());
            }
        }

        // 4. Read expected nonce from account_state using tx.signer
        let found = account_state.nonces.get(&tx.signer);
        let expected_nonce_ref = found.unwrap_or(&0);
        let expected_nonce = *expected_nonce_ref;

        // 5. If tx.nonce != expected nonce
        if tx.nonce != expected_nonce {
            return Err("invalid nonce".to_string());
        }

        // 6. Return Ok(())
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
}

impl Runtime {
    pub fn new(state: RuntimeState) -> Self {
        Self { state }
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

        TransactionValidator::validate(&tx, &cached_state.accounts)?;

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
