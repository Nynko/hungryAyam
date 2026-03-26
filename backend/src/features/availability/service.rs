use anyhow::Result;
use uuid::Uuid;

use crate::features::availability::{
    domain::{AvailabilityRule, CreateAvailabilityRule, UpdateAvailabilityRule},
    repository::AvailabilityRepository,
};

#[derive(Clone)]
pub struct AvailabilityService {
    repository: AvailabilityRepository,
}

impl AvailabilityService {
    pub fn new(repository: AvailabilityRepository) -> Self {
        Self { repository }
    }

    /// Create a new availability rule.
    pub async fn create_rule(&self, request: CreateAvailabilityRule) -> Result<AvailabilityRule> {
        self.repository.create(request).await
    }

    /// Get an availability rule by ID.
    pub async fn get_rule(&self, id: Uuid) -> Result<Option<AvailabilityRule>> {
        self.repository.get_by_id(id).await
    }

    /// Update an availability rule.
    pub async fn update_rule(&self, request: UpdateAvailabilityRule) -> Result<Option<AvailabilityRule>> {
        self.repository.update(request).await
    }

    /// Delete an availability rule.
    pub async fn delete_rule(&self, id: Uuid) -> Result<bool> {
        self.repository.delete(id).await
    }

    /// List all availability rules.
    pub async fn list_rules(&self) -> Result<Vec<AvailabilityRule>> {
        self.repository.list_all().await
    }

    /// Assign an availability rule to a menu.
    /// Pass `None` for `rule_id` to remove the assignment.
    pub async fn assign_to_menu(&self, menu_id: Uuid, rule_id: Option<Uuid>) -> Result<bool> {
        self.repository.assign_to_menu(menu_id, rule_id).await
    }

    /// Assign an availability rule to an item.
    /// Pass `None` for `rule_id` to remove the assignment.
    pub async fn assign_to_item(&self, item_id: Uuid, rule_id: Option<Uuid>) -> Result<bool> {
        self.repository.assign_to_item(item_id, rule_id).await
    }

    /// Assign an availability rule to an offer.
    /// Pass `None` for `rule_id` to remove the assignment.
    pub async fn assign_to_offer(&self, offer_id: Uuid, rule_id: Option<Uuid>) -> Result<bool> {
        self.repository.assign_to_offer(offer_id, rule_id).await
    }

    /// Get the availability rule for a menu (if assigned).
    pub async fn get_rule_for_menu(&self, menu_id: Uuid) -> Result<Option<AvailabilityRule>> {
        self.repository.get_for_menu(menu_id).await
    }

    /// Get the availability rule for an item (if assigned).
    pub async fn get_rule_for_item(&self, item_id: Uuid) -> Result<Option<AvailabilityRule>> {
        self.repository.get_for_item(item_id).await
    }

    /// Get the availability rule for an offer (if assigned).
    pub async fn get_rule_for_offer(&self, offer_id: Uuid) -> Result<Option<AvailabilityRule>> {
        self.repository.get_for_offer(offer_id).await
    }

    /// Assign an availability rule to a restaurant.
    /// Pass `None` for `rule_id` to remove the assignment.
    pub async fn assign_to_restaurant(&self, restaurant_id: Uuid, rule_id: Option<Uuid>) -> Result<bool> {
        self.repository.assign_to_restaurant(restaurant_id, rule_id).await
    }

    /// Get the availability rule for a restaurant (if assigned).
    pub async fn get_rule_for_restaurant(&self, restaurant_id: Uuid) -> Result<Option<AvailabilityRule>> {
        self.repository.get_for_restaurant(restaurant_id).await
    }
}