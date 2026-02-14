use enum_stringify::EnumStringify;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::str::FromStr;

#[derive(Debug, EnumStringify, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum AuthMethod {
    NoneWithCookie,
    Password,
}

// === SQLx Type: stored as TEXT in Postgres ===
impl sqlx::Type<sqlx::Postgres> for AuthMethod {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

// === SQLx Encode: serialize to string ===
impl sqlx::Encode<'_, sqlx::Postgres> for AuthMethod {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        let s = self.to_string();
        <String as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&s, buf)
    }
}

// === SQLx Decode: parse from string ===
impl sqlx::Decode<'_, sqlx::Postgres> for AuthMethod {
    fn decode(
        value: sqlx::postgres::PgValueRef<'_>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        AuthMethod::from_str(&s)
            .map_err(|e| format!("Invalid AuthMethod '{}': {}", s, e).into())
    }
}