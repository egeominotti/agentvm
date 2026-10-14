//! Native SDK MCP facade over an existing loopback engine.
mod catalog;
pub mod client;
mod launch_parameters;
mod parameters;
mod transfers;

use parameters::*;
use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

#[derive(Clone)]
pub struct McpServer {
    client: client::ApiClient,
}
impl McpServer {
    pub fn new(client: client::ApiClient) -> Self {
        Self { client }
    }

    async fn invoke(&self, name: &str, args: Value) -> Result<Value, String> {
        if name == "agentvm_output" {
            let args: OutputArgs = serde_json::from_value(args).map_err(|_| "invalid output tool arguments")?;
            let path = format!(
                "/api/tasks/{}/output?cursor={}&limit={}",
                segment(&args.id)?,
                args.cursor.unwrap_or(0),
                args.limit.unwrap_or(100)
            );
            return self.client.api("GET", &path, None).await;
        }
        if name == "agentvm_launch" {
            return self.client.api("POST", "/api/tasks", Some(typed::<launch_parameters::LaunchArgs>(args)?)).await;
        }
        if let Some(route) = catalog::ROUTES.iter().find(|route| route.name == name) {
            let parsed: RouteArgs = serde_json::from_value(args).map_err(|_| "invalid route tool arguments")?;
            let mut path = route.path.to_owned();
            for (name, value) in [("id", parsed.id), ("exec_id", parsed.exec_id)] {
                if path.contains(&format!("{{{name}}}")) {
                    let value = value.ok_or_else(|| format!("{name} is required"))?;
                    path = path.replace(&format!("{{{name}}}"), segment(&value)?);
                }
            }
            if let Some(query) = parsed.query {
                let query = url::form_urlencoded::Serializer::new(String::new()).extend_pairs(query).finish();
                if !query.is_empty() {
                    path.push('?');
                    path.push_str(&query);
                }
            }
            return self.client.api(route.method, &path, parsed.body).await;
        }
        match name {
            "agentvm_api" => {
                let parsed: ApiArgs = serde_json::from_value(args).map_err(|_| "invalid API tool arguments")?;
                self.client.api(&parsed.method, &parsed.path, parsed.body).await
            }
            "agentvm_transfer" => {
                let parsed = serde_json::from_value(args).map_err(|_| "invalid transfer tool arguments")?;
                transfers::transfer(&self.client, parsed).await
            }
            "agentvm_exec" | "agentvm_shell" | "agentvm_files" | "agentvm_terminal" => {
                let mut body = match name {
                    "agentvm_exec" => typed::<ExecArgs>(args)?,
                    "agentvm_shell" => typed::<ShellArgs>(args)?,
                    "agentvm_files" => typed::<FileArgs>(args)?,
                    _ => typed::<TerminalArgs>(args)?,
                };
                let object = body.as_object_mut().ok_or("arguments must be an object")?;
                let id = object.remove("id").and_then(|v| v.as_str().map(str::to_owned)).ok_or("id is required")?;
                if name == "agentvm_shell" {
                    let script = object.remove("script").ok_or("script is required")?;
                    object.insert("argv".into(), json!(["/bin/bash", "-lc", script]));
                }
                let group = match name {
                    "agentvm_files" => "files",
                    "agentvm_terminal" => "terminal",
                    _ => "exec",
                };
                let operation = body.get("op").and_then(Value::as_str).unwrap_or("start");
                let mut validated = body.clone();
                validated.as_object_mut().unwrap().remove("op");
                crate::domain::guest_control::validate(&format!("{group}.{operation}"), &validated)?;
                self.client.api("POST", &format!("/api/tasks/{}/control/{group}", segment(&id)?), Some(body)).await
            }
            _ => Err("unknown AgentVM tool".into()),
        }
    }
}

pub(crate) fn segment(value: &str) -> Result<&str, String> {
    if value.is_empty()
        || value.len() > 128
        || !value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("identifier must contain only letters, numbers, hyphens or underscores".into());
    }
    Ok(value)
}

fn typed<T: DeserializeOwned + Serialize>(args: Value) -> Result<Value, String> {
    let parsed: T = serde_json::from_value(args).map_err(|_| "invalid control tool arguments")?;
    let mut value = serde_json::to_value(parsed).map_err(|_| "cannot serialize tool arguments")?;
    value.as_object_mut().unwrap().retain(|_, v| !v.is_null());
    Ok(value)
}

fn special_tool<T: rmcp::schemars::JsonSchema + 'static>(name: &'static str, description: &'static str) -> Tool {
    Tool::new(name, description, serde_json::Map::new())
        .with_input_schema::<T>()
        .annotate(catalog::annotations(false, true))
}

fn tools() -> Vec<Tool> {
    let mut tools: Vec<_> = catalog::ROUTES
        .iter()
        .map(|route| {
            let tool = catalog::tool(route);
            if route.name == "agentvm_launch" {
                tool.with_input_schema::<launch_parameters::LaunchArgs>()
            } else if route.name == "agentvm_output" {
                tool.with_input_schema::<OutputArgs>()
            } else {
                tool
            }
        })
        .collect();
    tools.extend([
        special_tool::<ApiArgs>(
            "agentvm_api",
            "Call any finite AgentVM JSON REST API route; credentials can only be saved",
        ),
        special_tool::<ExecArgs>(
            "agentvm_exec",
            "Start a literal argv process in a VM; returns exec_id for reading/cancelling",
        ),
        special_tool::<ShellArgs>("agentvm_shell", "Explicitly execute a bash script in a VM; returns exec_id"),
        special_tool::<FileArgs>("agentvm_files", "Guest directory and chunked base64 binary file operations"),
        special_tool::<TerminalArgs>(
            "agentvm_terminal",
            "List, create, capture, send literal text/keys or resize persistent tmux sessions",
        ),
        special_tool::<TransferArgs>(
            "agentvm_transfer",
            "Stream snapshot export/import or VM upload through an explicit host file path",
        ),
    ]);
    tools
}

impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().enable_resources().build())
            .with_server_info(Implementation::new("agentvm-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions("Controls the existing AgentVM daemon. Read agentvm://api for routes. VM IDs are explicit; MCP disconnect does not stop VMs.")
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        tools().into_iter().find(|t| t.name == name)
    }

    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(tools()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let args = Value::Object(request.arguments.unwrap_or_default());
        let mut secrets = Vec::new();
        secret_values(&args, &mut secrets);
        let result = self.invoke(&request.name, args).await;
        let (value, error) = match result {
            Ok(value) => (value, false),
            Err(error) => (json!({"error":error}), true),
        };
        let value = scrub(value, &secrets);
        let text = value.to_string();
        let display =
            if text.len() <= 16384 { text } else { "Result is available in structuredContent (large output).".into() };
        let mut result = if error {
            CallToolResult::error(vec![ContentBlock::text(display)])
        } else {
            CallToolResult::success(vec![ContentBlock::text(display)])
        };
        result.structured_content = Some(value);
        Ok(result.into())
    }

    async fn list_resources(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult::with_all_items(vec![
            Resource::new("agentvm://api", "AgentVM API reference").with_mime_type("text/plain"),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        if request.uri != "agentvm://api" {
            return Err(ErrorData::invalid_params("unknown resource", None));
        }
        Ok(ReadResourceResult::new(vec![ResourceContents::text(catalog::reference(), "agentvm://api")]).into())
    }
}

fn secret_values(value: &Value, secrets: &mut Vec<String>) {
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            if ["token", "secret", "key", "api_key", "password", "credential"]
                .iter()
                .any(|name| key.to_lowercase().contains(name))
                && let Some(value) = value.as_str().filter(|v| !v.is_empty())
            {
                secrets.push(value.into());
            }
            secret_values(value, secrets);
        }
    } else if let Some(array) = value.as_array() {
        for value in array {
            secret_values(value, secrets);
        }
    }
}

fn scrub(value: Value, secrets: &[String]) -> Value {
    match value {
        Value::String(mut text) => {
            for secret in secrets {
                text = text.replace(secret, "[redacted]");
            }
            Value::String(text)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(|v| scrub(v, secrets)).collect()),
        Value::Object(values) => Value::Object(values.into_iter().map(|(k, v)| (k, scrub(v, secrets))).collect()),
        value => value,
    }
}
