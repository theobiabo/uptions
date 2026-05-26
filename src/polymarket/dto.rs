use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, Serialize, Default, IntoParams, ToSchema)]
pub struct MarketsQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    pub archived: Option<bool>,
    pub slug: Option<String>,
    pub id: Option<String>,
}
