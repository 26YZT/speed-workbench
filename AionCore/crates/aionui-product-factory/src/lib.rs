#![warn(clippy::disallowed_types)]

mod delivery;
mod execution;
mod handoff;
mod planning;
mod planning_cost;
mod planning_validation;
mod pricing;
mod review;
mod routes;
mod service;
mod task_draft;
mod versions;

pub use execution::{ProductFactoryDispatchReceipt, ProductFactoryExecutionPort};
pub use handoff::ProductFactoryTeamPort;
pub use planning::{
    ProductFactoryPlanningInvocation, ProductFactoryPlanningOutcome, ProductFactoryPlanningPort,
    ProductFactoryPlanningPrepared, ProductFactoryPlanningStarted,
};
pub use planning_cost::{ProductFactoryPlanningCosts, read_planning_costs};
pub use pricing::ProductFactoryRepricingPort;
pub use review::{ProductFactoryReviewPort, ProductFactoryTaskReviewSnapshot};
pub use routes::{ProductFactoryRouterState, product_factory_routes};
pub use service::{ProductFactoryError, ProductFactoryService};
pub use task_draft::{TaskDraftValidationError, generate_task_draft, is_integration_task, validate_task_draft};

pub use versions::{ProductFactoryVersionActivityPort, VersionError, VersionResult};
