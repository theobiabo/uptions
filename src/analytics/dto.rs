use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct AnalyticsOverviewResponse {
    pub counts: AnalyticsCounts,
    pub daily_activity: Vec<DailyActivity>,
    pub workflow_activity: Vec<WorkflowActivity>,
    pub trade_status_summary: Vec<StatusCount>,
    pub pnl: PnlAvailability,
    pub performance: PerformanceAvailability,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AnalyticsCounts {
    pub trade_intents: u64,
    pub automations: u64,
    pub active_automations: u64,
    pub automation_runs: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DailyActivity {
    #[schema(example = "2026-07-13")]
    pub date: String,
    pub trade_intents: u64,
    pub automation_runs: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StatusCount {
    pub status: String,
    pub count: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WorkflowActivity {
    pub automation_id: String,
    pub title: String,
    pub automation_status: String,
    pub total_runs: u64,
    pub run_status_summary: Vec<StatusCount>,
    pub last_run_at: Option<String>,
    pub last_run_status: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PnlAvailability {
    pub available: bool,
    pub realized_pnl: Option<f64>,
    pub unrealized_pnl: Option<f64>,
    pub total_pnl: Option<f64>,
    pub reason: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PerformanceAvailability {
    pub available: bool,
    pub win_rate: Option<f64>,
    pub return_percentage: Option<f64>,
    pub reason: String,
}
