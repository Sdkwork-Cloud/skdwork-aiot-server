//! Production Xiaozhi intelligence bridge: kernel agent runtime + Claw Router speech.

mod claw_router;
pub mod config;
pub mod kernel_runtime;
mod mcp;
pub mod session_map;
mod speech;

pub use config::{
    intelligence_mode_from_env, is_kernel_mode, IntelligenceConfig, IntelligenceMode,
    DEFAULT_KERNEL_AGENT_ID, ENV_INTELLIGENCE_ASR_MODEL, ENV_INTELLIGENCE_KERNEL_AGENT_ID,
    ENV_INTELLIGENCE_KERNEL_HTTP_URL, ENV_INTELLIGENCE_MODE, ENV_INTELLIGENCE_TTS_MODEL,
    ENV_INTELLIGENCE_TTS_VOICE, KERNEL_PUBLIC_HTTP_URL_FALLBACK_ENV,
};
pub use kernel_runtime::KernelRuntimeClient;
pub use mcp::{KernelMcpStack, KernelMcpToolInvoker, KernelMcpToolProvider};
pub use session_map::SessionMap;
pub use speech::{KernelSpeechPipeline, SpeechTurnInput, SpeechTurnOutput};

use std::sync::Arc;

/// Shared production stack: speech pipeline + MCP provider/invoker backed by one session map.
#[derive(Clone)]
pub struct KernelIntelligenceStack {
    pub speech: Arc<KernelSpeechPipeline>,
    pub mcp_provider: Arc<KernelMcpToolProvider>,
    pub mcp_invoker: Arc<KernelMcpToolInvoker>,
}

impl KernelIntelligenceStack {
    pub fn from_config(config: IntelligenceConfig) -> Result<Self, String> {
        let session_map = session_map::SessionMap::new();
        let kernel = kernel_runtime::KernelRuntimeClient::new(config.kernel_http_url.clone())?;
        let claw = claw_router::ClawRouterClient::from_config(&config)?;
        let speech = Arc::new(KernelSpeechPipeline::new(
            config.clone(),
            kernel.clone(),
            claw.clone(),
            session_map.clone(),
        ));
        let mcp_provider = Arc::new(KernelMcpToolProvider::new(
            kernel.clone(),
            session_map.clone(),
            config.kernel_agent_id.clone(),
        ));
        let mcp_invoker = Arc::new(KernelMcpToolInvoker::new(kernel, session_map));
        Ok(Self {
            speech,
            mcp_provider,
            mcp_invoker,
        })
    }
}

/// Build production stack from environment when mode is `kernel`.
pub fn kernel_stack_from_env() -> Result<KernelIntelligenceStack, String> {
    let config = IntelligenceConfig::from_env()?;
    KernelIntelligenceStack::from_config(config)
}
