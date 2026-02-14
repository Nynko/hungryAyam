use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::{atomic::AtomicBool, Arc};
use tower::ServiceExt;
use uuid::Uuid;

use crate::{app::build_app, state::build_state};

// ═══════════════════════════════════════════════════════════════════
// Test helpers
// ═══════════════════════════════════════════════════════════════════

/// Seed the database with app_settings and a user.
/// Returns the user_id.
async fn seed(pool: &PgPool) -> Uuid {
    sqlx::query("INSERT INTO app_settings (id, title) VALUES (1, 'Test App')")
        .execute(pool)
        .await
        .unwrap();

    let user_id: Uuid =
        sqlx::query_scalar("INSERT INTO users (name, auth_method) VALUES ('tester', 'Password') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();

    user_id
}

/// Seed a second user for multi-user scenarios.
async fn seed_second_user(pool: &PgPool) -> Uuid {
    let user_id: Uuid =
        sqlx::query_scalar("INSERT INTO users (name, auth_method) VALUES ('second_user', 'Password') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
    user_id
}

/// Build a fully-wired Router backed by the given pool.
fn app(pool: PgPool) -> Router {
    let setup_done = Arc::new(AtomicBool::new(true));
    let state = build_state(setup_done, pool);
    build_app(state)
}

/// Fire a request and return (status, parsed JSON body).
async fn send(
    router: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let req_body = match body {
        Some(v) => Body::from(serde_json::to_vec(&v).unwrap()),
        None => Body::empty(),
    };

    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(req_body)
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(json!(null));
    (status, json)
}

/// Convenience: POST with JSON body.
async fn post(router: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    send(router, Method::POST, uri, Some(body)).await
}

/// Convenience: GET.
async fn get(router: &Router, uri: &str) -> (StatusCode, Value) {
    send(router, Method::GET, uri, None).await
}

/// Convenience: DELETE.
async fn del(router: &Router, uri: &str) -> (StatusCode, Value) {
    send(router, Method::DELETE, uri, None).await
}

/// Create a restaurant via the API and return the response data object.
async fn create_restaurant(router: &Router, user_id: Uuid, name: &str) -> Value {
    let payload = json!({
        "name": name,
        "image_url": null,
        "created_by": user_id
    });
    let (status, body) = post(router, "/api/restaurants", payload).await;
    assert_eq!(status, StatusCode::CREATED, "create restaurant failed: {body}");
    assert_eq!(body["success"], true);
    body["data"].clone()
}

/// Create a restaurant with an image URL.
async fn create_restaurant_with_image(
    router: &Router,
    user_id: Uuid,
    name: &str,
    image_url: &str,
) -> Value {
    let payload = json!({
        "name": name,
        "image_url": image_url,
        "created_by": user_id
    });
    let (status, body) = post(router, "/api/restaurants", payload).await;
    assert_eq!(status, StatusCode::CREATED, "create restaurant failed: {body}");
    assert_eq!(body["success"], true);
    body["data"].clone()
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Create
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_create_restaurant_minimal(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "Test Restaurant",
        "image_url": null,
        "created_by": user_id
    });

    let (status, body) = post(&router, "/api/restaurants", payload).await;
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
async fn test_create_restaurant_with_image(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "Fancy Place",
        "image_url": "https://example.com/logo.png",
        "created_by": user_id
    });

    let (status, body) = post(&router, "/api/restaurants", payload).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["success"], true);

    let data = &body["data"];
    assert_eq!(data["name"], "Fancy Place");
    assert_eq!(data["image_url"], "https://example.com/logo.png");
}

#[sqlx::test]
async fn test_create_restaurant_sets_updated_by_to_created_by(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    let data = create_restaurant(&router, user_id, "Owner Restaurant").await;

    // On creation, updated_by should match created_by
    assert_eq!(data["created_by"], user_id.to_string());
    assert_eq!(data["updated_by"], user_id.to_string());
}

#[sqlx::test]
async fn test_create_multiple_restaurants(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    let r1 = create_restaurant(&router, user_id, "Restaurant One").await;
    let r2 = create_restaurant(&router, user_id, "Restaurant Two").await;
    let r3 = create_restaurant(&router, user_id, "Restaurant Three").await;

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
    let user_id = seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "Bad URL Restaurant",
        "image_url": "not-a-valid-url",
        "created_by": user_id
    });

    let (status, _body) = post(&router, "/api/restaurants", payload).await;
    // Invalid URL should fail validation during deserialization
    assert_ne!(status, StatusCode::CREATED);
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Get
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_get_restaurant_by_id(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, user_id, "Findable Restaurant").await;
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
    seed(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let (status, _body) = get(&router, &format!("/api/restaurants/{fake_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_get_restaurant_preserves_all_fields(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant_with_image(
        &router,
        user_id,
        "Full Restaurant",
        "https://example.com/full.jpg",
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
// Tests — List
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_list_restaurants_empty(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let (status, body) = get(&router, "/api/restaurants").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    assert!(body["data"].as_array().unwrap().is_empty());
}

#[sqlx::test]
async fn test_list_restaurants_multiple(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    create_restaurant(&router, user_id, "Alpha").await;
    create_restaurant(&router, user_id, "Beta").await;
    create_restaurant(&router, user_id, "Gamma").await;

    let (status, body) = get(&router, "/api/restaurants").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);

    let restaurants = body["data"].as_array().unwrap();
    assert_eq!(restaurants.len(), 3);

    let names: Vec<&str> = restaurants
        .iter()
        .filter_map(|r| r["name"].as_str())
        .collect();
    assert!(names.contains(&"Alpha"));
    assert!(names.contains(&"Beta"));
    assert!(names.contains(&"Gamma"));
}

#[sqlx::test]
async fn test_list_restaurants_order(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    create_restaurant(&router, user_id, "First").await;
    create_restaurant(&router, user_id, "Second").await;
    create_restaurant(&router, user_id, "Third").await;

    let (status, body) = get(&router, "/api/restaurants").await;
    assert_eq!(status, StatusCode::OK);

    let restaurants = body["data"].as_array().unwrap();
    assert_eq!(restaurants.len(), 3);

    // Restaurants are ordered by created_at DESC (newest first)
    assert_eq!(restaurants[0]["name"], "Third");
    assert_eq!(restaurants[1]["name"], "Second");
    assert_eq!(restaurants[2]["name"], "First");
}

// ═══════════════════════════════════════════════════════════════════
// Tests — List active (restaurants with active order sessions)
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_list_active_restaurants_empty_when_no_sessions(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    // Create restaurants but no order sessions
    create_restaurant(&router, user_id, "No Sessions").await;

    let (status, body) = get(&router, "/api/restaurants/active").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    assert!(body["data"].as_array().unwrap().is_empty());
}

#[sqlx::test]
async fn test_list_active_restaurants_with_active_session(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool.clone());

    let rest = create_restaurant(&router, user_id, "Active Place").await;
    let rest_id: Uuid = rest["id"].as_str().unwrap().parse().unwrap();

    // Create an active order session (end_date far in the future)
    sqlx::query(
        "INSERT INTO order_sessions (restaurant_id, start_date, end_date, status, created_by, updated_by)
         VALUES ($1, NOW(), NOW() + INTERVAL '1 day', 1, $2, $2)"
    )
    .bind(rest_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = get(&router, "/api/restaurants/active").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);

    let active = body["data"].as_array().unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0]["name"], "Active Place");
}

#[sqlx::test]
async fn test_list_active_restaurants_excludes_expired_sessions(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool.clone());

    let rest = create_restaurant(&router, user_id, "Expired Place").await;
    let rest_id: Uuid = rest["id"].as_str().unwrap().parse().unwrap();

    // Create an expired order session (end_date in the past)
    sqlx::query(
        "INSERT INTO order_sessions (restaurant_id, start_date, end_date, status, created_by, updated_by)
         VALUES ($1, NOW() - INTERVAL '2 days', NOW() - INTERVAL '1 day', 1, $2, $2)"
    )
    .bind(rest_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = get(&router, "/api/restaurants/active").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // Should be empty since the session is expired
    assert!(body["data"].as_array().unwrap().is_empty());
}

#[sqlx::test]
async fn test_list_active_restaurants_mixed(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool.clone());

    let active_rest = create_restaurant(&router, user_id, "Active").await;
    let active_id: Uuid = active_rest["id"].as_str().unwrap().parse().unwrap();

    let _inactive_rest = create_restaurant(&router, user_id, "Inactive").await;

    let expired_rest = create_restaurant(&router, user_id, "Expired").await;
    let expired_id: Uuid = expired_rest["id"].as_str().unwrap().parse().unwrap();

    // Active session for "Active" restaurant
    sqlx::query(
        "INSERT INTO order_sessions (restaurant_id, start_date, end_date, status, created_by, updated_by)
         VALUES ($1, NOW(), NOW() + INTERVAL '1 day', 1, $2, $2)"
    )
    .bind(active_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    // Expired session for "Expired" restaurant
    sqlx::query(
        "INSERT INTO order_sessions (restaurant_id, start_date, end_date, status, created_by, updated_by)
         VALUES ($1, NOW() - INTERVAL '2 days', NOW() - INTERVAL '1 day', 1, $2, $2)"
    )
    .bind(expired_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = get(&router, "/api/restaurants/active").await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let active = body["data"].as_array().unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0]["name"], "Active");
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Update
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_update_restaurant_name(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, user_id, "Old Name").await;
    let rest_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": rest_id,
        "name": "New Name",
        "updated_by": user_id
    });

    let (status, body) = post(&router, "/api/update-restaurant", update_payload).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["name"], "New Name");
    // created_by should remain unchanged
    assert_eq!(body["data"]["created_by"], user_id.to_string());
}

#[sqlx::test]
async fn test_update_restaurant_image_url(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, user_id, "Image Update").await;
    let rest_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": rest_id,
        "image_url": "https://example.com/new-logo.png",
        "updated_by": user_id
    });

    let (status, body) = post(&router, "/api/update-restaurant", update_payload).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["image_url"], "https://example.com/new-logo.png");
    // Name should remain unchanged
    assert_eq!(body["data"]["name"], "Image Update");
}

#[sqlx::test]
async fn test_update_restaurant_multiple_fields(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant_with_image(
        &router,
        user_id,
        "Multi Update",
        "https://example.com/old.png",
    )
    .await;
    let rest_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": rest_id,
        "name": "Updated Multi",
        "image_url": "https://example.com/updated.png",
        "updated_by": user_id
    });

    let (status, body) = post(&router, "/api/update-restaurant", update_payload).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let data = &body["data"];
    assert_eq!(data["name"], "Updated Multi");
    assert_eq!(data["image_url"], "https://example.com/updated.png");
}

#[sqlx::test]
async fn test_update_restaurant_updated_by_changes(pool: PgPool) {
    let user_id = seed(&pool).await;
    let second_user_id = seed_second_user(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, user_id, "Ownership Test").await;
    let rest_id = created["id"].as_str().unwrap();
    assert_eq!(created["updated_by"], user_id.to_string());

    // Update by a different user
    let update_payload = json!({
        "id": rest_id,
        "name": "Ownership Updated",
        "updated_by": second_user_id
    });

    let (status, body) = post(&router, "/api/update-restaurant", update_payload).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // created_by should remain the original user
    assert_eq!(body["data"]["created_by"], user_id.to_string());
    // updated_by should reflect the second user
    assert_eq!(body["data"]["updated_by"], second_user_id.to_string());
}

#[sqlx::test]
async fn test_update_restaurant_not_found(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let update_payload = json!({
        "id": fake_id,
        "name": "Ghost",
        "updated_by": user_id
    });

    let (status, _body) = post(&router, "/api/update-restaurant", update_payload).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_update_restaurant_with_invalid_image_url_rejected(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, user_id, "Bad URL Update").await;
    let rest_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": rest_id,
        "image_url": "not-a-url",
        "updated_by": user_id
    });

    let (status, _body) = post(&router, "/api/update-restaurant", update_payload).await;
    assert_ne!(status, StatusCode::OK, "should reject invalid URL");
}

#[sqlx::test]
async fn test_update_restaurant_partial_only_touches_specified_fields(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant_with_image(
        &router,
        user_id,
        "Partial Update",
        "https://example.com/original.png",
    )
    .await;
    let rest_id = created["id"].as_str().unwrap();

    // Only update the name, image_url should stay the same
    let update_payload = json!({
        "id": rest_id,
        "name": "Only Name Changed",
        "updated_by": user_id
    });

    let (status, body) = post(&router, "/api/update-restaurant", update_payload).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["name"], "Only Name Changed");
    assert_eq!(body["data"]["image_url"], "https://example.com/original.png");
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Delete
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_delete_restaurant(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, user_id, "To Delete").await;
    let rest_id = created["id"].as_str().unwrap();

    let (status, body) = del(&router, &format!("/api/restaurants/{rest_id}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);

    // Verify it's gone
    let (status, _body) = get(&router, &format!("/api/restaurants/{rest_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_delete_restaurant_not_found(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let (status, _body) = del(&router, &format!("/api/restaurants/{fake_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_delete_restaurant_idempotent(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, user_id, "Delete Twice").await;
    let rest_id = created["id"].as_str().unwrap();

    // First delete succeeds
    let (status, _body) = del(&router, &format!("/api/restaurants/{rest_id}")).await;
    assert_eq!(status, StatusCode::OK);

    // Second delete returns not found
    let (status, _body) = del(&router, &format!("/api/restaurants/{rest_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_delete_restaurant_with_active_session_rejected(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool.clone());

    let rest = create_restaurant(&router, user_id, "Has Active Session").await;
    let rest_id_str = rest["id"].as_str().unwrap();
    let rest_id: Uuid = rest_id_str.parse().unwrap();

    // Create an active order session
    sqlx::query(
        "INSERT INTO order_sessions (restaurant_id, start_date, end_date, status, created_by, updated_by)
         VALUES ($1, NOW(), NOW() + INTERVAL '1 day', 1, $2, $2)"
    )
    .bind(rest_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    // Attempt to delete should fail
    let (status, _body) = del(&router, &format!("/api/restaurants/{rest_id_str}")).await;
    assert_ne!(status, StatusCode::OK, "should reject deletion with active session");
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::INTERNAL_SERVER_ERROR,
        "expected error status, got {status}"
    );
}

#[sqlx::test]
async fn test_delete_restaurant_with_expired_session_allowed(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool.clone());

    let rest = create_restaurant(&router, user_id, "Expired Session OK").await;
    let rest_id_str = rest["id"].as_str().unwrap();
    let rest_id: Uuid = rest_id_str.parse().unwrap();

    // Create an expired order session
    sqlx::query(
        "INSERT INTO order_sessions (restaurant_id, start_date, end_date, status, created_by, updated_by)
         VALUES ($1, NOW() - INTERVAL '2 days', NOW() - INTERVAL '1 day', 1, $2, $2)"
    )
    .bind(rest_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    // Delete should succeed since the session is expired
    let (status, body) = del(&router, &format!("/api/restaurants/{rest_id_str}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
}

#[sqlx::test]
async fn test_delete_restaurant_removes_from_list(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    let r1 = create_restaurant(&router, user_id, "Keep Me").await;
    let r2 = create_restaurant(&router, user_id, "Remove Me").await;
    let r2_id = r2["id"].as_str().unwrap();

    // Verify both are listed
    let (status, body) = get(&router, "/api/restaurants").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().unwrap().len(), 2);

    // Delete one
    let (status, _body) = del(&router, &format!("/api/restaurants/{r2_id}")).await;
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
    let user_id = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant_with_image(
        &router,
        user_id,
        "Round Trip",
        "https://example.com/trip.webp",
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
    let user_id = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, user_id, "Timestamped").await;
    let created_at = created["created_at"].as_str().unwrap();
    let updated_at = created["updated_at"].as_str().unwrap();

    assert!(!created_at.is_empty());
    assert!(!updated_at.is_empty());
}

#[sqlx::test]
async fn test_update_then_get_reflects_changes(pool: PgPool) {
    let user_id = seed(&pool).await;
    let router = app(pool);

    let created = create_restaurant(&router, user_id, "Before Update").await;
    let rest_id = created["id"].as_str().unwrap();

    // Update the restaurant
    let update_payload = json!({
        "id": rest_id,
        "name": "After Update",
        "image_url": "https://example.com/after.png",
        "updated_by": user_id
    });
    let (status, _body) = post(&router, "/api/update-restaurant", update_payload).await;
    assert_eq!(status, StatusCode::OK);

    // Fetch and verify changes are persisted
    let (status, body) = get(&router, &format!("/api/restaurants/{rest_id}")).await;
    assert_eq!(status, StatusCode::OK);

    let data = &body["data"];
    assert_eq!(data["name"], "After Update");
    assert_eq!(data["image_url"], "https://example.com/after.png");
}

#[sqlx::test]
async fn test_different_users_can_own_restaurants(pool: PgPool) {
    let user_id = seed(&pool).await;
    let second_user_id = seed_second_user(&pool).await;
    let router = app(pool);

    let r1 = create_restaurant(&router, user_id, "User1 Restaurant").await;
    let r2 = create_restaurant(&router, second_user_id, "User2 Restaurant").await;

    assert_eq!(r1["created_by"], user_id.to_string());
    assert_eq!(r2["created_by"], second_user_id.to_string());

    // Both should appear in the full list
    let (status, body) = get(&router, "/api/restaurants").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().unwrap().len(), 2);
}