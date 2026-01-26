use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::types::url::UrlString;


#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppSetupRequest {
    pub app_name: String,
    pub image_url: Option<UrlString>
}
