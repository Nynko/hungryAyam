use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Router,
};
use uuid::Uuid;

use crate::{
    auth::middleware::AuthUser,
    errors::{api_errors::ApiError, json_extractor::ApiJson},
    features::availability::{
        domain::AvailabilityRule,
        dto::{AssignAvailabilityRequest, CreateAvailabilityRuleRequest, UpdateAvailabilityRuleRequest},
    },
    state::AppState,
    types::response::ApiResponse,
};

pub fn availability_routes() -> Router<AppState> {
    Router::new()
        // ── CRUD ────────────────────────────────────────────
        .route("/api/availability-rules", post(create_rule))
        .route("/api/availability-rules", get(list_rules))
        .route("/api/availability-rules/:id", get(get_rule))
        .route("/api/availability-rules/:id", delete(delete_rule))
        .route("/api/update-availability-rule", post(update_rule))
        // ── Assignment ──────────────────────────────────────
        .route("/api/restaurants/:id/availability-rule", post(assign_to_restaurant))
        .route("/api/restaurants/:id/availability-rule", get(get_rule_for_restaurant))
        .route("/api/menus/:id/availability-rule", post(assign_to_menu))
        .route("/api/menus/:id/availability-rule", get(get_rule_for_menu))
        .route("/api/items/:id/availability-rule", post(assign_to_item))
        .route("/api/items/:id/availability-rule", get(get_rule_for_item))
        .route("/api/offers/:id/availability-rule", post(assign_to_offer))
        .route("/api/offers/:id/availability-rule", get(get_rule_for_offer))
}

// ==================== CRUD HANDLERS ====================

/// Create a new availability rule.
pub async fn create_rule(
    AuthUser(_user): AuthUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<CreateAvailabilityRuleRequest>,
) -> Result<(StatusCode, ApiJson<ApiResponse<AvailabilityRule>>), ApiError> {
    let rule = app_state.availability_service.create_rule(request).await?;
    Ok((StatusCode::CREATED, ApiJson(ApiResponse::success(rule))))
}

/// List all availability rules.
pub async fn list_rules(
    State(app_state): State<AppState>,
) -> Result<ApiJson<ApiResponse<Vec<AvailabilityRule>>>, ApiError> {
    let rules = app_state.availability_service.list_rules().await?;
    Ok(ApiJson(ApiResponse::success(rules)))
}

/// Get an availability rule by ID.
pub async fn get_rule(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<AvailabilityRule>>, ApiError> {
    let rule = app_state
        .availability_service
        .get_rule(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(rule)))
}

/// Update an availability rule.
pub async fn update_rule(
    AuthUser(_user): AuthUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<UpdateAvailabilityRuleRequest>,
) -> Result<ApiJson<ApiResponse<AvailabilityRule>>, ApiError> {
    let rule = app_state
        .availability_service
        .update_rule(request)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(rule)))
}

/// Delete an availability rule.
pub async fn delete_rule(
    AuthUser(_user): AuthUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<()>>, ApiError> {
    let deleted = app_state.availability_service.delete_rule(id).await?;
    if deleted {
        Ok(ApiJson(ApiResponse::success(())))
    } else {
        Err(ApiError::NotFound)
    }
}

// ==================== ASSIGNMENT HANDLERS ====================

/// Assign (or remove) an availability rule on a restaurant.
pub async fn assign_to_restaurant(
    AuthUser(_user): AuthUser,
    State(app_state): State<AppState>,
    Path(restaurant_id): Path<Uuid>,
    ApiJson(request): ApiJson<AssignAvailabilityRequest>,
) -> Result<ApiJson<ApiResponse<()>>, ApiError> {
    let updated = app_state
        .availability_service
        .assign_to_restaurant(restaurant_id, request.availability_rule_id)
        .await?;
    if updated {
        Ok(ApiJson(ApiResponse::success(())))
    } else {
        Err(ApiError::NotFound)
    }
}

/// Get the availability rule for a restaurant.
pub async fn get_rule_for_restaurant(
    State(app_state): State<AppState>,
    Path(restaurant_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Option<AvailabilityRule>>>, ApiError> {
    let rule = app_state
        .availability_service
        .get_rule_for_restaurant(restaurant_id)
        .await?;
    Ok(ApiJson(ApiResponse::success(rule)))
}

/// Assign (or remove) an availability rule on a menu.
pub async fn assign_to_menu(
    AuthUser(_user): AuthUser,
    State(app_state): State<AppState>,
    Path(menu_id): Path<Uuid>,
    ApiJson(request): ApiJson<AssignAvailabilityRequest>,
) -> Result<ApiJson<ApiResponse<()>>, ApiError> {
    let updated = app_state
        .availability_service
        .assign_to_menu(menu_id, request.availability_rule_id)
        .await?;
    if updated {
        Ok(ApiJson(ApiResponse::success(())))
    } else {
        Err(ApiError::NotFound)
    }
}

/// Get the availability rule for a menu.
pub async fn get_rule_for_menu(
    State(app_state): State<AppState>,
    Path(menu_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Option<AvailabilityRule>>>, ApiError> {
    let rule = app_state
        .availability_service
        .get_rule_for_menu(menu_id)
        .await?;
    Ok(ApiJson(ApiResponse::success(rule)))
}

/// Assign (or remove) an availability rule on an item.
pub async fn assign_to_item(
    AuthUser(_user): AuthUser,
    State(app_state): State<AppState>,
    Path(item_id): Path<Uuid>,
    ApiJson(request): ApiJson<AssignAvailabilityRequest>,
) -> Result<ApiJson<ApiResponse<()>>, ApiError> {
    let updated = app_state
        .availability_service
        .assign_to_item(item_id, request.availability_rule_id)
        .await?;
    if updated {
        Ok(ApiJson(ApiResponse::success(())))
    } else {
        Err(ApiError::NotFound)
    }
}

/// Get the availability rule for an item.
pub async fn get_rule_for_item(
    State(app_state): State<AppState>,
    Path(item_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Option<AvailabilityRule>>>, ApiError> {
    let rule = app_state
        .availability_service
        .get_rule_for_item(item_id)
        .await?;
    Ok(ApiJson(ApiResponse::success(rule)))
}

/// Assign (or remove) an availability rule on an offer.
pub async fn assign_to_offer(
    AuthUser(_user): AuthUser,
    State(app_state): State<AppState>,
    Path(offer_id): Path<Uuid>,
    ApiJson(request): ApiJson<AssignAvailabilityRequest>,
) -> Result<ApiJson<ApiResponse<()>>, ApiError> {
    let updated = app_state
        .availability_service
        .assign_to_offer(offer_id, request.availability_rule_id)
        .await?;
    if updated {
        Ok(ApiJson(ApiResponse::success(())))
    } else {
        Err(ApiError::NotFound)
    }
}

/// Get the availability rule for an offer.
pub async fn get_rule_for_offer(
    State(app_state): State<AppState>,
    Path(offer_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Option<AvailabilityRule>>>, ApiError> {
    let rule = app_state
        .availability_service
        .get_rule_for_offer(offer_id)
        .await?;
    Ok(ApiJson(ApiResponse::success(rule)))
}