//! Localized AI Integration Bridge serving GET /v1/context with bearer auth.

use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{error, info};
use uuid::Uuid;
use std::sync::Arc;
use crate::session;

/// Start the background loopback server (127.0.0.1:8089) serving authenticated prompt context.
pub async fn start_bridge(project_root: &Path) -> Result<()> {
    let bridge_token = Uuid::new_v4().to_string();
    let neuron_dir = project_root.join(".neuron");
    tokio::fs::create_dir_all(&neuron_dir).await?;

    let token_path = neuron_dir.join("bridge_token");
    tokio::fs::write(&token_path, &bridge_token).await?;

    info!("Bridge token written to: {}", token_path.display());
    println!("  {} Bridge Token: {}", "🔑".green(), bridge_token.bright_yellow());
    println!("  {} Integration Bridge: http://127.0.0.1:8089/v1/context", "🌐".green());

    let listener = TcpListener::bind("127.0.0.1:8089").await
        .context("Failed to bind loopback address for the AI Integration Bridge")?;
    let project_root = project_root.to_path_buf();
    let token_shared = Arc::new(bridge_token);

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut socket, _)) => {
                    let project_root = project_root.clone();
                    let token = token_shared.clone();
                    tokio::spawn(async move {
                        let mut buf = [0; 4096];
                        let mut request = String::new();

                        match socket.read(&mut buf).await {
                            Ok(n) if n > 0 => {
                                request.push_str(&String::from_utf8_lossy(&buf[..n]));

                                // Parse Authorization: Bearer <token>
                                let mut authenticated = false;
                                for line in request.lines() {
                                    if line.to_lowercase().starts_with("authorization:") {
                                        if let Some(token_part) = line.split_whitespace().last() {
                                            if token_part == *token {
                                                authenticated = true;
                                            }
                                        }
                                    }
                                }

                                if !authenticated {
                                    let response = "HTTP/1.1 401 Unauthorized\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nUnauthorized: Invalid or missing Bearer token.\n";
                                    let _ = socket.write_all(response.as_bytes()).await;
                                    return;
                                }

                                // Basic routing
                                let is_get_context = request.starts_with("GET /v1/context");
                                if is_get_context {
                                    let manifest_res = crate::manifest::NeuronManifest::load(&project_root).await;
                                    match manifest_res {
                                        Ok(manifest) => {
                                            match session::regenerate_session_context(&project_root, &manifest).await {
                                                Ok(context) => {
                                                    let response_body = format!(
                                                        "<!-- NEURON_CONTEXT_START -->\n{}\n<!-- NEURON_CONTEXT_END -->",
                                                        context
                                                    );
                                                    let response = format!(
                                                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                                        response_body.len(),
                                                        response_body
                                                    );
                                                    let _ = socket.write_all(response.as_bytes()).await;
                                                }
                                                Err(e) => {
                                                    let err_msg = format!("Internal Server Error: {}\n", e);
                                                    let response = format!(
                                                        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                                        err_msg.len(),
                                                        err_msg
                                                    );
                                                    let _ = socket.write_all(response.as_bytes()).await;
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            let err_msg = format!("Failed to load manifest: {}\n", e);
                                            let response = format!(
                                                "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                                err_msg.len(),
                                                err_msg
                                            );
                                            let _ = socket.write_all(response.as_bytes()).await;
                                        }
                                    }
                                } else {
                                    let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nNot Found";
                                    let _ = socket.write_all(response.as_bytes()).await;
                                }
                            }
                            _ => {}
                        }
                    });
                }
                Err(e) => {
                    error!("TCP accept error in bridge: {}", e);
                }
            }
        }
    });

    Ok(())
}
