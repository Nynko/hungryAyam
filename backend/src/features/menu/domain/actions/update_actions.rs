use serde::{Serialize,Deserialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::{
    features::menu::domain::{
        menu::UpdateMenu, section::{
            UnitCreateMenuSection,
            UpdateMenuSection
        },
        section_item::{
            UnitCreateMenuSectionItem,
            UpdateMenuSectionItem
        }
    },
    types::{actions::EntityRef, position::Position
    }
};


/// For Update a user can either replace the full menu by deleting/creating OR he can:
/// 1. Update an item (price, name...) or a section (name) or the menu (name)
/// 2. Add an item / Add a section (see create actions)
/// 3. Change the order (position) of an item or subsection or section
/// 4. Change an item of section (or a subsection of section)

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum UpdateMenuAction {
    UpdateMenu(UpdateMenu),
    UpdateMenuSection{ section_id: Uuid, update: UpdateMenuSection},
    UpdateMenuSectionItem { item_id: Uuid, update: UpdateMenuSectionItem},
    AddSection {parent_id : EntityRef, section: UnitCreateMenuSection}, // EntityRef because the parent_id can be another section or the menu itself
    AddItem {section_id: EntityRef, item: UnitCreateMenuSectionItem},
    ChangePositionSection {section_id : EntityRef, position: Position},
    ChangePositionItem { item_id : EntityRef, position: Position},
    ChangeSectionForItem { item_id: EntityRef, section_id : EntityRef},
    ChangeSectionForSubSection { subsection_id: EntityRef, section_id: EntityRef}
}
