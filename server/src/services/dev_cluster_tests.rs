//! Service convergence against the dev Swarm (`pnpm dev:cluster`) through the real manager engine:
//! scale 3 → 2, a restart, and a rollback each end on Swarm's own state. Runs only when
//! `KONTAINER_TEST_DEV_MANAGER_HOST` names the manager, e.g. `tcp://localhost:2375`.

use std::{env, error::Error, sync::Arc, time::Duration};

use chrono::Utc;
use sea_orm::DatabaseConnection;
use tokio_util::sync::CancellationToken;

use crate::{
    operations::{
        domain::{DurableOperation, OperationState},
        executor::{ExecutionContext, OperationExecutor},
    },
    services::{
        actions::get_service,
        domain::{ServiceAction, ServiceActionPayload},
        operations::converge::ServiceConvergeExecutor,
    },
    swarm::engine::{ManagerEngine, connect_manager},
};

/// Run one service operation's executor to its end, based on the service's current version.
async fn converge(
    manager: &Arc<dyn ManagerEngine>,
    action: ServiceAction,
    service_id: &str,
    replicas: Option<u64>,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let detail = get_service::execute(manager.as_ref(), service_id, &CancellationToken::new())
        .await
        .map_err(|error| error.to_string())?
        .ok_or("the test service is gone")?;
    let payload = ServiceActionPayload {
        service_id: service_id.to_owned(),
        name: detail.row.service.name,
        version: detail.row.service.version,
        replicas,
    };
    let now = Utc::now();
    let operation = DurableOperation {
        id: uuid::Uuid::now_v7(),
        kind: "service.test".to_owned(),
        state: OperationState::Running,
        claimed_by: None,
        claimed_at: Some(now),
        created_at: now,
        updated_at: now,
        payload: serde_json::to_value(payload)?,
        finished_at: None,
        request_id: None,
    };
    // No database: progress writes fail and are only logged.
    let context = ExecutionContext::new(
        DatabaseConnection::default(),
        None,
        CancellationToken::new(),
        operation.id,
    )
    .with_manager(Some(manager.clone()));
    ServiceConvergeExecutor::new(action)
        .with_timing(Duration::from_millis(500), Duration::from_secs(180))
        .execute(&operation, &context)
        .await
        .map(Option::unwrap_or_default)
        .map_err(|failure| format!("{}: {}", failure.code, failure.message).into())
}

async fn update_state(
    manager: &Arc<dyn ManagerEngine>,
    service_id: &str,
) -> Result<Option<String>, Box<dyn Error>> {
    Ok(
        get_service::execute(manager.as_ref(), service_id, &CancellationToken::new())
            .await
            .map_err(|error| error.to_string())?
            .ok_or("the test service is gone")?
            .row
            .service
            .update_state,
    )
}

#[tokio::test]
async fn dev_cluster_scale_down_restart_and_rollback_converge() -> Result<(), Box<dyn Error>> {
    let Ok(host) = env::var("KONTAINER_TEST_DEV_MANAGER_HOST") else {
        eprintln!("skipping dev-cluster test: KONTAINER_TEST_DEV_MANAGER_HOST is not set");
        return Ok(());
    };
    let manager = connect_manager(&host.parse()?).ok_or("no manager engine for the host")?;
    // The global seed service's image is on every node, so no pull is needed.
    let image = manager
        .list_services(&CancellationToken::new())
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|service| service.name == "monitoring_collector")
        .ok_or("seed the dev cluster first: pnpm dev:cluster:seed")?
        .image;
    // The manager port can't create services, so the test asks Docker's API directly.
    let docker = host.replace("tcp://", "http://");
    let http = reqwest::Client::new();
    let created: serde_json::Value = http
        .post(format!("{docker}/services/create"))
        .json(&serde_json::json!({
            "Name": format!("kontainer-converge-test-{}", std::process::id()),
            "Mode": {"Replicated": {"Replicas": 3}},
            "TaskTemplate": {"ContainerSpec": {"Image": image}},
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let service_id = created["ID"]
        .as_str()
        .ok_or("Docker returned no service id")?
        .to_owned();

    let evidence = async {
        converge(&manager, ServiceAction::Scale, &service_id, Some(3)).await?;
        let down = converge(&manager, ServiceAction::Scale, &service_id, Some(2)).await?;
        assert_eq!(
            down,
            serde_json::json!({"desired": 2, "running": 2, "phase": "converged"})
        );

        let restart = converge(&manager, ServiceAction::Restart, &service_id, None).await?;
        assert_eq!(restart["phase"], "converged");
        assert_eq!(
            update_state(&manager, &service_id).await?.as_deref(),
            Some("completed")
        );

        let rollback = converge(&manager, ServiceAction::Rollback, &service_id, None).await?;
        assert_eq!(rollback["phase"], "converged");
        assert_eq!(
            update_state(&manager, &service_id).await?.as_deref(),
            Some("rollback_completed")
        );
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    http.delete(format!("{docker}/services/{service_id}"))
        .send()
        .await?
        .error_for_status()?;
    evidence
}
