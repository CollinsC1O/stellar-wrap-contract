use soroban_sdk::{contracttype, Address, BytesN, String, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractInfo {
    pub name: String,
    pub version: String,
    pub repo: String,
    pub description: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrapRecord {
    pub timestamp: u64,
    pub data_hash: BytesN<32>,
    pub archetype: Symbol,
    pub period: u64, // Standardized to u64 for better indexing/sorting
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Stores the Address of the admin
    Admin,
    /// Stores the BytesN<32> public key for Ed25519 verification
    AdminPubKey,
    /// Stores individual WrapRecords (mapped by User and Period)
    /// Using u64 for period ensures consistent indexing
    Wrap(Address, u64),
    /// Stores the total number of wraps for a specific user (for balance_of)
    WrapCount(Address),
    /// Tracks the latest (highest) period minted for a user
    LatestPeriod(Address),
    /// Auxiliary data hashes for tiered verification, keyed by
    /// (user, period, hash_type). The `hash_type` Symbol names the tier
    /// (e.g. "summary", "detail"). The primary hash stays in `WrapRecord.data_hash`.
    /// See [Issue #137](https://github.com/zintarh/stellar-wrap-contract/issues/137).
    AuxHash(Address, u64, Symbol),
    /// Temporary, invocation-scoped reentrancy guard for mint flow
    MintGuard(Address),
}
