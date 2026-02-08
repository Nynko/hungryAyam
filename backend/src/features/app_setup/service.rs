use anyhow::Result;

use crate::{features::app_setup::{
  domain::AppSetup,
  dto::AppSetupRequest,
  repository::AppSetupRepository
}, traits::domain_traits::IntoDomain};

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

        return self.repository.create(request.into_domain()).await;
    }
}
