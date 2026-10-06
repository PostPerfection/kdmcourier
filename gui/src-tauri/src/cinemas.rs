use crate::state::AppState;
use postkit::certificate::cert_info_from_pem;
use postkit::kdm_distribution::cinema::{read_flm_cinema, CinemaDb, Screen};
use postkit::kdm_distribution::database::{
    CinemaId, DistributionDatabase, ImportReport, ScreenId, StoredCinema,
};
use postkit::kdm_distribution::history;
use postkit::kdm_distribution::screen_checks::check_screen_certificates;
use postkit::kdm_distribution::window::check_time_zone;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRow {
    pub device_type: String,
    pub serial: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CertificateStatus {
    pub subject: String,
    pub not_after: String,
    pub failures: Vec<String>,
    pub warnings: Vec<String>,
    pub not_checked: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenRow {
    pub id: ScreenId,
    pub name: String,
    pub device_serial: Option<String>,
    pub devices: Vec<DeviceRow>,
    pub certificate: CertificateStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CinemaRow {
    pub id: CinemaId,
    pub name: String,
    pub facility_id: Option<String>,
    pub time_zone: Option<String>,
    pub emails: Vec<String>,
    pub screens: Vec<ScreenRow>,
}

fn certificate_status(
    label: &str,
    screen: &Screen,
    now: chrono::DateTime<chrono::Utc>,
) -> CertificateStatus {
    let report = check_screen_certificates(label, screen, now);
    let info = screen.cert.pem().and_then(|pem| cert_info_from_pem(&pem));
    CertificateStatus {
        subject: info
            .as_ref()
            .map(|info| info.subject_cn.clone())
            .unwrap_or_default(),
        not_after: info.map(|info| info.not_after).unwrap_or_default(),
        failures: report.failures.iter().map(ToString::to_string).collect(),
        warnings: report.warnings.iter().map(ToString::to_string).collect(),
        not_checked: report.not_checked,
    }
}

fn cinema_row(stored: StoredCinema, now: chrono::DateTime<chrono::Utc>) -> CinemaRow {
    let screens = stored
        .cinema
        .screens
        .iter()
        .zip(&stored.screen_ids)
        .map(|(screen, id)| ScreenRow {
            id: *id,
            name: screen.name.clone(),
            device_serial: screen.device_serial.clone(),
            devices: screen
                .authorized_devices
                .iter()
                .map(|device| DeviceRow {
                    device_type: device.device_type.clone(),
                    serial: device.serial.clone(),
                })
                .collect(),
            certificate: certificate_status(
                &format!("{} / {}", stored.cinema.name, screen.name),
                screen,
                now,
            ),
        })
        .collect();
    CinemaRow {
        id: stored.id,
        name: stored.cinema.name,
        facility_id: stored.cinema.facility_id,
        time_zone: stored.cinema.time_zone,
        emails: stored.cinema.emails,
        screens,
    }
}

pub fn list(
    database: &DistributionDatabase,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<CinemaRow>, String> {
    Ok(database
        .cinemas()?
        .into_iter()
        .map(|stored| cinema_row(stored, now))
        .collect())
}

// one line per file, the cinema and its screen count or why it was not imported
pub fn import_flm(database: &mut DistributionDatabase, paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| {
            let imported = read_flm_cinema(path).and_then(|cinema| {
                database.save_cinema(&cinema)?;
                Ok(format!(
                    "{}: {} ({} screens)",
                    path.display(),
                    cinema.name,
                    cinema.screens.len()
                ))
            });
            imported.unwrap_or_else(|error| format!("{}: {error}", path.display()))
        })
        .collect()
}

pub fn update(
    database: &mut DistributionDatabase,
    id: CinemaId,
    emails: Vec<String>,
    time_zone: Option<String>,
) -> Result<(), String> {
    if let Some(zone) = &time_zone {
        check_time_zone(zone)?;
    }
    let mut cinema = database.cinema(id)?.cinema;
    cinema.emails = emails;
    cinema.time_zone = time_zone;
    database.save_cinema(&cinema)?;
    Ok(())
}

pub fn import_dcpwizard_cinemas(
    database: &mut DistributionDatabase,
    path: &Path,
) -> Result<ImportReport, String> {
    database.import_cinema_database(&CinemaDb::load(path)?)
}

pub fn import_dcpwizard_history(
    database: &mut DistributionDatabase,
    path: &Path,
) -> Result<usize, String> {
    database.import_history(&history::read_all(path)?)
}

#[tauri::command(async)]
pub fn cinemas_list(state: tauri::State<'_, AppState>) -> Result<Vec<CinemaRow>, String> {
    state.with_database(|database| list(database, chrono::Utc::now()))
}

#[tauri::command(async)]
pub fn cinemas_import_flm(
    paths: Vec<PathBuf>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    state.with_database(|database| Ok(import_flm(database, &paths)))
}

#[tauri::command(async)]
pub fn cinemas_update(
    id: CinemaId,
    emails: Vec<String>,
    time_zone: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.with_database(|database| update(database, id, emails, time_zone))
}

#[tauri::command(async)]
pub fn cinemas_remove(id: CinemaId, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.with_database(|database| database.remove_cinema(id))
}

#[tauri::command(async)]
pub fn cinemas_import_dcpwizard(
    path: PathBuf,
    state: tauri::State<'_, AppState>,
) -> Result<ImportReport, String> {
    state.with_database(|database| import_dcpwizard_cinemas(database, &path))
}

#[tauri::command(async)]
pub fn history_import_dcpwizard(
    path: PathBuf,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    state.with_database(|database| import_dcpwizard_history(database, &path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{extended_flm, fixtures, SMPTE_EXAMPLE_FLM};

    #[test]
    fn an_flm_import_lists_screens_devices_and_certificate_status() {
        let directory = tempfile::tempdir().unwrap();
        let flm = directory.path().join("rex.xml");
        std::fs::write(&flm, extended_flm(fixtures(), "Rex", "Europe/London")).unwrap();
        let broken = directory.path().join("broken.xml");
        std::fs::write(&broken, "<something/>").unwrap();
        let mut database = DistributionDatabase::open_in_memory().unwrap();

        let lines = import_flm(
            &mut database,
            &[flm.clone(), PathBuf::from(SMPTE_EXAMPLE_FLM), broken],
        );
        assert!(lines[0].ends_with("Rex (2 screens)"), "{lines:?}");
        assert!(
            lines[1].ends_with("ExampleFacility Cinema (1 screens)"),
            "{lines:?}"
        );
        assert!(lines[2].contains("not an FLM document"), "{lines:?}");

        let rows = list(&database, chrono::Utc::now()).unwrap();
        let example = &rows[0];
        assert_eq!(example.time_zone.as_deref(), Some("Australia/Melbourne"));
        let failures = &example.screens[0].certificate.failures;
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("ST 430-2 rule 8 (role)")),
            "{failures:?}"
        );
        let rex = &rows[1];
        assert_eq!(rex.emails, vec!["kdm@rex.test"]);
        let screen = &rex.screens[0];
        assert_eq!(screen.device_serial.as_deref(), Some("1001"));
        assert_eq!(
            screen.devices,
            vec![
                DeviceRow {
                    device_type: "LD".into(),
                    serial: Some("2001".into())
                },
                DeviceRow {
                    device_type: "PR".into(),
                    serial: Some("3001".into())
                },
            ]
        );
        assert!(
            screen.certificate.failures.is_empty(),
            "{:?}",
            screen.certificate.failures
        );
        assert!(
            screen.certificate.subject.contains("SM."),
            "{}",
            screen.certificate.subject
        );
        assert!(screen
            .certificate
            .not_checked
            .iter()
            .any(|note| note.contains("rule 12")));
    }

    #[test]
    fn emails_and_time_zone_are_edited_and_a_wrong_zone_refused() {
        let directory = tempfile::tempdir().unwrap();
        let flm = directory.path().join("rex.xml");
        std::fs::write(&flm, extended_flm(fixtures(), "Rex", "Europe/London")).unwrap();
        let mut database = DistributionDatabase::open_in_memory().unwrap();
        import_flm(&mut database, &[flm]);
        let id = database.cinemas().unwrap()[0].id;

        update(
            &mut database,
            id,
            vec!["booking@rex.test".into()],
            Some("Europe/Dublin".into()),
        )
        .unwrap();
        let cinema = database.cinema(id).unwrap().cinema;
        assert_eq!(cinema.emails, vec!["booking@rex.test"]);
        assert_eq!(cinema.time_zone.as_deref(), Some("Europe/Dublin"));
        assert_eq!(cinema.screens.len(), 2, "the screens are kept");

        let error = update(&mut database, id, vec![], Some("Europe/Atlantis".into())).unwrap_err();
        assert!(error.contains("not an IANA time zone"), "{error}");
    }

    #[test]
    fn dcpwizard_files_import_with_what_was_skipped() {
        let f = fixtures();
        let directory = tempfile::tempdir().unwrap();
        let mut json = CinemaDb::default();
        json.add_cinema("Odeon", vec!["ops@odeon.test".into()], String::new())
            .unwrap();
        json.add_screen(
            "Odeon",
            "1",
            postkit::kdm_distribution::cinema::CertSource::Path(
                f.security_managers[0].certificate.clone(),
            ),
        )
        .unwrap();
        let json_path = directory.path().join("cinemas.json");
        json.save(&json_path).unwrap();
        let history_path = directory.path().join("kdm-history.jsonl");
        history::append(
            &history_path,
            &history::Record::now("cpl", "Feature", "SM", "4ca4", "a", "b", "/out.xml"),
        )
        .unwrap();

        let mut database = DistributionDatabase::open_in_memory().unwrap();
        let report = import_dcpwizard_cinemas(&mut database, &json_path).unwrap();
        assert_eq!((report.cinemas, report.screens), (1, 1));
        assert_eq!(
            import_dcpwizard_history(&mut database, &history_path).unwrap(),
            1
        );
        assert_eq!(database.issues().unwrap()[0].content_title, "Feature");
    }
}
