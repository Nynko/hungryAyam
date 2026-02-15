use anyhow::{anyhow, Result};
use uuid::Uuid;

use crate::features::{
    app_setup::repository::AppSetupRepository,
    menu::{
        domain::{
            menu::Menu,
            section::CreateMenuSection,
        },
        dto::{CreateMenuRequest, UpdateMenuActionsRequest},
        repository::MenuRepository,
    },
};

#[derive(Clone)]
pub struct MenuService {
    repository: MenuRepository,
    app_setup_repository: AppSetupRepository,
}

impl MenuService {
    pub fn new(repository: MenuRepository, app_setup_repository: AppSetupRepository) -> Self {
        Self {
            repository,
            app_setup_repository,
        }
    }

    // ==================== MENU OPERATIONS ====================

    /// Create a new menu with sections and items
    /// Validates nesting depth against app settings
    pub async fn create_menu(&self, request: CreateMenuRequest, user_id: Uuid) -> Result<Menu> {
        // Validate nesting depth
        let max_depth = self
            .app_setup_repository
            .get_max_menu_nesting_depth()
            .await?;
        self.validate_create_nesting_depth(&request.sections, 1, max_depth as i32)?;

        self.repository.create(request, user_id).await
    }

    /// Get a menu by ID with all sections and items
    pub async fn get_menu(&self, id: Uuid) -> Result<Option<Menu>> {
        self.repository.get_by_id(id).await
    }

    /// Get all menus for a restaurant
    pub async fn list_menus_by_restaurant(&self, restaurant_id: Uuid) -> Result<Vec<Menu>> {
        self.repository.get_by_restaurant(restaurant_id).await
    }

    /// Get only active menus for a restaurant
    pub async fn list_active_menus_by_restaurant(&self, restaurant_id: Uuid) -> Result<Vec<Menu>> {
        self.repository.get_active_by_restaurant(restaurant_id).await
    }

    /// Update a menu with update actions.
    ///
    /// Each action is executed sequentially inside a single transaction.
    /// Actions that create entities (AddSection, AddItem) store the newly
    /// created ID so that later actions can reference it via
    /// `EntityRef::CreatedBy(index)`.
    ///
    /// The authenticated `user_id` is stamped as `created_by` / `updated_by`
    /// on every row touched by the actions (clients never supply these fields).
    ///
    /// Validations performed per action:
    /// - For `AddSection` the maximum nesting depth is enforced.
    /// - Entity existence is checked at the repository level (queries fail if
    ///   the referenced row does not exist).
    pub async fn update_menu(
        &self,
        request: UpdateMenuActionsRequest,
        user_id: Uuid,
    ) -> Result<Option<Menu>> {
        let max_depth = self
            .app_setup_repository
            .get_max_menu_nesting_depth()
            .await?;

        // Delegate to the repository which runs everything in one transaction.
        self.repository
            .execute_update_actions(
                request.menu_id,
                &request.actions,
                user_id,
                max_depth as i32,
            )
            .await
    }

    /// Delete a menu
    pub async fn delete_menu(&self, id: Uuid) -> Result<bool> {
        self.repository.delete(id).await
    }

    /// Reset a non-permanent menu - sets all items to is_available = false
    /// This keeps the items in the "candidate pool" for easy re-selection
    /// Returns the number of items affected, or None if menu not found
    pub async fn reset_menu(&self, id: Uuid, user_id: Uuid) -> Result<Option<u64>> {
        self.repository.reset(id, user_id).await
    }

    // ==================== VALIDATION HELPERS ====================

    /// Validate that nesting depth doesn't exceed the maximum allowed
    /// (used during full menu creation with `CreateMenuSection` trees).
    fn validate_create_nesting_depth(
        &self,
        sections: &[CreateMenuSection],
        current_depth: i32,
        max_depth: i32,
    ) -> Result<()> {
        if current_depth > max_depth {
            return Err(anyhow!(
                "Menu section nesting depth exceeds maximum allowed ({}). Current depth: {}",
                max_depth,
                current_depth
            ));
        }

        for section in sections {
            if !section.subsections.is_empty() {
                self.validate_create_nesting_depth(
                    &section.subsections,
                    current_depth + 1,
                    max_depth,
                )?;
            }
        }

        Ok(())
    }
}