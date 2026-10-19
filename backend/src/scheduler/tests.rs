//! SMS fallback tests: real DB + a local mock of the OVH API.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use super::run_sms_fallback;
use crate::features::{order::domain::order_session::OrderSessionStatus, sms::OvhSmsClient};

#[derive(Clone, Default)]
struct MockOvh {
    /// (url, headers, body) of each SMS job request
    requests: Arc<Mutex<Vec<(String, HeaderMap, String)>>>,
    fail: Arc<Mutex<bool>>,
}

async fn auth_time() -> Json<i64> {
    Json(chrono::Utc::now().timestamp())
}

async fn sms_job(
    State(mock): State<MockOvh>,
    Path(service): Path<String>,
    headers: HeaderMap,
    body: String,
) -> (StatusCode, Json<Value>) {
    mock.requests
        .lock()
        .unwrap()
        .push((format!("/sms/{service}/jobs"), headers, body.clone()));
    if *mock.fail.lock().unwrap() {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"message": "boom"})));
    }
    let receivers: Value = serde_json::from_str::<Value>(&body).unwrap()["receivers"].clone();
    (
        StatusCode::OK,
        Json(json!({"ids": [42], "validReceivers": receivers, "invalidReceivers": [], "totalCreditsRemoved": 1})),
    )
}

/// Start the mock OVH server and return (client pointing at it, mock, base url).
async fn mock_ovh() -> (OvhSmsClient, MockOvh, String) {
    let mock = MockOvh::default();
    let app = Router::new()
        .route("/1.0/auth/time", get(auth_time))
        .route("/1.0/sms/:service/jobs", post(sms_job))
        .with_state(mock.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/1.0", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = OvhSmsClient::new(
        Some(base.clone()),
        "AK".into(),
        "AS".into(),
        "CK".into(),
        "sms-test-1".into(),
        None,
    );
    (client, mock, base)
}

/// Seed a user, restaurant and session in `status`. Returns (user_id, session_id).
async fn seed_session(pool: &PgPool, status: OrderSessionStatus) -> (Uuid, Uuid) {
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (name, auth_method, role) VALUES ('tester', 'Password', 'Editor') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let restaurant_id: Uuid = sqlx::query_scalar(
        "INSERT INTO restaurants (name, sms_phone_number, created_by, updated_by) \
         VALUES ('Ayam', '06 12 34 56 78', $1, $1) RETURNING id",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let session_id: Uuid = sqlx::query_scalar(
        "INSERT INTO order_sessions (restaurant_id, status, created_by, updated_by) \
         VALUES ($1, $2, $3, $3) RETURNING id",
    )
    .bind(restaurant_id)
    .bind(status.as_i16())
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (user_id, session_id)
}

/// Insert a notification event sent `sent_ago` seconds ago, optionally
/// received by the Shortcut `received_ago` seconds ago.
async fn seed_event(pool: &PgPool, session_id: Uuid, sent_ago: i64, received_ago: Option<i64>) -> String {
    let sig = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO notification_events \
            (sig, session_id, ts, phone, time_str, orders, restaurant, message, email_sent_at, email_received_at) \
         VALUES ($1, $2, 0, '06 12 34 56 78', '12:15', '2 Ayam Goreng', 'Ayam', 'Commande pour 12:15', \
                 NOW() - make_interval(secs => $3), NOW() - make_interval(secs => $4))",
    )
    .bind(&sig)
    .bind(session_id)
    .bind(sent_ago as f64)
    .bind(received_ago.map(|s| s as f64))
    .execute(pool)
    .await
    .unwrap();
    sig
}

async fn session_status(pool: &PgPool, id: Uuid) -> i16 {
    sqlx::query_scalar("SELECT status FROM order_sessions WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn event(pool: &PgPool, sig: &str) -> (bool, i32, Option<String>) {
    sqlx::query_as(
        "SELECT fallback_sent_at IS NOT NULL, fallback_attempts, fallback_error FROM notification_events WHERE sig = $1",
    )
    .bind(sig)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test]
async fn fallback_sends_when_received_but_not_confirmed(pool: PgPool) {
    let (client, mock, base) = mock_ovh().await;
    let (user_id, session_id) = seed_session(&pool, OrderSessionStatus::Closed).await;
    let sig = seed_event(&pool, session_id, 7 * 60, Some(6 * 60)).await;

    run_sms_fallback(&pool, user_id, Some(&client)).await.unwrap();

    let requests = mock.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 1);
    let (path, headers, body) = &requests[0];
    assert_eq!(path, "/sms/sms-test-1/jobs");

    let body_json: Value = serde_json::from_str(body).unwrap();
    assert_eq!(body_json["receivers"], json!(["+33612345678"]));
    assert_eq!(body_json["message"], "Commande pour 12:15");
    assert_eq!(body_json["senderForResponse"], true);
    assert_eq!(body_json["noStopClause"], true);

    // Signature must match OVH's algorithm over the exact URL + body + timestamp
    assert_eq!(headers["x-ovh-application"], "AK");
    assert_eq!(headers["x-ovh-consumer"], "CK");
    let ts: i64 = headers["x-ovh-timestamp"].to_str().unwrap().parse().unwrap();
    let expected = crate::features::sms::sign("AS", "CK", "POST", &format!("{base}/sms/sms-test-1/jobs"), body, ts);
    assert_eq!(headers["x-ovh-signature"], expected.as_str());

    assert_eq!(event(&pool, &sig).await, (true, 1, None));
    assert_eq!(session_status(&pool, session_id).await, OrderSessionStatus::SmsSent.as_i16());

    // Second run is a no-op
    run_sms_fallback(&pool, user_id, Some(&client)).await.unwrap();
    assert_eq!(mock.requests.lock().unwrap().len(), 1);
}

#[sqlx::test]
async fn fallback_sends_when_email_never_received(pool: PgPool) {
    let (client, mock, _) = mock_ovh().await;
    let (user_id, session_id) = seed_session(&pool, OrderSessionStatus::Requested).await;
    let sig = seed_event(&pool, session_id, 7 * 60, None).await;

    run_sms_fallback(&pool, user_id, Some(&client)).await.unwrap();

    assert_eq!(mock.requests.lock().unwrap().len(), 1);
    assert_eq!(event(&pool, &sig).await, (true, 1, None));
    assert_eq!(session_status(&pool, session_id).await, OrderSessionStatus::SmsSent.as_i16());
}

#[sqlx::test]
async fn fallback_waits_before_deadline(pool: PgPool) {
    let (client, mock, _) = mock_ovh().await;
    let (user_id, session_id) = seed_session(&pool, OrderSessionStatus::Closed).await;
    // Received 2 min ago (deadline 5 min) and an unreceived one sent 5 min ago (deadline 6 min)
    seed_event(&pool, session_id, 3 * 60, Some(2 * 60)).await;
    seed_event(&pool, session_id, 5 * 60, None).await;

    run_sms_fallback(&pool, user_id, Some(&client)).await.unwrap();

    assert!(mock.requests.lock().unwrap().is_empty());
    assert_eq!(session_status(&pool, session_id).await, OrderSessionStatus::Closed.as_i16());

    // The scheduler knows when to wake up for it
    let due = super::next_fallback_due(&pool).await.unwrap().expect("pending fallback");
    let in_secs = (due - chrono::Utc::now()).num_seconds();
    assert!((50..=70).contains(&in_secs), "due in {in_secs}s");
}

#[sqlx::test]
async fn fallback_skips_sessions_already_handled(pool: PgPool) {
    let (client, mock, _) = mock_ovh().await;
    let (user_id, session_id) = seed_session(&pool, OrderSessionStatus::Confirmed).await;
    seed_event(&pool, session_id, 10 * 60, Some(9 * 60)).await;

    run_sms_fallback(&pool, user_id, Some(&client)).await.unwrap();

    assert!(mock.requests.lock().unwrap().is_empty());
}

#[sqlx::test]
async fn fallback_skips_stale_events(pool: PgPool) {
    let (client, mock, _) = mock_ovh().await;
    let (user_id, session_id) = seed_session(&pool, OrderSessionStatus::Closed).await;
    seed_event(&pool, session_id, 2 * 60 * 60, None).await;

    run_sms_fallback(&pool, user_id, Some(&client)).await.unwrap();

    assert!(mock.requests.lock().unwrap().is_empty());
}

#[sqlx::test]
async fn fallback_records_error_and_retries_later(pool: PgPool) {
    let (client, mock, _) = mock_ovh().await;
    *mock.fail.lock().unwrap() = true;
    let (user_id, session_id) = seed_session(&pool, OrderSessionStatus::Closed).await;
    let sig = seed_event(&pool, session_id, 7 * 60, None).await;

    run_sms_fallback(&pool, user_id, Some(&client)).await.unwrap();
    let (sent, attempts, error) = event(&pool, &sig).await;
    assert!(!sent);
    assert_eq!(attempts, 1);
    assert!(error.unwrap().contains("500"));
    assert_eq!(session_status(&pool, session_id).await, OrderSessionStatus::Closed.as_i16());

    // Immediate re-run: within the retry delay, nothing is sent
    run_sms_fallback(&pool, user_id, Some(&client)).await.unwrap();
    assert_eq!(mock.requests.lock().unwrap().len(), 1);

    // After the retry delay, OVH is back up → sent
    *mock.fail.lock().unwrap() = false;
    sqlx::query("UPDATE notification_events SET fallback_attempted_at = NOW() - INTERVAL '2 minutes'")
        .execute(&pool)
        .await
        .unwrap();
    run_sms_fallback(&pool, user_id, Some(&client)).await.unwrap();
    assert_eq!(event(&pool, &sig).await, (true, 2, None));
    assert_eq!(session_status(&pool, session_id).await, OrderSessionStatus::SmsSent.as_i16());
}

#[sqlx::test]
async fn fallback_gives_up_on_invalid_phone(pool: PgPool) {
    let (client, mock, _) = mock_ovh().await;
    let (user_id, session_id) = seed_session(&pool, OrderSessionStatus::Closed).await;
    let sig = seed_event(&pool, session_id, 7 * 60, None).await;
    sqlx::query("UPDATE notification_events SET phone = '—'").execute(&pool).await.unwrap();

    run_sms_fallback(&pool, user_id, Some(&client)).await.unwrap();

    assert!(mock.requests.lock().unwrap().is_empty());
    let (sent, attempts, error) = event(&pool, &sig).await;
    assert!(!sent);
    assert_eq!(attempts, super::FALLBACK_MAX_ATTEMPTS);
    assert!(error.unwrap().contains("no valid SMS phone"));
}

#[sqlx::test]
async fn fallback_without_ovh_config_leaves_session_untouched(pool: PgPool) {
    let (user_id, session_id) = seed_session(&pool, OrderSessionStatus::Closed).await;
    let sig = seed_event(&pool, session_id, 7 * 60, None).await;

    run_sms_fallback(&pool, user_id, None).await.unwrap();

    assert_eq!(event(&pool, &sig).await, (false, 0, None));
    assert_eq!(session_status(&pool, session_id).await, OrderSessionStatus::Closed.as_i16());
}

#[sqlx::test]
async fn shortcut_is_told_not_to_send_once_fallback_claimed(pool: PgPool) {
    use tower::ServiceExt;

    let (_, session_id) = seed_session(&pool, OrderSessionStatus::Closed).await;
    let ts = chrono::Utc::now().timestamp();
    let sig = crate::features::notification::sign("secret", ts, "+33612345678", "12:15");
    sqlx::query(
        "INSERT INTO notification_events (sig, session_id, ts, phone, time_str, orders, restaurant, fallback_attempts) \
         VALUES ($1, $2, $3, '+33612345678', '12:15', '', 'Ayam', $4)",
    )
    .bind(&sig)
    .bind(session_id)
    .bind(ts)
    .bind(0)
    .execute(&pool)
    .await
    .unwrap();

    let state = crate::state::build_state(
        Arc::new(std::sync::atomic::AtomicBool::new(true)),
        pool.clone(),
        Arc::new(tokio::sync::Notify::new()),
        std::path::PathBuf::from("/tmp/test_uploads"),
        None,
        "http://localhost:5173".to_string(),
        Some("secret".to_string()),
        None,
    );
    let router = crate::app::build_app(state);
    let verify = |router: Router| {
        let uri = format!("/api/verify-notification?sig={sig}&ts={ts}&phone=%2B33612345678&time=12:15");
        async move {
            let res = router
                .oneshot(axum::http::Request::get(uri).body(axum::body::Body::empty()).unwrap())
                .await
                .unwrap();
            let bytes = http_body_util::BodyExt::collect(res.into_body()).await.unwrap().to_bytes();
            serde_json::from_slice::<Value>(&bytes).unwrap()["data"]["valid"].clone()
        }
    };

    assert_eq!(verify(router.clone()).await, json!(true));

    sqlx::query("UPDATE notification_events SET fallback_attempts = 1").execute(&pool).await.unwrap();
    assert_eq!(verify(router).await, json!(false));
}
