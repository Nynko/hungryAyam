use axum::{
    Router, middleware
};
use tower_http::cors::CorsLayer;

use crate::{
    setup_middleware::setup_redirect_guard,
    features::{
        app_setup::routes::setup_routes,
        restaurant::routes::restaurant_routes,
        user::routes::user_routes,
        item::routes::item_routes,
        menu::routes::menu_routes
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
        .layer(CorsLayer::permissive()) // TODO: Change
        .layer(middleware::from_fn_with_state(state.clone(), setup_redirect_guard))
        .with_state(state)
}
