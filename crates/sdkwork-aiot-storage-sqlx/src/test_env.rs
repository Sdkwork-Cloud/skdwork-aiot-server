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
    "SDKWORK_AIOT_DEVICE_DB_PATH",
    "SDKWORK_AIOT_DEVICE_DATABASE_URL",
    "SDKWORK_AIOT_DEVICE_DATABASE_ENGINE",
    "SDKWORK_AIOT_DEVICE_DATABASE_MODE",
    "SDKWORK_AIOT_DEVICE_DATABASE_TABLE_PREFIX",
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
