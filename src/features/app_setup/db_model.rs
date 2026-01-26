use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::{
    types::url::UrlString,
    features::app_setup::domain::AppSetup
};


#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AppSetupRow {
    pub id: i16,
    pub title: String,
    pub image_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>
}

impl AppSetupRow {
    pub fn into_domain(self) -> AppSetup {
        let option_url_string: Option<UrlString> = self.image_url
            .and_then(|s| url::Url::parse(&s).ok().map(UrlString));

        AppSetup {
            id: self.id,
            title: self.title,
            image_url: option_url_string,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
