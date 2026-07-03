#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, symbol_short, xdr::ToXdr, Address,
    Bytes, BytesN, Env, String, Symbol,
};

mod storage_types;
use storage_types::{ContractInfo, DataKey, WrapRecord};

soroban_sdk::contractmeta!(
    key = "Description",
    val = "Soulbound token registry for Stellar Wrap"
);
soroban_sdk::contractmeta!(key = "Version", val = "0.1.0");
soroban_sdk::contractmeta!(key = "Name", val = "Stellar Wrap Registry");
soroban_sdk::contractmeta!(key = "Author", val = "Stellar Wrap Team");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    WrapAlreadyExists = 4,
    WrapNotFound = 5,
    InvalidSignature = 6,
    InvalidDataHash = 7,
}

#[contract]
pub struct StellarWrapContract;

#[contractimpl]
impl StellarWrapContract {
    pub fn initialize(e: Env, admin: Address, admin_pubkey: BytesN<32>) {
        if e.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(e, ContractError::AlreadyInitialized);
        }
        e.storage().instance().set(&DataKey::Admin, &admin);
        e.storage()
            .instance()
            .set(&DataKey::AdminPubKey, &admin_pubkey);
    }

    pub fn update_admin(e: Env, new_admin: Address) {
        let current_admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

        current_admin.require_auth();
        e.storage().instance().set(&DataKey::Admin, &new_admin);

        e.events().publish(
            (
                symbol_short!("v1"),
                symbol_short!("admin"),
                symbol_short!("updated"),
            ),
            (current_admin, new_admin),
        );
    }

    pub fn mint_wrap(
        e: Env,
        user: Address,
        period: u64,
        archetype: Symbol,
        data_hash: BytesN<32>,
        signature: BytesN<64>,
    ) {
        user.require_auth();

        let guard_key = DataKey::MintGuard(user.clone());
        if e.storage().temporary().has(&guard_key) {
            panic_with_error!(e, ContractError::Unauthorized);
        }
        e.storage().temporary().set(&guard_key, &true);

        let admin_pubkey: BytesN<32> = e
            .storage()
            .instance()
            .get(&DataKey::AdminPubKey)
            .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

        if data_hash == BytesN::from_array(&e, &[0u8; 32]) {
            panic_with_error!(e, ContractError::InvalidDataHash);
        }

        let mut payload = Bytes::new(&e);
        payload.append(&e.current_contract_address().to_xdr(&e));
        payload.append(&user.clone().to_xdr(&e));
        payload.append(&period.to_xdr(&e));
        payload.append(&archetype.clone().to_xdr(&e));
        payload.append(&data_hash.clone().to_xdr(&e));

        e.crypto()
            .ed25519_verify(&admin_pubkey, &payload, &signature);

        let wrap_key = DataKey::Wrap(user.clone(), period);
        if e.storage().persistent().has(&wrap_key) {
            panic_with_error!(e, ContractError::WrapAlreadyExists);
        }

        let record = WrapRecord {
            timestamp: e.ledger().timestamp(),
            data_hash,
            archetype: archetype.clone(),
            period,
        };

        let ttl_one_year = 17280 * 365;
        e.storage().persistent().set(&wrap_key, &record);
        e.storage()
            .persistent()
            .extend_ttl(&wrap_key, ttl_one_year, ttl_one_year);

        let count_key = DataKey::WrapCount(user.clone());
        let current_count: u32 = e.storage().persistent().get(&count_key).unwrap_or(0);
        e.storage()
            .persistent()
            .set(&count_key, &(current_count + 1));
        e.storage()
            .persistent()
            .extend_ttl(&count_key, ttl_one_year, ttl_one_year);

        let latest_key = DataKey::LatestPeriod(user.clone());
        let current_latest: u64 = e.storage().persistent().get(&latest_key).unwrap_or(0);
        if period > current_latest {
            e.storage().persistent().set(&latest_key, &period);
            e.storage()
                .persistent()
                .extend_ttl(&latest_key, ttl_one_year, ttl_one_year);
        }

        e.storage().temporary().remove(&guard_key);

        e.events().publish(
            (symbol_short!("v1"), symbol_short!("mint"), user, period),
            archetype,
        );
    }

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

        let mut payload = Bytes::new(&e);
        payload.append(&e.current_contract_address().to_xdr(&e));
        payload.append(&user.clone().to_xdr(&e));
        payload.append(&period.to_xdr(&e));
        payload.append(&new_archetype.clone().to_xdr(&e));
        payload.append(&new_data_hash.clone().to_xdr(&e));
        e.crypto()
            .ed25519_verify(&admin_pubkey, &payload, &signature);

        let wrap_key = DataKey::Wrap(user.clone(), period);
        let existing: WrapRecord = e
            .storage()
            .persistent()
            .get(&wrap_key)
            .unwrap_or_else(|| panic_with_error!(e, ContractError::WrapNotFound));

        let updated = WrapRecord {
            timestamp: existing.timestamp,
            data_hash: new_data_hash,
            archetype: new_archetype.clone(),
            period,
        };

        let ttl_one_year = 17280 * 365;
        e.storage().persistent().set(&wrap_key, &updated);
        e.storage()
            .persistent()
            .extend_ttl(&wrap_key, ttl_one_year, ttl_one_year);

        e.events().publish(
            (symbol_short!("v1"), symbol_short!("update"), user, period),
            new_archetype,
        );
    }

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
        let current_count: u32 = e.storage().persistent().get(&count_key).unwrap_or(0);
        if current_count > 0 {
            e.storage()
                .persistent()
                .set(&count_key, &(current_count - 1));
        }

        e.events().publish(
            (symbol_short!("v1"), symbol_short!("revoke"), user, period),
            true,
        );
    }

    pub fn get_wrap(e: Env, user: Address, period: u64) -> Option<WrapRecord> {
        e.storage().persistent().get(&DataKey::Wrap(user, period))
    }

    pub fn balance_of(e: Env, id: Address) -> i128 {
        let count_key = DataKey::WrapCount(id);
        e.storage()
            .persistent()
            .get::<_, u32>(&count_key)
            .unwrap_or(0) as i128
    }

    pub fn verify_data(e: Env, user: Address, period: u64, data: Bytes) -> bool {
        let wrap: Option<WrapRecord> = e.storage().persistent().get(&DataKey::Wrap(user, period));
        match wrap {
            Some(record) => {
                let computed_hash = e.crypto().sha256(&data);
                record.data_hash == BytesN::from_array(&e, &computed_hash.to_array())
            }
            None => false,
        }
    }

    pub fn get_latest_wrap(e: Env, user: Address) -> Option<WrapRecord> {
        let latest_key = DataKey::LatestPeriod(user.clone());
        let period: u64 = e.storage().persistent().get(&latest_key)?;
        e.storage().persistent().get(&DataKey::Wrap(user, period))
    }

    pub fn extend_ttl(e: Env, user: Address, period: u64) {
        let wrap_key = DataKey::Wrap(user.clone(), period);
        let ttl = 17280 * 365;

        if e.storage().persistent().has(&wrap_key) {
            e.storage().persistent().extend_ttl(&wrap_key, ttl, ttl);
        }

        let count_key = DataKey::WrapCount(user.clone());
        if e.storage().persistent().has(&count_key) {
            e.storage().persistent().extend_ttl(&count_key, ttl, ttl);
        }

        let latest_key = DataKey::LatestPeriod(user);
        if e.storage().persistent().has(&latest_key) {
            e.storage().persistent().extend_ttl(&latest_key, ttl, ttl);
        }

        e.storage().instance().extend_ttl(ttl, ttl);
    }

    pub fn get_admin(e: Env) -> Option<Address> {
        e.storage().instance().get(&DataKey::Admin)
    }

    pub fn name(e: Env) -> String {
        String::from_str(&e, "Stellar Wrap Registry")
    }

    pub fn symbol(e: Env) -> String {
        String::from_str(&e, "WRAP")
    }

    pub fn decimals(_e: Env) -> u32 {
        0
    }

    pub fn contract_info(e: Env) -> ContractInfo {
        ContractInfo {
            name: String::from_str(&e, "Stellar Wrap Registry"),
            version: String::from_str(&e, "0.1.0"),
            repo: String::from_str(&e, "https://github.com/zintarh/stellar-wrap-contract"),
            description: String::from_str(&e, "Soulbound token registry for Stellar Wrap"),
        }
    }

    pub fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

        admin.require_auth();
        e.deployer().update_current_contract_wasm(new_wasm_hash);
    }
}

#[cfg(test)]
mod security_test;
#[cfg(test)]
mod test;
