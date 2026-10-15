use crate::recipient_certificates::CertificateSource;
use crate::settings::Settings;
use crate::state::AppState;
use chrono::{DateTime, NaiveDateTime, Utc};
use postkit::certificate::KdmFormulation;
use postkit::kdm_distribution::database::{DistributionDatabase, DkdmSource, TitleId};
use postkit::kdm_distribution::issue::{CertificateIssueOutcome, CertificateTarget};
use postkit::kdm_distribution::window::LocalWindow;
use serde::Deserialize;
use std::path::{Path, PathBuf};

// one KDM to a certificate that is no screen in Cinemas
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickKdm {
    pub title_id: TitleId,
    pub recipient: CertificateSource,
    pub trusted_devices: Vec<PathBuf>,
    pub media_block_serial: Option<String>,
    // wall clock times in time_zone
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub time_zone: String,
    pub formulation: Option<KdmFormulation>,
}

pub fn issue(
    database: &mut DistributionDatabase,
    settings: &Settings,
    data_dir: &Path,
    request: &QuickKdm,
    now: DateTime<Utc>,
) -> Result<CertificateIssueOutcome, String> {
    let issue_settings = settings.issue_settings(settings.output_dir(data_dir, None))?;
    let recipient = request.recipient.pem()?;
    let trusted_devices = request
        .trusted_devices
        .iter()
        .map(|path| CertificateSource::File { path: path.clone() }.pem())
        .collect::<Result<Vec<_>, _>>()?;
    let target = CertificateTarget {
        recipient: &recipient,
        trusted_devices: &trusted_devices,
        media_block_serial: request.media_block_serial.as_deref(),
        window: LocalWindow {
            start: request.start,
            end: request.end,
        },
        time_zone: &request.time_zone,
    };
    database.issue_to_certificate(
        DkdmSource::Title(request.title_id),
        &target,
        request.formulation,
        &issue_settings,
        now,
    )
}

#[tauri::command(async)]
pub fn quick_kdm_issue(
    request: QuickKdm,
    state: tauri::State<'_, AppState>,
) -> Result<CertificateIssueOutcome, String> {
    let settings = state.settings()?;
    state
        .with_database(|database| issue(database, &settings, &state.data_dir, &request, Utc::now()))
}
