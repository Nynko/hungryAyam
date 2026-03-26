use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::features::availability::domain::{CreateAvailabilityRule, UpdateAvailabilityRule};

pub type CreateAvailabilityRuleRequest = CreateAvailabilityRule;
pub type UpdateAvailabilityRuleRequest = UpdateAvailabilityRule;

/// Request body for assigning (or removing) an availability rule.
/// Send `null` for `availability_rule_id` to remove the assignment.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AssignAvailabilityRequest {
    pub availability_rule_id: Option<Uuid>,
}