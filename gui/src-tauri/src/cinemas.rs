use crate::bookings::ENDING_SOON_WITHIN;
use crate::settings::StoredFeed;
use crate::state::AppState;
use postkit::certificate::{cert_info_from_pem, inspect_certificate_pem, ChainInspection};
use postkit::kdm_distribution::cinema::{read_flm_cinema, CinemaDb, Screen};
use postkit::kdm_distribution::database::{
    CinemaId, CinemaSaveReport, DistributionDatabase, ImportReport, ScreenChange, ScreenId,
    StoredCinema,
};
use postkit::kdm_distribution::expiry::bookings_ending_within;
use postkit::kdm_distribution::flm_exchange::{apply_feed_refresh, refresh_feed, FeedImportReport};
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
    pub chain: Option<ChainInspection>,
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
    // booked screens across every booking that need a KDM issued
    pub pending_screens: usize,
    pub bookings_ending_soon: usize,
}

fn certificate_status(
    label: &str,
    screen: &Screen,
    now: chrono::DateTime<chrono::Utc>,
) -> CertificateStatus {
    let report = check_screen_certificates(label, screen, now);
    let pem = screen.cert.pem();
    let info = pem.clone().and_then(|pem| cert_info_from_pem(&pem));
    CertificateStatus {
        subject: info
            .as_ref()
            .map(|info| info.subject_cn.clone())
            .unwrap_or_default(),
        not_after: info.map(|info| info.not_after).unwrap_or_default(),
        failures: report.failures.iter().map(ToString::to_string).collect(),
        warnings: report.warnings.iter().map(ToString::to_string).collect(),
        not_checked: report.not_checked,
        chain: pem
            .ok()
            .and_then(|pem| inspect_certificate_pem(&pem, now).ok()),
    }
}

fn cinema_row(
    stored: StoredCinema,
    pending_screens: usize,
    bookings_ending_soon: usize,
    now: chrono::DateTime<chrono::Utc>,
) -> CinemaRow {
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
        pending_screens,
        bookings_ending_soon,
    }
}

pub fn list(
    database: &DistributionDatabase,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<CinemaRow>, String> {
    let ending_soon = bookings_ending_within(database, now, ENDING_SOON_WITHIN)?;
    database
        .cinemas()?
        .into_iter()
        .map(|stored| {
            let pending: usize = database
                .pending_screens_at_cinema(stored.id)?
                .iter()
                .map(|booking| booking.screen_ids.len())
                .sum();
            let ending_soon_here = ending_soon
                .iter()
                .filter(|ending| ending.cinema_id == stored.id)
                .count();
            Ok(cinema_row(stored, pending, ending_soon_here, now))
        })
        .collect()
}

// the rename, one line per changed screen, then the bookings to reissue
fn save_report_lines(
    database: &DistributionDatabase,
    cinema: &str,
    report: &CinemaSaveReport,
) -> Result<Vec<String>, String> {
    let rename = report
        .renamed_from
        .iter()
        .map(|old_name| format!("{old_name} is now {cinema}"));
    let mut lines: Vec<String> = rename
        .chain(report.screens.iter().map(|change| match change {
            ScreenChange::Added { screen } => format!("{cinema} / {screen}: new screen"),
            ScreenChange::Removed { screen } => format!("{cinema} / {screen}: removed"),
            ScreenChange::CertificatesChanged {
                screen,
                recipient,
                devices_changed,
            } => {
                let mut changes = Vec::new();
                if let Some(replacement) = recipient {
                    changes.push(format!(
                        "recipient certificate replaced, {} became {}",
                        replacement.old_thumbprint, replacement.new_thumbprint
                    ));
                }
                if *devices_changed {
                    changes.push("authorized device certificates changed".to_string());
                }
                format!("{cinema} / {screen}: {}", changes.join(", "))
            }
        }))
        .collect();
    if !report.bookings_to_reissue.is_empty() {
        let titles = report
            .bookings_to_reissue
            .iter()
            .map(|id| {
                Ok(database
                    .title(database.booking(*id)?.title_id)?
                    .content_title)
            })
            .collect::<Result<Vec<_>, String>>()?;
        lines.push(format!(
            "{cinema}: issued KDMs no longer match, reissue {}",
            titles.join(", ")
        ));
    }
    Ok(lines)
}

// per file its cinema and screen count or its error, then what changed on a known cinema
pub fn import_flm(database: &mut DistributionDatabase, paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .flat_map(|path| {
            let imported = read_flm_cinema(path).and_then(|cinema| {
                let report = database.save_cinema(&cinema)?;
                let mut lines = vec![format!(
                    "{}: {} ({} screens)",
                    path.display(),
                    cinema.name,
                    cinema.screens.len()
                )];
                if !report.created {
                    lines.extend(save_report_lines(database, &cinema.name, &report)?);
                }
                Ok(lines)
            });
            imported.unwrap_or_else(|error| vec![format!("{}: {error}", path.display())])
        })
        .collect()
}

fn feed_report_lines(
    database: &DistributionDatabase,
    feed_name: &str,
    report: &FeedImportReport,
) -> Result<Vec<String>, String> {
    let mut lines = vec![format!(
        "{feed_name}: {} FLMs imported, {} failed, {} no longer listed",
        report.imported.len(),
        report.failures.len(),
        report.removed_facility_ids.len()
    )];
    for facility in &report.imported {
        if facility.report.created {
            lines.push(format!(
                "{}: new cinema ({} screens)",
                facility.cinema,
                facility.report.screens.len()
            ));
        } else {
            lines.extend(save_report_lines(
                database,
                &facility.cinema,
                &facility.report,
            )?);
        }
    }
    lines.extend(
        report
            .failures
            .iter()
            .map(|failure| format!("{feed_name}: {}: {}", failure.facility_id, failure.reason)),
    );
    lines.extend(report.removed_facility_ids.iter().map(|facility_id| {
        format!("{feed_name}: {facility_id} is no longer listed, its cinema is kept")
    }));
    Ok(lines)
}

// the database stays usable while the feed is fetched
fn feed_lines(state: &AppState, feed: &StoredFeed) -> Result<Vec<String>, String> {
    let remote_feed = feed.feed()?;
    let previous = state.with_database(|database| database.feed_sync_state(&feed.site_list_url))?;
    let refresh = refresh_feed(&remote_feed, &previous)?;
    state.with_database(|database| {
        let report = apply_feed_refresh(database, &feed.site_list_url, &previous, refresh)?;
        feed_report_lines(database, &feed.name, &report)
    })
}

// per feed a summary or its error, then what changed on each cinema it imported
pub fn refresh_feeds(state: &AppState, feeds: &[StoredFeed]) -> Vec<String> {
    feeds
        .iter()
        .flat_map(|feed| {
            feed_lines(state, feed).unwrap_or_else(|error| vec![format!("{}: {error}", feed.name)])
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
pub fn cinemas_refresh_feeds(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(refresh_feeds(&state, &state.settings()?.feeds))
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
    use crate::bookings::{issue, issue_cinema_pending, IssueDestination};
    use crate::settings::{FeedAuthenticationSetting, FeedSetting};
    use crate::test_fixtures::{
        booked_database, dkdm, extended_flm, extended_flm_with_first_recipient, fixtures,
        local_window, signed_settings, site_list, FeedServer, DCNC_TITLE, FEED_SITE_LIST_PATH,
        SMPTE_EXAMPLE_FLM,
    };
    use chrono::{Duration, Utc};
    use postkit::kdm_distribution::database::IssueScope;
    use postkit::kdm_distribution::window::kdm_window_in_time_zone;
    use std::sync::Arc;

    fn pending_by_screen(database: &DistributionDatabase) -> Vec<(String, bool)> {
        crate::bookings::list(database, Utc::now()).unwrap()[0]
            .screens
            .iter()
            .map(|screen| {
                (
                    format!("{} / {}", screen.cinema, screen.screen),
                    screen.pending,
                )
            })
            .collect()
    }

    #[test]
    fn a_reimported_flm_with_a_new_recipient_flags_that_screen_and_issue_writes_one_zip() {
        let f = fixtures();
        let directory = tempfile::tempdir().unwrap();
        let settings = signed_settings(f, directory.path());
        let rex_flm = directory.path().join("rex.xml");
        let odeon_flm = directory.path().join("odeon.xml");
        std::fs::write(&rex_flm, extended_flm(f, "Rex", "Europe/London")).unwrap();
        std::fs::write(&odeon_flm, extended_flm(f, "Odeon", "America/New_York")).unwrap();
        let mut database = DistributionDatabase::open_in_memory().unwrap();
        let lines = import_flm(&mut database, &[rex_flm.clone(), odeon_flm]);
        assert_eq!(
            lines.len(),
            2,
            "a new cinema lists no screen changes: {lines:?}"
        );
        let screens: Vec<ScreenId> = database
            .cinemas()
            .unwrap()
            .iter()
            .flat_map(|stored| stored.screen_ids.clone())
            .collect();
        let title = database
            .add_title_from_dkdm(&dkdm(f, DCNC_TITLE, 30))
            .unwrap();
        let booking = database
            .add_booking(title, &screens, local_window(), None, Utc::now())
            .unwrap();
        let write_only = || IssueDestination {
            output_folder: None,
            send_email: false,
        };
        let first = issue(
            &mut database,
            &settings,
            directory.path(),
            booking,
            IssueScope::PendingScreens,
            write_only(),
            Utc::now(),
        )
        .unwrap();
        assert_eq!(first.outcome.bundles.len(), 2);

        let old_thumbprint = database.cinemas().unwrap()[1].cinema.screens[0]
            .cert_thumbprint
            .clone();
        std::fs::write(
            &rex_flm,
            extended_flm_with_first_recipient(
                f,
                "Rex",
                "Europe/London",
                &f.security_managers[2].certificate,
            ),
        )
        .unwrap();
        let lines = import_flm(&mut database, std::slice::from_ref(&rex_flm));
        let new_thumbprint = database.cinemas().unwrap()[1].cinema.screens[0]
            .cert_thumbprint
            .clone();
        assert_eq!(
            lines[1..],
            [
                format!(
                    "Rex / 1: recipient certificate replaced, {old_thumbprint} became {new_thumbprint}"
                ),
                format!("Rex: issued KDMs no longer match, reissue {DCNC_TITLE}"),
            ]
        );
        assert_eq!(
            pending_by_screen(&database),
            vec![
                ("Rex / 1".to_string(), true),
                ("Rex / 2".to_string(), false),
                ("Odeon / 1".to_string(), false),
                ("Odeon / 2".to_string(), false),
            ]
        );
        let rows = list(&database, Utc::now()).unwrap();
        let pending: Vec<(&str, usize)> = rows
            .iter()
            .map(|row| (row.name.as_str(), row.pending_screens))
            .collect();
        assert_eq!(pending, vec![("Odeon", 0), ("Rex", 1)]);

        let reissued = issue(
            &mut database,
            &settings,
            directory.path(),
            booking,
            IssueScope::PendingScreens,
            write_only(),
            Utc::now(),
        )
        .unwrap();
        let bundles = &reissued.outcome.bundles;
        assert_eq!(bundles.len(), 1);
        assert_eq!(bundles[0].cinema, "Rex");
        assert_eq!(bundles[0].kdms.len(), 1);
        assert_eq!(bundles[0].kdms[0].screen, "1");
        assert_eq!(bundles[0].kdms[0].recipient_thumbprint, new_thumbprint);
        assert_eq!(reissued.deliveries.len(), 1);
        assert!(pending_by_screen(&database)
            .iter()
            .all(|(_, pending)| !pending));

        std::fs::write(&rex_flm, extended_flm(f, "Rex", "Europe/London")).unwrap();
        import_flm(&mut database, &[rex_flm]);
        let rex_id = rows[1].id;
        let by_cinema = issue_cinema_pending(
            &mut database,
            &settings,
            directory.path(),
            rex_id,
            false,
            Utc::now(),
        )
        .unwrap();
        assert!(by_cinema.failed.is_empty(), "{:?}", by_cinema.failed);
        let by_cinema = by_cinema.issued;
        assert_eq!(by_cinema.len(), 1);
        assert_eq!(by_cinema[0].booking_id, booking);
        assert_eq!(by_cinema[0].outcome.bundles.len(), 1);
        assert_eq!(by_cinema[0].outcome.bundles[0].cinema, "Rex");
        assert_eq!(by_cinema[0].deliveries.len(), 1);
        assert_eq!(list(&database, Utc::now()).unwrap()[1].pending_screens, 0);
    }

    fn open_feed(name: &str, site_list_url: String) -> StoredFeed {
        FeedSetting {
            name: name.into(),
            site_list_url,
            authentication: FeedAuthenticationSetting::None,
        }
    }

    const FIRST_MODIFIED: &str = "2026-10-01T10:00:00Z";
    const SECOND_MODIFIED: &str = "2026-10-02T10:00:00Z";
    const COMMAND_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

    fn app_state(directory: &Path) -> AppState {
        AppState::new(directory.join("settings.json"), directory.join("data"))
    }

    fn rex_feed_server() -> FeedServer {
        let server = FeedServer::start();
        server.serve(
            "/flm/Rex.xml",
            extended_flm(fixtures(), "Rex", "Europe/London"),
        );
        server.serve(FEED_SITE_LIST_PATH, site_list(&[("Rex", FIRST_MODIFIED)]));
        server
    }

    #[test]
    fn a_feed_refresh_lists_each_feed_then_the_screen_changes_and_reissues() {
        let f = fixtures();
        let directory = tempfile::tempdir().unwrap();
        let server = rex_feed_server();
        let feeds = [
            open_feed("Exhibitor", server.url(FEED_SITE_LIST_PATH)),
            open_feed("Closed", server.url("/closed/sitelist.xml")),
        ];
        let state = app_state(directory.path());

        let lines = refresh_feeds(&state, &feeds);
        assert_eq!(
            lines[..2],
            [
                "Exhibitor: 1 FLMs imported, 0 failed, 0 no longer listed",
                "Rex: new cinema (2 screens)",
            ]
        );
        assert!(
            lines[2].starts_with("Closed: GET ") && lines[2].ends_with("returned HTTP 404"),
            "{lines:?}"
        );
        assert_eq!(lines.len(), 3);

        let rex = state
            .with_database(|database| {
                let rex = database.cinemas()?[0].clone();
                let title = database.add_title_from_dkdm(&dkdm(f, DCNC_TITLE, 30))?;
                let booking = database.add_booking(
                    title,
                    &rex.screen_ids,
                    local_window(),
                    None,
                    Utc::now(),
                )?;
                issue(
                    database,
                    &signed_settings(f, directory.path()),
                    directory.path(),
                    booking,
                    IssueScope::PendingScreens,
                    IssueDestination {
                        output_folder: None,
                        send_email: false,
                    },
                    Utc::now(),
                )?;
                Ok(rex)
            })
            .unwrap();

        server.serve(
            "/flm/Rex.xml",
            extended_flm_with_first_recipient(
                f,
                "Rex",
                "Europe/London",
                &f.security_managers[2].certificate,
            ),
        );
        server.serve(FEED_SITE_LIST_PATH, site_list(&[("Rex", SECOND_MODIFIED)]));
        let lines = refresh_feeds(&state, &feeds[..1]);
        let old_thumbprint = &rex.cinema.screens[0].cert_thumbprint;
        let new_thumbprint = state
            .with_database(|database| database.cinema(rex.id))
            .unwrap()
            .cinema
            .screens[0]
            .cert_thumbprint
            .clone();
        assert_eq!(
            lines,
            [
                "Exhibitor: 1 FLMs imported, 0 failed, 0 no longer listed".to_string(),
                format!(
                    "Rex / 1: recipient certificate replaced, {old_thumbprint} became {new_thumbprint}"
                ),
                format!("Rex: issued KDMs no longer match, reissue {DCNC_TITLE}"),
            ]
        );

        server.serve(FEED_SITE_LIST_PATH, site_list(&[]));
        let lines = refresh_feeds(&state, &feeds[..1]);
        assert_eq!(
            lines,
            [
                "Exhibitor: 0 FLMs imported, 0 failed, 1 no longer listed",
                "Exhibitor: urn:x-facilityID:example.com:Rex is no longer listed, its cinema is kept",
            ]
        );
        let stored = state.with_database(|database| database.cinemas()).unwrap();
        assert_eq!(stored.len(), 1);
    }

    #[test]
    fn the_cinema_list_answers_while_a_feed_refresh_waits_for_its_server() {
        let directory = tempfile::tempdir().unwrap();
        let server = rex_feed_server();
        let feeds = vec![open_feed("Exhibitor", server.url(FEED_SITE_LIST_PATH))];
        let state = Arc::new(app_state(directory.path()));
        let held = server.hold_next_response();

        let refreshing = {
            let state = state.clone();
            std::thread::spawn(move || refresh_feeds(&state, &feeds))
        };
        held.arrived
            .recv_timeout(COMMAND_WAIT)
            .expect("the refresh asks for the SiteList");
        let (listed_sender, listed) = std::sync::mpsc::channel();
        {
            let state = state.clone();
            std::thread::spawn(move || {
                let rows = state.with_database(|database| list(database, Utc::now()));
                listed_sender.send(rows.map(|rows| rows.len())).unwrap();
            });
        }
        let listed = listed
            .recv_timeout(COMMAND_WAIT)
            .expect("the cinema list answers while the SiteList is held");
        assert_eq!(listed, Ok(0));

        held.release.send(()).unwrap();
        let lines = refreshing.join().unwrap();
        assert_eq!(
            lines,
            [
                "Exhibitor: 1 FLMs imported, 0 failed, 0 no longer listed",
                "Rex: new cinema (2 screens)",
            ]
        );
    }

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
        let chain = screen.certificate.chain.as_ref().unwrap();
        let common_names: Vec<&str> = chain
            .certificates
            .iter()
            .map(|certificate| certificate.subject.rsplit_once(",CN=").unwrap().1)
            .collect();
        assert_eq!(
            common_names,
            [
                "SM.Vendor.IMB.1001",
                ".Vendor.smpte-430-2.INTERMEDIATE",
                ".Vendor.smpte-430-2.ROOT"
            ]
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

    #[test]
    fn a_cinema_counts_each_booking_ending_within_three_days_once() {
        let (mut database, booking) = booked_database(fixtures());
        let stored = database.booking(booking).unwrap();
        database
            .add_booking(
                stored.title_id,
                &stored.screen_ids[..1],
                stored.window,
                None,
                Utc::now(),
            )
            .unwrap();
        let rex_end = kdm_window_in_time_zone(&stored.window, "Europe/London")
            .unwrap()
            .end;

        let ending_soon = |now| list(&database, now).unwrap()[0].bookings_ending_soon;
        assert_eq!(ending_soon(Utc::now()), 0);
        assert_eq!(ending_soon(rex_end - Duration::days(1)), 2);
    }
}
