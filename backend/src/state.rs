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
        },
        menu::{
            repository::MenuRepository,
            service::MenuService
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
    pub menu_service: MenuService,
}

pub fn build_state(setup_completed: Arc<std::sync::atomic::AtomicBool>,
    db: PgPool) -> AppState {

    // Create repositories
    let setup_repository = AppSetupRepository::new(db.clone());
    let restaurant_repository = RestaurantRepository::new(db.clone());
    let user_repository = UserRepository::new(db.clone());
    let item_repository = ItemRepository::new(db.clone());
    let menu_repository = MenuRepository::new(db.clone(), item_repository.clone());

    // Create services
    let setup_service = AppSetupService::new(setup_repository.clone());
    let restaurant_service = RestaurantService::new(restaurant_repository);
    let user_service = UserService::new(user_repository);
    let item_service = ItemService::new(item_repository);
    let menu_service = MenuService::new(menu_repository, setup_repository);

    return AppState {
        setup_completed,
        restaurant_service,
        setup_service,
        user_service,
        item_service,
        menu_service,
    };
}
