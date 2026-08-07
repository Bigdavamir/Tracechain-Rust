use std::collections::HashMap;
use std::error::Error;
use std::fmt;

// =========================================================================
// 2. Data types
// =========================================================================

pub type AccountId = String;
pub type Balance = u64;
pub type Shares = u64;
pub type Nonce = u64;

pub const VAULT_ACCOUNT: &str = "vault";

// =========================================================================
// 3. RuntimeCall
// =========================================================================

/// An enum representing the closed set of calls/messages the runtime can dispatch.
/// Unlike Cosmos/Go where messages are represented as interface implementations,
/// Rust idiomatic designs leverage enums to represent variant dispatching.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeCall {
    Deposit { sender: AccountId, amount: Balance },
    Withdraw { owner: AccountId, shares: Shares },
}

impl RuntimeCall {
    /// Determines who is required to sign this specific call.
    pub fn required_signer(&self) -> &str {
        match self {
            RuntimeCall::Deposit { sender, .. } => sender,
            RuntimeCall::Withdraw { owner, .. } => owner,
        }
    }
}

// =========================================================================
// 4. Transaction
// =========================================================================

/// A Transaction holds the signer, nonce, and multiple calls to run sequentially.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transaction {
    pub signer: AccountId,
    pub nonce: Nonce,
    pub calls: Vec<RuntimeCall>,
}

// =========================================================================
// 5. State
// =========================================================================

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BankState {
    pub balances: HashMap<AccountId, Balance>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultState {
    pub shares: HashMap<AccountId, Shares>,
    pub total_shares: Shares,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountState {
    pub nonces: HashMap<AccountId, Nonce>,
}

/// The consolidated state of the entire runtime.
///
/// **Educational Note on Go Maps vs Rust HashMaps:**
/// In Go, maps are reference types. Creating a copy of a struct that contains a map
/// still references the same underlying map data, which requires a manual deep-copy loop.
/// In Rust, `HashMap` implements `Clone` by performing a full, safe deep copy of all its keys
/// and values. Thus, `self.state.clone()` creates an entirely independent, isolated duplicate
/// of the state that can be mutated safely in a temporary sandbox (e.g., during validation/execution)
/// without affecting the committed state until explicitly overwritten.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeState {
    pub bank: BankState,
    pub vault: VaultState,
    pub accounts: AccountState,
}

// =========================================================================
// 6. Error type
// =========================================================================

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum RuntimeError {
    EmptySigner,
    EmptyCalls,
    UnauthorizedSigner { call_index: usize },
    InvalidNonce { expected: Nonce, received: Nonce },
    ZeroAmount,
    InsufficientBalance,
    ZeroShares,
    InsufficientShares,
    InvalidVaultState,
    DepositTooSmall,
    WithdrawTooSmall,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::EmptySigner => write!(f, "Transaction signer cannot be empty"),
            RuntimeError::EmptyCalls => write!(f, "Transaction calls cannot be empty"),
            RuntimeError::UnauthorizedSigner { call_index } => {
                write!(
                    f,
                    "Call at index {} is not signed by authorized transaction signer",
                    call_index
                )
            }
            RuntimeError::InvalidNonce { expected, received } => {
                write!(
                    f,
                    "Invalid nonce: expected {}, received {}",
                    expected, received
                )
            }
            RuntimeError::ZeroAmount => write!(f, "Operation amount cannot be zero"),
            RuntimeError::InsufficientBalance => {
                write!(f, "Insufficient bank balance for this transfer")
            }
            RuntimeError::ZeroShares => write!(f, "Operation shares cannot be zero"),
            RuntimeError::InsufficientShares => write!(f, "Insufficient shares for this operation"),
            RuntimeError::InvalidVaultState => {
                write!(
                    f,
                    "Invalid vault state: positive shares but zero underlying assets"
                )
            }
            RuntimeError::DepositTooSmall => write!(f, "Deposit amount too small to issue shares"),
            RuntimeError::WithdrawTooSmall => {
                write!(
                    f,
                    "Withdrawing these shares results in zero assets returned"
                )
            }
        }
    }
}

impl Error for RuntimeError {}

// =========================================================================
// 7. BankModule
// =========================================================================

/// The BankModule manages balances and asset transfers.
///
/// **Educational Note on References and Lifetimes:**
/// `&'a mut BankState` is a mutable reference to the state owned elsewhere (inside the consolidated state).
/// The lifetime `'a` ensures that `BankModule` cannot outlive the borrowed `BankState`, preventing
/// use-after-free and dangling pointer bugs.
/// Rust's borrow checker strictly enforces that only ONE mutable reference can exist to a piece of data
/// at any time (aliasing prevention), which eliminates data races at compile time.
pub struct BankModule<'a> {
    state: &'a mut BankState,
}

impl<'a> BankModule<'a> {
    pub fn new(state: &'a mut BankState) -> Self {
        Self { state }
    }

    /// Read an account's balance.
    ///
    /// **Educational Note on `&self`:**
    /// `&self` is an immutable reference, meaning we can read data but cannot mutate it.
    /// Rust allows any number of concurrent immutable references (`&T`) to a value.
    pub fn balance(&self, account: &str) -> Balance {
        *self.state.balances.get(account).unwrap_or(&0)
    }

    /// Sends an amount from one account to another.
    ///
    /// **Educational Note on `&mut self`:**
    /// `&mut self` is a mutable reference, allowing exclusive mutation of the inner state.
    /// Because it requires exclusive access, no other code can read or write to `self` while this borrow is active.
    pub fn send(&mut self, from: &str, to: &str, amount: Balance) -> Result<(), RuntimeError> {
        if amount == 0 {
            return Err(RuntimeError::ZeroAmount);
        }

        let sender_balance = self.balance(from);
        if sender_balance < amount {
            return Err(RuntimeError::InsufficientBalance);
        }

        // Subtract from sender
        self.state
            .balances
            .insert(from.to_string(), sender_balance - amount);

        // Add to receiver
        let receiver_balance = self.balance(to);
        self.state
            .balances
            .insert(to.to_string(), receiver_balance + amount);

        Ok(())
    }
}

// =========================================================================
// 8. AccountModule
// =========================================================================

/// AccountModule manages account nonces to prevent replay attacks.
pub struct AccountModule<'a> {
    state: &'a mut AccountState,
}

impl<'a> AccountModule<'a> {
    pub fn new(state: &'a mut AccountState) -> Self {
        Self { state }
    }

    /// Read an account's nonce.
    pub fn nonce(&self, account: &str) -> Nonce {
        *self.state.nonces.get(account).unwrap_or(&0)
    }

    /// Increments the nonce for a specific account.
    pub fn increment_nonce(&mut self, account: &str) {
        let current = self.nonce(account);
        self.state.nonces.insert(account.to_string(), current + 1);
    }
}

// =========================================================================
// 9. VaultModule
// =========================================================================

/// VaultModule represents a basic tokenized vault, similar to ERC-4626.
/// It converts assets to shares on deposit and shares back to assets on withdrawal.
pub struct VaultModule<'a> {
    vault_state: &'a mut VaultState,
    bank_state: &'a mut BankState,
}

impl<'a> VaultModule<'a> {
    pub fn new(vault_state: &'a mut VaultState, bank_state: &'a mut BankState) -> Self {
        Self {
            vault_state,
            bank_state,
        }
    }

    /// Query the total assets held by the vault inside the bank.
    pub fn total_assets(&self) -> Balance {
        *self.bank_state.balances.get(VAULT_ACCOUNT).unwrap_or(&0)
    }

    /// Previews the shares that would be minted for a given asset amount.
    pub fn preview_deposit(&self, amount: Balance) -> Result<Shares, RuntimeError> {
        if amount == 0 {
            return Err(RuntimeError::ZeroAmount);
        }

        let total_assets = self.total_assets();
        let total_shares = self.vault_state.total_shares;

        if total_shares == 0 {
            // First deposit maps 1:1
            Ok(amount)
        } else {
            if total_assets == 0 {
                // If there are shares, there must be underlying assets. Otherwise, state is broken.
                return Err(RuntimeError::InvalidVaultState);
            }
            // shares = (amount * total_shares) / total_assets
            let shares = amount
                .checked_mul(total_shares)
                .ok_or(RuntimeError::DepositTooSmall)? // Protection against overflow
                / total_assets;

            if shares == 0 {
                return Err(RuntimeError::DepositTooSmall);
            }
            Ok(shares)
        }
    }

    /// Deposits assets from sender into the vault in exchange for shares.
    pub fn deposit(&mut self, sender: &str, amount: Balance) -> Result<Shares, RuntimeError> {
        // 1. Calculate shares BEFORE asset transfer
        let shares_to_add = self.preview_deposit(amount)?;

        // 2. Transfer assets from sender to vault account using BankModule
        //
        // **Borrow Checker Learning Note:**
        // We create a short lexical scope or temporary BankModule to perform the send operation.
        // We cannot keep `bank` alive across other operations if we want to mutate `vault_state` later,
        // as Rust prevents overlapping borrows. By instantiating BankModule locally, the mutable borrow
        // of `bank_state` ends immediately after `bank.send` completes, satisfying Rust's strict aliasing rules.
        {
            let mut bank = BankModule::new(self.bank_state);
            bank.send(sender, VAULT_ACCOUNT, amount)?;
        }

        // 3. Update vault accounting shares
        let user_shares = *self.vault_state.shares.get(sender).unwrap_or(&0);
        self.vault_state
            .shares
            .insert(sender.to_string(), user_shares + shares_to_add);

        // 4. Update total vault shares
        self.vault_state.total_shares += shares_to_add;

        Ok(shares_to_add)
    }

    /// Previews the assets that would be returned for burning a given amount of shares.
    pub fn preview_withdraw(&self, shares: Shares) -> Result<Balance, RuntimeError> {
        if shares == 0 {
            return Err(RuntimeError::ZeroShares);
        }

        let total_shares = self.vault_state.total_shares;
        if total_shares == 0 {
            return Err(RuntimeError::InvalidVaultState);
        }

        let total_assets = self.total_assets();
        let assets_out = shares
            .checked_mul(total_assets)
            .ok_or(RuntimeError::WithdrawTooSmall)?
            / total_shares;

        if assets_out == 0 {
            return Err(RuntimeError::WithdrawTooSmall);
        }
        Ok(assets_out)
    }

    /// Redeems shares for a corresponding amount of underlying assets from the vault.
    pub fn withdraw(
        &mut self,
        owner: &str,
        shares_to_burn: Shares,
    ) -> Result<Balance, RuntimeError> {
        let user_shares = *self.vault_state.shares.get(owner).unwrap_or(&0);
        if user_shares < shares_to_burn {
            return Err(RuntimeError::InsufficientShares);
        }

        // 1. Calculate assets out
        let assets_out = self.preview_withdraw(shares_to_burn)?;

        // 2. Transfer assets from vault to owner using BankModule
        {
            let mut bank = BankModule::new(self.bank_state);
            bank.send(VAULT_ACCOUNT, owner, assets_out)?;
        }

        // 3. Update vault accounting shares
        self.vault_state
            .shares
            .insert(owner.to_string(), user_shares - shares_to_burn);

        // 4. Update total vault shares
        self.vault_state.total_shares -= shares_to_burn;

        Ok(assets_out)
    }
}

// =========================================================================
// 10. TransactionValidator
// =========================================================================

pub struct TransactionValidator;

impl TransactionValidator {
    /// Validates a transaction against a state without modifying it.
    pub fn validate(tx: &Transaction, state: &RuntimeState) -> Result<(), RuntimeError> {
        if tx.signer.is_empty() {
            return Err(RuntimeError::EmptySigner);
        }

        if tx.calls.is_empty() {
            return Err(RuntimeError::EmptyCalls);
        }

        // Ensure every call's required signer matches transaction signer
        for (idx, call) in tx.calls.iter().enumerate() {
            if call.required_signer() != tx.signer {
                return Err(RuntimeError::UnauthorizedSigner { call_index: idx });
            }
        }

        // Verify Nonce
        let mut temp_accounts = state.accounts.clone();
        let account_module = AccountModule::new(&mut temp_accounts); // read-only check
        let expected_nonce = account_module.nonce(&tx.signer);
        if tx.nonce != expected_nonce {
            return Err(RuntimeError::InvalidNonce {
                expected: expected_nonce,
                received: tx.nonce,
            });
        }

        Ok(())
    }
}

// =========================================================================
// 11. Runtime
// =========================================================================

pub struct Runtime {
    pub state: RuntimeState,
}

impl Runtime {
    pub fn new(state: RuntimeState) -> Self {
        Self { state }
    }

    /// Executes a transaction atomically.
    ///
    /// Execution follows a Clone -> sandbox run -> Commit/Rollback strategy:
    /// 1. Clone the current committed state to create a temporary cache.
    /// 2. Validate transaction against this temporary state.
    /// 3. Execute all calls in order. If any call fails, return the error immediately, keeping committed state intact (Rollback).
    /// 4. If all calls succeed, increment the signer's nonce in the cache and overwrite the committed state (Commit).
    pub fn execute_transaction(&mut self, tx: Transaction) -> Result<Vec<u64>, RuntimeError> {
        // Step 1: Clone committed state
        let mut cached_state = self.state.clone();

        // Step 2: Validate the transaction against cached_state
        TransactionValidator::validate(&tx, &cached_state)?;

        // Step 3 & 4: Execute every RuntimeCall in order
        let mut results = Vec::new();

        for call in tx.calls {
            match call {
                RuntimeCall::Deposit { sender, amount } => {
                    // Create short lexical scopes to satisfy borrow-checker
                    let result = {
                        let mut vault =
                            VaultModule::new(&mut cached_state.vault, &mut cached_state.bank);
                        vault.deposit(&sender, amount)?
                    };
                    results.push(result);
                }
                RuntimeCall::Withdraw { owner, shares } => {
                    let result = {
                        let mut vault =
                            VaultModule::new(&mut cached_state.vault, &mut cached_state.bank);
                        vault.withdraw(&owner, shares)?
                    };
                    results.push(result);
                }
            }
        }

        // Step 7: Increment signer nonce by exactly 1 in cached_state
        {
            let mut accounts = AccountModule::new(&mut cached_state.accounts);
            accounts.increment_nonce(&tx.signer);
        }

        // Step 7 continued: Commit cached state to committed runtime state
        self.state = cached_state;

        Ok(results)
    }
}
