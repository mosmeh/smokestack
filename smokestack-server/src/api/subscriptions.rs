use crate::{Result, SharedState};
use axum::{
    extract::{
        ws::{self, WebSocket},
        State, WebSocketUpgrade,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use smokestack::{
    api::{ApiResponse, CreateSubscriptionRequest, ListSubscriptionResponse},
    model::Claims,
};

pub fn root() -> Router<SharedState> {
    Router::new()
        .route("/", post(create_subscription))
        .route("/", get(list_subscriptions))
        .route("/watch", get(watch))
}

async fn create_subscription(
    claims: Claims,
    State(state): State<SharedState>,
    Json(req): Json<CreateSubscriptionRequest>,
) -> Result<(StatusCode, Json<ApiResponse<()>>)> {
    state
        .write()
        .unwrap()
        .subscribe(&claims.username, req.operation, req.component, req.tag)?;
    Ok((StatusCode::CREATED, Json(ApiResponse::Ok(()))))
}

async fn list_subscriptions(
    claims: Claims,
    State(state): State<SharedState>,
) -> Result<Json<ApiResponse<ListSubscriptionResponse>>> {
    let state = state.read().unwrap();
    let user = state.user(&claims.username)?;
    let subscriptions = &user.subscriptions;
    let mut response = ListSubscriptionResponse {
        operations: subscriptions.operations.iter().copied().collect(),
        components: subscriptions.components.iter().cloned().collect(),
        tags: subscriptions.tags.iter().cloned().collect(),
    };
    response.operations.sort_unstable();
    response.components.sort_unstable();
    response.tags.sort_unstable();
    Ok(Json(ApiResponse::Ok(response)))
}

async fn watch(
    claims: Claims,
    State(state): State<SharedState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(claims, state, socket))
}

async fn handle_socket(claims: Claims, state: SharedState, mut socket: WebSocket) {
    let (subscriptions, mut rx) = {
        let state = state.read().unwrap();
        let Ok(user) = state.user(&claims.username) else {
            return;
        };
        let subscriptions = user.subscriptions.clone();
        let rx = state.operation_tx.subscribe();
        (subscriptions, rx)
    };
    #[allow(clippy::redundant_pub_crate)]
    loop {
        tokio::select! {
            Ok(operation) = rx.recv() => {
                if !subscriptions.is_match(&operation) {
                    continue;
                }
                let msg = match serde_json::to_string(&operation) {
                    Ok(msg) => ws::Message::Text(msg),
                    Err(e) => {
                        tracing::warn!("failed to serialize operation: {}", e);
                        return;
                    }
                };
                if socket.send(msg).await.is_err() {
                    return;
                }
            }
            Some(_) = socket.recv() => (),
            else => return,
        }
    }
}
