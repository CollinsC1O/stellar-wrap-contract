use core::fmt;

use soroban_sdk::{contracttype, Address, BytesN, String, Symbol};

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
pub struct WrapRecord {
    pub timestamp: u64,
    pub data_hash: BytesN<32>,
    pub archetype: Symbol,
    pub period: u64, // Standardized to u64 for better indexing/sorting
    /// Optional off-chain image URI (schema v2+). Empty for legacy records.
    pub image_uri: String,
}

impl fmt::Display for WrapRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hash = self.data_hash.to_array();

        write!(
            f,
            "WrapRecord {{ period: {}, archetype: {:?}, timestamp: {}, data_hash: ",
            self.period, self.archetype, self.timestamp
        )?;

        for byte in hash.iter().take(HASH_PREVIEW_BYTES) {
            write!(f, "{byte:02x}")?;
        }

        write!(f, "..., image_uri: {:?} }}", self.image_uri)
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WrapRecordOption {
    Some(WrapRecord),
    None,
}

impl WrapRecordOption {
    pub fn is_some(&self) -> bool {
        matches!(self, Self::Some(_))
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrapComparison {
    pub user_a_wrap: WrapRecordOption,
    pub user_b_wrap: WrapRecordOption,
    pub both_have_wrap: bool,
    pub same_archetype: bool,
    pub period: u64,
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
}

/// Current schema version written by `initialize()` and advanced by `migrate()`.
pub const SCHEMA_VERSION: u32 = 1;
/// Target schema version after v1 → v2 migration (`image_uri` field).
pub const SCHEMA_VERSION_V2: u32 = 2;

