//! Budget policy supplied by the composition layer before every actual Send.

#[async_trait::async_trait]
pub trait ConversationSendAdmissionPort: Send + Sync {
    /// Verify the current recorded spend. The caller serializes this check with
    /// insertion of its unknown usage marker, including retries/continuations.
    async fn check(
        &self,
        user_id: &str,
        conversation_id: &str,
        team_id: Option<&str>,
        app_turn_id: &str,
    ) -> Result<(), String>;
}
