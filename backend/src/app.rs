use axum::{
    Router, middleware
};
use tower_http::cors::{CorsLayer, AllowOrigin};
use tower_http::limit::RequestBodyLimitLayer;

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
        offer::routes::offer_routes,
        availability::routes::availability_routes,
        upload::routes::upload_routes,
        menu_scan::routes::menu_scan_routes,
    },
    state::AppState
};

pub fn build_app(state: AppState) -> Router {
    // In production, nginx proxies all requests (same-origin), so CORS is
    // effectively a no-op.  We restrict it to same-origin by default and
    // allow the CORS_ORIGIN env var to override for development.
    let cors = match std::env::var("CORS_ORIGIN") {
        Ok(origin) if !origin.is_empty() => {
            CorsLayer::new()
                .allow_origin(origin.parse::<http::HeaderValue>().expect("Invalid CORS_ORIGIN"))
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any)
                .allow_credentials(true)
        }
        _ => {
            // Default: no cross-origin allowed (same-origin only via nginx proxy)
            CorsLayer::new()
                .allow_origin(AllowOrigin::exact(
                    http::HeaderValue::from_static("null"),
                ))
        }
    };

    // Each route group carries its own body-size limit so they don't interfere.
    // A global RequestBodyLimitLayer would override per-router limits, so limits
    // are applied per-group before merging.
    let regular_routes = Router::new()
        .merge(setup_routes())
        .merge(restaurant_routes())
        .merge(user_routes())
        .merge(item_routes())
        .merge(menu_routes())
        .merge(order_routes())
        .merge(offer_routes())
        .merge(availability_routes())
        .merge(auth_routes())
        .merge(admin_auth_routes())
        .layer(RequestBodyLimitLayer::new(2 * 1024 * 1024));

    let upload_router = upload_routes()
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024));

    // Menu scan: up to 5 images × 10 MB each + overhead
    let menu_scan_router = menu_scan_routes()
        .layer(RequestBodyLimitLayer::new(55 * 1024 * 1024));

    Router::new()
        .merge(regular_routes)
        .merge(upload_router)
        .merge(menu_scan_router)
        .layer(cors)
        .layer(middleware::from_fn_with_state(state.clone(), setup_redirect_guard))
        .with_state(state)
}