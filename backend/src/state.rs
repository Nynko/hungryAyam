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
        },
        user::{
            repository::UserRepository,
            service::UserService
        },
        item::{
            repository::ItemRepository,
            service::ItemService
        }
    }
};

#[derive(Clone)]
pub struct AppState {
    pub setup_completed: Arc<std::sync::atomic::AtomicBool>,
    pub restaurant_service: RestaurantService,
    pub setup_service: AppSetupService,
    pub user_service: UserService,
    pub item_service: ItemService,
}

pub fn build_state(setup_completed: Arc<std::sync::atomic::AtomicBool>,
    db: PgPool) -> AppState {

    // Create repository and service
    let setup_repository = AppSetupRepository::new(db.clone());
    let setup_service = AppSetupService::new(setup_repository);
    let restaurant_repository = RestaurantRepository::new(db.clone());
    let restaurant_service = RestaurantService::new(restaurant_repository);
    let user_repository = UserRepository::new(db.clone());
    let user_service = UserService::new(user_repository);
    let item_repository = ItemRepository::new(db.clone());
    let item_service = ItemService::new(item_repository);

    return AppState {
        setup_completed,
        restaurant_service,
        setup_service,
        user_service,
        item_service,
    };
}