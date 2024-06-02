use crate::Result;
use crate::SharedState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch, post},
    Json, Router,
};
use axum_extra::extract::Query;
use smokestack::{
    api::{
        ApiResponse, CreateOperationRequest, ListOperationsQuery, ListOperationsResponse,
        UpdateOperationRequest,
    },
    model::{Claims, Operation, OperationState},
};

pub fn root() -> Router<SharedState> {
    Router::new()
        .route("/", post(create_operation))
        .route("/", get(list_operations))
        .route("/:id", get(get_operation))
        .route("/:id", patch(update_operation))
}

async fn create_operation(
    claims: Claims,
    State(state): State<SharedState>,
    Json(mut req): Json<CreateOperationRequest>,
) -> Result<(StatusCode, Json<ApiResponse<Operation>>)> {
    let mut state = state.write().unwrap();
    if req.operators.is_empty() {
        req.operators.push(claims.username);
    }
    let id = state.next_id();
    let operation = Operation {
        id,
        title: req.title,
        purpose: req.purpose,
        url: req.url,
        components: req.components,
        locks: req.locks,
        tags: req.tags,
        depends_on: req.depends_on,
        operators: req.operators,
        status: OperationState::Planned,
        annotations: req.annotations,
    };
    let operation = state.upsert_operation(operation)?;
    Ok((StatusCode::CREATED, Json(ApiResponse::Ok(operation))))
}

async fn list_operations(
    _claims: Claims,
    State(state): State<SharedState>,
    Query(query): Query<ListOperationsQuery>,
) -> impl IntoResponse {
    let state = state.read().unwrap();
    let operations = state.operations().filter(|operation| {
        if !query.components.is_empty()
            && !operation
                .components
                .iter()
                .any(|component| query.components.contains(component))
        {
            return false;
        }
        if !query.tags.is_empty() && !operation.tags.iter().any(|tag| query.tags.contains(tag)) {
            return false;
        }
        if !query.operators.is_empty()
            && !operation
                .operators
                .iter()
                .any(|operator| query.operators.contains(operator))
        {
            return false;
        }
        if !query.statuses.is_empty() && !query.statuses.contains(&operation.status) {
            return false;
        }
        true
    });
    Json(ApiResponse::Ok(ListOperationsResponse {
        operations: operations.cloned().collect::<Vec<_>>(),
    }))
}

async fn get_operation(
    _claims: Claims,
    State(state): State<SharedState>,
    Path(id): Path<u64>,
) -> Result<Json<ApiResponse<Operation>>> {
    let state = state.read().unwrap();
    Ok(Json(ApiResponse::Ok(state.operation(id)?.clone())))
}

async fn update_operation(
    _claims: Claims,
    State(state): State<SharedState>,
    Path(id): Path<u64>,
    Json(req): Json<UpdateOperationRequest>,
) -> Result<Json<ApiResponse<Operation>>> {
    let mut state = state.write().unwrap();
    let mut operation = state.operation(id)?.clone();
    if let Some(title) = req.title {
        operation.title = title;
    }
    if let Some(purpose) = req.purpose {
        operation.purpose = purpose;
    }
    if let Some(url) = req.url {
        operation.url = url;
    }
    if let Some(components) = req.components {
        operation.components = components;
    }
    if let Some(locks) = req.locks {
        operation.locks = locks;
    }
    if let Some(tags) = req.tags {
        operation.tags = tags;
    }
    if let Some(depends_on) = req.depends_on {
        operation.depends_on = depends_on;
    }
    if let Some(operators) = req.operators {
        operation.operators = operators;
    }
    if let Some(status) = req.status {
        operation.status = status;
    }
    operation.annotations.extend(req.annotations);
    Ok(Json(ApiResponse::Ok(state.upsert_operation(operation)?)))
}
