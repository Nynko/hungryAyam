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
    features::menu::{
        domain::menu::Menu,
        dto::{
            CreateMenuRequest, ResetMenuRequest, ResetMenuResponse, UpdateMenuActionsRequest
        },
    },
    state::AppState,
    types::response::ApiResponse,
};


pub fn menu_routes() -> Router<AppState> {
    Router::new()
        // Menu CRUD routes
        .route("/api/menus", post(create_menu))
        .route("/api/menus/:id", get(get_menu))
        .route("/api/menus/:id", delete(delete_menu))
        .route("/api/update-menu", post(update_menu))
        .route("/api/reset-menu", post(reset_menu))
        // Menu listing routes
        .route("/api/restaurants/:restaurant_id/menus", get(list_menus_for_restaurant))
        .route("/api/restaurants/:restaurant_id/menus/active", get(list_active_menus_for_restaurant))
}

// ==================== MENU HANDLERS ====================

/// Create a new menu with sections and items (requires authenticated user)
pub async fn create_menu(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<CreateMenuRequest>,
) -> Result<(StatusCode, ApiJson<ApiResponse<Menu>>), ApiError> {
    let menu = app_state.menu_service.create_menu(request, user.id).await?;
    Ok((StatusCode::CREATED, ApiJson(ApiResponse::success(menu))))
}

/// Get a menu by ID with full structure
pub async fn get_menu(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Menu>>, ApiError> {
    let menu = app_state
        .menu_service
        .get_menu(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(menu)))
}

/// List all menus for a restaurant
pub async fn list_menus_for_restaurant(
    State(app_state): State<AppState>,
    Path(restaurant_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Vec<Menu>>>, ApiError> {
    let menus = app_state
        .menu_service
        .list_menus_by_restaurant(restaurant_id)
        .await?;
    Ok(ApiJson(ApiResponse::success(menus)))
}

/// List only active menus for a restaurant
pub async fn list_active_menus_for_restaurant(
    State(app_state): State<AppState>,
    Path(restaurant_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Vec<Menu>>>, ApiError> {
    let menus = app_state
        .menu_service
        .list_active_menus_by_restaurant(restaurant_id)
        .await?;
    Ok(ApiJson(ApiResponse::success(menus)))
}

/// Update a menu with actions (requires authenticated user)
pub async fn update_menu(
    AuthUser(user): AuthUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<UpdateMenuActionsRequest>,
) -> Result<ApiJson<ApiResponse<Menu>>, ApiError> {
    let menu = app_state
        .menu_service
        .update_menu(request, user.id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(menu)))
}

/// Reset a non-permanent menu - sets all items to is_available = false
/// This keeps items in the "candidate pool" for easy re-selection
/// (requires editor user)
pub async fn reset_menu(
    EditorUser(user): EditorUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<ResetMenuRequest>,
) -> Result<ApiJson<ApiResponse<ResetMenuResponse>>, ApiError> {
    let items_reset = app_state
        .menu_service
        .reset_menu(request.id, user.id)
        .await?
        .ok_or(ApiError::NotFound)?;

    Ok(ApiJson(ApiResponse::success(ResetMenuResponse {
        menu_id: request.id,
        items_reset,
    })))
}

/// Delete a menu (requires editor user)
pub async fn delete_menu(
    EditorUser(_user): EditorUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<()>>, ApiError> {
    let deleted = app_state.menu_service.delete_menu(id).await?;
    if deleted {
        Ok(ApiJson(ApiResponse::success(())))
    } else {
        Err(ApiError::NotFound)
    }
}