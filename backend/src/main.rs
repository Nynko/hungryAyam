use anyhow::Context;
use sqlx::PgPool;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use dotenv::dotenv;
use std::sync::{Arc, atomic::AtomicBool};
use tokio::sync::Notify;


mod features;

mod app;
mod state;
mod errors;
mod auth;
mod setup_middleware;
mod types;
mod traits;
mod utils;
mod scheduler;

use crate::{
    state::build_state,
    app::build_app,
    features::email::EmailService,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // --------------------------------------------------
    // Logging
    // --------------------------------------------------
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting tracing default failed");

    // --------------------------------------------------
    // Config
    // --------------------------------------------------
    dotenv().ok();
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL is not set")?;
    let bind_addr =
        std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
    let upload_dir = std::path::PathBuf::from(
        std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "/data/uploads".to_string()),
    );
    let base_url = std::env::var("APP_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:5173".to_string());
    let email_service = match (
        std::env::var("SMTP_HOST"),
        std::env::var("SMTP_USER"),
        std::env::var("SMTP_PASSWORD"),
        std::env::var("SMTP_FROM"),
    ) {
        (Ok(host), Ok(user), Ok(password), Ok(from)) => {
            let port = std::env::var("SMTP_PORT")
                .ok()
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(587);
            match EmailService::new(&host, port, &user, &password, &from) {
                Ok(svc) => {
                    info!("email service configured (host={})", host);
                    Some(svc)
                }
                Err(e) => {
                    tracing::warn!("failed to build email service: {e} — email sending disabled");
                    None
                }
            }
        }
        _ => {
            tracing::warn!("SMTP_HOST/USER/PASSWORD/FROM not set — email sending disabled");
            None
        }
    };

    // --------------------------------------------------
    // Database
    // --------------------------------------------------
    info!("connecting to database");
    let db = PgPool::connect(&database_url)
        .await
        .context("failed to connect to database")?;

    // --------------------------------------------------
    // Migrations
    // --------------------------------------------------
    info!("running database migrations");
    sqlx::migrate!()
        .run(&db)
        .await
        .context("database migrations failed")?;


    // --------------------------------------------------
    // Check if the setup has been done
    // --------------------------------------------------
    info!("Checking if setup has been done");
    let settings = sqlx::query!("SELECT id FROM app_settings WHERE id = 1")
        .fetch_optional(&db)
        .await.context("failed to check if the app exist")?;
    let setup_completed_bool = settings.is_some();
    info!("Setup: {}", setup_completed_bool);
    let setup_completed = Arc::new(AtomicBool::new(setup_completed_bool));

    // --------------------------------------------------
    // Scheduler notify handle (shared between app state and scheduler)
    // --------------------------------------------------
    let scheduler_notify = Arc::new(Notify::new());

    // --------------------------------------------------
    // Upload directory
    // --------------------------------------------------
    tokio::fs::create_dir_all(&upload_dir)
        .await
        .context("failed to create upload directory")?;

    // --------------------------------------------------
    // App state
    // --------------------------------------------------
    let scheduler_email = email_service.clone();
    let state = build_state(setup_completed, db.clone(), scheduler_notify.clone(), upload_dir, email_service, base_url);

    // --------------------------------------------------
    // Background scheduler (menu auto-reset, session auto-close)
    // --------------------------------------------------
    scheduler::spawn_scheduler(db, scheduler_notify, scheduler_email);

    // --------------------------------------------------
    // Router
    // --------------------------------------------------
    let app = build_app(state);

    // --------------------------------------------------
    // Server
    // --------------------------------------------------
    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(bind_addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
