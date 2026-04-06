use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::Notify;
use tower::ServiceExt;
use uuid::Uuid;

use crate::{app::build_app, utils::password::sha256_hex, state::build_state};

// ═══════════════════════════════════════════════════════════════════
// Test helpers
// ═══════════════════════════════════════════════════════════════════

/// Seed the database with app_settings, a user, and a restaurant.
/// Returns (user_id, restaurant_id).
async fn seed(pool: &PgPool) -> (Uuid, Uuid, String) {
    let access_hash = sha256_hex("test_access_code");
    sqlx::query("INSERT INTO app_settings (id, access_hash) VALUES (1, $1)")
        .bind(&access_hash)
        .execute(pool)
        .await
        .unwrap();

    let user_id: Uuid =
        sqlx::query_scalar("INSERT INTO users (name, auth_method, role) VALUES ('tester', 'Password', 'Editor') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();

    let token: String = sqlx::query_scalar(
        "INSERT INTO user_sessions (user_id, token, expires_at) \
         VALUES ($1, gen_random_uuid()::text, NOW() + INTERVAL '1 day') \
         RETURNING token",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let rest_id: Uuid = sqlx::query_scalar(
        "INSERT INTO restaurants (name, created_by, updated_by) VALUES ('Test Restaurant', $1, $1) RETURNING id",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();

    (user_id, rest_id, token)
}

/// Seed a second restaurant for cross-restaurant tests.
async fn seed_second_restaurant(pool: &PgPool, user_id: Uuid) -> Uuid {
    let rest_id: Uuid = sqlx::query_scalar(
        "INSERT INTO restaurants (name, created_by, updated_by) VALUES ('Second Restaurant', $1, $1) RETURNING id",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    rest_id
}

/// Build a fully-wired Router backed by the given pool.
fn app(pool: PgPool) -> Router {
    let setup_done = Arc::new(AtomicBool::new(true));
    let state = build_state(setup_done, pool, Arc::new(Notify::new()), std::path::PathBuf::from("/tmp/test_uploads"));
    build_app(state)
}

/// Fire a request and return (status, parsed JSON body).
async fn send(router: &Router, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    send_with_auth(router, method, uri, body, None).await
}

async fn send_with_auth(router: &Router, method: Method, uri: &str, body: Option<Value>, token: Option<&str>) -> (StatusCode, Value) {
    let req_body = match body {
        Some(v) => Body::from(serde_json::to_vec(&v).unwrap()),
        None => Body::empty(),
    };
    let mut builder = Request::builder().method(method).uri(uri).header("content-type", "application/json");
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {}", t));
    }
    let req = builder.body(req_body).unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(json!(null));
    (status, json)
}

async fn post(router: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    send(router, Method::POST, uri, Some(body)).await
}

async fn post_auth(router: &Router, uri: &str, body: Value, token: &str) -> (StatusCode, Value) {
    send_with_auth(router, Method::POST, uri, Some(body), Some(token)).await
}

async fn get(router: &Router, uri: &str) -> (StatusCode, Value) {
    send(router, Method::GET, uri, None).await
}

async fn get_auth(router: &Router, uri: &str, token: &str) -> (StatusCode, Value) {
    send_with_auth(router, Method::GET, uri, None, Some(token)).await
}

async fn del(router: &Router, uri: &str) -> (StatusCode, Value) {
    send(router, Method::DELETE, uri, None).await
}

async fn del_auth(router: &Router, uri: &str, token: &str) -> (StatusCode, Value) {
    send_with_auth(router, Method::DELETE, uri, None, Some(token)).await
}

/// Create a simple item via the API and return the response data object.
async fn create_item(
    router: &Router,
    restaurant_id: Uuid,
    name: &str,
    price_cents: i32,
    token: &str,
) -> Value {
    let payload = json!({
        "restaurant_id": restaurant_id,
        "name": name,
        "description": null,
        "base_price_cents": price_cents,
        "image_url": null,
        "tags": []
    });
    let (status, body) = post_auth(router, "/api/items", payload, token).await;
    assert_eq!(status, StatusCode::CREATED, "create item failed: {body}");
    assert_eq!(body["success"], true);
    body["data"].clone()
}

/// Create an item with tags via the API and return the response data object.
async fn create_item_with_tags(
    router: &Router,
    restaurant_id: Uuid,
    name: &str,
    price_cents: i32,
    tags: Value,
    token: &str,
) -> Value {
    let payload = json!({
        "restaurant_id": restaurant_id,
        "name": name,
        "description": "A delicious dish",
        "base_price_cents": price_cents,
        "image_url": "https://example.com/food.jpg",
        "tags": tags
    });
    let (status, body) = post_auth(router, "/api/items", payload, token).await;
    assert_eq!(status, StatusCode::CREATED, "create item with tags failed: {body}");
    assert_eq!(body["success"], true);
    body["data"].clone()
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Item Create
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_create_item_minimal(pool: PgPool) {
    let (user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "restaurant_id": restaurant_id,
        "name": "Nasi Goreng",
        "description": null,
        "base_price_cents": 1500,
        "image_url": null,
        "tags": []
    });

    let (status, body) = post_auth(&router, "/api/items", payload, &token).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["success"], true);

    let data = &body["data"];
    assert!(data["id"].is_string());
    assert_eq!(data["restaurant_id"], restaurant_id.to_string());
    assert_eq!(data["name"], "Nasi Goreng");
    assert_eq!(data["description"], Value::Null);
    assert_eq!(data["base_price_cents"], 1500);
    assert_eq!(data["image_url"], Value::Null);
    assert_eq!(data["active"], true); // default
    assert_eq!(data["created_by"], user_id.to_string());
    assert_eq!(data["updated_by"], user_id.to_string());
    assert!(data["created_at"].is_string());
    assert!(data["updated_at"].is_string());
    assert!(data["tags"].as_array().unwrap().is_empty());
}

#[sqlx::test]
async fn test_create_item_with_all_fields(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "restaurant_id": restaurant_id,
        "name": "Mie Goreng",
        "description": "Indonesian fried noodles",
        "base_price_cents": 1200,
        "image_url": "https://example.com/mie.jpg",
        "tags": []
    });

    let (status, body) = post_auth(&router, "/api/items", payload, &token).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let data = &body["data"];
    assert_eq!(data["name"], "Mie Goreng");
    assert_eq!(data["description"], "Indonesian fried noodles");
    assert_eq!(data["base_price_cents"], 1200);
    assert_eq!(data["image_url"], "https://example.com/mie.jpg");
}

#[sqlx::test]
async fn test_create_item_with_tags_by_name(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let tags = json!([
        { "New": "Spicy" },
        { "New": "Vegetarian" }
    ]);

    let data = create_item_with_tags(
        &router,
        restaurant_id,
        "Gado Gado",
        1000,
        tags,
        &token,
    )
    .await;

    let item_tags = data["tags"].as_array().unwrap();
    assert_eq!(item_tags.len(), 2);

    let tag_names: Vec<&str> = item_tags.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(tag_names.contains(&"Spicy"));
    assert!(tag_names.contains(&"Vegetarian"));

    // Each tag should have an id
    for tag in item_tags {
        assert!(tag["id"].is_string());
    }
}

#[sqlx::test]
async fn test_create_item_with_invalid_url_rejected(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "restaurant_id": restaurant_id,
        "name": "Bad URL Item",
        "description": null,
        "base_price_cents": 500,
        "image_url": "not-a-url",
        "tags": []
    });

    let (status, _body) = post_auth(&router, "/api/items", payload, &token).await;
    assert_ne!(status, StatusCode::CREATED);
}

#[sqlx::test]
async fn test_create_item_with_negative_price_rejected(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "restaurant_id": restaurant_id,
        "name": "Negative Price",
        "description": null,
        "base_price_cents": -100,
        "image_url": null,
        "tags": []
    });

    let (status, _body) = post_auth(&router, "/api/items", payload, &token).await;
    assert_ne!(status, StatusCode::CREATED, "should reject negative price");
}

#[sqlx::test]
async fn test_create_item_with_zero_price(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "restaurant_id": restaurant_id,
        "name": "Free Item",
        "description": null,
        "base_price_cents": 0,
        "image_url": null,
        "tags": []
    });

    let (status, body) = post_auth(&router, "/api/items", payload, &token).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["base_price_cents"], 0);
}

#[sqlx::test]
async fn test_create_item_tag_deduplication_by_name(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    // Create first item with tag "Halal"
    let data1 = create_item_with_tags(
        &router,
        restaurant_id,
        "Item A",
        500,
        json!([{ "New": "Halal" }]),
        &token,
    )
    .await;
    let tag_id_1 = data1["tags"][0]["id"].as_str().unwrap().to_string();

    // Create second item with same tag name "Halal" — should reuse the same tag
    let data2 = create_item_with_tags(
        &router,
        restaurant_id,
        "Item B",
        600,
        json!([{ "New": "Halal" }]),
        &token,
    )
    .await;
    let tag_id_2 = data2["tags"][0]["id"].as_str().unwrap().to_string();

    assert_eq!(tag_id_1, tag_id_2, "same tag name should produce the same tag id");
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Item Create Batch
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_create_batch_items(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let payload = json!([
        {
            "restaurant_id": restaurant_id,
            "name": "Batch Item 1",
            "description": null,
            "base_price_cents": 100,
            "image_url": null,
            "tags": []
        },
        {
            "restaurant_id": restaurant_id,
            "name": "Batch Item 2",
            "description": "Second item",
            "base_price_cents": 200,
            "image_url": null,
            "tags": [{ "New": "New" }]
        },
        {
            "restaurant_id": restaurant_id,
            "name": "Batch Item 3",
            "description": null,
            "base_price_cents": 300,
            "image_url": null,
            "tags": []
        }
    ]);

    let (status, body) = post_auth(&router, "/api/items/batch", payload, &token).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["success"], true);

    let items = body["data"].as_array().unwrap();
    assert_eq!(items.len(), 3);

    assert_eq!(items[0]["name"], "Batch Item 1");
    assert_eq!(items[0]["base_price_cents"], 100);

    assert_eq!(items[1]["name"], "Batch Item 2");
    assert_eq!(items[1]["base_price_cents"], 200);
    assert_eq!(items[1]["tags"].as_array().unwrap().len(), 1);

    assert_eq!(items[2]["name"], "Batch Item 3");
    assert_eq!(items[2]["base_price_cents"], 300);
}

#[sqlx::test]
async fn test_create_batch_items_empty(pool: PgPool) {
    let (_user_id, _restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let payload = json!([]);

    let (status, body) = post_auth(&router, "/api/items/batch", payload, &token).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["success"], true);
    assert!(body["data"].as_array().unwrap().is_empty());
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Item Get
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_get_item_by_id(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_item(&router, restaurant_id, "Sate Ayam", 2000, &token).await;
    let item_id = created["id"].as_str().unwrap();

    let (status, body) = get_auth(&router, &format!("/api/items/{item_id}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);

    let data = &body["data"];
    assert_eq!(data["id"], item_id);
    assert_eq!(data["name"], "Sate Ayam");
    assert_eq!(data["base_price_cents"], 2000);
    assert_eq!(data["restaurant_id"], restaurant_id.to_string());
}

#[sqlx::test]
async fn test_get_item_not_found(pool: PgPool) {
    let (_u, _r, token) = seed(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let (status, _body) = get_auth(&router, &format!("/api/items/{fake_id}"), &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_get_item_includes_tags(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_item_with_tags(
        &router,
        restaurant_id,
        "Tagged Item",
        1500,
        json!([{ "New": "Spicy" }, { "New": "Chicken" }]),
        &token,
    )
    .await;
    let item_id = created["id"].as_str().unwrap();

    let (status, body) = get_auth(&router, &format!("/api/items/{item_id}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let tags = body["data"]["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 2);

    let tag_names: Vec<&str> = tags.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(tag_names.contains(&"Spicy"));
    assert!(tag_names.contains(&"Chicken"));
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Item List
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_list_items_for_restaurant_empty(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let (status, body) = get_auth(&router, &format!("/api/restaurants/{restaurant_id}/items"), &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    assert!(body["data"].as_array().unwrap().is_empty());
}

#[sqlx::test]
async fn test_list_items_for_restaurant(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    create_item(&router, restaurant_id, "Alpha", 100, &token).await;
    create_item(&router, restaurant_id, "Beta", 200, &token).await;
    create_item(&router, restaurant_id, "Gamma", 300, &token).await;

    let (status, body) = get_auth(&router, &format!("/api/restaurants/{restaurant_id}/items"), &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);

    let items = body["data"].as_array().unwrap();
    assert_eq!(items.len(), 3);

    // Items are ordered by name ASC
    assert_eq!(items[0]["name"], "Alpha");
    assert_eq!(items[1]["name"], "Beta");
    assert_eq!(items[2]["name"], "Gamma");
}

#[sqlx::test]
async fn test_list_items_scoped_to_restaurant(pool: PgPool) {
    let (user_id, restaurant_id, token) = seed(&pool).await;
    let second_restaurant_id = seed_second_restaurant(&pool, user_id).await;
    let router = app(pool);

    create_item(&router, restaurant_id, "Rest1 Item", 100, &token).await;
    create_item(&router, second_restaurant_id, "Rest2 Item", 200, &token).await;

    // List items for restaurant 1
    let (status, body) = get_auth(&router, &format!("/api/restaurants/{restaurant_id}/items"), &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let items = body["data"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "Rest1 Item");

    // List items for restaurant 2
    let (status, body) = get_auth(
        &router,
        &format!("/api/restaurants/{second_restaurant_id}/items"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let items = body["data"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "Rest2 Item");
}

#[sqlx::test]
async fn test_list_items_includes_tags(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    create_item_with_tags(
        &router,
        restaurant_id,
        "TaggedList Item",
        500,
        json!([{ "New": "Vegan" }]),
        &token,
    )
    .await;

    let (status, body) = get_auth(&router, &format!("/api/restaurants/{restaurant_id}/items"), &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let items = body["data"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    let tags = items[0]["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0]["name"], "Vegan");
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Item List Active
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_list_active_items_for_restaurant(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    // Create two items, both active by default
    create_item(&router, restaurant_id, "Active Item", 100, &token).await;
    let inactive = create_item(&router, restaurant_id, "To Deactivate", 200, &token).await;
    let inactive_id = inactive["id"].as_str().unwrap();

    // Deactivate one item
    let update_payload = json!({
        "id": inactive_id,
        "active": false,
    });
    let (status, _body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK);

    // List all items
    let (status, body) = get_auth(&router, &format!("/api/restaurants/{restaurant_id}/items"), &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().unwrap().len(), 2);

    // List only active items
    let (status, body) = get_auth(
        &router,
        &format!("/api/restaurants/{restaurant_id}/items/active"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let items = body["data"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "Active Item");
}

#[sqlx::test]
async fn test_list_active_items_empty_when_all_inactive(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let item = create_item(&router, restaurant_id, "Only Item", 100, &token).await;
    let item_id = item["id"].as_str().unwrap();

    // Deactivate it
    let update_payload = json!({
        "id": item_id,
        "active": false,
    });
    let (status, _body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = get_auth(
        &router,
        &format!("/api/restaurants/{restaurant_id}/items/active"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["data"].as_array().unwrap().is_empty());
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Item Update
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_update_item_name(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_item(&router, restaurant_id, "Old Name", 1000, &token).await;
    let item_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": item_id,
        "name": "New Name",
    });

    let (status, body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["name"], "New Name");
    // Price should remain unchanged
    assert_eq!(body["data"]["base_price_cents"], 1000);
}

#[sqlx::test]
async fn test_update_item_price(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_item(&router, restaurant_id, "Price Update", 500, &token).await;
    let item_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": item_id,
        "base_price_cents": 750,
    });

    let (status, body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["base_price_cents"], 750);
    assert_eq!(body["data"]["name"], "Price Update");
}

#[sqlx::test]
async fn test_update_item_description(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_item(&router, restaurant_id, "Desc Update", 500, &token).await;
    let item_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": item_id,
        "description": "Brand new description",
    });

    let (status, body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["description"], "Brand new description");
}

#[sqlx::test]
async fn test_update_item_image_url(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_item(&router, restaurant_id, "Image Update", 500, &token).await;
    let item_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": item_id,
        "image_url": "https://example.com/new-image.png",
    });

    let (status, body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["image_url"], "https://example.com/new-image.png");
}

#[sqlx::test]
async fn test_update_item_active_flag(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_item(&router, restaurant_id, "Toggle Active", 500, &token).await;
    let item_id = created["id"].as_str().unwrap();
    assert_eq!(created["active"], true);

    // Deactivate
    let update_payload = json!({
        "id": item_id,
        "active": false,
    });
    let (status, body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["active"], false);

    // Reactivate
    let update_payload = json!({
        "id": item_id,
        "active": true,
    });
    let (status, body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["active"], true);
}

#[sqlx::test]
async fn test_update_item_multiple_fields(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_item(&router, restaurant_id, "Multi Update", 500, &token).await;
    let item_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": item_id,
        "name": "Updated Multi",
        "description": "Updated description",
        "base_price_cents": 999,
        "image_url": "https://example.com/updated.png",
        "active": false,
    });

    let (status, body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let data = &body["data"];
    assert_eq!(data["name"], "Updated Multi");
    assert_eq!(data["description"], "Updated description");
    assert_eq!(data["base_price_cents"], 999);
    assert_eq!(data["image_url"], "https://example.com/updated.png");
    assert_eq!(data["active"], false);
}

#[sqlx::test]
async fn test_update_item_not_found(pool: PgPool) {
    let (_user_id, _restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let update_payload = json!({
        "id": fake_id,
        "name": "Ghost",
    });

    let (status, _body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_update_item_with_invalid_url_rejected(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_item(&router, restaurant_id, "Bad URL Update", 500, &token).await;
    let item_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": item_id,
        "image_url": "not-a-url",
    });

    let (status, _body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_ne!(status, StatusCode::OK, "should reject invalid URL");
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Item Update with Tags
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_update_item_add_tags(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    // Create item without tags
    let created = create_item(&router, restaurant_id, "Add Tags", 500, &token).await;
    let item_id = created["id"].as_str().unwrap();
    assert!(created["tags"].as_array().unwrap().is_empty());

    // Update with tags
    let update_payload = json!({
        "id": item_id,
        "tags": [{ "New": "Spicy" }, { "New": "Hot" }],
    });

    let (status, body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let tags = body["data"]["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 2);
    let tag_names: Vec<&str> = tags.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(tag_names.contains(&"Spicy"));
    assert!(tag_names.contains(&"Hot"));
}

#[sqlx::test]
async fn test_update_item_replace_tags(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    // Create item with initial tags
    let created = create_item_with_tags(
        &router,
        restaurant_id,
        "Replace Tags",
        500,
        json!([{ "New": "OldTag1" }, { "New": "OldTag2" }]),
        &token,
    )
    .await;
    let item_id = created["id"].as_str().unwrap();
    assert_eq!(created["tags"].as_array().unwrap().len(), 2);

    // Replace with completely new tags
    let update_payload = json!({
        "id": item_id,
        "tags": [{ "New": "NewTag1" }, { "New": "NewTag2" }, { "New": "NewTag3" }],
    });

    let (status, body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let tags = body["data"]["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 3);
    let tag_names: Vec<&str> = tags.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(tag_names.contains(&"NewTag1"));
    assert!(tag_names.contains(&"NewTag2"));
    assert!(tag_names.contains(&"NewTag3"));
    assert!(!tag_names.contains(&"OldTag1"));
    assert!(!tag_names.contains(&"OldTag2"));
}

#[sqlx::test]
async fn test_update_item_remove_all_tags(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    // Create item with tags
    let created = create_item_with_tags(
        &router,
        restaurant_id,
        "Remove Tags",
        500,
        json!([{ "New": "ToRemove" }]),
        &token,
    )
    .await;
    let item_id = created["id"].as_str().unwrap();
    assert_eq!(created["tags"].as_array().unwrap().len(), 1);

    // Update with empty tags array to remove all
    let update_payload = json!({
        "id": item_id,
        "tags": [],
    });

    let (status, body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["data"]["tags"].as_array().unwrap().is_empty());
}

#[sqlx::test]
async fn test_update_item_without_tags_field_preserves_existing_tags(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    // Create item with tags
    let created = create_item_with_tags(
        &router,
        restaurant_id,
        "Preserve Tags",
        500,
        json!([{ "New": "Keep Me" }]),
        &token,
    )
    .await;
    let item_id = created["id"].as_str().unwrap();

    // Update without specifying tags at all — tags should be preserved
    let update_payload = json!({
        "id": item_id,
        "name": "Renamed But Tags Stay",
    });

    let (status, body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["name"], "Renamed But Tags Stay");

    let tags = body["data"]["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0]["name"], "Keep Me");
}

#[sqlx::test]
async fn test_update_item_tags_by_existing_id(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    // Create an item with a tag to get the tag's ID
    let first = create_item_with_tags(
        &router,
        restaurant_id,
        "Tag Source",
        500,
        json!([{ "New": "SharedTag" }]),
        &token,
    )
    .await;
    let tag_id = first["tags"][0]["id"].as_str().unwrap().to_string();

    // Create another item without tags
    let second = create_item(&router, restaurant_id, "Tag Target", 600, &token).await;
    let second_id = second["id"].as_str().unwrap();

    // Update second item to use the existing tag by ID
    let update_payload = json!({
        "id": second_id,
        "tags": [{ "Existing": tag_id }],
    });

    let (status, body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let tags = body["data"]["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0]["id"], tag_id);
    assert_eq!(tags[0]["name"], "SharedTag");
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Item Delete
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_delete_item(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_item(&router, restaurant_id, "To Delete", 500, &token).await;
    let item_id = created["id"].as_str().unwrap();

    let (status, body) = del_auth(&router, &format!("/api/items/{item_id}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);

    // Verify it's gone
    let (status, _body) = get_auth(&router, &format!("/api/items/{item_id}"), &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_delete_item_not_found(pool: PgPool) {
    let (_u, _r, token) = seed(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let (status, _body) = del_auth(&router, &format!("/api/items/{fake_id}"), &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_delete_item_idempotent(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_item(&router, restaurant_id, "Delete Twice", 500, &token).await;
    let item_id = created["id"].as_str().unwrap();

    let (status, _body) = del_auth(&router, &format!("/api/items/{item_id}"), &token).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _body) = del_auth(&router, &format!("/api/items/{item_id}"), &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_delete_item_removes_from_list(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let _keep = create_item(&router, restaurant_id, "Keep Me", 100, &token).await;
    let remove = create_item(&router, restaurant_id, "Remove Me", 200, &token).await;
    let remove_id = remove["id"].as_str().unwrap();

    // Verify both listed
    let (status, body) = get_auth(&router, &format!("/api/restaurants/{restaurant_id}/items"), &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().unwrap().len(), 2);

    // Delete one
    let (status, _body) = del_auth(&router, &format!("/api/items/{remove_id}"), &token).await;
    assert_eq!(status, StatusCode::OK);

    // Verify only one remains
    let (status, body) = get_auth(&router, &format!("/api/restaurants/{restaurant_id}/items"), &token).await;
    assert_eq!(status, StatusCode::OK);
    let items = body["data"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "Keep Me");
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Tag CRUD
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_list_tags_empty(pool: PgPool) {
    let (_u, _r, token) = seed(&pool).await;
    let router = app(pool);

    let (status, body) = get_auth(&router, "/api/tags", &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    assert!(body["data"].as_array().unwrap().is_empty());
}

#[sqlx::test]
async fn test_list_tags_after_item_creation(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    // Create items with tags to populate the tags table
    create_item_with_tags(
        &router,
        restaurant_id,
        "Item1",
        500,
        json!([{ "New": "Chicken" }, { "New": "Spicy" }]),
        &token,
    )
    .await;

    create_item_with_tags(
        &router,
        restaurant_id,
        "Item2",
        600,
        json!([{ "New": "Vegetarian" }, { "New": "Spicy" }]),  // Spicy is shared
        &token,
    )
    .await;

    let (status, body) = get_auth(&router, "/api/tags", &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let tags = body["data"].as_array().unwrap();
    assert_eq!(tags.len(), 3); // Chicken, Spicy, Vegetarian (deduplicated)

    // Tags are ordered by name ASC
    let tag_names: Vec<&str> = tags.iter().filter_map(|t| t["name"].as_str()).collect();
    assert_eq!(tag_names, vec!["Chicken", "Spicy", "Vegetarian"]);
}

#[sqlx::test]
async fn test_get_tag_by_id(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let item = create_item_with_tags(
        &router,
        restaurant_id,
        "Tagged",
        500,
        json!([{ "New": "Halal" }]),
        &token,
    )
    .await;
    let tag_id = item["tags"][0]["id"].as_str().unwrap();

    let (status, body) = get_auth(&router, &format!("/api/tags/{tag_id}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["id"], tag_id);
    assert_eq!(body["data"]["name"], "Halal");
}

#[sqlx::test]
async fn test_get_tag_not_found(pool: PgPool) {
    let (_u, _r, token) = seed(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let (status, _body) = get_auth(&router, &format!("/api/tags/{fake_id}"), &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_update_tag_rename(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let item = create_item_with_tags(
        &router,
        restaurant_id,
        "Tag Rename Item",
        500,
        json!([{ "New": "OldTagName" }]),
        &token,
    )
    .await;
    let tag_id = item["tags"][0]["id"].as_str().unwrap();

    let update_payload = json!({
        "id": tag_id,
        "name": "NewTagName"
    });

    let (status, body) = post_auth(&router, "/api/update-tag", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["id"], tag_id);
    assert_eq!(body["data"]["name"], "NewTagName");

    // Verify the change is persisted
    let (status, body) = get_auth(&router, &format!("/api/tags/{tag_id}"), &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["name"], "NewTagName");
}

#[sqlx::test]
async fn test_update_tag_not_found(pool: PgPool) {
    let (_u, _r, token) = seed(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let update_payload = json!({
        "id": fake_id,
        "name": "Ghost Tag"
    });

    let (status, _body) = post_auth(&router, "/api/update-tag", update_payload, &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_update_tag_reflected_in_items(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    // Create two items sharing the same tag
    let item1 = create_item_with_tags(
        &router,
        restaurant_id,
        "Shared Tag Item 1",
        500,
        json!([{ "New": "SharedTag" }]),
        &token,
    )
    .await;
    let tag_id = item1["tags"][0]["id"].as_str().unwrap().to_string();
    let item1_id = item1["id"].as_str().unwrap();

    let _item2 = create_item_with_tags(
        &router,
        restaurant_id,
        "Shared Tag Item 2",
        600,
        json!([{ "Existing": tag_id }]),
        &token,
    )
    .await;

    // Rename the tag
    let update_payload = json!({
        "id": tag_id,
        "name": "RenamedSharedTag"
    });
    let (status, _body) = post_auth(&router, "/api/update-tag", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK);

    // Both items should reflect the renamed tag
    let (status, body) = get_auth(&router, &format!("/api/items/{item1_id}"), &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["tags"][0]["name"], "RenamedSharedTag");
}

#[sqlx::test]
async fn test_delete_tag(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let item = create_item_with_tags(
        &router,
        restaurant_id,
        "Delete Tag Item",
        500,
        json!([{ "New": "ToDelete" }, { "New": "ToKeep" }]),
        &token,
    )
    .await;
    let item_id = item["id"].as_str().unwrap();
    let tag_to_delete_id = item["tags"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "ToDelete")
        .unwrap()["id"]
        .as_str()
        .unwrap();

    // Delete the tag
    let (status, body) = del_auth(&router, &format!("/api/tags/{tag_to_delete_id}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);

    // Verify tag is gone
    let (status, _body) = get_auth(&router, &format!("/api/tags/{tag_to_delete_id}"), &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Item should still exist but only have the remaining tag
    let (status, body) = get_auth(&router, &format!("/api/items/{item_id}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tags = body["data"]["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0]["name"], "ToKeep");
}

#[sqlx::test]
async fn test_delete_tag_not_found(pool: PgPool) {
    let (_u, _r, token) = seed(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let (status, _body) = del_auth(&router, &format!("/api/tags/{fake_id}"), &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_delete_tag_cascade_removes_from_all_items(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    // Create two items sharing the same tag
    let item1 = create_item_with_tags(
        &router,
        restaurant_id,
        "Cascade Item 1",
        500,
        json!([{ "New": "CascadeTag" }]),
        &token,
    )
    .await;
    let tag_id = item1["tags"][0]["id"].as_str().unwrap().to_string();
    let item1_id = item1["id"].as_str().unwrap();

    let item2 = create_item_with_tags(
        &router,
        restaurant_id,
        "Cascade Item 2",
        600,
        json!([{ "Existing": tag_id }]),
        &token,
    )
    .await;
    let item2_id = item2["id"].as_str().unwrap();

    // Delete the shared tag
    let (status, _body) = del_auth(&router, &format!("/api/tags/{tag_id}"), &token).await;
    assert_eq!(status, StatusCode::OK);

    // Both items should now have no tags
    let (status, body) = get_auth(&router, &format!("/api/items/{item1_id}"), &token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["data"]["tags"].as_array().unwrap().is_empty());

    let (status, body) = get_auth(&router, &format!("/api/items/{item2_id}"), &token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["data"]["tags"].as_array().unwrap().is_empty());
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Edge cases & round-trips
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_create_and_get_item_round_trip(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_item_with_tags(
        &router,
        restaurant_id,
        "Round Trip",
        1234,
        json!([{ "New": "A" }, { "New": "B" }]),
        &token,
    )
    .await;
    let item_id = created["id"].as_str().unwrap();

    let (status, body) = get_auth(&router, &format!("/api/items/{item_id}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let fetched = &body["data"];
    assert_eq!(fetched["id"], created["id"]);
    assert_eq!(fetched["name"], created["name"]);
    assert_eq!(fetched["description"], created["description"]);
    assert_eq!(fetched["base_price_cents"], created["base_price_cents"]);
    assert_eq!(fetched["image_url"], created["image_url"]);
    assert_eq!(fetched["active"], created["active"]);
    assert_eq!(fetched["restaurant_id"], created["restaurant_id"]);
    assert_eq!(fetched["created_by"], created["created_by"]);
    assert_eq!(fetched["updated_by"], created["updated_by"]);
    assert_eq!(fetched["tags"].as_array().unwrap().len(), 2);
}

#[sqlx::test]
async fn test_item_timestamps_are_set(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_item(&router, restaurant_id, "Timestamped", 100, &token).await;
    assert!(created["created_at"].is_string());
    assert!(created["updated_at"].is_string());
    assert!(!created["created_at"].as_str().unwrap().is_empty());
    assert!(!created["updated_at"].as_str().unwrap().is_empty());
}

#[sqlx::test]
async fn test_update_then_get_reflects_changes(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_item(&router, restaurant_id, "Before", 100, &token).await;
    let item_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": item_id,
        "name": "After",
        "base_price_cents": 999,
        "description": "Now with a description",
    });
    let (status, _body) = post_auth(&router, "/api/update-item", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = get_auth(&router, &format!("/api/items/{item_id}"), &token).await;
    assert_eq!(status, StatusCode::OK);

    let data = &body["data"];
    assert_eq!(data["name"], "After");
    assert_eq!(data["base_price_cents"], 999);
    assert_eq!(data["description"], "Now with a description");
}

#[sqlx::test]
async fn test_item_default_active_is_true(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let data = create_item(&router, restaurant_id, "Default Active", 100, &token).await;
    assert_eq!(data["active"], true);
}

#[sqlx::test]
async fn test_item_updated_by_tracks_updater(pool: PgPool) {
    let (user_id, restaurant_id, token) = seed(&pool).await;

    // Create second user via direct SQL (before pool is moved)
    let second_user_id: Uuid =
        sqlx::query_scalar("INSERT INTO users (name, auth_method, role) VALUES ('updater', 'Password', 'Editor') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    let second_token: String = sqlx::query_scalar(
        "INSERT INTO user_sessions (user_id, token, expires_at) \
         VALUES ($1, gen_random_uuid()::text, NOW() + INTERVAL '1 day') \
         RETURNING token",
    )
    .bind(second_user_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let router = app(pool);

    let created = create_item(&router, restaurant_id, "Track Updater", 100, &token).await;
    let item_id = created["id"].as_str().unwrap();
    assert_eq!(created["created_by"], user_id.to_string());
    assert_eq!(created["updated_by"], user_id.to_string());

    // Update by a different user
    let update_payload = json!({
        "id": item_id,
        "name": "Updated By Another",
    });

    let (status, body) = post_auth(&router, "/api/update-item", update_payload, &second_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // created_by stays the same
    assert_eq!(body["data"]["created_by"], user_id.to_string());
    // updated_by changes to the second user (derived from auth)
    assert_eq!(body["data"]["updated_by"], second_user_id.to_string());
}

#[sqlx::test]
async fn test_multiple_items_same_restaurant_different_tags(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let item1 = create_item_with_tags(
        &router,
        restaurant_id,
        "Chicken Rice",
        1200,
        json!([{ "New": "Chicken" }, { "New": "Rice" }]),
        &token,
    )
    .await;

    let item2 = create_item_with_tags(
        &router,
        restaurant_id,
        "Veggie Bowl",
        1000,
        json!([{ "New": "Vegetarian" }, { "New": "Rice" }]),
        &token,
    )
    .await;

    // Verify items have correct tags
    let item1_tags: Vec<&str> = item1["tags"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(item1_tags.contains(&"Chicken"));
    assert!(item1_tags.contains(&"Rice"));
    assert!(!item1_tags.contains(&"Vegetarian"));

    let item2_tags: Vec<&str> = item2["tags"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(item2_tags.contains(&"Vegetarian"));
    assert!(item2_tags.contains(&"Rice"));
    assert!(!item2_tags.contains(&"Chicken"));

    // "Rice" tag should be shared (same id)
    let rice_id_1 = item1["tags"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "Rice")
        .unwrap()["id"]
        .as_str()
        .unwrap();
    let rice_id_2 = item2["tags"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "Rice")
        .unwrap()["id"]
        .as_str()
        .unwrap();
    assert_eq!(rice_id_1, rice_id_2);

    // Total unique tags should be 3
    let (status, body) = get_auth(&router, "/api/tags", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().unwrap().len(), 3);
}
