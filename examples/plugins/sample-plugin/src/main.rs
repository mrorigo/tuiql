use serde::{Deserialize, Serialize};
use std::io::{self, Read};

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<u64>,
    method: String,
    params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<u64>,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct PluginManifest {
    name: String,
    version: String,
    description: Option<String>,
    capabilities: Vec<PluginCapability>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum PluginCapability {
    #[serde(rename = "command")]
    Command { name: String, description: String, usage: Option<String> },
    #[serde(rename = "panel")]
    Panel { name: String, description: String, category: String },
    #[serde(rename = "exporter")]
    Exporter { format: String, description: String, extension: Option<String> },
}

fn main() {
    let manifest = PluginManifest {
        name: "sample-plugin".to_string(),
        version: "0.1.0".to_string(),
        description: Some("Sample TuiQL plugin demonstrating JSON-RPC interface".to_string()),
        capabilities: vec![
            PluginCapability::Command {
                name: "hello".to_string(),
                description: "Prints a greeting message".to_string(),
                usage: Some("hello [name]".to_string()),
            },
            PluginCapability::Panel {
                name: "sample-stats".to_string(),
                description: "Shows sample statistics".to_string(),
                category: "analysis".to_string(),
            },
            PluginCapability::Exporter {
                format: "txt".to_string(),
                description: "Plain text export".to_string(),
                extension: Some("txt".to_string()),
            },
        ],
    };

    // Read from stdin
    let mut buffer = String::new();
    match io::stdin().read_to_string(&mut buffer) {
        Ok(_) => {
            let request: JsonRpcRequest = match serde_json::from_str(&buffer) {
                Ok(req) => req,
                Err(e) => {
                    let error_response = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: None,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32700,
                            message: format!("Parse error: {}", e),
                            data: None,
                        }),
                    };
                    println!("{}", serde_json::to_string(&error_response).unwrap());
                    return;
                }
            };

            // Handle the request
            let response = match request.method.as_str() {
                "manifest" => {
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: Some(serde_json::to_value(manifest).unwrap()),
                        error: None,
                    }
                }
                "hello" => {
                    let name = request.params.as_ref()
                        .and_then(|p| p.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("World");
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: Some(serde_json::json!({
                            "message": format!("Hello, {}!", name)
                        })),
                        error: None,
                    }
                }
                "sample-stats" => {
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: Some(serde_json::json!({
                            "tables": 5,
                            "queries_today": 42,
                            "connections": ["main.db", "cache.db"]
                        })),
                        error: None,
                    }
                }
                "export-txt" => {
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: Some(serde_json::json!({
                            "data": "Sample export data in plain text format",
                            "format": "txt"
                        })),
                        error: None,
                    }
                }
                _ => {
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32601,
                            message: format!("Method '{}' not found", request.method),
                            data: None,
                        }),
                    }
                }
            };

            println!("{}", serde_json::to_string(&response).unwrap());
        }
        Err(e) => {
            let error_response = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: None,
                result: None,
                error: Some(JsonRpcError {
                    code: -32603,
                    message: format!("Internal error: {}", e),
                    data: None,
                }),
            };
            println!("{}", serde_json::to_string(&error_response).unwrap());
        }
    }
}
