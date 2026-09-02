//! API assembly for sdkwork-aiot.
//! Application bootstrap lives in `bootstrap.rs`; route inventory is in `assembly-manifest.json`.
//! SDKWORK-ASSEMBLY-LIB-CUSTOM: exports beyond the canonical materializer template.

mod bootstrap;
mod generated;
mod readiness;

pub use bootstrap::{
    assemble_api_router, assemble_api_router_with_database_host, assemble_api_router_with_pool,
    web_module, web_module_with_pool, ApiAssembly,
};

pub fn assembly_route_count() -> usize {
    generated::ROUTE_CRATE_COUNT
}
