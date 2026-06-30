
use crate::constants::HASH_PREVIEW_BYTES;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractStats {
    pub total_mints: u64,
    pub admin: Option<Address>,
    pub is_initialized: bool,
    pub last_mint_timestamp: Option<u64>,
    pub schema_version: u32,
}

/// Token metadata constants.
///
/// WARNING: Changing these values is a BREAKING CHANGE for indexers and
/// downstream consumers that rely on stable token identifiers. The `name()`,
/// `symbol()`, and `decimals()` functions must remain stable across contract
/// upgrades to preserve indexer compatibility.
pub const TOKEN_NAME: &str = "Stellar Wrap Registry";
pub const TOKEN_SYMBOL: &str = "WRAP";
pub const TOKEN_DECIMALS: u32 = 0;
pub const CONTRACT_DESCRIPTION: &str = "Soulbound token registry for Stellar Wrap";

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractInfo {
    pub name: String,
    pub version: String,
    pub repo: String,
    pub description: String,
}

/// Schema v1 wrap record (no `image_uri`). Retained for lazy migration reads.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrapRecordV1 {
    pub timestamp: u64,
    pub data_hash: BytesN<32>,
    pub archetype: Symbol,
    pub period: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrapRecordV2 {
    pub timestamp: u64,
    pub data_hash: BytesN<32>,
    pub archetype: Symbol,
    pub period: u64,
    pub image_uri: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrapRecord {
    pub timestamp: u64,
    pub data_hash: BytesN<32>,
    pub archetype: Symbol,
    pub period: WrapPeriod,
    pub image_uri: String,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Stores the Address of the admin
    Admin,
    /// Stores the BytesN<32> public key for Ed25519 verification
    AdminPubKey,
    /// Current storage schema version (instance storage)
    SchemaVersion,
    /// Stores individual WrapRecords (mapped by User and Period)
    /// Using u64 for period ensures consistent indexing
    Wrap(Address, u64),
    /// Stores the total number of wraps for a specific user (for balance_of)
    WrapCount(Address),
    /// Tracks the latest (highest) period minted for a user
    LatestPeriod(Address),
    /// Temporary, invocation-scoped reentrancy guard for mint flow
    MintGuard(Address),
    /// Global counter of currently active (non-revoked) minted wraps
    TotalMints,
    /// Ledger timestamp of the most recent successful mint
    LastMintTimestamp,
    /// Schema version set at initialization; bumped on breaking storage migrations
    SchemaVersion,
    /// Merkle root for batch claims per period
    MerkleRoot(u64),
    /// Tracks whether a user has claimed via merkle for a period
    MerkleClaimed(Address, u64),
    /// User privacy opt-out flag (persistent)
    UserOptOut(Address),
    /// Registered delegate address → Ed25519 public key for mint signatures
    Delegate(Address),
    /// Ordered list of registered delegate addresses (instance storage)
    DelegateList,
    /// Global count of wraps minted with this archetype
    ArchetypeCount(Symbol),
}

/// Current schema version written by `initialize()` and advanced by `migrate()`.
pub const SCHEMA_VERSION: u32 = 1;
/// Target schema version after v1 → v2 migration (`image_uri` field).
pub const SCHEMA_VERSION_V2: u32 = 2;
/// Target schema version after v2 → v3 migration (wrap record updates).
pub const SCHEMA_VERSION_V3: u32 = 3;
