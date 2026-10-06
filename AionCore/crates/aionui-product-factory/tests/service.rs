use std::sync::Arc;

use aionui_api_types::{CreateProductFactoryRunRequest, ProductFactoryRunStatus};
use aionui_db::{SqliteProductFactoryRepository, init_database_memory};
use aionui_product_factory::{ProductFactoryError, ProductFactoryService};

async fn service() -> ProductFactoryService {
    let db = init_database_memory().await.expect("memory database");
    ProductFactoryService::new(Arc::new(SqliteProductFactoryRepository::new(db.pool().clone())))
}

fn request(name: &str) -> CreateProductFactoryRunRequest {
    CreateProductFactoryRunRequest {
        name: name.to_owned(),
        idea: "A focused product idea".to_owned(),
        target_user: Some("indie makers".to_owned()),
        problem: None,
        expected_output: None,
        workspace_path: Some("/tmp/product-factory".to_owned()),
        budget_usd: Some(25.0),
    }
}

#[tokio::test]
async fn creates_and_lists_runs_only_for_the_owner() {
    let service = service().await;

    let created = service.create_run("user-a", request("Alpha")).await.unwrap();
    assert_eq!(created.status, ProductFactoryRunStatus::Draft);
    assert_eq!(created.name, "Alpha");

    let owner_runs = service.list_runs("user-a").await.unwrap();
    assert_eq!(owner_runs.len(), 1);
    assert_eq!(service.list_runs("user-b").await.unwrap().len(), 0);
}

#[tokio::test]
async fn omitted_paths_receive_unique_existing_managed_folders() {
    let root = tempfile::tempdir().unwrap();
    let service = service().await.with_managed_workspace_root(root.path().into());
    let mut first = request("Alpha");
    first.workspace_path = None;
    let mut second = request("Beta");
    second.workspace_path = Some(" ".into());
    let a = service.create_run("owner-a", first).await.unwrap();
    let b = service.create_run("owner-b", second).await.unwrap();
    assert_ne!(a.workspace_path, b.workspace_path);
    for run in [a, b] {
        let folder = std::path::Path::new(&run.workspace_path);
        assert!(folder.is_absolute() && folder.is_dir());
        assert!(folder.starts_with(root.path().canonicalize().unwrap()));
        assert!(!folder.join(".tasks").exists());
    }
}

#[tokio::test]
async fn explicit_workspace_is_established_without_allocating_a_managed_folder() {
    let root = tempfile::tempdir().unwrap();
    let service = service().await.with_managed_workspace_root(root.path().into());
    let created = service.create_run("owner", request("Alpha")).await.unwrap();
    let expected = std::fs::canonicalize("/tmp").unwrap().join("product-factory");
    assert_eq!(std::path::Path::new(&created.workspace_path), expected);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn persistence_failure_removes_only_the_new_empty_workspace() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("keep.txt"), "existing user file").unwrap();
    let db = init_database_memory().await.unwrap();
    let service = ProductFactoryService::new(Arc::new(SqliteProductFactoryRepository::new(db.pool().clone())))
        .with_managed_workspace_root(root.path().into());
    db.close().await;
    let mut input = request("Alpha");
    input.workspace_path = None;
    assert!(matches!(
        service.create_run("owner", input).await,
        Err(ProductFactoryError::Database(_))
    ));
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    assert_eq!(
        std::fs::read_to_string(root.path().join("keep.txt")).unwrap(),
        "existing user file"
    );
}

#[tokio::test]
async fn transitions_follow_the_product_factory_state_machine() {
    let service = service().await;
    let created = service.create_run("user-a", request("Alpha")).await.unwrap();

    let interviewing = service
        .transition_status("user-a", &created.id, ProductFactoryRunStatus::Interviewing)
        .await
        .unwrap();
    assert_eq!(interviewing.status, ProductFactoryRunStatus::Interviewing);

    let invalid = service
        .transition_status("user-a", &created.id, ProductFactoryRunStatus::TaskDraftReady)
        .await
        .unwrap_err();
    assert!(matches!(invalid, ProductFactoryError::InvalidTransition { .. }));
}

#[tokio::test]
async fn another_user_cannot_read_or_transition_a_run() {
    let service = service().await;
    let created = service.create_run("user-a", request("Alpha")).await.unwrap();

    assert!(matches!(
        service.get_run("user-b", &created.id).await.unwrap_err(),
        ProductFactoryError::NotFound(_)
    ));
    assert!(matches!(
        service
            .transition_status("user-b", &created.id, ProductFactoryRunStatus::Interviewing)
            .await
            .unwrap_err(),
        ProductFactoryError::NotFound(_)
    ));
}

#[tokio::test]
async fn generic_status_changes_cannot_claim_execution_or_handoff_success() {
    for (from, to) in [
        (
            ProductFactoryRunStatus::TaskDraftReady,
            ProductFactoryRunStatus::HandedOff,
        ),
        (ProductFactoryRunStatus::HandedOff, ProductFactoryRunStatus::Running),
        (ProductFactoryRunStatus::Running, ProductFactoryRunStatus::InReview),
        (ProductFactoryRunStatus::InReview, ProductFactoryRunStatus::Completed),
    ] {
        let db = init_database_memory().await.unwrap();
        let repo = Arc::new(SqliteProductFactoryRepository::new(db.pool().clone()));
        let service = ProductFactoryService::new(repo.clone());
        let created = service.create_run("user-a", request("Alpha")).await.unwrap();
        let mut row = aionui_db::IProductFactoryRepository::get_run(repo.as_ref(), "user-a", &created.id)
            .await
            .unwrap()
            .unwrap();
        row.status = from.as_str().into();
        row.plan_revision += 1;
        aionui_db::IProductFactoryRepository::update_run(repo.as_ref(), &row)
            .await
            .unwrap();
        assert!(
            matches!(
                service.transition_status("user-a", &created.id, to).await,
                Err(ProductFactoryError::InvalidTransition { .. })
            ),
            "must reject {from:?} -> {to:?}"
        );
        assert_eq!(service.get_run("user-a", &created.id).await.unwrap().status, from);
    }
}
