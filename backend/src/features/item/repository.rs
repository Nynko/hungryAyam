use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    features::item::{
    db_model::ItemRow, domain::Item, dto::CreateItemRequest
    },
    types::utils::option_to_string
};


#[derive(Clone)]
pub struct ItemRepository {
    pool: PgPool,
}

impl ItemRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, request: CreateItemRequest) -> Result<Item> {
        let restaurant = sqlx::query_as!(
            ItemRow,
            r#"
            INSERT INTO items (name, image_url, base_price_cents, description, active)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, name, image_url, base_price_cents, description, active, created_at, updated_at
            "#,
            request.name,
            option_to_string(request.image_url),
            request.price,
            request.description,
            true
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(restaurant.into_domain())
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<Item>> {
        let restaurant_row = sqlx::query_as!(
            ItemRow,
            "SELECT id, name, image_url, created_at FROM items WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(restaurant_row.map(|resto| resto.into_domain()))
    }

    pub async fn get_all(&self) -> Result<Vec<Item>> {
        let items = sqlx::query_as!(
            ItemRow,
            "SELECT id, name, image_url, created_at FROM items ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(items.into_iter().map(|resto| resto.into_domain()).collect())
    }

    /// Update a restaurant
    pub async fn update(&self, id: Uuid, request: CreateItemRequest) -> Result<Option<Item>> {
        let restaurant = sqlx::query_as!(
            ItemRow,
            r#"
            UPDATE items
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
    //         "DELETE FROM items WHERE id = $1",
    //         id
    //     )
    //     .execute(&self.pool)
    //     .await?;

    //     Ok(result.rows_affected() > 0)
    // }

    /// Get items with active orders
    pub async fn get_with_active_orders(&self) -> Result<Vec<Item>> {
        let items = sqlx::query_as!(
            ItemRow,
            r#"
            SELECT DISTINCT r.id, r.name, r.image_url, r.created_at
            FROM items r
            INNER JOIN orders o ON r.id = o.restaurant_id
            WHERE o.active = true
            ORDER BY r.created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(items.into_iter().map(|resto| resto.into_domain()).collect())
    }
}
