//! Model Context Protocol (MCP) server implementation.
//! Provides stdin/stdout JSON-RPC 2.0 stdio channel for agentic tool query integrations.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::{audit, dedup, sanitize, search, session};
use std::time::Instant;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct JsonRpcRequest {
    pub(crate) jsonrpc: String,
    pub(crate) id: Option<serde_json::Value>,
    pub(crate) method: String,
    pub(crate) params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonRpcResponse {
    pub(crate) jsonrpc: String,
    pub(crate) id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonRpcError {
    pub(crate) code: i32,
    pub(crate) message: String,
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    arguments: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct SearchSymbolsArgs {
    query: String,
}

#[derive(Debug, Deserialize)]
struct GetImpactGraphArgs {
    symbol: String,
}

#[derive(Debug, Deserialize)]
struct GetSymbolInfoArgs {
    name: String,
}

#[derive(Debug, Deserialize)]
struct GetFileContentArgs {
    path: String,
}

#[derive(Debug, Deserialize)]
struct GetUserContextArgs {
    tab_id: String,
    topic: Option<String>,
    llm: Option<String>,
}

<<<<<<< HEAD
#[derive(Debug, Deserialize)]
struct PushToMasterBrainArgs {
    title: String,
    content: String,
    author: Option<String>,
    local_episode_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QueryMasterBrainArgs {
    query: String,
    limit: Option<u32>,
}

pub async fn run_mcp_server(project_root: &Path) -> Result<()> {
    eprintln!("  [MCP] Starting Model Context Protocol (MCP) server over stdio...");
    eprintln!("  [MCP] Exposing tools: get_project_context, search_symbols, get_impact_graph, get_symbol_info, get_file_content, get_user_context, push_to_master_brain, query_master_brain");
=======
pub async fn run_mcp_server(project_root: &Path) -> Result<()> {
    eprintln!("  [MCP] Starting Model Context Protocol (MCP) server over stdio...");
    eprintln!("  [MCP] Exposing tools: get_project_context, search_symbols, get_impact_graph, get_symbol_info, get_file_content, get_user_context");
>>>>>>> origin/main

    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let mut stdout = io::stdout();

    while let Some(line) = reader.next_line().await? {
        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let err_res = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                    }),
                };
                let res_json = serde_json::to_string(&err_res)? + "\n";
                stdout.write_all(res_json.as_bytes()).await?;
                stdout.flush().await?;
                continue;
            }
        };

        let res = handle_request(project_root, &req).await;
        if let Some(response) = res {
            let res_json = serde_json::to_string(&response)? + "\n";
            stdout.write_all(res_json.as_bytes()).await?;
            stdout.flush().await?;
        }
    }

    Ok(())
}

pub(crate) async fn handle_request(project_root: &Path, req: &JsonRpcRequest) -> Option<JsonRpcResponse> {
    match req.method.as_str() {
        "initialize" => {
            let result = serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "ai-neuron-mcp",
                    "version": "1.0.0"
                }
            });
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id.clone(),
                result: Some(result),
                error: None,
            })
        }
        "notifications/initialized" => {
            // Notifications do not return responses
            None
        }
        "tools/list" => {
            let tools = serde_json::json!({
                "tools": [
                    {
                        "name": "get_project_context",
                        "description": "Get highly dense, deduplicated markdown prompt context of the active project.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "search_symbols",
                        "description": "Search across workspace databases for symbols/files matching a query.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": {
                                    "type": "string",
                                    "description": "The search term or query pattern"
                                }
                            },
                            "required": ["query"]
                        }
                    },
                    {
                        "name": "get_impact_graph",
                        "description": "Trace cascading downstream mutation impact for a structural symbol.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "symbol": {
                                    "type": "string",
                                    "description": "Name of the symbol/method/struct to trace"
                                }
                            },
                            "required": ["symbol"]
                        }
                    },
                    {
                        "name": "get_symbol_info",
                        "description": "Retrieve detailed definition snippet, language, and semantic intent for a specific structural symbol (struct, function, class).",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "name": {
                                    "type": "string",
                                    "description": "Name of the symbol to retrieve"
                                }
                            },
                            "required": ["name"]
                        }
                    },
                    {
                        "name": "get_file_content",
                        "description": "Retrieve the sanitized source content of a specific file by path or partial path. Returns up to 16 KB. Use this to inspect a file before editing.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string",
                                    "description": "File path or partial path to look up (e.g. 'src/mcp.rs' or 'mcp')"
                                }
                            },
                            "required": ["path"]
                        }
                    },
                    {
                        "name": "get_user_context",
                        "description": "Retrieve the token-efficient global personal AI memory block (user profile, active goals, recent episodes, and other tabs' active contexts) for cross-tab coherence and session personalization.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "tab_id": {
                                    "type": "string",
                                    "description": "Opaque identifier for the active editor tab or session window"
                                },
                                "topic": {
                                    "type": "string",
                                    "description": "Optional updated short summary/topic of the conversation in this tab"
                                },
                                "llm": {
                                    "type": "string",
                                    "description": "Optional name of the LLM provider/client for this session (e.g. gemini, claude)"
                                }
                            },
                            "required": ["tab_id"]
                        }
                    },
                    {
                        "name": "push_to_master_brain",
                        "description": "Contribute a sanitized architectural decision, rationale, or milestone from this developer's local Child Brain to the Corporate Master Brain vault. Credentials and secrets are automatically stripped before indexing. Use this after completing a key decision so future team members can discover the reasoning.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "title": {
                                    "type": "string",
                                    "description": "Short title of the decision or milestone (e.g. 'Switched HTTP framework to Axum')"
                                },
                                "content": {
                                    "type": "string",
                                    "description": "Full rationale, context, and consequences of the decision. May include code snippets."
                                },
                                "author": {
                                    "type": "string",
                                    "description": "Optional author identifier (username or email)"
                                },
                                "local_episode_id": {
                                    "type": "string",
                                    "description": "Optional local session episode ID to link this milestone to a recorded session"
                                }
                            },
                            "required": ["title", "content"]
                        }
                    },
                    {
                        "name": "query_master_brain",
                        "description": "Search the Corporate Master Brain episodic memory vault for past architectural decisions, rationale, or technical milestones. Use this when a developer asks 'why did we do X?' or 'what was decided about Y?'. Returns matching memories contributed by any team member.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": {
                                    "type": "string",
                                    "description": "Search term or question (e.g. 'why axum', 'database choice', 'authentication approach')"
                                },
                                "limit": {
                                    "type": "number",
                                    "description": "Maximum number of results to return (default: 5)"
                                }
                            },
                            "required": ["query"]
                        }
                    }
                ]
            });
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id.clone(),
                result: Some(tools),
                error: None,
            })
        }
        "tools/call" => {
            let params_val = match &req.params {
                Some(p) => p,
                None => {
                    return Some(error_response(req.id.clone(), -32602, "Invalid params".to_string()));
                }
            };
            let call_params: ToolCallParams = match serde_json::from_value(params_val.clone()) {
                Ok(p) => p,
                Err(e) => {
                    return Some(error_response(req.id.clone(), -32602, format!("Invalid params: {}", e)));
                }
            };

            // Enterprise audit: start wall-clock timer for this tool call
            let _t0 = Instant::now();
            let _proj = project_root.display().to_string();

            match call_params.name.as_str() {
                "get_project_context" => {
                    match session::get_agent_context_string(project_root, &[]).await {
                        Ok(raw_ctx) => {
                            let deduped = dedup::deduplicate_context(&raw_ctx);
                            let sanitized = sanitize::sanitize_content(&deduped);
                            audit::record("get_project_context", &serde_json::Value::Null, sanitized.len(), _t0.elapsed().as_millis() as u64, &_proj);
                            let content = serde_json::json!({
                                "content": [
                                    {
                                        "type": "text",
                                        "text": sanitized
                                    }
                                ]
                            });
                            Some(JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: req.id.clone(),
                                result: Some(content),
                                error: None,
                            })
                        }
                        Err(e) => Some(error_response(req.id.clone(), -32603, format!("Error getting context: {}", e))),
                    }
                }
                "search_symbols" => {
                    let args_val = match call_params.arguments {
                        Some(a) => a,
                        None => {
                            return Some(error_response(req.id.clone(), -32602, "Missing arguments".to_string()));
                        }
                    };
                    let args: SearchSymbolsArgs = match serde_json::from_value(args_val) {
                        Ok(a) => a,
                        Err(e) => {
                            return Some(error_response(req.id.clone(), -32602, format!("Invalid arguments: {}", e)));
                        }
                    };

                    match search::search_symbols_string(project_root, &args.query).await {
                        Ok(res_str) => {
                            let sanitized = sanitize::sanitize_content(&res_str);
                            audit::record("search_symbols", &serde_json::json!({"query": &args.query}), sanitized.len(), _t0.elapsed().as_millis() as u64, &_proj);
                            let content = serde_json::json!({
                                "content": [
                                    {
                                        "type": "text",
                                        "text": sanitized
                                    }
                                ]
                            });
                            Some(JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: req.id.clone(),
                                result: Some(content),
                                error: None,
                            })
                        }
                        Err(e) => Some(error_response(req.id.clone(), -32603, format!("Search failed: {}", e))),
                    }
                }
                "get_impact_graph" => {
                    let args_val = match call_params.arguments {
                        Some(a) => a,
                        None => {
                            return Some(error_response(req.id.clone(), -32602, "Missing arguments".to_string()));
                        }
                    };
                    let args: GetImpactGraphArgs = match serde_json::from_value(args_val) {
                        Ok(a) => a,
                        Err(e) => {
                            return Some(error_response(req.id.clone(), -32602, format!("Invalid arguments: {}", e)));
                        }
                    };

                    match crate::graph::get_trace_symbol_string(&args.symbol).await {
                        Ok(trace_str) => {
                            audit::record("get_impact_graph", &serde_json::json!({"symbol": &args.symbol}), trace_str.len(), _t0.elapsed().as_millis() as u64, &_proj);
                            let content = serde_json::json!({
                                "content": [
                                    {
                                        "type": "text",
                                        "text": trace_str
                                    }
                                ]
                            });
                            Some(JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: req.id.clone(),
                                result: Some(content),
                                error: None,
                            })
                        }
                        Err(e) => Some(error_response(req.id.clone(), -32603, format!("Impact trace failed: {}", e))),
                    }
                }
                "get_symbol_info" => {
                    let args_val = match call_params.arguments {
                        Some(a) => a,
                        None => {
                            return Some(error_response(req.id.clone(), -32602, "Missing arguments".to_string()));
                        }
                    };
                    let args: GetSymbolInfoArgs = match serde_json::from_value(args_val) {
                        Ok(a) => a,
                        Err(e) => {
                            return Some(error_response(req.id.clone(), -32602, format!("Invalid arguments: {}", e)));
                        }
                    };

                    match search::get_symbol_info_string(project_root, &args.name).await {
                        Ok(res_str) => {
                            let sanitized = sanitize::sanitize_content(&res_str);
                            audit::record("get_symbol_info", &serde_json::json!({"name": &args.name}), sanitized.len(), _t0.elapsed().as_millis() as u64, &_proj);
                            let content = serde_json::json!({
                                "content": [
                                    {
                                        "type": "text",
                                        "text": sanitized
                                    }
                                ]
                            });
                            Some(JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: req.id.clone(),
                                result: Some(content),
                                error: None,
                            })
                        }
                        Err(e) => Some(error_response(req.id.clone(), -32603, format!("Symbol lookup failed: {}", e))),
                    }
                }
                "get_file_content" => {
                    let args_val = match call_params.arguments {
                        Some(a) => a,
                        None => {
                            return Some(error_response(req.id.clone(), -32602, "Missing arguments".to_string()));
                        }
                    };
                    let args: GetFileContentArgs = match serde_json::from_value(args_val) {
                        Ok(a) => a,
                        Err(e) => {
                            return Some(error_response(req.id.clone(), -32602, format!("Invalid arguments: {}", e)));
                        }
                    };

                    match search::get_file_content_string(project_root, &args.path).await {
                        Ok(res_str) => {
                            let sanitized = sanitize::sanitize_content(&res_str);
                            audit::record("get_file_content", &serde_json::json!({"path": &args.path}), sanitized.len(), _t0.elapsed().as_millis() as u64, &_proj);
                            let content = serde_json::json!({
                                "content": [
                                    {
                                        "type": "text",
                                        "text": sanitized
                                    }
                                ]
                            });
                            Some(JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: req.id.clone(),
                                result: Some(content),
                                error: None,
                            })
                        }
                        Err(e) => Some(error_response(req.id.clone(), -32603, format!("File content lookup failed: {}", e))),
                    }
                }
                "get_user_context" => {
                    let args_val = match call_params.arguments {
                        Some(a) => a,
                        None => {
                            return Some(error_response(req.id.clone(), -32602, "Missing arguments".to_string()));
                        }
                    };
                    let args: GetUserContextArgs = match serde_json::from_value(args_val) {
                        Ok(a) => a,
                        Err(e) => {
                            return Some(error_response(req.id.clone(), -32602, format!("Invalid arguments: {}", e)));
                        }
                    };

                    match crate::sessions::open_pool().await {
                        Ok(pool) => {
                            if args.topic.is_some() || args.llm.is_some() {
                                let topic_val = args.topic.as_deref().unwrap_or("active context");
                                let llm_val = args.llm.as_deref().unwrap_or("unknown");
                                if let Err(e) = crate::sessions::upsert_session(&pool, &args.tab_id, topic_val, llm_val).await {
                                    tracing::warn!("Failed to log active session via MCP: {}", e);
                                }
                            }
                            match crate::sessions::get_context_block(&pool, &args.tab_id).await {
                                Ok(ctx_block) => {
                                    audit::record(
                                        "get_user_context",
                                        &serde_json::json!({
                                            "tab_id": &args.tab_id,
                                            "topic": &args.topic,
                                            "llm": &args.llm
                                        }),
                                        ctx_block.len(),
                                        _t0.elapsed().as_millis() as u64,
                                        &_proj
                                    );
                                    let content = serde_json::json!({
                                        "content": [
                                            {
                                                "type": "text",
                                                "text": ctx_block
                                            }
                                        ]
                                    });
                                    Some(JsonRpcResponse {
                                        jsonrpc: "2.0".to_string(),
                                        id: req.id.clone(),
                                        result: Some(content),
                                        error: None,
                                    })
                                }
                                Err(e) => Some(error_response(req.id.clone(), -32603, format!("Failed to generate sessions context: {}", e))),
                            }
                        }
                        Err(e) => Some(error_response(req.id.clone(), -32603, format!("Failed to open sessions database: {}", e))),
                    }
                }
                "push_to_master_brain" => {
                    let args_val = match call_params.arguments {
                        Some(a) => a,
                        None => return Some(error_response(req.id.clone(), -32602, "Missing arguments".to_string())),
                    };
                    let args: PushToMasterBrainArgs = match serde_json::from_value(args_val) {
                        Ok(a) => a,
                        Err(e) => return Some(error_response(req.id.clone(), -32602, format!("Invalid arguments: {}", e))),
                    };

                    // Sanitize the content to strip credentials before indexing
                    let sanitized = sanitize::sanitize_content(&args.content);
                    let author = args.author.as_deref().unwrap_or("agent").to_string();

                    match crate::sessions::open_pool().await {
                        Ok(pool) => {
                            let sync_id = format!("sync-{}", uuid::Uuid::new_v4());
                            let now = chrono::Utc::now().to_rfc3339();
                            let result = sqlx::query(
                                "INSERT INTO synced_memories (id, local_episode_id, title, content, synced_at, author) VALUES (?, ?, ?, ?, ?, ?)"
                            )
                            .bind(&sync_id)
                            .bind(&args.local_episode_id)
                            .bind(&args.title)
                            .bind(&sanitized)
                            .bind(&now)
                            .bind(&author)
                            .execute(&pool)
                            .await;

                            let text = match result {
                                Ok(_) => format!(
                                    "✓ Memory contributed to Master Brain\n\
                                     ID: {}\n\
                                     Title: {}\n\
                                     Author: {}\n\
                                     Sanitized length: {} chars\n\
                                     Indexed at: {}",
                                    sync_id, args.title, author, sanitized.len(), now
                                ),
                                Err(e) => format!("Failed to index memory: {}", e),
                            };

                            audit::record("push_to_master_brain", &serde_json::json!({"title": &args.title}), sanitized.len(), _t0.elapsed().as_millis() as u64, &_proj);
                            Some(JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: req.id.clone(),
                                result: Some(serde_json::json!({ "content": [{ "type": "text", "text": text }] })),
                                error: None,
                            })
                        }
                        Err(e) => Some(error_response(req.id.clone(), -32603, format!("Failed to open sessions db: {}", e))),
                    }
                }
                "query_master_brain" => {
                    let args_val = match call_params.arguments {
                        Some(a) => a,
                        None => return Some(error_response(req.id.clone(), -32602, "Missing arguments".to_string())),
                    };
                    let args: QueryMasterBrainArgs = match serde_json::from_value(args_val) {
                        Ok(a) => a,
                        Err(e) => return Some(error_response(req.id.clone(), -32602, format!("Invalid arguments: {}", e))),
                    };

                    let limit = args.limit.unwrap_or(5) as i64;
                    let pattern = format!("%{}%", args.query.to_lowercase());

                    match crate::sessions::open_pool().await {
                        Ok(pool) => {
                            let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
                                "SELECT id, title, content, author, synced_at FROM synced_memories \
                                 WHERE LOWER(title) LIKE ? OR LOWER(content) LIKE ? \
                                 ORDER BY synced_at DESC LIMIT ?"
                            )
                            .bind(&pattern)
                            .bind(&pattern)
                            .bind(limit)
                            .fetch_all(&pool)
                            .await
                            .unwrap_or_default();

                            let text = if rows.is_empty() {
                                format!("No memories found in Master Brain matching '{}'. Ask a team member to push relevant decisions using push_to_master_brain.", args.query)
                            } else {
                                let mut out = format!("## Master Brain — {} result(s) for '{}'", rows.len(), args.query);
                                for (id, title, content, author, synced_at) in &rows {
                                    out.push_str(&format!(
                                        "\n\n### {title}\n\
                                         - **Author**: {author}\n\
                                         - **Indexed**: {synced_at}\n\
                                         - **ID**: `{id}`\n\
                                         \n{content}"
                                    ));
                                }
                                out
                            };

                            audit::record("query_master_brain", &serde_json::json!({"query": &args.query}), text.len(), _t0.elapsed().as_millis() as u64, &_proj);
                            Some(JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: req.id.clone(),
                                result: Some(serde_json::json!({ "content": [{ "type": "text", "text": text }] })),
                                error: None,
                            })
                        }
                        Err(e) => Some(error_response(req.id.clone(), -32603, format!("Failed to open sessions db: {}", e))),
                    }
                }
                _ => Some(error_response(req.id.clone(), -32601, format!("Tool not found: {}", call_params.name))),
            }
        }
        _ => Some(error_response(req.id.clone(), -32601, format!("Method not found: {}", req.method))),
    }
}

pub(crate) fn error_response(id: Option<serde_json::Value>, code: i32, message: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: None,
        error: Some(JsonRpcError { code, message }),
    }
}
