use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::{
    features::{restaurant::domain::Restaurant}, types::url::UrlString
};
use uuid::Uuid;


#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RestaurantRow {
    pub id: Uuid,
    pub name: String,
    pub image_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl RestaurantRow {
    pub fn into_domain(self) -> Restaurant {
        let option_url_string: Option<UrlString> = self.image_url
            .and_then(|s| url::Url::parse(&s).ok().map(UrlString));

        Restaurant {
            id: self.id,
            name: self.name,
            image_url: option_url_string,
            created_at: self.created_at }
    }
}
