//! Device credential hashing per SECURITY_SPEC — HMAC-SHA256 with server pepper in production.

use sdkwork_utils_rust::{hmac_sha256, secure_compare, sha256_hash};

pub const ENV_CREDENTIAL_PEPPER: &str = "SDKWORK_AIOT_CREDENTIAL_PEPPER";
pub const HMAC_SHA256_V1_PREFIX: &str = "hmac-sha256-v1:";

pub fn credential_pepper_from_env() -> Option<String> {
    std::env::var(ENV_CREDENTIAL_PEPPER)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn hash_device_credential_secret(secret: &[u8]) -> String {
    if let Some(pepper) = credential_pepper_from_env() {
        format!(
            "{HMAC_SHA256_V1_PREFIX}{}",
            hmac_sha256(secret, pepper.as_bytes())
        )
    } else {
        sha256_hash(secret)
    }
}

pub fn verify_device_credential_secret(stored_hash: &str, secret: &[u8]) -> bool {
    if stored_hash.starts_with(HMAC_SHA256_V1_PREFIX) {
        let Some(pepper) = credential_pepper_from_env() else {
            return false;
        };
        let expected = format!(
            "{HMAC_SHA256_V1_PREFIX}{}",
            hmac_sha256(secret, pepper.as_bytes())
        );
        return secure_compare(stored_hash, &expected);
    }

    let legacy = sha256_hash(secret);
    secure_compare(stored_hash, &legacy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_hash_supports_pepper_and_legacy_sha256() {
        let _lock = crate::test_env::lock_env_tests();
        let _guard = crate::test_env::EnvGuard::clear(&[ENV_CREDENTIAL_PEPPER]);
        std::env::set_var(ENV_CREDENTIAL_PEPPER, "test-pepper-value");
        let secret = b"device-secret-001";
        let stored = hash_device_credential_secret(secret);
        assert!(stored.starts_with(HMAC_SHA256_V1_PREFIX));
        assert!(verify_device_credential_secret(&stored, secret));
        assert!(!verify_device_credential_secret(&stored, b"wrong"));

        let legacy_secret = b"legacy-secret";
        let legacy_stored = sha256_hash(legacy_secret);
        assert!(verify_device_credential_secret(
            &legacy_stored,
            legacy_secret
        ));
    }
}
