use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Notify;
use crate::{
    auth::{
        service::AuthService,
        session::SessionRepository,
    },
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
        },
        order::{
            repository::OrderRepository,
            service::OrderService
        },
        offer::{
            repository::OfferRepository,
            service::OfferService
        }
    }
};

#[derive(Clone)]
pub struct AppState {
    pub setup_completed: Arc<std::sync::atomic::AtomicBool>,
    pub restaurant_service: RestaurantService,
    pub setup_service: AppSetupService,
    pub setup_repository: AppSetupRepository,
    pub user_service: UserService,
    pub item_service: ItemService,
    pub menu_service: MenuService,
    pub order_service: OrderService,
    pub offer_service: OfferService,
    pub auth_service: AuthService,
    pub session_repository: SessionRepository,
    /// Shared handle used to wake the background scheduler when data it cares
    /// about changes (e.g. session created/updated/closed, order settings
    /// changed). Call `scheduler_notify.notify_one()` after any such mutation.
    pub scheduler_notify: Arc<Notify>,
}

pub fn build_state(
    setup_completed: Arc<std::sync::atomic::AtomicBool>,
    db: PgPool,
    scheduler_notify: Arc<Notify>,
) -> AppState {

    // Create repositories
    let setup_repository = AppSetupRepository::new(db.clone());
    let restaurant_repository = RestaurantRepository::new(db.clone());
    let user_repository = UserRepository::new(db.clone());
    let item_repository = ItemRepository::new(db.clone());
    let menu_repository = MenuRepository::new(db.clone(), item_repository.clone());
    let order_repository = OrderRepository::new(db.clone());
    let offer_repository = OfferRepository::new(db.clone());
    let session_repository = SessionRepository::new(db.clone());

    // Create services
    let auth_service = AuthService::new(user_repository.clone(), session_repository.clone());
    let setup_service = AppSetupService::new(setup_repository.clone(), auth_service.clone());
    let restaurant_service = RestaurantService::new(restaurant_repository);
    let user_service = UserService::new(user_repository);
    let item_service = ItemService::new(item_repository);
    let menu_service = MenuService::new(menu_repository, setup_repository.clone());
    let offer_service = OfferService::new(offer_repository);
    let order_service = OrderService::new(order_repository, offer_service.clone(), scheduler_notify.clone());

    AppState {
        setup_completed,
        restaurant_service,
        setup_service,
        setup_repository,
        user_service,
        item_service,
        menu_service,
        order_service,
        offer_service,
        auth_service,
        session_repository,
        scheduler_notify,
    }
}