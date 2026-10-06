#![cfg(any(target_os = "macos", target_os = "linux"))]
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aionui_api_types::{
    ActivateProductFactoryVersionRequest, CreateProductFactoryRunRequest, IterateProductFactoryVersionRequest,
    ProductFactorySnapshotFileRequest, SnapshotProductFactoryVersionRequest,
};
use aionui_db::{
    IProductFactoryRepository, IProductFactoryVersionsRepository, SqliteProductFactoryRepository,
    SqliteProductFactoryVersionsRepository,
};
use aionui_product_factory::{ProductFactoryService, VersionError};
use sha2::{Digest, Sha256};

struct IdleFixture;
#[async_trait::async_trait]
impl aionui_product_factory::ProductFactoryVersionActivityPort for IdleFixture {
    async fn check_idle(&self, _: &str, _: &str) -> aionui_product_factory::VersionResult<()> {
        Ok(())
    }
}

struct Fixture {
    _root: tempfile::TempDir,
    db: aionui_db::Database,
    service: ProductFactoryService,
    repository: Arc<SqliteProductFactoryRepository>,
    versions: Arc<SqliteProductFactoryVersionsRepository>,
    run_id: String,
    workspace: PathBuf,
    snapshots: PathBuf,
}
impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("source");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::write(
            workspace.join("START.md"),
            "Run python3 server.py; runtime data stays separate.",
        )
        .unwrap();
        std::fs::write(
            workspace.join("ACCEPTANCE.md"),
            "Unit fixture for completed code tasks; this is not customer acceptance.",
        )
        .unwrap();
        std::fs::write(workspace.join("server.py"), "print('sealed original')\n").unwrap();
        std::fs::write(
            workspace.join("history.sqlite"),
            b"SQLite format 3\0fixture-business-data",
        )
        .unwrap();
        std::fs::write(workspace.join("history.json"), "{\"private_history\":true}").unwrap();
        std::fs::write(workspace.join(".env"), "API_KEY=fixture-only-private-value").unwrap();
        std::fs::write(
            workspace.join("settings.json"),
            "{\"api_key\":\"fixture_private_value\"}",
        )
        .unwrap();
        let db = aionui_db::init_database_memory().await.unwrap();
        sqlx::query("INSERT INTO users (id,username,password_hash,created_at,updated_at) VALUES ('u','u','fixture-hash',0,0),('foreign','foreign','fixture-hash',0,0)").execute(db.pool()).await.unwrap();
        let repository = Arc::new(SqliteProductFactoryRepository::new(db.pool().clone()));
        let versions = Arc::new(SqliteProductFactoryVersionsRepository::new(db.pool().clone()));
        let snapshots = root.path().join("snapshots");
        let service = ProductFactoryService::new(repository.clone())
            .with_managed_workspace_root(root.path().join("runs"))
            .with_versioning(versions.clone(), snapshots.clone())
            .with_version_activity_port(Arc::new(IdleFixture));
        let run = service
            .create_run(
                "u",
                CreateProductFactoryRunRequest {
                    name: "Version fixture".into(),
                    idea: "Local code lineage fixture".into(),
                    target_user: None,
                    problem: None,
                    expected_output: None,
                    workspace_path: Some(workspace.to_string_lossy().into_owned()),
                    budget_usd: Some(10.0),
                },
            )
            .await
            .unwrap();
        let fixture = Self {
            _root: root,
            db,
            service,
            repository,
            versions,
            run_id: run.id,
            workspace,
            snapshots,
        };
        fixture.complete(&fixture.run_id).await;
        sqlx::query("INSERT INTO agent_usage (id,user_id,task_id,team_id,model,input_tokens,output_tokens,cached_read_tokens,cached_write_tokens,cost_est,cost_unknown_reason,conversation_id,turn_id,attempt_id,created_at) VALUES ('old-fee','u',?,?,'fixture-model',100,20,0,0,NULL,'model_price_missing','fixture-conv','fixture-turn','fixture-attempt',1)").bind(format!("team-{}-build",fixture.run_id)).bind(format!("team-{}",fixture.run_id)).execute(fixture.db.pool()).await.unwrap();
        fixture
    }
    async fn complete(&self, run: &str) {
        let team = format!("team-{run}");
        sqlx::query(
            "INSERT INTO teams (id,user_id,name,workspace,created_at,updated_at) VALUES (?,'u','fixture team',?,1,1)",
        )
        .bind(&team)
        .bind(self.repository.get_run("u", run).await.unwrap().unwrap().workspace_path)
        .execute(self.db.pool())
        .await
        .unwrap();
        for task in ["build", "test"] {
            sqlx::query("INSERT INTO team_tasks (id,team_id,subject,status,created_at,updated_at) VALUES (?,?,'completed fixture','completed',1,1)")
                .bind(format!("{team}-{task}")).bind(&team).execute(self.db.pool()).await.unwrap();
        }
        let draft = serde_json::json!({"version":1,"generated_by":"draft","confirmed":true,"revision":1,"updated_at":1,"tasks":[
            {"id":"build","title":"Build fixture","description":"Code fixture","type":"backend","blocked_by":[],"acceptance_criteria":["Checked"],"suggested_role":"developer","effort":"low"},
            {"id":"test","title":"Test fixture","description":"Integration fixture","type":"test","blocked_by":["build"],"acceptance_criteria":["Checked"],"suggested_role":"tester","effort":"low"}
        ]});
        sqlx::query(
            "UPDATE product_factory_runs SET status='completed',team_id=?,task_draft_json=? WHERE id=? AND user_id='u'",
        )
        .bind(&team)
        .bind(draft.to_string())
        .bind(run)
        .execute(self.db.pool())
        .await
        .unwrap();
        sqlx::query("INSERT INTO product_factory_execution (run_id,user_id,team_id,task_id,state,message_id,team_run_id,requested_at,updated_at) VALUES (?,'u',?,?,'enqueued','fixture-msg','fixture-run',1,1)")
            .bind(run).bind(&team).bind(format!("{team}-test")).execute(self.db.pool()).await.unwrap();
    }
    async fn request(&self, run: &str, key: &str) -> SnapshotProductFactoryVersionRequest {
        let response = self.service.get_source_manifest("u", run).await.unwrap();
        SnapshotProductFactoryVersionRequest {
            expected_plan_revision: response.plan_revision,
            idempotency_key: key.into(),
            files: response
                .manifest
                .files
                .into_iter()
                .map(|file| ProductFactorySnapshotFileRequest {
                    path: file.path,
                    sha256: file.sha256,
                })
                .collect(),
        }
    }
    async fn seal(&self) -> aionui_api_types::ProductFactoryVersionMutationResponse {
        self.service
            .snapshot_version("u", &self.run_id, self.request(&self.run_id, "seal-v1").await)
            .await
            .unwrap()
    }
    async fn iterate(
        &self,
        source: &str,
        revision: i64,
        key: &str,
    ) -> aionui_api_types::ProductFactoryVersionIterationResponse {
        self.service
            .iterate_version(
                "u",
                &self.run_id,
                IterateProductFactoryVersionRequest {
                    source_version_id: source.into(),
                    expected_product_revision: revision,
                    idempotency_key: key.into(),
                    change_request: "Add a second local feature".into(),
                    budget_usd: Some(2.0),
                },
            )
            .await
            .unwrap()
    }
}
fn digest(path: &Path) -> String {
    format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
}

#[tokio::test]
async fn legacy_read_is_pure_and_completed_code_seals_without_pricing_or_runtime_data() {
    let fixture = Fixture::new().await;
    let before = digest(&fixture.workspace.join("history.sqlite"));
    let list = fixture.service.get_versions("u", &fixture.run_id).await.unwrap();
    assert!(list.product.is_none());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_factory_products")
        .fetch_one(fixture.db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
    let candidate = fixture.service.get_source_manifest("u", &fixture.run_id).await.unwrap();
    assert_eq!(candidate.manifest.files.len(), 3);
    assert_eq!(candidate.excluded_count, 4);
    let sealed = fixture.seal().await;
    assert_eq!(sealed.version.state, "sealed");
    assert_eq!(sealed.version.version_no, 1);
    assert!(sealed.product.active_version_id.is_none());
    let snapshot = PathBuf::from(sealed.version.snapshot_path.unwrap());
    assert_ne!(snapshot, fixture.workspace);
    assert_eq!(
        std::fs::read(snapshot.join("server.py")).unwrap(),
        std::fs::read(fixture.workspace.join("server.py")).unwrap()
    );
    for denied in ["history.sqlite", "history.json", ".env", "settings.json"] {
        assert!(!snapshot.join(denied).exists());
    }
    assert_eq!(before, digest(&fixture.workspace.join("history.sqlite")));
    let ledger: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_usage")
        .fetch_one(fixture.db.pool())
        .await
        .unwrap();
    assert_eq!(
        ledger, 1,
        "sealing must retain the old fee and not create a fake zero-cost model report"
    );
    let fee: (i64, Option<f64>) = sqlx::query_as("SELECT input_tokens,cost_est FROM agent_usage WHERE id='old-fee'")
        .fetch_one(fixture.db.pool())
        .await
        .unwrap();
    assert_eq!(fee, (100, None));
}

#[tokio::test]
async fn snapshot_double_window_is_idempotent_and_same_key_different_payload_conflicts() {
    let fixture = Fixture::new().await;
    let a = fixture.request(&fixture.run_id, "same-key").await;
    let b = fixture.request(&fixture.run_id, "same-key").await;
    let (a, b) = tokio::join!(
        fixture.service.snapshot_version("u", &fixture.run_id, a),
        fixture.service.snapshot_version("u", &fixture.run_id, b)
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.version.id, b.version.id);
    assert_eq!(a.operation.id, b.operation.id);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_factory_versions")
        .fetch_one(fixture.db.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
    let mut changed = fixture.request(&fixture.run_id, "same-key").await;
    changed.expected_plan_revision += 1;
    assert!(matches!(
        fixture.service.snapshot_version("u", &fixture.run_id, changed).await,
        Err(VersionError::Conflict("PRODUCT_FACTORY_VERSION_IDEMPOTENCY_CONFLICT"))
    ));
}

#[tokio::test]
async fn independent_iteration_resets_confirmation_and_fees_and_active_selection_uses_cas() {
    let fixture = Fixture::new().await;
    let sealed = fixture.seal().await;
    let source = sealed.version.id.clone();
    let original = digest(&fixture.workspace.join("history.sqlite"));
    let active = fixture
        .service
        .activate_version(
            "u",
            &fixture.run_id,
            ActivateProductFactoryVersionRequest {
                version_id: source.clone(),
                expected_product_revision: sealed.product.revision,
            },
        )
        .await
        .unwrap();
    let iteration = fixture.iterate(&source, active.product.revision, "branch-a").await;
    assert_eq!(iteration.version.version_no, 2);
    assert_eq!(
        iteration.version.parent_run_id.as_deref(),
        Some(fixture.run_id.as_str())
    );
    assert_eq!(iteration.version.parent_version_id.as_deref(), Some(source.as_str()));
    assert_eq!(iteration.version.state, "working");
    assert_eq!(iteration.run.status, aionui_api_types::ProductFactoryRunStatus::Draft);
    assert!(iteration.run.team_id.is_none());
    assert!(iteration.run.interview.is_none());
    assert!(iteration.run.blueprint.is_none());
    assert!(iteration.run.task_draft.is_none());
    assert_eq!(iteration.run.budget_usd, Some(2.0));
    let workspace = PathBuf::from(&iteration.run.workspace_path);
    assert_ne!(workspace, fixture.workspace);
    assert!(!workspace.join("history.sqlite").exists());
    assert_eq!(original, digest(&fixture.workspace.join("history.sqlite")));
    assert!(matches!(
        fixture
            .service
            .activate_version(
                "u",
                &fixture.run_id,
                ActivateProductFactoryVersionRequest {
                    version_id: iteration.version.id.clone(),
                    expected_product_revision: iteration.product.revision
                }
            )
            .await,
        Err(VersionError::Blocked("PRODUCT_FACTORY_VERSION_NOT_SEALED"))
    ));
    assert!(matches!(
        fixture
            .service
            .activate_version(
                "u",
                &fixture.run_id,
                ActivateProductFactoryVersionRequest {
                    version_id: source.clone(),
                    expected_product_revision: active.product.revision
                }
            )
            .await,
        Err(VersionError::Conflict("PRODUCT_FACTORY_VERSION_REVISION_CONFLICT"))
    ));
    let replay = fixture.iterate(&source, active.product.revision, "branch-a").await;
    assert_eq!(replay.run.id, iteration.run.id);
    let next = fixture
        .iterate(&source, replay.product.revision, "branch-from-v1-again")
        .await;
    assert_eq!(next.version.version_no, 3);
    assert_eq!(next.version.parent_version_id.as_deref(), Some(source.as_str()));
}

#[tokio::test]
async fn source_workspace_changes_do_not_change_sealed_code_and_working_version_seals_in_place() {
    let fixture = Fixture::new().await;
    let sealed = fixture.seal().await;
    let snapshot = PathBuf::from(sealed.version.snapshot_path.clone().unwrap());
    let hash = digest(&snapshot.join("server.py"));
    std::fs::write(fixture.workspace.join("server.py"), "print('external source change')\n").unwrap();
    assert_eq!(hash, digest(&snapshot.join("server.py")));
    let iteration = fixture
        .iterate(&sealed.version.id, sealed.product.revision, "independent")
        .await;
    assert_eq!(
        hash,
        digest(&Path::new(&iteration.run.workspace_path).join("server.py"))
    );
    fixture.complete(&iteration.run.id).await;
    let sealed_two = fixture
        .service
        .snapshot_version(
            "u",
            &iteration.run.id,
            fixture.request(&iteration.run.id, "seal-v2").await,
        )
        .await
        .unwrap();
    assert_eq!(sealed_two.version.id, iteration.version.id);
    assert_eq!(sealed_two.version.version_no, 2);
    assert_eq!(sealed_two.version.state, "sealed");
    let active = fixture
        .service
        .activate_version(
            "u",
            &fixture.run_id,
            ActivateProductFactoryVersionRequest {
                version_id: sealed.version.id,
                expected_product_revision: sealed_two.product.revision,
            },
        )
        .await
        .unwrap();
    assert_eq!(active.workspace_path, snapshot.to_string_lossy());
}

#[tokio::test]
async fn completion_docs_hash_paths_and_symlinks_are_verified() {
    let fixture = Fixture::new().await;
    sqlx::query("UPDATE team_tasks SET status='pending' WHERE id=?")
        .bind(format!("team-{}-build", fixture.run_id))
        .execute(fixture.db.pool())
        .await
        .unwrap();
    assert!(matches!(
        fixture
            .service
            .snapshot_version(
                "u",
                &fixture.run_id,
                fixture.request(&fixture.run_id, "unfinished").await
            )
            .await,
        Err(VersionError::Blocked("PRODUCT_FACTORY_VERSION_SNAPSHOT_INCOMPLETE"))
    ));
    sqlx::query("UPDATE team_tasks SET status='completed'")
        .execute(fixture.db.pool())
        .await
        .unwrap();
    let mut request = fixture.request(&fixture.run_id, "stale").await;
    request
        .files
        .iter_mut()
        .find(|file| file.path == "server.py")
        .unwrap()
        .sha256 = "0".repeat(64);
    assert!(matches!(
        fixture.service.snapshot_version("u", &fixture.run_id, request).await,
        Err(VersionError::Conflict("PRODUCT_FACTORY_VERSION_STALE_HASH"))
    ));
    let mut request = fixture.request(&fixture.run_id, "escape").await;
    request.files[0].path = "../outside.py".into();
    assert!(matches!(
        fixture.service.snapshot_version("u", &fixture.run_id, request).await,
        Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_INVALID_PATH"))
    ));
    let mut request = fixture.request(&fixture.run_id, "missing-doc").await;
    request.files.retain(|file| file.path != "ACCEPTANCE.md");
    assert!(matches!(
        fixture.service.snapshot_version("u", &fixture.run_id, request).await,
        Err(VersionError::Blocked("PRODUCT_FACTORY_VERSION_SNAPSHOT_INCOMPLETE"))
    ));
    std::os::unix::fs::symlink(fixture.workspace.join("server.py"), fixture.workspace.join("alias.py")).unwrap();
    assert!(matches!(
        fixture.service.get_source_manifest("u", &fixture.run_id).await,
        Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_SYMLINK_NOT_ALLOWED"))
    ));
}

#[tokio::test]
async fn foreign_owner_cannot_read_snapshot_iterate_or_activate() {
    let fixture = Fixture::new().await;
    let request = fixture.request(&fixture.run_id, "foreign").await;
    assert!(matches!(
        fixture.service.get_versions("foreign", &fixture.run_id).await,
        Err(VersionError::NotFound)
    ));
    assert!(matches!(
        fixture.service.get_source_manifest("foreign", &fixture.run_id).await,
        Err(VersionError::NotFound)
    ));
    assert!(matches!(
        fixture
            .service
            .snapshot_version("foreign", &fixture.run_id, request)
            .await,
        Err(VersionError::NotFound)
    ));
    let sealed = fixture.seal().await;
    assert!(matches!(
        fixture
            .service
            .activate_version(
                "foreign",
                &fixture.run_id,
                ActivateProductFactoryVersionRequest {
                    version_id: sealed.version.id.clone(),
                    expected_product_revision: sealed.product.revision
                }
            )
            .await,
        Err(VersionError::NotFound)
    ));
    assert!(matches!(
        fixture
            .service
            .iterate_version(
                "foreign",
                &fixture.run_id,
                IterateProductFactoryVersionRequest {
                    source_version_id: sealed.version.id,
                    expected_product_revision: sealed.product.revision,
                    idempotency_key: "foreign".into(),
                    change_request: "Change".into(),
                    budget_usd: None
                }
            )
            .await,
        Err(VersionError::NotFound)
    ));
}

#[tokio::test]
async fn tampered_sealed_code_cannot_be_selected_or_cloned() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new().await;
    let sealed = fixture.seal().await;
    let path = PathBuf::from(sealed.version.snapshot_path.unwrap()).join("server.py");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(&path, "print('tampered')").unwrap();
    assert!(matches!(
        fixture
            .service
            .activate_version(
                "u",
                &fixture.run_id,
                ActivateProductFactoryVersionRequest {
                    version_id: sealed.version.id.clone(),
                    expected_product_revision: sealed.product.revision
                }
            )
            .await,
        Err(VersionError::Conflict("PRODUCT_FACTORY_VERSION_STALE_HASH"))
    ));
    assert!(matches!(
        fixture
            .service
            .iterate_version(
                "u",
                &fixture.run_id,
                IterateProductFactoryVersionRequest {
                    source_version_id: sealed.version.id,
                    expected_product_revision: sealed.product.revision,
                    idempotency_key: "tampered".into(),
                    change_request: "Change".into(),
                    budget_usd: None
                }
            )
            .await,
        Err(VersionError::Conflict("PRODUCT_FACTORY_VERSION_STALE_HASH"))
    ));
}

#[tokio::test]
async fn existing_destination_is_retained_and_failed_clone_remains_blocked_after_restart() {
    let fixture = Fixture::new().await;
    let sealed = fixture.seal().await;
    let managed = fixture._root.path().join("runs");
    std::fs::write(&managed, "user file that must survive").unwrap();
    assert!(matches!(
        fixture
            .service
            .iterate_version(
                "u",
                &fixture.run_id,
                IterateProductFactoryVersionRequest {
                    source_version_id: sealed.version.id.clone(),
                    expected_product_revision: sealed.product.revision,
                    idempotency_key: "blocked-copy".into(),
                    change_request: "Change".into(),
                    budget_usd: None
                }
            )
            .await,
        Err(VersionError::Unavailable("PRODUCT_FACTORY_VERSION_COPY_FAILED"))
    ));
    assert_eq!(
        std::fs::read_to_string(&managed).unwrap(),
        "user file that must survive"
    );
    let operations = fixture.versions.list_operations("u", &sealed.product.id).await.unwrap();
    let operation = operations
        .iter()
        .find(|op| op.idempotency_key == "blocked-copy")
        .unwrap();
    assert_eq!(operation.state, "failed");
    let version = fixture
        .versions
        .get_version("u", &operation.version_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(version.state, "failed");
    let restored = ProductFactoryService::new(fixture.repository.clone())
        .with_managed_workspace_root(managed)
        .with_versioning(fixture.versions.clone(), fixture.snapshots.clone())
        .with_version_activity_port(Arc::new(IdleFixture));
    assert!(restored.save_interview("u",&version.run_id,serde_json::json!({"version":1,"generated_by":"draft","confirmed":false,"summary":"fixture","questions":[]})).await.is_err(),"a failed copy may not start preparation");
    let replay = restored
        .iterate_version(
            "u",
            &fixture.run_id,
            IterateProductFactoryVersionRequest {
                source_version_id: sealed.version.id,
                expected_product_revision: sealed.product.revision,
                idempotency_key: "blocked-copy".into(),
                change_request: "Change".into(),
                budget_usd: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(replay.version.state, "failed");
    assert_eq!(replay.run.id, version.run_id);
}

#[tokio::test]
async fn interrupted_snapshot_is_read_back_and_explicit_new_key_preserves_old_copy_directory() {
    use aionui_db::{ProductFactoryVersionOperationRow, SnapshotVersionReservation};
    let fixture = Fixture::new().await;
    let request = fixture.request(&fixture.run_id, "interrupted").await;
    let candidate = fixture.service.get_source_manifest("u", &fixture.run_id).await.unwrap();
    let run = fixture.repository.get_run("u", &fixture.run_id).await.unwrap().unwrap();
    let product = aionui_common::generate_id();
    let version = aionui_common::generate_id();
    let operation = aionui_common::generate_id();
    let hash=format!("{:x}",Sha256::digest(serde_json::json!({"kind":"snapshot","expected_plan_revision":request.expected_plan_revision,"files":request.files}).to_string().as_bytes()));
    let receipt = ProductFactoryVersionOperationRow {
        id: operation.clone(),
        user_id: "u".into(),
        source_run_id: fixture.run_id.clone(),
        version_id: version.clone(),
        idempotency_key: "interrupted".into(),
        input_hash: hash,
        kind: "snapshot".into(),
        state: "reserved".into(),
        error_code: None,
        created_at: 1,
        updated_at: 1,
    };
    let target = fixture.snapshots.join(&product).join(&version).join(&operation);
    let manifest = serde_json::to_string(&candidate.manifest).unwrap();
    let tasks = vec![
        format!("team-{}-build", fixture.run_id),
        format!("team-{}-test", fixture.run_id),
    ];
    fixture
        .versions
        .reserve_snapshot(SnapshotVersionReservation {
            run: &run,
            product_id: &product,
            version_id: &version,
            operation: &receipt,
            manifest_json: &manifest,
            snapshot_path: target.to_str().unwrap(),
            completed_task_ids: &tasks,
        })
        .await
        .unwrap();
    fixture.versions.claim_copy("u", &operation, 2).await.unwrap();
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("partial.txt"), "retained interrupted bytes").unwrap();
    let restored = ProductFactoryService::new(fixture.repository.clone())
        .with_managed_workspace_root(fixture._root.path().join("runs"))
        .with_versioning(fixture.versions.clone(), fixture.snapshots.clone())
        .with_version_activity_port(Arc::new(IdleFixture));
    let replay = restored
        .snapshot_version(
            "u",
            &fixture.run_id,
            fixture.request(&fixture.run_id, "interrupted").await,
        )
        .await
        .unwrap();
    assert_eq!(replay.operation.id, operation);
    assert_eq!(replay.operation.state, "copying");
    assert_eq!(replay.version.state, "copying");
    assert_eq!(
        std::fs::read_to_string(target.join("partial.txt")).unwrap(),
        "retained interrupted bytes"
    );
    let retry = restored
        .snapshot_version(
            "u",
            &fixture.run_id,
            fixture.request(&fixture.run_id, "explicit-new-key").await,
        )
        .await
        .unwrap();
    assert_eq!(retry.version.id, version);
    assert_eq!(retry.version.state, "sealed");
    assert_ne!(retry.version.snapshot_path.as_deref(), target.to_str());
    assert_eq!(
        std::fs::read_to_string(target.join("partial.txt")).unwrap(),
        "retained interrupted bytes"
    );
    assert!(
        !fixture
            .versions
            .finish_copy("u", &operation, true, None, 3)
            .await
            .unwrap(),
        "a superseded callback cannot seal or overwrite the new snapshot"
    );
    let previous = fixture
        .versions
        .get_operation_by_key("u", &fixture.run_id, "interrupted")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(previous.state, "failed");
    assert_eq!(
        previous.error_code.as_deref(),
        Some("PRODUCT_FACTORY_VERSION_COPY_SUPERSEDED")
    );
}

#[tokio::test]
async fn fifo_hardlink_sensitive_content_and_size_limit_are_rejected_before_copy() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let fixture = Fixture::new().await;
    let mut request = fixture.request(&fixture.run_id, "sensitive").await;
    let sensitive = fixture.workspace.join("config.json");
    std::fs::write(&sensitive, "{\"accessToken\":\"fixture_private_secret\"}").unwrap();
    request.files.push(ProductFactorySnapshotFileRequest {
        path: "config.json".into(),
        sha256: digest(&sensitive),
    });
    assert!(matches!(
        fixture.service.snapshot_version("u", &fixture.run_id, request).await,
        Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_CREDENTIAL_FILE"))
    ));
    let hardlink = fixture.workspace.join("hardlink.py");
    std::fs::hard_link(fixture.workspace.join("server.py"), &hardlink).unwrap();
    assert!(matches!(
        fixture.service.get_source_manifest("u", &fixture.run_id).await,
        Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_FILE_NOT_ALLOWED"))
    ));
    std::fs::remove_file(&hardlink).unwrap();
    let fifo = fixture.workspace.join("pipe.py");
    let fifo_c = CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo_c.as_ptr(), 0o600) }, 0);
    let request = SnapshotProductFactoryVersionRequest {
        expected_plan_revision: 1,
        idempotency_key: "fifo".into(),
        files: vec![ProductFactorySnapshotFileRequest {
            path: "pipe.py".into(),
            sha256: "0".repeat(64),
        }],
    };
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        fixture.service.snapshot_version("u", &fixture.run_id, request),
    )
    .await
    .expect("a special file must not block descriptor open");
    assert!(matches!(
        result,
        Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_FILE_NOT_ALLOWED"))
    ));
    std::fs::remove_file(&fifo).unwrap();
    let large = fixture.workspace.join("large.py");
    std::fs::File::create(&large)
        .unwrap()
        .set_len(8 * 1024 * 1024 + 1)
        .unwrap();
    assert!(matches!(
        fixture.service.get_source_manifest("u", &fixture.run_id).await,
        Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_FILE_LIMIT"))
    ));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_factory_versions")
        .fetch_one(fixture.db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn team_version_guard_keeps_ordinary_and_unregistered_teams_open_but_freezes_sealed_calls() {
    let fixture = Fixture::new().await;
    let team = format!("team-{}", fixture.run_id);
    fixture
        .service
        .ensure_team_version_mutable("u", "ordinary-team-without-factory-binding")
        .await
        .unwrap();
    fixture.service.ensure_team_version_mutable("u", &team).await.unwrap();
    fixture.seal().await;
    assert!(
        matches!(fixture.service.ensure_team_version_mutable("u",&team).await,Err(aionui_product_factory::ProductFactoryError::ExecutionBlocked(message)) if message.contains("code version"))
    );
    fixture
        .service
        .ensure_team_version_mutable("u", "ordinary-team-without-factory-binding")
        .await
        .unwrap();
    fixture
        .service
        .ensure_team_version_mutable("foreign", &team)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_usage")
        .fetch_one(fixture.db.pool())
        .await
        .unwrap();
    assert_eq!(count, 1, "a rejected guard must not mint or change model receipts");
}

#[tokio::test]
async fn manifest_keeps_explicit_static_examples_but_rejects_runtime_json_and_bare_yaml_secrets() {
    let fixture = Fixture::new().await;
    std::fs::create_dir_all(fixture.workspace.join("examples")).unwrap();
    std::fs::create_dir_all(fixture.workspace.join("tests/fixtures")).unwrap();
    std::fs::write(
        fixture.workspace.join("examples/expenses.csv"),
        "date,category,amount\n2026-10-06,fixture,1.25\n",
    )
    .unwrap();
    std::fs::write(
        fixture.workspace.join("examples/history.json"),
        "{\"expected_fixture\":true}",
    )
    .unwrap();
    std::fs::write(
        fixture.workspace.join("tests/fixtures/transactions.json"),
        "{\"synthetic_fixture\":true}",
    )
    .unwrap();
    std::fs::write(
        fixture.workspace.join("transactions.json"),
        "{\"runtime_transaction\":true}",
    )
    .unwrap();
    std::fs::write(
        fixture.workspace.join("app.yml"),
        "api_key: ${API_KEY}\nclient_secret: YOUR_CLIENT_SECRET\n",
    )
    .unwrap();
    let manifest = fixture
        .service
        .get_source_manifest("u", &fixture.run_id)
        .await
        .unwrap()
        .manifest;
    for expected in [
        "examples/expenses.csv",
        "examples/history.json",
        "tests/fixtures/transactions.json",
        "app.yml",
    ] {
        assert!(
            manifest.files.iter().any(|file| file.path == expected),
            "static source asset {expected}"
        );
    }
    assert!(!manifest.files.iter().any(|file| file.path == "transactions.json"));
    assert!(!manifest.files.iter().any(|file| file.path == "history.json"));
    let mut request = fixture.request(&fixture.run_id, "yaml-secret").await;
    std::fs::write(
        fixture.workspace.join("app.yml"),
        "api_key: live-not-a-real-token-123456789\n",
    )
    .unwrap();
    request
        .files
        .iter_mut()
        .find(|file| file.path == "app.yml")
        .unwrap()
        .sha256 = digest(&fixture.workspace.join("app.yml"));
    assert!(matches!(
        fixture.service.snapshot_version("u", &fixture.run_id, request).await,
        Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_CREDENTIAL_FILE"))
    ));
    std::fs::write(
        fixture.workspace.join("app.yml"),
        "api_key: ${API_KEY}\nclient_secret: YOUR_CLIENT_SECRET\n",
    )
    .unwrap();
    let sealed = fixture
        .service
        .snapshot_version(
            "u",
            &fixture.run_id,
            fixture.request(&fixture.run_id, "static-fixtures").await,
        )
        .await
        .unwrap();
    let snapshot = PathBuf::from(sealed.version.snapshot_path.unwrap());
    assert!(snapshot.join("examples/expenses.csv").is_file());
    assert!(snapshot.join("examples/history.json").is_file());
    assert!(!snapshot.join("transactions.json").exists());
}

struct BusyFixture {
    calls: std::sync::atomic::AtomicUsize,
    after_copy: bool,
}
#[async_trait::async_trait]
impl aionui_product_factory::ProductFactoryVersionActivityPort for BusyFixture {
    async fn check_idle(&self, _: &str, _: &str) -> aionui_product_factory::VersionResult<()> {
        let call = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if !self.after_copy || call > 0 {
            Err(VersionError::Blocked("PRODUCT_FACTORY_VERSION_RUNTIME_BUSY"))
        } else {
            Ok(())
        }
    }
}
#[tokio::test]
async fn idle_is_verified_before_reservation_and_again_before_sealing_and_missing_port_fails_closed() {
    let fixture = Fixture::new().await;
    let base = ProductFactoryService::new(fixture.repository.clone())
        .with_managed_workspace_root(fixture._root.path().join("runs"))
        .with_versioning(fixture.versions.clone(), fixture.snapshots.clone());
    assert!(matches!(
        base.snapshot_version(
            "u",
            &fixture.run_id,
            fixture.request(&fixture.run_id, "no-activity-port").await
        )
        .await,
        Err(VersionError::Unavailable("PRODUCT_FACTORY_VERSION_RUNTIME_UNAVAILABLE"))
    ));
    let busy = base.clone().with_version_activity_port(Arc::new(BusyFixture {
        calls: Default::default(),
        after_copy: false,
    }));
    assert!(matches!(
        busy.snapshot_version(
            "u",
            &fixture.run_id,
            fixture.request(&fixture.run_id, "busy-before-copy").await
        )
        .await,
        Err(VersionError::Blocked("PRODUCT_FACTORY_VERSION_RUNTIME_BUSY"))
    ));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_factory_versions")
        .fetch_one(fixture.db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert!(!fixture.snapshots.exists());
    let busy_after = base.with_version_activity_port(Arc::new(BusyFixture {
        calls: Default::default(),
        after_copy: true,
    }));
    assert!(matches!(
        busy_after
            .snapshot_version(
                "u",
                &fixture.run_id,
                fixture.request(&fixture.run_id, "busy-after-copy").await
            )
            .await,
        Err(VersionError::Blocked("PRODUCT_FACTORY_VERSION_RUNTIME_BUSY"))
    ));
    let version = fixture
        .versions
        .get_by_run("u", &fixture.run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(version.state, "failed");
    assert!(PathBuf::from(version.snapshot_path.unwrap()).join("START.md").is_file());
    assert!(
        fixture
            .versions
            .get_product("u", &version.product_id)
            .await
            .unwrap()
            .unwrap()
            .active_version_id
            .is_none()
    );
}

#[tokio::test]
async fn source_root_ancestor_symlink_changed_after_establishment_is_not_followed() {
    let fixture = Fixture::new().await;
    let base = fixture._root.path().join("mutable-source-parent");
    let selected = base.join("run");
    std::fs::create_dir_all(&selected).unwrap();
    for (name, content) in [
        ("START.md", "Source guide"),
        ("ACCEPTANCE.md", "Internal source fixture"),
        ("app.py", "print('source')"),
    ] {
        std::fs::write(selected.join(name), content).unwrap();
    }
    let run = fixture
        .service
        .create_run(
            "u",
            CreateProductFactoryRunRequest {
                name: "Root boundary".into(),
                idea: "Root ancestor fixture".into(),
                target_user: None,
                problem: None,
                expected_output: None,
                workspace_path: Some(selected.to_string_lossy().into_owned()),
                budget_usd: None,
            },
        )
        .await
        .unwrap();
    fixture.complete(&run.id).await;
    let retained = fixture._root.path().join("retained-source-parent");
    std::fs::rename(&base, &retained).unwrap();
    let outside = fixture._root.path().join("outside-source-parent");
    std::fs::create_dir_all(outside.join("run")).unwrap();
    std::fs::write(
        outside.join("run/private.py"),
        "outside bytes must not be read into manifest",
    )
    .unwrap();
    std::os::unix::fs::symlink(&outside, &base).unwrap();
    assert!(matches!(
        fixture.service.get_source_manifest("u", &run.id).await,
        Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_SYMLINK_NOT_ALLOWED"))
    ));
    assert_eq!(
        std::fs::read_to_string(retained.join("run/app.py")).unwrap(),
        "print('source')"
    );
    assert_eq!(
        std::fs::read_to_string(outside.join("run/private.py")).unwrap(),
        "outside bytes must not be read into manifest"
    );
}

#[tokio::test]
async fn target_root_ancestor_symlink_changed_after_di_is_not_followed_and_outside_is_untouched() {
    let fixture = Fixture::new().await;
    let base = fixture._root.path().join("mutable-target-parent");
    std::fs::create_dir(&base).unwrap();
    let source_hash = digest(&fixture.workspace.join("server.py"));
    let service = ProductFactoryService::new(fixture.repository.clone())
        .with_managed_workspace_root(fixture._root.path().join("runs"))
        .with_versioning(fixture.versions.clone(), base.join("snapshots"))
        .with_version_activity_port(Arc::new(IdleFixture));
    let retained = fixture._root.path().join("retained-target-parent");
    std::fs::rename(&base, &retained).unwrap();
    let outside = fixture._root.path().join("outside-target-parent");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("keep.txt"), "outside user bytes").unwrap();
    std::os::unix::fs::symlink(&outside, &base).unwrap();
    assert!(matches!(
        service
            .snapshot_version(
                "u",
                &fixture.run_id,
                fixture.request(&fixture.run_id, "target-root-link").await
            )
            .await,
        Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_SYMLINK_NOT_ALLOWED"))
    ));
    assert_eq!(
        std::fs::read_to_string(outside.join("keep.txt")).unwrap(),
        "outside user bytes"
    );
    assert!(!outside.join("snapshots").exists());
    assert!(!retained.join("snapshots").exists());
    assert_eq!(source_hash, digest(&fixture.workspace.join("server.py")));
}
