/// Token name constant. Changing this is a BREAKING CHANGE for indexers.
pub const TOKEN_NAME: &str = "Stellar Wrap Registry";

/// Token symbol constant. Changing this is a BREAKING CHANGE for indexers.
pub const TOKEN_SYMBOL: &str = "WRAP";

/// Token decimals constant. Soulbound tokens are non-divisible.
pub const TOKEN_DECIMALS: u32 = 0;

/// Contract description for metadata.
pub const CONTRACT_DESCRIPTION: &str = "Soulbound token registry for Stellar Wrap";

/// Contract version. Bump this on every WASM upgrade.
pub const VERSION: u32 = 1;

/// Hash preview bytes length constant.
pub const HASH_PREVIEW_BYTES: usize = 8;