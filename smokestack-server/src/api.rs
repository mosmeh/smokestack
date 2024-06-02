mod components;
mod operations;
mod subscriptions;
mod tags;

use crate::{Error, Result, SharedState};
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use smokestack::{
    api::{ApiResponse, AuthRequest, AuthResponse},
    model::Claims,
};
use std::time::{Duration, SystemTime};

pub fn root() -> Router<SharedState> {
    Router::new()
        .route("/auth", post(auth))
        .nest("/operations", operations::root())
        .nest("/components", components::root())
        .nest("/tags", tags::root())
        .nest("/subscriptions", subscriptions::root())
}

async fn auth(
    State(state): State<SharedState>,
    Json(req): Json<AuthRequest>,
) -> Result<(StatusCode, Json<ApiResponse<AuthResponse>>)> {
    state.write().unwrap().create_user(req.username.clone())?;
    let claims = Claims {
        exp: SystemTime::now()
            .checked_add(Duration::from_secs(60 * 60 * 24 * 365)) // FIXME: 1 year
            .unwrap()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        username: req.username,
    };
    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(crate::JWT_SECRET),
    )
    .map_or(Err(Error::Internal), |token| {
        Ok((
            StatusCode::CREATED,
            Json(ApiResponse::Ok(AuthResponse { token })),
        ))
    })
}
