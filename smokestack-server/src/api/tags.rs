use crate::{Result, SharedState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use smokestack::{
    api::{ApiResponse, CreateTagRequest, ListTagsResponse},
    model::{Claims, Tag},
};

pub fn root() -> Router<SharedState> {
    Router::new()
        .route("/", post(create_tag))
        .route("/", get(list_tags))
        .route("/:name", get(get_tag))
}

async fn list_tags(_claims: Claims, State(state): State<SharedState>) -> impl IntoResponse {
    let state = state.read().unwrap();
    Json(ApiResponse::Ok(ListTagsResponse {
        tags: state.tags().cloned().collect::<Vec<_>>(),
    }))
}

async fn create_tag(
    _claims: Claims,
    State(state): State<SharedState>,
    Json(req): Json<CreateTagRequest>,
) -> Result<(StatusCode, Json<ApiResponse<Tag>>)> {
    let mut state = state.write().unwrap();
    let tag = Tag {
        name: req.name,
        description: req.description,
    };
    let tag = state.create_tag(tag)?;
    Ok((StatusCode::CREATED, Json(ApiResponse::Ok(tag))))
}

async fn get_tag(
    _claims: Claims,
    State(state): State<SharedState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<Tag>>> {
    let state = state.read().unwrap();
    Ok(Json(ApiResponse::Ok(state.tag(&name)?.clone())))
}
