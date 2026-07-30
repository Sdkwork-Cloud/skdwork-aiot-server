//! Serializes process-environment mutations across crate unit tests.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Holds the global env-test lock for the duration of a test that mutates process env.
pub fn lock_env_tests() -> MutexGuard<'static, ()> {
    ENV_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub const DEVICE_DATABASE_ENV_KEYS: &[&str] = &[
    "SDKWORK_DATABASE_URL",
    "SDKWORK_DATABASE_ENGINE",
    "SDKWORK_DATABASE_HOST",
    "SDKWORK_DATABASE_PORT",
    "SDKWORK_DATABASE_NAME",
    "SDKWORK_DATABASE_SCHEMA",
    "SDKWORK_DATABASE_USERNAME",
    "SDKWORK_DATABASE_PASSWORD",
    "SDKWORK_DATABASE_PASSWORD_FILE",
    "SDKWORK_DATABASE_SSL_MODE",
    "SDKWORK_DATABASE_FILE",
];

pub struct EnvGuard {
    saved: HashMap<String, Option<String>>,
}

impl EnvGuard {
    pub fn clear(keys: &[&str]) -> Self {
        let mut saved = HashMap::new();
        for key in keys {
            saved.insert((*key).to_owned(), std::env::var(key).ok());
            std::env::remove_var(key);
        }
        Self { saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.saved.drain() {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
