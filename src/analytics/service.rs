use std::collections::{BTreeMap, HashMap};

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::{
    analytics::dto::{
        AnalyticsCounts, AnalyticsOverviewResponse, DailyActivity, PerformanceAvailability,
        PnlAvailability, StatusCount, WorkflowActivity,
    },
    db::Db,
    entities::{automation, automation_run, trade_intent},
    error::AppError,
};

#[derive(Clone)]
pub struct AnalyticsService {
    db: Db,
}

impl AnalyticsService {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn overview(&self, user_id: &str) -> Result<AnalyticsOverviewResponse, AppError> {
        let trades = trade_intent::Entity::find()
            .filter(trade_intent::Column::UserId.eq(user_id))
            .all(&self.db)
            .await?;
        let automations = automation::Entity::find()
            .filter(automation::Column::UserId.eq(user_id))
            .all(&self.db)
            .await?;
        let runs = automation_run::Entity::find()
            .filter(automation_run::Column::UserId.eq(user_id))
            .all(&self.db)
            .await?;

        let mut daily = BTreeMap::<String, (u64, u64)>::new();
        let mut trade_statuses = BTreeMap::<String, u64>::new();
        for trade in &trades {
            daily
                .entry(trade.created_at.date_naive().to_string())
                .or_default()
                .0 += 1;
            *trade_statuses.entry(trade.status.clone()).or_default() += 1;
        }

        let mut runs_by_automation = HashMap::<String, Vec<&automation_run::Model>>::new();
        for run in &runs {
            daily
                .entry(run.created_at.date_naive().to_string())
                .or_default()
                .1 += 1;
            runs_by_automation
                .entry(run.automation_id.clone())
                .or_default()
                .push(run);
        }

        let workflow_activity = automations
            .iter()
            .map(|automation| {
                let automation_runs = runs_by_automation
                    .get(&automation.id)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                let mut statuses = BTreeMap::<String, u64>::new();
                for run in automation_runs {
                    *statuses.entry(run.status.clone()).or_default() += 1;
                }

                WorkflowActivity {
                    automation_id: automation.id.clone(),
                    title: automation.title.clone(),
                    automation_status: automation.status.clone(),
                    total_runs: automation_runs.len() as u64,
                    run_status_summary: status_counts(statuses),
                    last_run_at: automation.last_run_at.map(|value| value.to_rfc3339()),
                    last_run_status: automation.last_run_status.clone(),
                }
            })
            .collect();

        Ok(AnalyticsOverviewResponse {
            counts: AnalyticsCounts {
                trade_intents: trades.len() as u64,
                automations: automations.len() as u64,
                active_automations: automations
                    .iter()
                    .filter(|automation| automation.status == "active")
                    .count() as u64,
                automation_runs: runs.len() as u64,
            },
            daily_activity: daily
                .into_iter()
                .map(|(date, (trade_intents, automation_runs))| DailyActivity {
                    date,
                    trade_intents,
                    automation_runs,
                })
                .collect(),
            workflow_activity,
            trade_status_summary: status_counts(trade_statuses),
            pnl: PnlAvailability {
                available: false,
                realized_pnl: None,
                unrealized_pnl: None,
                total_pnl: None,
                reason:
                    "Persisted trade intents do not contain settlement or position valuation data"
                        .to_owned(),
            },
            performance: PerformanceAvailability {
                available: false,
                win_rate: None,
                return_percentage: None,
                reason: "Persisted trade intents do not contain settled outcomes or returns"
                    .to_owned(),
            },
        })
    }
}

fn status_counts(statuses: BTreeMap<String, u64>) -> Vec<StatusCount> {
    statuses
        .into_iter()
        .map(|(status, count)| StatusCount { status, count })
        .collect()
}
