use anyhow::Result;

use crate::domain::restaurant::Restaurant;
use crate::api::dtos::restaurant::CreateRestaurantRequest;
use crate::repository::restaurants::RestaurantRepository;

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
        // Validate restaurant name
        if request.name.trim().is_empty() {
            anyhow::bail!("Restaurant name cannot be empty");
        }

        if request.name.len() > 100 {
            anyhow::bail!("Restaurant name cannot exceed 100 characters");
        }

        // Validate image URL if provided
        // if let Some(ref url) = request.image_url {
        //     if url.len() > 500 {
        //         anyhow::bail!("Image URL cannot exceed 500 characters");
        //     }

        //     // Basic URL validation - starts with http/https
        //     if !url.starts_with("http://") && !url.starts_with("https://") {
        //         anyhow::bail!("Image URL must be a valid HTTP/HTTPS URL");
        //     }
        // }

        // Create the restaurant
        self.repository.create(request).await
    }

    // /// Get a restaurant by ID
    // pub async fn get_restaurant(&self, id: Uuid) -> Result<Option<Restaurant>> {
    //     self.repository.get_by_id(id).await
    // }

    // /// Get all restaurants
    // pub async fn list_restaurants(&self) -> Result<Vec<Restaurant>> {
    //     self.repository.get_all().await
    // }

    // /// Update a restaurant with business validation
    // pub async fn update_restaurant(&self, id: Uuid, request: CreateRestaurantRequest) -> Result<Option<Restaurant>> {
    //     // First check if restaurant exists
    //     if !self.repository.exists(id).await? {
    //         return Ok(None);
    //     }

    //     // Same validation as create
    //     if request.name.trim().is_empty() {
    //         anyhow::bail!("Restaurant name cannot be empty");
    //     }

    //     if request.name.len() > 100 {
    //         anyhow::bail!("Restaurant name cannot exceed 100 characters");
    //     }

    //     if let Some(ref url) = request.image_url {
    //         if url.len() > 500 {
    //             anyhow::bail!("Image URL cannot exceed 500 characters");
    //         }

    //         if !url.starts_with("http://") && !url.starts_with("https://") {
    //             anyhow::bail!("Image URL must be a valid HTTP/HTTPS URL");
    //         }
    //     }

    //     self.repository.update(id, request).await
    // }

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

    // /// Get restaurants that currently have active orders
    // pub async fn get_restaurants_with_active_orders(&self) -> Result<Vec<Restaurant>> {
    //     self.repository.get_with_active_orders().await
    // }

    // /// Check if a restaurant exists
    // pub async fn restaurant_exists(&self, id: Uuid) -> Result<bool> {
    //     self.repository.exists(id).await
    // }
}
