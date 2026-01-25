use serde::{Deserialize, Serialize};
use ts_rs::TS;


#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppSetupRequest {
    pub app_name: String,
}
