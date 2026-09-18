use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "github_login_attempts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub state_hash: String,
    pub code_challenge: String,
    pub status: String,
    pub provider_code: Option<String>,
    pub expires_at: DateTimeUtc,
    pub consumed_at: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

impl std::fmt::Debug for Model {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Model")
            .field("id", &self.id)
            .field("state_hash", &"<redacted>")
            .field("code_challenge", &"<redacted>")
            .field("status", &self.status)
            .field(
                "provider_code",
                &self.provider_code.as_ref().map(|_| "<redacted>"),
            )
            .field("expires_at", &self.expires_at)
            .field("consumed_at", &self.consumed_at)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
    use super::Model;

    #[test]
    fn attempt_debug_omits_secrets() {
        let model = Model {
            id: uuid::Uuid::now_v7(),
            state_hash: "secret-canary-state".to_owned(),
            code_challenge: "secret-canary-challenge".to_owned(),
            status: "pending".to_owned(),
            provider_code: Some("secret-canary-code".to_owned()),
            expires_at: chrono::Utc::now(),
            consumed_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let rendered = format!("{model:?}");
        assert!(!rendered.contains("secret-canary"));
    }
}
