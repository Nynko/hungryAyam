use axum::{extract::State, response::IntoResponse};

async fn root_handler(State(state): State<AppState>) -> impl IntoResponse {
    if state.setup_complete.load(Ordering::SeqCst) {
        show_app().await
    } else {
        show_setup().await
    }
}
