use std::sync::Arc;

use crate::kernel_runtime::{KernelRuntimeClient, KernelToolDescriptor};
use crate::session_map::SessionMap;

/// Gateway-facing MCP tool provider backed by kernel runtime tool catalog.
pub struct KernelMcpToolProvider {
    kernel: KernelRuntimeClient,
    session_map: SessionMap,
    agent_id: String,
    local_tools: Vec<KernelToolDescriptor>,
}

impl KernelMcpToolProvider {
    pub fn new(kernel: KernelRuntimeClient, session_map: SessionMap, agent_id: String) -> Self {
        Self {
            kernel,
            session_map,
            agent_id,
            local_tools: Vec::new(),
        }
    }

    pub fn with_local_tools(mut self, local_tools: Vec<KernelToolDescriptor>) -> Self {
        self.local_tools = local_tools;
        self
    }

    pub fn list_tools_for_session(
        &self,
        xiaozhi_session_id: &str,
    ) -> Result<Vec<KernelToolDescriptor>, String> {
        let kernel_session_id =
            self.kernel
                .ensure_session(&self.session_map, xiaozhi_session_id, &self.agent_id)?;
        let mut tools = self.kernel.list_tools(&kernel_session_id)?;
        for local in &self.local_tools {
            if !tools.iter().any(|tool| tool.name == local.name) {
                tools.push(local.clone());
            }
        }
        Ok(tools)
    }
}

/// Gateway-facing MCP tool invoker backed by kernel runtime execute endpoint.
pub struct KernelMcpToolInvoker {
    kernel: KernelRuntimeClient,
    session_map: SessionMap,
}

impl KernelMcpToolInvoker {
    pub fn new(kernel: KernelRuntimeClient, session_map: SessionMap) -> Self {
        Self {
            kernel,
            session_map,
        }
    }

    pub fn invoke_tool(
        &self,
        xiaozhi_session_id: &str,
        tool_name: &str,
        tool_arguments_json: Option<&str>,
    ) -> Result<String, String> {
        let kernel_session_id = session_map_kernel_session(&self.session_map, xiaozhi_session_id)?;
        let input = tool_arguments_json.unwrap_or("{}").to_string();
        self.kernel
            .execute_tool(&kernel_session_id, tool_name, &input)
    }
}

#[derive(Clone)]
pub struct KernelMcpStack {
    pub provider: Arc<KernelMcpToolProvider>,
    pub invoker: Arc<KernelMcpToolInvoker>,
}

impl KernelMcpStack {
    pub fn new(provider: Arc<KernelMcpToolProvider>, invoker: Arc<KernelMcpToolInvoker>) -> Self {
        Self { provider, invoker }
    }
}

fn session_map_kernel_session(
    session_map: &SessionMap,
    xiaozhi_session_id: &str,
) -> Result<String, String> {
    session_map
        .get_kernel_session(xiaozhi_session_id)
        .ok_or_else(|| {
            format!("kernel session not established for xiaozhi session {xiaozhi_session_id}")
        })
}
