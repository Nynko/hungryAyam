use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    features::restaurant::{db_model::RestaurantRow, domain::{
        CreateRestaurant, Restaurant, UpdateRestaurant
    }},
    types::{
        name::Name, url::UrlString
    }
};

#[derive(Clone)]
pub struct RestaurantRepository {
    pool: PgPool,
}

impl RestaurantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, request: CreateRestaurant, operator_id: Uuid) -> Result<Restaurant> {
        let restaurant = sqlx::query_as!(
            RestaurantRow,
            r#"
            INSERT INTO restaurants (name, image_url, created_by, updated_by)
            VALUES ($1, $2, $3, $4)
            RETURNING
                id,
                name as "name: Name",
                image_url as "image_url?: UrlString",
                created_at,
                created_by,
                updated_at,
                updated_by
            "#,
            request.name.as_ref(),
            request.image_url.as_ref().map(|u| u.to_string()),
            operator_id,
            operator_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(restaurant)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<Restaurant>> {
        let restaurant = sqlx::query_as!(
            RestaurantRow,
            r#"
            SELECT
                id,
                name as "name: Name",
                image_url as "image_url?: UrlString",
                created_at,
                created_by,
                updated_at,
                updated_by
            FROM restaurants
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(restaurant)
    }

    pub async fn get_all(&self) -> Result<Vec<Restaurant>> {
        let restaurants = sqlx::query_as!(
            RestaurantRow,
            r#"
            SELECT
                id,
                name as "name: Name",
                image_url as "image_url?: UrlString",
                created_at,
                created_by,
                updated_at,
                updated_by
            FROM restaurants
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(restaurants)
    }

    /// Update a restaurant
    pub async fn update(&self, request: UpdateRestaurant, operator_id: Uuid) -> Result<Option<Restaurant>> {
        let restaurant = sqlx::query_as!(
            RestaurantRow,
            r#"
            UPDATE restaurants
            SET name = COALESCE($1, name),
                image_url = COALESCE($2, image_url),
                updated_at = NOW(),
                updated_by = $4
            WHERE id = $3
            RETURNING
                id,
                name as "name: Name",
                image_url as "image_url?: UrlString",
                created_at,
                created_by,
                updated_at,
                updated_by
            "#,
            request.name.as_ref().map(|n| n.as_ref()),
            request.image_url.as_ref().map(|u| u.to_string()),
            request.id,
            operator_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(restaurant)
    }

    /// Get all restaurants that have active order sessions
    /// A session is considered active if its end_date is in the future
    pub async fn get_with_active_sessions(&self) -> Result<Vec<Restaurant>> {
        let restaurants = sqlx::query_as!(
            RestaurantRow,
            r#"
            SELECT DISTINCT
                r.id,
                r.name as "name: Name",
                r.image_url as "image_url?: UrlString",
                r.created_at,
                r.created_by,
                r.updated_at,
                r.updated_by
            FROM restaurants r
            INNER JOIN order_sessions os ON os.restaurant_id = r.id
            WHERE os.end_date > NOW()
            ORDER BY r.created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(restaurants)
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

    /// Delete a restaurant
    pub async fn delete(&self, id: Uuid) -> Result<bool> {
        let result = sqlx::query!("DELETE FROM restaurants WHERE id = $1", id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
