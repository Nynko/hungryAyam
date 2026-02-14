use chrono::{DateTime, Utc};
use hungry_ayam_derive::domain_struct;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::types::name::Name;
use crate::features::menu::domain::section::CreateMenuSection;
use super::section::MenuSection;


/// Menu domain struct - represents a menu belonging to a restaurant
/// A restaurant can have multiple menus (e.g., Lunch Menu, Dinner Menu)
#[domain_struct(create, update(all_optional), unit_create)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Menu {
    #[create_ignore]
    #[update_required]
    #[unit_create_ignore]
    pub id: Uuid,
    #[update_ignore]
    pub restaurant_id: Uuid,
    pub name: Name,
    pub description: Option<String>,
    pub is_active: bool,
    /// Permanent menus keep their item associations.
    /// Non-permanent (rotating) menus can be "reset" - items become unavailable
    /// but stay in the candidate pool for easy re-selection.
    pub permanent: bool,
    // #[derived_domain_ignore]
    // pub created_at: DateTime<Utc>,
    #[derived_domain_ignore]
    pub updated_at: DateTime<Utc>,
    pub created_by: Uuid,
    #[create_ignore]
    #[unit_create_ignore]
    #[update_required]
    pub updated_by: Uuid,

    #[serde(default)]
    #[derived_nested]
    #[update_ignore] // We only want to update the menu itself for updates otherwise we update each parts with actions
    #[unit_create_ignore]
    pub sections: Vec<MenuSection>,
}
