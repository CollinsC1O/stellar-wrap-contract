#![cfg(test)]
use super::*;
use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    xdr::ToXdr,
    Address, Bytes, BytesN, Env, String, Symbol, TryIntoVal,
};

fn sign_payload(
    env: &Env,
    signer: &SigningKey,
    contract: &Address,
    user: &Address,
    period: u64,
    archetype: &Symbol,
    data_hash: &BytesN<32>,
) -> BytesN<64> {
    let mut payload = Bytes::new(env);
    payload.append(&contract.to_xdr(env));
    payload.append(&user.clone().to_xdr(env));
    payload.append(&period.to_xdr(env));
    payload.append(&archetype.clone().to_xdr(env));
    payload.append(&data_hash.clone().to_xdr(env));

    let mut out = [0u8; 512];
    let len = payload.len() as usize;
    payload.copy_into_slice(&mut out[..len]);

    let signature = signer.sign(&out[..len]);
    BytesN::from_array(env, &signature.to_bytes())
}

#[test]
fn test_minting_flow() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let dummy_hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 2024u64;

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &dummy_hash,
    );
    client.mint_wrap(&user, &period, &archetype, &dummy_hash, &signature);

    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, dummy_hash);
}

#[test]
fn test_mint_emits_event() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[2u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let period = 2024u64;
    let archetype = symbol_short!("arch");
    let hash = BytesN::from_array(&env, &[1u8; 32]);
    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
    );

    client.mint_wrap(&user, &period, &archetype, &hash, &signature);

    let events = env.events().all();
    let last_event = events.last().expect("No events found");
    let (_, topics, data) = last_event;

    // Convert Vals to concrete types for comparison
    let event_topic: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let event_user: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let event_period: u64 = topics.get(2).unwrap().try_into_val(&env).unwrap();
    let event_archetype: Symbol = data.try_into_val(&env).unwrap();

    assert_eq!(event_topic, symbol_short!("mint"));
    assert_eq!(event_user, user);
    assert_eq!(event_period, period);
    assert_eq!(event_archetype, archetype);
}

#[test]
fn test_balance_of_and_count() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[3u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("soroban");
    let hash = BytesN::from_array(&env, &[42u8; 32]);

    let sig1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        2021,
        &archetype,
        &hash,
    );
    client.mint_wrap(&user, &2021, &archetype, &hash, &sig1);

    let sig2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        2022,
        &archetype,
        &hash,
    );
    client.mint_wrap(&user, &2022, &archetype, &hash, &sig2);

    assert_eq!(client.balance_of(&user), 2);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_initialize_twice_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);

    client.initialize(&admin, &pubkey);
    client.initialize(&admin, &pubkey);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_duplicate_period_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[4u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 2024u64;

    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
    );

    client.mint_wrap(&user, &period, &archetype, &hash, &sig);
    client.mint_wrap(&user, &period, &archetype, &hash, &sig);
}

#[test]
fn test_update_admin_success() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);

    client.initialize(&admin, &pubkey);
    env.mock_all_auths();

    client.update_admin(&new_admin);
    assert_eq!(client.get_admin().unwrap(), new_admin);
}

#[test]
fn test_token_metadata() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    assert_eq!(client.decimals(), 0);
    assert_eq!(
        client.name(),
        String::from_str(&env, "Stellar Wrap Registry")
    );
    assert_eq!(client.symbol(), String::from_str(&env, "WRAP"));
}

// ─── Issue #84: extend_ttl tests ────────────────────────────────────────────

#[test]
fn test_extend_ttl_existing_wrap() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[9u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 202512u64;

    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
    );
    client.mint_wrap(&user, &period, &archetype, &hash, &sig);

    // Anyone can call extend_ttl — no auth required
    client.extend_ttl(&user, &period);

    // Record should still be readable after extending TTL
    let wrap = client.get_wrap(&user, &period).unwrap();
    assert_eq!(wrap.data_hash, hash);
}

#[test]
fn test_extend_ttl_nonexistent_wrap_does_not_panic() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);

    let user = Address::generate(&env);
    // Should not panic even if no wrap exists for this user/period
    client.extend_ttl(&user, &9999);
}

// ─── Issue #81: concurrent mints for different users same period ────────────

#[test]
fn test_concurrent_mints_different_users_same_period() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[10u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let period = 202512u64;
    let archetype = symbol_short!("arch");
    let hash_a = BytesN::from_array(&env, &[10u8; 32]);
    let hash_b = BytesN::from_array(&env, &[20u8; 32]);

    let sig_a = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_a,
        period,
        &archetype,
        &hash_a,
    );
    let sig_b = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_b,
        period,
        &archetype,
        &hash_b,
    );

    // Both mints for the same period should succeed
    client.mint_wrap(&user_a, &period, &archetype, &hash_a, &sig_a);
    client.mint_wrap(&user_b, &period, &archetype, &hash_b, &sig_b);

    // Records are independent
    let wrap_a = client.get_wrap(&user_a, &period).unwrap();
    let wrap_b = client.get_wrap(&user_b, &period).unwrap();
    assert_eq!(wrap_a.data_hash, hash_a);
    assert_eq!(wrap_b.data_hash, hash_b);
    assert_ne!(wrap_a.data_hash, wrap_b.data_hash);

    // Individual balances are correct
    assert_eq!(client.balance_of(&user_a), 1);
    assert_eq!(client.balance_of(&user_b), 1);

    // Each user's record doesn't affect the other
    assert!(client.get_wrap(&user_a, &period).is_some());
    assert!(client.get_wrap(&user_b, &period).is_some());
}

// ─── Issue #75: structured event verification ──────────────────────────────

#[test]
fn test_mint_event_structured_matching() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[11u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let period = 202512u64;
    let archetype = symbol_short!("arch");
    let hash = BytesN::from_array(&env, &[42u8; 32]);

    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
    );
    client.mint_wrap(&user, &period, &archetype, &hash, &sig);

    // Event schema: topics = (Symbol("mint"), Address, u64), data = Symbol
    let events = env.events().all();
    let last_event = events.last().expect("Expected at least one event");
    let (event_contract, topics, data) = last_event;

    // Verify event is emitted by the correct contract
    assert_eq!(event_contract, contract_id);

    // Verify topic count — mint events must have exactly 3 topics
    assert_eq!(topics.len(), 3, "Mint event must have exactly 3 topics");

    // Verify each topic by type and value
    let topic_0: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let topic_1: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let topic_2: u64 = topics.get(2).unwrap().try_into_val(&env).unwrap();

    assert_eq!(
        topic_0,
        symbol_short!("mint"),
        "Topic 0 must be 'mint' Symbol"
    );
    assert_eq!(topic_1, user, "Topic 1 must be the user Address");
    assert_eq!(topic_2, period, "Topic 2 must be the period u64");

    // Verify data is the archetype Symbol
    let event_data: Symbol = data.try_into_val(&env).unwrap();
    assert_eq!(
        event_data, archetype,
        "Event data must be the archetype Symbol"
    );
}

#[test]
fn test_mint_events_multiple_users_correct_schema() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[12u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype_a = symbol_short!("builder");
    let archetype_b = symbol_short!("defi");
    let hash_a = BytesN::from_array(&env, &[10u8; 32]);
    let hash_b = BytesN::from_array(&env, &[20u8; 32]);
    let period_a = 202501u64;
    let period_b = 202502u64;

    let sig_a = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_a,
        period_a,
        &archetype_a,
        &hash_a,
    );
    let sig_b = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_b,
        period_b,
        &archetype_b,
        &hash_b,
    );

    client.mint_wrap(&user_a, &period_a, &archetype_a, &hash_a, &sig_a);
    client.mint_wrap(&user_b, &period_b, &archetype_b, &hash_b, &sig_b);

    let events = env.events().all();

    // Collect mint events emitted by our contract
    let mut mint_events = soroban_sdk::vec![&env];
    for event in events.iter() {
        let (addr, topics, _data) = &event;
        if *addr == contract_id && topics.len() == 3 {
            let t: Result<Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
            if t.map_or(false, |s| s == symbol_short!("mint")) {
                mint_events.push_back(event.clone());
            }
        }
    }

    assert_eq!(mint_events.len(), 2, "Expected exactly 2 mint events");

    // Verify first mint event (user_a)
    let (_, topics_a, data_a) = mint_events.get(0).unwrap();
    let ev_user_a: Address = topics_a.get(1).unwrap().try_into_val(&env).unwrap();
    let ev_period_a: u64 = topics_a.get(2).unwrap().try_into_val(&env).unwrap();
    let ev_arch_a: Symbol = data_a.try_into_val(&env).unwrap();
    assert_eq!(ev_user_a, user_a);
    assert_eq!(ev_period_a, period_a);
    assert_eq!(ev_arch_a, archetype_a);

    // Verify second mint event (user_b)
    let (_, topics_b, data_b) = mint_events.get(1).unwrap();
    let ev_user_b: Address = topics_b.get(1).unwrap().try_into_val(&env).unwrap();
    let ev_period_b: u64 = topics_b.get(2).unwrap().try_into_val(&env).unwrap();
    let ev_arch_b: Symbol = data_b.try_into_val(&env).unwrap();
    assert_eq!(ev_user_b, user_b);
    assert_eq!(ev_period_b, period_b);
    assert_eq!(ev_arch_b, archetype_b);
}

// ─── Issue #80: verify_data tests ───────────────────────────────────────────

#[test]
fn test_verify_data_matching_hash() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[5u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let data_json = Bytes::from_slice(&env, b"{\"score\":100,\"level\":\"gold\"}");
    let data_hash_raw = env.crypto().sha256(&data_json);
    let data_hash = BytesN::from_array(&env, &data_hash_raw.to_array());
    let archetype = symbol_short!("arch");
    let period = 2024u64;

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
    );
    client.mint_wrap(&user, &period, &archetype, &data_hash, &signature);

    assert!(client.verify_data(&user, &period, &data_json));
}

#[test]
fn test_verify_data_non_matching_hash() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[6u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let original_data = Bytes::from_slice(&env, b"{\"score\":100}");
    let data_hash_raw = env.crypto().sha256(&original_data);
    let data_hash = BytesN::from_array(&env, &data_hash_raw.to_array());
    let archetype = symbol_short!("arch");
    let period = 2024u64;

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
    );
    client.mint_wrap(&user, &period, &archetype, &data_hash, &signature);

    let tampered_data = Bytes::from_slice(&env, b"{\"score\":999}");
    assert!(!client.verify_data(&user, &period, &tampered_data));
}

#[test]
fn test_verify_data_no_wrap_exists() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);

    let user = Address::generate(&env);
    let data = Bytes::from_slice(&env, b"anything");
    assert!(!client.verify_data(&user, &9999, &data));
}

// ─── Issue #87: get_latest_wrap tests ───────────────────────────────────────

#[test]
fn test_get_latest_wrap_returns_most_recent() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");
    let hash1 = BytesN::from_array(&env, &[10u8; 32]);
    let hash2 = BytesN::from_array(&env, &[20u8; 32]);
    let hash3 = BytesN::from_array(&env, &[30u8; 32]);

    let sig1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        2022,
        &archetype,
        &hash1,
    );
    let sig2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        2024,
        &archetype,
        &hash2,
    );
    let sig3 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        2023,
        &archetype,
        &hash3,
    );

    client.mint_wrap(&user, &2022, &archetype, &hash1, &sig1);
    client.mint_wrap(&user, &2024, &archetype, &hash2, &sig2);
    client.mint_wrap(&user, &2023, &archetype, &hash3, &sig3);

    let latest = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest.period, 2024);
    assert_eq!(latest.data_hash, hash2);
}

#[test]
fn test_get_latest_wrap_no_wraps() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    client.initialize(&admin, &pubkey);

    let user = Address::generate(&env);
    assert!(client.get_latest_wrap(&user).is_none());
}

#[test]
fn test_get_latest_wrap_single_mint() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[8u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let hash = BytesN::from_array(&env, &[55u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 2025u64;

    let sig = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &hash,
    );
    client.mint_wrap(&user, &period, &archetype, &hash, &sig);

    let latest = client.get_latest_wrap(&user).unwrap();
    assert_eq!(latest.period, 2025);
    assert_eq!(latest.data_hash, hash);
}

// ─── Issue #85: negative tests before initialize ────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_mint_wrap_before_init_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    env.mock_all_auths();

    let user = Address::generate(&env);
    let hash = BytesN::from_array(&env, &[1u8; 32]);
    let archetype = symbol_short!("arch");
    let sig = BytesN::from_array(&env, &[0u8; 64]);

    client.mint_wrap(&user, &2024, &archetype, &hash, &sig);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_update_admin_before_init_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);
    env.mock_all_auths();

    let new_admin = Address::generate(&env);
    client.update_admin(&new_admin);
}

#[test]
fn test_get_admin_before_init_returns_none() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    assert!(client.get_admin().is_none());
}

// ─── Issue #55: data_hash edge-case tests ───────────────────────────────────

/// Zero-filled data_hash must be rejected with InvalidDataHash (#6).
/// An all-zero hash almost certainly means the caller forgot to hash real data
/// and is passing a default/uninitialized value — the contract rejects it early.
#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_zero_data_hash_rejected() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[20u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    // All-zero hash — explicitly invalid per contract rules
    let zero_hash = BytesN::from_array(&env, &[0u8; 32]);
    let archetype = symbol_short!("arch");
    let period = 2025u64;

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &zero_hash,
    );

    // Must panic with ContractError::InvalidDataHash (#6)
    client.mint_wrap(&user, &period, &archetype, &zero_hash, &signature);
}

/// Non-zero edge-case hashes that look "almost zero" must still be accepted.
/// This guards against an overly broad rejection that would block legitimate hashes
/// such as all-ones ([0xFF; 32]) or a hash where only the first byte is non-zero.
#[test]
fn test_near_zero_data_hashes_are_accepted() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[21u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let archetype = symbol_short!("arch");

    // Case 1: all-ones (0xFF) — max-value hash, should be accepted
    let all_ones_hash = BytesN::from_array(&env, &[0xFFu8; 32]);
    let sig1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        2025u64,
        &archetype,
        &all_ones_hash,
    );
    client.mint_wrap(&user, &2025u64, &archetype, &all_ones_hash, &sig1);
    let wrap1 = client.get_wrap(&user, &2025u64).unwrap();
    assert_eq!(wrap1.data_hash, all_ones_hash);

    // Case 2: only the first byte is non-zero, rest are zero — still non-zero overall
    let mut almost_zero_bytes = [0u8; 32];
    almost_zero_bytes[0] = 0x01;
    let almost_zero_hash = BytesN::from_array(&env, &almost_zero_bytes);
    let sig2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        2026u64,
        &archetype,
        &almost_zero_hash,
    );
    client.mint_wrap(&user, &2026u64, &archetype, &almost_zero_hash, &sig2);
    let wrap2 = client.get_wrap(&user, &2026u64).unwrap();
    assert_eq!(wrap2.data_hash, almost_zero_hash);

    // Case 3: only the last byte is non-zero
    let mut last_byte_hash_bytes = [0u8; 32];
    last_byte_hash_bytes[31] = 0x01;
    let last_byte_hash = BytesN::from_array(&env, &last_byte_hash_bytes);
    let sig3 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        2027u64,
        &archetype,
        &last_byte_hash,
    );
    client.mint_wrap(&user, &2027u64, &archetype, &last_byte_hash, &sig3);
    let wrap3 = client.get_wrap(&user, &2027u64).unwrap();
    assert_eq!(wrap3.data_hash, last_byte_hash);
}
