use anyhow::Result;
use sqlx::PgPool;

use crate::features::restaurant::{
    dto::CreateRestaurantRequest,
    domain::Restaurant
};

pub type RestaurantRow = Restaurant;


#[derive(Clone)]
pub struct RestaurantRepository {
    pool: PgPool,
}

impl RestaurantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new restaurant
    pub async fn create(&self, request: CreateRestaurantRequest) -> Result<RestaurantRow> {
        let restaurant = sqlx::query_as!(
            RestaurantRow,
            r#"
            INSERT INTO restaurants (name, image_url)
            VALUES ($1, $2)
            RETURNING id, name, image_url, created_at
            "#,
            request.name,
            request.image_url
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(restaurant)
    }

    // /// Get a restaurant by ID
    // pub async fn get_by_id(&self, id: Uuid) -> Result<Option<Restaurant>> {
    //     let restaurant = sqlx::query_as!(
    //         Restaurant,
    //         "SELECT id, name, image_url, created_at FROM restaurants WHERE id = $1",
    //         id
    //     )
    //     .fetch_optional(&self.pool)
    //     .await?;

    //     Ok(restaurant)
    // }

    // /// Get all restaurants
    // pub async fn get_all(&self) -> Result<Vec<Restaurant>> {
    //     let restaurants = sqlx::query_as!(
    //         Restaurant,
    //         "SELECT id, name, image_url, created_at FROM restaurants ORDER BY created_at DESC"
    //     )
    //     .fetch_all(&self.pool)
    //     .await?;

    //     Ok(restaurants)
    // }

    // /// Update a restaurant
    // pub async fn update(&self, id: Uuid, request: CreateRestaurantRequest) -> Result<Option<Restaurant>> {
    //     let restaurant = sqlx::query_as!(
    //         Restaurant,
    //         r#"
    //         UPDATE restaurants
    //         SET name = $1, image_url = $2
    //         WHERE id = $3
    //         RETURNING id, name, image_url, created_at
    //         "#,
    //         request.name,
    //         request.image_url,
    //         id
    //     )
    //     .fetch_optional(&self.pool)
    //     .await?;

    //     Ok(restaurant)
    // }

    // /// Delete a restaurant
    // pub async fn delete(&self, id: Uuid) -> Result<bool> {
    //     let result = sqlx::query!(
    //         "DELETE FROM restaurants WHERE id = $1",
    //         id
    //     )
    //     .execute(&self.pool)
    //     .await?;

    //     Ok(result.rows_affected() > 0)
    // }

    // /// Check if a restaurant exists
    // pub async fn exists(&self, id: Uuid) -> Result<bool> {
    //     let exists = sqlx::query!(
    //         "SELECT EXISTS(SELECT 1 FROM restaurants WHERE id = $1)",
    //         id
    //     )
    //     .fetch_one(&self.pool)
    //     .await?
    //     .exists
    //     .unwrap_or(false);

    //     Ok(exists)
    // }

    // /// Get restaurants with active orders
    // pub async fn get_with_active_orders(&self) -> Result<Vec<Restaurant>> {
    //     let restaurants = sqlx::query_as!(
    //         Restaurant,
    //         r#"
    //         SELECT DISTINCT r.id, r.name, r.image_url, r.created_at
    //         FROM restaurants r
    //         INNER JOIN orders o ON r.id = o.restaurant_id
    //         WHERE o.active = true
    //         ORDER BY r.created_at DESC
    //         "#
    //     )
    //     .fetch_all(&self.pool)
    //     .await?;

    //     Ok(restaurants)
    // }
}
