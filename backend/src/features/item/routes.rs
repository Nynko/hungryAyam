use axum::{
    routing::{delete, get, post},
    Router,
    extract::{State, Path},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    auth::middleware::EditorUser,
    features::item::{
        domain::{
            item::Item,
            tag::{Tag, UpdateTag}
        },
        dto::{CreateItemRequest, UpdateItemRequest},
    },
    state::AppState,
    errors::{
        api_errors::ApiError,
        json_extractor::ApiJson,
    },
    types::response::ApiResponse,
};

pub fn item_routes() -> Router<AppState> {
    Router::new()
        // Item routes
        .route("/api/items", post(create_item))
        .route("/api/items/batch", post(create_batch_items))
        .route("/api/items/:id", get(get_item))
        .route("/api/items/:id", delete(delete_item))
        .route("/api/update-item", post(update_item))
        .route("/api/restaurants/:restaurant_id/items", get(list_items_for_restaurant))
        .route("/api/restaurants/:restaurant_id/items/active", get(list_active_items_for_restaurant))
        // Tag routes (read, update, delete - tags are created through item operations)
        .route("/api/tags", get(list_tags))
        .route("/api/tags/:id", get(get_tag))
        .route("/api/update-tag", post(update_tag))
        .route("/api/tags/:id", delete(delete_tag))
}

// ==================== ITEM HANDLERS ====================

/// Create a new item (with tags if provided; requires editor user)
pub async fn create_item(
    EditorUser(user): EditorUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<CreateItemRequest>,
) -> Result<(StatusCode, ApiJson<ApiResponse<Item>>), ApiError> {
    let item = app_state.item_service.create_item(user.id, request).await?;
    Ok((StatusCode::CREATED, ApiJson(ApiResponse::success(item))))
}

/// Create multiple items in batch (requires editor user)
pub async fn create_batch_items(
    EditorUser(user): EditorUser,
    State(app_state): State<AppState>,
    ApiJson(requests): ApiJson<Vec<CreateItemRequest>>,
) -> Result<(StatusCode, ApiJson<ApiResponse<Vec<Item>>>), ApiError> {
    let items = app_state.item_service.create_batch_items(user.id, requests).await?;
    Ok((StatusCode::CREATED, ApiJson(ApiResponse::success(items))))
}

/// Get an item by ID
pub async fn get_item(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Item>>, ApiError> {
    let item = app_state.item_service
        .get_item(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(item)))
}

/// List all items for a specific restaurant
pub async fn list_items_for_restaurant(
    State(app_state): State<AppState>,
    Path(restaurant_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Vec<Item>>>, ApiError> {
    let items = app_state.item_service.list_items_by_restaurant(restaurant_id).await?;
    Ok(ApiJson(ApiResponse::success(items)))
}

/// List only active items for a specific restaurant
pub async fn list_active_items_for_restaurant(
    State(app_state): State<AppState>,
    Path(restaurant_id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Vec<Item>>>, ApiError> {
    let items = app_state.item_service.list_active_items_by_restaurant(restaurant_id).await?;
    Ok(ApiJson(ApiResponse::success(items)))
}

/// Update an item (ID provided in request body, with tags if provided; requires editor user)
pub async fn update_item(
    EditorUser(user): EditorUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<UpdateItemRequest>,
) -> Result<ApiJson<ApiResponse<Item>>, ApiError> {
    let item = app_state.item_service
        .update_item(user.id,request)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(item)))
}

/// Delete an item (requires editor user)
pub async fn delete_item(
    EditorUser(_user): EditorUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<()>>, ApiError> {
    let deleted = app_state.item_service.delete_item(id).await?;
    if deleted {
        Ok(ApiJson(ApiResponse::success(())))
    } else {
        Err(ApiError::NotFound)
    }
}

// ==================== TAG HANDLERS ====================

/// List all tags
pub async fn list_tags(
    State(app_state): State<AppState>,
) -> Result<ApiJson<ApiResponse<Vec<Tag>>>, ApiError> {
    let tags = app_state.item_service.list_tags().await?;
    Ok(ApiJson(ApiResponse::success(tags)))
}

/// Get a tag by ID
pub async fn get_tag(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<Tag>>, ApiError> {
    let tag = app_state.item_service
        .get_tag(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(tag)))
}

/// Update a tag (ID provided in request body; requires editor user)
pub async fn update_tag(
    EditorUser(_user): EditorUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<UpdateTag>,
) -> Result<ApiJson<ApiResponse<Tag>>, ApiError> {
    let tag = app_state.item_service
        .update_tag(request)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(tag)))
}

/// Delete a tag (will cascade remove from all items; requires editor user)
pub async fn delete_tag(
    EditorUser(_user): EditorUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<()>>, ApiError> {
    let deleted = app_state.item_service.delete_tag(id).await?;
    if deleted {
        Ok(ApiJson(ApiResponse::success(())))
    } else {
        Err(ApiError::NotFound)
    }
}