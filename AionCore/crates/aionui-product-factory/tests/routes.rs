use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use http_body_util::BodyExt;
use tower::ServiceExt;

use aionui_auth::CurrentUser;
use aionui_db::{SqliteProductFactoryRepository, init_database_memory};
use aionui_product_factory::{ProductFactoryRouterState, ProductFactoryService, product_factory_routes};

async fn app() -> axum::Router {
    let db = init_database_memory().await.unwrap();
    let service = Arc::new(ProductFactoryService::new(Arc::new(
        SqliteProductFactoryRepository::new(db.pool().clone()),
    )));
    product_factory_routes(ProductFactoryRouterState { service }).layer(middleware::from_fn(
        |mut request: Request<Body>, next: Next| async move {
            request.extensions_mut().insert(CurrentUser::local_default());
            Ok::<Response, std::convert::Infallible>(next.run(request).await)
        },
    ))
}

#[tokio::test]
async fn creates_and_transitions_a_run_over_http() {
    let app = app().await;
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/product-factory/runs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Alpha","idea":"Build a better brief"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = json["data"]["id"].as_str().unwrap().to_owned();
    assert_eq!(json["data"]["status"], "draft");

    let response = app
        .oneshot(
            Request::post(format!("/api/product-factory/runs/{id}/status"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"interviewing"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["data"]["status"], "interviewing");
}

#[tokio::test]
async fn rejects_invalid_state_transition_over_http() {
    let app = app().await;
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/product-factory/runs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Alpha","idea":"Build a better brief"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = json["data"]["id"].as_str().unwrap().to_owned();

    let response = app
        .oneshot(
            Request::post(format!("/api/product-factory/runs/{id}/status"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"task_draft_ready"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["code"], "PRODUCT_FACTORY_INVALID_TRANSITION");
}

#[tokio::test]
async fn rejects_invalid_create_payloads_with_stable_bad_request_errors() {
    let app = app().await;
    let malformed = app
        .clone()
        .oneshot(
            Request::post("/api/product-factory/runs")
                .header("content-type", "application/json")
                .body(Body::from("{"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    let body = malformed.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["code"], "BAD_REQUEST");
    assert_eq!(json["error"], "Invalid JSON request body.");

    let invalid_budget = app
        .oneshot(
            Request::post("/api/product-factory/runs")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"Alpha","idea":"Build a better brief","budget_usd":0}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_budget.status(), StatusCode::BAD_REQUEST);
    let body = invalid_budget.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["code"], "BAD_REQUEST");
    assert_eq!(json["error"], "budget_usd must be a finite positive number");
}

#[tokio::test]
async fn persists_confirmed_interview_and_creates_blueprint_draft() {
    let app = app().await;
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/product-factory/runs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Alpha","idea":"Build a research brief"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = json["data"]["id"].as_str().unwrap().to_owned();

    let response = app
        .clone()
        .oneshot(
            Request::put(format!("/api/product-factory/runs/{id}/interview"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"interview":{"version":1,"summary":"A research brief for product teams","questions":[]}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["data"]["status"], "interviewing");

    let response = app
        .oneshot(
            Request::post(format!("/api/product-factory/runs/{id}/interview/confirm"))
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["data"]["status"], "blueprint_ready");
    assert_eq!(json["data"]["blueprint"]["generated_by"], "draft");
    assert_eq!(json["data"]["blueprint"]["confirmed"], false);
}

#[tokio::test]
async fn generates_saves_and_confirms_task_draft_without_starting_execution() {
    let app = app().await;
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/product-factory/runs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Alpha","idea":"Build a research brief"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let id = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["data"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let app = app.clone();
    let request = |method: &str, path: String, body: &'static str| {
        let mut builder = Request::builder().method(method).uri(path);
        builder = builder.header("content-type", "application/json");
        builder.body(Body::from(body)).unwrap()
    };
    let _ = app
        .clone()
        .oneshot(request(
            "PUT",
            format!("/api/product-factory/runs/{id}/interview"),
            r#"{"interview":{"version":1,"summary":"A useful brief","questions":[]}}"#,
        ))
        .await
        .unwrap();
    let _ = app
        .clone()
        .oneshot(request(
            "POST",
            format!("/api/product-factory/runs/{id}/interview/confirm"),
            "",
        ))
        .await
        .unwrap();
    let _ = app
        .clone()
        .oneshot(request(
            "POST",
            format!("/api/product-factory/runs/{id}/blueprint/confirm"),
            "",
        ))
        .await
        .unwrap();

    let response = app
        .clone()
        .oneshot(request(
            "POST",
            format!("/api/product-factory/runs/{id}/task-draft/generate"),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let generated = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(generated["data"]["status"], "task_draft_ready");
    assert_eq!(generated["data"]["task_draft"]["generated_by"], "draft");
    assert_eq!(generated["data"]["task_draft"]["confirmed"], false);
    let revision = generated["data"]["task_draft"]["revision"].as_u64().unwrap();
    let tasks = generated["data"]["task_draft"]["tasks"].clone();

    let response = app
        .clone()
        .oneshot(request(
            "PUT",
            format!("/api/product-factory/runs/{id}/task-draft"),
            Box::leak(
                serde_json::json!({"expected_revision": revision, "tasks": tasks})
                    .to_string()
                    .into_boxed_str(),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let saved = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    let next_revision = saved["data"]["task_draft"]["revision"].as_u64().unwrap();

    let response = app
        .clone()
        .oneshot(request(
            "POST",
            format!("/api/product-factory/runs/{id}/task-draft/confirm"),
            Box::leak(
                serde_json::json!({"expected_revision": next_revision})
                    .to_string()
                    .into_boxed_str(),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let confirmed = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(confirmed["data"]["status"], "task_draft_ready");
    assert_eq!(confirmed["data"]["task_draft"]["confirmed"], true);
    assert!(confirmed["data"]["team_id"].is_null());

    // The generic status endpoint must not claim that a confirmed task list
    // created a Team; only the real handoff operation may set this state.
    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/api/product-factory/runs/{id}/status"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"handed_off"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["code"], "PRODUCT_FACTORY_INVALID_TRANSITION");
    let response = app
        .oneshot(
            Request::get(format!("/api/product-factory/runs/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let unchanged: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(unchanged["data"]["status"], "task_draft_ready");
    assert!(unchanged["data"]["team_id"].is_null());
}
