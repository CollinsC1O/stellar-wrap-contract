use soroban_sdk::BytesN;

/// Stellar targets a ledger close roughly every 5 seconds, which is 17,280 ledgers per day.
pub(crate) const LEDGERS_PER_DAY: u32 = 17_280;

/// Default instance storage lifetime for wrap records, expressed in days.
pub(crate) const TTL_DAYS: u32 = 365;

/// Default wrap storage lifetime, approximately one year of Stellar ledgers.
pub(crate) const DEFAULT_TTL_LEDGERS: u32 = LEDGERS_PER_DAY * TTL_DAYS;

/// SHA-256 hash length in bytes.
pub const HASH_BYTES: usize = 32;

/// Fixed-size SHA-256 hash value used for off-chain wrap data and WASM hashes.
pub type Sha256Hash = BytesN<32>;

/// Ed25519 signature length in bytes.
pub const ED25519_SIGNATURE_BYTES: usize = 64;

/// Default value for counters before any records are created.
pub(crate) const DEFAULT_COUNTER_VALUE: u32 = 0;

/// Amount added to a user's wrap count after each successful mint.
pub(crate) const USER_COUNT_INCREMENT: u32 = 1;

/// Number of hash bytes shown by `WrapRecord` display output.
pub(crate) const HASH_PREVIEW_BYTES: usize = 4;

/// Zero-filled SHA-256 hash sentinel used to reject missing wrap data.
pub(crate) const ZERO_HASH_BYTES: [u8; HASH_BYTES] = [0u8; HASH_BYTES];

/// Soulbound wrap records are non-divisible.
pub(crate) const TOKEN_DECIMALS: u32 = 0;
