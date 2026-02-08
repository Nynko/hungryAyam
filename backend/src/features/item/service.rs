use anyhow::Result;
use uuid::Uuid;

use crate::features::item::{
    domain::Item,
    dto::{CreateItemRequest, UpdateItemRequest},
    repository::ItemRepository
};

#[derive(Clone)]
pub struct ItemService {
    repository: ItemRepository,
}

impl ItemService {
    pub fn new(repository: ItemRepository) -> Self {
        Self { repository }
    }

    /// Create a new item
    /// Note: Name and price validation happens automatically during deserialization via validated_type
    pub async fn create_item(&self, request: CreateItemRequest) -> Result<Item> {
        self.repository.create(request).await
    }

    /// Create multiple items in batch
    pub async fn create_batch_items(&self, requests: Vec<CreateItemRequest>) -> Result<Vec<Item>> {
        let mut items = Vec::with_capacity(requests.len());
        for request in requests {
            let item = self.repository.create(request).await?;
            items.push(item);
        }
        Ok(items)
    }

    /// Get an item by ID
    pub async fn get_item(&self, id: Uuid) -> Result<Option<Item>> {
        self.repository.get_by_id(id).await
    }

    /// Get all items
    pub async fn list_items(&self) -> Result<Vec<Item>> {
        self.repository.get_all().await
    }

    /// Get all items for a specific restaurant
    pub async fn list_items_by_restaurant(&self, restaurant_id: Uuid) -> Result<Vec<Item>> {
        self.repository.get_by_restaurant(restaurant_id).await
    }

    /// Get only active items for a specific restaurant
    pub async fn list_active_items_by_restaurant(&self, restaurant_id: Uuid) -> Result<Vec<Item>> {
        self.repository.get_active_by_restaurant(restaurant_id).await
    }

    /// Update an item
    /// Note: Name and price validation happens automatically during deserialization via validated_type
    pub async fn update_item(&self, request: UpdateItemRequest) -> Result<Option<Item>> {
        if self.repository.get_by_id(request.id).await?.is_none() {
            return Ok(None);
        }

        self.repository.update(request).await
    }

    /// Delete an item
    pub async fn delete_item(&self, id: Uuid) -> Result<bool> {
        self.repository.delete(id).await
    }
}