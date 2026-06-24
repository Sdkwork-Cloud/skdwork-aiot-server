use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
pub struct SessionMap {
    inner: Arc<Mutex<HashMap<String, String>>>,
}

impl SessionMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_kernel_session(&self, xiaozhi_session_id: &str) -> Option<String> {
        self.inner
            .lock()
            .ok()
            .and_then(|guard| guard.get(xiaozhi_session_id).cloned())
    }

    pub fn put_kernel_session(
        &self,
        xiaozhi_session_id: impl Into<String>,
        kernel_session_id: impl Into<String>,
    ) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.insert(xiaozhi_session_id.into(), kernel_session_id.into());
        }
    }
}
