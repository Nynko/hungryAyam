use serde::{Serialize, Deserialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::{
    features::{
        item::domain::tag::TagInput,
        menu::domain::{
            menu::UpdateMenu,
            section::{UnitCreateMenuSection, UpdateMenuSection},
            section_item::{UnitCreateMenuSectionItem, UpdateMenuSectionItem},
        },
    },
    types::{actions::EntityRef, position::Position},
};

/// Actions available when updating a menu. A user can either replace the full
/// menu (delete + recreate) or send a sequence of these granular actions:
///
/// 1. Update scalar fields — menu name, section name, or catalog item fields
/// 2. Add a section or item
/// 3. Reorder — change position of a section or item
/// 4. Reparent — move an item to a different section, or a subsection under another section
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum UpdateMenuAction {
    UpdateMenu(UpdateMenu),

    UpdateMenuSection {
        section_id: Uuid,
        update: UpdateMenuSection,
    },

    /// Update a menu section item (position, price override, availability) and
    /// optionally the underlying catalog item's scalar fields and/or tags.
    /// `item_tags` replaces all tags on the catalog item when provided.
    UpdateMenuSectionItem {
        item_id: Uuid,
        update: UpdateMenuSectionItem,
        #[serde(default)]
        item_tags: Option<Vec<TagInput>>,
    },

    /// Add a new section to the menu.
    /// `parent_id` is an `EntityRef` because the parent can be either the menu
    /// itself or another section (subsection nesting).
    AddSection {
        parent_id: EntityRef,
        section: UnitCreateMenuSection,
    },

    /// Add a new item to a section, creating the catalog item inline.
    /// `item_tags` optionally sets tags on the newly created catalog item.
    AddItem {
        section_id: EntityRef,
        item: UnitCreateMenuSectionItem,
        #[serde(default)]
        item_tags: Vec<TagInput>,
    },

    ChangePositionSection {
        section_id: EntityRef,
        position: Position,
    },

    ChangePositionItem {
        item_id: EntityRef,
        position: Position,
    },

    ChangeSectionForItem {
        item_id: EntityRef,
        section_id: EntityRef,
    },

    ChangeSectionForSubSection {
        subsection_id: EntityRef,
        section_id: EntityRef,
    },
}