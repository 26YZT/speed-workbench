use sqlx::SqlitePool;

use crate::error::DbError;
use crate::models::{ProductFactoryExecutionRow, ProductFactoryRunRow, TeamRow, TeamTaskRow};
use crate::repository::product_factory::IProductFactoryRepository;

/// SQLite-backed Product Factory workflow storage.
#[derive(Clone, Debug)]
pub struct SqliteProductFactoryRepository {
    pool: SqlitePool,
}

impl SqliteProductFactoryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl IProductFactoryRepository for SqliteProductFactoryRepository {
    async fn create_run(&self, run: &ProductFactoryRunRow) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO product_factory_runs (\
                id, user_id, name, idea, target_user, problem, expected_output, workspace_path, budget_usd, status, \
                interview_json, blueprint_json, task_draft_json, team_id, created_at, updated_at, plan_revision\
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&run.id)
        .bind(&run.user_id)
        .bind(&run.name)
        .bind(&run.idea)
        .bind(&run.target_user)
        .bind(&run.problem)
        .bind(&run.expected_output)
        .bind(&run.workspace_path)
        .bind(run.budget_usd)
        .bind(&run.status)
        .bind(&run.interview_json)
        .bind(&run.blueprint_json)
        .bind(&run.task_draft_json)
        .bind(&run.team_id)
        .bind(run.created_at)
        .bind(run.updated_at)
        .bind(run.plan_revision)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_run(&self, user_id: &str, run_id: &str) -> Result<Option<ProductFactoryRunRow>, DbError> {
        Ok(
            sqlx::query_as::<_, ProductFactoryRunRow>(
                "SELECT * FROM product_factory_runs WHERE user_id = ? AND id = ?",
            )
            .bind(user_id)
            .bind(run_id)
            .fetch_optional(&self.pool)
            .await?,
        )
    }

    async fn list_runs(&self, user_id: &str) -> Result<Vec<ProductFactoryRunRow>, DbError> {
        Ok(sqlx::query_as::<_, ProductFactoryRunRow>(
            "SELECT * FROM product_factory_runs WHERE user_id = ? ORDER BY updated_at DESC, id DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    async fn update_run(&self, run: &ProductFactoryRunRow) -> Result<(), DbError> {
        let result = sqlx::query(
            "UPDATE product_factory_runs SET \
                name = ?, idea = ?, target_user = ?, problem = ?, expected_output = ?, workspace_path = ?, \
                budget_usd = ?, status = ?, interview_json = ?, blueprint_json = ?, task_draft_json = ?, \
                team_id = ?, updated_at = ?, plan_revision = ? \
             WHERE user_id = ? AND id = ? AND team_id IS NULL \
             AND status NOT IN ('handed_off', 'running', 'in_review', 'completed') AND plan_revision = ?",
        )
        .bind(&run.name)
        .bind(&run.idea)
        .bind(&run.target_user)
        .bind(&run.problem)
        .bind(&run.expected_output)
        .bind(&run.workspace_path)
        .bind(run.budget_usd)
        .bind(&run.status)
        .bind(&run.interview_json)
        .bind(&run.blueprint_json)
        .bind(&run.task_draft_json)
        .bind(&run.team_id)
        .bind(run.updated_at)
        .bind(run.plan_revision)
        .bind(&run.user_id)
        .bind(&run.id)
        .bind(run.plan_revision - 1)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            if self.get_run(&run.user_id, &run.id).await?.is_some() {
                return Err(DbError::Conflict(
                    "committed handoff cannot be overwritten by a workflow save".into(),
                ));
            }
            return Err(DbError::NotFound(run.id.clone()));
        }
        Ok(())
    }

    async fn update_task_draft_if_current(
        &self,
        run: &ProductFactoryRunRow,
        expected_revision: Option<u64>,
        expected_task_draft_json: Option<&str>,
        expected_status: &str,
    ) -> Result<(), DbError> {
        let mut query = String::from(
            "UPDATE product_factory_runs SET task_draft_json = ?, status = ?, updated_at = ?, plan_revision = ? \
             WHERE user_id = ? AND id = ? AND status = ? AND plan_revision = ?",
        );
        if expected_task_draft_json.is_some() {
            query.push_str(" AND task_draft_json = ?");
        } else {
            query.push_str(" AND task_draft_json IS NULL");
        }
        let mut statement = sqlx::query(&query)
            .bind(&run.task_draft_json)
            .bind(&run.status)
            .bind(run.updated_at)
            .bind(run.plan_revision)
            .bind(&run.user_id)
            .bind(&run.id)
            .bind(expected_status)
            .bind(run.plan_revision - 1);
        if let Some(expected_json) = expected_task_draft_json {
            statement = statement.bind(expected_json);
        }
        let result = statement.execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            return Err(DbError::Conflict(format!(
                "task draft revision conflict for {} (expected {:?})",
                run.id, expected_revision
            )));
        }
        Ok(())
    }

    async fn delete_run(&self, user_id: &str, run_id: &str) -> Result<(), DbError> {
        let result = sqlx::query("DELETE FROM product_factory_runs WHERE user_id = ? AND id = ?")
            .bind(user_id)
            .bind(run_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound(run_id.to_owned()));
        }
        Ok(())
    }

    async fn complete_handoff(
        &self,
        run: &ProductFactoryRunRow,
        team: &TeamRow,
        tasks: &[TeamTaskRow],
    ) -> Result<(), DbError> {
        if team.user_id != run.user_id
            || team.workspace != run.workspace_path
            || team.name != run.name
            || tasks.is_empty()
            || tasks.iter().any(|task| task.team_id != team.id)
        {
            return Err(DbError::Conflict("handoff owner or graph mismatch".into()));
        }
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE product_factory_runs SET team_id = ?, status = 'handed_off', updated_at = ? \
             WHERE user_id = ? AND id = ? AND status = 'task_draft_ready' AND team_id IS NULL \
             AND task_draft_json = ? AND json_extract(task_draft_json, '$.confirmed') = 1 \
             AND workspace_path = ? AND budget_usd IS ? AND name = ?",
        )
        .bind(&team.id)
        .bind(team.updated_at)
        .bind(&run.user_id)
        .bind(&run.id)
        .bind(&run.task_draft_json)
        .bind(&run.workspace_path)
        .bind(run.budget_usd)
        .bind(&run.name)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            tx.rollback().await?;
            return Err(DbError::Conflict(
                "handoff snapshot changed or already handed off".into(),
            ));
        }
        sqlx::query(
            "INSERT INTO teams (id, user_id, name, workspace, workspace_mode, agents, lead_agent_id, \
             session_mode, agents_version, created_at, updated_at, project_id, folder_id) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&team.id)
        .bind(&team.user_id)
        .bind(&team.name)
        .bind(&team.workspace)
        .bind(&team.workspace_mode)
        .bind(&team.agents)
        .bind(&team.lead_agent_id)
        .bind(&team.session_mode)
        .bind(&team.agents_version)
        .bind(team.created_at)
        .bind(team.updated_at)
        .bind(&team.project_id)
        .bind(&team.folder_id)
        .execute(&mut *tx)
        .await?;
        for task in tasks {
            sqlx::query(
                "INSERT INTO team_tasks (id, team_id, subject, description, status, owner, blocked_by, blocks, \
                 metadata, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&task.id)
            .bind(&task.team_id)
            .bind(&task.subject)
            .bind(&task.description)
            .bind(&task.status)
            .bind(&task.owner)
            .bind(&task.blocked_by)
            .bind(&task.blocks)
            .bind(&task.metadata)
            .bind(task.created_at)
            .bind(task.updated_at)
            .execute(&mut *tx)
            .await?;
        }
        if let Some(limit) = run.budget_usd {
            sqlx::query("INSERT INTO team_budgets (team_id, user_id, limit_usd, updated_at) VALUES (?, ?, ?, ?)")
                .bind(&team.id)
                .bind(&run.user_id)
                .bind(limit)
                .bind(team.updated_at)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn get_handoff_team(&self, user_id: &str, team_id: &str) -> Result<Option<TeamRow>, DbError> {
        Ok(
            sqlx::query_as::<_, TeamRow>("SELECT * FROM teams WHERE user_id = ? AND id = ?")
                .bind(user_id)
                .bind(team_id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    async fn get_handoff_task(
        &self,
        user_id: &str,
        team_id: &str,
        task_id: &str,
    ) -> Result<Option<TeamTaskRow>, DbError> {
        Ok(sqlx::query_as(
            "SELECT task.* FROM team_tasks task JOIN teams team ON team.id = task.team_id \
             WHERE team.user_id = ? AND task.team_id = ? AND task.id = ?",
        )
        .bind(user_id)
        .bind(team_id)
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    async fn get_execution(&self, user_id: &str, run_id: &str) -> Result<Option<ProductFactoryExecutionRow>, DbError> {
        Ok(sqlx::query_as(
            "SELECT execution.* FROM product_factory_execution execution \
             JOIN product_factory_runs run ON run.id = execution.run_id \
             WHERE execution.user_id = ? AND run.user_id = ? AND run.id = ?",
        )
        .bind(user_id)
        .bind(user_id)
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    async fn reserve_execution(
        &self,
        run: &ProductFactoryRunRow,
        team: &TeamRow,
        task: &TeamTaskRow,
        now: i64,
    ) -> Result<bool, DbError> {
        let result = sqlx::query(
            "INSERT INTO product_factory_execution (run_id, user_id, team_id, task_id, state, requested_at, updated_at) \
             SELECT ?, ?, ?, ?, 'pending', ?, ? WHERE EXISTS (\
                 SELECT 1 FROM product_factory_runs WHERE id = ? AND user_id = ? AND status = 'handed_off' \
                 AND team_id = ? AND task_draft_json = ? AND blueprint_json IS ? AND workspace_path = ?) \
             AND EXISTS (SELECT 1 FROM teams WHERE id = ? AND user_id = ? AND workspace = ? AND agents = ?) \
             AND EXISTS (SELECT 1 FROM team_tasks WHERE id = ? AND team_id = ? AND status = 'pending' \
                 AND owner IS NULL AND blocked_by = ? AND subject = ? AND description IS ? AND metadata IS ?) \
             ON CONFLICT(run_id) DO NOTHING",
        )
        .bind(&run.id).bind(&run.user_id).bind(&team.id).bind(&task.id).bind(now).bind(now)
        .bind(&run.id).bind(&run.user_id).bind(&team.id).bind(&run.task_draft_json)
        .bind(&run.blueprint_json).bind(&run.workspace_path)
        .bind(&team.id).bind(&run.user_id).bind(&run.workspace_path).bind(&team.agents)
        .bind(&task.id).bind(&team.id).bind(&task.blocked_by).bind(&task.subject)
        .bind(&task.description).bind(&task.metadata).execute(&self.pool).await?;
        if result.rows_affected() == 1 {
            return Ok(true);
        }
        if self.get_execution(&run.user_id, &run.id).await?.is_some() {
            return Ok(false);
        }
        Err(DbError::Conflict("execution snapshot changed before dispatch".into()))
    }

    async fn finish_execution(
        &self,
        user_id: &str,
        run_id: &str,
        state: &str,
        message_id: Option<&str>,
        team_run_id: Option<&str>,
        now: i64,
    ) -> Result<(), DbError> {
        if !matches!(state, "enqueued" | "uncertain")
            || (state == "enqueued" && (message_id.is_none_or(str::is_empty) || team_run_id.is_none_or(str::is_empty)))
        {
            return Err(DbError::Conflict("invalid execution receipt".into()));
        }
        let result = sqlx::query(
            "UPDATE product_factory_execution SET state = ?, message_id = ?, team_run_id = ?, updated_at = ? \
             WHERE user_id = ? AND run_id = ? AND state = 'pending' \
             AND EXISTS (SELECT 1 FROM product_factory_runs WHERE id = ? AND user_id = ?)",
        )
        .bind(state)
        .bind(message_id)
        .bind(team_run_id)
        .bind(now)
        .bind(user_id)
        .bind(run_id)
        .bind(run_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(DbError::Conflict("execution receipt cannot be overwritten".into()));
        }
        Ok(())
    }

    async fn advance_execution(
        &self,
        user_id: &str,
        run_id: &str,
        team_id: &str,
        previous_task_id: &str,
        next_task: &TeamTaskRow,
        now: i64,
    ) -> Result<bool, DbError> {
        let result = sqlx::query(
            "UPDATE product_factory_execution SET task_id = ?, state = 'pending', message_id = NULL,
                team_run_id = NULL, requested_at = ?, updated_at = ?
             WHERE user_id = ? AND run_id = ? AND team_id = ? AND task_id = ? AND state IN ('enqueued', 'uncertain')
             AND EXISTS (SELECT 1 FROM team_tasks previous WHERE previous.id = ? AND previous.team_id = ? AND previous.status = 'completed')
             AND EXISTS (SELECT 1 FROM team_tasks next WHERE next.id = ? AND next.team_id = ? AND next.status = 'pending'
                 AND next.owner IS NULL AND next.blocked_by = ? AND next.subject = ? AND next.description IS ? AND next.metadata IS ?)",
        )
        .bind(&next_task.id)
        .bind(now)
        .bind(now)
        .bind(user_id)
        .bind(run_id)
        .bind(team_id)
        .bind(previous_task_id)
        .bind(previous_task_id)
        .bind(team_id)
        .bind(&next_task.id)
        .bind(team_id)
        .bind(&next_task.blocked_by)
        .bind(&next_task.subject)
        .bind(&next_task.description)
        .bind(&next_task.metadata)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    async fn retry_execution(
        &self,
        user_id: &str,
        run_id: &str,
        team_id: &str,
        task_id: &str,
        now: i64,
    ) -> Result<bool, DbError> {
        let result = sqlx::query(
            "UPDATE product_factory_execution SET state = 'pending', message_id = NULL, team_run_id = NULL,
                requested_at = ?, updated_at = ?
             WHERE user_id = ? AND run_id = ? AND team_id = ? AND task_id = ? AND state IN ('enqueued', 'uncertain')
             AND EXISTS (SELECT 1 FROM team_tasks task WHERE task.id = ? AND task.team_id = ? AND task.status = 'in_progress')",
        )
        .bind(now)
        .bind(now)
        .bind(user_id)
        .bind(run_id)
        .bind(team_id)
        .bind(task_id)
        .bind(task_id)
        .bind(team_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    async fn set_execution_run_status(
        &self,
        user_id: &str,
        run_id: &str,
        expected_status: &str,
        next_status: &str,
        now: i64,
    ) -> Result<bool, DbError> {
        let result = sqlx::query(
            "UPDATE product_factory_runs SET status = ?, updated_at = ?
             WHERE user_id = ? AND id = ? AND team_id IS NOT NULL AND status = ?",
        )
        .bind(next_status)
        .bind(now)
        .bind(user_id)
        .bind(run_id)
        .bind(expected_status)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    async fn list_prepared_conversation_ids(
        &self,
        user_id: &str,
        preparation_id: &str,
    ) -> Result<Vec<String>, DbError> {
        Ok(sqlx::query_scalar(
            "SELECT id FROM conversations WHERE user_id = ? AND \
             json_extract(CASE WHEN json_valid(extra) THEN extra ELSE '{}' END, '$.product_factory_preparation_id') = ?",
        ).bind(user_id).bind(preparation_id).fetch_all(&self.pool).await?)
    }
}
