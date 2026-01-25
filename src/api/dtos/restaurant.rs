use serde::{Deserialize, Serialize};
use crate::domain::restaurant::Restaurant;
use ts_rs::TS;

pub type RestaurantDto = Restaurant;


#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateRestaurantRequest {
    pub name: String,
    pub image_url: Option<String>,
}
