use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Router,
};
use uuid::Uuid;

use crate::{
    auth::middleware::{AuthUser, EditorUser},
    errors::{api_errors::ApiError, json_extractor::ApiJson},
    features::order::{
        domain::{
            order::Order,
            order_session::OrderSession,
            order_settings::RestaurantOrderSettings,
        },
        dto::{
            CreateOrderRequest, CreateOrderSessionRequest, MoveOrderToSessionRequest,
            OrderSessionStatusResponse, OrderSummary, UpdateOrderSessionRequest,
            UpdateOrderSettingsRequest,
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
        .route("/api/order-sessions/:id/request", post(request_session))
        .route("/api/order-sessions/:id/confirm", post(confirm_session))
        .route("/api/order-sessions/:id/finish", post(finish_session))
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
        .route(
            "/api/restaurants/:restaurant_id/order-sessions/open",
            get(list_open_sessions),
        )
        // ── Order routes ──────────────────────────────────────────
        .route("/api/orders", post(create_order))
        .route("/api/orders/:id", get(get_order))
        .route("/api/orders/:id", delete(delete_order))
        .route("/api/orders/:id/move-to-session", post(move_order_to_session))
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
        .route(
            "/api/restaurants/:restaurant_id/orders/mine",
            get(list_my_orders_in_open_sessions),
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

/// Update an order session's mutable fields (requires editor user)
pub async fn update_session(
    EditorUser(user): EditorUser,
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

/// Delete a cancelled order session (requires editor user)
pub async fn delete_session(
    EditorUser(_user): EditorUser,
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

/// Cancel an order session (requires editor user)
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

/// Close an order session — stop accepting new orders
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

/// Send an order request to the restaurant — transitions Closed → Requested
pub async fn request_session(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<OrderSessionStatusResponse>>, ApiError> {
    let session = app_state
        .order_service
        .request_session(id, user.id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(OrderSessionStatusResponse {
        session,
    })))
}

/// Confirm the restaurant will fulfil the order — transitions Closed/Requested → Confirmed
pub async fn confirm_session(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<OrderSessionStatusResponse>>, ApiError> {
    let session = app_state
        .order_service
        .confirm_session(id, user.id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(OrderSessionStatusResponse {
        session,
    })))
}

/// Mark a session as finished — transitions Confirmed → Finished
pub async fn finish_session(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<OrderSessionStatusResponse>>, ApiError> {
    let session = app_state
        .order_service
        .finish_session(id, user.id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(OrderSessionStatusResponse {
        session,
    })))
}

/// Reopen a closed session — resume accepting orders (requires editor user)
pub async fn reopen_session(
    EditorUser(user): EditorUser,
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

/// List all open sessions for a restaurant (by pickup time, ascending)
pub async fn list_open_sessions(
    State(app_state): State<AppState>,
    Path(restaurant_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Vec<OrderSession>>>, ApiError> {
    let sessions = app_state
        .order_service
        .list_open_sessions(restaurant_id)
        .await?;
    Ok(ApiJson(ApiResponse::success(sessions)))
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

/// Delete an order (only while the parent session is Open; user must own the order)
pub async fn delete_order(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<()>>, ApiError> {
    let deleted = app_state.order_service.delete_order(id, user.id, user.role).await?;
    if deleted {
        Ok(ApiJson(ApiResponse::success(())))
    } else {
        Err(ApiError::NotFound)
    }
}

/// Move an order to a different session
pub async fn move_order_to_session(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    Path(order_id): Path<Uuid>,
    ApiJson(request): ApiJson<MoveOrderToSessionRequest>,
) -> Result<ApiJson<ApiResponse<Order>>, ApiError> {
    let order = app_state
        .order_service
        .move_order_to_session(order_id, request.new_session_id, user.id, user.role)
        .await?;
    Ok(ApiJson(ApiResponse::success(order)))
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

/// List orders placed by the current user across all open sessions for a restaurant
pub async fn list_my_orders_in_open_sessions(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    Path(restaurant_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Vec<Order>>>, ApiError> {
    let orders = app_state
        .order_service
        .list_orders_by_user_in_open_sessions(restaurant_id, user.id)
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

/// Update order settings for a restaurant (requires editor user)
pub async fn update_order_settings(
    EditorUser(_user): EditorUser,
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
