use anyhow::Context;
use sqlx::PgPool;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use dotenv::dotenv;
use std::sync::{Arc, atomic::AtomicBool};


mod features;

mod app;
mod state;
mod errors;
mod auth;
mod setup_middleware;
mod types;
mod traits;
mod utils;

use crate::{
    state::build_state,
    app::build_app
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
    // App state
    // --------------------------------------------------
    let state = build_state(setup_completed, db);

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
