use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    features::{
        availability::{db_model::AvailabilityRuleRow, domain::AvailabilityRule},
        restaurant::{
            db_model::RestaurantRow,
            domain::{CreateRestaurant, Restaurant, UpdateRestaurant},
        },
    },
    types::{name::Name, url::ImageSource},
};

#[derive(Clone)]
pub struct RestaurantRepository {
    pool: PgPool,
}

impl RestaurantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        request: CreateRestaurant,
        operator_id: Uuid,
    ) -> Result<Restaurant> {
        let row = sqlx::query_as!(
            RestaurantRow,
            r#"
            INSERT INTO restaurants (name, image_url, phone_number, address, created_by, updated_by)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING
                id,
                name as "name: Name",
                image_url as "image_url?: ImageSource",
                phone_number,
                sms_phone_number,
                address,
                created_at,
                created_by,
                updated_at,
                updated_by,
                availability_rule_id
            "#,
            request.name.as_ref(),
            request.image_url.as_ref().map(|u| u.to_string()),
            request.phone_number.as_deref(),
            request.address.as_deref(),
            operator_id,
            operator_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(self.row_to_domain(row, None))
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<Restaurant>> {
        let row = sqlx::query_as!(
            RestaurantRow,
            r#"
            SELECT
                id,
                name as "name: Name",
                image_url as "image_url?: ImageSource",
                phone_number,
                sms_phone_number,
                address,
                created_at,
                created_by,
                updated_at,
                updated_by,
                availability_rule_id
            FROM restaurants
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let rule = self.load_availability_rule(r.availability_rule_id).await?;
                Ok(Some(self.row_to_domain(r, rule)))
            }
            None => Ok(None),
        }
    }

    pub async fn get_all(&self) -> Result<Vec<Restaurant>> {
        let rows = sqlx::query_as!(
            RestaurantRow,
            r#"
            SELECT
                id,
                name as "name: Name",
                image_url as "image_url?: ImageSource",
                phone_number,
                sms_phone_number,
                address,
                created_at,
                created_by,
                updated_at,
                updated_by,
                availability_rule_id
            FROM restaurants
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        self.rows_to_domains(rows).await
    }

    /// Update a restaurant
    pub async fn update(
        &self,
        request: UpdateRestaurant,
        operator_id: Uuid,
    ) -> Result<Option<Restaurant>> {
        let row = sqlx::query_as!(
            RestaurantRow,
            r#"
            UPDATE restaurants
            SET name = COALESCE($1, name),
                image_url = COALESCE($2, image_url),
                phone_number = COALESCE($3, phone_number),
                address = COALESCE($4, address),
                updated_at = NOW(),
                updated_by = $6
            WHERE id = $5
            RETURNING
                id,
                name as "name: Name",
                image_url as "image_url?: ImageSource",
                phone_number,
                sms_phone_number,
                address,
                created_at,
                created_by,
                updated_at,
                updated_by,
                availability_rule_id
            "#,
            request.name.as_ref().map(|n| n.as_ref()),
            request.image_url.as_ref().map(|u| u.to_string()),
            request.phone_number.as_deref(),
            request.address.as_deref(),
            request.id,
            operator_id
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let rule = self.load_availability_rule(r.availability_rule_id).await?;
                Ok(Some(self.row_to_domain(r, rule)))
            }
            None => Ok(None),
        }
    }

    /// Get all restaurants that have active order sessions
    /// A session is considered active if its end_date is in the future
    pub async fn get_with_active_sessions(&self) -> Result<Vec<Restaurant>> {
        let rows = sqlx::query_as!(
            RestaurantRow,
            r#"
            SELECT DISTINCT
                r.id,
                r.name as "name: Name",
                r.image_url as "image_url?: ImageSource",
                r.phone_number,
                r.sms_phone_number,
                r.address,
                r.created_at,
                r.created_by,
                r.updated_at,
                r.updated_by,
                r.availability_rule_id
            FROM restaurants r
            INNER JOIN order_sessions os ON os.restaurant_id = r.id
            WHERE os.end_date > NOW()
            ORDER BY r.created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        self.rows_to_domains(rows).await
    }

    /// Check if a restaurant has any active order sessions
    /// A session is considered active if its end_date is in the future
    pub async fn has_active_session(&self, restaurant_id: Uuid) -> Result<bool> {
        let result = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM order_sessions
                WHERE restaurant_id = $1
                AND end_date > NOW()
            ) as "exists!"
            "#,
            restaurant_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    /// Get the SMS phone number for a restaurant (admin only).
    pub async fn get_sms_phone(&self, id: Uuid) -> Result<Option<String>> {
        let row = sqlx::query!(
            r#"SELECT sms_phone_number FROM restaurants WHERE id = $1"#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(|r| r.sms_phone_number))
    }

    /// Set (or clear) the SMS phone number for a restaurant (admin only).
    pub async fn update_sms_phone(
        &self,
        id: Uuid,
        sms_phone: Option<&str>,
        operator_id: Uuid,
    ) -> Result<bool> {
        let result = sqlx::query!(
            r#"UPDATE restaurants SET sms_phone_number = $1, updated_at = NOW(), updated_by = $2 WHERE id = $3"#,
            sms_phone,
            operator_id,
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Delete a restaurant
    pub async fn delete(&self, id: Uuid) -> Result<bool> {
        let result = sqlx::query!("DELETE FROM restaurants WHERE id = $1", id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    // ==================== HELPERS ====================

    /// Load an availability rule by its ID (if present).
    async fn load_availability_rule(
        &self,
        rule_id: Option<Uuid>,
    ) -> Result<Option<AvailabilityRule>> {
        match rule_id {
            Some(id) => {
                let row = sqlx::query_as!(
                    AvailabilityRuleRow,
                    r#"
                    SELECT id, valid_from, valid_to, start_time, end_time, weekdays, public_holidays_country, public_holidays_mode, active
                    FROM availability_rules
                    WHERE id = $1
                    "#,
                    id
                )
                .fetch_optional(&self.pool)
                .await?;

                Ok(row.map(|r| AvailabilityRule {
                    id: r.id,
                    valid_from: r.valid_from,
                    valid_to: r.valid_to,
                    start_time: r.start_time,
                    end_time: r.end_time,
                    weekdays: r.weekdays,
                        public_holidays_country: r.public_holidays_country,
                        public_holidays_mode: r.public_holidays_mode.as_deref().and_then(|s| match s { "exclude" => Some(crate::features::availability::domain::PublicHolidaysMode::Exclude), "only" => Some(crate::features::availability::domain::PublicHolidaysMode::Only), _ => None }),
                    active: r.active,
                }))
            }
            None => Ok(None),
        }
    }

    /// Convert multiple rows into domain objects, batch-loading availability rules.
    async fn rows_to_domains(&self, rows: Vec<RestaurantRow>) -> Result<Vec<Restaurant>> {
        if rows.is_empty() {
            return Ok(vec![]);
        }

        // Collect all non-None availability_rule_ids
        let rule_ids: Vec<Uuid> = rows
            .iter()
            .filter_map(|r| r.availability_rule_id)
            .collect();

        let rules_map: std::collections::HashMap<Uuid, AvailabilityRule> = if !rule_ids.is_empty() {
            let rule_rows = sqlx::query_as!(
                AvailabilityRuleRow,
                r#"
                SELECT id, valid_from, valid_to, start_time, end_time, weekdays, public_holidays_country, public_holidays_mode, active
                FROM availability_rules
                WHERE id = ANY($1)
                "#,
                &rule_ids
            )
            .fetch_all(&self.pool)
            .await?;

            rule_rows
                .into_iter()
                .map(|r| {
                    let rule = AvailabilityRule {
                        id: r.id,
                        valid_from: r.valid_from,
                        valid_to: r.valid_to,
                        start_time: r.start_time,
                        end_time: r.end_time,
                        weekdays: r.weekdays,
                        public_holidays_country: r.public_holidays_country,
                        public_holidays_mode: r.public_holidays_mode.as_deref().and_then(|s| match s { "exclude" => Some(crate::features::availability::domain::PublicHolidaysMode::Exclude), "only" => Some(crate::features::availability::domain::PublicHolidaysMode::Only), _ => None }),
                        active: r.active,
                    };
                    (r.id, rule)
                })
                .collect()
        } else {
            std::collections::HashMap::new()
        };

        let restaurants = rows
            .into_iter()
            .map(|row| {
                let rule = row
                    .availability_rule_id
                    .and_then(|rid| rules_map.get(&rid).cloned());
                self.row_to_domain(row, rule)
            })
            .collect();

        Ok(restaurants)
    }

    /// Convert a `RestaurantRow` into the `Restaurant` domain object.
    fn row_to_domain(
        &self,
        row: RestaurantRow,
        availability_rule: Option<AvailabilityRule>,
    ) -> Restaurant {
        Restaurant {
            id: row.id,
            name: row.name,
            image_url: row.image_url,
            phone_number: row.phone_number,
            sms_enabled: row.sms_phone_number.is_some(),
            sms_phone_number: row.sms_phone_number,
            address: row.address,
            created_by: row.created_by,
            updated_by: row.updated_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
            availability_rule,
        }
    }
}