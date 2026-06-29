/// Number of ledgers produced per day on the Stellar network (approximately 5-second block time).
pub const LEDGERS_PER_DAY: u32 = 17280;

/// Number of days used as the default TTL period for persistent storage entries.
pub const TTL_DAYS: u32 = 365;

/// Default TTL in ledgers (~1 year). Used when setting or extending persistent storage entries.
/// Derived as `LEDGERS_PER_DAY * TTL_DAYS` = 17280 × 365 = 6,307,200 ledgers.
pub const DEFAULT_TTL_LEDGERS: u32 = LEDGERS_PER_DAY * TTL_DAYS;

/// Length in bytes of a SHA-256 hash and an Ed25519 public key.
pub const HASH_AND_KEY_LEN: usize = 32;

/// Length in bytes of an Ed25519 signature.
pub const SIGNATURE_LEN: usize = 64;

/// Default/initial counter value for wrap counts.
pub const DEFAULT_COUNT: u32 = 0;

/// Default/initial value for the latest period tracker.
pub const DEFAULT_LATEST_PERIOD: u64 = 0;
