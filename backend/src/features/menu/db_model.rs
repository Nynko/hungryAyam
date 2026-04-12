use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::types::{name::Name, price::PriceCents};

/// MenuRow - Database row representation of a menu
/// Maps directly to the `menus` table
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct MenuRow {
    pub id: Uuid,
    pub restaurant_id: Uuid,
    pub name: Name,
    pub description: Option<String>,
    pub is_active: bool,
    pub permanent: bool,
    pub position: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub availability_rule_id: Option<Uuid>,
}

/// MenuSectionRow - Database row representation of a menu section
/// Maps directly to the `menu_sections` table
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct MenuSectionRow {
    pub id: Uuid,
    pub menu_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: Name,
    pub description: Option<String>,
    pub position: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
}

/// MenuSectionItemRow - Database row representation of a menu section item link
/// Maps directly to the `menu_section_items` table (without the joined Item)
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct MenuSectionItemRow {
    pub id: Uuid,
    pub section_id: Uuid,
    pub item_id: Uuid,
    pub position: i32,
    pub price_override_cents: Option<PriceCents>,
    pub is_available: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
}