#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, symbol_short, xdr::ToXdr, Address,
    Bytes, BytesN, Env, IntoVal, String, Symbol,
};

mod storage_types;

soroban_sdk::contractmeta!(
    key = "Description",
    val = "Soulbound token registry for Stellar Wrap"
);
soroban_sdk::contractmeta!(key = "Version", val = "0.1.0");
soroban_sdk::contractmeta!(key = "Name", val = "Stellar Wrap Registry");
soroban_sdk::contractmeta!(key = "Author", val = "Stellar Wrap Team");

/// Errors returned by the StellarWrap contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    /// `initialize()` was called on a contract that is already initialized. (code 1)
    AlreadyInitialized = 1,
    /// A function was called before `initialize()` has been run. (code 2)
    NotInitialized = 2,
    /// The caller is not the admin. (code 3)
    Unauthorized = 3,
    /// A wrap record for this `(user, period)` pair already exists. (code 4)
    WrapAlreadyExists = 4,
    /// The wrap record was not found. (code 5)
    WrapNotFound = 5,
    /// The provided Ed25519 signature did not verify against the admin public key. (code 6)
    InvalidSignature = 6,
    /// `data_hash` is all-zero bytes, which indicates missing or invalid data. (code 7)
    InvalidDataHash = 7,
}


#[contract]
pub struct StellarWrapContract;

#[contractimpl]
impl StellarWrapContract {
    /// Initialize the contract with an admin address and the Ed25519 public key used to
    /// verify off-chain wrap signatures.
    ///
    /// # Parameters
    /// - `admin`: The `Address` that will have privileged control (upgrade, update_admin).
    /// - `admin_pubkey`: The 32-byte Ed25519 public key whose private key signs wrap payloads.
    ///
    /// # Panics
    /// - [`ContractError::AlreadyInitialized`] if called more than once.
    pub fn initialize(e: Env, admin: Address, admin_pubkey: BytesN<32>) {
        if e.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(e, ContractError::AlreadyInitialized);
        }
        e.storage().instance().set(&DataKey::Admin, &admin);
        e.storage()
            .instance()
            .set(&DataKey::AdminPubKey, &admin_pubkey);
        e.storage()
            .instance()
            .set(&DataKey::SchemaVersion, &SCHEMA_VERSION);
    }

    /// Replace the current admin with a new address.
    ///
    /// # Parameters
    /// - `new_admin`: The `Address` that will become the new admin.
    ///
    /// # Authorization
    /// Requires authorization from the **current** admin.
    ///
    /// # Panics
    /// - [`ContractError::NotInitialized`] if the contract has not been initialized.
    pub fn update_admin(e: Env, new_admin: Address) {
        let current_admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

        current_admin.require_auth();
        e.storage().instance().set(&DataKey::Admin, &new_admin);

        e.events().publish(
            (symbol_short!("admin"), symbol_short!("updated")),
            (current_admin, new_admin),
        );
    }

    /// Charge storage deposit units for a user before performing a persistent write.
    ///
    /// This is a lightweight DoS prevention mechanism: without it, an attacker can
    /// spam unique `(user, period)` keys to exhaust the contract's storage.
    ///
    /// The contract uses "deposit units" rather than trying to measure real
    /// Soroban storage cost.
    fn charge_storage_or_panic(e: &Env, user: &Address, amount: u64) {
        let total_budget: u64 = e
            .storage()
            .instance()
            .get(&DataKey::StorageBudgetTotal)
            .unwrap_or(0);
        let per_user_budget: u64 = e
            .storage()
            .instance()
            .get(&DataKey::StorageBudgetPerUser)
            .unwrap_or(0);

        // If budgets are unset/zero, treat as unlimited (backwards-compatible).
        if total_budget == 0 && per_user_budget == 0 {
            return;
        }

        let total_used: u64 = e
            .storage()
            .instance()
            .get(&DataKey::StorageDepositTotalUsed)
            .unwrap_or(0);

        let user_used: u64 = e
            .storage()
            .persistent()
            .get::<_, u64>(&DataKey::StorageDepositUsed(user.clone()))
            .unwrap_or(0);

        let new_total = total_used.saturating_add(amount);
        let new_user = user_used.saturating_add(amount);

        if total_budget != 0 && new_total > total_budget {
            panic_with_error!(e, ContractError::StorageDepositExceeded);
        }
        if per_user_budget != 0 && new_user > per_user_budget {
            panic_with_error!(e, ContractError::StorageDepositExceeded);
        }

        if total_budget != 0 {
            e.storage()
                .instance()
                .set(&DataKey::StorageDepositTotalUsed, &new_total);
        }
        if per_user_budget != 0 {
            e.storage()
                .persistent()
                .set(&DataKey::StorageDepositUsed(user.clone()), &new_user);
        }
    }


    /// Mint a soulbound wrap record for a user.
    ///
    /// The backend generates a payload of `(contract_id ‖ user ‖ period ‖ archetype ‖ data_hash)`,
    /// signs it with the admin Ed25519 private key, and delivers the signature to the user.
    /// The user then calls this function to claim their on-chain wrap record.
    ///
    /// **Invariant:** The admin must issue at most one signature per `(user, period)` pair.
    /// Only one archetype can ever be stored for a given period; a second valid signature for
    /// the same user+period with a different archetype is permanently unusable.
    /// See [Issue #31](https://github.com/zintarh/stellar-wrap-contract/issues/31).
    ///
    /// # Parameters
    /// - `user`: The `Address` receiving the wrap. Must authorize this call.
    /// - `period`: A `u64` identifier for the wrap period (e.g. `202412` for December 2024).
    /// - `archetype`: A short `Symbol` describing the user's persona (e.g. `"builder"`).
    /// - `data_hash`: SHA-256 hash of the off-chain JSON data. Must not be all-zero bytes.
    /// - `signature`: 64-byte Ed25519 signature from the admin over the canonical payload.
    ///
    /// # Returns
    /// Nothing on success. Emits a `(mint, user, period) → archetype` event.
    ///
    /// # Authorization
    /// Requires authorization from `user`.
    ///
    /// # Panics
    /// - [`ContractError::NotInitialized`] if the contract has not been initialized.
    /// - [`ContractError::InvalidDataHash`] if `data_hash` is all-zero bytes.
    /// - [`ContractError::InvalidSignature`] if the Ed25519 signature is invalid.
    /// - [`ContractError::WrapAlreadyExists`] if a wrap for `(user, period)` already exists.
    pub fn mint_wrap(
        e: Env,
        caller: Address,
        user: Address,
        period: u64,
        archetype: Symbol,
        data_hash: BytesN<32>,
        signature: BytesN<64>,
    ) {
        // 1. Caller (admin or minter) must authorize this invocation.
        caller.require_auth();

        // 1a. Enforce caller holds admin or minter role.
        {
            let admin: Address = e
                .storage()
                .instance()
                .get(&DataKey::Admin)
                .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));
            let is_minter = e
                .storage()
                .instance()
                .get::<_, bool>(&DataKey::Minter(caller.clone()))
                .unwrap_or(false);
            if caller != admin && !is_minter {
                panic_with_error!(e, ContractError::Unauthorized);
            }
        }

        // 2. Security: Ensure the user actually signed this transaction
        user.require_auth();

        // 1b. Reentrancy guard in temporary storage.
        // If execution panics, the temporary TTL naturally clears stale entries.
        let guard_key = DataKey::MintGuard(user.clone());
        if e.storage().temporary().has(&guard_key) {
            panic_with_error!(e, ContractError::Unauthorized);
        }
        e.storage().temporary().set(&guard_key, &true);

        // 2. Verify initialization
        let admin_pubkey: BytesN<32> = e
            .storage()
            .instance()
            .get(&DataKey::AdminPubKey)
            .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

        // 3. Reject zero data_hash — all-zero bytes indicate missing or invalid data
        if data_hash == BytesN::from_array(&e, &[0u8; 32]) {
            panic_with_error!(e, ContractError::InvalidDataHash);
        }

        // 4. Reconstruct payload: contract_id ‖ user ‖ period ‖ archetype ‖ data_hash
        let mut payload = Bytes::new(&e);
        payload.append(&e.current_contract_address().to_xdr(&e));
        payload.append(&user.clone().to_xdr(&e));
        payload.append(&period.to_xdr(&e));
        payload.append(&archetype.clone().to_xdr(&e));
        payload.append(&data_hash.clone().to_xdr(&e));

        // 5. Verify Admin Signature
        verify_signature(&e, &admin_pubkey, &payload, &signature);

        // 6. Check Duplicates & Store Record
        let wrap_key = DataKey::Wrap(user.clone(), period);
        if e.storage().persistent().has(&wrap_key) {
            panic_with_error!(e, ContractError::WrapAlreadyExists);
        }

        // DoS protection: charge before any new persistent writes.
        // We conservatively charge 3 units for the new persistent keys that
        // `persist_wrap_record` would add/update.
        Self::charge_storage_or_panic(&e, &user, 3);

        let record = WrapRecord {
            timestamp: e.ledger().timestamp(),
            data_hash,
            archetype: archetype.clone(),
            period,
            image_uri: String::from_str(&e, ""),
        };

        Self::persist_wrap_record(&e, user.clone(), period, record, archetype);

        e.storage().temporary().remove(&guard_key);
    }

    /// Publish a merkle root for batch wrap claims in a given period (admin-only).
    ///
    /// Off-chain, build a binary merkle tree over leaves defined as:
    /// `SHA-256(XDR(user) ‖ XDR(period) ‖ XDR(archetype) ‖ XDR(data_hash))`.
    /// Internal nodes use `SHA-256(min(h1,h2) ‖ max(h1,h2))` (lexicographic order).
    pub fn set_merkle_root(e: Env, period: u64, root: BytesN<32>) {
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));
        admin.require_auth();

        e.storage()
            .instance()
            .set(&DataKey::MerkleRoot(period), &root);

        e.events()
            .publish((symbol_short!("merkle"), symbol_short!("root"), period), root);
    }

    /// Claim a wrap using a merkle proof against a published root for `period`.
    ///
    /// Requires `user.require_auth()`. Produces the same `WrapRecord` and `mint` event as
    /// `mint_wrap`, without an admin signature per claim.
    pub fn claim_wrap(
        e: Env,
        user: Address,
        period: u64,
        archetype: Symbol,
        data_hash: BytesN<32>,
        proof: soroban_sdk::Vec<BytesN<32>>,
    ) {
        user.require_auth();

        let guard_key = DataKey::MintGuard(user.clone());
        if e.storage().temporary().has(&guard_key) {
            panic_with_error!(e, ContractError::Unauthorized);
        }
        e.storage().temporary().set(&guard_key, &true);

        e.storage()
            .instance()
            .get::<_, Address>(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

        if data_hash == BytesN::from_array(&e, &[0u8; 32]) {
            panic_with_error!(e, ContractError::InvalidDataHash);
        }

        let root: BytesN<32> = e
            .storage()
            .instance()
            .get(&DataKey::MerkleRoot(period))
            .unwrap_or_else(|| panic_with_error!(e, ContractError::MerkleRootNotSet));

        let leaf = compute_merkle_leaf(&e, &user, period, &archetype, &data_hash);
        if !verify_merkle_proof(&e, &root, &leaf, &proof) {
            panic_with_error!(e, ContractError::InvalidMerkleProof);
        }

        let claim_key = DataKey::MerkleClaimed(user.clone(), period);
        if e.storage().persistent().has(&claim_key) {
            panic_with_error!(e, ContractError::MerkleAlreadyClaimed);
        }

        let wrap_key = DataKey::Wrap(user.clone(), period);
        if e.storage().persistent().has(&wrap_key) {
            panic_with_error!(e, ContractError::WrapAlreadyExists);
        }

        let record = WrapRecord {
            timestamp: e.ledger().timestamp(),
            data_hash,
            archetype: archetype.clone(),
            period,
            image_uri: String::from_str(&e, ""),
        };

        e.storage()
            .persistent()
            .extend_ttl(&wrap_key, DEFAULT_TTL_LEDGERS, DEFAULT_TTL_LEDGERS);

        let count_key = DataKey::WrapCount(user.clone());
        e.storage()
            .persistent()
            .set(&count_key, &next_count);
        e.storage()
            .persistent()
            .extend_ttl(&count_key, DEFAULT_TTL_LEDGERS, DEFAULT_TTL_LEDGERS);

        let latest_key = DataKey::LatestPeriod(user.clone());
        if period > current_latest {
            e.storage().persistent().set(&latest_key, &period);
            e.storage()
                .persistent()
                .extend_ttl(&latest_key, DEFAULT_TTL_LEDGERS, DEFAULT_TTL_LEDGERS);
        }

        e.storage().persistent().set(&streak_key, &next_streak);
        e.storage()
            .persistent()
            .extend_ttl(&streak_key, ttl_one_year, ttl_one_year);

        // Update global counters
        let total: u64 = e
            .storage()
            .instance()
            .get(&DataKey::TotalMints)
            .unwrap_or(0);
        e.storage()
            .instance()
            .set(&DataKey::TotalMints, &(total + 1));
        e.storage()
            .instance()
            .set(&DataKey::LastMintTimestamp, &e.ledger().timestamp());

        e.events()
            .publish((symbol_short!("mint"), user, period), archetype);
    }

    fn load_wrap_record(e: &Env, user: &Address, period: u64) -> Option<WrapRecord> {
        let wrap_key = DataKey::Wrap(user.clone(), period);
        let schema = Self::schema_version(e);

        if schema < SCHEMA_VERSION_V2 {
            return e
                .storage()
                .persistent()
                .get::<_, WrapRecordV1>(&wrap_key)
                .map(|v1| Self::v1_to_v2(e, &v1));
        }

        // After migration, unread v1 records must be tried before v2 deserialization.
        if let Some(v1) = e.storage().persistent().get::<_, WrapRecordV1>(&wrap_key) {
            return Some(Self::migrate_v1_record(e, user, period, v1));
        }

        e.storage().persistent().get::<_, WrapRecord>(&wrap_key)
    }

    fn user_is_opted_out(e: &Env, user: &Address) -> bool {
        e.storage()
            .persistent()
            .get(&DataKey::UserOptOut(user.clone()))
            .unwrap_or(false)
    }

    /// Update an existing wrap record's data_hash and archetype (admin-only).
    ///
    /// The original `timestamp` is preserved. A new `update` event is emitted.
    ///
    /// # Parameters
    /// - `user`: The address whose wrap record is being updated.
    /// - `period`: The period identifier of the record to update.
    /// - `new_data_hash`: Replacement SHA-256 hash. Must not be all-zero bytes.
    /// - `new_archetype`: Replacement archetype symbol.
    /// - `signature`: 64-byte Ed25519 signature from the admin over the new payload.
    ///
    /// # Authorization
    /// Requires authorization from the **admin**.
    ///
    /// # Panics
    /// - [`ContractError::NotInitialized`] if the contract has not been initialized.
    /// - [`ContractError::Unauthorized`] if the caller is not the admin.
    /// - [`ContractError::InvalidDataHash`] if `new_data_hash` is all-zero bytes.
    /// - [`ContractError::InvalidSignature`] if the Ed25519 signature is invalid.
    /// - [`ContractError::WrapNotFound`] if no wrap exists for `(user, period)`.
    pub fn update_wrap(
        e: Env,
        user: Address,
        period: u64,
        new_data_hash: BytesN<32>,
        new_archetype: Symbol,
        signature: BytesN<64>,
    ) {
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));
        admin.require_auth();

        let admin_pubkey: BytesN<32> = e
            .storage()
            .instance()
            .get(&DataKey::AdminPubKey)
            .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

        if new_data_hash == BytesN::from_array(&e, &[0u8; 32]) {
            panic_with_error!(e, ContractError::InvalidDataHash);
        }

        // Payload: contract_id ‖ user ‖ period ‖ new_archetype ‖ new_data_hash
        let mut payload = Bytes::new(&e);
        payload.append(&e.current_contract_address().to_xdr(&e));
        payload.append(&user.clone().to_xdr(&e));
        payload.append(&period.to_xdr(&e));
        payload.append(&new_archetype.clone().to_xdr(&e));
        payload.append(&new_data_hash.clone().to_xdr(&e));
        verify_signature(&e, &admin_pubkey, &payload, &signature);

        let wrap_key = DataKey::Wrap(user.clone(), period);
        let existing: WrapRecord = Self::load_wrap_record(&e, &user, period)
            .unwrap_or_else(|| panic_with_error!(e, ContractError::WrapNotFound));

        let updated = WrapRecord {
            timestamp: existing.timestamp, // preserve original timestamp
            data_hash: new_data_hash,
            archetype: new_archetype.clone(),
            period,
            image_uri: existing.image_uri,
        };

        e.storage()
            .persistent()
            .extend_ttl(&wrap_key, DEFAULT_TTL_LEDGERS, DEFAULT_TTL_LEDGERS);

        e.events()
            .publish((symbol_short!("update"), user, period), new_archetype);
    }

    /// Store an auxiliary data hash for tiered verification (admin-only).
    ///
    /// A wrap's primary `data_hash` (set at mint) covers one view of the off-chain data.
    /// Some integrations need more than one hash per period — e.g. a `"summary"` hash over
    /// the top-line stats and a `"detail"` hash over the full activity log. Rather than
    /// minting a second wrap, the admin records each extra hash here under a `hash_type`
    /// label. Auxiliary hashes are stored in their own `DataKey::AuxHash` entries, so the
    /// `WrapRecord` layout and the `mint_wrap` signing payload are unchanged.
    ///
    /// Calling this again with the same `(user, period, hash_type)` overwrites the prior
    /// value, allowing the admin to correct a hash.
    ///
    /// # Parameters
    /// - `user`: The address whose wrap the aux hash belongs to.
    /// - `period`: The period identifier of the parent wrap.
    /// - `hash_type`: A short `Symbol` naming the tier (e.g. `"summary"`, `"detail"`).
    /// - `hash`: SHA-256 hash for this tier. Must not be all-zero bytes.
    ///
    /// # Authorization
    /// Requires authorization from the **admin**.
    ///
    /// # Panics
    /// - [`ContractError::NotInitialized`] if the contract has not been initialized.
    /// - [`ContractError::Unauthorized`] if the caller is not the admin.
    /// - [`ContractError::InvalidDataHash`] if `hash` is all-zero bytes.
    /// - [`ContractError::WrapNotFound`] if no wrap exists for `(user, period)`.
    pub fn set_aux_hash(e: Env, user: Address, period: u64, hash_type: Symbol, hash: BytesN<32>) {
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));
        admin.require_auth();

        if hash == BytesN::from_array(&e, &[0u8; 32]) {
            panic_with_error!(e, ContractError::InvalidDataHash);
        }

        // The aux hash supplements an existing wrap — refuse to create orphans.
        let wrap_key = DataKey::Wrap(user.clone(), period);
        if !e.storage().persistent().has(&wrap_key) {
            panic_with_error!(e, ContractError::WrapNotFound);
        }

        let aux_key = DataKey::AuxHash(user.clone(), period, hash_type.clone());
        let ttl_one_year = 17280 * 365;
        e.storage().persistent().set(&aux_key, &hash);
        e.storage()
            .persistent()
            .extend_ttl(&aux_key, ttl_one_year, ttl_one_year);

        e.events()
            .publish((symbol_short!("auxhash"), user, period), hash_type);
    }

    /// Retrieve an auxiliary data hash previously stored via [`set_aux_hash`].
    ///
    /// # Parameters
    /// - `user`: The address whose aux hash is requested.
    /// - `period`: The period identifier of the parent wrap.
    /// - `hash_type`: The tier label used when the hash was stored.
    ///
    /// # Returns
    /// `Some(BytesN<32>)` if a hash was stored for that `(user, period, hash_type)`, else `None`.
    pub fn get_aux_hash(
        e: Env,
        user: Address,
        period: u64,
        hash_type: Symbol,
    ) -> Option<BytesN<32>> {
        e.storage()
            .persistent()
            .get(&DataKey::AuxHash(user, period, hash_type))
    }

    /// Admin-only revocation for incorrect or fraudulent records.
    pub fn revoke_wrap(e: Env, user: Address, period: u64) {
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));
        admin.require_auth();

        let wrap_key = DataKey::Wrap(user.clone(), period);
        if !e.storage().persistent().has(&wrap_key) {
            panic_with_error!(e, ContractError::WrapNotFound);
        }

        e.storage().persistent().remove(&wrap_key);

        let count_key = DataKey::WrapCount(user.clone());
        let current_count: u32 = e
            .storage()
            .persistent()
            .get(&count_key)
            .unwrap_or(DEFAULT_COUNT);
        if current_count > 0 {
            e.storage()
                .persistent()
                .set(&count_key, &(current_count - 1));
        }

        let total: u64 = e
            .storage()
            .instance()
            .get(&DataKey::TotalMints)
            .unwrap_or(0);
        if total > 0 {
            e.storage()
                .instance()
                .set(&DataKey::TotalMints, &(total - 1));
        }

        // Streak is not recalculated on revoke to avoid expensive on-chain scans.
        // This means streak may temporarily remain stale after a removal.

        e.events()
            .publish((symbol_short!("revoke"), user, period), true);
    }

    // --- Read Functions ---

    /// Retrieve the wrap record for a specific `(user, period)` pair.
    ///
    /// # Parameters
    /// - `user`: The address whose wrap record is requested.
    /// - `period`: The period identifier to look up.
    ///
    /// # Returns
    /// `Some(WrapRecord)` if a record exists, `None` otherwise.
    pub fn get_wrap(e: Env, user: Address, period: u64) -> Option<WrapRecord> {
    }

    /// Return the total number of wrap records minted for a user (their SBT balance).
    ///
    /// # Parameters
    /// - `id`: The address to query.
    ///
    /// # Returns
    /// The number of wraps as `i128`. Returns `0` if the user has no wraps or the contract
    /// has not been initialized.
    pub fn balance_of(e: Env, id: Address) -> i128 {
        let count_key = DataKey::WrapCount(id);
        e.storage()
            .persistent()
            .get::<_, u32>(&count_key)
            .unwrap_or(DEFAULT_COUNT) as i128
    }

    /// Return the total number of wraps minted across all users.
    pub fn total_supply(e: Env) -> u64 {
        e.storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0)
    }

    /// Verify that the SHA-256 hash of `data` matches the `data_hash` stored in a wrap record.
    ///
    /// Useful for off-chain integrity checks: hash the original JSON off-chain, then call this
    /// to confirm the on-chain record matches without re-uploading the full data.
    ///
    /// # Parameters
    /// - `user`: The address whose wrap record is checked.
    /// - `period`: The period identifier to look up.
    /// - `data`: The raw bytes whose SHA-256 will be compared against the stored hash.
    ///
    /// # Returns
    /// `true` if the hash matches, `false` if it does not or if no record exists.
    pub fn verify_data(e: Env, user: Address, period: u64, data: Bytes) -> bool {
        match Self::load_wrap_record(&e, &user, period) {
            Some(record) => {
                let computed_hash = e.crypto().sha256(&data);
                record.data_hash == BytesN::from_array(&e, &computed_hash.to_array())
            }
            None => false,
        }
    }

    /// Verify `data` against a specific tier's hash, choosing which hash to check.
    ///
    /// Companion to [`verify_data`], which always checks the primary `WrapRecord.data_hash`.
    /// Pass the `hash_type` of any auxiliary hash stored via [`set_aux_hash`] (e.g.
    /// `"summary"` or `"detail"`) to verify against that tier instead. This lets an
    /// integrator confirm, say, the summary blob without holding the full detail blob.
    ///
    /// # Parameters
    /// - `user`: The address whose wrap is checked.
    /// - `period`: The period identifier to look up.
    /// - `hash_type`: The tier label of the auxiliary hash to compare against.
    /// - `data`: The raw bytes whose SHA-256 will be compared against the stored hash.
    ///
    /// # Returns
    /// `true` if the SHA-256 of `data` matches the stored aux hash, `false` if it does not
    /// or if no aux hash exists for that `(user, period, hash_type)`.
    pub fn verify_aux_data(
        e: Env,
        user: Address,
        period: u64,
        hash_type: Symbol,
        data: Bytes,
    ) -> bool {
        let aux: Option<BytesN<32>> = e
            .storage()
            .persistent()
            .get(&DataKey::AuxHash(user, period, hash_type));
        match aux {
            Some(stored_hash) => {
                let computed_hash = e.crypto().sha256(&data);
                stored_hash == BytesN::from_array(&e, &computed_hash.to_array())
            }
            None => false,
        }
    }

    /// Return the most recent wrap record minted for a user (highest period value).
    ///
    /// # Parameters
    /// - `user`: The address to query.
    ///
    /// # Returns
    /// `Some(WrapRecord)` for the latest period, or `None` if the user has no wraps.
    pub fn get_latest_wrap(e: Env, user: Address) -> Option<WrapRecord> {
        if Self::user_is_opted_out(&e, &user) {
            return None;
        }
        let latest_key = DataKey::LatestPeriod(user.clone());
        let period: u64 = e.storage().persistent().get(&latest_key)?;
        Self::load_wrap_record(&e, &user, period)
    }

    /// Extend the TTL (time-to-live) for all persistent storage entries belonging to a user.
    ///
    /// Soroban persistent storage entries expire after their TTL lapses. This function lets
    /// anyone renew a user's wrap records so they remain accessible indefinitely.
    ///
    /// # Parameters
    /// - `user`: The address whose storage entries will be extended.
    /// - `period`: The specific wrap period whose record TTL will be extended.
    pub fn extend_ttl(e: Env, user: Address, period: u64) {
        let wrap_key = DataKey::Wrap(user.clone(), period);

        if e.storage().persistent().has(&wrap_key) {
            e.storage()
                .persistent()
                .extend_ttl(&wrap_key, DEFAULT_TTL_LEDGERS, DEFAULT_TTL_LEDGERS);
        }

        let count_key = DataKey::WrapCount(user.clone());
        if e.storage().persistent().has(&count_key) {
            e.storage()
                .persistent()
                .extend_ttl(&count_key, DEFAULT_TTL_LEDGERS, DEFAULT_TTL_LEDGERS);
        }

        let latest_key = DataKey::LatestPeriod(user.clone());
        if e.storage().persistent().has(&latest_key) {
            e.storage()
                .persistent()
                .extend_ttl(&latest_key, DEFAULT_TTL_LEDGERS, DEFAULT_TTL_LEDGERS);
        }

    }

    /// Return the current consecutive wrap streak for a user.
    pub fn get_streak(e: Env, user: Address) -> u32 {
        if Self::user_is_opted_out(&e, &user) {
            return 0;
        }
        e.storage()
            .persistent()
            .get::<_, u32>(&DataKey::WrapStreak(user))
            .unwrap_or(0)
    }

    /// Return the current admin address, or `None` if the contract is not yet initialized.
    pub fn get_admin(e: Env) -> Option<Address> {
        e.storage().instance().get(&DataKey::Admin)
    }

    /// Return aggregate health metrics for admin monitoring and frontend dashboards.
    ///
    /// All fields are read from instance storage — no auth required.
    ///
    /// # Returns
    /// A [`ContractStats`] struct containing:
    /// - `total_mints`: current count of active (non-revoked) wraps across all users.
    /// - `admin`: the current admin address.
    /// - `is_initialized`: whether [`initialize`] has been called.
    /// - `last_mint_timestamp`: ledger timestamp of the most recent successful mint, or `None`.
    /// - `schema_version`: storage schema version set at initialization.
    pub fn get_contract_stats(e: Env) -> ContractStats {
        let admin: Option<Address> = e.storage().instance().get(&DataKey::Admin);
        let is_initialized = admin.is_some();
        let total_mints: u64 = e
            .storage()
            .instance()
            .get(&DataKey::TotalMints)
            .unwrap_or(0);
        let last_mint_timestamp: Option<u64> =
            e.storage().instance().get(&DataKey::LastMintTimestamp);
        let schema_version: u32 = e
            .storage()
            .instance()
            .get(&DataKey::SchemaVersion)
            .unwrap_or(0);

        ContractStats {
            total_mints,
            admin,
            is_initialized,
            last_mint_timestamp,
            schema_version,
        }
    }

    /// Return the ledger timestamp of the most recent successful mint, or `None` if no mint
    /// has occurred yet.
    pub fn get_last_mint_timestamp(e: Env) -> Option<u64> {
        e.storage().instance().get(&DataKey::LastMintTimestamp)
    }

    /// Return the human-readable name of this token registry.
    ///
    /// # Returns
    /// `"Stellar Wrap Registry"`
    pub fn name(e: Env) -> String {
        String::from_str(&e, "Stellar Wrap Registry")
    }

    /// Return the ticker symbol for this token registry.
    ///
    /// # Returns
    /// `"WRAP"`
    pub fn symbol(e: Env) -> String {
        String::from_str(&e, "WRAP")
    }

    /// Return the number of decimals. Soulbound tokens are non-divisible, so this is always `0`.
    pub fn decimals(_e: Env) -> u32 {
        0
    }

    /// Return contract-level metadata useful for explorers and indexers.
    pub fn contract_info(e: Env) -> ContractInfo {
        ContractInfo {
            name: String::from_str(&e, "Stellar Wrap Registry"),
            version: String::from_str(&e, "0.1.0"),
            repo: String::from_str(&e, "https://github.com/zintarh/stellar-wrap-contract"),
            description: String::from_str(&e, "Soulbound token registry for Stellar Wrap"),
        }
    }

    /// Upgrade the contract WASM to a new version.
    ///
    /// The Soroban runtime validates the WASM hash against the uploaded blob in the ledger.
    /// After a successful upgrade, subsequent invocations run the new WASM code while all
    /// persistent storage (wrap records, admin key, etc.) is preserved.
    ///
    /// # Parameters
    /// - `new_wasm_hash`: The 32-byte hash of the new WASM blob as uploaded to the network.
    ///
    /// # Authorization
    /// Requires authorization from the **current** admin.
    ///
    /// # Panics
    /// - [`ContractError::NotInitialized`] if the contract has not been initialized.
    pub fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

        admin.require_auth();
        e.deployer().update_current_contract_wasm(new_wasm_hash);
    }

    /// Verify an Ed25519 signature. Exposes the host `ed25519_verify` function
    /// as a public contract method so that it can be invoked via `try_invoke_contract`
    /// to catch panics.
    pub fn verify_sig(
        e: Env,
        public_key: BytesN<32>,
        payload: Bytes,
        signature: BytesN<64>,
    ) {
        e.crypto()
            .ed25519_verify(&public_key, &payload, &signature);
    }
}

/// Helper to verify Ed25519 signature and handle panic explicitly.
fn verify_signature(
    e: &Env,
    admin_pubkey: &BytesN<32>,
    payload: &Bytes,
    signature: &BytesN<64>,
) {
    // 1. Input validation first:
    // Ensure public key is not all zeros
    if admin_pubkey == &BytesN::from_array(e, &[0u8; 32]) {
        panic_with_error!(e, ContractError::InvalidSignature);
    }

    // Ensure signature is not all zeros
    if signature == &BytesN::from_array(e, &[0u8; 64]) {
        panic_with_error!(e, ContractError::InvalidSignature);
    }

    // Validate signature S-value scalar range (Ed25519 malleability protection / range check)
    let s_part = signature.slice(32..64);
    const L: [u8; 32] = [
        0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
    ];
    let mut s_array = [0u8; 32];
    s_part.copy_into_slice(&mut s_array);
    
    let mut is_s_valid = false;
    for i in (0..32).rev() {
        if s_array[i] < L[i] {
            is_s_valid = true;
            break;
        } else if s_array[i] > L[i] {
            break;
        }
    }
    
    if !is_s_valid {
        panic_with_error!(e, ContractError::InvalidSignature);
    }

    // 2. Call verify_sig via try_invoke_contract to catch host-level verification panics
    // and explicitly map them to ContractError::InvalidSignature.
    let current_address = e.current_contract_address();
    let func = Symbol::new(e, "verify_sig");
    let args = (admin_pubkey.clone(), payload.clone(), signature.clone()).into_val(e);

    let result = e.try_invoke_contract::<(), soroban_sdk::Error>(&current_address, &func, args);
    match result {
        Ok(Ok(())) => {}
        _ => panic_with_error!(e, ContractError::InvalidSignature),
    }
}

#[cfg(test)]
mod security_test;
#[cfg(test)]
