use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    features::item::domain::{CreateItem, UpdateItem, Item},
    types::{url::UrlString, price::PriceCents, name::Name}
};

#[derive(Clone)]
pub struct ItemRepository {
    pool: PgPool,
}

impl ItemRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, request: CreateItem) -> Result<Item> {
        let item = sqlx::query_as!(
            Item,
            r#"
            INSERT INTO items (restaurant_id, name, description, base_price_cents, image_url, created_by, updated_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id,
                restaurant_id,
                name as "name: Name",
                description,
                base_price_cents as "base_price_cents: PriceCents",
                image_url as "image_url?: UrlString",
                active,
                created_at,
                updated_at,
                created_by,
                updated_by
            "#,
            request.restaurant_id,
            request.name.as_ref(),
            request.description,
            request.base_price_cents.as_ref(),
            request.image_url.as_ref().map(|u| u.to_string()),
            request.created_by,
            request.created_by
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(item)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<Item>> {
        let item = sqlx::query_as!(
            Item,
            r#"
            SELECT
                id,
                restaurant_id,
                name as "name: Name",
                description,
                base_price_cents as "base_price_cents: PriceCents",
                image_url as "image_url?: UrlString",
                active,
                created_at,
                updated_at,
                created_by,
                updated_by
            FROM items
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(item)
    }

    pub async fn get_all(&self) -> Result<Vec<Item>> {
        let items = sqlx::query_as!(
            Item,
            r#"
            SELECT
                id,
                restaurant_id,
                name as "name: Name",
                description,
                base_price_cents as "base_price_cents: PriceCents",
                image_url as "image_url?: UrlString",
                active,
                created_at,
                updated_at,
                created_by,
                updated_by
            FROM items
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(items)
    }

    /// Get all items for a specific restaurant
    pub async fn get_by_restaurant(&self, restaurant_id: Uuid) -> Result<Vec<Item>> {
        let items = sqlx::query_as!(
            Item,
            r#"
            SELECT
                id,
                restaurant_id,
                name as "name: Name",
                description,
                base_price_cents as "base_price_cents: PriceCents",
                image_url as "image_url?: UrlString",
                active,
                created_at,
                updated_at,
                created_by,
                updated_by
            FROM items
            WHERE restaurant_id = $1
            ORDER BY name ASC
            "#,
            restaurant_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(items)
    }

    /// Get only active items for a specific restaurant
    pub async fn get_active_by_restaurant(&self, restaurant_id: Uuid) -> Result<Vec<Item>> {
        let items = sqlx::query_as!(
            Item,
            r#"
            SELECT
                id,
                restaurant_id,
                name as "name: Name",
                description,
                base_price_cents as "base_price_cents: PriceCents",
                image_url as "image_url?: UrlString",
                active,
                created_at,
                updated_at,
                created_by,
                updated_by
            FROM items
            WHERE restaurant_id = $1 AND active = true
            ORDER BY name ASC
            "#,
            restaurant_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(items)
    }

    /// Update an item
    pub async fn update(&self, request: UpdateItem) -> Result<Option<Item>> {
        let item = sqlx::query_as!(
            Item,
            r#"
            UPDATE items
            SET name = COALESCE($1, name),
                description = COALESCE($2, description),
                base_price_cents = COALESCE($3, base_price_cents),
                image_url = COALESCE($4, image_url),
                active = COALESCE($5, active),
                updated_at = NOW(),
                updated_by = $7
            WHERE id = $6
            RETURNING
                id,
                restaurant_id,
                name as "name: Name",
                description,
                base_price_cents as "base_price_cents: PriceCents",
                image_url as "image_url?: UrlString",
                active,
                created_at,
                updated_at,
                created_by,
                updated_by
            "#,
            request.name.as_ref().map(|n| n.as_ref()),
            request.description,
            request.base_price_cents.as_ref().map(|p| p.as_ref()),
            request.image_url.as_ref().map(|u| u.to_string()),
            request.active,
            request.id,
            request.updated_by
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(item)
    }

    /// Delete an item
    pub async fn delete(&self, id: Uuid) -> Result<bool> {
        let result = sqlx::query!("DELETE FROM items WHERE id = $1", id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
