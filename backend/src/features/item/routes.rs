use axum::{
    routing::{get, post},
    Router,
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    features::item::{
        dto::CreateItemrequest,
        domain::Item
    },
    state::AppState,
    errors::api_errors::ApiError,
};

pub fn item_routes() -> Router<AppState>{
    Router::new()
        .route("/api/item", post(create_item))
        .route("/api/batch_items", post(create_batch_items))
        .route("/api/item/:id", get(get_item))
        .route("/api/all-items/:restaurant_id", get(list_items_for_restaurant))
        .route("/api/update-item", post(update_item))
        .route("/api/delete-item/:id", post(delete_item))
}


pub async fn create_item(
    State(app_state): State<AppState>,
    Json(request): Json<CreateItemrequest>,
) -> Result<(StatusCode, Json<Item>), ApiError> {
    // let restaurant = app_state.restaurant_service.create_restaurant(request).await?;
    // Ok((StatusCode::CREATED, Json(restaurant)))
    todo!()
}

pub async fn create_batch_items(
    State(app_state): State<AppState>,
    Json(request): Json<Vec<CreateItemrequest>>,
) -> Result<(StatusCode, Json<Vec<Item>>), ApiError> {
    // let restaurant = app_state.restaurant_service.create_restaurant(request).await?;
    // Ok((StatusCode::CREATED, Json(restaurant)))
    todo!()
}

pub async fn get_item(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Item>, ApiError> {
    // let restaurant = app_state.restaurant_service
    //     .get_restaurant(id)
    //     .await?
    //     .ok_or(ApiError::NotFound)?;
    // Ok(Json(restaurant))
    //
    todo!()
}

pub async fn list_items_for_restaurant(
    State(app_state): State<AppState>,
    Path(restaurant_id): Path<Uuid>
) -> Result<Json<Vec<Item>>, ApiError> {
    // let restaurants = app_state.restaurant_service.list_restaurants().await?;
    // Ok(Json(restaurants))
    todo!()
}

pub async fn update_item(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<CreateItemrequest>,
) -> Result<Json<Item>, ApiError> {
    // let restaurant = app_state.restaurant_service
    //     .update_restaurant(id, request)
    //     .await?
    //     .ok_or(ApiError::NotFound)?;
    // Ok(Json(restaurant))
    todo!()
}

pub async fn delete_item(
    State(restaurant_service): State<RestaurantService>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    // let deleted = restaurant_service.delete_restaurant(id).await?;
    // if deleted {
    //     Ok(StatusCode::NO_CONTENT)
    // } else {
    //     Err(ApiError::NotFound)
    // }
    todo!()
}

/// Get restaurants with active orders
pub async fn get_restaurants_with_active_orders(
    State(app_state): State<AppState>,
) -> Result<Json<Vec<Restaurant>>, ApiError> {
    let restaurants = app_state.restaurant_service.get_restaurants_with_active_orders().await?;
    Ok(Json(restaurants))
}
