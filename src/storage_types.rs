use soroban_sdk::{contracttype, Address, BytesN, String, Symbol};

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

/// Period encoded as YYYYMM (e.g., 202512 = December 2025)
pub type WrapPeriod = u64;

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
    /// Optional on-chain metadata/notes (schema v3+)
    pub metadata: Option<String>,
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
    /// Tracks all periods for a user (default campaign)
    UserPeriods(Address),
    /// Stores campaign-specific WrapRecords
    CampaignWrap(Symbol, Address, u64),
    /// Stores the total number of campaign wraps for a specific user
    CampaignWrapCount(Symbol, Address),
    /// Tracks the latest (highest) campaign period minted for a user
    CampaignLatestPeriod(Symbol, Address),
    /// Tracks all periods for a user in a specific campaign
    CampaignUserPeriods(Symbol, Address),
    /// List of all campaigns created by admin
    Campaigns,
    /// Tracks the current consecutive wrap streak for a user
    WrapStreak(Address),
    /// Temporary, invocation-scoped reentrancy guard for mint flow
    MintGuard(Address),
    /// Merkle root for batch claims per period
    MerkleRoot(u64),
    /// Tracks whether a user has claimed via merkle for a period
    MerkleClaimed(Address, u64),
    /// User privacy opt-out flag (persistent)
    UserOptOut(Address),
}

/// Current schema version written by `initialize()` and advanced by `migrate()`.
pub const SCHEMA_VERSION: u32 = 1;
/// Target schema version after v1 → v2 migration (`image_uri` field).
pub const SCHEMA_VERSION_V2: u32 = 2;
/// Target schema version after v2 → v3 migration (`metadata` field).
pub const SCHEMA_VERSION_V3: u32 = 3;
