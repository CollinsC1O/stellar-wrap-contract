# Security Recommendations for Stellar Wrap Contract

## Overview
This document outlines critical security enhancements needed before mainnet deployment. The current implementation has a **stub signature verification** that must be replaced with proper cryptographic verification.

---

## 🔴 CRITICAL: Signature Verification Enhancement

### Current State
The `verify_signature()` function in `lib.rs` currently returns `true` unconditionally:

```rust
fn verify_signature(_data_hash: &BytesN<32>) -> bool {
    true  // ⚠️ INSECURE - Always passes
}
```

### Required Implementation

The signature must cryptographically bind:
1. **User Address** (prevents identity theft)
2. **Contract Address** (prevents cross-contract replay)
3. **Period** (prevents time-based replay)
4. **Data Hash** (prevents data tampering)
5. **Nonce or Sequence** (optional, for additional replay protection)

### Recommended Approach

#### Option 1: Ed25519 Signature Verification (Recommended)

```rust
use soroban_sdk::crypto::ed25519;

fn verify_signature(
    e: &Env,
    admin: &Address,
    user: &Address,
    period: &Symbol,
    data_hash: &BytesN<32>,
    signature: &BytesN<64>
) -> bool {
    // Construct the payload that was signed
    let mut payload = Bytes::new(e);
    
    // Include contract address (prevents cross-contract replay)
    payload.append(&e.current_contract_address().to_bytes());
    
    // Include user address (prevents identity theft)
    payload.append(&user.to_bytes());
    
    // Include period (prevents period replay)
    payload.append(&period.to_bytes());
    
    // Include data hash
    payload.append(&data_hash.to_bytes());
    
    // Hash the payload
    let message = e.crypto().sha256(&payload);
    
    // Get admin's public key and verify signature
    // Note: You'll need to store/retrieve the admin's public key
    let admin_pubkey = get_admin_pubkey(e);
    
    e.crypto().ed25519_verify(
        &admin_pubkey,
        &message,
        signature
    );
    
    true
}
```

#### Option 2: Use Soroban's Built-in Auth (Simpler)

Instead of manual signature verification, leverage Soroban's authorization framework:

```rust
pub fn mint_wrap(
    e: Env,
    to: Address,
    data_hash: BytesN<32>,
    archetype: Symbol,
    period: Symbol,
) {
    let admin: Address = e
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(e, ContractError::NotInitialized));

    // This already provides cryptographic verification
    admin.require_auth();
    
    // Rest of implementation...
}
```

**Current code already uses `admin.require_auth()` which is secure!** However, for additional security layers (like binding to specific parameters), you may want to add custom signature verification.

---

## ✅ Security Features Already Implemented

### 1. Replay Attack Protection ✓
**Status:** IMPLEMENTED

The contract prevents replay attacks within the same contract instance through the duplicate check:

```rust
let wrap_key = DataKey::Wrap(to.clone(), period.clone());
if e.storage().instance().has(&wrap_key) {
    panic_with_error!(e, ContractError::WrapAlreadyExists);
}
```

**What it prevents:**
- Same user + same period cannot be minted twice
- Attacker cannot replay a valid transaction

**Test coverage:** 
- `test_replay_attack_same_period_fails` ✓
- `test_duplicate_period_fails` ✓

### 2. Authorization Protection ✓
**Status:** IMPLEMENTED

Only the admin can authorize minting:

```rust
admin.require_auth();
```

**What it prevents:**
- Unauthorized users cannot mint wraps
- Only admin-signed transactions succeed

**Test coverage:**
- `test_mint_wrap_unauthorized` ✓
- `test_non_admin_cannot_mint` ✓

### 3. Initialization Protection ✓
**Status:** IMPLEMENTED

Contract can only be initialized once:

```rust
if e.storage().instance().has(&key) {
    panic_with_error!(e, ContractError::AlreadyInitialized);
}
```

**Test coverage:**
- `test_initialize_twice_fails` ✓

---

## ⚠️ Security Considerations for Deployment

### 1. Cross-Contract Replay Protection
**Status:** NEEDS ENHANCEMENT

**Current behavior:**
- Each contract instance has independent storage
- A signature valid for Contract V1 could theoretically work on Contract V2

**Recommendation:**
If you plan to deploy multiple versions, include `env.current_contract_address()` in the signature payload.

```rust
// In signature verification
let contract_id = e.current_contract_address();
payload.append(&contract_id.to_bytes());
```

**Test coverage:**
- `test_cross_contract_replay_protection` ✓ (documents current behavior)

### 2. Timestamp Integrity
**Status:** SECURE ✓

The contract correctly uses `env.ledger().timestamp()` rather than accepting user-provided timestamps.

**Test coverage:**
- `test_timestamp_is_from_ledger_not_user` ✓

### 3. Identity Binding
**Status:** SECURE (via admin auth) ✓

The current `admin.require_auth()` ensures only authorized transactions succeed. The `to` parameter is part of the storage key, preventing one user from claiming another's wrap.

**Test coverage:**
- `test_signature_cannot_be_stolen_by_another_user` ✓

---

## 📊 Gas/Resource Analysis Results

Run the tests to get exact numbers:

```bash
cargo test test_gas_analysis -- --nocapture
```

Expected output:
```
=== GAS ANALYSIS REPORT ===
Operation: mint_wrap
CPU Instructions: ~[TO BE MEASURED]
Memory Bytes: ~[TO BE MEASURED]
===========================
```

### Optimization Recommendations:
1. **Storage**: Instance storage is used correctly (ephemeral, cheaper than persistent)
2. **Event emission**: Minimal data in events (only period as u64)
3. **Signature verification**: If implementing custom crypto, measure impact

---

## 🧪 Test Suite Summary

### Security Tests (`src/security_test.rs`)

| Test | Purpose | Expected Behavior |
|------|---------|-------------------|
| `test_replay_attack_same_period_fails` | Replay protection | PANIC #4 |
| `test_replay_attack_different_hash_same_period_fails` | Duplicate period prevention | PANIC #4 |
| `test_multiple_periods_for_same_user_success` | Valid multi-period usage | SUCCESS |
| `test_signature_cannot_be_stolen_by_another_user` | Identity theft prevention | SUCCESS (isolation) |
| `test_cross_contract_replay_protection` | Cross-contract isolation | SUCCESS (independent storage) |
| `test_gas_analysis_mint_operation` | Resource consumption | Prints metrics |
| `test_gas_analysis_multiple_mints` | Scaling analysis | Prints metrics |
| `test_timestamp_is_from_ledger_not_user` | Timestamp integrity | SUCCESS |
| `test_edge_case_long_symbols` | Symbol length limits | SUCCESS |
| `test_non_admin_cannot_mint` | Authorization check | PANIC |

### Running Tests

```bash
# Run all tests
cargo test

# Run only security tests
cargo test security_test

# Run with output for gas analysis
cargo test test_gas_analysis -- --nocapture

# Run with detailed output
cargo test -- --nocapture --test-threads=1
```

---

## 🚀 Pre-Mainnet Checklist

- [x] Replay attack protection implemented
- [x] Authorization verification implemented
- [x] Duplicate period prevention implemented
- [x] Timestamp integrity verified
- [x] Comprehensive test suite created
- [ ] **CRITICAL**: Decide on signature verification approach (current admin.require_auth() may be sufficient)
- [ ] **CRITICAL**: If deploying multiple versions, add contract address binding
- [ ] Run gas analysis and document costs
- [ ] Security audit by third party
- [ ] Fuzz testing with property-based tests
- [ ] Load testing for high-volume scenarios

---

## 🔒 Issue #31: One-Signature-Per-Period Invariant

### Overview

The duplicate check in `mint_wrap` uses `DataKey::Wrap(user, period)` as the deduplication key.
This means:

- Once a user successfully mints for a given period, **no further mint for that period is ever
  accepted**, regardless of the archetype or data hash carried in the new call.
- A signature for `(user, period, archetype_A)` and a second signature for
  `(user, period, archetype_B)` cannot both be consumed — the first one to land wins, and the
  second becomes permanently orphaned.

### The Invariant

> **The admin backend MUST issue exactly one signature per `(user, period)` pair.**

This is an off-chain operational invariant, not enforced by the contract. The contract enforces
that at most one record can be stored per `(user, period)`, but it cannot prevent the backend from
accidentally generating multiple valid signatures before the first is consumed.

### Why Orphaned Signatures Are a Liability

If two valid signatures for the same `(user, period)` exist but only one can ever be used:

1. **Upgrade risk:** If contract logic changes (e.g., the deduplication key is widened or the
   archetype is added to the storage key), the previously orphaned signature becomes usable and
   could write an unexpected record.
2. **Information leakage:** A second signature reveals that the admin considered a different
   archetype for the user, which may be undesirable.
3. **Key rotation edge cases:** If the admin key is rotated and old signatures are not explicitly
   revoked, orphaned signatures from the old key remain valid until the key is removed.

### Mitigation Options

#### Adopted: Document and Enforce Off-Chain

The simplest and currently adopted approach is to **document this invariant here** and ensure the
backend service guarantees it:

- Compute and sign exactly one `(user, period, archetype, data_hash)` tuple per period.
- Never re-sign with a different archetype if a signature has already been issued for that period.
- Maintain an idempotency log: if a period is requested again, return the existing signature, never
  generate a new one.

#### Optional: Nonce-Based Replay Protection

For stronger guarantees a nonce can be added to the signed payload and stored on-chain:

```rust
// In DataKey:
NonceUsed(Address, u64),   // user + period → bool

// In mint_wrap, after signature verification:
let nonce_key = DataKey::NonceUsed(user.clone(), period);
if e.storage().persistent().has(&nonce_key) {
    panic_with_error!(e, ContractError::WrapAlreadyExists);
}
e.storage().persistent().set(&nonce_key, &true);
```

This is redundant with the existing `Wrap(user, period)` key check for the current storage schema,
but becomes valuable if the schema ever changes to allow multiple records per period.

#### Optional: Include Archetype in the Storage Key

If the design goal ever changes to allow one wrap per `(user, period, archetype)` triple, update
`DataKey::Wrap` to `DataKey::Wrap(Address, u64, Symbol)`. This would make every archetype-specific
signature independently consumable, but would also allow a user to mint multiple records for the
same period, which contradicts the SBT one-record-per-period model.

### Current Status

- ✅ At most one record per `(user, period)` — enforced on-chain.
- ✅ Cross-contract replay protection — contract address is included in the signed payload.
- ✅ Cross-period replay protection — period is included in the signed payload.
- ⚙️  One-signature-per-period — enforced **off-chain** by the backend (documented invariant).

---

## 📚 Additional Security Best Practices

### 1. Invariant Testing
Consider adding property-based tests:
- No user should ever have duplicate periods
- Total wraps minted should equal sum of all user wraps
- Timestamps should be monotonic within a session

### 2. Fuzz Testing
Use `cargo-fuzz` to test with random inputs:
```bash
cargo install cargo-fuzz
cargo fuzz init
cargo fuzz run fuzz_target_1
```

### 3. Access Control Review
- Ensure `initialize()` is called during deployment
- Verify admin key is secured in production
- Consider multi-sig admin for production

### 4. Upgrade Strategy
- Plan for contract upgrades if needed
- Consider using a proxy pattern
- Document migration procedures

---

## 🔗 References

- [Soroban Security Best Practices](https://soroban.stellar.org/docs/learn/security)
- [Stellar Smart Contract Audit Guidelines](https://stellar.org/developers)
- [Soroban Auth Framework](https://soroban.stellar.org/docs/learn/authorization)

---

## 📧 Questions?

If implementing custom signature verification, consider:
1. Key management strategy
2. Signature format and standards
3. Off-chain signature generation process
4. Recovery mechanisms

**Current Assessment:** The contract uses Soroban's built-in `require_auth()` which is cryptographically secure and prevents most attack vectors. Additional custom signature verification is optional and depends on your specific security requirements.
