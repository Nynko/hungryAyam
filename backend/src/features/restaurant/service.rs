use anyhow::Result;
use uuid::Uuid;

use crate::features::restaurant::{
    domain::Restaurant,
    dto::{CreateRestaurantRequest, UpdateRestaurantRequest},
    repository::RestaurantRepository
};

#[derive(Clone)]
pub struct RestaurantService {
    repository: RestaurantRepository,
}

impl RestaurantService {
    pub fn new(repository: RestaurantRepository) -> Self {
        Self { repository }
    }

    /// Create a new restaurant with business validation
    pub async fn create_restaurant(&self, request: CreateRestaurantRequest) -> Result<Restaurant> {
        Restaurant::validate_name(&request.name)?;
        self.repository.create(request).await
    }

    /// Get a restaurant by ID
    pub async fn get_restaurant(&self, id: Uuid) -> Result<Option<Restaurant>> {
        self.repository.get_by_id(id).await
    }

    /// Get all restaurants
    pub async fn list_restaurants(&self) -> Result<Vec<Restaurant>> {
        self.repository.get_all().await
    }

    /// Update a restaurant with business validation
    pub async fn update_restaurant(&self, id: Uuid, request: UpdateRestaurantRequest) -> Result<Option<Restaurant>> {

        if !self.repository.get_by_id(id).await?.is_some() {
            return Ok(None);
        }

        if let Some(name) = &request.name {
            Restaurant::validate_name(&name)?;
        }


        self.repository.update(id, request).await
    }


    // TODO:  delete with the other repository to delete dependencies first

    // /// Delete a restaurant
    // /// Note: This will cascade delete all menus and orders for this restaurant
    // pub async fn delete_restaurant(&self, id: Uuid) -> Result<bool> {
    //     // Check if restaurant has any active orders
    //     let restaurants_with_orders = self.repository.get_with_active_orders().await?;
    //     let has_active_orders = restaurants_with_orders.iter().any(|r| r.id == id);

    //     if has_active_orders {
    //         anyhow::bail!("Cannot delete restaurant with active orders");
    //     }

    //     self.repository.delete(id).await
    // }

    /// Get restaurants that currently have active orders
    pub async fn get_restaurants_with_active_orders(&self) -> Result<Vec<Restaurant>> {
        self.repository.get_with_active_orders().await
    }

}
