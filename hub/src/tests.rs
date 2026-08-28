use super::*;

#[test]
fn provider_secrets_are_encrypted_and_never_present_in_public_record() {
    let key = [7u8; 32];
    let stored = encrypt_provider(&key, "anthropic".into(), "work".into(), "secret-value").unwrap();
    assert!(!stored.ciphertext.contains("secret-value"));
    assert_eq!(decrypt_provider(&key, &stored).unwrap(), "secret-value");
    let public = serde_json::to_string(&stored.account).unwrap();
    assert!(!public.contains("secret-value"));
}

#[test]
fn setup_keys_are_hashed_and_one_spoke_scoped() {
    let mut store = Store::default();
    let (spoke_id, setup_key) = create_spoke(&mut store, "framework".into());
    let record = &store.spokes[&spoke_id];
    assert_ne!(record.setup_key_hash.as_deref(), Some(setup_key.as_str()));
    assert_eq!(
        record.setup_key_hash.as_deref(),
        Some(hash(&setup_key).as_str())
    );
    assert!(record.setup_expires_at.unwrap() > now());
}
