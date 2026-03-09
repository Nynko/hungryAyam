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

/// Seed the database with app_settings, a user, and a session.
/// Returns (user_id, session_token).
async fn seed(pool: &PgPool) -> (Uuid, String) {
    let access_hash = sha256_hex("test_access_code");
    sqlx::query("INSERT INTO app_settings (id, access_hash) VALUES (1, $1)")
        .bind(&access_hash)
        .execute(pool)
        .await
        .unwrap();

    let user_id: Uuid =
        sqlx::query_scalar("INSERT INTO users (name, auth_method, role) VALUES ('tester', 'Password', 'User') RETURNING id")
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

    (user_id, token)
}

/// Seed a second user (with session) for multi-user scenarios.
/// Returns (user_id, session_token).
async fn seed_second_user(pool: &PgPool) -> (Uuid, String) {
    let user_id: Uuid =
        sqlx::query_scalar("INSERT INTO users (name, auth_method, role) VALUES ('second_user', 'Password', 'User') RETURNING id")
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

    (user_id, token)
}

/// Build a fully-wired Router backed by the given pool.
fn app(pool: PgPool) -> Router {
    let setup_done = Arc::new(AtomicBool::new(true));
    let state = build_state(setup_done, pool, Arc::new(Notify::new()));
    build_app(state)
}

/// Fire a request and return (status, parsed JSON body).
/// No authentication header is sent.
async fn send(
    router: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    send_with_auth(router, method, uri, body, None).await
}

/// Fire a request with an optional Bearer token.
async fn send_with_auth(
    router: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let req_body = match body {
        Some(v) => Body::from(serde_json::to_vec(&v).unwrap()),
        None => Body::empty(),
    };

    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");

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

/// Convenience: POST with JSON body (no auth).
async fn post(router: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    send(router, Method::POST, uri, Some(body)).await
}

/// Convenience: POST with JSON body and auth token.
async fn post_auth(router: &Router, uri: &str, body: Value, token: &str) -> (StatusCode, Value) {
    send_with_auth(router, Method::POST, uri, Some(body), Some(token)).await
}

/// Convenience: GET (no auth — public routes).
async fn get(router: &Router, uri: &str) -> (StatusCode, Value) {
    send(router, Method::GET, uri, None).await
}

/// Convenience: DELETE (no auth).
async fn del(router: &Router, uri: &str) -> (StatusCode, Value) {
    send(router, Method::DELETE, uri, None).await
}

/// Convenience: DELETE with auth token.
async fn del_auth(router: &Router, uri: &str, token: &str) -> (StatusCode, Value) {
    send_with_auth(router, Method::DELETE, uri, None, Some(token)).await
}

/// Create a restaurant via the API and return the response data object.
/// The server derives created_by / updated_by from the authenticated user.
async fn create_restaurant(router: &Router, name: &str, token: &str) -> Value {
    let payload = json!({
        "name": name,
        "image_url": null
    });
    let (status, body) = post_auth(router, "/api/restaurants", payload, token).await;
    assert_eq!(status, StatusCode::CREATED, "create restaurant failed: {body}");
    assert_eq!(body["success"], true);
    body["data"].clone()
}

/// Create a restaurant with an image URL.
async fn create_restaurant_with_image(
    router: &Router,
    name: &str,
    image_url: &str,
    token: &str,
) -> Value {
    let payload = json!({
        "name": name,
        "image_url": image_url
    });
    let (status, body) = post_auth(router, "/api/restaurants", payload, token).await;
    assert_eq!(status, StatusCode::CREATED, "create restaurant failed: {body}");
    assert_eq!(body["success"], true);
    body["data"].clone()
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Create
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_create_restaurant_minimal(pool: PgPool) {
    let (user_id, token) = seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "Test Restaurant",
        "image_url": null
    });

    let (status, body) = post_auth(&router, "/api/restaurants", payload, &token).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["success"], true);

    let data = &body["data"];
    assert!(data["id"].is_string());
    assert_eq!(data["name"], "Test Restaurant");
    assert_eq!(data["image_url"], Value::Null);
    assert_eq!(data["created_by"], user_id.to_string());
    assert_eq!(data["updated_by"], user_id.to_string());
    assert!(data["created_at"].is_string());
    assert!(data["updated_at"].is_string());
}

#[sqlx::test]
async fn test_create_restaurant_requires_auth(pool: PgPool) {
    let (_user_id, _token) = seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "No Auth Restaurant",
        "image_url": null
    });

    // POST without auth token should be rejected
    let (status, _body) = post(&router, "/api/restaurants", payload).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn test_create_restaurant_with_image(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "Fancy Place",
        "image_url": "https://example.com/logo.png"
    });

    let (status, body) = post_auth(&router, "/api/restaurants", payload, &token).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["success"], true);

    let data = &body["data"];
    assert_eq!(data["name"], "Fancy Place");
    assert_eq!(data["image_url"], "https://example.com/logo.png");
}

#[sqlx::test]
async fn test_create_restaurant_sets_updated_by_to_created_by(pool: PgPool) {
    let (user_id, token) = seed(&pool).await;
    let router = app(pool);

    let data = create_restaurant(&router, "Owner Restaurant", &token).await;

    // On creation, updated_by should match created_by (both from auth user)
    assert_eq!(data["created_by"], user_id.to_string());
    assert_eq!(data["updated_by"], user_id.to_string());
}

#[sqlx::test]
async fn test_create_multiple_restaurants(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let r1 = create_restaurant(&router, "Restaurant One", &token).await;
    let r2 = create_restaurant(&router, "Restaurant Two", &token).await;
    let r3 = create_restaurant(&router, "Restaurant Three", &token).await;

    // All should have distinct IDs
    let id1 = r1["id"].as_str().unwrap();
    let id2 = r2["id"].as_str().unwrap();
    let id3 = r3["id"].as_str().unwrap();
    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
    assert_ne!(id1, id3);
}

#[sqlx::test]
async fn test_create_restaurant_with_invalid_image_url_rejected(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "Bad URL Restaurant",
        "image_url": "not-a-valid-url"
    });

    let (status, _body) = post_auth(&router, "/api/restaurants", payload, &token).await;
    // Invalid URL should fail validation during deserialization
    assert_ne!(status, StatusCode::CREATED);
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Get (public — no auth required)
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_get_restaurant_by_id(pool: PgPool) {
    let (user_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, "Findable Restaurant", &token).await;
    let rest_id = created["id"].as_str().unwrap();

    let (status, body) = get(&router, &format!("/api/restaurants/{rest_id}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);

    let data = &body["data"];
    assert_eq!(data["id"], rest_id);
    assert_eq!(data["name"], "Findable Restaurant");
    assert_eq!(data["created_by"], user_id.to_string());
}

#[sqlx::test]
async fn test_get_restaurant_not_found(pool: PgPool) {
    let (_user_id, _token) = seed(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let (status, _body) = get(&router, &format!("/api/restaurants/{fake_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_get_restaurant_preserves_all_fields(pool: PgPool) {
    let (user_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant_with_image(
        &router,
        "Full Restaurant",
        "https://example.com/full.jpg",
        &token,
    )
    .await;
    let rest_id = created["id"].as_str().unwrap();

    let (status, body) = get(&router, &format!("/api/restaurants/{rest_id}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let data = &body["data"];
    assert_eq!(data["name"], "Full Restaurant");
    assert_eq!(data["image_url"], "https://example.com/full.jpg");
    assert_eq!(data["created_by"], user_id.to_string());
    assert_eq!(data["updated_by"], user_id.to_string());
    assert!(data["created_at"].is_string());
    assert!(data["updated_at"].is_string());
}

// ═══════════════════════════════════════════════════════════════════
// Tests — List (public — no auth required)
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_list_restaurants_empty(pool: PgPool) {
    let (_user_id, _token) = seed(&pool).await;
    let router = app(pool);

    let (status, body) = get(&router, "/api/restaurants").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    assert!(body["data"].as_array().unwrap().is_empty());
}

#[sqlx::test]
async fn test_list_restaurants_multiple(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    create_restaurant(&router, "Restaurant A", &token).await;
    create_restaurant(&router, "Restaurant B", &token).await;
    create_restaurant(&router, "Restaurant C", &token).await;

    let (status, body) = get(&router, "/api/restaurants").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);

    let restaurants = body["data"].as_array().unwrap();
    assert_eq!(restaurants.len(), 3);

    let names: Vec<&str> = restaurants
        .iter()
        .filter_map(|r| r["name"].as_str())
        .collect();
    assert!(names.contains(&"Restaurant A"));
    assert!(names.contains(&"Restaurant B"));
    assert!(names.contains(&"Restaurant C"));
}

#[sqlx::test]
async fn test_list_restaurants_order(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let r1 = create_restaurant(&router, "First", &token).await;
    let r2 = create_restaurant(&router, "Second", &token).await;

    let (status, body) = get(&router, "/api/restaurants").await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let restaurants = body["data"].as_array().unwrap();
    assert_eq!(restaurants.len(), 2);

    // Ordered by created_at DESC — Second should come first
    assert_eq!(restaurants[0]["id"], r2["id"]);
    assert_eq!(restaurants[1]["id"], r1["id"]);
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Active restaurants (public)
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_list_active_restaurants_empty_when_no_sessions(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    create_restaurant(&router, "No Sessions", &token).await;

    let (status, body) = get(&router, "/api/restaurants/active").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["data"].as_array().unwrap().is_empty());
}

#[sqlx::test]
async fn test_list_active_restaurants_with_active_session(pool: PgPool) {
    let (user_id, token) = seed(&pool).await;
    let router = app(pool.clone());

    let created = create_restaurant(&router, "Active Place", &token).await;
    let rest_id: Uuid = created["id"].as_str().unwrap().parse().unwrap();

    // Insert an active order session (end_date in the future)
    sqlx::query(
        "INSERT INTO order_sessions (restaurant_id, start_date, end_date, status, created_by, updated_by) \
         VALUES ($1, NOW(), NOW() + INTERVAL '1 day', 0, $2, $2)",
    )
    .bind(rest_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = get(&router, "/api/restaurants/active").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let data = body["data"].as_array().unwrap();
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["name"], "Active Place");
}

#[sqlx::test]
async fn test_list_active_restaurants_excludes_expired_sessions(pool: PgPool) {
    let (user_id, token) = seed(&pool).await;
    let router = app(pool.clone());

    let created = create_restaurant(&router, "Expired Place", &token).await;
    let rest_id: Uuid = created["id"].as_str().unwrap().parse().unwrap();

    // Insert an expired order session
    sqlx::query(
        "INSERT INTO order_sessions (restaurant_id, start_date, end_date, status, created_by, updated_by) \
         VALUES ($1, NOW() - INTERVAL '2 days', NOW() - INTERVAL '1 day', 0,  $2, $2)",
    )
    .bind(rest_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = get(&router, "/api/restaurants/active").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["data"].as_array().unwrap().is_empty());
}

#[sqlx::test]
async fn test_list_active_restaurants_mixed(pool: PgPool) {
    let (user_id, token) = seed(&pool).await;
    let router = app(pool.clone());

    let active = create_restaurant(&router, "Has Active", &token).await;
    let expired = create_restaurant(&router, "Has Expired", &token).await;
    create_restaurant(&router, "No Session", &token).await;

    let active_id: Uuid = active["id"].as_str().unwrap().parse().unwrap();
    let expired_id: Uuid = expired["id"].as_str().unwrap().parse().unwrap();

    // Active session
    sqlx::query(
        "INSERT INTO order_sessions (restaurant_id, start_date, end_date, status, created_by, updated_by) \
         VALUES ($1, NOW(), NOW() + INTERVAL '1 day', 0, $2, $2)",
    )
    .bind(active_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    // Expired session
    sqlx::query(
        "INSERT INTO order_sessions (restaurant_id, start_date, end_date, status, created_by, updated_by) \
         VALUES ($1, NOW() - INTERVAL '2 days', NOW() - INTERVAL '1 day', 0,  $2, $2)",
    )
    .bind(expired_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = get(&router, "/api/restaurants/active").await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let data = body["data"].as_array().unwrap();
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["name"], "Has Active");
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Update (requires auth)
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_update_restaurant_requires_auth(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, "Auth Test", &token).await;
    let rest_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": rest_id,
        "name": "Should Fail"
    });

    // POST without auth token should be rejected
    let (status, _body) = post(&router, "/api/update-restaurant", update_payload).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn test_update_restaurant_name(pool: PgPool) {
    let (user_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, "Old Name", &token).await;
    let rest_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": rest_id,
        "name": "New Name"
    });

    let (status, body) = post_auth(&router, "/api/update-restaurant", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["name"], "New Name");
    // created_by should remain unchanged
    assert_eq!(body["data"]["created_by"], user_id.to_string());
}

#[sqlx::test]
async fn test_update_restaurant_image_url(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, "Image Update", &token).await;
    let rest_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": rest_id,
        "image_url": "https://example.com/new-logo.png"
    });

    let (status, body) = post_auth(&router, "/api/update-restaurant", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["image_url"], "https://example.com/new-logo.png");
    // Name should remain unchanged
    assert_eq!(body["data"]["name"], "Image Update");
}

#[sqlx::test]
async fn test_update_restaurant_multiple_fields(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant_with_image(
        &router,
        "Multi Update",
        "https://example.com/old.png",
        &token,
    )
    .await;
    let rest_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": rest_id,
        "name": "Updated Multi",
        "image_url": "https://example.com/updated.png"
    });

    let (status, body) = post_auth(&router, "/api/update-restaurant", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let data = &body["data"];
    assert_eq!(data["name"], "Updated Multi");
    assert_eq!(data["image_url"], "https://example.com/updated.png");
}

#[sqlx::test]
async fn test_update_restaurant_updated_by_changes(pool: PgPool) {
    let (user_id, token) = seed(&pool).await;
    let (second_user_id, second_token) = seed_second_user(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, "Ownership Test", &token).await;
    let rest_id = created["id"].as_str().unwrap();
    assert_eq!(created["updated_by"], user_id.to_string());

    // Update by a different user — server derives updated_by from auth
    let update_payload = json!({
        "id": rest_id,
        "name": "Ownership Updated"
    });

    let (status, body) = post_auth(&router, "/api/update-restaurant", update_payload, &second_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // created_by should remain the original user
    assert_eq!(body["data"]["created_by"], user_id.to_string());
    // updated_by should reflect the second user
    assert_eq!(body["data"]["updated_by"], second_user_id.to_string());
}

#[sqlx::test]
async fn test_update_restaurant_not_found(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let update_payload = json!({
        "id": fake_id,
        "name": "Ghost"
    });

    let (status, _body) = post_auth(&router, "/api/update-restaurant", update_payload, &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_update_restaurant_with_invalid_image_url_rejected(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, "Bad URL Update", &token).await;
    let rest_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": rest_id,
        "image_url": "not-a-url"
    });

    let (status, _body) = post_auth(&router, "/api/update-restaurant", update_payload, &token).await;
    assert_ne!(status, StatusCode::OK, "should reject invalid URL");
}

#[sqlx::test]
async fn test_update_restaurant_partial_only_touches_specified_fields(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant_with_image(
        &router,
        "Partial Update",
        "https://example.com/original.png",
        &token,
    )
    .await;
    let rest_id = created["id"].as_str().unwrap();

    // Only update the name, image_url should stay the same
    let update_payload = json!({
        "id": rest_id,
        "name": "Only Name Changed"
    });

    let (status, body) = post_auth(&router, "/api/update-restaurant", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["name"], "Only Name Changed");
    assert_eq!(body["data"]["image_url"], "https://example.com/original.png");
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Delete (requires auth)
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_delete_restaurant_requires_auth(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, "Delete Auth Test", &token).await;
    let rest_id = created["id"].as_str().unwrap();

    // DELETE without auth token should be rejected
    let (status, _body) = del(&router, &format!("/api/restaurants/{rest_id}")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn test_delete_restaurant(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, "To Delete", &token).await;
    let rest_id = created["id"].as_str().unwrap();

    let (status, body) = del_auth(&router, &format!("/api/restaurants/{rest_id}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);

    // Verify it's gone
    let (status, _body) = get(&router, &format!("/api/restaurants/{rest_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_delete_restaurant_not_found(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let (status, _body) = del_auth(&router, &format!("/api/restaurants/{fake_id}"), &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_delete_restaurant_idempotent(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, "Delete Twice", &token).await;
    let rest_id = created["id"].as_str().unwrap();

    let (status, _body) = del_auth(&router, &format!("/api/restaurants/{rest_id}"), &token).await;
    assert_eq!(status, StatusCode::OK);

    // Second delete should return 404
    let (status, _body) = del_auth(&router, &format!("/api/restaurants/{rest_id}"), &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_delete_restaurant_with_active_session_rejected(pool: PgPool) {
    let (user_id, token) = seed(&pool).await;
    let router = app(pool.clone());

    let created = create_restaurant(&router, "Active Session", &token).await;
    let rest_id: Uuid = created["id"].as_str().unwrap().parse().unwrap();

    // Insert an active order session
    sqlx::query!(
        "INSERT INTO order_sessions (restaurant_id, start_date, end_date, status, created_by, updated_by) \
         VALUES ($1, NOW(), NOW() + INTERVAL '1 day', 0, $2, $2)",
         rest_id,
         user_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = del_auth(&router, &format!("/api/restaurants/{rest_id}"), &token).await;
    assert_ne!(status, StatusCode::OK, "should reject deletion: {body}");
}

#[sqlx::test]
async fn test_delete_restaurant_with_expired_session_allowed(pool: PgPool) {
    let (user_id, token) = seed(&pool).await;
    let router = app(pool.clone());

    let created = create_restaurant(&router, "Expired Session", &token).await;
    let rest_id: Uuid = created["id"].as_str().unwrap().parse().unwrap();

    // Insert an expired order session
    sqlx::query(
        "INSERT INTO order_sessions (restaurant_id, start_date, end_date, status, created_by, updated_by) \
         VALUES ($1, NOW() - INTERVAL '2 days', NOW() - INTERVAL '1 day', 0,  $2, $2)",
    )
    .bind(rest_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = del_auth(&router, &format!("/api/restaurants/{rest_id}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[sqlx::test]
async fn test_delete_restaurant_removes_from_list(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let r1 = create_restaurant(&router, "Keep Me", &token).await;
    let r2 = create_restaurant(&router, "Delete Me", &token).await;
    let r2_id = r2["id"].as_str().unwrap();

    // Both should be listed
    let (status, body) = get(&router, "/api/restaurants").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().unwrap().len(), 2);

    // Delete one
    let (status, _body) = del_auth(&router, &format!("/api/restaurants/{r2_id}"), &token).await;
    assert_eq!(status, StatusCode::OK);

    // Verify only one remains
    let (status, body) = get(&router, "/api/restaurants").await;
    assert_eq!(status, StatusCode::OK);
    let restaurants = body["data"].as_array().unwrap();
    assert_eq!(restaurants.len(), 1);
    assert_eq!(restaurants[0]["name"], "Keep Me");
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Edge cases & round-trips
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_create_and_get_round_trip(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant_with_image(
        &router,
        "Round Trip",
        "https://example.com/trip.webp",
        &token,
    )
    .await;
    let rest_id = created["id"].as_str().unwrap();

    let (status, body) = get(&router, &format!("/api/restaurants/{rest_id}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let fetched = &body["data"];
    assert_eq!(fetched["id"], created["id"]);
    assert_eq!(fetched["name"], created["name"]);
    assert_eq!(fetched["image_url"], created["image_url"]);
    assert_eq!(fetched["created_by"], created["created_by"]);
    assert_eq!(fetched["updated_by"], created["updated_by"]);
}

#[sqlx::test]
async fn test_restaurant_timestamps_are_set(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, "Timestamped", &token).await;
    let created_at = created["created_at"].as_str().unwrap();
    let updated_at = created["updated_at"].as_str().unwrap();

    assert!(!created_at.is_empty());
    assert!(!updated_at.is_empty());
}

#[sqlx::test]
async fn test_update_then_get_reflects_changes(pool: PgPool) {
    let (_user_id, token) = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, "Before Update", &token).await;
    let rest_id = created["id"].as_str().unwrap();

    // Update the restaurant
    let update_payload = json!({
        "id": rest_id,
        "name": "After Update",
        "image_url": "https://example.com/after.png"
    });
    let (status, _body) = post_auth(&router, "/api/update-restaurant", update_payload, &token).await;
    assert_eq!(status, StatusCode::OK);

    // Fetch and verify changes are persisted (GET is public)
    let (status, body) = get(&router, &format!("/api/restaurants/{rest_id}")).await;
    assert_eq!(status, StatusCode::OK);

    let data = &body["data"];
    assert_eq!(data["name"], "After Update");
    assert_eq!(data["image_url"], "https://example.com/after.png");
}

#[sqlx::test]
async fn test_different_users_can_own_restaurants(pool: PgPool) {
    let (user_id, token) = seed(&pool).await;
    let (second_user_id, second_token) = seed_second_user(&pool).await;
    let router = app(pool);

    let r1 = create_restaurant(&router, "User1 Restaurant", &token).await;
    let r2 = create_restaurant(&router, "User2 Restaurant", &second_token).await;

    assert_eq!(r1["created_by"], user_id.to_string());
    assert_eq!(r2["created_by"], second_user_id.to_string());

    // Both should appear in the full list (GET is public)
    let (status, body) = get(&router, "/api/restaurants").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().unwrap().len(), 2);
}
