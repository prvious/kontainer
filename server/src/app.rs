use std::path::Path;

use async_trait::async_trait;
use loco_rs::{
    Result,
    app::{AppContext, Hooks},
    bgworker::Queue,
    boot::{BootResult, ServeParams, StartMode, create_app},
    config::Config,
    controller::AppRoutes,
    environment::Environment,
    task::Tasks,
};
use migration::Migrator;

use crate::{controllers, server};

pub struct App;

#[async_trait]
impl Hooks for App {
    fn app_name() -> &'static str {
        env!("CARGO_CRATE_NAME")
    }

    fn app_version() -> String {
        format!(
            "{} ({})",
            env!("CARGO_PKG_VERSION"),
            option_env!("BUILD_SHA")
                .or(option_env!("GITHUB_SHA"))
                .unwrap_or("dev")
        )
    }

    async fn boot(
        mode: StartMode,
        environment: &Environment,
        config: Config,
    ) -> Result<BootResult> {
        create_app::<Self, Migrator>(mode, environment, config).await
    }

    fn routes(ctx: &AppContext) -> AppRoutes {
        // Access-session middleware covers the application group only.
        // Lifecycle routes validate their own credentials; probes stay
        // anonymous. Layering is per group, never a path-prefix test.
        let require_session = axum::middleware::from_fn_with_state(
            ctx.db.clone(),
            crate::auth::request::require_access_session,
        );
        let protected = controllers::protected_application_routes()
            .into_iter()
            .map(|routes| routes.layer(require_session.clone()))
            .collect::<Vec<_>>();
        AppRoutes::empty()
            .add_routes(vec![controllers::health::routes()])
            .nest_routes("/api/v1", controllers::auth_lifecycle_routes())
            .nest_routes("/api/v1", protected)
    }

    async fn after_context(ctx: AppContext) -> Result<AppContext> {
        crate::server::init_shutdown_state(&ctx)?;
        crate::server::init_shared_auth_state(&ctx)?;
        crate::server::init_docker_state(&ctx)?;
        Ok(ctx)
    }

    async fn serve(
        router: axum::Router,
        ctx: &AppContext,
        serve_params: &ServeParams,
    ) -> Result<()> {
        let result = server::serve(router, ctx, serve_params).await;
        Self::on_shutdown(ctx).await;
        result
    }

    async fn connect_workers(_ctx: &AppContext, _queue: &Queue) -> Result<()> {
        Ok(())
    }

    fn register_tasks(tasks: &mut Tasks) {
        tasks.register(crate::tasks::reset_password::ResetPassword);
    }

    async fn truncate(_ctx: &AppContext) -> Result<()> {
        Ok(())
    }

    async fn seed(_ctx: &AppContext, _base: &Path) -> Result<()> {
        Ok(())
    }
}
