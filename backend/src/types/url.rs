use std::ops::Deref;
use url::Url;
use sqlx::{FromRow, Row, Error, postgres::PgRow};
use serde::{Serialize, Deserialize};
use ts_rs::TS;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, TS, Serialize, Deserialize)]
pub struct UrlString(pub Url);

impl Deref for UrlString {
    type Target = Url;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'r> FromRow<'r, PgRow> for UrlString {
    fn from_row(row: &'r PgRow) -> Result<Self, Error> {
        let s: String = row.try_get("url")?;
        Url::parse(&s)
            .map(UrlString)
            .map_err(|_| Error::ColumnDecode {
                index: "url".into(),
                source: sqlx::error::BoxDynError::from("Invalid URL"),
            })
    }
}

impl fmt::Display for UrlString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
