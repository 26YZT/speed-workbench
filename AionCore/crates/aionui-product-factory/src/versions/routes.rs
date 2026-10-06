#![allow(clippy::disallowed_types)]

use aionui_api_types::{
    ActivateProductFactoryVersionRequest, ApiResponse, IterateProductFactoryVersionRequest,
    ProductFactorySourceManifestResponse, ProductFactoryVersionActivationResponse,
    ProductFactoryVersionIterationResponse, ProductFactoryVersionMutationResponse, ProductFactoryVersionsResponse,
    SnapshotProductFactoryVersionRequest,
};
use aionui_auth::CurrentUser;
use aionui_common::ApiError;
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};

use super::VersionError;
use crate::ProductFactoryRouterState;

pub(crate) fn version_routes() -> Router<ProductFactoryRouterState> {
    Router::new()
        .route("/api/product-factory/runs/{id}/versions", get(list))
        .route("/api/product-factory/runs/{id}/versions/source-manifest", get(source))
        .route("/api/product-factory/runs/{id}/versions/snapshot", post(snapshot))
        .route("/api/product-factory/runs/{id}/versions/iterate", post(iterate))
        .route("/api/product-factory/runs/{id}/versions/activate", post(activate))
}
async fn list(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ProductFactoryVersionsResponse>>, ApiError> {
    Ok(Json(ApiResponse::ok(
        state.service.get_versions(&user.id, &id).await.map_err(map_error)?,
    )))
}
async fn source(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ProductFactorySourceManifestResponse>>, ApiError> {
    Ok(Json(ApiResponse::ok(
        state
            .service
            .get_source_manifest(&user.id, &id)
            .await
            .map_err(map_error)?,
    )))
}
async fn snapshot(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    body: Result<Json<SnapshotProductFactoryVersionRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<ApiResponse<ProductFactoryVersionMutationResponse>>, ApiError> {
    let Json(request) = body.map_err(ApiError::from)?;
    Ok(Json(ApiResponse::ok(
        state
            .service
            .snapshot_version(&user.id, &id, request)
            .await
            .map_err(map_error)?,
    )))
}
async fn iterate(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    body: Result<Json<IterateProductFactoryVersionRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<ApiResponse<ProductFactoryVersionIterationResponse>>, ApiError> {
    let Json(request) = body.map_err(ApiError::from)?;
    Ok(Json(ApiResponse::ok(
        state
            .service
            .iterate_version(&user.id, &id, request)
            .await
            .map_err(map_error)?,
    )))
}
async fn activate(
    State(state): State<ProductFactoryRouterState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    body: Result<Json<ActivateProductFactoryVersionRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<ApiResponse<ProductFactoryVersionActivationResponse>>, ApiError> {
    let Json(request) = body.map_err(ApiError::from)?;
    Ok(Json(ApiResponse::ok(
        state
            .service
            .activate_version(&user.id, &id, request)
            .await
            .map_err(map_error)?,
    )))
}
fn map_error(error: VersionError) -> ApiError {
    match error {
        VersionError::Invalid(code) => ApiError::coded(
            StatusCode::BAD_REQUEST,
            code,
            "Code version input or file boundary is invalid",
            None,
        ),
        VersionError::Conflict(code) | VersionError::Blocked(code) => ApiError::coded(
            StatusCode::CONFLICT,
            code,
            "Code version changed or cannot be selected in this state",
            None,
        ),
        VersionError::Unavailable(code) => ApiError::coded(
            if code == "PRODUCT_FACTORY_VERSION_PLATFORM_UNSUPPORTED" {
                StatusCode::NOT_IMPLEMENTED
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            },
            code,
            "Code version operation is unavailable",
            None,
        ),
        VersionError::NotFound => ApiError::coded(
            StatusCode::NOT_FOUND,
            "PRODUCT_FACTORY_VERSION_NOT_FOUND",
            "Owned code version or run was not found",
            None,
        ),
        VersionError::Database(error) => {
            tracing::warn!(%error,"code version storage operation failed");
            ApiError::coded(
                StatusCode::SERVICE_UNAVAILABLE,
                "PRODUCT_FACTORY_VERSION_STORAGE_UNAVAILABLE",
                "Code version storage is unavailable",
                None,
            )
        }
    }
}
