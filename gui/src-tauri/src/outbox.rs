use crate::settings::Settings;
use crate::state::AppState;
use chrono::{DateTime, Utc};
use postkit::kdm_distribution::database::{
    DeliveryId, DistributionDatabase, IssueRecord, StoredDelivery,
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outbox {
    pub issues: Vec<IssueRecord>,
    pub deliveries: Vec<StoredDelivery>,
}

// newest first, the way the page lists them
pub fn outbox(database: &DistributionDatabase) -> Result<Outbox, String> {
    let mut issues = database.issues()?;
    issues.reverse();
    let mut deliveries = database.deliveries()?;
    deliveries.reverse();
    Ok(Outbox { issues, deliveries })
}

pub fn resend(
    database: &mut DistributionDatabase,
    settings: &Settings,
    id: DeliveryId,
    now: DateTime<Utc>,
) -> Result<StoredDelivery, String> {
    let smtp = settings
        .smtp
        .as_ref()
        .ok_or_else(|| "set the SMTP server in Settings before resending".to_string())?;
    database.resend_delivery(id, smtp, now)
}

#[tauri::command(async)]
pub fn outbox_list(state: tauri::State<'_, AppState>) -> Result<Outbox, String> {
    state.with_database(|database| outbox(database))
}

#[tauri::command(async)]
pub fn outbox_resend(
    id: DeliveryId,
    state: tauri::State<'_, AppState>,
) -> Result<StoredDelivery, String> {
    let settings = state.settings()?;
    state.with_database(|database| resend(database, &settings, id, Utc::now()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bookings::{issue, IssueDestination};
    use crate::test_fixtures::{booked_database, fixtures, signed_settings, DCNC_TITLE};
    use postkit::kdm_distribution::database::{DeliveryResult, IssueScope};
    use postkit::kdm_distribution::email::{Security, SmtpConfig};

    // nothing listens on port 1
    const REFUSING_PORT: u16 = 1;

    #[test]
    fn a_resend_needs_smtp_and_records_a_new_delivery_with_its_result() {
        let directory = tempfile::tempdir().unwrap();
        let (mut database, booking) = booked_database(fixtures());
        let mut settings = signed_settings(fixtures(), directory.path());
        issue(
            &mut database,
            &settings,
            directory.path(),
            booking,
            IssueScope::PendingScreens,
            IssueDestination {
                output_folder: None,
                send_email: false,
            },
            Utc::now(),
        )
        .unwrap();
        let written = outbox(&database).unwrap().deliveries.remove(0);
        assert_eq!(written.record.content_title, DCNC_TITLE);
        assert_eq!(written.record.result, DeliveryResult::Written);

        let error = resend(&mut database, &settings, written.id, Utc::now()).unwrap_err();
        assert_eq!(error, "set the SMTP server in Settings before resending");
        assert_eq!(outbox(&database).unwrap().deliveries.len(), 1);

        settings.smtp = Some(SmtpConfig {
            host: "127.0.0.1".into(),
            port: REFUSING_PORT,
            security: Security::None,
            username: None,
            password: None,
            from: "kdm@distributor.test".into(),
            subject_template: None,
            body_template: None,
        });
        let resent = resend(&mut database, &settings, written.id, Utc::now()).unwrap();
        assert!(
            matches!(&resent.record.result, DeliveryResult::Failed(reason) if reason.contains("smtp send")),
            "{:?}",
            resent.record.result
        );
        assert_eq!(resent.record.recipients, vec!["kdm@rex.test"]);
        assert_eq!(resent.record.zip_path, written.record.zip_path);
        assert_eq!(resent.record.content_title, DCNC_TITLE);
        let deliveries = outbox(&database).unwrap().deliveries;
        assert_eq!(deliveries.len(), 2);
        assert_eq!(deliveries[0], resent);
    }
}
