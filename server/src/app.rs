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
use tokio_util::sync::CancellationToken;

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

    fn routes(_ctx: &AppContext) -> AppRoutes {
        AppRoutes::empty()
            .add_routes(vec![controllers::health::routes()])
            .nest_routes(
                "/api/v1",
                vec![
                    controllers::status::routes(),
                    controllers::operations::routes(),
                    controllers::sessions::routes(),
                ],
            )
    }

    async fn after_context(ctx: AppContext) -> Result<AppContext> {
        ctx.shared_store.insert(CancellationToken::new());
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

    fn register_tasks(_tasks: &mut Tasks) {}

    async fn truncate(_ctx: &AppContext) -> Result<()> {
        Ok(())
    }

    async fn seed(_ctx: &AppContext, _base: &Path) -> Result<()> {
        Ok(())
    }
}
