#![allow(clippy::disallowed_types)]

use std::sync::Arc;

use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Json, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};

use aionui_api_types::{
    ApiResponse, ApplyProductFactoryPlanningRequest, CancelProductFactoryPlanningRequest, ConfirmTaskDraftRequest,
    CreateProductFactoryRunRequest, HandoffProductFactoryRequest, ProductFactoryCostReportResponse,
    ProductFactoryDeliveryResponse, ProductFactoryExecutionResponse, ProductFactoryHandoffResponse,
    ProductFactoryPlanningResponse, ProductFactoryRepriceResponse, ProductFactoryRunResponse,
    ProductFactoryTaskReviewResponse, RepriceProductFactoryRequest, SaveProductFactoryBlueprintRequest,
    SaveProductFactoryInterviewRequest, SaveTaskDraftRequest, StartProductFactoryPlanningRequest,
    StartProductFactoryRequest, TeamTaskReviewRequest, TransitionProductFactoryRunRequest,
};
use aionui_auth::CurrentUser;
use aionui_common::ApiError;

use crate::service::{ProductFactoryError, ProductFactoryService};

#[derive(Clone)]
pub struct ProductFactoryRouterState {
    pub service: Arc<ProductFactoryService>,
}

pub fn product_factory_routes(state: ProductFactoryRouterState) -> Router {
    Router::new()
        .route("/api/product-factory/runs", get(list_runs).post(create_run))
        .route("/api/product-factory/runs/{id}", get(get_run))
        .route(
            "/api/product-factory/runs/{id}/planning",
            get(list_planning).post(start_planning),
        )
        .route(
            "/api/product-factory/runs/{id}/planning/{attempt_id}",
            get(get_planning),
        )
        .route(
            "/api/product-factory/runs/{id}/planning/{attempt_id}/apply",
            post(apply_planning),
        )
        .route(
            "/api/product-factory/runs/{id}/planning/{attempt_id}/cancel",
            post(cancel_planning),
        )
        .route("/api/product-factory/runs/{id}/status", post(transition_status))
        .route("/api/product-factory/runs/{id}/handoff", post(handoff))
        .route("/api/product-factory/runs/{id}/execution", get(get_execution))
        .route("/api/product-factory/runs/{id}/start", post(start_run))
        .route("/api/product-factory/runs/{id}/continue", post(continue_run))
        .route("/api/product-factory/runs/{id}/delivery", get(get_delivery))
        .route("/api/product-factory/runs/{id}/cost-report", get(get_cost_report))
        .route(
            "/api/product-factory/runs/{id}/cost-report/reprice",
            post(reprice_costs),
        )
        .route("/api/product-factory/runs/{id}/review", get(get_review).post(review))
        .route("/api/product-factory/runs/{id}/interview", put(save_interview))
        .route(
            "/api/product-factory/runs/{id}/interview/confirm",
            post(confirm_interview),
        )
        .route("/api/product-factory/runs/{id}/blueprint", put(save_blueprint))
        .route(
            "/api/product-factory/runs/{id}/blueprint/confirm",
            post(confirm_blueprint),
        )
        .route(
            "/api/product-factory/runs/{id}/task-draft/generate",
            post(generate_task_draft),
        )
        .route("/api/product-factory/runs/{id}/task-draft", put(save_task_draft))
        .route(
            "/api/product-factory/runs/{id}/task-draft/confirm",
            post(confirm_task_draft),
        )
        .merge(crate::versions::routes::version_routes())
        .with_state(state)
}

async fn start_planning(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    body: Result<Json<StartProductFactoryPlanningRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<ApiResponse<ProductFactoryPlanningResponse>>), ApiError> {
    let Json(request) = body.map_err(ApiError::from)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(ApiResponse::ok(
            state
                .service
                .start_planning(&user.id, &id, request)
                .await
                .map_err(map_error)?,
        )),
    ))
}
async fn list_planning(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vec<ProductFactoryPlanningResponse>>>, ApiError> {
    Ok(Json(ApiResponse::ok(
        state.service.list_planning(&user.id, &id).await.map_err(map_error)?,
    )))
}
async fn get_planning(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path((id, attempt_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<ProductFactoryPlanningResponse>>, ApiError> {
    Ok(Json(ApiResponse::ok(
        state
            .service
            .get_planning(&user.id, &id, &attempt_id)
            .await
            .map_err(map_error)?,
    )))
}
async fn apply_planning(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path((id, attempt_id)): Path<(String, String)>,
    body: Result<Json<ApplyProductFactoryPlanningRequest>, JsonRejection>,
) -> Result<Json<ApiResponse<ProductFactoryRunResponse>>, ApiError> {
    let Json(request) = body.map_err(ApiError::from)?;
    Ok(Json(ApiResponse::ok(
        state
            .service
            .apply_planning(&user.id, &id, &attempt_id, request)
            .await
            .map_err(map_error)?,
    )))
}
async fn cancel_planning(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path((id, attempt_id)): Path<(String, String)>,
    body: Result<Json<CancelProductFactoryPlanningRequest>, JsonRejection>,
) -> Result<Json<ApiResponse<ProductFactoryPlanningResponse>>, ApiError> {
    let Json(_) = body.map_err(ApiError::from)?;
    Ok(Json(ApiResponse::ok(
        state
            .service
            .cancel_planning(&user.id, &id, &attempt_id)
            .await
            .map_err(map_error)?,
    )))
}
async fn create_run(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    body: Result<Json<CreateProductFactoryRunRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<ApiResponse<ProductFactoryRunResponse>>), ApiError> {
    let Json(request) = body.map_err(ApiError::from)?;
    let run = state.service.create_run(&user.id, request).await.map_err(map_error)?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(run))))
}

async fn handoff(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    body: Result<Json<HandoffProductFactoryRequest>, JsonRejection>,
) -> Result<Json<ApiResponse<ProductFactoryHandoffResponse>>, ApiError> {
    let Json(request) = body.map_err(ApiError::from)?;
    let result = state.service.handoff(&user.id, &id, request).await.map_err(map_error)?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn get_execution(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ProductFactoryExecutionResponse>>, ApiError> {
    Ok(Json(ApiResponse::ok(
        state.service.get_execution(&user.id, &id).await.map_err(map_error)?,
    )))
}

async fn start_run(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    body: Result<Json<StartProductFactoryRequest>, JsonRejection>,
) -> Result<Json<ApiResponse<ProductFactoryExecutionResponse>>, ApiError> {
    let Json(request) = body.map_err(ApiError::from)?;
    Ok(Json(ApiResponse::ok(
        state
            .service
            .start_run(&user.id, &id, request)
            .await
            .map_err(map_error)?,
    )))
}

async fn continue_run(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ProductFactoryExecutionResponse>>, ApiError> {
    Ok(Json(ApiResponse::ok(
        state.service.continue_run(&user.id, &id).await.map_err(map_error)?,
    )))
}

async fn get_delivery(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ProductFactoryDeliveryResponse>>, ApiError> {
    Ok(Json(ApiResponse::ok(
        state.service.get_delivery(&user.id, &id).await.map_err(map_error)?,
    )))
}

async fn reprice_costs(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    body: Result<Json<RepriceProductFactoryRequest>, JsonRejection>,
) -> Result<Json<ApiResponse<ProductFactoryRepriceResponse>>, ApiError> {
    let Json(_) = body.map_err(ApiError::from)?;
    Ok(Json(ApiResponse::ok(
        state.service.reprice_costs(&user.id, &id).await.map_err(map_error)?,
    )))
}

async fn get_cost_report(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ProductFactoryCostReportResponse>>, ApiError> {
    Ok(Json(ApiResponse::ok(
        state.service.get_cost_report(&user.id, &id).await.map_err(map_error)?,
    )))
}

async fn get_review(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ProductFactoryTaskReviewResponse>>, ApiError> {
    Ok(Json(ApiResponse::ok(
        state.service.get_review(&user.id, &id).await.map_err(map_error)?,
    )))
}

async fn review(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    body: Result<Json<TeamTaskReviewRequest>, JsonRejection>,
) -> Result<Json<ApiResponse<ProductFactoryTaskReviewResponse>>, ApiError> {
    let Json(request) = body.map_err(ApiError::from)?;
    Ok(Json(ApiResponse::ok(
        state.service.review(&user.id, &id, request).await.map_err(map_error)?,
    )))
}

async fn list_runs(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
) -> Result<Json<ApiResponse<Vec<ProductFactoryRunResponse>>>, ApiError> {
    let runs = state.service.list_runs(&user.id).await.map_err(map_error)?;
    Ok(Json(ApiResponse::ok(runs)))
}

async fn get_run(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ProductFactoryRunResponse>>, ApiError> {
    let run = state.service.get_run(&user.id, &id).await.map_err(map_error)?;
    Ok(Json(ApiResponse::ok(run)))
}

async fn transition_status(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    body: Result<Json<TransitionProductFactoryRunRequest>, JsonRejection>,
) -> Result<Json<ApiResponse<ProductFactoryRunResponse>>, ApiError> {
    let Json(request) = body.map_err(ApiError::from)?;
    let run = state
        .service
        .transition_status(&user.id, &id, request.status)
        .await
        .map_err(map_error)?;
    Ok(Json(ApiResponse::ok(run)))
}

async fn save_interview(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    body: Result<Json<SaveProductFactoryInterviewRequest>, JsonRejection>,
) -> Result<Json<ApiResponse<ProductFactoryRunResponse>>, ApiError> {
    let Json(request) = body.map_err(ApiError::from)?;
    let run = state
        .service
        .save_interview(&user.id, &id, request.interview)
        .await
        .map_err(map_error)?;
    Ok(Json(ApiResponse::ok(run)))
}

async fn confirm_interview(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ProductFactoryRunResponse>>, ApiError> {
    let run = state
        .service
        .confirm_interview(&user.id, &id)
        .await
        .map_err(map_error)?;
    Ok(Json(ApiResponse::ok(run)))
}

async fn save_blueprint(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    body: Result<Json<SaveProductFactoryBlueprintRequest>, JsonRejection>,
) -> Result<Json<ApiResponse<ProductFactoryRunResponse>>, ApiError> {
    let Json(request) = body.map_err(ApiError::from)?;
    let run = state
        .service
        .save_blueprint(&user.id, &id, request.blueprint)
        .await
        .map_err(map_error)?;
    Ok(Json(ApiResponse::ok(run)))
}

async fn confirm_blueprint(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ProductFactoryRunResponse>>, ApiError> {
    let run = state
        .service
        .confirm_blueprint(&user.id, &id)
        .await
        .map_err(map_error)?;
    Ok(Json(ApiResponse::ok(run)))
}

async fn generate_task_draft(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ProductFactoryRunResponse>>, ApiError> {
    let run = state
        .service
        .generate_task_draft(&user.id, &id)
        .await
        .map_err(map_error)?;
    Ok(Json(ApiResponse::ok(run)))
}

async fn save_task_draft(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    body: Result<Json<SaveTaskDraftRequest>, JsonRejection>,
) -> Result<Json<ApiResponse<ProductFactoryRunResponse>>, ApiError> {
    let Json(request) = body.map_err(ApiError::from)?;
    let run = state
        .service
        .save_task_draft(&user.id, &id, request)
        .await
        .map_err(map_error)?;
    Ok(Json(ApiResponse::ok(run)))
}

async fn confirm_task_draft(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    body: Result<Json<ConfirmTaskDraftRequest>, JsonRejection>,
) -> Result<Json<ApiResponse<ProductFactoryRunResponse>>, ApiError> {
    let Json(request) = body.map_err(ApiError::from)?;
    let run = state
        .service
        .confirm_task_draft(&user.id, &id, request)
        .await
        .map_err(map_error)?;
    Ok(Json(ApiResponse::ok(run)))
}

fn map_error(error: ProductFactoryError) -> ApiError {
    match error {
        ProductFactoryError::PlanningBlocked(code) => ApiError::coded(
            StatusCode::CONFLICT,
            planning_api_code(&code),
            "Product Factory planning is blocked",
            None,
        ),
        ProductFactoryError::PlanningFailed(code) => ApiError::coded(
            StatusCode::SERVICE_UNAVAILABLE,
            planning_api_code(&code),
            "Product Factory planning is unavailable",
            None,
        ),
        ProductFactoryError::ExecutionBlocked(message) => {
            ApiError::coded(StatusCode::CONFLICT, "PRODUCT_FACTORY_EXECUTION_BLOCKED", message, None)
        }
        ProductFactoryError::ExecutionCostUnknown => ApiError::coded(
            StatusCode::CONFLICT,
            "PRODUCT_FACTORY_COST_UNKNOWN",
            "Team cost cannot be verified while a budget is configured",
            None,
        ),
        ProductFactoryError::ExecutionDispatchFailed => ApiError::coded(
            StatusCode::SERVICE_UNAVAILABLE,
            "PRODUCT_FACTORY_EXECUTION_UNCERTAIN",
            "Inspect Team before attempting further work",
            None,
        ),
        ProductFactoryError::ReviewBlocked(message) => {
            ApiError::coded(StatusCode::CONFLICT, "PRODUCT_FACTORY_REVIEW_BLOCKED", message, None)
        }
        ProductFactoryError::ReviewFailed(message) => {
            tracing::warn!(error = %message, "product factory review adapter failed");
            ApiError::coded(
                StatusCode::SERVICE_UNAVAILABLE,
                "PRODUCT_FACTORY_REVIEW_FAILED",
                "Review data is unavailable",
                None,
            )
        }
        ProductFactoryError::RevisionConflict => ApiError::coded(
            StatusCode::CONFLICT,
            "PRODUCT_FACTORY_REVISION_CONFLICT",
            "Task draft revision changed",
            None,
        ),
        ProductFactoryError::InvalidHandoff(message) => {
            ApiError::coded(StatusCode::CONFLICT, "PRODUCT_FACTORY_HANDOFF_BLOCKED", message, None)
        }
        ProductFactoryError::TeamPreparation(message) => {
            tracing::warn!(error = %message, "product factory Team preparation failed");
            ApiError::coded(
                StatusCode::SERVICE_UNAVAILABLE,
                "PRODUCT_FACTORY_TEAM_PREPARATION_FAILED",
                "Selected Team assistants could not be prepared",
                None,
            )
        }
        ProductFactoryError::NotFound(id) => ApiError::NotFound(format!("Product Factory run not found: {id}")),
        ProductFactoryError::InvalidRequest(message) => ApiError::BadRequest(message),
        ProductFactoryError::InvalidTransition { from, to } => ApiError::coded(
            StatusCode::CONFLICT,
            "PRODUCT_FACTORY_INVALID_TRANSITION",
            format!("Cannot transition Product Factory run from {from:?} to {to:?}"),
            Some(serde_json::json!({ "from": from, "to": to })),
        ),
        ProductFactoryError::Database(error) => match error {
            aionui_db::DbError::NotFound(id) => ApiError::NotFound(id),
            aionui_db::DbError::Conflict(message) => {
                ApiError::coded(StatusCode::CONFLICT, "PRODUCT_FACTORY_REVISION_CONFLICT", message, None)
            }
            other => {
                tracing::error!(error = %other, "product factory database operation failed");
                ApiError::Internal("Product Factory service failed".to_owned())
            }
        },
    }
}

fn planning_api_code(code: &str) -> &'static str {
    match code {
        "PLANNING_ALREADY_APPLIED" => "PLANNING_ALREADY_APPLIED",
        "PLANNING_ASSISTANT_SNAPSHOT_INVALID" => "PLANNING_ASSISTANT_SNAPSHOT_INVALID",
        "PLANNING_ASSISTANT_SNAPSHOT_MISSING" => "PLANNING_ASSISTANT_SNAPSHOT_MISSING",
        "PLANNING_ASSISTANT_UNAVAILABLE" => "PLANNING_ASSISTANT_UNAVAILABLE",
        "PLANNING_ATTEMPT_ALREADY_ACTIVE" => "PLANNING_ATTEMPT_ALREADY_ACTIVE",
        "PLANNING_ATTEMPT_NOT_RUNNING" => "PLANNING_ATTEMPT_NOT_RUNNING",
        "PLANNING_BUDGET_BLOCKED" => "PLANNING_BUDGET_BLOCKED",
        "PLANNING_BUDGET_EXHAUSTED" => "PLANNING_BUDGET_EXHAUSTED",
        "PLANNING_CANCELLATION_UNCONFIRMED" => "PLANNING_CANCELLATION_UNCONFIRMED",
        "PLANNING_CANDIDATE_NOT_READY" => "PLANNING_CANDIDATE_NOT_READY",
        "PLANNING_CONVERSATION_NOT_BOUND" => "PLANNING_CONVERSATION_NOT_BOUND",
        "PLANNING_COST_UNKNOWN" => "PLANNING_COST_UNKNOWN",
        "PLANNING_IDEMPOTENCY_CONFLICT" => "PLANNING_IDEMPOTENCY_CONFLICT",
        "PLANNING_INPUT_CHANGED" => "PLANNING_INPUT_CHANGED",
        "PLANNING_INVALID_JSON" => "PLANNING_INVALID_JSON",
        "PLANNING_INVALID_PHASE" => "PLANNING_INVALID_PHASE",
        "PLANNING_INVALID_SCHEMA" => "PLANNING_INVALID_SCHEMA",
        "PLANNING_INVALID_TASK_GRAPH" => "PLANNING_INVALID_TASK_GRAPH",
        "PLANNING_INVOCATION_UNCERTAIN" => "PLANNING_INVOCATION_UNCERTAIN",
        "PLANNING_OUTPUT_INCOMPLETE" => "PLANNING_OUTPUT_INCOMPLETE",
        "PLANNING_PHASE_NOT_EDITABLE" => "PLANNING_PHASE_NOT_EDITABLE",
        "PLANNING_PREPARATION_FAILED" => "PLANNING_PREPARATION_FAILED",
        "PLANNING_RECEIPT_LOST" => "PLANNING_RECEIPT_LOST",
        "PLANNING_RESULT_INVALID" => "PLANNING_RESULT_INVALID",
        "PLANNING_RESULT_MISSING" => "PLANNING_RESULT_MISSING",
        "PLANNING_RUNTIME_OUTCOME_UNCERTAIN" => "PLANNING_RUNTIME_OUTCOME_UNCERTAIN",
        "PLANNING_UNAVAILABLE" => "PLANNING_UNAVAILABLE",
        "PLANNING_UNCERTAIN_ATTEMPT" => "PLANNING_UNCERTAIN_ATTEMPT",
        "PLANNING_USAGE_OUT_OF_RANGE" => "PLANNING_USAGE_OUT_OF_RANGE",
        "PLANNING_USAGE_UNAVAILABLE" => "PLANNING_USAGE_UNAVAILABLE",
        "PLANNING_WORKSPACE_UNAVAILABLE" => "PLANNING_WORKSPACE_UNAVAILABLE",
        "USER_MODEL_SEND_ADMISSION_REJECTED" => "USER_MODEL_SEND_ADMISSION_REJECTED",
        "PLANNING_APP_TURN_NOT_BOUND" => "PLANNING_APP_TURN_NOT_BOUND",
        _ => "PRODUCT_FACTORY_PLANNING_FAILED",
    }
}
