use sea_orm_migration::{prelude::*, schema::*};

/// Email and password authentication for the owner (ADR 0001): an email and
/// display name on principals, and one Argon2id hash per principal.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Principal::Table)
                    .add_column(string_len_null(Principal::Email, 254))
                    .add_column(string_len_null(Principal::DisplayName, 80))
                    .to_owned(),
            )
            .await?;

        // Case-insensitive uniqueness: sign-in looks principals up by lower(email).
        manager
            .create_index(
                Index::create()
                    .name("uq_principals_email_lower")
                    .table(Principal::Table)
                    .col(Func::lower(Expr::col(Principal::Email)))
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PasswordCredential::Table)
                    .if_not_exists()
                    .col(uuid(PasswordCredential::PrincipalId).primary_key())
                    .col(text(PasswordCredential::PasswordHash))
                    .col(timestamp_with_time_zone_default_now(
                        PasswordCredential::CreatedAt,
                    ))
                    .col(timestamp_with_time_zone_default_now(
                        PasswordCredential::UpdatedAt,
                    ))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_password_credentials_principal")
                            .from(PasswordCredential::Table, PasswordCredential::PrincipalId)
                            .to(Principal::Table, Principal::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PasswordCredential::Table).to_owned())
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("uq_principals_email_lower")
                    .table(Principal::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Principal::Table)
                    .drop_column(Principal::DisplayName)
                    .drop_column(Principal::Email)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Principal {
    #[sea_orm(iden = "principals")]
    Table,
    Id,
    Email,
    DisplayName,
}

#[derive(DeriveIden)]
enum PasswordCredential {
    #[sea_orm(iden = "password_credentials")]
    Table,
    PrincipalId,
    PasswordHash,
    CreatedAt,
    UpdatedAt,
}
