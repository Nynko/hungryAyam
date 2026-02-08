use hungry_ayam_derive::IntoDomain;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    features::app_setup::domain::CreateAppSetup,
    types::url::UrlString,
    traits::domain_traits::IntoDomain
};


#[derive(Debug, Serialize, Deserialize, TS, IntoDomain)]
#[ts(export)]
#[into_domain(CreateAppSetup)]
pub struct AppSetupRequest {
    #[into_domain_name="title"]
    pub app_name: String,
    pub image_url: Option<UrlString>
}
