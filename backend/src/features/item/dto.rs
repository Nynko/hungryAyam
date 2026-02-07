use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::types::url::UrlString;

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateItemRequest {
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<UrlString>,
    pub price : u32,
    // pub tags : Vec<Tags>
}
