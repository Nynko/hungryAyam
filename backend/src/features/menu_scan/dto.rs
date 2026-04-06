use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A tag inferred by the AI from the menu image (e.g. "spicy", "vegetarian").
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ScannedTag {
    pub name: String,
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
