//! MCP server state machine — tracks initialization and session state.

use super::protocol::*;
use crate::router::ToolRouter;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq)]
pub enum ServerState {
    /// Waiting for the client's `initialize` request.
    Uninitialized,
    /// `initialize` received, `initialized` notification not yet sent.
    Initializing,
    /// Fully initialized; ready to handle requests.
    Ready,
}

pub struct McpServerState {
    pub state: RwLock<ServerState>,
    pub router: Arc<ToolRouter>,
}

impl McpServerState {
    pub fn new(router: Arc<ToolRouter>) -> Self {
        McpServerState {
            state: RwLock::new(ServerState::Uninitialized),
            router,
        }
    }

    pub fn build_initialize_result() -> InitializeResult {
        InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(true),
                }),
                ..Default::default()
            },
            server_info: ServerInfo {
                name: "konnect".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        }
    }

    pub async fn is_ready(&self) -> bool {
        *self.state.read().await == ServerState::Ready
    }
}
