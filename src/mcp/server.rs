use anyhow::Result;
use serde_json::json;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use super::protocol::*;
use super::{resources, tools};

pub fn run(project_root: PathBuf) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse::error(None, -32700, format!("Parse error: {e}"));
                send(&mut stdout, &resp)?;
                continue;
            }
        };

        let response = dispatch(&request, &project_root);

        if let Some(resp) = response {
            send(&mut stdout, &resp)?;
        }
    }

    Ok(())
}

fn send(out: &mut impl Write, response: &JsonRpcResponse) -> Result<()> {
    let json = serde_json::to_string(response)?;
    writeln!(out, "{json}")?;
    out.flush()?;
    Ok(())
}

fn dispatch(request: &JsonRpcRequest, project_root: &PathBuf) -> Option<JsonRpcResponse> {
    match request.method.as_str() {
        "initialize" => Some(handle_initialize(request)),
        "initialized" => None, // notification, no response
        "ping" => Some(JsonRpcResponse::success(request.id.clone(), json!({}))),
        "tools/list" => Some(handle_tools_list(request)),
        "tools/call" => Some(handle_tools_call(request, project_root)),
        "resources/list" => Some(handle_resources_list(request, project_root)),
        "resources/read" => Some(handle_resources_read(request, project_root)),
        _ => Some(JsonRpcResponse::error(
            request.id.clone(),
            -32601,
            format!("Method not found: {}", request.method),
        )),
    }
}

fn handle_initialize(request: &JsonRpcRequest) -> JsonRpcResponse {
    let result = InitializeResult {
        protocol_version: "2024-11-05".into(),
        capabilities: ServerCapabilities {
            tools: Some(ToolsCapability {
                list_changed: false,
            }),
            resources: Some(ResourcesCapability {
                list_changed: false,
            }),
        },
        server_info: ServerInfo {
            name: "booger".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
    };
    JsonRpcResponse::success(request.id.clone(), serde_json::to_value(result).unwrap())
}

fn handle_tools_list(request: &JsonRpcRequest) -> JsonRpcResponse {
    let tools = tools::list_tools();
    JsonRpcResponse::success(request.id.clone(), json!({ "tools": tools }))
}

fn handle_tools_call(request: &JsonRpcRequest, project_root: &PathBuf) -> JsonRpcResponse {
    let name = request
        .params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let args = request
        .params
        .get("arguments")
        .cloned()
        .unwrap_or(json!({}));

    let result = tools::call_tool(name, &args, project_root);
    JsonRpcResponse::success(request.id.clone(), serde_json::to_value(result).unwrap())
}

fn handle_resources_list(request: &JsonRpcRequest, project_root: &PathBuf) -> JsonRpcResponse {
    let resources = resources::list_resources(project_root);
    JsonRpcResponse::success(request.id.clone(), json!({ "resources": resources }))
}

fn handle_resources_read(request: &JsonRpcRequest, project_root: &PathBuf) -> JsonRpcResponse {
    let uri = request
        .params
        .get("uri")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match resources::read_resource(uri, project_root) {
        Ok(contents) => {
            JsonRpcResponse::success(request.id.clone(), json!({ "contents": contents }))
        }
        Err(e) => JsonRpcResponse::error(request.id.clone(), -32602, e),
    }
}
