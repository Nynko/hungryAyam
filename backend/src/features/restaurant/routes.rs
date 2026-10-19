use axum::{
    Router, extract::{Path, State}, http::StatusCode, routing::{delete, get, post}
};
use serde::{Deserialize, Serialize};
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
        .route("/api/admin/restaurants/:id/sms-settings", get(get_sms_settings).put(set_sms_settings))
}

#[derive(Serialize, Deserialize)]
pub struct SmsSettings {
    pub sms_phone_number: Option<String>,
    /// Message with `{PickupTime}` and `{Orders}` placeholders. `None` → default format.
    pub sms_message_template: Option<String>,
}

fn non_blank(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Get the SMS settings for a restaurant (admin only).
pub async fn get_sms_settings(
    EditorUser(_user): EditorUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<SmsSettings>>, ApiError> {
    let (sms_phone_number, sms_message_template) = app_state
        .restaurant_service
        .get_sms_settings(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(SmsSettings { sms_phone_number, sms_message_template })))
}

/// Set or clear the SMS phone number and message template for a restaurant (admin only).
pub async fn set_sms_settings(
    EditorUser(user): EditorUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
    ApiJson(body): ApiJson<SmsSettings>,
) -> Result<ApiJson<ApiResponse<SmsSettings>>, ApiError> {
    let sms_phone_number = non_blank(body.sms_phone_number);
    if let Some(phone) = &sms_phone_number {
        if crate::features::sms::normalize_phone(phone).is_none() {
            return Err(ApiError::BadRequest(format!(
                "Invalid SMS phone number '{phone}'. Use international format, e.g. +33612345678."
            )));
        }
    }
    let sms_message_template = non_blank(body.sms_message_template);
    let updated = app_state
        .restaurant_service
        .update_sms_settings(id, sms_phone_number.clone(), sms_message_template.clone(), user.id)
        .await?;
    if !updated {
        return Err(ApiError::NotFound);
    }
    Ok(ApiJson(ApiResponse::success(SmsSettings { sms_phone_number, sms_message_template })))
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