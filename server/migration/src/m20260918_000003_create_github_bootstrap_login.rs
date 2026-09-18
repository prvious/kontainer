use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(GithubLoginAttempt::Table)
                    .if_not_exists()
                    .col(pk_uuid(GithubLoginAttempt::Id))
                    .col(char_len(GithubLoginAttempt::StateHash, 64))
                    .col(string_len(GithubLoginAttempt::CodeChallenge, 128))
                    .col(
                        ColumnDef::new(GithubLoginAttempt::Status)
                            .string_len(16)
                            .not_null()
                            .check((
                                "github_login_attempts_status_valid",
                                Expr::col(GithubLoginAttempt::Status).is_in([
                                    "pending",
                                    "ready",
                                    "redeeming",
                                    "consumed",
                                ]),
                            )),
                    )
                    .col(string_len_null(GithubLoginAttempt::ProviderCode, 512))
                    .col(timestamp_with_time_zone(GithubLoginAttempt::ExpiresAt))
                    .col(timestamp_with_time_zone_null(
                        GithubLoginAttempt::ConsumedAt,
                    ))
                    .col(timestamp_with_time_zone_default_now(
                        GithubLoginAttempt::CreatedAt,
                    ))
                    .col(timestamp_with_time_zone_default_now(
                        GithubLoginAttempt::UpdatedAt,
                    ))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_github_login_attempts_state_hash")
                    .table(GithubLoginAttempt::Table)
                    .col(GithubLoginAttempt::StateHash)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_github_login_attempts_code_challenge")
                    .table(GithubLoginAttempt::Table)
                    .col(GithubLoginAttempt::CodeChallenge)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(GithubIdentity::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GithubIdentity::GithubAccountId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(uuid(GithubIdentity::PrincipalId))
                    .col(string_len(GithubIdentity::Login, 255))
                    .col(timestamp_with_time_zone_default_now(
                        GithubIdentity::CreatedAt,
                    ))
                    .col(timestamp_with_time_zone_default_now(
                        GithubIdentity::UpdatedAt,
                    ))
                    .col(timestamp_with_time_zone(
                        GithubIdentity::LastAuthenticatedAt,
                    ))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_github_identities_principal")
                            .from(GithubIdentity::Table, GithubIdentity::PrincipalId)
                            .to(Principal::Table, Principal::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_github_identities_principal")
                    .table(GithubIdentity::Table)
                    .col(GithubIdentity::PrincipalId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(BootstrapState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BootstrapState::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(uuid_null(BootstrapState::OwnerPrincipalId))
                    .col(timestamp_with_time_zone_null(BootstrapState::ClaimedAt))
                    .col(timestamp_with_time_zone_default_now(
                        BootstrapState::CreatedAt,
                    ))
                    .col(timestamp_with_time_zone_default_now(
                        BootstrapState::UpdatedAt,
                    ))
                    .check((
                        "bootstrap_state_singleton",
                        Expr::col(BootstrapState::Id).eq(1),
                    ))
                    .check((
                        "bootstrap_state_owner_claimed_consistent",
                        Expr::cust("(owner_principal_id IS NULL) = (claimed_at IS NULL)"),
                    ))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_bootstrap_state_owner")
                            .from(BootstrapState::Table, BootstrapState::OwnerPrincipalId)
                            .to(Principal::Table, Principal::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_bootstrap_state_owner")
                    .table(BootstrapState::Table)
                    .col(BootstrapState::OwnerPrincipalId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // The singleton row is the bootstrap compare-and-set target. Its
        // presence is a migration invariant, not operator input.
        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO bootstrap_state (id) VALUES (1) ON CONFLICT (id) DO NOTHING",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BootstrapState::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(GithubIdentity::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(GithubLoginAttempt::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum GithubLoginAttempt {
    #[sea_orm(iden = "github_login_attempts")]
    Table,
    Id,
    StateHash,
    CodeChallenge,
    Status,
    ProviderCode,
    ExpiresAt,
    ConsumedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum GithubIdentity {
    #[sea_orm(iden = "github_identities")]
    Table,
    GithubAccountId,
    PrincipalId,
    Login,
    CreatedAt,
    UpdatedAt,
    LastAuthenticatedAt,
}

#[derive(DeriveIden)]
enum BootstrapState {
    #[sea_orm(iden = "bootstrap_state")]
    Table,
    Id,
    OwnerPrincipalId,
    ClaimedAt,
    CreatedAt,
    UpdatedAt,
}

/// `principals` is owned by the earlier sessions migration; only the table
/// name and key are repeated here for the foreign-key targets.
#[derive(DeriveIden)]
enum Principal {
    #[sea_orm(iden = "principals")]
    Table,
    Id,
}
