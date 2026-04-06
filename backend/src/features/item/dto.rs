use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::features::item::domain::{
    item::{CreateItem, UpdateItem},
    tag::TagInput,
};

/// Request body for creating an item.
/// Scalar fields come from the macro-generated `CreateItem`;
/// `tags` is handled separately since it uses a different input type.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateItemRequest {
    #[serde(flatten)]
    pub item: CreateItem,
    #[serde(default)]
    pub tags: Vec<TagInput>,
}

/// Request body for updating an item.
/// Scalar fields come from the macro-generated `UpdateItem` (all optional);
/// `tags` is `None` to leave tags untouched, or `Some(vec)` to replace all tags.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UpdateItemRequest {
    #[serde(flatten)]
    pub item: UpdateItem,
    pub tags: Option<Vec<TagInput>>,
}