use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Principal::Table)
                    .if_not_exists()
                    .col(pk_uuid(Principal::Id))
                    .col(
                        ColumnDef::new(Principal::Kind)
                            .string_len(16)
                            .not_null()
                            .check((
                                "principals_kind_valid",
                                Expr::col(Principal::Kind).is_in(["human", "service"]),
                            )),
                    )
                    .col(
                        ColumnDef::new(Principal::Status)
                            .string_len(16)
                            .not_null()
                            .check((
                                "principals_status_valid",
                                Expr::col(Principal::Status).is_in([
                                    "pending",
                                    "active",
                                    "suspended",
                                    "deleted",
                                ]),
                            )),
                    )
                    .col(integer(Principal::AuthorizationEpoch))
                    .col(timestamp_with_time_zone_default_now(Principal::CreatedAt))
                    .col(timestamp_with_time_zone_default_now(Principal::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SessionFamily::Table)
                    .if_not_exists()
                    .col(pk_uuid(SessionFamily::Id))
                    .col(uuid(SessionFamily::PrincipalId))
                    .col(char_len(SessionFamily::AccessHash, 64))
                    .col(timestamp_with_time_zone(SessionFamily::AccessExpiresAt))
                    .col(timestamp_with_time_zone(SessionFamily::AbsoluteExpiresAt))
                    .col(timestamp_with_time_zone_null(SessionFamily::RevokedAt))
                    .col(string_len_null(SessionFamily::RevokedReason, 64))
                    .col(timestamp_with_time_zone_default_now(
                        SessionFamily::CreatedAt,
                    ))
                    .col(timestamp_with_time_zone_default_now(
                        SessionFamily::UpdatedAt,
                    ))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_session_families_principal")
                            .from(SessionFamily::Table, SessionFamily::PrincipalId)
                            .to(Principal::Table, Principal::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_session_families_access_hash")
                    .table(SessionFamily::Table)
                    .col(SessionFamily::AccessHash)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_session_families_principal")
                    .table(SessionFamily::Table)
                    .col(SessionFamily::PrincipalId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(RefreshCredential::Table)
                    .if_not_exists()
                    .col(pk_uuid(RefreshCredential::Id))
                    .col(uuid(RefreshCredential::FamilyId))
                    .col(integer(RefreshCredential::Generation))
                    .col(char_len(RefreshCredential::CredentialHash, 64))
                    .col(timestamp_with_time_zone(RefreshCredential::ExpiresAt))
                    .col(timestamp_with_time_zone_null(RefreshCredential::ConsumedAt))
                    .col(uuid_null(RefreshCredential::ReplacedBy))
                    .col(timestamp_with_time_zone_null(RefreshCredential::RevokedAt))
                    .col(timestamp_with_time_zone_default_now(
                        RefreshCredential::CreatedAt,
                    ))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_refresh_credentials_family")
                            .from(RefreshCredential::Table, RefreshCredential::FamilyId)
                            .to(SessionFamily::Table, SessionFamily::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_refresh_credentials_hash")
                    .table(RefreshCredential::Table)
                    .col(RefreshCredential::CredentialHash)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_refresh_family_generation")
                    .table(RefreshCredential::Table)
                    .col(RefreshCredential::FamilyId)
                    .col(RefreshCredential::Generation)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_refresh_credentials_family")
                    .table(RefreshCredential::Table)
                    .col(RefreshCredential::FamilyId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SecurityAuditIntent::Table)
                    .if_not_exists()
                    .col(pk_uuid(SecurityAuditIntent::Id))
                    .col(uuid(SecurityAuditIntent::PrincipalId))
                    .col(uuid(SecurityAuditIntent::FamilyId))
                    .col(
                        ColumnDef::new(SecurityAuditIntent::Action)
                            .string_len(64)
                            .not_null()
                            .check((
                                "security_audit_intents_action_valid",
                                Expr::col(SecurityAuditIntent::Action).is_in([
                                    "session.create",
                                    "session.refresh.rotate",
                                    "session.refresh.reuse",
                                    "session.revoke",
                                ]),
                            )),
                    )
                    .col(
                        ColumnDef::new(SecurityAuditIntent::State)
                            .string_len(32)
                            .not_null()
                            .check((
                                "security_audit_intents_state_valid",
                                Expr::col(SecurityAuditIntent::State).is_in(["pending"]),
                            )),
                    )
                    .col(timestamp_with_time_zone_default_now(
                        SecurityAuditIntent::CreatedAt,
                    ))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_security_audit_principal")
                            .from(SecurityAuditIntent::Table, SecurityAuditIntent::PrincipalId)
                            .to(Principal::Table, Principal::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_security_audit_family")
                            .from(SecurityAuditIntent::Table, SecurityAuditIntent::FamilyId)
                            .to(SessionFamily::Table, SessionFamily::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_security_audit_family")
                    .table(SecurityAuditIntent::Table)
                    .col(SecurityAuditIntent::FamilyId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_security_audit_principal")
                    .table(SecurityAuditIntent::Table)
                    .col(SecurityAuditIntent::PrincipalId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SecurityAuditIntent::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(RefreshCredential::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SessionFamily::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Principal::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Principal {
    #[sea_orm(iden = "principals")]
    Table,
    Id,
    Kind,
    Status,
    AuthorizationEpoch,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum SessionFamily {
    #[sea_orm(iden = "session_families")]
    Table,
    Id,
    PrincipalId,
    AccessHash,
    AccessExpiresAt,
    AbsoluteExpiresAt,
    RevokedAt,
    RevokedReason,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum RefreshCredential {
    #[sea_orm(iden = "refresh_credentials")]
    Table,
    Id,
    FamilyId,
    Generation,
    CredentialHash,
    ExpiresAt,
    ConsumedAt,
    ReplacedBy,
    RevokedAt,
    CreatedAt,
}

#[derive(DeriveIden)]
enum SecurityAuditIntent {
    #[sea_orm(iden = "security_audit_intents")]
    Table,
    Id,
    PrincipalId,
    FamilyId,
    Action,
    State,
    CreatedAt,
}
