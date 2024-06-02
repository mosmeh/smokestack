use http::Uri;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub subscriptions: SubscriptionSet,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub exp: u64,
    pub username: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    pub id: u64,
    pub title: String,
    pub purpose: String,

    #[serde(with = "crate::serde_uri")]
    pub url: Uri,

    pub components: Vec<String>,
    pub locks: Vec<String>,
    pub tags: Vec<String>,
    pub depends_on: Vec<u64>,
    pub operators: Vec<String>,
    pub status: OperationState,
    pub annotations: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    /// The operation is planned but not started yet.
    Planned,

    /// The operation is in progress.
    InProgress,

    /// The operation is paused.
    Paused,

    /// The operation was finished successfully.
    Completed,

    /// The operation was finished unsuccessfully.
    Aborted,

    /// The operation was canceled before starting.
    Canceled,
}

impl FromStr for OperationState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "planned" => Ok(Self::Planned),
            "in_progress" => Ok(Self::InProgress),
            "paused" => Ok(Self::Paused),
            "completed" => Ok(Self::Completed),
            "aborted" => Ok(Self::Aborted),
            "canceled" => Ok(Self::Canceled),
            _ => Err(format!("unknown operation state: {s}")),
        }
    }
}

impl std::fmt::Display for OperationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Planned => "planned",
            Self::InProgress => "in_progress",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Aborted => "aborted",
            Self::Canceled => "canceled",
        }
        .fmt(f)
    }
}

impl OperationState {
    pub fn can_transition_to(self, new: Self) -> bool {
        if self == new {
            return true;
        }
        matches!(
            (self, new),
            (Self::Planned | Self::Paused, Self::InProgress)
                | (
                    Self::InProgress,
                    Self::Paused | Self::Completed | Self::Aborted
                )
                | (Self::Planned, Self::Canceled)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub name: String,
    pub description: String,
    pub owners: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubscriptionSet {
    pub operations: HashSet<u64>,
    pub components: HashSet<String>,
    pub tags: HashSet<String>,
}

impl SubscriptionSet {
    pub fn is_match(&self, operation: &Operation) -> bool {
        self.operations.contains(&operation.id)
            || operation
                .components
                .iter()
                .any(|c| self.components.contains(c))
            || operation.tags.iter().any(|t| self.tags.contains(t))
    }
}
