use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    features::restaurant::{
    db_model::RestaurantRow, domain::Restaurant, dto::CreateRestaurantRequest
    },
    types::utils::option_to_string
};


#[derive(Clone)]
pub struct RestaurantRepository {
    pool: PgPool,
}

impl RestaurantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, request: CreateRestaurantRequest) -> Result<Restaurant> {
        let restaurant = sqlx::query_as!(
            RestaurantRow,
            r#"
            INSERT INTO restaurants (name, image_url)
            VALUES ($1, $2)
            RETURNING id, name, image_url, created_at
            "#,
            request.name,
            option_to_string(request.image_url)
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(restaurant.into_domain())
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<Restaurant>> {
        let restaurant_row = sqlx::query_as!(
            RestaurantRow,
            "SELECT id, name, image_url, created_at FROM restaurants WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(restaurant_row.map(|resto| resto.into_domain()))
    }

    pub async fn get_all(&self) -> Result<Vec<Restaurant>> {
        let restaurants = sqlx::query_as!(
            RestaurantRow,
            "SELECT id, name, image_url, created_at FROM restaurants ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(restaurants.into_iter().map(|resto| resto.into_domain()).collect())
    }

    /// Update a restaurant
    pub async fn update(&self, id: Uuid, request: CreateRestaurantRequest) -> Result<Option<Restaurant>> {
        let restaurant = sqlx::query_as!(
            RestaurantRow,
            r#"
            UPDATE restaurants
            SET name = $1, image_url = $2
            WHERE id = $3
            RETURNING id, name, image_url, created_at
            "#,
            request.name,
            option_to_string(request.image_url),
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(restaurant.map(|resto| resto.into_domain()))
    }

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

    /// Get restaurants with active orders
    pub async fn get_with_active_orders(&self) -> Result<Vec<Restaurant>> {
        let restaurants = sqlx::query_as!(
            RestaurantRow,
            r#"
            SELECT DISTINCT r.id, r.name, r.image_url, r.created_at
            FROM restaurants r
            INNER JOIN orders o ON r.id = o.restaurant_id
            WHERE o.active = true
            ORDER BY r.created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(restaurants.into_iter().map(|resto| resto.into_domain()).collect())
    }
}
