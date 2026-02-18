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

    // Before setup is completed: allow GET and POST /setup, block everything else
    if !setup_completed {
        if path == "/setup" {
            return Ok(next.run(req).await);
        }
        return Ok(Redirect::to("/setup").into_response());
    }

    // After setup is completed: allow GET /setup (status check), block POST /setup
    if path == "/setup" {
        if req.method() == Method::GET {
            return Ok(next.run(req).await);
        }
        return Ok(Redirect::to("/").into_response());
    }

    Ok(next.run(req).await)
}