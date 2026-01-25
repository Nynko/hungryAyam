use anyhow::Result;

use crate::domain::app_setup::AppSetup;
use crate::api::dtos::app_setup::{AppSetupRequest};
use crate::repository::app_setup::AppSetupRepository;

#[derive(Clone)]
pub struct AppSetupService {
    repository: AppSetupRepository,
}

impl AppSetupService {
    pub fn new(repository: AppSetupRepository) -> Self {
        Self { repository }
    }

    pub async fn setup_app(&self, request: AppSetupRequest) -> Result<AppSetup, anyhow::Error> {
        // Validate app name
        if request.app_name.trim().is_empty() {
            anyhow::bail!("App name cannot be empty");
        }

        if request.app_name.len() > 100 {
            anyhow::bail!("App name cannot exceed 100 characters");
        }

        return self.repository.create(request).await;
    }
}
