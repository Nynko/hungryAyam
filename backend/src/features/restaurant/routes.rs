use axum::{
    Router, extract::{Path, State}, http::StatusCode, routing::{delete, get, post}
};
use uuid::Uuid;

use crate::{
    auth::middleware::EditorUser,
    errors::{
        api_errors::ApiError,
        json_extractor::ApiJson,
    },
    features::restaurant::domain::{CreateRestaurant, Restaurant, UpdateRestaurant},
    state::AppState,
    types::response::ApiResponse
};

pub fn restaurant_routes() -> Router<AppState> {
    Router::new()
        .route("/api/restaurants", post(create_restaurant))
        .route("/api/restaurants", get(list_restaurants))
        .route("/api/restaurants/active", get(list_active_restaurants))
        .route("/api/restaurants/:id", get(get_restaurant))
        .route("/api/restaurants/:id", delete(delete_restaurant))
        .route("/api/update-restaurant", post(update_restaurant))
}

/// Create a new restaurant (requires editor user)
pub async fn create_restaurant(
    EditorUser(user): EditorUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<CreateRestaurant>,
) -> Result<(StatusCode, ApiJson<ApiResponse<Restaurant>>), ApiError> {
    let restaurant = app_state.restaurant_service.create_restaurant(request, user.id).await?;
    Ok((StatusCode::CREATED, ApiJson(ApiResponse::success(restaurant))))
}

/// Get all restaurants
pub async fn list_restaurants(
    State(app_state): State<AppState>,
) -> Result<ApiJson<ApiResponse<Vec<Restaurant>>>, ApiError> {
    let restaurants = app_state.restaurant_service.list_restaurants().await?;
    Ok(ApiJson(ApiResponse::success(restaurants)))
}

/// Get restaurants with active order sessions
pub async fn list_active_restaurants(
    State(app_state): State<AppState>,
) -> Result<ApiJson<ApiResponse<Vec<Restaurant>>>, ApiError> {
    let restaurants = app_state.restaurant_service.list_active_restaurants().await?;
    Ok(ApiJson(ApiResponse::success(restaurants)))
}

/// Get a restaurant by ID
pub async fn get_restaurant(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Restaurant>>, ApiError> {
    let restaurant = app_state.restaurant_service
        .get_restaurant(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(restaurant)))
}

/// Update a restaurant (requires editor user)
pub async fn update_restaurant(
    EditorUser(user): EditorUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<UpdateRestaurant>,
) -> Result<ApiJson<ApiResponse<Restaurant>>, ApiError> {
    let restaurant = app_state.restaurant_service
        .update_restaurant(request, user.id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(restaurant)))
}

/// Delete a restaurant (requires editor user)
/// Returns success on deletion, 404 if not found
/// Returns 400 if restaurant has active order sessions
pub async fn delete_restaurant(
    EditorUser(_user): EditorUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<()>>, ApiError> {
    let deleted = app_state.restaurant_service.delete_restaurant(id).await?;
    if deleted {
        Ok(ApiJson(ApiResponse::success(())))
    } else {
        Err(ApiError::NotFound)
    }
}