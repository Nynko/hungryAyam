use axum::{
    routing::{get, post},
    Router,
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    features::restaurant::{
        dto::CreateRestaurantRequest,
        domain::Restaurant
    },
    state::AppState,
    errors::api_errors::ApiError,
};

pub fn restaurant_routes() -> Router<AppState>{
    Router::new()
        .route("/api/restaurants", post(create_restaurant))
        .route("/api/restaurant", get(get_restaurant))
        .route("/api/all-restaurants", get(list_restaurants))
        .route("/api/active-restaurants", get(get_restaurants_with_active_orders))
        .route("/api/update-restaurant", post(update_restaurant))
}


/// Create a new restaurant
pub async fn create_restaurant(
    State(app_state): State<AppState>,
    Json(request): Json<CreateRestaurantRequest>,
) -> Result<(StatusCode, Json<Restaurant>), ApiError> {
    let restaurant = app_state.restaurant_service.create_restaurant(request).await?;
    Ok((StatusCode::CREATED, Json(restaurant)))
}

/// Get all restaurants
pub async fn list_restaurants(
    State(app_state): State<AppState>,
) -> Result<Json<Vec<Restaurant>>, ApiError> {
    let restaurants = app_state.restaurant_service.list_restaurants().await?;
    Ok(Json(restaurants))
}

/// Get a restaurant by ID
pub async fn get_restaurant(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Restaurant>, ApiError> {
    let restaurant = app_state.restaurant_service
        .get_restaurant(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(restaurant))
}

/// Update a restaurant
pub async fn update_restaurant(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<CreateRestaurantRequest>,
) -> Result<Json<Restaurant>, ApiError> {
    let restaurant = app_state.restaurant_service
        .update_restaurant(id, request)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(restaurant))
}

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

/// Get restaurants with active orders
pub async fn get_restaurants_with_active_orders(
    State(app_state): State<AppState>,
) -> Result<Json<Vec<Restaurant>>, ApiError> {
    let restaurants = app_state.restaurant_service.get_restaurants_with_active_orders().await?;
    Ok(Json(restaurants))
}
