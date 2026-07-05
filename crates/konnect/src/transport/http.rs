use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use konnect_core::mcp::handler::McpHandler;
use serde_json::Value;
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;
use tower_http::cors::CorsLayer;
use tracing::info;

#[derive(Clone)]
struct AppState {
    handler: Arc<McpHandler>,
}

/// Run the MCP server over HTTP with SSE for server-to-client streaming.
///
/// Endpoints:
///   POST /mcp       — client sends JSON-RPC request, gets JSON response
///   GET  /mcp/sse   — SSE stream for server-initiated messages (notifications)
pub async fn run_http(handler: McpHandler, addr: &str) -> Result<()> {
    let state = AppState {
        handler: Arc::new(handler),
    };

    let app = Router::new()
        .route("/mcp", post(handle_post))
        .route("/mcp/sse", get(handle_sse))
        .route("/health", get(handle_health))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("HTTP/SSE transport listening on http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_post(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    match state.handler.handle_message(body).await {
        Some(resp) => Json(resp).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

async fn handle_sse(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Event>(32);

    // Register SSE sender with handler so it can push notifications
    state.handler.register_sse_sender(tx).await;

    use tokio_stream::StreamExt;
    let stream = ReceiverStream::new(rx).map(Ok);
    Sse::new(stream)
}

async fn handle_health() -> &'static str {
    "ok"
}
