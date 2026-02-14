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

/// Seed the database with app_settings so the setup middleware allows requests.
async fn seed(pool: &PgPool) {
    sqlx::query("INSERT INTO app_settings (id, title) VALUES (1, 'Test App')")
        .execute(pool)
        .await
        .unwrap();
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

/// Create a user via the API and return the response data object.
async fn create_user(router: &Router, name: &str, email: Option<&str>) -> Value {
    let mut payload = json!({
        "name": name,
        "auth_method": "password",
        "user_cookie": null
    });
    if let Some(e) = email {
        payload["email"] = json!(e);
    }
    let (status, body) = post(router, "/api/user", payload).await;
    assert_eq!(status, StatusCode::CREATED, "create user failed: {body}");
    assert_eq!(body["success"], true);
    body["data"].clone()
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Create
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_create_user_minimal(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": null,
        "email": null,
        "auth_method": null,
        "user_cookie": null
    });

    let (status, body) = post(&router, "/api/user", payload).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["success"], true);

    let data = &body["data"];
    assert!(data["id"].is_string());
    assert_eq!(data["name"], Value::Null);
    assert_eq!(data["email"], Value::Null);
    assert_eq!(data["auth_method"], Value::Null);
    assert_eq!(data["user_cookie"], Value::Null);
    assert!(data["created_at"].is_string());
    assert!(data["updated_at"].is_string());
}

#[sqlx::test]
async fn test_create_user_with_all_fields(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "Alice",
        "email": "alice@example.com",
        "auth_method": "password",
        "user_cookie": "cookie_abc_123"
    });

    let (status, body) = post(&router, "/api/user", payload).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["success"], true);

    let data = &body["data"];
    assert_eq!(data["name"], "Alice");
    assert_eq!(data["email"], "alice@example.com");
    assert_eq!(data["auth_method"], "password");
    assert_eq!(data["user_cookie"], "cookie_abc_123");
}

#[sqlx::test]
async fn test_create_user_duplicate_email_rejected(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    // Create first user with email
    create_user(&router, "User1", Some("dup@example.com")).await;

    // Try creating another user with the same email
    let payload = json!({
        "name": "User2",
        "email": "dup@example.com",
        "auth_method": null,
        "user_cookie": null
    });

    let (status, body) = post(&router, "/api/user", payload).await;
    // The service returns an error for duplicate email
    assert_ne!(status, StatusCode::CREATED, "should reject duplicate email");
    // Depending on error mapping, this should be a 400 or 500
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::INTERNAL_SERVER_ERROR,
        "expected error status, got {status}: {body}"
    );
}

#[sqlx::test]
async fn test_create_user_with_invalid_email_rejected(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "BadEmail",
        "email": "not-an-email",
        "auth_method": null,
        "user_cookie": null
    });

    let (status, _body) = post(&router, "/api/user", payload).await;
    // Invalid email should fail validation during deserialization
    assert_ne!(status, StatusCode::CREATED);
}

#[sqlx::test]
async fn test_create_user_guest_with_cookie(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": null,
        "email": null,
        "auth_method": "guest",
        "user_cookie": "guest_session_xyz"
    });

    let (status, body) = post(&router, "/api/user", payload).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let data = &body["data"];
    assert_eq!(data["auth_method"], "guest");
    assert_eq!(data["user_cookie"], "guest_session_xyz");
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Get
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_get_user_by_id(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let created = create_user(&router, "GetMe", Some("getme@example.com")).await;
    let user_id = created["id"].as_str().unwrap();

    let (status, body) = get(&router, &format!("/api/user/{user_id}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);

    let data = &body["data"];
    assert_eq!(data["id"], user_id);
    assert_eq!(data["name"], "GetMe");
    assert_eq!(data["email"], "getme@example.com");
}

#[sqlx::test]
async fn test_get_user_not_found(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let (status, _body) = get(&router, &format!("/api/user/{fake_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_get_user_by_email(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let created = create_user(&router, "EmailLookup", Some("lookup@example.com")).await;
    let user_id = created["id"].as_str().unwrap();

    let (status, body) = get(&router, "/api/user/email/lookup@example.com").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["id"], user_id);
    assert_eq!(body["data"]["email"], "lookup@example.com");
}

#[sqlx::test]
async fn test_get_user_by_email_not_found(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let (status, _body) = get(&router, "/api/user/email/nobody@example.com").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_get_user_by_cookie(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": null,
        "email": null,
        "auth_method": "guest",
        "user_cookie": "my_unique_cookie"
    });
    let (status, body) = post(&router, "/api/user", payload).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let user_id = body["data"]["id"].as_str().unwrap().to_string();

    let (status, body) = get(&router, "/api/user/cookie/my_unique_cookie").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["id"], user_id);
    assert_eq!(body["data"]["user_cookie"], "my_unique_cookie");
}

#[sqlx::test]
async fn test_get_user_by_cookie_not_found(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let (status, _body) = get(&router, "/api/user/cookie/nonexistent_cookie").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_get_user_by_name(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let created = create_user(&router, "FindByName", None).await;
    let user_id = created["id"].as_str().unwrap().to_string();

    let (status, body) = get(&router, "/api/user/name/FindByName").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["id"], user_id);
    assert_eq!(body["data"]["name"], "FindByName");
}

#[sqlx::test]
async fn test_get_user_by_name_not_found(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let (status, _body) = get(&router, "/api/user/name/DoesNotExist").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ═══════════════════════════════════════════════════════════════════
// Tests — List
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_list_users_empty(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let (status, body) = get(&router, "/api/users").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    assert!(body["data"].as_array().unwrap().is_empty());
}

#[sqlx::test]
async fn test_list_users_multiple(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    create_user(&router, "UserA", Some("a@example.com")).await;
    create_user(&router, "UserB", Some("b@example.com")).await;
    create_user(&router, "UserC", None).await;

    let (status, body) = get(&router, "/api/users").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);

    let users = body["data"].as_array().unwrap();
    assert_eq!(users.len(), 3);

    // Users are ordered by created_at DESC, so newest first
    let names: Vec<&str> = users
        .iter()
        .filter_map(|u| u["name"].as_str())
        .collect();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"UserA"));
    assert!(names.contains(&"UserB"));
    assert!(names.contains(&"UserC"));
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Update
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_update_user_name(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let created = create_user(&router, "OldName", Some("update@example.com")).await;
    let user_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": user_id,
        "name": "NewName"
    });

    let (status, body) = post(&router, "/api/update-user", update_payload).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["name"], "NewName");
    // Email should remain unchanged
    assert_eq!(body["data"]["email"], "update@example.com");
}

#[sqlx::test]
async fn test_update_user_email(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let created = create_user(&router, "EmailUpdate", Some("old@example.com")).await;
    let user_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": user_id,
        "email": "new@example.com"
    });

    let (status, body) = post(&router, "/api/update-user", update_payload).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["email"], "new@example.com");
    // Name should remain unchanged
    assert_eq!(body["data"]["name"], "EmailUpdate");
}

#[sqlx::test]
async fn test_update_user_multiple_fields(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let created = create_user(&router, "MultiUpdate", Some("multi@example.com")).await;
    let user_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": user_id,
        "name": "UpdatedMulti",
        "email": "updated_multi@example.com",
        "auth_method": "oauth",
        "user_cookie": "new_cookie_value"
    });

    let (status, body) = post(&router, "/api/update-user", update_payload).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let data = &body["data"];
    assert_eq!(data["name"], "UpdatedMulti");
    assert_eq!(data["email"], "updated_multi@example.com");
    assert_eq!(data["auth_method"], "oauth");
    assert_eq!(data["user_cookie"], "new_cookie_value");
}

#[sqlx::test]
async fn test_update_user_not_found(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let update_payload = json!({
        "id": fake_id,
        "name": "Ghost"
    });

    let (status, _body) = post(&router, "/api/update-user", update_payload).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_update_user_duplicate_email_rejected(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    create_user(&router, "Holder", Some("taken@example.com")).await;
    let second = create_user(&router, "Stealer", Some("original@example.com")).await;
    let second_id = second["id"].as_str().unwrap();

    // Try to update second user's email to the first user's email
    let update_payload = json!({
        "id": second_id,
        "email": "taken@example.com"
    });

    let (status, _body) = post(&router, "/api/update-user", update_payload).await;
    assert_ne!(status, StatusCode::OK, "should reject duplicate email");
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::INTERNAL_SERVER_ERROR,
        "expected error status, got {status}"
    );
}

#[sqlx::test]
async fn test_update_user_same_email_allowed(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let created = create_user(&router, "SameEmail", Some("same@example.com")).await;
    let user_id = created["id"].as_str().unwrap();

    // Update with the same email should succeed (common in "save" operations)
    let update_payload = json!({
        "id": user_id,
        "email": "same@example.com",
        "name": "SameEmailRenamed"
    });

    let (status, body) = post(&router, "/api/update-user", update_payload).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["name"], "SameEmailRenamed");
    assert_eq!(body["data"]["email"], "same@example.com");
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Delete
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_delete_user(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let created = create_user(&router, "ToDelete", None).await;
    let user_id = created["id"].as_str().unwrap();

    let (status, body) = del(&router, &format!("/api/user/{user_id}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);

    // Verify it's gone
    let (status, _body) = get(&router, &format!("/api/user/{user_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_delete_user_not_found(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let (status, _body) = del(&router, &format!("/api/user/{fake_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_delete_user_idempotent(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let created = create_user(&router, "DeleteTwice", None).await;
    let user_id = created["id"].as_str().unwrap();

    // First delete succeeds
    let (status, _body) = del(&router, &format!("/api/user/{user_id}")).await;
    assert_eq!(status, StatusCode::OK);

    // Second delete returns not found
    let (status, _body) = del(&router, &format!("/api/user/{user_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Edge cases
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_create_and_retrieve_preserves_all_fields(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "FullUser",
        "email": "full@example.com",
        "auth_method": "google",
        "user_cookie": "session_token_full"
    });

    let (status, body) = post(&router, "/api/user", payload).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let user_id = body["data"]["id"].as_str().unwrap();

    // Fetch by ID and verify all fields match
    let (status, body) = get(&router, &format!("/api/user/{user_id}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let data = &body["data"];
    assert_eq!(data["name"], "FullUser");
    assert_eq!(data["email"], "full@example.com");
    assert_eq!(data["auth_method"], "google");
    assert_eq!(data["user_cookie"], "session_token_full");

    // Also verify via email lookup
    let (status, body) = get(&router, "/api/user/email/full@example.com").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["id"], user_id);

    // Also verify via cookie lookup
    let (status, body) = get(&router, "/api/user/cookie/session_token_full").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["id"], user_id);

    // Also verify via name lookup
    let (status, body) = get(&router, "/api/user/name/FullUser").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["id"], user_id);
}

#[sqlx::test]
async fn test_user_timestamps_are_set(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let created = create_user(&router, "Timestamped", None).await;
    let created_at = created["created_at"].as_str().unwrap();
    let updated_at = created["updated_at"].as_str().unwrap();

    // Both timestamps should be set and equal at creation
    assert!(!created_at.is_empty());
    assert!(!updated_at.is_empty());
}

#[sqlx::test]
async fn test_list_users_order(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    // Create users in sequence
    let first = create_user(&router, "First", None).await;
    let second = create_user(&router, "Second", None).await;
    let third = create_user(&router, "Third", None).await;

    let (status, body) = get(&router, "/api/users").await;
    assert_eq!(status, StatusCode::OK);

    let users = body["data"].as_array().unwrap();
    assert_eq!(users.len(), 3);

    // Users are ordered by created_at DESC (newest first)
    assert_eq!(users[0]["name"], "Third");
    assert_eq!(users[1]["name"], "Second");
    assert_eq!(users[2]["name"], "First");
}