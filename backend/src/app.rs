use axum::{
    Router, middleware
};
use tower_http::cors::CorsLayer;

use crate::{
    setup_middleware::setup_redirect_guard,
    auth::routes::{auth_routes, admin_auth_routes},
    features::{
        app_setup::routes::setup_routes,
        restaurant::routes::restaurant_routes,
        user::routes::user_routes,
        item::routes::item_routes,
        menu::routes::menu_routes,
        order::routes::order_routes,
        offer::routes::offer_routes
    },
    state::AppState
};

pub fn build_app(state: AppState) -> Router {
    Router::new()
        .merge(setup_routes())
        .merge(restaurant_routes())
        .merge(user_routes())
        .merge(item_routes())
        .merge(menu_routes())
        .merge(order_routes())
        .merge(offer_routes())
        .merge(auth_routes())
        .merge(admin_auth_routes())
        // Add CORS middleware
        .layer(CorsLayer::permissive()) // TODO: Change
        .layer(middleware::from_fn_with_state(state.clone(), setup_redirect_guard))
        .with_state(state)
}