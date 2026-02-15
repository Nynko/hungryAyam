use anyhow::Result;
use uuid::Uuid;

use crate::features::{
    item::{
        domain::{
            item::Item,
            tag::{Tag, UpdateTag}
        },
        dto::{CreateItemRequest, UpdateItemRequest},
        repository::ItemRepository,
    },
    user::domain::User
};

#[derive(Clone)]
pub struct ItemService {
    repository: ItemRepository,
}

impl ItemService {
    pub fn new(repository: ItemRepository) -> Self {
        Self { repository }
    }

    // ==================== ITEM OPERATIONS ====================

    /// Create a new item (with tags if provided)
    /// Note: Name and price validation happens automatically during deserialization via validated_type
    pub async fn create_item(&self, user_id: Uuid, request: CreateItemRequest) -> Result<Item> {
        self.repository.create(user_id, request).await
    }

    /// Create multiple items in batch
    pub async fn create_batch_items(&self, user_id: Uuid, requests: Vec<CreateItemRequest>) -> Result<Vec<Item>> {
        let mut items = Vec::with_capacity(requests.len());
        for request in requests {
            let item = self.repository.create(user_id, request).await?;
            items.push(item);
        }
        Ok(items)
    }

    /// Get an item by ID
    pub async fn get_item(&self, id: Uuid) -> Result<Option<Item>> {
        self.repository.get_by_id(id).await
    }

    /// Get all items for a specific restaurant
    pub async fn list_items_by_restaurant(&self, restaurant_id: Uuid) -> Result<Vec<Item>> {
        self.repository.get_by_restaurant(restaurant_id).await
    }

    /// Get only active items for a specific restaurant
    pub async fn list_active_items_by_restaurant(&self, restaurant_id: Uuid) -> Result<Vec<Item>> {
        self.repository.get_active_by_restaurant(restaurant_id).await
    }

    /// Update an item (with tags if provided)
    /// Note: Name and price validation happens automatically during deserialization via validated_type
    pub async fn update_item(&self, user_id: Uuid, request: UpdateItemRequest) -> Result<Option<Item>> {
        if self.repository.get_by_id(request.id).await?.is_none() {
            return Ok(None);
        }

        self.repository.update(user_id, request).await
    }

    /// Delete an item
    pub async fn delete_item(&self, id: Uuid) -> Result<bool> {
        self.repository.delete(id).await
    }

    // ==================== TAG OPERATIONS ====================

    /// Get all available tags
    pub async fn list_tags(&self) -> Result<Vec<Tag>> {
        self.repository.get_all_tags().await
    }

    /// Get a tag by ID
    pub async fn get_tag(&self, id: Uuid) -> Result<Option<Tag>> {
        self.repository.get_tag_by_id(id).await
    }

    /// Update a tag (rename)
    pub async fn update_tag(&self, request: UpdateTag) -> Result<Option<Tag>> {
        self.repository.update_tag(request).await
    }

    /// Delete a tag (will cascade remove from all items)
    pub async fn delete_tag(&self, id: Uuid) -> Result<bool> {
        self.repository.delete_tag(id).await
    }
}
