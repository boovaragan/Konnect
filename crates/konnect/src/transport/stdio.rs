use anyhow::Result;
use konnect_core::mcp::{handler::McpHandler, protocol::*};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info};

/// Run the MCP server over STDIO (stdin/stdout).
/// All logging must go to stderr — stdout is reserved for the MCP protocol.
pub async fn run_stdio(handler: McpHandler) -> Result<()> {
    info!("Starting STDIO transport");

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut stdout = stdout;
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            // EOF — client disconnected
            info!("STDIO: EOF received, shutting down");
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        debug!("STDIO recv: {}", trimmed);

        let response = match serde_json::from_str::<Value>(trimmed) {
            Ok(msg) => handler.handle_message(msg).await,
            Err(e) => {
                error!("Failed to parse JSON: {}", e);
                Some(JsonRpcResponse::error(
                    Value::Null,
                    JsonRpcError {
                        code: PARSE_ERROR,
                        message: format!("Parse error: {}", e),
                        data: None,
                    },
                ))
            }
        };

        if let Some(resp) = response {
            let mut json = serde_json::to_string(&resp)?;
            json.push('\n');
            debug!("STDIO send: {}", json.trim());
            stdout.write_all(json.as_bytes()).await?;
            stdout.flush().await?;
        }
    }

    Ok(())
}
