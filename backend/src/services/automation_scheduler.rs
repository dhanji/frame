use tokio_cron_scheduler::{JobScheduler, Job};
use sqlx::SqlitePool;
use std::sync::Arc;
use crate::services::agent::AgentEngine;
use std::error::Error as StdError;

type BoxError = Box<dyn StdError + Send + Sync>;

pub struct AutomationScheduler {
    scheduler: JobScheduler,
    pool: SqlitePool,
    agent_engine: Arc<AgentEngine>,
}

impl AutomationScheduler {
    pub async fn new(pool: SqlitePool, agent_engine: Arc<AgentEngine>) -> Result<Self, BoxError> {
        let scheduler = JobScheduler::new().await?;
        Ok(Self {
            scheduler,
            pool,
            agent_engine,
        })
    }

    pub async fn start(&mut self) -> Result<(), BoxError> {
        log::info!("Starting automation scheduler...");
        
        // Load all enabled automations from database
        let automations = sqlx::query_as::<_, (String, String, String)>(
            "SELECT id, schedule, prompt FROM automations WHERE enabled = 1"
        )
        .fetch_all(&self.pool)
        .await?;

        log::info!("Found {} enabled automations", automations.len());

        for (automation_id, schedule, prompt) in automations {
            self.schedule_automation(automation_id, schedule, prompt).await?;
        }

        self.scheduler.start().await?;
        log::info!("Automation scheduler started");
        Ok(())
    }

    pub async fn schedule_automation(
        &mut self,
        automation_id: String,
        schedule: String,
        prompt: String,
    ) -> Result<(), BoxError> {
        let pool = self.pool.clone();
        let agent_engine = self.agent_engine.clone();
        let automation_id_clone = automation_id.clone();

        let job = Job::new_async(schedule.as_str(), move |_uuid, _l| {
            let pool = pool.clone();
            let agent_engine = agent_engine.clone();
            let automation_id = automation_id_clone.clone();
            let prompt = prompt.clone();

            Box::pin(async move {
                log::info!("Executing automation: {}", automation_id);
                
                // Create automation run record
                let run_id = uuid::Uuid::new_v4().to_string();
                let _ = sqlx::query(
                    r#"
                    INSERT INTO automation_runs (id, automation_id, status, started_at)
                    VALUES (?, ?, 'running', CURRENT_TIMESTAMP)
                    "#
                )
                .bind(&run_id)
                .bind(&automation_id)
                .execute(&pool)
                .await;

                // Execute automation with AI agent
                let result = agent_engine
                    .process_message(prompt.clone(), vec![])
                    .await;

                match result {
                    Ok(response) => {
                        let (response_text, _tool_calls) = response;
                        
                        // Update automation run with success
                        let _ = sqlx::query(
                            r#"
                            UPDATE automation_runs 
                            SET status = 'success', result = ?, completed_at = CURRENT_TIMESTAMP
                            WHERE id = ?
                            "#
                        )
                        .bind(&response_text)
                        .bind(&run_id)
                        .execute(&pool)
                        .await;

                        // Update last_run timestamp
                        let _ = sqlx::query(
                            "UPDATE automations SET last_run = CURRENT_TIMESTAMP WHERE id = ?"
                        )
                        .bind(&automation_id)
                        .execute(&pool)
                        .await;

                        log::info!("Automation {} completed successfully", automation_id);
                    }
                    Err(e) => {
                        // Update automation run with error
                        let _ = sqlx::query(
                            r#"
                            UPDATE automation_runs 
                            SET status = 'failed', error = ?, completed_at = CURRENT_TIMESTAMP
                            WHERE id = ?
                            "#
                        )
                        .bind(format!("{}", e))
                        .bind(&run_id)
                        .execute(&pool)
                        .await;

                        log::error!("Automation {} failed: {}", automation_id, e);
                    }
                }
            })
        })?;

        self.scheduler.add(job).await?;
        log::info!("Scheduled automation {} with schedule: {}", automation_id, schedule);
        Ok(())
    }

    pub async fn reload_automations(&mut self) -> Result<(), BoxError> {
        log::info!("Reloading automations...");
        
        // Remove all existing jobs
        self.scheduler.shutdown().await?;
        self.scheduler = JobScheduler::new().await?;
        
        // Reload and reschedule
        self.start().await?;
        Ok(())
    }
}
