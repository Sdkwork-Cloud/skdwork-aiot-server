use serde::Deserialize;
use serde_json::Value;

use crate::config::INTERNAL_RUNTIME_MOUNT_PREFIX;

#[derive(Clone)]
pub struct KernelRuntimeClient {
    base_url: String,
}

impl KernelRuntimeClient {
    pub fn new(base_url: String) -> Result<Self, String> {
        Ok(Self { base_url })
    }

    fn runtime_url(&self, relative: &str) -> String {
        format!(
            "{}{INTERNAL_RUNTIME_MOUNT_PREFIX}{relative}",
            self.base_url.trim_end_matches('/')
        )
    }

    pub fn ensure_session(
        &self,
        session_map: &crate::session_map::SessionMap,
        xiaozhi_session_id: &str,
        agent_id: &str,
    ) -> Result<String, String> {
        if let Some(kernel_session_id) = session_map.get_kernel_session(xiaozhi_session_id) {
            return Ok(kernel_session_id);
        }
        let url = self.runtime_url("/sessions");
        let response = ureq::post(&url)
            .set("Content-Type", "application/json")
            .send_string(
                &serde_json::to_string(&serde_json::json!({
                    "agentId": agent_id,
                    "title": format!("xiaozhi:{xiaozhi_session_id}"),
                }))
                .map_err(|error| format!("kernel session create json failed: {error}"))?,
            )
            .map_err(|error| format!("kernel session create failed: {error}"))?;
        if !(200..300).contains(&response.status()) {
            return Err(format!(
                "kernel session create failed with status {}",
                response.status()
            ));
        }
        let body = response
            .into_string()
            .map_err(|error| format!("kernel session create body read failed: {error}"))?;
        let session: InternalSessionResponse = serde_json::from_str(&body)
            .map_err(|error| format!("kernel session create parse failed: {error}"))?;
        session_map.put_kernel_session(xiaozhi_session_id, session.session_id.clone());
        Ok(session.session_id)
    }

    pub fn send_user_message(
        &self,
        kernel_session_id: &str,
        content: &str,
    ) -> Result<String, String> {
        let url = self.runtime_url(&format!("/sessions/{kernel_session_id}/messages"));
        let response = ureq::post(&url)
            .set("Content-Type", "application/json")
            .send_string(
                &serde_json::to_string(&serde_json::json!({ "content": content }))
                    .map_err(|error| format!("kernel message send json failed: {error}"))?,
            )
            .map_err(|error| format!("kernel message send failed: {error}"))?;
        if !(200..300).contains(&response.status()) {
            return Err(format!(
                "kernel message send failed with status {}",
                response.status()
            ));
        }
        let body = response
            .into_string()
            .map_err(|error| format!("kernel message send body read failed: {error}"))?;
        extract_assistant_reply(&body)
    }

    pub fn list_tools(&self, kernel_session_id: &str) -> Result<Vec<KernelToolDescriptor>, String> {
        let url = self.runtime_url(&format!("/sessions/{kernel_session_id}/tools"));
        let response = ureq::get(&url)
            .call()
            .map_err(|error| format!("kernel tools list failed: {error}"))?;
        if !(200..300).contains(&response.status()) {
            return Err(format!(
                "kernel tools list failed with status {}",
                response.status()
            ));
        }
        let body = response
            .into_string()
            .map_err(|error| format!("kernel tools list body read failed: {error}"))?;
        let payload: ToolListResponse = serde_json::from_str(&body)
            .map_err(|error| format!("kernel tools list parse failed: {error}"))?;
        Ok(payload
            .items
            .into_iter()
            .filter_map(KernelToolDescriptor::from_value)
            .collect())
    }

    pub fn execute_tool(
        &self,
        kernel_session_id: &str,
        tool_name: &str,
        input: &str,
    ) -> Result<String, String> {
        let url = self.runtime_url(&format!(
            "/sessions/{kernel_session_id}/tools/{tool_name}/execute"
        ));
        let response = ureq::post(&url)
            .set("Content-Type", "application/json")
            .send_string(
                &serde_json::to_string(&serde_json::json!({ "input": input }))
                    .map_err(|error| format!("kernel tool execute json failed: {error}"))?,
            )
            .map_err(|error| format!("kernel tool execute failed: {error}"))?;
        if !(200..300).contains(&response.status()) {
            return Err(format!(
                "kernel tool execute failed with status {}",
                response.status()
            ));
        }
        let body = response
            .into_string()
            .map_err(|error| format!("kernel tool execute body read failed: {error}"))?;
        extract_tool_result_text(&body)
    }
}

#[derive(Debug, Clone)]
pub struct KernelToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema_json: String,
    pub user_only: bool,
}

impl KernelToolDescriptor {
    fn from_value(value: Value) -> Option<Self> {
        let name = value
            .get("name")
            .or_else(|| value.get("toolName"))
            .and_then(Value::as_str)?
            .to_string();
        let description = value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let input_schema_json = value
            .get("inputSchema")
            .or_else(|| value.get("input_schema"))
            .map(|schema| schema.to_string())
            .unwrap_or_else(|| "{}".to_string());
        let user_only = value
            .get("userOnly")
            .or_else(|| value.get("user_only"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Some(Self {
            name,
            description,
            input_schema_json,
            user_only,
        })
    }
}

#[derive(Debug, Deserialize)]
struct InternalSessionResponse {
    #[serde(rename = "sessionId")]
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct ToolListResponse {
    #[serde(default)]
    items: Vec<Value>,
}

fn extract_assistant_reply(body: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(body)
        .map_err(|error| format!("kernel message parse failed: {error}"))?;
    if let Some(content) = value.get("content").and_then(Value::as_str) {
        return Ok(content.to_string());
    }
    if let Some(items) = value.get("items").and_then(Value::as_array) {
        for item in items.iter().rev() {
            let role = item.get("role").and_then(Value::as_str).unwrap_or_default();
            if role.eq_ignore_ascii_case("assistant") {
                if let Some(content) = item.get("content").and_then(Value::as_str) {
                    return Ok(content.to_string());
                }
            }
        }
    }
    Err("kernel message response did not include assistant content".to_string())
}

fn extract_tool_result_text(body: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(body)
        .map_err(|error| format!("kernel tool execute parse failed: {error}"))?;
    if let Some(text) = value.get("output").and_then(Value::as_str) {
        return Ok(text.to_string());
    }
    if let Some(text) = value.get("result").and_then(Value::as_str) {
        return Ok(text.to_string());
    }
    if let Some(text) = value.get("content").and_then(Value::as_str) {
        return Ok(text.to_string());
    }
    Ok(value.to_string())
}
