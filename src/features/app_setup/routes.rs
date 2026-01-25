use std::sync::atomic::Ordering;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    Router,
    routing::get
};
use serde::Serialize;

use crate::{
    features::app_setup::{
        domain::AppSetup,
        dto::AppSetupRequest
    },
    state::AppState,
    errors::api_errors::ApiError
};


#[derive(Serialize)]
pub struct SetupStatus {
    completed: bool,
}

pub fn setup_routes() -> Router<AppState>{
    Router::new()
        .route("/setup", get(get_setup_status).post(setup_app))
}


pub async fn get_setup_status(State(state): State<AppState>) -> (StatusCode, Json<SetupStatus>) {
    let completed = state.setup_completed.load(std::sync::atomic::Ordering::SeqCst);
    (StatusCode::OK ,Json(SetupStatus { completed }))
}

pub async fn setup_app(
    State(state): State<AppState>,
    Json(request): Json<AppSetupRequest>
) -> Result<(StatusCode, Json<AppSetup>), ApiError> {
    let setup_result = state.setup_service.setup_app(request).await;

    match setup_result {
        Ok(setup) => {
            // Setup succeeded, return 201 Created (or 200 OK if you prefer)
            state.setup_completed.store(true, Ordering::SeqCst);
            Ok((StatusCode::CREATED, Json(setup)))
        }
        Err(e) => {
            Err(ApiError::from(e))
        }
    }
}
