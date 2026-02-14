use hungry_ayam_derive::domain_struct;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::types::name::Name;

use super::section_item::{MenuSectionItem, CreateMenuSectionItem};


/// MenuSection domain struct - represents a section within a menu
/// Sections can be nested (parent_id references another section)
/// Sections contain items at the leaf level
#[domain_struct(create, update(all_optional), unit_create)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MenuSection {
    #[derived_domain_ignore] // Updates needs to pass by actions and will contains the id (potentially as a previous ref (RefEntity))
    pub id: Uuid,
    pub menu_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: Name,
    pub description: Option<String>,
    pub position: i32,
    pub is_active: bool,
    #[update_ignore]
    pub created_by: Uuid,
    #[create_ignore]
    #[unit_create_ignore]
    #[update_required]
    pub updated_by: Uuid,

    /// Items in this section
    #[serde(default)]
    #[unit_create_ignore]
    #[update_ignore]
    #[derived_nested]
    pub items: Vec<MenuSectionItem>,

    /// Nested subsections
    #[serde(default)]
    #[unit_create_ignore]
    #[update_ignore]
    #[derived_nested]
    pub subsections: Vec<MenuSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub enum DerivedMenuSection {
    CreateMenuSection(CreateMenuSection),
    UpdateMenuSection(UpdateMenuSection)
}
