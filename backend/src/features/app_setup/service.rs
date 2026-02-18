use crate::{
    auth::service::AuthService,
    features::app_setup::{
        domain::{AppSetup, CreateAppSetup},
        dto::AppSetupRequest,
        repository::AppSetupRepository,
    },
    types::password::parse_hashed_password,
    utils::password::sha256_hex,
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

    pub async fn setup_app(&self, request: AppSetupRequest) -> anyhow::Result<AppSetup> {
        // Hash the access code with SHA-256 before storing
        // (ClearPassword is already validated by serde deserialization)
        let access_hash_str = sha256_hex(request.access_code.as_ref());
        let access_hash = parse_hashed_password(access_hash_str)?;

        // Create the app settings
        let create_setup = CreateAppSetup { access_hash };

        let setup = self.repository.create(create_setup).await?;

        // Create the first admin user
        self.auth_service
            .create_first_admin(request.admin_name, request.admin_email, &request.admin_password)
            .await?;

        Ok(setup)
    }
}
