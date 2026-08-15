use sdkwork_database_sqlx::DatabasePool;
use sdkwork_web_bootstrap::{ReadinessCheck, ReadinessFuture};

/// Database-backed readiness for the AIoT standalone gateway profile.
pub struct AiotDatabaseReadinessCheck {
    pool: DatabasePool,
}

impl AiotDatabaseReadinessCheck {
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }
}

impl ReadinessCheck for AiotDatabaseReadinessCheck {
    fn check(&self) -> ReadinessFuture<'_> {
        let pool = self.pool.clone();
        Box::pin(async move {
            match pool.test_connection().await {
                Ok(true) => Ok(()),
                Ok(false) => Err("AIoT database readiness query returned no row".to_owned()),
                Err(error) => Err(error.to_string()),
            }
        })
    }
}
