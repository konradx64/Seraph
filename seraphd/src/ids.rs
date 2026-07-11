use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RouteId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TunnelId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub Uuid);

impl RouteId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl TunnelId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl AgentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}