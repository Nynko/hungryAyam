use anyhow::Result;

use crate::{
    auth::{
        password::sha256_hex,
        service::AuthService,
    },
    features::app_setup::{
        domain::{AppSetup, CreateAppSetup},
        dto::AppSetupRequest,
        repository::AppSetupRepository,
    },
};

#[derive(Clone)]
pub struct AppSetupService {
    repository: AppSetupRepository,
    auth_service: AuthService,
}

impl AppSetupService {
    pub fn new(repository: AppSetupRepository, auth_service: AuthService) -> Self {
        Self {
            repository,
            auth_service,
        }
    }

    pub async fn setup_app(&self, request: AppSetupRequest) -> Result<AppSetup, anyhow::Error> {
        // Validate app name
        if request.app_name.trim().is_empty() {
            anyhow::bail!("App name cannot be empty");
        }

        if request.app_name.len() > 100 {
            anyhow::bail!("App name cannot exceed 100 characters");
        }

        // Validate access code
        if request.access_code.trim().is_empty() {
            anyhow::bail!("Access code cannot be empty");
        }

        // Hash the access code with SHA-256 before storing
        let access_hash = sha256_hex(&request.access_code);

        // Create the app settings
        let create_setup = CreateAppSetup {
            title: request.app_name,
            image_url: request.image_url,
            access_hash,
        };

        let setup = self.repository.create(create_setup).await?;

        // Create the first admin user
        self.auth_service
            .create_first_admin(request.admin_name, request.admin_email, &request.admin_password)
            .await?;

        Ok(setup)
    }
}