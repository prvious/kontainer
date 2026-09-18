use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "refresh_credentials")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub family_id: Uuid,
    pub generation: i32,
    pub credential_hash: String,
    pub expires_at: DateTimeUtc,
    pub consumed_at: Option<DateTimeUtc>,
    pub replaced_by: Option<Uuid>,
    pub revoked_at: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
}

impl std::fmt::Debug for Model {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Model")
            .field("id", &self.id)
            .field("family_id", &self.family_id)
            .field("generation", &self.generation)
            .field("credential_hash", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .field("consumed_at", &self.consumed_at)
            .field("replaced_by", &self.replaced_by)
            .field("revoked_at", &self.revoked_at)
            .field("created_at", &self.created_at)
            .finish()
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::session_family::Entity",
        from = "Column::FamilyId",
        to = "super::session_family::Column::Id",
        on_update = "NoAction",
        on_delete = "Restrict"
    )]
    SessionFamily,
}

impl Related<super::session_family::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SessionFamily.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
    use super::Model;

    #[test]
    fn refresh_debug_omits_the_credential_hash() {
        let model = Model {
            id: uuid::Uuid::now_v7(),
            family_id: uuid::Uuid::now_v7(),
            generation: 0,
            credential_hash: "secret-canary-refresh-hash".to_owned(),
            expires_at: chrono::Utc::now(),
            consumed_at: None,
            replaced_by: None,
            revoked_at: None,
            created_at: chrono::Utc::now(),
        };
        let rendered = format!("{model:?}");
        assert!(!rendered.contains("secret-canary"));
    }
}
