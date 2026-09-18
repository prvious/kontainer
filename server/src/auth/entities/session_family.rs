use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "session_families")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub principal_id: Uuid,
    pub access_hash: String,
    pub access_expires_at: DateTimeUtc,
    pub absolute_expires_at: DateTimeUtc,
    pub revoked_at: Option<DateTimeUtc>,
    pub revoked_reason: Option<String>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

impl std::fmt::Debug for Model {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Model")
            .field("id", &self.id)
            .field("principal_id", &self.principal_id)
            .field("access_hash", &"<redacted>")
            .field("access_expires_at", &self.access_expires_at)
            .field("absolute_expires_at", &self.absolute_expires_at)
            .field("revoked_at", &self.revoked_at)
            .field("revoked_reason", &self.revoked_reason)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::principal::Entity",
        from = "Column::PrincipalId",
        to = "super::principal::Column::Id",
        on_update = "NoAction",
        on_delete = "Restrict"
    )]
    Principal,
    #[sea_orm(has_many = "super::refresh_credential::Entity")]
    RefreshCredentials,
    #[sea_orm(has_many = "super::security_audit_intent::Entity")]
    SecurityAuditIntents,
}

impl Related<super::principal::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Principal.def()
    }
}

impl Related<super::refresh_credential::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RefreshCredentials.def()
    }
}

impl Related<super::security_audit_intent::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SecurityAuditIntents.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
    use super::Model;

    #[test]
    fn family_debug_omits_the_access_hash() {
        let model = Model {
            id: uuid::Uuid::now_v7(),
            principal_id: uuid::Uuid::now_v7(),
            access_hash: "secret-canary-access-hash".to_owned(),
            access_expires_at: chrono::Utc::now(),
            absolute_expires_at: chrono::Utc::now(),
            revoked_at: None,
            revoked_reason: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let rendered = format!("{model:?}");
        assert!(!rendered.contains("secret-canary"));
    }
}
