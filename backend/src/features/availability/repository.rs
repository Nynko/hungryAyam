use anyhow::Result;
use chrono::{NaiveDate, NaiveTime};
use sqlx::PgPool;
use uuid::Uuid;

use crate::features::availability::{
    db_model::AvailabilityRuleRow,
    domain::{AvailabilityRule, CreateAvailabilityRule, PublicHolidaysMode, UpdateAvailabilityRule},
};

#[derive(Clone)]
pub struct AvailabilityRepository {
    pool: PgPool,
}

impl AvailabilityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new availability rule.
    pub async fn create(&self, request: CreateAvailabilityRule) -> Result<AvailabilityRule> {
        let ph_mode = request.public_holidays_mode.as_ref().map(mode_to_str);
        let row = sqlx::query_as!(
            AvailabilityRuleRow,
            r#"
            INSERT INTO availability_rules (
                valid_from, valid_to, start_time, end_time, weekdays,
                public_holidays_country, public_holidays_mode, active
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING
                id, valid_from, valid_to, start_time, end_time, weekdays,
                public_holidays_country, public_holidays_mode, active
            "#,
            request.valid_from,
            request.valid_to,
            request.start_time,
            request.end_time,
            request.weekdays.as_deref(),
            request.public_holidays_country,
            ph_mode,
            request.active,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(self.row_to_domain(row))
    }

    /// Get an availability rule by ID.
    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<AvailabilityRule>> {
        let row = sqlx::query_as!(
            AvailabilityRuleRow,
            r#"
            SELECT id, valid_from, valid_to, start_time, end_time, weekdays,
                   public_holidays_country, public_holidays_mode, active
            FROM availability_rules
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| self.row_to_domain(r)))
    }

    /// Update an availability rule.
    pub async fn update(&self, request: UpdateAvailabilityRule) -> Result<Option<AvailabilityRule>> {
        let ph_mode = request.public_holidays_mode.as_ref().map(mode_to_str);
        let row = sqlx::query_as!(
            AvailabilityRuleRow,
            r#"
            UPDATE availability_rules
            SET valid_from              = COALESCE($1, valid_from),
                valid_to                = COALESCE($2, valid_to),
                start_time              = COALESCE($3, start_time),
                end_time                = COALESCE($4, end_time),
                weekdays                = COALESCE($5, weekdays),
                public_holidays_country = $6,
                public_holidays_mode    = $7,
                active                  = COALESCE($8, active)
            WHERE id = $9
            RETURNING id, valid_from, valid_to, start_time, end_time, weekdays,
                      public_holidays_country, public_holidays_mode, active
            "#,
            request.valid_from,
            request.valid_to,
            request.start_time,
            request.end_time,
            request.weekdays.as_deref(),
            request.public_holidays_country,
            ph_mode,
            request.active,
            request.id,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| self.row_to_domain(r)))
    }

    /// Delete an availability rule by ID.
    pub async fn delete(&self, id: Uuid) -> Result<bool> {
        sqlx::query!("UPDATE restaurants SET availability_rule_id = NULL WHERE availability_rule_id = $1", id)
            .execute(&self.pool).await?;
        sqlx::query!("UPDATE menus SET availability_rule_id = NULL WHERE availability_rule_id = $1", id)
            .execute(&self.pool).await?;
        sqlx::query!("UPDATE items SET availability_rule_id = NULL WHERE availability_rule_id = $1", id)
            .execute(&self.pool).await?;
        sqlx::query!("UPDATE offers SET availability_rule_id = NULL WHERE availability_rule_id = $1", id)
            .execute(&self.pool).await?;

        let result = sqlx::query!("DELETE FROM availability_rules WHERE id = $1", id)
            .execute(&self.pool).await?;

        Ok(result.rows_affected() > 0)
    }

    /// List all availability rules.
    pub async fn list_all(&self) -> Result<Vec<AvailabilityRule>> {
        let rows = sqlx::query_as!(
            AvailabilityRuleRow,
            r#"
            SELECT id, valid_from, valid_to, start_time, end_time, weekdays,
                   public_holidays_country, public_holidays_mode, active
            FROM availability_rules
            ORDER BY id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| self.row_to_domain(r)).collect())
    }

    pub async fn assign_to_menu(&self, menu_id: Uuid, rule_id: Option<Uuid>) -> Result<bool> {
        let result = sqlx::query!(
            "UPDATE menus SET availability_rule_id = $1 WHERE id = $2", rule_id, menu_id)
            .execute(&self.pool).await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn assign_to_item(&self, item_id: Uuid, rule_id: Option<Uuid>) -> Result<bool> {
        let result = sqlx::query!(
            "UPDATE items SET availability_rule_id = $1 WHERE id = $2", rule_id, item_id)
            .execute(&self.pool).await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn assign_to_offer(&self, offer_id: Uuid, rule_id: Option<Uuid>) -> Result<bool> {
        let result = sqlx::query!(
            "UPDATE offers SET availability_rule_id = $1 WHERE id = $2", rule_id, offer_id)
            .execute(&self.pool).await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn assign_to_restaurant(&self, restaurant_id: Uuid, rule_id: Option<Uuid>) -> Result<bool> {
        let result = sqlx::query!(
            "UPDATE restaurants SET availability_rule_id = $1 WHERE id = $2", rule_id, restaurant_id)
            .execute(&self.pool).await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_for_menu(&self, menu_id: Uuid) -> Result<Option<AvailabilityRule>> {
        let row = sqlx::query_as!(
            AvailabilityRuleRow,
            r#"
            SELECT ar.id, ar.valid_from, ar.valid_to, ar.start_time, ar.end_time, ar.weekdays,
                   ar.public_holidays_country, ar.public_holidays_mode, ar.active
            FROM availability_rules ar
            JOIN menus m ON m.availability_rule_id = ar.id
            WHERE m.id = $1
            "#, menu_id)
        .fetch_optional(&self.pool).await?;
        Ok(row.map(|r| self.row_to_domain(r)))
    }

    pub async fn get_for_item(&self, item_id: Uuid) -> Result<Option<AvailabilityRule>> {
        let row = sqlx::query_as!(
            AvailabilityRuleRow,
            r#"
            SELECT ar.id, ar.valid_from, ar.valid_to, ar.start_time, ar.end_time, ar.weekdays,
                   ar.public_holidays_country, ar.public_holidays_mode, ar.active
            FROM availability_rules ar
            JOIN items i ON i.availability_rule_id = ar.id
            WHERE i.id = $1
            "#, item_id)
        .fetch_optional(&self.pool).await?;
        Ok(row.map(|r| self.row_to_domain(r)))
    }

    pub async fn get_for_offer(&self, offer_id: Uuid) -> Result<Option<AvailabilityRule>> {
        let row = sqlx::query_as!(
            AvailabilityRuleRow,
            r#"
            SELECT ar.id, ar.valid_from, ar.valid_to, ar.start_time, ar.end_time, ar.weekdays,
                   ar.public_holidays_country, ar.public_holidays_mode, ar.active
            FROM availability_rules ar
            JOIN offers o ON o.availability_rule_id = ar.id
            WHERE o.id = $1
            "#, offer_id)
        .fetch_optional(&self.pool).await?;
        Ok(row.map(|r| self.row_to_domain(r)))
    }

    pub async fn get_for_restaurant(&self, restaurant_id: Uuid) -> Result<Option<AvailabilityRule>> {
        let row = sqlx::query_as!(
            AvailabilityRuleRow,
            r#"
            SELECT ar.id, ar.valid_from, ar.valid_to, ar.start_time, ar.end_time, ar.weekdays,
                   ar.public_holidays_country, ar.public_holidays_mode, ar.active
            FROM availability_rules ar
            JOIN restaurants r ON r.availability_rule_id = ar.id
            WHERE r.id = $1
            "#, restaurant_id)
        .fetch_optional(&self.pool).await?;
        Ok(row.map(|r| self.row_to_domain(r)))
    }

    // ── Helpers ────────────────────────────────────────────────────

    fn row_to_domain(&self, row: AvailabilityRuleRow) -> AvailabilityRule {
        AvailabilityRule {
            id: row.id,
            valid_from: row.valid_from,
            valid_to: row.valid_to,
            start_time: row.start_time,
            end_time: row.end_time,
            weekdays: row.weekdays,
            public_holidays_country: row.public_holidays_country,
            public_holidays_mode: row.public_holidays_mode.as_deref().and_then(str_to_mode),
            active: row.active,
        }
    }
}

fn mode_to_str(mode: &PublicHolidaysMode) -> &'static str {
    match mode {
        PublicHolidaysMode::Exclude => "exclude",
        PublicHolidaysMode::Only => "only",
    }
}

fn str_to_mode(s: &str) -> Option<PublicHolidaysMode> {
    match s {
        "exclude" => Some(PublicHolidaysMode::Exclude),
        "only" => Some(PublicHolidaysMode::Only),
        _ => None,
    }
}
