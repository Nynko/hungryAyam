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
    features::order::{
        domain::{
            order::Order,
            order_session::OrderSession,
            order_settings::RestaurantOrderSettings,
        },
        dto::{
            CreateOrderRequest, CreateOrderSessionRequest, OrderSessionStatusResponse,
            OrderSummary, UpdateOrderSessionRequest, UpdateOrderSettingsRequest,
        },
    },
    state::AppState,
    types::response::ApiResponse,
};

pub fn order_routes() -> Router<AppState> {
    Router::new()
        // ── Order Session routes ──────────────────────────────────
        .route("/api/order-sessions", post(create_session))
        .route("/api/order-sessions/:id", get(get_session))
        .route("/api/update-order-session", post(update_session))
        .route("/api/order-sessions/:id", delete(delete_session))
        // Session lifecycle transitions
        .route("/api/order-sessions/:id/cancel", post(cancel_session))
        .route("/api/order-sessions/:id/close", post(close_session))
        .route("/api/order-sessions/:id/send", post(send_session))
        .route("/api/order-sessions/:id/reopen", post(reopen_session))
        // Session listing
        .route(
            "/api/restaurants/:restaurant_id/order-sessions",
            get(list_sessions_for_restaurant),
        )
        .route(
            "/api/restaurants/:restaurant_id/order-sessions/active",
            get(get_active_session),
        )
        // ── Order routes ──────────────────────────────────────────
        .route("/api/orders", post(create_order))
        .route("/api/orders/:id", get(get_order))
        .route("/api/orders/:id", delete(delete_order))
        .route(
            "/api/order-sessions/:session_id/orders",
            get(list_orders_for_session),
        )
        .route(
            "/api/order-sessions/:session_id/orders/summaries",
            get(list_order_summaries),
        )
        .route(
            "/api/order-sessions/:session_id/orders/mine",
            get(list_my_orders_in_session),
        )
        // ── Restaurant Order Settings routes ──────────────────────
        .route(
            "/api/restaurants/:restaurant_id/order-settings",
            get(get_order_settings),
        )
        .route(
            "/api/update-order-settings",
            post(update_order_settings),
        )
}

// ==================== ORDER SESSION HANDLERS ====================

/// Create a new order session (requires authenticated user)
pub async fn create_session(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<CreateOrderSessionRequest>,
) -> Result<(StatusCode, ApiJson<ApiResponse<OrderSession>>), ApiError> {
    let session = app_state
        .order_service
        .create_session(request, user.id)
        .await?;
    Ok((StatusCode::CREATED, ApiJson(ApiResponse::success(session))))
}

/// Get an order session by ID (with orders and items)
pub async fn get_session(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<OrderSession>>, ApiError> {
    let session = app_state
        .order_service
        .get_session_with_orders(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(session)))
}

/// Update an order session's mutable fields (requires authenticated user)
pub async fn update_session(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<UpdateOrderSessionRequest>,
) -> Result<ApiJson<ApiResponse<OrderSession>>, ApiError> {
    let session = app_state
        .order_service
        .update_session(request, user.id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(session)))
}

/// Delete a cancelled order session (requires authenticated user)
pub async fn delete_session(
    AuthUser(_user): AuthUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<()>>, ApiError> {
    let deleted = app_state.order_service.delete_session(id).await?;
    if deleted {
        Ok(ApiJson(ApiResponse::success(())))
    } else {
        Err(ApiError::NotFound)
    }
}

/// Cancel an order session (requires authenticated user)
pub async fn cancel_session(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<OrderSessionStatusResponse>>, ApiError> {
    let session = app_state
        .order_service
        .cancel_session(id, user.id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(OrderSessionStatusResponse {
        session,
    })))
}

/// Close an order session — stop accepting new orders (requires authenticated user)
pub async fn close_session(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<OrderSessionStatusResponse>>, ApiError> {
    let session = app_state
        .order_service
        .close_session(id, user.id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(OrderSessionStatusResponse {
        session,
    })))
}

/// Mark a session as sent — orders dispatched to restaurant (requires authenticated user)
pub async fn send_session(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<OrderSessionStatusResponse>>, ApiError> {
    let session = app_state
        .order_service
        .send_session(id, user.id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(OrderSessionStatusResponse {
        session,
    })))
}

/// Reopen a closed session — resume accepting orders (requires authenticated user)
pub async fn reopen_session(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<OrderSessionStatusResponse>>, ApiError> {
    let session = app_state
        .order_service
        .reopen_session(id, user.id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(OrderSessionStatusResponse {
        session,
    })))
}

/// List all sessions for a restaurant (most recent first)
pub async fn list_sessions_for_restaurant(
    State(app_state): State<AppState>,
    Path(restaurant_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Vec<OrderSession>>>, ApiError> {
    let sessions = app_state
        .order_service
        .list_sessions_by_restaurant(restaurant_id)
        .await?;
    Ok(ApiJson(ApiResponse::success(sessions)))
}

/// Get the currently active (Open) session for a restaurant
pub async fn get_active_session(
    State(app_state): State<AppState>,
    Path(restaurant_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Option<OrderSession>>>, ApiError> {
    let session = app_state
        .order_service
        .get_active_session(restaurant_id)
        .await?;
    Ok(ApiJson(ApiResponse::success(session)))
}

// ==================== ORDER HANDLERS ====================

/// Create a new order (requires authenticated user).
/// If no session_id is given, resolves or auto-creates a session.
pub async fn create_order(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<CreateOrderRequest>,
) -> Result<(StatusCode, ApiJson<ApiResponse<Order>>), ApiError> {
    let order = app_state
        .order_service
        .create_order(request, user.id)
        .await?;
    Ok((StatusCode::CREATED, ApiJson(ApiResponse::success(order))))
}

/// Get an order by ID (with items)
pub async fn get_order(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Order>>, ApiError> {
    let order = app_state
        .order_service
        .get_order(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(order)))
}

/// Delete an order (only while the parent session is Open; requires authenticated user)
pub async fn delete_order(
    AuthUser(_user): AuthUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<()>>, ApiError> {
    let deleted = app_state.order_service.delete_order(id).await?;
    if deleted {
        Ok(ApiJson(ApiResponse::success(())))
    } else {
        Err(ApiError::NotFound)
    }
}

/// List all orders in a session (with items)
pub async fn list_orders_for_session(
    State(app_state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Vec<Order>>>, ApiError> {
    let orders = app_state
        .order_service
        .list_orders_by_session(session_id)
        .await?;
    Ok(ApiJson(ApiResponse::success(orders)))
}

/// List lightweight order summaries for a session (no item details)
pub async fn list_order_summaries(
    State(app_state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Vec<OrderSummary>>>, ApiError> {
    let summaries = app_state
        .order_service
        .list_order_summaries(session_id)
        .await?;
    Ok(ApiJson(ApiResponse::success(summaries)))
}

/// List orders placed by the current user in a session (with items)
pub async fn list_my_orders_in_session(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Vec<Order>>>, ApiError> {
    let orders = app_state
        .order_service
        .list_orders_by_user_in_session(session_id, user.id)
        .await?;
    Ok(ApiJson(ApiResponse::success(orders)))
}

// ==================== ORDER SETTINGS HANDLERS ====================

/// Get order settings for a restaurant (creates defaults if none exist)
pub async fn get_order_settings(
    State(app_state): State<AppState>,
    Path(restaurant_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<RestaurantOrderSettings>>, ApiError> {
    let settings = app_state
        .order_service
        .get_order_settings(restaurant_id)
        .await?;
    Ok(ApiJson(ApiResponse::success(settings)))
}

/// Update order settings for a restaurant (requires authenticated user)
pub async fn update_order_settings(
    AuthUser(_user): AuthUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<UpdateOrderSettingsRequest>,
) -> Result<ApiJson<ApiResponse<RestaurantOrderSettings>>, ApiError> {
    let settings = app_state
        .order_service
        .update_order_settings(request)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(settings)))
}