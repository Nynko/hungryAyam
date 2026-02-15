use anyhow::Result;
use uuid::Uuid;

use crate::features::restaurant::{
    domain::Restaurant,
    dto::{CreateRestaurantRequest, UpdateRestaurantRequest},
    repository::RestaurantRepository,
};

#[derive(Clone)]
pub struct RestaurantService {
    repository: RestaurantRepository,
}

impl RestaurantService {
    pub fn new(repository: RestaurantRepository) -> Self {
        Self { repository }
    }

    /// Create a new restaurant
    /// Note: Name validation happens automatically during deserialization via validated_type
    pub async fn create_restaurant(&self, request: CreateRestaurantRequest, operator_id: Uuid) -> Result<Restaurant> {
        self.repository.create(request, operator_id).await
    }

    /// Get a restaurant by ID
    pub async fn get_restaurant(&self, id: Uuid) -> Result<Option<Restaurant>> {
        self.repository.get_by_id(id).await
    }

    /// Get all restaurants
    pub async fn list_restaurants(&self) -> Result<Vec<Restaurant>> {
        self.repository.get_all().await
    }

    /// Get restaurants that have active order sessions
    pub async fn list_active_restaurants(&self) -> Result<Vec<Restaurant>> {
        self.repository.get_with_active_sessions().await
    }

    /// Update a restaurant
    /// Note: Name validation happens automatically during deserialization via validated_type
    pub async fn update_restaurant(&self, request: UpdateRestaurantRequest, operator_id: Uuid) -> Result<Option<Restaurant>> {
        if !self.repository.get_by_id(request.id).await?.is_some() {
            return Ok(None);
        }

        self.repository.update(request, operator_id).await
    }


    /// Delete a restaurant
    /// Note: This will cascade delete all menus, items, and sessions for this restaurant
    pub async fn delete_restaurant(&self, id: Uuid) -> Result<bool> {
        // Check if restaurant has any active sessions
        if self.repository.has_active_session(id).await? {
            anyhow::bail!("Cannot delete restaurant with active order sessions");
        }

        self.repository.delete(id).await
    }

}
