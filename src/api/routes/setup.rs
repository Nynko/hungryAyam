use axum::{Json, extract::State,http::StatusCode};
use serde::Serialize;
use crate::{
    state::AppState,
    domain::app_setup::AppSetup,
    api::dtos::app_setup::AppSetupRequest,
    errors::api_errors::ApiError
};


#[derive(Serialize)]
pub struct SetupStatus {
    completed: bool,
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
            Ok((StatusCode::CREATED, Json(setup)))
        }
        Err(e) => {
            Err(ApiError::from(e))
        }
    }
}
