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

/// Seed the database with app_settings so the setup middleware allows requests.
async fn seed(pool: &PgPool) {
    let access_hash = sha256_hex("test_access_code");
    sqlx::query("INSERT INTO app_settings (id, access_hash) VALUES (1, $1)")
        .bind(&access_hash)
        .execute(pool)
        .await
        .unwrap();
}

/// Seed the database AND create an admin user.
/// Returns (admin_user_id, admin_session_token).
async fn seed_with_admin(pool: &PgPool) -> (Uuid, String) {
    seed(pool).await;

    let router = app(pool.clone());

    // Create admin user directly in DB (bootstrap — no admin exists yet)
    let admin_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (name, email, auth_method, password_hash, role) \
         VALUES ('admin', 'admin@example.com', 'Password', '$argon2id$v=19$m=19456,t=2,p=1$fake$fake', 'Admin') \
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    // Create a session for the admin
    let token: String = sqlx::query_scalar(
        "INSERT INTO user_sessions (user_id, token, expires_at) \
         VALUES ($1, gen_random_uuid()::text, NOW() + INTERVAL '1 day') \
         RETURNING token",
    )
    .bind(admin_id)
    .fetch_one(pool)
    .await
    .unwrap();

    (admin_id, token)
}

/// Build a fully-wired Router backed by the given pool.
fn app(pool: PgPool) -> Router {
    let setup_done = Arc::new(AtomicBool::new(true));
    let state = build_state(setup_done, pool, Arc::new(Notify::new()), std::path::PathBuf::from("/tmp/test_uploads"), None, "http://localhost:5173".to_string());
    build_app(state)
}

/// Fire a request and return (status, parsed JSON body).
async fn send(
    router: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    send_with_auth(router, method, uri, body, None).await
}

/// Fire a request with an optional Bearer token.
/// Always includes the `site_access` cookie so SiteAccess-protected
/// routes work even without a Bearer token.
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

    let site_access_hash = sha256_hex("test_access_code");
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("cookie", format!("site_access={}", site_access_hash));

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

/// Convenience: POST with JSON body.
async fn post(router: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    send(router, Method::POST, uri, Some(body)).await
}

/// Convenience: POST with JSON body and auth token.
async fn post_auth(
    router: &Router,
    uri: &str,
    body: Value,
    token: &str,
) -> (StatusCode, Value) {
    send_with_auth(router, Method::POST, uri, Some(body), Some(token)).await
}

/// Convenience: PUT with JSON body and auth token.
async fn put_auth(
    router: &Router,
    uri: &str,
    body: Value,
    token: &str,
) -> (StatusCode, Value) {
    send_with_auth(router, Method::PUT, uri, Some(body), Some(token)).await
}

/// Convenience: GET.
async fn get(router: &Router, uri: &str) -> (StatusCode, Value) {
    send(router, Method::GET, uri, None).await
}

/// Convenience: GET with auth token.
async fn get_auth(router: &Router, uri: &str, token: &str) -> (StatusCode, Value) {
    send_with_auth(router, Method::GET, uri, None, Some(token)).await
}

/// Convenience: DELETE (no auth).
async fn del(router: &Router, uri: &str) -> (StatusCode, Value) {
    send(router, Method::DELETE, uri, None).await
}

/// Convenience: DELETE with auth token.
async fn del_auth(router: &Router, uri: &str, token: &str) -> (StatusCode, Value) {
    send_with_auth(router, Method::DELETE, uri, None, Some(token)).await
}

/// Create a NameWithCookie (guest) user via the API and return the response data.
async fn create_guest(router: &Router, name: &str) -> Value {
    let payload = json!({
        "name": name,
        "email": null,
        "auth_method": "NameWithCookie",
        "role": "User"
    });
    let (status, body) = post(router, "/api/user", payload).await;
    assert_eq!(status, StatusCode::CREATED, "create guest failed: {body}");
    assert_eq!(body["success"], true);
    body["data"].clone()
}

/// Create a guest user via the auth endpoint and return (data, session_token).
async fn create_guest_via_auth(router: &Router, name: &str) -> (Value, String) {
    let payload = json!({ "name": name });
    let (status, body) = post(router, "/api/auth/guest", payload).await;
    assert_eq!(status, StatusCode::CREATED, "create guest via auth failed: {body}");
    assert_eq!(body["success"], true);
    let token = body["data"]["token"].as_str().unwrap().to_string();
    let user = body["data"]["user"].clone();
    (user, token)
}

/// Admin-register a password user and return the response data.
async fn admin_create_password_user(
    router: &Router,
    token: &str,
    name: &str,
    email: &str,
    password: &str,
    role: &str,
) -> (StatusCode, Value) {
    let payload = json!({
        "name": name,
        "email": email,
        "password": password,
        "role": role,
    });
    post_auth(router, "/api/admin/users/register", payload, token).await
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Guest (NameWithCookie) creation via /api/user
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_create_guest_user_minimal(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "GuestBob",
        "email": null,
        "auth_method": "NameWithCookie",
        "role": "User"
    });

    let (status, body) = post(&router, "/api/user", payload).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["success"], true);

    let data = &body["data"];
    assert!(data["id"].is_string());
    assert_eq!(data["name"], "GuestBob");
    assert_eq!(data["email"], Value::Null);
    assert_eq!(data["auth_method"], "NameWithCookie");
    assert_eq!(data["role"], "User");
    // password_hash is #[serde(skip)] — should not appear in response
    assert_eq!(data.get("password_hash"), None);
    assert!(data["created_at"].is_string());
    assert!(data["updated_at"].is_string());
}

#[sqlx::test]
async fn test_create_password_user_via_public_endpoint_rejected(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    // Attempting to create a Password user via the public endpoint should fail
    let payload = json!({
        "name": "NotAllowed",
        "email": "na@example.com",
        "auth_method": "Password",
        "role": "User"
    });

    let (status, _body) = post(&router, "/api/user", payload).await;
    assert_ne!(status, StatusCode::CREATED, "should reject Password creation");
}

#[sqlx::test]
async fn test_create_guest_with_invalid_auth_method_rejected(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "BadAuth",
        "email": null,
        "auth_method": "invalid_method",
        "role": null
    });

    let (status, _body) = post(&router, "/api/user", payload).await;
    assert_ne!(status, StatusCode::CREATED);
}

#[sqlx::test]
async fn test_create_guest_with_email(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "EmailGuest",
        "email": "guest@example.com",
        "auth_method": "NameWithCookie",
        "role": "User"
    });

    let (status, body) = post(&router, "/api/user", payload).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["email"], "guest@example.com");
}

#[sqlx::test]
async fn test_create_guest_with_invalid_email_rejected(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "BadEmail",
        "email": "not-an-email",
        "auth_method": "NameWithCookie",
        "role": null
    });

    let (status, _body) = post(&router, "/api/user", payload).await;
    assert_ne!(status, StatusCode::CREATED);
}

#[sqlx::test]
async fn test_create_guest_duplicate_email_rejected(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let payload1 = json!({
        "name": "Guest1",
        "email": "dup@example.com",
        "auth_method": "NameWithCookie",
        "role": "User"
    });
    let (status, _) = post(&router, "/api/user", payload1).await;
    assert_eq!(status, StatusCode::CREATED);

    let payload2 = json!({
        "name": "Guest2",
        "email": "dup@example.com",
        "auth_method": "NameWithCookie",
        "role": "User"
    });
    let (status, _body) = post(&router, "/api/user", payload2).await;
    assert_ne!(status, StatusCode::CREATED, "should reject duplicate email");
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Guest creation via /api/auth/guest
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_auth_guest_creates_user_and_session(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let (user, token) = create_guest_via_auth(&router, "AuthGuest").await;
    assert_eq!(user["name"], "AuthGuest");
    assert_eq!(user["auth_method"], "NameWithCookie");
    assert_eq!(user["role"], "User");
    assert!(!token.is_empty());

    // Session should be valid — /api/auth/me should work
    let (status, body) = get_auth(&router, "/api/auth/me", &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["name"], "AuthGuest");
}

#[sqlx::test]
async fn test_auth_me_without_session_returns_401(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let (status, _body) = get(&router, "/api/auth/me").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn test_auth_me_with_invalid_token_returns_401(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let (status, _body) = get_auth(&router, "/api/auth/me", "invalid-token").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Get
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_get_user_by_id(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    // GET /api/user/:id requires SiteAccess — cookie is included automatically
    let (created, token) = create_guest_via_auth(&router, "GetMe").await;
    let user_id = created["id"].as_str().unwrap();

    let (status, body) = get_auth(&router, &format!("/api/user/{user_id}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["id"], user_id);
    assert_eq!(body["data"]["name"], "GetMe");
}

#[sqlx::test]
async fn test_get_user_not_found(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let (_, token) = create_guest_via_auth(&router, "Dummy").await;
    let fake_id = Uuid::new_v4();
    let (status, _body) = get_auth(&router, &format!("/api/user/{fake_id}"), &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_get_user_by_email(pool: PgPool) {
    // GET /api/user/email/:email requires AdminUser
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "EmailLookup",
        "email": "lookup@example.com",
        "auth_method": "NameWithCookie",
        "role": "User"
    });
    let (status, body) = post(&router, "/api/user", payload).await;
    assert_eq!(status, StatusCode::CREATED);
    let user_id = body["data"]["id"].as_str().unwrap().to_string();

    let (status, body) = get_auth(&router, "/api/user/email/lookup@example.com", &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["id"], user_id);
    assert_eq!(body["data"]["email"], "lookup@example.com");
}

#[sqlx::test]
async fn test_get_user_by_email_not_found(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let (status, _body) = get_auth(&router, "/api/user/email/nobody@example.com", &admin_token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_get_user_by_email_without_admin_rejected(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    // Without admin auth, should get 401
    let (status, _body) = get(&router, "/api/user/email/nobody@example.com").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn test_get_user_by_name(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    // GET /api/user/name/:name requires SiteAccess — cookie is included automatically
    let (created, token) = create_guest_via_auth(&router, "FindByName").await;
    let user_id = created["id"].as_str().unwrap().to_string();

    let (status, body) = get_auth(&router, "/api/user/name/FindByName", &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["id"], user_id);
    assert_eq!(body["data"]["name"], "FindByName");
}

#[sqlx::test]
async fn test_get_user_by_name_not_found(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let (_, token) = create_guest_via_auth(&router, "Dummy").await;
    let (status, _body) = get_auth(&router, "/api/user/name/DoesNotExist", &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ═══════════════════════════════════════════════════════════════════
// Tests — List
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_list_users_requires_admin(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    // GET /api/users requires AdminUser — without auth, should get 401
    let (status, _body) = get(&router, "/api/users").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn test_list_users_no_extra_users(pool: PgPool) {
    // GET /api/users requires AdminUser
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let (status, body) = get_auth(&router, "/api/users", &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    // Only the admin user exists
    let users = body["data"].as_array().unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0]["name"], "admin");
}

#[sqlx::test]
async fn test_list_users_multiple(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    create_guest(&router, "UserA").await;
    create_guest(&router, "UserB").await;
    create_guest(&router, "UserC").await;

    let (status, body) = get_auth(&router, "/api/users", &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let users = body["data"].as_array().unwrap();
    assert_eq!(users.len(), 4); // 3 guests + 1 admin

    let names: Vec<&str> = users.iter().filter_map(|u| u["name"].as_str()).collect();
    assert!(names.contains(&"UserA"));
    assert!(names.contains(&"UserB"));
    assert!(names.contains(&"UserC"));
    assert!(names.contains(&"admin"));
}

#[sqlx::test]
async fn test_list_users_order(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    create_guest(&router, "First").await;
    create_guest(&router, "Second").await;
    create_guest(&router, "Third").await;

    let (status, body) = get_auth(&router, "/api/users", &admin_token).await;
    assert_eq!(status, StatusCode::OK);

    let users = body["data"].as_array().unwrap();
    assert_eq!(users.len(), 4); // 3 guests + 1 admin
    // Users are ordered by created_at DESC (newest first)
    assert_eq!(users[0]["name"], "Third");
    assert_eq!(users[1]["name"], "Second");
    assert_eq!(users[2]["name"], "First");
    assert_eq!(users[3]["name"], "admin");
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Update (name and email only)
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_update_user_requires_auth(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let created = create_guest(&router, "OldName").await;
    let user_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": user_id,
        "name": "NewName"
    });

    // POST /api/update-user requires AuthUser — without auth, should get 401
    let (status, _body) = post(&router, "/api/update-user", update_payload).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn test_update_user_name(pool: PgPool) {
    // POST /api/update-user requires AuthUser
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let created = create_guest(&router, "OldName").await;
    let user_id = created["id"].as_str().unwrap();

    let update_payload = json!({
        "id": user_id,
        "name": "NewName"
    });

    let (status, body) = post_auth(&router, "/api/update-user", update_payload, &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["name"], "NewName");
}

#[sqlx::test]
async fn test_update_user_email(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "EmailUpdate",
        "email": "old@example.com",
        "auth_method": "NameWithCookie",
        "role": "User"
    });
    let (status, body) = post(&router, "/api/user", payload).await;
    assert_eq!(status, StatusCode::CREATED);
    let user_id = body["data"]["id"].as_str().unwrap().to_string();

    let update_payload = json!({
        "id": user_id,
        "email": "new@example.com"
    });

    let (status, body) = post_auth(&router, "/api/update-user", update_payload, &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["email"], "new@example.com");
    assert_eq!(body["data"]["name"], "EmailUpdate");
}

#[sqlx::test]
async fn test_update_user_not_found(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let update_payload = json!({
        "id": fake_id,
        "name": "Ghost"
    });

    let (status, _body) = post_auth(&router, "/api/update-user", update_payload, &admin_token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_update_user_duplicate_email_rejected(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let payload1 = json!({
        "name": "Holder",
        "email": "taken@example.com",
        "auth_method": "NameWithCookie",
        "role": "User"
    });
    let (status, _) = post(&router, "/api/user", payload1).await;
    assert_eq!(status, StatusCode::CREATED);

    let payload2 = json!({
        "name": "Stealer",
        "email": "original@example.com",
        "auth_method": "NameWithCookie",
        "role": "User"
    });
    let (status, body) = post(&router, "/api/user", payload2).await;
    assert_eq!(status, StatusCode::CREATED);
    let second_id = body["data"]["id"].as_str().unwrap().to_string();

    let update_payload = json!({
        "id": second_id,
        "email": "taken@example.com"
    });
    let (status, _body) = post_auth(&router, "/api/update-user", update_payload, &admin_token).await;
    assert_ne!(status, StatusCode::OK, "should reject duplicate email");
}

#[sqlx::test]
async fn test_update_user_same_email_allowed(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let payload = json!({
        "name": "SameEmail",
        "email": "same@example.com",
        "auth_method": "NameWithCookie",
        "role": "User"
    });
    let (status, body) = post(&router, "/api/user", payload).await;
    assert_eq!(status, StatusCode::CREATED);
    let user_id = body["data"]["id"].as_str().unwrap().to_string();

    let update_payload = json!({
        "id": user_id,
        "email": "same@example.com",
        "name": "SameEmailRenamed"
    });

    let (status, body) = post_auth(&router, "/api/update-user", update_payload, &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["name"], "SameEmailRenamed");
    assert_eq!(body["data"]["email"], "same@example.com");
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Delete
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_delete_user_requires_admin(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let created = create_guest(&router, "ToDelete").await;
    let user_id = created["id"].as_str().unwrap();

    // DELETE /api/user/:id requires AdminUser — without auth, should get 401
    let (status, _body) = del(&router, &format!("/api/user/{user_id}")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn test_delete_user(pool: PgPool) {
    // DELETE /api/user/:id requires AdminUser
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let created = create_guest(&router, "ToDelete").await;
    let user_id = created["id"].as_str().unwrap();

    let (status, body) = del_auth(&router, &format!("/api/user/{user_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);

    let (status, _body) = get_auth(&router, &format!("/api/user/{user_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_delete_user_not_found(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let fake_id = Uuid::new_v4();
    let (status, _body) = del_auth(&router, &format!("/api/user/{fake_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_delete_user_idempotent(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let created = create_guest(&router, "DeleteTwice").await;
    let user_id = created["id"].as_str().unwrap();

    let (status, _body) = del_auth(&router, &format!("/api/user/{user_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _body) = del_auth(&router, &format!("/api/user/{user_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_delete_removes_from_list(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let _keep = create_guest(&router, "KeepUser").await;
    let remove = create_guest(&router, "RemoveUser").await;
    let remove_id = remove["id"].as_str().unwrap();

    let (status, body) = get_auth(&router, "/api/users", &admin_token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().unwrap().len(), 3); // admin + 2 guests

    let (status, _body) = del_auth(&router, &format!("/api/user/{remove_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = get_auth(&router, "/api/users", &admin_token).await;
    assert_eq!(status, StatusCode::OK);
    let users = body["data"].as_array().unwrap();
    assert_eq!(users.len(), 2); // admin + KeepUser
    let names: Vec<&str> = users.iter().filter_map(|u| u["name"].as_str()).collect();
    assert!(names.contains(&"KeepUser"));
    assert!(names.contains(&"admin"));
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Edge cases
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_user_timestamps_are_set(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let created = create_guest(&router, "Timestamped").await;
    let created_at = created["created_at"].as_str().unwrap();
    let updated_at = created["updated_at"].as_str().unwrap();

    assert!(!created_at.is_empty());
    assert!(!updated_at.is_empty());
}

#[sqlx::test]
async fn test_password_hash_never_in_response(pool: PgPool) {
    let (admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    // Admin creates a password user
    let (status, body) = admin_create_password_user(
        &router,
        &admin_token,
        "PwUser",
        "pw@example.com",
        "securepass123",
        "User",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    // password_hash must not appear in the response
    assert_eq!(body["data"].get("password_hash"), None);

    // Also verify via GET
    let user_id = body["data"]["id"].as_str().unwrap();
    let (status, body) = get_auth(&router, &format!("/api/user/{user_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].get("password_hash"), None);
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Admin: register password user
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_admin_register_password_user(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let (status, body) = admin_create_password_user(
        &router,
        &admin_token,
        "NewUser",
        "newuser@example.com",
        "password123",
        "User",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["success"], true);

    let data = &body["data"];
    assert_eq!(data["name"], "NewUser");
    assert_eq!(data["email"], "newuser@example.com");
    assert_eq!(data["auth_method"], "Password");
    assert_eq!(data["role"], "User");
}

#[sqlx::test]
async fn test_admin_register_password_user_as_admin_role(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let (status, body) = admin_create_password_user(
        &router,
        &admin_token,
        "NewAdmin",
        "newadmin@example.com",
        "password123",
        "Admin",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["role"], "Admin");
}

#[sqlx::test]
async fn test_admin_register_password_user_duplicate_email_rejected(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    // First user
    let (status, _) = admin_create_password_user(
        &router,
        &admin_token,
        "First",
        "dup@example.com",
        "password123",
        "User",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Second user with same email
    let (status, _body) = admin_create_password_user(
        &router,
        &admin_token,
        "Second",
        "dup@example.com",
        "password123",
        "User",
    )
    .await;
    assert_ne!(status, StatusCode::CREATED, "should reject duplicate email");
}

#[sqlx::test]
async fn test_admin_register_password_user_short_password_rejected(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let (status, _body) = admin_create_password_user(
        &router,
        &admin_token,
        "ShortPw",
        "short@example.com",
        "abc",
        "User",
    )
    .await;
    assert_ne!(status, StatusCode::CREATED, "should reject short password");
}

#[sqlx::test]
async fn test_register_password_user_without_admin_rejected(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    // No auth token — should be rejected
    let payload = json!({
        "name": "Sneaky",
        "email": "sneaky@example.com",
        "password": "password123",
        "role": "User"
    });
    let (status, _body) = post(&router, "/api/admin/users/register", payload).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn test_register_password_user_with_non_admin_session_rejected(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    // Create a guest with a session — they're a User, not Admin
    let (_user, token) = create_guest_via_auth(&router, "NotAdmin").await;

    let (status, _body) = admin_create_password_user(
        &router,
        &token,
        "Attempt",
        "attempt@example.com",
        "password123",
        "User",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Admin: upgrade guest to password
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_admin_upgrade_guest_to_password(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    // Create a guest user
    let guest = create_guest(&router, "UpgradeMe").await;
    let guest_id = guest["id"].as_str().unwrap();
    assert_eq!(guest["auth_method"], "NameWithCookie");

    // Admin upgrades the guest
    let payload = json!({
        "email": "upgraded@example.com",
        "password": "newpassword123",
        "role": "User"
    });
    let (status, body) = post_auth(
        &router,
        &format!("/api/admin/users/{guest_id}/upgrade"),
        payload,
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let data = &body["data"];
    assert_eq!(data["id"], guest_id);
    assert_eq!(data["name"], "UpgradeMe");
    assert_eq!(data["auth_method"], "Password");
    assert_eq!(data["email"], "upgraded@example.com");
    assert_eq!(data["role"], "User");
}

#[sqlx::test]
async fn test_admin_upgrade_already_password_user_rejected(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    // Create a password user
    let (status, body) = admin_create_password_user(
        &router,
        &admin_token,
        "AlreadyPw",
        "already@example.com",
        "password123",
        "User",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let user_id = body["data"]["id"].as_str().unwrap();

    // Try to upgrade — should fail
    let payload = json!({
        "email": "new@example.com",
        "password": "newpassword123",
        "role": "Admin"
    });
    let (status, _body) = post_auth(
        &router,
        &format!("/api/admin/users/{user_id}/upgrade"),
        payload,
        &admin_token,
    )
    .await;
    assert_ne!(status, StatusCode::OK, "should reject upgrading a Password user");
}

#[sqlx::test]
async fn test_upgrade_without_admin_rejected(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let guest = create_guest(&router, "CantUpgrade").await;
    let guest_id = guest["id"].as_str().unwrap();

    let payload = json!({
        "email": "nope@example.com",
        "password": "password123",
        "role": "User"
    });
    let (status, _body) = post(&router, &format!("/api/admin/users/{guest_id}/upgrade"), payload).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Admin: change role
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_admin_change_role(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    // Create a password user with role User
    let (status, body) = admin_create_password_user(
        &router,
        &admin_token,
        "Promotable",
        "promotable@example.com",
        "password123",
        "User",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let user_id = body["data"]["id"].as_str().unwrap();
    assert_eq!(body["data"]["role"], "User");

    // Admin promotes to Admin
    let payload = json!({ "role": "Admin" });
    let (status, body) = put_auth(
        &router,
        &format!("/api/admin/users/{user_id}/role"),
        payload,
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["role"], "Admin");
}

#[sqlx::test]
async fn test_admin_change_role_of_guest_rejected(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let guest = create_guest(&router, "GuestNoRole").await;
    let guest_id = guest["id"].as_str().unwrap();

    let payload = json!({ "role": "Admin" });
    let (status, _body) = put_auth(
        &router,
        &format!("/api/admin/users/{guest_id}/role"),
        payload,
        &admin_token,
    )
    .await;
    assert_ne!(
        status,
        StatusCode::OK,
        "should reject role change for NameWithCookie user"
    );
}

#[sqlx::test]
async fn test_change_role_without_admin_rejected(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let payload = json!({ "role": "Admin" });
    let fake_id = Uuid::new_v4();
    let (status, _body) = send_with_auth(
        &router,
        Method::PUT,
        &format!("/api/admin/users/{fake_id}/role"),
        Some(payload),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Login / Logout
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_login_with_valid_credentials(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    // Admin creates a password user
    let (status, _body) = admin_create_password_user(
        &router,
        &admin_token,
        "LoginUser",
        "login@example.com",
        "mypassword123",
        "User",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Login
    let payload = json!({
        "email": "login@example.com",
        "password": "mypassword123"
    });
    let (status, body) = post(&router, "/api/auth/login", payload).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["user"]["name"], "LoginUser");
    assert!(body["data"]["token"].is_string());

    // The token should work for /api/auth/me
    let token = body["data"]["token"].as_str().unwrap();
    let (status, body) = get_auth(&router, "/api/auth/me", token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["name"], "LoginUser");
}

#[sqlx::test]
async fn test_login_with_wrong_password_rejected(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    let (status, _) = admin_create_password_user(
        &router,
        &admin_token,
        "WrongPw",
        "wrongpw@example.com",
        "correctpassword",
        "User",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let payload = json!({
        "email": "wrongpw@example.com",
        "password": "incorrectpassword"
    });
    let (status, _body) = post(&router, "/api/auth/login", payload).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn test_login_with_nonexistent_email_rejected(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "email": "nobody@example.com",
        "password": "password123"
    });
    let (status, _body) = post(&router, "/api/auth/login", payload).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn test_login_with_guest_email_rejected(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    // Create a guest with an email
    let payload = json!({
        "name": "GuestWithEmail",
        "email": "guest@example.com",
        "auth_method": "NameWithCookie",
        "role": "User"
    });
    let (status, _) = post(&router, "/api/user", payload).await;
    assert_eq!(status, StatusCode::CREATED);

    // Try to login with that email — should fail (not a Password user)
    let payload = json!({
        "email": "guest@example.com",
        "password": "anything"
    });
    let (status, _body) = post(&router, "/api/auth/login", payload).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn test_logout_clears_cookies(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/logout")
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Check that Set-Cookie headers are present to clear cookies
    let set_cookies: Vec<&str> = resp
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();

    assert!(
        set_cookies.iter().any(|c| c.contains("session_token=;") && c.contains("Max-Age=0")),
        "Expected session_token clearing cookie, got: {:?}",
        set_cookies
    );
    assert!(
        set_cookies.iter().any(|c| c.contains("site_access=;") && c.contains("Max-Age=0")),
        "Expected site_access clearing cookie, got: {:?}",
        set_cookies
    );
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Site access
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_site_access_with_correct_code(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let payload = json!({ "code": "test_access_code" });
    let (status, body) = post(&router, "/api/auth/site-access", payload).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["granted"], true);
}

#[sqlx::test]
async fn test_site_access_with_wrong_code(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let payload = json!({ "code": "wrong_code" });
    let (status, _body) = post(&router, "/api/auth/site-access", payload).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn test_site_access_via_url_hash(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let hash = sha256_hex("test_access_code");
    let (status, body) = get(&router, &format!("/api/auth/site-access/{hash}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["granted"], true);
}

#[sqlx::test]
async fn test_site_access_via_url_wrong_hash(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    let (status, _body) = get(&router, "/api/auth/site-access/badhash").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ═══════════════════════════════════════════════════════════════════
// Tests — Full flow
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_full_guest_flow(pool: PgPool) {
    seed(&pool).await;
    let router = app(pool);

    // 1. Create guest via auth endpoint (gets session)
    let (user, token) = create_guest_via_auth(&router, "FullFlowGuest").await;
    assert_eq!(user["auth_method"], "NameWithCookie");
    assert_eq!(user["role"], "User");

    // 2. Can access /api/auth/me
    let (status, body) = get_auth(&router, "/api/auth/me", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["name"], "FullFlowGuest");

    // 3. Can be found by name
    let (status, body) = get_auth(&router, "/api/user/name/FullFlowGuest", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["id"], user["id"]);
}

#[sqlx::test]
async fn test_full_admin_upgrade_flow(pool: PgPool) {
    let (_admin_id, admin_token) = seed_with_admin(&pool).await;
    let router = app(pool);

    // 1. Create a guest
    let guest = create_guest(&router, "WillUpgrade").await;
    let guest_id = guest["id"].as_str().unwrap();

    // 2. Admin upgrades the guest to a password user
    let payload = json!({
        "email": "willupgrade@example.com",
        "password": "securepass123",
        "role": "User"
    });
    let (status, body) = post_auth(
        &router,
        &format!("/api/admin/users/{guest_id}/upgrade"),
        payload,
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["auth_method"], "Password");
    assert_eq!(body["data"]["role"], "User");
    // User ID should be preserved
    assert_eq!(body["data"]["id"], guest_id);
    assert_eq!(body["data"]["name"], "WillUpgrade");

    // 3. The upgraded user can now login with password
    let login_payload = json!({
        "email": "willupgrade@example.com",
        "password": "securepass123"
    });
    let (status, body) = post(&router, "/api/auth/login", login_payload).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let user_token = body["data"]["token"].as_str().unwrap();

    // 4. Can access /api/auth/me with new session
    let (status, body) = get_auth(&router, "/api/auth/me", user_token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["name"], "WillUpgrade");
}
