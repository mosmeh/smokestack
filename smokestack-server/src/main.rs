mod api;

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, Response, StatusCode},
    response::IntoResponse,
    Json, RequestPartsExt, Router,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use smokestack::{
    api::ApiResponse,
    model::{Claims, Component, Operation, OperationState, SubscriptionSet, Tag, User},
};
use std::{
    collections::{BTreeMap, HashMap},
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, RwLock},
};
use tokio::{net::TcpListener, sync::broadcast};
use tower_http::trace::TraceLayer;

#[derive(Debug, Parser)]
#[clap(version)]
struct Cli {
    #[arg(short, long, default_value = "0.0.0.0:3000")]
    addr: SocketAddr,

    #[arg(short, long, default_value = "state.json")]
    state_file: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let cli = Cli::parse();
    let database = if let Ok(serialized) = std::fs::read(&cli.state_file) {
        tracing::info!("loading state from {}", cli.state_file.display());
        serde_json::from_slice(&serialized)?
    } else {
        Database::default()
    };
    let (operation_tx, _) = broadcast::channel(1024);
    let mut state = AppState {
        database,
        locks: LockTable::default(),
        operation_tx,
    };
    for operation in state.database.operations.values() {
        if !matches!(
            operation.status,
            OperationState::InProgress | OperationState::Paused
        ) {
            continue;
        }
        for lock in &operation.locks {
            state.locks.lock(lock, ComponentLock::Exclusive).unwrap();
        }
        for component in &operation.components {
            if operation.locks.contains(component) {
                state.locks.lock(component, ComponentLock::Shared).unwrap();
            }
        }
    }
    let state = SharedState(Arc::new(RwLock::new(state)));

    // We don't care about losing some data in PoC.
    tokio::spawn({
        let state = state.clone();
        async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                let serialized = {
                    let state = state.read().unwrap();
                    let db = &state.database;
                    tracing::debug!(
                        "saving state: users={}, operations={}, components={}, tags={}",
                        db.users.len(),
                        db.operations.len(),
                        db.components.len(),
                        db.tags.len(),
                    );
                    serde_json::to_string(&db).unwrap()
                };
                std::fs::write(&cli.state_file, serialized).unwrap();
            }
        }
    });

    let routes =
        Router::new()
            .nest("/api/v1", api::root())
            .layer(TraceLayer::new_for_http().make_span_with(
                tower_http::trace::DefaultMakeSpan::default().include_headers(true),
            ))
            .with_state(state);

    let listener = TcpListener::bind(cli.addr).await?;
    tracing::info!("listening on {}", listener.local_addr()?);
    axum::serve(listener, routes).await?;
    Ok(())
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("no token provided")]
    MissingToken,

    #[error("invalid token")]
    InvalidToken,

    #[error("{} {} already exists", .entity, .id)]
    AlreadyExists { entity: &'static str, id: String },

    #[error("{} {} not found", .entity, .id)]
    NotFound { entity: &'static str, id: String },

    #[error("at least one {0} is required")]
    MissingItem(&'static str),

    #[error("{0} cannot be blank")]
    BlankItem(&'static str),

    #[error("url should have http or https scheme")]
    InvalidUrlScheme,

    #[error("locked component must be one of the affected components")]
    LockingNonAffectedComponent,

    #[error("failed to acquire lock on component {0}")]
    LockFailed(String),

    #[error("Dependent operations must be completed before starting this operation")]
    UnmetDependency,

    #[error("invalid state transition")]
    InvalidStateTransition,

    #[error("exactly one of operation, component, or tag must be specified")]
    SubscribingMultipleEntities,

    #[error("internal error")]
    Internal,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response<axum::body::Body> {
        let status = match self {
            Self::MissingToken => StatusCode::UNAUTHORIZED,
            Self::InvalidToken
            | Self::AlreadyExists { .. }
            | Self::MissingItem(_)
            | Self::BlankItem(_)
            | Self::InvalidUrlScheme
            | Self::LockingNonAffectedComponent
            | Self::InvalidStateTransition
            | Self::SubscribingMultipleEntities => StatusCode::BAD_REQUEST,
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::UnmetDependency => StatusCode::FAILED_DEPENDENCY,
            Self::LockFailed(_) => StatusCode::LOCKED,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(ApiResponse::err(self))).into_response()
    }
}

#[derive(Clone)]
struct SharedState(Arc<RwLock<AppState>>);

impl std::ops::Deref for SharedState {
    type Target = Arc<RwLock<AppState>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComponentLock {
    /// The component is locked for shared access.
    ///
    /// When operations target the component, they must acquire a shared lock.
    /// Multiple operations can acquire a shared lock at the same time.
    Shared,

    /// The component is locked for exclusive access.
    ///
    /// When operations specify the component in their "locks" field,
    /// they acquire an exclusive lock.
    /// Only one operation can acquire an exclusive lock at a time.
    Exclusive,
}

#[derive(Default)]
struct LockTable(HashMap<String, ComponentLock>);

impl LockTable {
    fn lock(&mut self, component: &str, lock: ComponentLock) -> Result<()> {
        match self.0.entry(component.to_string()) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(lock);
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if lock == ComponentLock::Exclusive || *entry.get() == ComponentLock::Exclusive {
                    return Err(Error::LockFailed(component.to_string()));
                }
                *entry.get_mut() = lock;
            }
        }
        Ok(())
    }

    #[allow(unused)]
    fn unlock(&mut self, component: &str) {
        self.0.remove(component).unwrap();
    }
}

struct AppState {
    database: Database,
    locks: LockTable,
    operation_tx: broadcast::Sender<Operation>,
}

impl AppState {
    fn next_id(&mut self) -> u64 {
        let id = self.database.next_id;
        self.database.next_id += 1;
        id
    }

    fn user(&self, username: &str) -> Result<&User> {
        self.database.users.get(username).ok_or(Error::NotFound {
            entity: "user",
            id: username.to_string(),
        })
    }

    fn user_mut(&mut self, username: &str) -> Result<&mut User> {
        self.database
            .users
            .get_mut(username)
            .ok_or(Error::NotFound {
                entity: "user",
                id: username.to_string(),
            })
    }

    fn create_user(&mut self, username: String) -> Result<User> {
        let user = User {
            name: username.clone(),
            subscriptions: SubscriptionSet::default(),
        };
        match self.database.users.entry(username) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(user.clone());
                Ok(user)
            }
            std::collections::hash_map::Entry::Occupied(_) => Err(Error::AlreadyExists {
                entity: "user",
                id: user.name,
            }),
        }
    }

    fn operation(&self, id: u64) -> Result<&Operation> {
        self.database.operations.get(&id).ok_or(Error::NotFound {
            entity: "operation",
            id: id.to_string(),
        })
    }

    fn operations(&self) -> impl Iterator<Item = &Operation> {
        self.database.operations.values()
    }

    fn upsert_operation(&mut self, mut operation: Operation) -> Result<Operation> {
        operation.title = operation.title.trim().to_string();
        if operation.title.is_empty() {
            return Err(Error::BlankItem("title"));
        }

        operation.purpose = operation.purpose.trim().to_string();
        if operation.purpose.is_empty() {
            return Err(Error::BlankItem("purpose"));
        }

        if operation
            .url
            .scheme_str()
            .map_or(true, |scheme| !matches!(scheme, "http" | "https"))
        {
            return Err(Error::InvalidUrlScheme);
        }

        if operation.components.is_empty() {
            return Err(Error::MissingItem("component"));
        }
        for component in &mut operation.components {
            *component = component.trim().to_string();
        }
        operation.components.sort_unstable();
        operation.components.dedup();
        for component in &operation.components {
            self.component(component)?;
        }

        for lock in &mut operation.locks {
            *lock = lock.trim().to_string();
        }
        operation.locks.sort_unstable();
        operation.locks.dedup();
        for lock in &operation.locks {
            if !operation.components.contains(lock) {
                return Err(Error::LockingNonAffectedComponent);
            }
        }

        for tag in &mut operation.tags {
            *tag = tag.trim().to_string();
        }
        operation.tags.sort_unstable();
        operation.tags.dedup();
        for tag in &operation.tags {
            self.tag(tag)?;
        }

        operation.depends_on.sort_unstable();
        operation.depends_on.dedup();
        for depends_on in &operation.depends_on {
            self.operation(*depends_on)?;
        }

        if operation.operators.is_empty() {
            return Err(Error::MissingItem("operator"));
        }
        for operator in &mut operation.operators {
            *operator = operator.trim().to_string();
        }
        operation.operators.sort_unstable();
        operation.operators.dedup();
        for operator in &operation.operators {
            self.user(operator)?;
        }

        match self.operation(operation.id) {
            Ok(current) => {
                if !current.status.can_transition_to(operation.status) {
                    return Err(Error::InvalidStateTransition);
                }
                if operation.status == OperationState::InProgress {
                    for depends_on in &operation.depends_on {
                        if self.operation(*depends_on)?.status != OperationState::Completed {
                            return Err(Error::UnmetDependency);
                        }
                    }
                }
                // TODO: lock/unlock components
            }
            Err(Error::NotFound { .. }) => assert_eq!(operation.status, OperationState::Planned),
            Err(e) => return Err(e),
        }

        let prev = self
            .database
            .operations
            .insert(operation.id, operation.clone());
        if prev.map_or(true, |prev| prev != operation) {
            if let Err(e) = self.operation_tx.send(operation.clone()) {
                tracing::warn!("failed to broadcast operation: {}", e);
            }
        }
        Ok(operation)
    }

    fn component(&self, name: &str) -> Result<&Component> {
        self.database.components.get(name).ok_or(Error::NotFound {
            entity: "component",
            id: name.to_string(),
        })
    }

    fn components(&self) -> impl Iterator<Item = &Component> {
        self.database.components.values()
    }

    fn create_component(&mut self, mut component: Component) -> Result<Component> {
        component.name = component.name.trim().to_string();
        if component.name.is_empty() {
            return Err(Error::BlankItem("name"));
        }

        component.description = component.description.trim().to_string();
        if component.description.is_empty() {
            return Err(Error::BlankItem("description"));
        }

        if component.owners.is_empty() {
            return Err(Error::MissingItem("owner"));
        }
        for owner in &mut component.owners {
            *owner = owner.trim().to_string();
        }
        component.owners.sort_unstable();
        component.owners.dedup();
        for owner in &component.owners {
            self.user(owner)?;
        }

        match self.database.components.entry(component.name.clone()) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(component.clone());
                Ok(component)
            }
            std::collections::hash_map::Entry::Occupied(_) => Err(Error::AlreadyExists {
                entity: "component",
                id: component.name,
            }),
        }
    }

    fn tag(&self, name: &str) -> Result<&Tag> {
        self.database.tags.get(name).ok_or(Error::NotFound {
            entity: "tag",
            id: name.to_string(),
        })
    }

    fn tags(&self) -> impl Iterator<Item = &Tag> {
        self.database.tags.values()
    }

    fn create_tag(&mut self, mut tag: Tag) -> Result<Tag> {
        tag.name = tag.name.trim().to_string();
        if tag.name.is_empty() {
            return Err(Error::BlankItem("name"));
        }

        tag.description = tag.description.trim().to_string();
        if tag.description.is_empty() {
            return Err(Error::BlankItem("description"));
        }

        match self.database.tags.entry(tag.name.clone()) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(tag.clone());
                Ok(tag)
            }
            std::collections::hash_map::Entry::Occupied(_) => Err(Error::AlreadyExists {
                entity: "tag",
                id: tag.name,
            }),
        }
    }

    fn subscribe(
        &mut self,
        username: &str,
        operation: Option<u64>,
        component: Option<String>,
        tag: Option<String>,
    ) -> Result<()> {
        let num_specified = usize::from(operation.is_some())
            + usize::from(component.is_some())
            + usize::from(tag.is_some());
        if num_specified != 1 {
            return Err(Error::SubscribingMultipleEntities);
        }
        if let Some(operation) = operation {
            self.operation(operation)?;
        }
        if let Some(component) = &component {
            self.component(component)?;
        }
        if let Some(tag) = &tag {
            self.tag(tag)?;
        }
        let subscriptions = &mut self.user_mut(username)?.subscriptions;
        if let Some(operation) = operation {
            subscriptions.operations.insert(operation);
        }
        if let Some(component) = component {
            subscriptions.components.insert(component);
        }
        if let Some(tag) = tag {
            subscriptions.tags.insert(tag);
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct Database {
    next_id: u64,
    users: HashMap<String, User>,
    operations: BTreeMap<u64, Operation>,
    components: HashMap<String, Component>,
    tags: HashMap<String, Tag>,
}

impl Default for Database {
    fn default() -> Self {
        Self {
            next_id: 1234,
            users: HashMap::new(),
            operations: BTreeMap::new(),
            components: HashMap::new(),
            tags: HashMap::new(),
        }
    }
}

const JWT_SECRET: &[u8] = b"secret"; // hardcoded secret for PoC

#[async_trait]
impl FromRequestParts<SharedState> for Claims {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &SharedState) -> Result<Self> {
        let TypedHeader(Authorization(bearer)) =
            match parts.extract::<TypedHeader<Authorization<Bearer>>>().await {
                Ok(a) => a,
                Err(e) if e.is_missing() => return Err(Error::MissingToken),
                Err(_) => return Err(Error::InvalidToken),
            };
        let token_data = jsonwebtoken::decode(
            bearer.token(),
            &jsonwebtoken::DecodingKey::from_secret(JWT_SECRET),
            &jsonwebtoken::Validation::default(),
        )
        .map_err(|_| Error::InvalidToken)?;
        let claims: Self = token_data.claims;
        state.read().unwrap().user(&claims.username)?;
        Ok(claims)
    }
}
