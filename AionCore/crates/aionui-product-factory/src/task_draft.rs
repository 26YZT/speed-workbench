use std::collections::{HashMap, HashSet};

use aionui_api_types::{TaskDraftArtifact, TaskDraftEffort, TaskDraftExecutionScope, TaskDraftTask, TaskDraftType};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TaskDraftValidationError {
    #[error("task draft must contain at least one task")]
    Empty,
    #[error("task id is invalid: {0}")]
    InvalidId(String),
    #[error("task id is duplicated: {0}")]
    DuplicateId(String),
    #[error("task dependency does not exist: {0}")]
    MissingDependency(String),
    #[error("task cannot depend on itself: {0}")]
    SelfDependency(String),
    #[error("task dependency is duplicated: {0}")]
    DuplicateDependency(String),
    #[error("task dependency graph contains a cycle")]
    Cycle,
    #[error("task title is required: {0}")]
    EmptyTitle(String),
    #[error("task description is required: {0}")]
    EmptyDescription(String),
    #[error("task acceptance criteria is required: {0}")]
    EmptyAcceptanceCriteria(String),
    #[error("task draft contains too many tasks")]
    TooManyTasks,
    #[error("unsupported task schema or generation source")]
    UnsupportedSchema,
    #[error("invalid execution scope or requirement references: {0}")]
    InvalidScope(String),
    #[error("v2 requires one terminal integration task depending on every other task")]
    InvalidIntegration,
}

pub fn validate_task_draft(draft: &TaskDraftArtifact, require_complete: bool) -> Result<(), TaskDraftValidationError> {
    if !matches!(
        (draft.version, draft.generated_by.as_str()),
        (1, "draft") | (2, "model")
    ) {
        return Err(TaskDraftValidationError::UnsupportedSchema);
    }
    if draft.tasks.is_empty() && require_complete {
        return Err(TaskDraftValidationError::Empty);
    }
    if draft.tasks.len() > 100 {
        return Err(TaskDraftValidationError::TooManyTasks);
    }

    let mut ids = HashSet::with_capacity(draft.tasks.len());
    for task in &draft.tasks {
        if task.id.is_empty()
            || task.id.len() > 64
            || !task
                .id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        {
            return Err(TaskDraftValidationError::InvalidId(task.id.clone()));
        }
        if !ids.insert(task.id.clone()) {
            return Err(TaskDraftValidationError::DuplicateId(task.id.clone()));
        }
        if require_complete && task.title.trim().is_empty() {
            return Err(TaskDraftValidationError::EmptyTitle(task.id.clone()));
        }
        if require_complete && task.description.trim().is_empty() {
            return Err(TaskDraftValidationError::EmptyDescription(task.id.clone()));
        }
        if require_complete && !task.acceptance_criteria.iter().any(|item| !item.trim().is_empty()) {
            return Err(TaskDraftValidationError::EmptyAcceptanceCriteria(task.id.clone()));
        }
        let mut dependencies = HashSet::with_capacity(task.blocked_by.len());
        for dependency in &task.blocked_by {
            if dependency == &task.id {
                return Err(TaskDraftValidationError::SelfDependency(task.id.clone()));
            }
            if !dependencies.insert(dependency) {
                return Err(TaskDraftValidationError::DuplicateDependency(task.id.clone()));
            }
        }
    }

    for task in &draft.tasks {
        for dependency in &task.blocked_by {
            if !ids.contains(dependency) {
                return Err(TaskDraftValidationError::MissingDependency(dependency.clone()));
            }
        }
    }

    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    let graph = draft
        .tasks
        .iter()
        .map(|task| {
            (
                task.id.as_str(),
                task.blocked_by.iter().map(String::as_str).collect::<Vec<_>>(),
            )
        })
        .collect::<HashMap<_, _>>();
    for id in graph.keys() {
        if has_cycle(id, &graph, &mut visiting, &mut visited) {
            return Err(TaskDraftValidationError::Cycle);
        }
    }
    if draft.version == 2 {
        let integrations = draft
            .tasks
            .iter()
            .filter(|task| task.execution_scope == Some(TaskDraftExecutionScope::ProjectIntegration))
            .collect::<Vec<_>>();
        if draft.tasks.len() < 2 || integrations.len() != 1 || integrations[0].task_type != TaskDraftType::Test {
            return Err(TaskDraftValidationError::InvalidIntegration);
        }
        let integration = integrations[0];
        if draft.tasks.iter().any(|task| task.blocked_by.contains(&integration.id)) {
            return Err(TaskDraftValidationError::InvalidIntegration);
        }
        let mut ancestors = HashSet::new();
        let mut pending = integration.blocked_by.iter().map(String::as_str).collect::<Vec<_>>();
        while let Some(id) = pending.pop() {
            if ancestors.insert(id) {
                pending.extend(graph[id].iter().copied());
            }
        }
        if ancestors.len() != draft.tasks.len() - 1 {
            return Err(TaskDraftValidationError::InvalidIntegration);
        }
        for task in &draft.tasks {
            if task.execution_scope.is_none()
                || task.requirement_ids.is_empty()
                || task.requirement_ids.len() > 32
                || task.requirement_ids.iter().any(|id| id.is_empty() || id.len() > 64)
            {
                return Err(TaskDraftValidationError::InvalidScope(task.id.clone()));
            }
        }
    }
    Ok(())
}

pub fn is_integration_task(draft: &TaskDraftArtifact, task: &TaskDraftTask) -> bool {
    if validate_task_draft(draft, true).is_err()
        || task.task_type != TaskDraftType::Test
        || !draft.tasks.iter().any(|candidate| candidate == task)
        || draft
            .tasks
            .iter()
            .any(|candidate| candidate.blocked_by.contains(&task.id))
        || (draft.version == 2 && task.execution_scope != Some(TaskDraftExecutionScope::ProjectIntegration))
    {
        return false;
    }
    let mut covered = HashSet::new();
    let mut pending = task.blocked_by.iter().map(String::as_str).collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        if id == task.id {
            return false;
        }
        if covered.insert(id) {
            let Some(dependency) = draft.tasks.iter().find(|candidate| candidate.id == id) else {
                return false;
            };
            pending.extend(dependency.blocked_by.iter().map(String::as_str));
        }
    }
    covered.len() == draft.tasks.len() - 1
}

pub(crate) fn validate_requirement_links(
    draft: &TaskDraftArtifact,
    blueprint: &serde_json::Value,
) -> Result<(), TaskDraftValidationError> {
    if draft.version != 2 {
        return Ok(());
    }
    let ids = blueprint
        .get("requirements")
        .and_then(serde_json::Value::as_array)
        .map(|requirements| {
            requirements
                .iter()
                .filter_map(|requirement| requirement.get("id").and_then(serde_json::Value::as_str))
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    if ids.is_empty() {
        return Err(TaskDraftValidationError::InvalidScope(
            "blueprint requirements are missing".into(),
        ));
    }
    let mut covered = HashSet::new();
    for task in &draft.tasks {
        let mut seen = HashSet::new();
        for id in &task.requirement_ids {
            if !ids.contains(id.as_str()) || !seen.insert(id.as_str()) {
                return Err(TaskDraftValidationError::InvalidScope(task.id.clone()));
            }
            covered.insert(id.as_str());
        }
    }
    if covered != ids {
        return Err(TaskDraftValidationError::InvalidScope(
            "some blueprint requirements have no task".into(),
        ));
    }
    Ok(())
}

fn has_cycle<'a>(
    id: &'a str,
    graph: &HashMap<&'a str, Vec<&'a str>>,
    visiting: &mut HashSet<&'a str>,
    visited: &mut HashSet<&'a str>,
) -> bool {
    if visiting.contains(id) {
        return true;
    }
    if visited.contains(id) {
        return false;
    }
    visiting.insert(id);
    if graph.get(id).is_some_and(|dependencies| {
        dependencies
            .iter()
            .any(|dependency| has_cycle(dependency, graph, visiting, visited))
    }) {
        return true;
    }
    visiting.remove(id);
    visited.insert(id);
    false
}

pub fn generate_task_draft(blueprint: &serde_json::Value, updated_at: i64) -> TaskDraftArtifact {
    let context = blueprint
        .get("sections")
        .and_then(serde_json::Value::as_array)
        .map(|sections| {
            sections
                .iter()
                .filter_map(|section| {
                    let content = section.get("content")?.as_str()?.trim();
                    if content.is_empty() {
                        return None;
                    }
                    let title = section
                        .get("title")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("需求");
                    Some(format!("{title}: {content}"))
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|context| !context.is_empty())
        .unwrap_or_else(|| "根据已确认蓝图实现产品目标".to_owned());
    let tasks = vec![
        TaskDraftTask {
            id: "data-1".to_owned(),
            title: "定义数据契约与存储".to_owned(),
            description: format!(
                "围绕以下已确认需求，定义实际领域的数据契约、校验规则与必要的持久化；不要添加需求之外的实体或功能。\n\n{context}"
            ),
            task_type: TaskDraftType::Data,
            blocked_by: vec![],
            acceptance_criteria: vec!["数据结构可持久化并可被后续接口读取".to_owned()],
            suggested_role: "backend".to_owned(),
            effort: TaskDraftEffort::Medium,
            execution_scope: None,
            requirement_ids: vec![],
        },
        TaskDraftTask {
            id: "backend-1".to_owned(),
            title: "实现后端接口".to_owned(),
            description: format!(
                "读取已批准的数据任务产物，根据以下需求实现核心业务逻辑、所需接口和错误处理；接口名称与行为由实际产品决定。若需求包含 AI 调用，应明确真实模型接入与未配置时的降级，不把模板输出冒充模型结果。\n\n{context}"
            ),
            task_type: TaskDraftType::Backend,
            blocked_by: vec!["data-1".to_owned()],
            acceptance_criteria: vec!["接口可完成核心流程并返回稳定数据契约".to_owned()],
            suggested_role: "backend".to_owned(),
            effort: TaskDraftEffort::Medium,
            execution_scope: None,
            requirement_ids: vec![],
        },
        TaskDraftTask {
            id: "frontend-1".to_owned(),
            title: "实现前端核心流程".to_owned(),
            description: format!(
                "读取已批准的数据与业务任务产物，按照以下已确认需求及交付形态实现用户操作入口，连接实际业务接口，展示加载、成功、失败和必要的历史数据；不要套用其他产品的输入、输出数量或接口。\n\n{context}"
            ),
            task_type: TaskDraftType::Frontend,
            blocked_by: vec!["backend-1".to_owned()],
            acceptance_criteria: vec![
                "用户可通过已确认的交付形态完成核心需求，操作入口调用实际业务实现".to_owned(),
                "结果与已确认需求的数据契约一致，必要的持久化数据在重新打开后可读取".to_owned(),
                "无效输入和调用失败有明确反馈，执行期间防止重复提交".to_owned(),
            ],
            suggested_role: "frontend".to_owned(),
            effort: TaskDraftEffort::High,
            execution_scope: None,
            requirement_ids: vec![],
        },
        TaskDraftTask {
            id: "test-1".to_owned(),
            title: "完成最终集成与交付验收".to_owned(),
            description: format!(
                "执行最终集成：检查已批准任务的真实产物，将它们组装到项目根目录；生成可运行产品、START.md 和 ACCEPTANCE.md，实际验证核心流程、错误恢复与服务停止重启后的持久化。验收文档逐项区分已通过、失败和未验证，不预先声明完成。\n\n{context}"
            ),
            task_type: TaskDraftType::Test,
            blocked_by: vec!["backend-1".to_owned(), "frontend-1".to_owned()],
            acceptance_criteria: vec![
                "项目根目录存在可运行产品和 START.md 启动说明".to_owned(),
                "ACCEPTANCE.md 记录关键流程、错误恢复和人工验收证据".to_owned(),
                "停止并重启产品后，核心数据仍可读取".to_owned(),
                "交付检查接口显示所有任务完成、工作区可用且成本状态明确".to_owned(),
            ],
            suggested_role: "qa".to_owned(),
            effort: TaskDraftEffort::Medium,
            execution_scope: None,
            requirement_ids: vec![],
        },
    ];
    TaskDraftArtifact {
        version: 1,
        generated_by: "draft".to_owned(),
        confirmed: false,
        revision: 1,
        updated_at,
        tasks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(tasks: Vec<TaskDraftTask>) -> TaskDraftArtifact {
        TaskDraftArtifact {
            version: 1,
            generated_by: "draft".to_owned(),
            confirmed: false,
            revision: 1,
            updated_at: 0,
            tasks,
        }
    }

    fn task(id: &str, blocked_by: Vec<&str>) -> TaskDraftTask {
        TaskDraftTask {
            id: id.to_owned(),
            title: "Title".to_owned(),
            description: "Description".to_owned(),
            task_type: TaskDraftType::Backend,
            blocked_by: blocked_by.into_iter().map(str::to_owned).collect(),
            acceptance_criteria: vec!["Done".to_owned()],
            suggested_role: "backend".to_owned(),
            effort: TaskDraftEffort::Low,
            execution_scope: None,
            requirement_ids: vec![],
        }
    }

    #[test]
    fn accepts_valid_graph_and_rejects_cycle() {
        assert!(validate_task_draft(&draft(vec![task("a", vec![]), task("b", vec!["a"])]), true).is_ok());
        assert_eq!(
            validate_task_draft(&draft(vec![task("a", vec!["b"]), task("b", vec!["a"])]), true),
            Err(TaskDraftValidationError::Cycle)
        );
    }

    #[test]
    fn legacy_middle_test_is_isolated_and_only_complete_terminal_test_can_integrate() {
        let mut graph = generate_task_draft(&serde_json::json!({}), 1);
        let mut middle = task("middle-test", vec!["frontend-1"]);
        middle.task_type = TaskDraftType::Test;
        graph.tasks.last_mut().unwrap().blocked_by = vec![middle.id.clone()];
        graph.tasks.insert(graph.tasks.len() - 1, middle);
        let middle = &graph.tasks[graph.tasks.len() - 2];
        let last = graph.tasks.last().unwrap();
        assert!(!is_integration_task(&graph, middle));
        assert!(is_integration_task(&graph, last));
        graph.tasks.last_mut().unwrap().blocked_by = vec!["data-1".into()];
        assert!(
            !is_integration_task(&graph, graph.tasks.last().unwrap()),
            "a terminal Test with partial coverage cannot grant root access"
        );
    }

    #[test]
    fn model_scope_needs_valid_terminal_graph_and_an_explicit_integration_scope() {
        let mut graph = generate_task_draft(&serde_json::json!({}), 1);
        graph.version = 2;
        graph.generated_by = "model".into();
        let last_index = graph.tasks.len() - 1;
        for (index, task) in graph.tasks.iter_mut().enumerate() {
            task.execution_scope = Some(if index == last_index {
                TaskDraftExecutionScope::ProjectIntegration
            } else {
                TaskDraftExecutionScope::TaskWorkspace
            });
            task.requirement_ids = vec!["r1".into()];
        }
        graph.tasks[2].task_type = TaskDraftType::Test;
        assert!(!is_integration_task(&graph, &graph.tasks[2]));
        assert!(is_integration_task(&graph, graph.tasks.last().unwrap()));
        graph.tasks[2].execution_scope = Some(TaskDraftExecutionScope::ProjectIntegration);
        assert!(!is_integration_task(&graph, &graph.tasks[2]));
        assert!(!is_integration_task(&graph, graph.tasks.last().unwrap()));
        graph.tasks[2].execution_scope = Some(TaskDraftExecutionScope::TaskWorkspace);
        graph.tasks.last_mut().unwrap().execution_scope = Some(TaskDraftExecutionScope::TaskWorkspace);
        assert!(!is_integration_task(&graph, graph.tasks.last().unwrap()));
    }

    #[test]
    fn generated_draft_contains_context_and_stable_dependencies() {
        let generated = generate_task_draft(
            &serde_json::json!({
                "sections": [{"content": "Ship a focused workflow"}]
            }),
            42,
        );
        assert_eq!(generated.generated_by, "draft");
        assert_eq!(generated.updated_at, 42);
        assert!(generated.tasks[0].description.contains("Ship a focused workflow"));
        assert_eq!(generated.tasks[2].blocked_by, vec!["backend-1"]);
    }

    #[test]
    fn generated_integration_task_contains_delivery_requirements() {
        let generated = generate_task_draft(&serde_json::json!({}), 42);
        let integration = generated.tasks.last().unwrap();
        assert_eq!(integration.task_type, TaskDraftType::Test);
        assert!(integration.description.contains("最终集成"));
        assert!(
            integration
                .acceptance_criteria
                .iter()
                .any(|criterion| criterion.contains("START.md"))
        );
        assert!(
            integration
                .acceptance_criteria
                .iter()
                .any(|criterion| criterion.contains("重启") || criterion.contains("restart"))
        );
    }

    #[test]
    fn different_products_keep_their_own_context_without_sample_requirements() {
        for (goal, users, output) in [
            ("Track warehouse stock", "Warehouse staff", "A local inventory web app"),
            ("Plan study sessions", "Students", "A calendar desktop prototype"),
        ] {
            let generated = generate_task_draft(
                &serde_json::json!({"sections": [
                    {"title":"Goals", "content":goal},
                    {"title":"Users", "content":users},
                    {"title":"Deliverable", "content":output}
                ]}),
                42,
            );
            for task in &generated.tasks {
                assert!(task.description.contains(goal));
                assert!(task.description.contains(users));
                assert!(task.description.contains(output));
                assert!(!task.description.contains("三个标题"));
                assert!(!task.description.contains("/api/generate"));
                assert!(!task.description.contains("已完成："));
            }
            validate_task_draft(&generated, true).unwrap();
        }
    }
}
