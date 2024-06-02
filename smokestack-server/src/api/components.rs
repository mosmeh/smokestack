use crate::{Result, SharedState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use smokestack::{
    api::{ApiResponse, CreateComponentRequest, ListComponentsResponse},
    model::{Claims, Component},
};

pub fn root() -> Router<SharedState> {
    Router::new()
        .route("/", post(create_component))
        .route("/", get(list_components))
        .route("/:name", get(get_component))
}

async fn list_components(_claims: Claims, State(state): State<SharedState>) -> impl IntoResponse {
    let state = state.read().unwrap();
    Json(ApiResponse::Ok(ListComponentsResponse {
        components: state.components().cloned().collect::<Vec<_>>(),
    }))
}

async fn create_component(
    _claims: Claims,
    State(state): State<SharedState>,
    Json(req): Json<CreateComponentRequest>,
) -> Result<(StatusCode, Json<ApiResponse<Component>>)> {
    let mut state = state.write().unwrap();
    let component = Component {
        name: req.name,
        description: req.description,
        owners: req.owners,
    };
    let component = state.create_component(component)?;
    Ok((StatusCode::CREATED, Json(ApiResponse::Ok(component))))
}

async fn get_component(
    _claims: Claims,
    State(state): State<SharedState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<Component>>> {
    let state = state.read().unwrap();
    Ok(Json(ApiResponse::Ok(state.component(&name)?.clone())))
}
