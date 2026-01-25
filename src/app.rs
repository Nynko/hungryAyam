use axum::{
    Router, middleware, routing::{get, post}
};
use tower_http::cors::CorsLayer;

use crate::{
    api::routes::{
        restaurants::create_restaurant,
        setup::{setup_app,get_setup_status}
    },
    api::setup_middleware::setup_redirect_guard,
    state::AppState
};

pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/setup", get(get_setup_status).post(setup_app))
        .route("/api/restaurants", post(create_restaurant))
        // .route("/api/restaurants", get(list_restaurants))
        // .route("/api/restaurants/:id", get(get_restaurant))
        // .route("/api/restaurants/:id", put(update_restaurant))
        // .route("/api/restaurants/:id", delete(delete_restaurant))
        // .route(
        //     "/api/restaurants/active-orders",
        //     get(get_restaurants_with_active_orders),
        // )
        // Health check
        // .route("/health", get(health_check))
        // Add CORS middleware
        .layer(CorsLayer::permissive())
        .layer(middleware::from_fn_with_state(state.clone(), setup_redirect_guard))
        .with_state(state)
}
