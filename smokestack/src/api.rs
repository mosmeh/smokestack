use crate::model::{Component, Operation, OperationState, Tag};
use http::Uri;
use serde::{de, Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug)]
pub enum ApiResponse<T> {
    Ok(T),
    Err(String),
}

impl ApiResponse<()> {
    pub fn err<E: ToString>(e: E) -> Self {
        Self::Err(e.to_string())
    }
}

impl<T> From<ApiResponse<T>> for Result<T, String> {
    fn from(val: ApiResponse<T>) -> Self {
        match val {
            ApiResponse::Ok(v) => Ok(v),
            ApiResponse::Err(e) => Err(e),
        }
    }
}

impl<T> Serialize for ApiResponse<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let map = match self {
            Self::Ok(v) => {
                let mut map = serde_json::to_value(v)
                    .map_err(serde::ser::Error::custom)?
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                map.insert("ok".to_owned(), true.into());
                map
            }
            Self::Err(e) => serde_json::Map::from_iter([
                ("ok".to_owned(), false.into()),
                ("error".to_owned(), e.clone().into()),
            ]),
        };
        map.serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for ApiResponse<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut map = serde_json::Map::deserialize(deserializer)?;
        let ok: bool = map
            .remove("ok")
            .ok_or_else(|| de::Error::missing_field("ok"))
            .map(Deserialize::deserialize)?
            .map_err(de::Error::custom)?;
        if !ok {
            let error = map
                .get("error")
                .ok_or_else(|| de::Error::missing_field("error"))
                .map(Deserialize::deserialize)?
                .map_err(de::Error::custom)?;
            return Ok(Self::Err(error));
        }
        let value = if map.is_empty() {
            // Corresponds to unit type
            serde_json::Value::Null
        } else {
            serde_json::Value::Object(map)
        };
        T::deserialize(value)
            .map(Self::Ok)
            .map_err(de::Error::custom)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthRequest {
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateOperationRequest {
    pub title: String,
    pub purpose: String,

    #[serde(with = "crate::serde_uri")]
    pub url: Uri,

    pub components: Vec<String>,

    #[serde(default)]
    pub locks: Vec<String>,

    #[serde(default)]
    pub tags: Vec<String>,

    #[serde(default)]
    pub depends_on: Vec<u64>,

    #[serde(default)]
    pub operators: Vec<String>,

    #[serde(default)]
    pub annotations: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListOperationsQuery {
    #[serde(alias = "component", default)]
    pub components: Vec<String>,

    #[serde(alias = "tag", default)]
    pub tags: Vec<String>,

    #[serde(alias = "operator", default)]
    pub operators: Vec<String>,

    #[serde(alias = "status", default)]
    pub statuses: Vec<OperationState>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListOperationsResponse {
    pub operations: Vec<Operation>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct UpdateOperationRequest {
    pub title: Option<String>,
    pub purpose: Option<String>,

    #[serde(with = "crate::serde_uri_option")]
    pub url: Option<Uri>,

    pub components: Option<Vec<String>>,
    pub locks: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub depends_on: Option<Vec<u64>>,
    pub operators: Option<Vec<String>>,
    pub status: Option<OperationState>,

    #[serde(default)]
    pub annotations: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateComponentRequest {
    pub name: String,
    pub description: String,
    pub owners: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListComponentsResponse {
    pub components: Vec<Component>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListTagsResponse {
    pub tags: Vec<Tag>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSubscriptionRequest {
    pub operation: Option<u64>,
    pub component: Option<String>,
    pub tag: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ListSubscriptionResponse {
    pub operations: Vec<u64>,
    pub components: Vec<String>,
    pub tags: Vec<String>,
}
