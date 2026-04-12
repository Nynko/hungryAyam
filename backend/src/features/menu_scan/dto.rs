use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// A tag inferred by the AI from the menu image (e.g. "spicy", "vegetarian").
///
/// Claude sometimes returns tags as plain strings (`"vegetarian"`) and sometimes
/// as objects (`{"name": "vegetarian"}`). The custom Deserialize handles both.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ScannedTag {
    pub name: String,
}

impl<'de> Deserialize<'de> for ScannedTag {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = ScannedTag;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string or an object with a name field")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<ScannedTag, E> {
                Ok(ScannedTag { name: v.to_owned() })
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(self, mut map: M) -> Result<ScannedTag, M::Error> {
                let mut name = None;
                while let Some(key) = map.next_key::<String>()? {
                    if key == "name" {
                        name = Some(map.next_value()?);
                    } else {
                        map.next_value::<serde::de::IgnoredAny>()?;
                    }
                }
                name.map(|n| ScannedTag { name: n })
                    .ok_or_else(|| serde::de::Error::missing_field("name"))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

/// A single item extracted from the menu image.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ScannedItem {
    pub name: String,
    pub description: Option<String>,
    /// Price in cents (e.g. 1250 for 12.50).
    pub base_price_cents: i32,
    pub tags: Vec<ScannedTag>,
    /// Image URL extracted from the webpage (only set for URL-based scans).
    #[serde(default)]
    pub image_url: Option<String>,
}

/// A section item — wraps a `ScannedItem` with a position.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ScannedSectionItem {
    pub position: i32,
    pub item: ScannedItem,
}

/// A menu section extracted from the image, with optional subsections.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ScannedSection {
    pub name: String,
    pub description: Option<String>,
    pub position: i32,
    pub items: Vec<ScannedSectionItem>,
    pub subsections: Vec<ScannedSection>,
}

/// Top-level response from the menu scan endpoint.
/// The frontend adds `restaurant_id` and other defaults before creating the menu.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MenuScanResponse {
    pub name: String,
    pub description: Option<String>,
    pub sections: Vec<ScannedSection>,
}

/// Returned immediately after creating an async URL scan job.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct MenuScanJobCreated {
    pub job_id: Uuid,
}

/// Returned by the polling endpoint.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct MenuScanJobStatus {
    pub job_id: Uuid,
    /// One of: "pending", "processing", "completed", "failed"
    pub status: String,
    pub result: Option<MenuScanResponse>,
    pub error: Option<String>,
}
