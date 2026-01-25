use axum::{
    extract::{State},
    http::StatusCode,
    Json,
};

use crate::{
    api::dtos::restaurant::{CreateRestaurantRequest,RestaurantDto},
    state::AppState,
    errors::api_errors::ApiError,
};

/// Create a new restaurant
pub async fn create_restaurant(
    State(app_state): State<AppState>,
    Json(request): Json<CreateRestaurantRequest>,
) -> Result<(StatusCode, Json<RestaurantDto>), ApiError> {
    let restaurant = app_state.restaurant_service.create_restaurant(request).await?;
    Ok((StatusCode::CREATED, Json(restaurant)))
}

// /// Get all restaurants
// pub async fn list_restaurants(
//     State(restaurant_service): State<RestaurantService>,
// ) -> Result<Json<Vec<Restaurant>>, ApiError> {
//     let restaurants = restaurant_service.list_restaurants().await?;
//     Ok(Json(restaurants))
// }

// /// Get a restaurant by ID
// pub async fn get_restaurant(
//     State(restaurant_service): State<RestaurantService>,
//     Path(id): Path<Uuid>,
// ) -> Result<Json<Restaurant>, ApiError> {
//     let restaurant = restaurant_service
//         .get_restaurant(id)
//         .await?
//         .ok_or(ApiError::NotFound)?;
//     Ok(Json(restaurant))
// }

// /// Update a restaurant
// pub async fn update_restaurant(
//     State(restaurant_service): State<RestaurantService>,
//     Path(id): Path<Uuid>,
//     Json(request): Json<CreateRestaurantRequest>,
// ) -> Result<Json<Restaurant>, ApiError> {
//     let restaurant = restaurant_service
//         .update_restaurant(id, request)
//         .await?
//         .ok_or(ApiError::NotFound)?;
//     Ok(Json(restaurant))
// }

// /// Delete a restaurant
// pub async fn delete_restaurant(
//     State(restaurant_service): State<RestaurantService>,
//     Path(id): Path<Uuid>,
// ) -> Result<StatusCode, ApiError> {
//     let deleted = restaurant_service.delete_restaurant(id).await?;
//     if deleted {
//         Ok(StatusCode::NO_CONTENT)
//     } else {
//         Err(ApiError::NotFound)
//     }
// }

// /// Get restaurants with active orders
// pub async fn get_restaurants_with_active_orders(
//     State(restaurant_service): State<RestaurantService>,
// ) -> Result<Json<Vec<Restaurant>>, ApiError> {
//     let restaurants = restaurant_service.get_restaurants_with_active_orders().await?;
//     Ok(Json(restaurants))
// }
