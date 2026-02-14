use uuid::Uuid;

use crate::{features::menu::domain::{
        menu::{CreateMenu,
            UnitCreateMenu
        },
        section::UnitCreateMenuSection,
        section_item::{MenuSectionItem, UnitCreateMenuSectionItem}
    }, types::actions::EntityRef};


pub enum CreateMenuAction {
    CreateMenu(CreateMenu),
    AddSection { menu_id: EntityRef, section: UnitCreateMenu },
    AddSubSection { section_id: EntityRef, sub_section: UnitCreateMenuSection },
    AddItem { section_id: EntityRef, item: UnitCreateMenuSectionItem },
}
