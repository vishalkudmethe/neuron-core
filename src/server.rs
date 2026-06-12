//! High-performance TCP Model Context Protocol (MCP) server for containerized and cloud-hosted environments.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use crate::mcp::{self, JsonRpcRequest, JsonRpcResponse, JsonRpcError};

/// Start a TCP-based Model Context Protocol (MCP) server.
/// Listens on the specified port and routes requests to the unified handle_request dispatcher.
pub async fn start_mcp_tcp_server(project_root: &Path, port: u16) -> Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    println!("  [SERVER] Neuron TCP MCP Server listening on {}", addr);
    println!("  [SERVER] Bound workspace: {}", project_root.display());

    let root_arc = Arc::new(PathBuf::from(project_root));

    loop {
        let (mut socket, peer) = match listener.accept().await {
            Ok(val) => val,
            Err(e) => {
                eprintln!("[SERVER] Failed to accept socket: {}", e);
                continue;
            }
        };

        let root = root_arc.clone();
        tokio::spawn(async move {
            let (rx, mut tx) = socket.split();
            let mut reader = BufReader::new(rx).lines();

            while let Ok(Some(line)) = reader.next_line().await {
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
                        if let Ok(res_json) = serde_json::to_string(&err_res) {
                            let _ = tx.write_all((res_json + "\n").as_bytes()).await;
                            let _ = tx.flush().await;
                        }
                        continue;
                    }
                };

                let res = mcp::handle_request(&root, &req).await;
                if let Some(response) = res {
                    if let Ok(res_json) = serde_json::to_string(&response) {
                        if let Err(e) = tx.write_all((res_json + "\n").as_bytes()).await {
                            eprintln!("[SERVER] Write error to peer {}: {}", peer, e);
                            break;
                        }
                        let _ = tx.flush().await;
                    }
                }
            }
        });
    }
}
