use std::{future::Future, time::Duration};

use sea_orm::{DatabaseConnection, DbErr};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

// A probe deadline, not an availability SLO or a shutdown budget.
const PROBE_TIMEOUT: Duration = Duration::from_secs(1);

pub async fn execute(database: &DatabaseConnection, shutdown: &CancellationToken) -> bool {
    check_ping(database.ping(), shutdown, PROBE_TIMEOUT).await
}

async fn check_ping(
    ping: impl Future<Output = Result<(), DbErr>>,
    shutdown: &CancellationToken,
    budget: Duration,
) -> bool {
    tokio::select! {
        biased;
        _ = shutdown.cancelled() => false,
        result = timeout(budget, ping) => {
            matches!(result, Ok(Ok(()))) && !shutdown.is_cancelled()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::{pending, ready};

    #[tokio::test]
    async fn readiness_requires_a_successful_bounded_database_ping() {
        let shutdown = CancellationToken::new();
        let budget = Duration::from_millis(10);
        assert!(check_ping(ready(Ok(())), &shutdown, budget).await);
        assert!(
            !check_ping(
                ready(Err(DbErr::Custom("secret-canary".into()))),
                &shutdown,
                budget
            )
            .await
        );
        assert!(!check_ping(pending(), &shutdown, budget).await);
    }

    #[tokio::test]
    async fn draining_does_not_poll_the_database() {
        let shutdown = CancellationToken::new();
        shutdown.cancel();
        assert!(
            !check_ping(
                async { panic!("database must not be polled") },
                &shutdown,
                PROBE_TIMEOUT
            )
            .await
        );
    }

    #[tokio::test]
    async fn shutdown_interrupts_a_database_check_and_wins_a_completion_race() {
        let shutdown = CancellationToken::new();
        let check = check_ping(pending(), &shutdown, Duration::from_secs(10));
        let cancel = async {
            tokio::task::yield_now().await;
            shutdown.cancel();
        };
        let (ready, ()) = timeout(Duration::from_secs(1), async {
            tokio::join!(check, cancel)
        })
        .await
        .expect("shutdown must interrupt the pending ping before its probe deadline");
        assert!(!ready);

        let shutdown = CancellationToken::new();
        assert!(
            !check_ping(
                async {
                    shutdown.cancel();
                    Ok(())
                },
                &shutdown,
                PROBE_TIMEOUT
            )
            .await
        );
    }
}
