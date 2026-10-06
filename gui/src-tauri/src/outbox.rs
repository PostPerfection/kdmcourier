use crate::state::AppState;
use postkit::kdm_distribution::database::{DeliveryRecord, DistributionDatabase, IssueRecord};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outbox {
    pub issues: Vec<IssueRecord>,
    pub deliveries: Vec<DeliveryRecord>,
}

// newest first, the way the page lists them
pub fn outbox(database: &DistributionDatabase) -> Result<Outbox, String> {
    let mut issues = database.issues()?;
    issues.reverse();
    let mut deliveries = database.deliveries()?;
    deliveries.reverse();
    Ok(Outbox { issues, deliveries })
}

#[tauri::command(async)]
pub fn outbox_list(state: tauri::State<'_, AppState>) -> Result<Outbox, String> {
    state.with_database(|database| outbox(database))
}
