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

use crate::{app::build_app, state::build_state, utils::password::sha256_hex};

// ═══════════════════════════════════════════════════════════════════
// Test helpers
// ═══════════════════════════════════════════════════════════════════

/// Seed the database with app_settings, a user (with session), and a restaurant.
/// Returns (user_id, restaurant_id, session_token).
async fn seed(pool: &PgPool) -> (Uuid, Uuid, String) {
    // app_settings (needed by get_max_menu_nesting_depth)
    let access_hash = sha256_hex("test_access_code");
    sqlx::query("INSERT INTO app_settings (id, access_hash) VALUES (1, $1)")
        .bind(&access_hash)
        .execute(pool)
        .await
        .unwrap();

    // user
    let user_id: Uuid =
        sqlx::query_scalar("INSERT INTO users (name, auth_method, role) VALUES ('tester', 'Password', 'User') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();

    // session
    let token: String = sqlx::query_scalar(
        "INSERT INTO user_sessions (user_id, token, expires_at) \
         VALUES ($1, gen_random_uuid()::text, NOW() + INTERVAL '1 day') \
         RETURNING token",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();

    // restaurant
    let rest_id: Uuid = sqlx::query_scalar(
        "INSERT INTO restaurants (name, created_by, updated_by) VALUES ('Test Restaurant', $1, $1) RETURNING id",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();

    (user_id, rest_id, token)
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

/// Build a CreateMenuRequest JSON with one section containing one item.
fn full_menu_json(restaurant_id: Uuid) -> Value {
    json!({
        "restaurant_id": restaurant_id,
        "name": "Lunch Menu",
        "description": "Our lunch offerings",
        "is_active": true,
        "permanent": false,
        "sections": [
            {
                "menu_id": restaurant_id,
                "parent_id": null,
                "name": "Main Dishes",
                "description": "Mains",
                "position": 0,
                "is_active": true,
                "items": [
                    {
                        "section_id": restaurant_id,
                        "position": 0,
                        "price_override_cents": null,
                        "is_available": true,
                        "item": {
                            "restaurant_id": restaurant_id,
                            "name": "Nasi Goreng",
                            "description": "Indonesian fried rice",
                            "base_price_cents": 1500,
                            "image_url": null,
                            "tags": []
                        }
                    }
                ],
                "subsections": []
            }
        ]
    })
}

/// Create a menu via the API and return the response data object.
async fn create_menu(router: &Router, restaurant_id: Uuid, token: &str) -> Value {
    let (status, body) = post_auth(router, "/api/menus", full_menu_json(restaurant_id), token).await;
    assert_eq!(status, StatusCode::CREATED, "create menu failed: {body}");
    assert_eq!(body["success"], true);
    body["data"].clone()
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

// ── Create ────────────────────────────────────────────────────────

#[sqlx::test]
async fn test_create_empty_menu(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "restaurant_id": restaurant_id,
        "name": "Empty Menu",
        "description": null,
        "is_active": false,
        "permanent": true,
        "sections": []
    });

    let (status, body) = post_auth(&router, "/api/menus", payload, &token).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["success"], true);

    let data = &body["data"];
    assert_eq!(data["name"], "Empty Menu");
    assert_eq!(data["is_active"], false);
    assert_eq!(data["permanent"], true);
    assert_eq!(data["restaurant_id"], restaurant_id.to_string());
    assert!(data["sections"].as_array().unwrap().is_empty());
}

#[sqlx::test]
async fn test_create_menu_with_sections_and_items(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let data = create_menu(&router, restaurant_id, &token).await;

    assert_eq!(data["name"], "Lunch Menu");
    let sections = data["sections"].as_array().unwrap();
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0]["name"], "Main Dishes");

    let items = sections[0]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["item"]["name"], "Nasi Goreng");
    assert_eq!(items[0]["item"]["base_price_cents"], 1500);
    assert_eq!(items[0]["is_available"], true);
}

#[sqlx::test]
async fn test_create_menu_with_subsections(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    let payload = json!({
        "restaurant_id": restaurant_id,
        "name": "Nested Menu",
        "description": null,
        "is_active": true,
        "permanent": false,
        "sections": [
            {
                "menu_id": restaurant_id,
                "parent_id": null,
                "name": "Food",
                "description": null,
                "position": 0,
                "is_active": true,
                "items": [],
                "subsections": [
                    {
                        "menu_id": restaurant_id,
                        "parent_id": null,
                        "name": "Rice Dishes",
                        "description": null,
                        "position": 0,
                        "is_active": true,
                        "items": [],
                        "subsections": []
                    }
                ]
            }
        ]
    });

    let (status, body) = post_auth(&router, "/api/menus", payload, &token).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let sections = body["data"]["sections"].as_array().unwrap();
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0]["name"], "Food");

    let subs = sections[0]["subsections"].as_array().unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0]["name"], "Rice Dishes");
}

// ── Get ───────────────────────────────────────────────────────────

#[sqlx::test]
async fn test_get_menu(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();

    let (status, body) = get(&router, &format!("/api/menus/{menu_id}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["id"], menu_id);
    assert_eq!(body["data"]["name"], "Lunch Menu");
    assert_eq!(
        body["data"]["sections"].as_array().unwrap().len(),
        1
    );
}

#[sqlx::test]
async fn test_get_menu_not_found(pool: PgPool) {
    let _ = seed(&pool).await;
    let router = app(pool);
    let fake_id = Uuid::new_v4();

    let (status, body) = get(&router, &format!("/api/menus/{fake_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["success"], false);
}

// ── List ──────────────────────────────────────────────────────────

#[sqlx::test]
async fn test_list_menus_for_restaurant(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    // Create two menus.
    create_menu(&router, restaurant_id, &token).await;

    let payload2 = json!({
        "restaurant_id": restaurant_id,
        "name": "Dinner Menu",
        "description": null,
        "is_active": false,
        "permanent": false,
        "sections": []
    });
    let (s, _) = post_auth(&router, "/api/menus", payload2, &token).await;
    assert_eq!(s, StatusCode::CREATED);

    let (status, body) = get(
        &router,
        &format!("/api/restaurants/{restaurant_id}/menus"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"].as_array().unwrap().len(), 2);
}

#[sqlx::test]
async fn test_list_active_menus_for_restaurant(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    // Lunch menu is created with is_active: true (via full_menu_json).
    create_menu(&router, restaurant_id, &token).await;

    // Dinner menu is created with is_active: false.
    let payload2 = json!({
        "restaurant_id": restaurant_id,
        "name": "Dinner Menu",
        "description": null,
        "is_active": false,
        "permanent": false,
        "sections": []
    });
    let (s, _) = post_auth(&router, "/api/menus", payload2, &token).await;
    assert_eq!(s, StatusCode::CREATED);

    let (status, body) = get(
        &router,
        &format!("/api/restaurants/{restaurant_id}/menus/active"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let menus = body["data"].as_array().unwrap();
    assert_eq!(menus.len(), 1);
    assert_eq!(menus[0]["name"], "Lunch Menu");
}

// ── Delete ────────────────────────────────────────────────────────

#[sqlx::test]
async fn test_delete_menu(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();

    let (status, body) = del_auth(&router, &format!("/api/menus/{menu_id}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);

    // Confirm it's gone.
    let (status, _) = get(&router, &format!("/api/menus/{menu_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_delete_menu_not_found(pool: PgPool) {
    let (_user_id, _restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let fake_id = Uuid::new_v4();

    let (status, body) = del_auth(&router, &format!("/api/menus/{fake_id}"), &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

// ── Reset ─────────────────────────────────────────────────────────

#[sqlx::test]
async fn test_reset_menu(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();

    // All items should be available after creation.
    let section_items = created["sections"][0]["items"].as_array().unwrap();
    assert_eq!(section_items[0]["is_available"], true);

    // Reset — sets all items to is_available = false.
    let (status, body) = post_auth(
        &router,
        "/api/reset-menu",
        json!({ "id": menu_id }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["menu_id"], menu_id);
    assert!(body["data"]["items_reset"].as_u64().unwrap() >= 1);

    // Verify items are now unavailable.
    let (_, get_body) = get(&router, &format!("/api/menus/{menu_id}")).await;
    let items = &get_body["data"]["sections"][0]["items"];
    for item in items.as_array().unwrap() {
        assert_eq!(item["is_available"], false);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Update-menu action tests
// ═══════════════════════════════════════════════════════════════════

// ── UpdateMenu action (rename / toggle active) ───────────────────

#[sqlx::test]
async fn test_update_menu_metadata(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();

    let (status, body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                {
                    "UpdateMenu": {
                        "id": menu_id,
                        "name": "Brunch Menu",
                        "is_active": false,
                    }
                }
            ]
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["name"], "Brunch Menu");
    assert_eq!(body["data"]["is_active"], false);
}

// ── UpdateMenuSection action ─────────────────────────────────────

#[sqlx::test]
async fn test_update_menu_section(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();
    let section_id = created["sections"][0]["id"].as_str().unwrap();

    let (status, body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                {
                    "UpdateMenuSection": {
                        "section_id": section_id,
                        "update": {
                            "name": "Renamed Section",
                            "is_active": false,
                        }
                    }
                }
            ]
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let section = &body["data"]["sections"][0];
    assert_eq!(section["name"], "Renamed Section");
    assert_eq!(section["is_active"], false);
}

// ── UpdateMenuSectionItem action ─────────────────────────────────

#[sqlx::test]
async fn test_update_menu_section_item(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();
    let item_id = created["sections"][0]["items"][0]["id"].as_str().unwrap();

    let (status, body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                {
                    "UpdateMenuSectionItem": {
                        "item_id": item_id,
                        "update": {
                            "is_available": false,
                            "price_override_cents": 2000,
                        }
                    }
                }
            ]
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let item = &body["data"]["sections"][0]["items"][0];
    assert_eq!(item["is_available"], false);
    assert_eq!(item["price_override_cents"], 2000);
}

// ── UpdateMenuSectionItem with catalog item + tag update ─────────

#[sqlx::test]
async fn test_update_menu_section_item_with_catalog_and_tags(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();
    let msi_id = created["sections"][0]["items"][0]["id"].as_str().unwrap();
    let catalog_item_id = created["sections"][0]["items"][0]["item"]["id"]
        .as_str()
        .unwrap();

    let (status, body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                {
                    "UpdateMenuSectionItem": {
                        "item_id": msi_id,
                        "update": {
                            "item": {
                                "id": catalog_item_id,
                                "name": "Special Nasi Goreng",
                                "base_price_cents": 1800,
                                "tags": [
                                    { "name": "spicy" },
                                    { "name": "rice" }
                                ]
                            }
                        }
                    }
                }
            ]
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let item = &body["data"]["sections"][0]["items"][0]["item"];
    assert_eq!(item["name"], "Special Nasi Goreng");
    assert_eq!(item["base_price_cents"], 1800);

    // Verify tags were set by fetching through the item API.
    let (s2, b2) = get(&router, &format!("/api/items/{catalog_item_id}")).await;
    assert_eq!(s2, StatusCode::OK, "{b2}");
    let tags = b2["data"]["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 2);
    let tag_names: Vec<&str> = tags.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(tag_names.contains(&"spicy"));
    assert!(tag_names.contains(&"rice"));
}

// ── AddSection action ────────────────────────────────────────────

#[sqlx::test]
async fn test_update_add_section(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();

    let (status, body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                {
                    "AddSection": {
                        "parent_id": { "Existing": menu_id },
                        "section": {
                            "menu_id": menu_id,
                            "parent_id": null,
                            "name": "Drinks",
                            "description": "Beverages",
                            "position": 1,
                            "is_active": true,
                        }
                    }
                }
            ]
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let sections = body["data"]["sections"].as_array().unwrap();
    assert_eq!(sections.len(), 2);
    let names: Vec<&str> = sections.iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"Drinks"));
}

// ── AddSection as subsection ─────────────────────────────────────

#[sqlx::test]
async fn test_update_add_subsection(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();
    let parent_section_id = created["sections"][0]["id"].as_str().unwrap();

    let (status, body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                {
                    "AddSection": {
                        "parent_id": { "Existing": parent_section_id },
                        "section": {
                            "menu_id": menu_id,
                            "parent_id": null,
                            "name": "Rice Bowls",
                            "description": null,
                            "position": 0,
                            "is_active": true,
                        }
                    }
                }
            ]
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let subs = body["data"]["sections"][0]["subsections"]
        .as_array()
        .unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0]["name"], "Rice Bowls");
}

// ── AddItem action ───────────────────────────────────────────────

#[sqlx::test]
async fn test_update_add_item(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();
    let section_id = created["sections"][0]["id"].as_str().unwrap();

    let (status, body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                {
                    "AddItem": {
                        "section_id": { "Existing": section_id },
                        "item": {
                            "section_id": section_id,
                            "position": 1,
                            "price_override_cents": 1200,
                            "is_available": true,
                            "item": {
                                "restaurant_id": restaurant_id,
                                "name": "Mie Goreng",
                                "description": "Fried noodles",
                                "base_price_cents": 1300,
                                "image_url": null,
                                "active": true,
                                "tags": []
                            }
                        }
                    }
                }
            ]
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let items = body["data"]["sections"][0]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    let names: Vec<&str> = items
        .iter()
        .map(|i| i["item"]["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"Mie Goreng"));
}

// ── AddSection + AddItem with CreatedBy EntityRef ────────────────

#[sqlx::test]
async fn test_update_add_section_then_add_item_via_created_by(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();

    let (status, body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                // Action 0: create a new section — its ID will be available via CreatedBy(0)
                {
                    "AddSection": {
                        "parent_id": { "Existing": menu_id },
                        "section": {
                            "menu_id": menu_id,
                            "parent_id": null,
                            "name": "Desserts",
                            "description": null,
                            "position": 2,
                            "is_active": true,
                        }
                    }
                },
                // Action 1: add an item into the section created by action 0
                {
                    "AddItem": {
                        "section_id": { "CreatedByAction": 0 },
                        "item": {
                            "section_id": Uuid::nil(),
                            "position": 0,
                            "price_override_cents": null,
                            "is_available": true,
                            "item": {
                                "restaurant_id": restaurant_id,
                                "name": "Es Cendol",
                                "description": "Iced dessert",
                                "base_price_cents": 800,
                                "image_url": null,
                                "active": true,
                                "tags": []
                            }
                        }
                    }
                }
            ]
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let sections = body["data"]["sections"].as_array().unwrap();
    // Should now have 2 top-level sections (original + Desserts).
    assert_eq!(sections.len(), 2);

    let desserts = sections
        .iter()
        .find(|s| s["name"] == "Desserts")
        .expect("Desserts section not found");
    let dessert_items = desserts["items"].as_array().unwrap();
    assert_eq!(dessert_items.len(), 1);
    assert_eq!(dessert_items[0]["item"]["name"], "Es Cendol");
}

// ── ChangePositionSection action ─────────────────────────────────

#[sqlx::test]
async fn test_update_change_position_section(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();
    let section_id = created["sections"][0]["id"].as_str().unwrap();

    let (status, body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                {
                    "ChangePositionSection": {
                        "section_id": { "Existing": section_id },
                        "position": 5
                    }
                }
            ]
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["sections"][0]["position"], 5);
}

// ── ChangePositionItem action ────────────────────────────────────

#[sqlx::test]
async fn test_update_change_position_item(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();
    let item_id = created["sections"][0]["items"][0]["id"].as_str().unwrap();

    let (status, body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                {
                    "ChangePositionItem": {
                        "item_id": { "Existing": item_id },
                        "position": 10
                    }
                }
            ]
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["sections"][0]["items"][0]["position"], 10);
}

// ── ChangeSectionForItem action ──────────────────────────────────

#[sqlx::test]
async fn test_update_move_item_to_another_section(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();
    let item_id = created["sections"][0]["items"][0]["id"].as_str().unwrap();

    // First, add a second section.
    let (_, add_body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                {
                    "AddSection": {
                        "parent_id": { "Existing": menu_id },
                        "section": {
                            "menu_id": menu_id,
                            "parent_id": null,
                            "name": "Sides",
                            "description": null,
                            "position": 1,
                            "is_active": true,
                        }
                    }
                }
            ]
        }),
        &token,
    )
    .await;

    let new_section_id = add_body["data"]["sections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "Sides")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Now move the item to the new section.
    let (status, body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                {
                    "ChangeSectionForItem": {
                        "item_id": { "Existing": item_id },
                        "section_id": { "Existing": new_section_id }
                    }
                }
            ]
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let sections = body["data"]["sections"].as_array().unwrap();
    let old_section = sections.iter().find(|s| s["name"] == "Main Dishes").unwrap();
    let new_section = sections.iter().find(|s| s["name"] == "Sides").unwrap();

    assert!(old_section["items"].as_array().unwrap().is_empty());
    assert_eq!(new_section["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        new_section["items"][0]["item"]["name"],
        "Nasi Goreng"
    );
}

// ── ChangeSectionForSubSection action ────────────────────────────

#[sqlx::test]
async fn test_update_move_subsection(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);

    // Create menu with two top-level sections, one having a subsection.
    let payload = json!({
        "restaurant_id": restaurant_id,
        "name": "Move Test",
        "description": null,
        "is_active": true,
        "permanent": false,
        "sections": [
            {
                "menu_id": restaurant_id,
                "parent_id": null,
                "name": "Section A",
                "description": null,
                "position": 0,
                "is_active": true,
                "items": [],
                "subsections": [
                    {
                        "menu_id": restaurant_id,
                        "parent_id": null,
                        "name": "Sub A1",
                        "description": null,
                        "position": 0,
                        "is_active": true,
                        "items": [],
                        "subsections": []
                    }
                ]
            },
            {
                "menu_id": restaurant_id,
                "parent_id": null,
                "name": "Section B",
                "description": null,
                "position": 1,
                "is_active": true,
                "items": [],
                "subsections": []
            }
        ]
    });

    let (s, created) = post_auth(&router, "/api/menus", payload, &token).await;
    assert_eq!(s, StatusCode::CREATED, "{created}");

    let sections = created["data"]["sections"].as_array().unwrap();
    let menu_id = created["data"]["id"].as_str().unwrap();

    let section_a = sections.iter().find(|s| s["name"] == "Section A").unwrap();
    let section_b = sections.iter().find(|s| s["name"] == "Section B").unwrap();
    let sub_a1_id = section_a["subsections"][0]["id"].as_str().unwrap();
    let section_b_id = section_b["id"].as_str().unwrap();

    // Move Sub A1 under Section B.
    let (status, body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                {
                    "ChangeSectionForSubSection": {
                        "subsection_id": { "Existing": sub_a1_id },
                        "section_id": { "Existing": section_b_id }
                    }
                }
            ]
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let updated_sections = body["data"]["sections"].as_array().unwrap();
    let a = updated_sections
        .iter()
        .find(|s| s["name"] == "Section A")
        .unwrap();
    let b = updated_sections
        .iter()
        .find(|s| s["name"] == "Section B")
        .unwrap();

    assert!(a["subsections"].as_array().unwrap().is_empty());
    assert_eq!(b["subsections"].as_array().unwrap().len(), 1);
    assert_eq!(b["subsections"][0]["name"], "Sub A1");
}

// ── Move subsection to top-level (parent = menu_id) ──────────────

#[sqlx::test]
async fn test_update_move_subsection_to_top_level(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();
    let section_id = created["sections"][0]["id"].as_str().unwrap();

    // Add a subsection first.
    let (_, add_body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                {
                    "AddSection": {
                        "parent_id": { "Existing": section_id },
                        "section": {
                            "menu_id": menu_id,
                            "parent_id": null,
                            "name": "Nested",
                            "description": null,
                            "position": 0,
                            "is_active": true,
                        }
                    }
                }
            ]
        }),
        &token,
    )
    .await;

    let nested_id = add_body["data"]["sections"][0]["subsections"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Move it to top-level by referencing menu_id as the new parent.
    let (status, body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                {
                    "ChangeSectionForSubSection": {
                        "subsection_id": { "Existing": nested_id },
                        "section_id": { "Existing": menu_id }
                    }
                }
            ]
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let sections = body["data"]["sections"].as_array().unwrap();
    // Should now have 2 top-level sections.
    assert_eq!(sections.len(), 2);
    assert!(sections.iter().any(|s| s["name"] == "Nested"));
}

// ═══════════════════════════════════════════════════════════════════
// Validation / error tests
// ═══════════════════════════════════════════════════════════════════

#[sqlx::test]
async fn test_update_menu_not_found(pool: PgPool) {
    let (user_id, _, token) = seed(&pool).await;
    let router = app(pool);
    let fake = Uuid::new_v4();

    let (status, body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": fake,
            "actions": []
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[sqlx::test]
async fn test_update_nesting_depth_exceeded(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    // Default max_menu_nesting_depth is 2.
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();
    let section_id = created["sections"][0]["id"].as_str().unwrap();

    // Add sub-section under the existing section (depth 2 — allowed).
    let (_, mid) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                {
                    "AddSection": {
                        "parent_id": { "Existing": section_id },
                        "section": {
                            "menu_id": menu_id,
                            "parent_id": null,
                            "name": "Depth 2",
                            "description": null,
                            "position": 0,
                            "is_active": true,
                        }
                    }
                }
            ]
        }),
        &token,
    )
    .await;

    let depth2_id = mid["data"]["sections"][0]["subsections"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Try adding sub-sub-section under depth-2 section (depth 3 — should fail).
    let (status, body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                {
                    "AddSection": {
                        "parent_id": { "Existing": depth2_id },
                        "section": {
                            "menu_id": menu_id,
                            "parent_id": null,
                            "name": "Too Deep",
                            "description": null,
                            "position": 0,
                            "is_active": true,
                        }
                    }
                }
            ]
        }),
        &token,
    )
    .await;
    assert_ne!(status, StatusCode::OK, "{body}");
    assert_eq!(body["success"], false);
}

// ── Multiple actions in one batch ────────────────────────────────

#[sqlx::test]
async fn test_update_multiple_actions_in_batch(pool: PgPool) {
    let (_user_id, restaurant_id, token) = seed(&pool).await;
    let router = app(pool);
    let created = create_menu(&router, restaurant_id, &token).await;
    let menu_id = created["id"].as_str().unwrap();

    // Batch: rename menu + add a section + add an item into that new section.
    let (status, body) = post_auth(
        &router,
        "/api/update-menu",
        json!({
            "menu_id": menu_id,
            "actions": [
                // 0: rename menu
                {
                    "UpdateMenu": {
                        "id": menu_id,
                        "name": "Updated Menu",
                    }
                },
                // 1: add section (produced id referenced below)
                {
                    "AddSection": {
                        "parent_id": { "Existing": menu_id },
                        "section": {
                            "menu_id": menu_id,
                            "parent_id": null,
                            "name": "Appetizers",
                            "description": null,
                            "position": 5,
                            "is_active": true,
                        }
                    }
                },
                // 2: add item into section from action 1
                {
                    "AddItem": {
                        "section_id": { "CreatedByAction": 1 },
                        "item": {
                            "section_id": Uuid::nil(),
                            "position": 0,
                            "price_override_cents": null,
                            "is_available": true,
                            "item": {
                                "restaurant_id": restaurant_id,
                                "name": "Spring Rolls",
                                "description": null,
                                "base_price_cents": 600,
                                "image_url": null,
                                "active": true,
                                "tags": []
                            }
                        }
                    }
                }
            ]
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["name"], "Updated Menu");

    let sections = body["data"]["sections"].as_array().unwrap();
    let appetizers = sections
        .iter()
        .find(|s| s["name"] == "Appetizers")
        .expect("Appetizers section not found");
    let items = appetizers["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["item"]["name"], "Spring Rolls");
}
