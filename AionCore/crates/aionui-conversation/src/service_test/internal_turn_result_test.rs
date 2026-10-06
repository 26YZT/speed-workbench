use super::*;
use crate::skill_resolver::LoadedAgentSkill;

struct PlanningSkillResolver;

#[async_trait::async_trait]
impl SkillResolver for PlanningSkillResolver {
    async fn auto_inject_names(&self) -> Vec<String> {
        vec!["product-blueprint".into()]
    }

    async fn resolve_skills(&self, _names: &[String]) -> Vec<ResolvedAgentSkill> {
        Vec::new()
    }

    async fn load_skill_bodies_for_user(&self, _user_id: &str, names: &[String]) -> Vec<LoadedAgentSkill> {
        names
            .iter()
            .map(|name| LoadedAgentSkill {
                name: name.clone(),
                body: "Return a scoped plan".into(),
                source_path: PathBuf::from("/planning-skill"),
            })
            .collect()
    }
}

fn service_with_scripted_agents(
    agents: Vec<AgentInstance>,
) -> (
    ConversationService,
    Arc<RebuildingScriptedTaskManager>,
    Arc<MockBroadcaster>,
) {
    let manager = Arc::new(RebuildingScriptedTaskManager::new(agents));
    let broadcaster = Arc::new(MockBroadcaster::new());
    let service = ConversationService::new(
        std::env::temp_dir(),
        broadcaster.clone(),
        Arc::new(PlanningSkillResolver),
        manager.clone(),
        Arc::new(MockRepo::new()),
        Arc::new(StubAgentMetadataRepo),
        Arc::new(StubAcpSessionRepo::default()),
    );
    (service, manager, broadcaster)
}

fn internal_request(conversation_id: &str) -> ConversationAgentTurnRequest {
    ConversationAgentTurnRequest {
        user_id: "user_1".into(),
        conversation_id: conversation_id.into(),
        task_id: None,
        task_workspace: None,
        content: "Plan this product".into(),
        files: Vec::new(),
        inject_skills: Vec::new(),
        required_runtime_mode: None,
        persist_user_message: true,
        user_message_hidden: true,
        on_started: None,
    }
}

fn reply(text: &str) -> Vec<AgentStreamEvent> {
    vec![
        AgentStreamEvent::Text(TextEventData { content: text.into() }),
        AgentStreamEvent::Finish(FinishEventData::default()),
    ]
}

fn retryable_error(message: &str) -> Vec<AgentStreamEvent> {
    vec![AgentStreamEvent::Error(ErrorEventData {
        message: message.into(),
        code: Some(AgentErrorCode::UnknownUpstreamError),
        ownership: None,
        detail: None,
        workspace_path: None,
        retryable: Some(true),
        feedback_recommended: None,
        resolution: None,
    })]
}

struct RecordedBudgetPort {
    usage: Arc<dyn aionui_db::IAgentUsageRepository>,
    checks: std::sync::atomic::AtomicUsize,
}

#[async_trait::async_trait]
impl crate::ConversationSendAdmissionPort for RecordedBudgetPort {
    async fn check(&self, user: &str, conversation: &str, team: Option<&str>, _turn: &str) -> Result<(), String> {
        self.checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let rows = match team {
            Some(team) => self.usage.list_unassigned_by_team(user, team).await,
            None => self.usage.list_by_conversation(user, conversation).await,
        }
        .map_err(|_| "budget_storage_failed".to_owned())?;
        if rows.iter().any(|row| row.cost_est.is_none()) {
            return Err("budget_cost_unknown".into());
        }
        if rows.iter().filter_map(|row| row.cost_est).sum::<f64>() >= 1.0 {
            return Err("budget_exceeded".into());
        }
        // Widen the competing sender's read/write window in the race test.
        tokio::task::yield_now().await;
        Ok(())
    }
}

async fn with_recorded_budget(
    service: ConversationService,
) -> (
    ConversationService,
    Arc<aionui_db::SqliteAgentUsageRepository>,
    Arc<RecordedBudgetPort>,
) {
    let database = aionui_db::init_database_memory().await.unwrap();
    let usage = Arc::new(aionui_db::SqliteAgentUsageRepository::new(database.pool().clone()));
    let port = Arc::new(RecordedBudgetPort {
        usage: usage.clone(),
        checks: Default::default(),
    });
    let service = service.with_usage_repository(usage.clone());
    service.with_send_admission_port(port.clone());
    (service, usage, port)
}

#[tokio::test]
async fn common_budget_blocks_skill_continuation_after_unknown_first_send() {
    use aionui_db::IAgentUsageRepository;
    let agent = Arc::new(ScriptedAgent::new(
        "placeholder",
        vec![
            reply("Loading [LOAD_SKILL: product-blueprint]"),
            reply("{\"ready\":true}"),
        ],
    ));
    let (service, manager, _) = service_with_scripted_agents(vec![AgentInstance::Mock(agent.clone())]);
    let (service, usage, port) = with_recorded_budget(service).await;
    let conversation = service.create("user_1", make_create_req()).await.unwrap();
    let result = service
        .run_agent_turn(internal_request(&conversation.id))
        .await
        .unwrap();
    assert_eq!(result.status, ConversationAgentTurnStatus::Failed);
    assert!(
        result
            .error_message
            .as_deref()
            .unwrap()
            .contains("budget cannot be verified or is exhausted")
    );
    assert!(!result.output_complete);
    assert_eq!(result.final_text, None);
    assert!(result.admission_rejected);
    assert_eq!(manager.build_count(), 1);
    assert_eq!(
        manager.kill_count(),
        0,
        "budget refusal must not evict a healthy runtime"
    );
    assert_eq!(agent.sent_contents().len(), 1, "the second paid Send must never occur");
    assert_eq!(port.checks.load(std::sync::atomic::Ordering::SeqCst), 2);
    let rows = usage.list_by_conversation("user_1", &conversation.id).await.unwrap();
    assert_eq!(rows.len(), 1, "a rejected send must not create another billable marker");
    assert_eq!(rows[0].cost_unknown_reason.as_deref(), Some("usage_not_reported"));
}

#[tokio::test]
async fn common_budget_blocks_auto_replay_after_unknown_first_send() {
    use aionui_db::IAgentUsageRepository;
    let first = Arc::new(ScriptedAgent::new(
        "placeholder",
        vec![retryable_error("retryable fixture failure")],
    ));
    let second = Arc::new(ScriptedAgent::new("placeholder", vec![reply("{\"ready\":true}")]));
    let (service, manager, _) = service_with_scripted_agents(vec![
        AgentInstance::Mock(first.clone()),
        AgentInstance::Mock(second.clone()),
    ]);
    let (service, usage, port) = with_recorded_budget(service).await;
    let conversation = service.create("user_1", make_create_req()).await.unwrap();
    let result = service
        .run_agent_turn(internal_request(&conversation.id))
        .await
        .unwrap();
    assert_eq!(result.status, ConversationAgentTurnStatus::Failed);
    assert!(
        result
            .error_message
            .as_deref()
            .unwrap()
            .contains("budget cannot be verified or is exhausted")
    );
    assert_eq!(first.sent_contents().len(), 1);
    assert!(result.admission_rejected);
    assert_eq!(manager.build_count(), 2);
    assert_eq!(
        manager.kill_count(),
        1,
        "only the original provider error may evict its runtime"
    );
    assert!(
        second.sent_contents().is_empty(),
        "rebuilding a runtime must not bypass the budget gate"
    );
    assert!(port.checks.load(std::sync::atomic::Ordering::SeqCst) >= 2);
    assert_eq!(
        usage
            .list_by_conversation("user_1", &conversation.id)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn common_budget_check_and_marker_are_serialized_across_service_clones() {
    use aionui_db::IAgentUsageRepository;
    let (service, _, _) = service_with_scripted_agents(Vec::new());
    let (service, usage, _) = with_recorded_budget(service).await;
    let send = |attempt: &str| aionui_ai_agent::types::SendMessageData {
        content: "fixture".into(),
        msg_id: format!("message-{attempt}"),
        turn_id: Some("turn".into()),
        task_id: None,
        files: Vec::new(),
        inject_skills: Vec::new(),
        usage_attempt_id: Some(attempt.into()),
    };
    let other = service.clone();
    let send_a = send("a");
    let send_b = send("b");
    let (a, b) = tokio::join!(
        service.ensure_usage_admission("user_1", "conversation-a", Some("team".into()), &send_a),
        other.ensure_usage_admission("user_1", "conversation-b", Some("team".into()), &send_b),
    );
    assert_ne!(
        a.is_ok(),
        b.is_ok(),
        "only one concurrent sender may reserve the first unknown call"
    );
    let rows = usage.list_unassigned_by_team("user_1", "team").await.unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].cost_est.is_none());
}

struct CancelDuringAdmission {
    runtime: Arc<crate::runtime_state::ConversationRuntimeStateService>,
}

#[async_trait::async_trait]
impl crate::ConversationSendAdmissionPort for CancelDuringAdmission {
    async fn check(&self, _user: &str, conversation: &str, _team: Option<&str>, _turn: &str) -> Result<(), String> {
        tokio::task::yield_now().await;
        self.runtime.mark_cancelling(conversation);
        Ok(())
    }
}

#[tokio::test]
async fn common_budget_wait_cancellation_never_sends_or_evicts_the_runtime() {
    use aionui_db::IAgentUsageRepository;
    let agent = Arc::new(ScriptedAgent::new(
        "placeholder",
        vec![reply("must never be generated")],
    ));
    let (service, manager, _) = service_with_scripted_agents(vec![AgentInstance::Mock(agent.clone())]);
    let (service, usage, _) = with_recorded_budget(service).await;
    service.with_send_admission_port(Arc::new(CancelDuringAdmission {
        runtime: service.runtime_state(),
    }));
    let conversation = service.create("user_1", make_create_req()).await.unwrap();
    let result = service
        .run_agent_turn(internal_request(&conversation.id))
        .await
        .unwrap();
    assert_eq!(result.status, ConversationAgentTurnStatus::Completed);
    assert_eq!(result.error_message, None);
    assert_eq!(result.final_text, None);
    assert!(!result.output_complete);
    assert!(agent.sent_contents().is_empty());
    assert_eq!(manager.build_count(), 1);
    assert_eq!(manager.kill_count(), 0);
    let rows = usage.list_by_conversation("user_1", &conversation.id).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].cost_est, Some(0.0));
    assert_eq!(rows[0].cost_source.as_deref(), Some("not_sent"));
    assert_eq!(rows[0].input_tokens, 0);
}

#[tokio::test]
async fn normalized_internal_result_keeps_app_turn_id_and_middleware_text() {
    let mut script = reply(" <think>private</think>{\"plan\":\"ready\"} ");
    script.insert(0, AgentStreamEvent::BackendTurnBound("provider-native-turn".into()));
    let agent = Arc::new(ScriptedAgent::new("placeholder", vec![script]));
    let (service, _, broadcaster) = service_with_scripted_agents(vec![AgentInstance::Mock(agent)]);
    let conversation = service.create("user_1", make_create_req()).await.unwrap();
    let started = Arc::new(Mutex::new(None));
    let recorded = started.clone();
    let mut request = internal_request(&conversation.id);
    request.on_started = Some(Arc::new(move |receipt| {
        let recorded = recorded.clone();
        Box::pin(async move {
            *recorded.lock().unwrap() = Some(receipt.turn_id);
        })
    }));
    let result = service.run_agent_turn(request).await.unwrap();
    assert_eq!(result.status, ConversationAgentTurnStatus::Completed);
    assert_eq!(result.final_text.as_deref(), Some("{\"plan\":\"ready\"}"));
    assert!(result.output_complete);
    assert_eq!(started.lock().unwrap().as_deref(), Some(result.turn_id.as_str()));
    assert_ne!(result.turn_id, "provider-native-turn");
    let terminal = broadcaster
        .take_events()
        .into_iter()
        .find(|event| event.name == "turn.completed")
        .unwrap();
    assert_eq!(terminal.data["turn_id"], result.turn_id);
}

#[tokio::test]
async fn normalized_internal_result_returns_last_skill_continuation_only() {
    let agent = Arc::new(ScriptedAgent::new(
        "placeholder",
        vec![
            reply("Loading [LOAD_SKILL: product-blueprint]"),
            reply("<think>details</think>{\"tasks\":[\"final\"]}"),
        ],
    ));
    let (service, _, _) = service_with_scripted_agents(vec![AgentInstance::Mock(agent.clone())]);
    let conversation = service.create("user_1", make_create_req()).await.unwrap();
    let result = service
        .run_agent_turn(internal_request(&conversation.id))
        .await
        .unwrap();
    assert_eq!(result.status, ConversationAgentTurnStatus::Completed);
    assert_eq!(result.final_text.as_deref(), Some("{\"tasks\":[\"final\"]}"));
    assert!(result.output_complete);
    let sent = agent.sent_contents();
    assert_eq!(sent.len(), 2);
    assert!(sent[1].starts_with("[Skill: product-blueprint]"));
}

#[tokio::test]
async fn normalized_internal_result_rejects_skill_continuation_limit() {
    let scripts = (0..5)
        .map(|_| reply("Loading [LOAD_SKILL: product-blueprint]"))
        .collect();
    let agent = Arc::new(ScriptedAgent::new("placeholder", scripts));
    let (service, _, _) = service_with_scripted_agents(vec![AgentInstance::Mock(agent.clone())]);
    let conversation = service.create("user_1", make_create_req()).await.unwrap();
    let result = service
        .run_agent_turn(internal_request(&conversation.id))
        .await
        .unwrap();
    assert_eq!(result.status, ConversationAgentTurnStatus::Completed);
    assert_eq!(agent.sent_contents().len(), 5);
    assert_eq!(result.final_text, None);
    assert!(!result.output_complete);
}

#[tokio::test]
async fn normalized_internal_result_does_not_fall_back_to_intermediate_text_after_failure() {
    let agent = Arc::new(ScriptedAgent::new(
        "placeholder",
        vec![
            reply("Loading [LOAD_SKILL: product-blueprint]"),
            retryable_error("continuation failed"),
        ],
    ));
    let (service, manager, _) = service_with_scripted_agents(vec![AgentInstance::Mock(agent.clone())]);
    let conversation = service.create("user_1", make_create_req()).await.unwrap();
    let result = service
        .run_agent_turn(internal_request(&conversation.id))
        .await
        .unwrap();
    assert_eq!(result.status, ConversationAgentTurnStatus::Failed);
    assert!(result.error_message.is_some());
    assert_eq!(agent.sent_contents().len(), 2);
    assert_eq!(manager.build_count(), 1, "visible continuation work must not replay");
    assert_eq!(result.final_text, None);
    assert!(!result.output_complete);
}

#[tokio::test]
async fn normalized_internal_result_uses_successful_replay_output() {
    let first = Arc::new(ScriptedAgent::new("placeholder", vec![retryable_error("temporary")]));
    let second = Arc::new(ScriptedAgent::new(
        "placeholder",
        vec![reply("{\"plan\":\"recovered\"}")],
    ));
    let (service, manager, _) =
        service_with_scripted_agents(vec![AgentInstance::Mock(first), AgentInstance::Mock(second)]);
    let conversation = service.create("user_1", make_create_req()).await.unwrap();
    let result = service
        .run_agent_turn(internal_request(&conversation.id))
        .await
        .unwrap();
    assert_eq!(manager.build_count(), 2);
    assert_eq!(result.status, ConversationAgentTurnStatus::Completed);
    assert_eq!(result.error_message, None);
    assert_eq!(result.final_text.as_deref(), Some("{\"plan\":\"recovered\"}"));
    assert!(result.output_complete);
}

#[tokio::test]
async fn normalized_internal_result_keeps_terminal_replay_failure() {
    let first = Arc::new(ScriptedAgent::new("placeholder", vec![retryable_error("temporary")]));
    let second = Arc::new(ScriptedAgent::new(
        "placeholder",
        vec![retryable_error("second failure")],
    ));
    let (service, manager, _) =
        service_with_scripted_agents(vec![AgentInstance::Mock(first), AgentInstance::Mock(second)]);
    let conversation = service.create("user_1", make_create_req()).await.unwrap();
    let result = service
        .run_agent_turn(internal_request(&conversation.id))
        .await
        .unwrap();
    assert_eq!(manager.build_count(), 2);
    assert_eq!(result.status, ConversationAgentTurnStatus::Failed);
    assert!(
        result
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("second failure"))
    );
    assert_eq!(result.final_text, None);
    assert!(!result.output_complete);
}

#[tokio::test]
async fn normalized_internal_result_rejects_pending_confirmation_even_with_finish() {
    let agent = Arc::new(
        ScriptedAgent::new("placeholder", vec![reply("{\"plan\":\"waiting\"}")])
            .with_confirmations(make_test_confirmations()),
    );
    let (service, _, _) = service_with_scripted_agents(vec![AgentInstance::Mock(agent)]);
    let conversation = service.create("user_1", make_create_req()).await.unwrap();
    let result = service
        .run_agent_turn(internal_request(&conversation.id))
        .await
        .unwrap();
    assert_eq!(result.status, ConversationAgentTurnStatus::Completed);
    assert_eq!(result.final_text, None);
    assert!(!result.output_complete);
}
