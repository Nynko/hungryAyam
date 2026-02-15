use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;
use crate::features::menu::domain::{
    actions::update_actions::UpdateMenuAction,
    menu::CreateMenu,
};

pub type CreateMenuRequest = CreateMenu;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UpdateMenuActionsRequest {
    pub menu_id: Uuid,
    pub actions: Vec<UpdateMenuAction>,
}

/// Response for menu reset operation
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ResetMenuResponse {
    pub menu_id: Uuid,
    pub items_reset: u64,
}

/// Request for menu reset operation
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ResetMenuRequest {
    pub id: Uuid,
}