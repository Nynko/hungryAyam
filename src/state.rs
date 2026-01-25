use sqlx::PgPool;
use std::sync::Arc;
use crate::{
    features::{
        restaurant::{
        repository::RestaurantRepository,
        service::RestaurantService
        },
        app_setup::{
            repository::AppSetupRepository,
            service::AppSetupService
        }
    }
};

#[derive(Clone)]
pub struct AppState {
    pub setup_completed: Arc<std::sync::atomic::AtomicBool>,
    pub restaurant_service: RestaurantService,
    pub setup_service: AppSetupService
}

pub fn build_state(setup_completed: Arc<std::sync::atomic::AtomicBool>,
    db: PgPool) -> AppState {

    // Create repository and service
    let setup_repository = AppSetupRepository::new(db.clone());
    let setup_service = AppSetupService::new(setup_repository);
    let restaurant_repository = RestaurantRepository::new(db.clone());
    let restaurant_service = RestaurantService::new(restaurant_repository);

    return AppState {
        setup_completed,
        restaurant_service,
        setup_service
    };
}
