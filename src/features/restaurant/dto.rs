use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateRestaurantRequest {
    pub name: String,
    pub image_url: Option<String>,
}
