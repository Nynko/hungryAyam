use axum::{
    http::{Request, StatusCode, Method},
    body::Body,
    middleware::Next,
    response::{Response, Redirect, IntoResponse},
    extract::State,

};
use std::sync::atomic::Ordering;
use crate::state::AppState;

pub async fn setup_redirect_guard(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    let setup_completed = state.setup_completed.load(Ordering::SeqCst);

     // Allow GET /setup always
    if path == "/setup" && req.method() ==  Method::GET {
        return Ok(next.run(req).await);
    }

    if !setup_completed && path != "/setup" {
        // Redirect all requests to /setup if setup is not completed
        return Ok(Redirect::to("/setup").into_response());
    }
    if setup_completed && path == "/setup" {
        // Redirect /setup to / if setup is completed
        return Ok(Redirect::to("/").into_response());
    }
    Ok(next.run(req).await)
}
