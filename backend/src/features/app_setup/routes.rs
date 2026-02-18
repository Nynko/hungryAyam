use std::sync::atomic::Ordering;
use axum::{
    extract::State,
    http::StatusCode,
    Router,
    routing::get
};
use serde::Serialize;

use crate::{
    features::app_setup::{
        domain::AppSetup,
        dto::AppSetupRequest,
    },
    state::AppState,
    errors::{
        api_errors::ApiError,
        json_extractor::ApiJson,
    },
    types::response::ApiResponse
};


#[derive(Serialize)]
pub struct SetupStatus {
    completed: bool,
}

pub fn setup_routes() -> Router<AppState>{
    Router::new()
        .route("/setup", get(get_setup_status).post(setup_app))
}


pub async fn get_setup_status(State(state): State<AppState>) -> ApiJson<ApiResponse<SetupStatus>> {
    let completed = state.setup_completed.load(std::sync::atomic::Ordering::SeqCst);
    ApiJson(ApiResponse::success(SetupStatus { completed }))
}

pub async fn setup_app(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<AppSetupRequest>
) -> Result<(StatusCode, ApiJson<ApiResponse<AppSetup>>), ApiError> {
    let setup_result = state.setup_service.setup_app(request).await;

    match setup_result {
        Ok(setup) => {
            // Setup succeeded, return 201 Created
            state.setup_completed.store(true, Ordering::SeqCst);
            Ok((StatusCode::CREATED, ApiJson(ApiResponse::success(setup))))
        }
        Err(e) => {
            Err(ApiError::from(e))
        }
    }
}