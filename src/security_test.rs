#![cfg(test)]

use super::*;
use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    xdr::ToXdr,
    Address, Bytes, BytesN, Env,
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
#[should_panic(expected = "Error(Contract, #4)")]
fn test_replay_attack_same_period_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let data_hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("architect");
    let period = 202512u64;

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

    let wrap = client.get_wrap(&user, &period);
    assert!(wrap.is_some(), "First mint should succeed");

    client.mint_wrap(&user, &period, &archetype, &data_hash, &signature);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_replay_attack_different_hash_same_period_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let data_hash_1 = BytesN::from_array(&env, &[42u8; 32]);
    let data_hash_2 = BytesN::from_array(&env, &[99u8; 32]);
    let archetype = symbol_short!("architect");
    let period = 202512u64;

    let signature_1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash_1,
    );

    client.mint_wrap(&user, &period, &archetype, &data_hash_1, &signature_1);

    let signature_2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash_2,
    );

    client.mint_wrap(&user, &period, &archetype, &data_hash_2, &signature_2);
}

#[test]
fn test_multiple_periods_for_same_user_success() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let data_hash_1 = BytesN::from_array(&env, &[42u8; 32]);
    let data_hash_2 = BytesN::from_array(&env, &[99u8; 32]);
    let data_hash_3 = BytesN::from_array(&env, &[77u8; 32]);
    let archetype = symbol_short!("architect");

    let period_1 = 202512u64;
    let period_2 = 202601u64;
    let period_3 = 202602u64;

    let signature_1 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period_1,
        &archetype,
        &data_hash_1,
    );
    let signature_2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period_2,
        &archetype,
        &data_hash_2,
    );
    let signature_3 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period_3,
        &archetype,
        &data_hash_3,
    );

    client.mint_wrap(&user, &period_1, &archetype, &data_hash_1, &signature_1);
    client.mint_wrap(&user, &period_2, &archetype, &data_hash_2, &signature_2);
    client.mint_wrap(&user, &period_3, &archetype, &data_hash_3, &signature_3);

    assert!(client.get_wrap(&user, &period_1).is_some());
    assert!(client.get_wrap(&user, &period_2).is_some());
    assert!(client.get_wrap(&user, &period_3).is_some());
}

#[test]
fn test_signature_cannot_be_stolen_by_another_user() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let data_hash_for_a = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("architect");
    let period = 202512u64;

    let signature_a = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_a,
        period,
        &archetype,
        &data_hash_for_a,
    );

    client.mint_wrap(&user_a, &period, &archetype, &data_hash_for_a, &signature_a);

    let wrap_a = client.get_wrap(&user_a, &period);
    assert!(wrap_a.is_some(), "User A should have the wrap");

    let data_hash_for_b = BytesN::from_array(&env, &[99u8; 32]);
    let period_b = 202601u64;

    let signature_b = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user_b,
        period_b,
        &archetype,
        &data_hash_for_b,
    );

    client.mint_wrap(
        &user_b,
        &period_b,
        &archetype,
        &data_hash_for_b,
        &signature_b,
    );

    let wrap_a = client.get_wrap(&user_a, &period).unwrap();
    let wrap_b = client.get_wrap(&user_b, &period_b).unwrap();

    assert_eq!(wrap_a.data_hash, data_hash_for_a);
    assert_eq!(wrap_b.data_hash, data_hash_for_b);

    let user_b_period_dec = client.get_wrap(&user_b, &period);
    assert!(
        user_b_period_dec.is_none(),
        "User B should not have User A's period"
    );
}

#[test]
fn test_cross_contract_replay_protection() {
    let env = Env::default();

    let contract_v1 = env.register_contract(None, StellarWrapContract);
    let contract_v2 = env.register_contract(None, StellarWrapContract);

    let client_v1 = StellarWrapContractClient::new(&env, &contract_v1);
    let client_v2 = StellarWrapContractClient::new(&env, &contract_v2);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client_v1.initialize(&admin, &admin_pubkey);
    client_v2.initialize(&admin, &admin_pubkey);

    env.mock_all_auths();

    let data_hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("architect");
    let period = 202512u64;

    let signature_v1 = sign_payload(
        &env,
        &signing_key,
        &contract_v1,
        &user,
        period,
        &archetype,
        &data_hash,
    );

    client_v1.mint_wrap(&user, &period, &archetype, &data_hash, &signature_v1);

    let wrap_v1 = client_v1.get_wrap(&user, &period);
    assert!(wrap_v1.is_some(), "Wrap should exist on contract V1");

    let signature_v2 = sign_payload(
        &env,
        &signing_key,
        &contract_v2,
        &user,
        period,
        &archetype,
        &data_hash,
    );

    client_v2.mint_wrap(&user, &period, &archetype, &data_hash, &signature_v2);

    let wrap_v2 = client_v2.get_wrap(&user, &period);
    assert!(wrap_v2.is_some(), "Wrap should exist on contract V2");

    assert!(client_v1.get_wrap(&user, &period).is_some());
    assert!(client_v2.get_wrap(&user, &period).is_some());
}

#[test]
fn test_gas_analysis_mint_operation() {
    let env = Env::default();
    env.budget().reset_unlimited();

    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let data_hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("architect");
    let period = 202512u64;

    let signature = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period,
        &archetype,
        &data_hash,
    );

    env.budget().reset_default();

    client.mint_wrap(&user, &period, &archetype, &data_hash, &signature);

    env.budget().print();

    let cpu_insns = env.budget().cpu_instruction_cost();
    let mem_bytes = env.budget().memory_bytes_cost();

    assert!(
        cpu_insns < 10_000_000,
        "CPU instructions too high: {}",
        cpu_insns
    );
    assert!(mem_bytes < 100_000, "Memory usage too high: {}", mem_bytes);
}

#[test]
fn test_gas_analysis_multiple_mints() {
    let env = Env::default();
    env.budget().reset_unlimited();

    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    env.budget().reset_default();

    for i in 1..6 {
        let data_hash = BytesN::from_array(&env, &[i as u8; 32]);
        let archetype = symbol_short!("architect");

        let period = match i {
            1 => 202512u64,
            2 => 202601u64,
            3 => 202602u64,
            4 => 202603u64,
            _ => 202604u64,
        };

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
    }

    let cpu_insns = env.budget().cpu_instruction_cost();
    let mem_bytes = env.budget().memory_bytes_cost();

    assert!(cpu_insns < 50_000_000, "Batch CPU too high: {}", cpu_insns);
    assert!(mem_bytes < 500_000, "Batch memory too high: {}", mem_bytes);
}

#[test]
fn test_timestamp_is_from_ledger_not_user() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    env.ledger().with_mut(|li| {
        li.timestamp = 1000000;
    });

    let data_hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("architect");
    let period = 202512u64;

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

    let wrap = client.get_wrap(&user, &period).unwrap();

    assert_eq!(wrap.timestamp, 1000000, "Timestamp should come from ledger");

    env.ledger().with_mut(|li| {
        li.timestamp = 2000000;
    });

    let period_2 = 202601u64;
    let signature_2 = sign_payload(
        &env,
        &signing_key,
        &contract_id,
        &user,
        period_2,
        &archetype,
        &data_hash,
    );

    client.mint_wrap(&user, &period_2, &archetype, &data_hash, &signature_2);

    let wrap_2 = client.get_wrap(&user, &period_2).unwrap();
    assert_eq!(
        wrap_2.timestamp, 2000000,
        "Second timestamp should match new ledger time"
    );
}

#[test]
fn test_edge_case_long_symbols() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);
    env.mock_all_auths();

    let data_hash = BytesN::from_array(&env, &[42u8; 32]);

    let archetype = symbol_short!("architect");
    let period = 202512u64;

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

    let wrap = client.get_wrap(&user, &period);
    assert!(wrap.is_some(), "Should handle reasonably long symbols");
}

#[test]
#[should_panic]
fn test_non_admin_cannot_mint() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let _attacker = Address::generate(&env);

    client.initialize(&admin, &admin_pubkey);

    let data_hash = BytesN::from_array(&env, &[42u8; 32]);
    let archetype = symbol_short!("architect");
    let period = 202512u64;

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
}
