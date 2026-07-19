use std::future::Future;
use std::sync::{Arc, OnceLock};

use tokio::runtime::{Builder, Handle, Runtime};

static BLOCKING_RUNTIME: OnceLock<Result<Arc<Runtime>, String>> = OnceLock::new();

pub(crate) fn shared_runtime() -> Result<Arc<Runtime>, String> {
    BLOCKING_RUNTIME
        .get_or_init(|| {
            Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .map(Arc::new)
                .map_err(|error| error.to_string())
        })
        .clone()
}

pub(crate) fn block_on<F>(runtime: &Runtime, future: F) -> F::Output
where
    F: Future,
{
    if Handle::try_current().is_ok() {
        return tokio::task::block_in_place(|| runtime.block_on(future));
    }
    runtime.block_on(future)
}
