use soroban_sdk::{contracttype, Address};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    AdminPubKey,
    SchemaVersion,
    Wrap(Address, u64),
    WrapCount(Address),
    LatestPeriod(Address),
    MintGuard(Address),
    AllowedArchetypes,
    MerkleRoot(u64),
    MerkleClaimed(Address, u64),
    UserOptOut(Address),
}

pub const SCHEMA_VERSION: u32 = 1;