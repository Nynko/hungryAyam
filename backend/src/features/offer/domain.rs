use chrono::{DateTime, Utc};
use hungry_ayam_derive::domain_struct;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::types::{name::Name, price::PriceCents};

// ==================== SlotConstraintKind ====================

/// The kind of constraint on an offer slot.
///
/// Each constraint allows items matching a single criterion:
/// - `Item`: only this specific item is allowed
/// - `Tag`: any item with this tag is allowed
/// - `Section`: any available item in this menu section is allowed
///
/// Maps to the DB's XOR check constraint on
/// `(allowed_item_id, allowed_tag_id, allowed_section_id)`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum SlotConstraintKind {
    /// Only this specific item is allowed in the slot.
    Item(Uuid),
    /// Any item carrying this tag is allowed in the slot.
    Tag(Uuid),
    /// Any available item belonging to this menu section is allowed.
    Section(Uuid),
}

impl SlotConstraintKind {
    /// Extract the allowed_item_id (if Item variant).
    pub fn item_id(&self) -> Option<Uuid> {
        match self {
            Self::Item(id) => Some(*id),
            _ => None,
        }
    }

    /// Extract the allowed_tag_id (if Tag variant).
    pub fn tag_id(&self) -> Option<Uuid> {
        match self {
            Self::Tag(id) => Some(*id),
            _ => None,
        }
    }

    /// Extract the allowed_section_id (if Section variant).
    pub fn section_id(&self) -> Option<Uuid> {
        match self {
            Self::Section(id) => Some(*id),
            _ => None,
        }
    }

    /// Build a `SlotConstraintKind` from the three nullable DB columns.
    /// Exactly one must be `Some` (enforced by the DB check constraint).
    pub fn from_db(
        item_id: Option<Uuid>,
        tag_id: Option<Uuid>,
        section_id: Option<Uuid>,
    ) -> anyhow::Result<Self> {
        match (item_id, tag_id, section_id) {
            (Some(id), None, None) => Ok(Self::Item(id)),
            (None, Some(id), None) => Ok(Self::Tag(id)),
            (None, None, Some(id)) => Ok(Self::Section(id)),
            _ => anyhow::bail!(
                "Exactly one of allowed_item_id, allowed_tag_id, allowed_section_id must be set"
            ),
        }
    }
}

// ==================== OfferSlotConstraint Domain ====================

/// A constraint on which items are allowed in an offer slot.
///
/// Each constraint row specifies one allowed source (item, tag, or section).
/// A slot's effective allowed-item set is the *union* of all its constraints.
#[domain_struct(create, update(all_optional))]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OfferSlotConstraint {
    #[create_ignore]
    #[update_ignore]
    pub id: Uuid,
    /// Assigned server-side from the parent slot.
    #[create_ignore]
    #[update_ignore]
    pub slot_id: Uuid,
    /// What kind of constraint this is and the referenced entity.
    /// Required on both create and update (not truly optional on update
    /// since constraints use replace-all semantics).
    #[update_required]
    pub kind: SlotConstraintKind,
}

// ==================== OfferSlot Domain ====================

/// A slot within an offer (e.g. "Pick your starter", "Pick your main").
///
/// `min_items` / `max_items` control how many items the user must/can pick
/// for this slot. Constraints define which items are eligible.
#[domain_struct(create, update(all_optional))]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OfferSlot {
    /// Generated server-side. Ignored on both create and update (replace-all semantics).
    #[create_ignore]
    #[update_ignore]
    pub id: Uuid,
    /// Assigned server-side from the parent offer.
    #[create_ignore]
    #[update_ignore]
    pub offer_id: Uuid,
    pub label: Name,
    pub min_items: i32,
    pub max_items: i32,

    /// Constraints for this slot (populated when loading a full offer).
    /// On create, nested `CreateOfferSlotConstraint` items are expected.
    /// On update, if provided the full set replaces existing constraints (replace-all semantics).
    #[serde(default)]
    #[derived_nested]
    pub constraints: Vec<OfferSlotConstraint>,
}

// ==================== Offer Domain ====================

/// An offer / deal belonging to a restaurant (e.g. "Menu du Jour" at a fixed price).
///
/// An offer is optionally linked to a specific menu via `menu_id` — useful for
/// temporary menus like "menu du jour" where the menu's available items change
/// daily but the offer structure (slots + constraints) stays the same.
///
/// The `is_active` flag controls whether the offer is currently orderable.
#[domain_struct(create, update(all_optional))]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Offer {
    #[create_ignore]
    #[update_required]
    pub id: Uuid,
    #[update_ignore]
    pub restaurant_id: Uuid,
    /// Optional link to a specific menu (e.g. a temporary "menu du jour" menu).
    pub menu_id: Option<Uuid>,
    pub title: Name,
    pub description: Option<String>,
    pub fixed_price_cents: PriceCents,
    /// Whether this offer is currently available for ordering.
    pub is_active: bool,
    #[derived_domain_ignore]
    pub created_at: DateTime<Utc>,
    #[derived_domain_ignore]
    pub created_by: Uuid,

    /// Slots composing this offer (populated when loading a full offer).
    /// On create, nested `CreateOfferSlot` items are expected.
    /// On update, if provided the full set replaces existing slots (replace-all semantics).
    #[serde(default)]
    #[derived_nested]
    pub slots: Vec<OfferSlot>,
}