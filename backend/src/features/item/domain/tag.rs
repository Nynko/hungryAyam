use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;
use hungry_ayam_derive::domain_struct;

use crate::types::name::Name;

/// Tag domain struct - represents a tag that can be attached to items
/// Tags are used for categorization (e.g., vegetarian, spicy, allergens, etc.)
#[domain_struct(update)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Tag {
    #[update_required]
    pub id: Uuid,
    pub name: Name,
}

/// TagInput - used in Create/Update item requests
/// Either reference an existing tag by ID, or create/find by name
/// If both are provided, ID takes precedence
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TagInput {
    /// If provided, use existing tag with this ID
    pub id: Option<Uuid>,
    /// If provided (and no id), create or find tag by name
    pub name: Option<Name>,
}

impl TagInput {
    /// Check if this TagInput is valid (has at least id or name)
    pub fn is_valid(&self) -> bool {
        self.id.is_some() || self.name.is_some()
    }
}
