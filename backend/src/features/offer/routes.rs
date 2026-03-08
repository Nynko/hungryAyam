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
    features::offer::{
        domain::Offer,
        dto::{CreateOfferRequest, UpdateOfferRequest},
    },
    state::AppState,
    types::response::ApiResponse,
};

pub fn offer_routes() -> Router<AppState> {
    Router::new()
        // ── CRUD ──────────────────────────────────────────────────
        .route("/api/offers", post(create_offer))
        .route("/api/offers/:id", get(get_offer))
        .route("/api/offers/:id", delete(delete_offer))
        .route("/api/update-offer", post(update_offer))
        // ── Listing ───────────────────────────────────────────────
        .route(
            "/api/restaurants/:restaurant_id/offers",
            get(list_offers_for_restaurant),
        )
        .route(
            "/api/restaurants/:restaurant_id/offers/active",
            get(list_active_offers_for_restaurant),
        )
        // ── Activation toggle ─────────────────────────────────────
        .route("/api/offers/:id/activate", post(activate_offer))
        .route("/api/offers/:id/deactivate", post(deactivate_offer))
        // ── Slot helpers ──────────────────────────────────────────
        .route(
            "/api/offer-slots/:slot_id/allowed-items",
            get(get_allowed_items_for_slot),
        )
        // ── Order validation ──────────────────────────────────────
        .route(
            "/api/offers/:id/validate-selection",
            post(validate_selection),
        )
}

// ==================== CRUD HANDLERS ====================

/// Create a new offer with nested slots and constraints (requires authenticated user).
pub async fn create_offer(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<CreateOfferRequest>,
) -> Result<(StatusCode, ApiJson<ApiResponse<Offer>>), ApiError> {
    let offer = app_state
        .offer_service
        .create_offer(request, user.id)
        .await?;
    Ok((StatusCode::CREATED, ApiJson(ApiResponse::success(offer))))
}

/// Get an offer by ID (with slots and constraints).
pub async fn get_offer(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Offer>>, ApiError> {
    let offer = app_state
        .offer_service
        .get_offer(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(offer)))
}

/// Update an offer (partial top-level fields; replace-all for slots if provided).
/// Requires authenticated user.
pub async fn update_offer(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<UpdateOfferRequest>,
) -> Result<ApiJson<ApiResponse<Offer>>, ApiError> {
    let offer = app_state
        .offer_service
        .update_offer(request, user.id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(offer)))
}

/// Delete an offer by ID (requires authenticated user).
pub async fn delete_offer(
    AuthUser(_user): AuthUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<()>>, ApiError> {
    let deleted = app_state.offer_service.delete_offer(id).await?;
    if deleted {
        Ok(ApiJson(ApiResponse::success(())))
    } else {
        Err(ApiError::NotFound)
    }
}

// ==================== LISTING HANDLERS ====================

/// List all offers for a restaurant (with slots and constraints).
pub async fn list_offers_for_restaurant(
    State(app_state): State<AppState>,
    Path(restaurant_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Vec<Offer>>>, ApiError> {
    let offers = app_state
        .offer_service
        .list_offers_by_restaurant(restaurant_id)
        .await?;
    Ok(ApiJson(ApiResponse::success(offers)))
}

/// List only active offers for a restaurant (with slots and constraints).
pub async fn list_active_offers_for_restaurant(
    State(app_state): State<AppState>,
    Path(restaurant_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Vec<Offer>>>, ApiError> {
    let offers = app_state
        .offer_service
        .list_active_offers_by_restaurant(restaurant_id)
        .await?;
    Ok(ApiJson(ApiResponse::success(offers)))
}

// ==================== ACTIVATION HANDLERS ====================

/// Activate an offer (set is_active = true). Requires authenticated user.
pub async fn activate_offer(
    AuthUser(_user): AuthUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Offer>>, ApiError> {
    let offer = app_state
        .offer_service
        .activate_offer(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(offer)))
}

/// Deactivate an offer (set is_active = false). Requires authenticated user.
pub async fn deactivate_offer(
    AuthUser(_user): AuthUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Offer>>, ApiError> {
    let offer = app_state
        .offer_service
        .deactivate_offer(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(offer)))
}

// ==================== SLOT HELPERS ====================

/// Get the resolved list of allowed item IDs for a specific offer slot.
/// Resolves item, tag, and section constraints into concrete item IDs.
pub async fn get_allowed_items_for_slot(
    State(app_state): State<AppState>,
    Path(slot_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Vec<Uuid>>>, ApiError> {
    let item_ids = app_state
        .offer_service
        .get_allowed_items_for_slot(slot_id)
        .await?;
    Ok(ApiJson(ApiResponse::success(item_ids)))
}

// ==================== ORDER VALIDATION ====================

/// Request body for validate-selection endpoint.
#[derive(Debug, Clone, serde::Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ValidateOfferSelectionRequest {
    pub restaurant_id: Uuid,
    /// Each entry is (item_id, slot_id).
    pub selections: Vec<OfferSelectionEntry>,
}

#[derive(Debug, Clone, serde::Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct OfferSelectionEntry {
    pub item_id: Uuid,
    pub slot_id: Uuid,
}

/// Response for validate-selection endpoint.
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ValidateOfferSelectionResponse {
    pub valid: bool,
    /// The offer's base price (before supplements).
    pub base_price_cents: i32,
    /// The fully computed price including slot and constraint supplements.
    pub total_price_cents: i32,
}

/// Validate a user's offer slot selections without creating an order.
/// Returns whether the selection is valid, the base price, and the computed total
/// (including slot and constraint supplements).
pub async fn validate_selection(
    State(app_state): State<AppState>,
    Path(offer_id): Path<Uuid>,
    ApiJson(request): ApiJson<ValidateOfferSelectionRequest>,
) -> Result<ApiJson<ApiResponse<ValidateOfferSelectionResponse>>, ApiError> {
    let items: Vec<(Uuid, Option<Uuid>)> = request
        .selections
        .iter()
        .map(|s| (s.item_id, Some(s.slot_id)))
        .collect();

    let offer = app_state
        .offer_service
        .validate_offer_order(offer_id, request.restaurant_id, &items)
        .await?;

    let total_price_cents = app_state
        .offer_service
        .compute_offer_price(&offer, &items)
        .await?;

    Ok(ApiJson(ApiResponse::success(
        ValidateOfferSelectionResponse {
            valid: true,
            base_price_cents: *offer.base_price_cents,
            total_price_cents,
        },
    )))
}