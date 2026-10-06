use aionui_db::models::{AgentUsageCost, AgentUsageRow};
use aionui_db::{IAgentUsageRepository, SqliteAgentUsageRepository, init_database_memory};

#[tokio::test]
async fn explicit_repricing_preserves_legacy_and_known_costs_and_captures_source() {
    let db = init_database_memory().await.unwrap();
    let repo = SqliteAgentUsageRepository::new(db.pool().clone());
    for (id, cache, cost) in [
        ("measured", Some(80), None),
        ("legacy", None, None),
        ("known", Some(80), Some(0.9)),
    ] {
        repo.record_usage(&AgentUsageRow {
            id: id.into(),
            user_id: "u".into(),
            task_id: Some("t".into()),
            agent_id: None,
            model: Some("test-model".into()),
            input_tokens: 100,
            output_tokens: 50,
            cost_est: cost,
            cached_read_tokens: cache,
            cached_write_tokens: cache.map(|_| 0),
            cost_source: cost.map(|_| "provider_report".into()),
            pricing_snapshot: None,
            cost_unknown_reason: cost.is_none().then(|| "model_price_missing".into()),
            conversation_id: format!("c-{id}"),
            turn_id: format!("turn-{id}"),
            created_at: 0,
            attempt_id: "legacy".into(),
            team_id: None,
        })
        .await
        .unwrap();
    }
    let updated = repo
        .backfill_unknown_costs(&|row| {
            Some(AgentUsageCost {
                cost_usd: ((row.input_tokens - row.cached_read_tokens.unwrap()) as f64 * 2.0
                    + row.cached_read_tokens.unwrap() as f64 * 0.1
                    + row.output_tokens as f64 * 4.0)
                    / 1_000_000.0,
                source: "configured_estimate".into(),
                pricing_snapshot: r#"{"source":"fixture prices"}"#.into(),
            })
        })
        .await
        .unwrap();
    assert_eq!(updated, 1);
    let rows = repo.list_by_task("u", "t").await.unwrap();
    let measured = rows.iter().find(|row| row.id == "measured").unwrap();
    assert_eq!(measured.cost_est, Some(0.000248));
    assert_eq!(measured.cost_source.as_deref(), Some("configured_estimate"));
    assert_eq!(
        measured.pricing_snapshot.as_deref(),
        Some(r#"{"source":"fixture prices"}"#)
    );
    assert_eq!(measured.cost_unknown_reason, None);
    let legacy = rows.iter().find(|row| row.id == "legacy").unwrap();
    assert_eq!(legacy.cost_est, None);
    assert_eq!(legacy.cached_read_tokens, None);
    let known = rows.iter().find(|row| row.id == "known").unwrap();
    assert_eq!(known.cost_est, Some(0.9));
    assert_eq!(known.cost_source.as_deref(), Some("provider_report"));
}

#[tokio::test]
async fn repricing_cannot_overwrite_known_cost_or_price_a_stale_token_snapshot() {
    let db = init_database_memory().await.unwrap();
    let repo = SqliteAgentUsageRepository::new(db.pool().clone());
    let mut row = AgentUsageRow {
        id: "row".into(),
        user_id: "owner".into(),
        task_id: Some("t".into()),
        agent_id: None,
        model: Some("test-model".into()),
        input_tokens: 100,
        output_tokens: 10,
        cost_est: None,
        cached_read_tokens: Some(80),
        cached_write_tokens: Some(0),
        cost_source: None,
        pricing_snapshot: None,
        cost_unknown_reason: Some("model_price_missing".into()),
        conversation_id: "c".into(),
        turn_id: "turn".into(),
        created_at: 1,
        attempt_id: "legacy".into(),
        team_id: None,
    };
    repo.record_usage(&row).await.unwrap();
    let stale = row.clone();
    row.input_tokens = 200;
    repo.record_usage(&row).await.unwrap();
    let quote = AgentUsageCost {
        cost_usd: 0.1,
        source: "configured_estimate".into(),
        pricing_snapshot: "{}".into(),
    };
    assert!(!repo.apply_cost_estimate(&stale, &quote).await.unwrap());
    let mut foreign = row.clone();
    foreign.user_id = "other".into();
    assert!(!repo.apply_cost_estimate(&foreign, &quote).await.unwrap());
    assert!(repo.apply_cost_estimate(&row, &quote).await.unwrap());
    let second = AgentUsageCost { cost_usd: 9.0, ..quote };
    assert!(!repo.apply_cost_estimate(&row, &second).await.unwrap());
    let saved = repo.list_by_task("owner", "t").await.unwrap();
    assert_eq!(saved[0].cost_est, Some(0.1));
    assert_eq!(saved[0].input_tokens, 200);
}

#[tokio::test]
async fn admitted_turn_without_report_is_unknown_and_repeated_admission_preserves_real_usage() {
    let db = init_database_memory().await.unwrap();
    let repo = SqliteAgentUsageRepository::new(db.pool().clone());
    let mut row = AgentUsageRow {
        id: "admission".into(),
        user_id: "u".into(),
        task_id: Some("t".into()),
        agent_id: None,
        model: Some("m".into()),
        input_tokens: 0,
        output_tokens: 0,
        cost_est: None,
        cached_read_tokens: None,
        cached_write_tokens: None,
        cost_source: None,
        pricing_snapshot: None,
        cost_unknown_reason: None,
        conversation_id: "c".into(),
        turn_id: "turn".into(),
        created_at: 1,
        attempt_id: "legacy".into(),
        team_id: None,
    };
    repo.ensure_turn_admission(&row).await.unwrap();
    let admitted = repo.list_by_task("u", "t").await.unwrap();
    assert_eq!(admitted[0].cost_est, None);
    assert_eq!(admitted[0].cost_unknown_reason.as_deref(), Some("usage_not_reported"));
    row.input_tokens = 100;
    row.cost_est = Some(0.1);
    row.cached_read_tokens = Some(80);
    row.cached_write_tokens = Some(0);
    row.cost_unknown_reason = None;
    repo.record_usage(&row).await.unwrap();
    repo.ensure_turn_admission(&row).await.unwrap();
    let saved = repo.list_by_task("u", "t").await.unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].cost_est, Some(0.1));
    assert_eq!(saved[0].input_tokens, 100);
}

#[tokio::test]
async fn billing_uncertainty_preserves_measurements_and_cannot_be_repriced_from_a_stale_snapshot() {
    let db = init_database_memory().await.unwrap();
    let repo = SqliteAgentUsageRepository::new(db.pool().clone());
    let mut row = AgentUsageRow {
        id: "measured".into(),
        user_id: "owner".into(),
        task_id: Some("t".into()),
        team_id: Some("team".into()),
        agent_id: None,
        model: Some("model".into()),
        input_tokens: 100,
        output_tokens: 10,
        cached_read_tokens: Some(80),
        cached_write_tokens: Some(5),
        cost_est: None,
        cost_source: None,
        pricing_snapshot: None,
        cost_unknown_reason: Some("model_price_missing".into()),
        conversation_id: "c".into(),
        turn_id: "turn".into(),
        attempt_id: "a".into(),
        created_at: 1,
    };
    repo.record_usage(&row).await.unwrap();
    let stale = row.clone();
    repo.invalidate_billing_attempts("other-user", "c", &["a".into()])
        .await
        .unwrap();
    assert_eq!(repo.list_by_task("owner", "t").await.unwrap()[0], stale);
    repo.invalidate_billing_attempts("owner", "other-conversation", &["a".into()])
        .await
        .unwrap();
    assert_eq!(repo.list_by_task("owner", "t").await.unwrap()[0], stale);
    repo.invalidate_billing_attempts("owner", "c", &["a".into()])
        .await
        .unwrap();
    let quote = AgentUsageCost {
        cost_usd: 1.0,
        source: "configured_estimate".into(),
        pricing_snapshot: "{}".into(),
    };
    assert!(
        !repo.apply_cost_estimate(&stale, &quote).await.unwrap(),
        "same buckets with a newly invalid reason cannot pass CAS"
    );
    let invalid = repo.list_by_task("owner", "t").await.unwrap().remove(0);
    assert!(!repo.apply_cost_estimate(&invalid, &quote).await.unwrap());
    row.input_tokens = 0;
    row.output_tokens = 0;
    row.cached_read_tokens = Some(0);
    row.cached_write_tokens = None;
    row.cost_unknown_reason = Some("billing_baseline_missing_or_reset".into());
    repo.record_usage(&row).await.unwrap();
    let saved = repo.list_by_task("owner", "t").await.unwrap().remove(0);
    assert_eq!(
        (
            saved.input_tokens,
            saved.output_tokens,
            saved.cached_read_tokens,
            saved.cached_write_tokens
        ),
        (100, 10, Some(80), Some(5))
    );
    assert_eq!(saved.cost_est, None);
    assert_eq!(
        saved.cost_unknown_reason.as_deref(),
        Some("billing_attribution_uncertain")
    );
    row.cost_est = Some(2.0);
    row.cost_source = Some("provider_report".into());
    row.cost_unknown_reason = None;
    repo.record_usage(&row).await.unwrap();
    assert_eq!(
        repo.list_by_task("owner", "t").await.unwrap()[0].cost_est,
        None,
        "a later unverified report cannot silently clear cumulative uncertainty"
    );
}

#[tokio::test]
async fn invalidating_one_attempt_keeps_other_conversations_users_and_attempts_known() {
    let db = init_database_memory().await.unwrap();
    let repo = SqliteAgentUsageRepository::new(db.pool().clone());
    for (id, user, conversation, attempt) in [
        ("affected", "owner", "c", "a"),
        ("foreign", "other", "c", "a"),
        ("other-conv", "owner", "other", "a"),
        ("other-attempt", "owner", "c", "b"),
    ] {
        repo.record_usage(&AgentUsageRow {
            id: id.into(),
            user_id: user.into(),
            task_id: Some("t".into()),
            team_id: None,
            agent_id: None,
            model: Some("m".into()),
            input_tokens: 100,
            output_tokens: 10,
            cached_read_tokens: Some(80),
            cached_write_tokens: Some(0),
            cost_est: Some(0.1),
            cost_source: Some("provider_report".into()),
            pricing_snapshot: None,
            cost_unknown_reason: None,
            conversation_id: conversation.into(),
            turn_id: format!("turn-{id}"),
            attempt_id: attempt.into(),
            created_at: 1,
        })
        .await
        .unwrap();
    }
    repo.invalidate_billing_attempts("owner", "c", &["a".into()])
        .await
        .unwrap();
    let rows = repo.list_by_task("owner", "t").await.unwrap();
    assert_eq!(rows.iter().find(|row| row.id == "affected").unwrap().cost_est, None);
    for id in ["other-conv", "other-attempt"] {
        assert_eq!(rows.iter().find(|row| row.id == id).unwrap().cost_est, Some(0.1));
    }
    assert_eq!(repo.list_by_task("other", "t").await.unwrap()[0].cost_est, Some(0.1));
}

#[tokio::test]
async fn only_an_untouched_owned_common_admission_can_be_settled_as_not_sent() {
    let db = init_database_memory().await.unwrap();
    let repo = SqliteAgentUsageRepository::new(db.pool().clone());
    let row = AgentUsageRow {
        id: "common".into(),
        user_id: "owner".into(),
        task_id: Some("t".into()),
        team_id: Some("team".into()),
        agent_id: None,
        model: None,
        input_tokens: 0,
        output_tokens: 0,
        cached_read_tokens: None,
        cached_write_tokens: None,
        cost_est: None,
        cost_source: None,
        pricing_snapshot: None,
        cost_unknown_reason: Some("usage_not_reported".into()),
        conversation_id: "c".into(),
        turn_id: "turn".into(),
        attempt_id: "attempt".into(),
        created_at: 1,
    };
    repo.ensure_turn_admission(&row).await.unwrap();
    assert!(!repo.mark_admission_not_sent("other", "c", "attempt").await.unwrap());
    assert!(!repo.mark_admission_not_sent("owner", "other", "attempt").await.unwrap());
    assert!(!repo.mark_admission_not_sent("owner", "c", "other").await.unwrap());
    assert!(repo.mark_admission_not_sent("owner", "c", "attempt").await.unwrap());
    assert!(!repo.mark_admission_not_sent("owner", "c", "attempt").await.unwrap());
    let settled = repo.list_by_task("owner", "t").await.unwrap().remove(0);
    assert_eq!(settled.id, row.id);
    assert_eq!(settled.attempt_id, row.attempt_id);
    assert_eq!(settled.cost_est, Some(0.0));
    assert_eq!(settled.cost_source.as_deref(), Some("not_sent"));
    assert_eq!(
        (settled.cached_read_tokens, settled.cached_write_tokens),
        (Some(0), Some(0))
    );
    assert_eq!(settled.cost_unknown_reason, None);
    for (id, model, agent, input, output, cache, cost, reason) in [
        ("provider", None, None, 0, 0, None, Some(0.5), None),
        ("measured", None, None, 100, 0, None, None, Some("usage_not_reported")),
        ("output", None, None, 0, 1, None, None, Some("usage_not_reported")),
        (
            "runtime-model",
            Some("m"),
            None,
            0,
            0,
            None,
            None,
            Some("usage_not_reported"),
        ),
        (
            "runtime-agent",
            None,
            Some("agent"),
            0,
            0,
            None,
            None,
            Some("usage_not_reported"),
        ),
        ("cache", None, None, 0, 0, Some(0), None, Some("usage_not_reported")),
        (
            "uncertain",
            None,
            None,
            0,
            0,
            None,
            None,
            Some("billing_attribution_uncertain"),
        ),
    ] {
        let mut captured = row.clone();
        captured.id = id.into();
        captured.turn_id = id.into();
        captured.attempt_id = id.into();
        captured.model = model.map(str::to_owned);
        captured.agent_id = agent.map(str::to_owned);
        captured.input_tokens = input;
        captured.output_tokens = output;
        captured.cached_read_tokens = cache;
        captured.cost_est = cost;
        captured.cost_unknown_reason = reason.map(str::to_owned);
        repo.record_usage(&captured).await.unwrap();
        assert!(
            !repo.mark_admission_not_sent("owner", "c", id).await.unwrap(),
            "must preserve actual or uncertain report {id}"
        );
        assert_eq!(
            repo.list_by_task("owner", "t")
                .await
                .unwrap()
                .into_iter()
                .find(|entry| entry.id == id)
                .unwrap(),
            captured
        );
    }
    for id in ["legacy-one", "legacy-two"] {
        let mut ambiguous = row.clone();
        ambiguous.id = id.into();
        ambiguous.turn_id = id.into();
        ambiguous.attempt_id = "legacy".into();
        repo.ensure_turn_admission(&ambiguous).await.unwrap();
    }
    assert!(
        !repo.mark_admission_not_sent("owner", "c", "legacy").await.unwrap(),
        "ambiguous legacy attempts must not be modified"
    );
    assert!(
        repo.list_by_task("owner", "t")
            .await
            .unwrap()
            .iter()
            .filter(|row| row.attempt_id == "legacy")
            .all(|row| row.cost_est.is_none())
    );
}
