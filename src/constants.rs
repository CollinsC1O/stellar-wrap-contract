use soroban_sdk::{contracttype, Address, BytesN, String, Symbol};

/// Alias for the wrap period type
pub type WrapPeriod = u64;

/// Aggregate contract stats
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractStats {
    pub total_mints: u64,
    pub admin: Option<Address>,
    pub is_initialized: bool,
    pub last_mint_timestamp: Option<u64>,
    pub schema_version: u32,
}

/// Static contract metadata
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractInfo {
    pub name: String,
    pub version: String,
    pub repo: String,
    pub description: String,
}

/// Result of comparing two wraps
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrapComparison {
    pub period: WrapPeriod,
    pub user_a_wrap: Option<WrapRecord>,
    pub user_b_wrap: Option<WrapRecord>,
    pub both_have_wrap: bool,
    pub same_archetype: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    AdminPubKey,
    SchemaVersion,
    Wrap(Address, WrapPeriod),
    WrapCount(Address),
    LatestPeriod(Address),
    MintGuard(Address),
    AllowedArchetypes,
    MerkleRoot(WrapPeriod),
    MerkleClaimed(Address, WrapPeriod),
    UserOptOut(Address),
}

pub const SCHEMA_VERSION: u32 = 1;