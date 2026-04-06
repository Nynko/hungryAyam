use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;
use hungry_ayam_derive::domain_struct;

use crate::types::name::Name;

/// Tag domain struct - represents a tag that can be attached to items.
/// Tags are used for categorization (e.g., vegetarian, spicy, allergens, etc.)
#[domain_struct(update(all_optional))]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Tag {
    #[update_required]
    pub id: Uuid,
    pub name: Name,
}

/// Input type for specifying tags on create/update item requests.
/// Either reference an existing tag by ID, or create/find one by name.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum TagInput {
    /// Reference an existing tag by its ID.
    Existing(Uuid),
    /// Create or find a tag by name (upsert by name).
    New(Name),
}