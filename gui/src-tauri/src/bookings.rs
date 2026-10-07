use crate::settings::Settings;
use crate::state::AppState;
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use postkit::certificate::KdmFormulation;
use postkit::kdm_distribution::database::{
    BookingId, BookingIssueFailure, CinemaId, DistributionDatabase, IssueScope, ScreenId,
    StoredDelivery, TitleId,
};
use postkit::kdm_distribution::email::SmtpConfig;
use postkit::kdm_distribution::expiry::bookings_ending_within;
use postkit::kdm_distribution::issue::{DkdmIssueOutcome, IssuePlan};
use postkit::kdm_distribution::window::LocalWindow;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const ENDING_SOON_WITHIN: Duration = Duration::days(3);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookedScreen {
    pub id: ScreenId,
    pub cinema: String,
    pub screen: String,
    // no KDM yet, or the issued one no longer matches the booking or the screen
    pub pending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookingRow {
    pub id: BookingId,
    pub title_id: TitleId,
    pub content_title: String,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub formulation: Option<KdmFormulation>,
    pub screens: Vec<BookedScreen>,
    // the earliest end at a booked cinema within ENDING_SOON_WITHIN of now
    pub ends_soon_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewBooking {
    pub title_id: TitleId,
    pub screen_ids: Vec<ScreenId>,
    // wall clock times, read in each cinema's own time zone
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub formulation: Option<KdmFormulation>,
}

// the title stays, a different title is a different booking
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookingChange {
    pub screen_ids: Vec<ScreenId>,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub formulation: Option<KdmFormulation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueDestination {
    // None writes to the KDM folder from Settings
    pub output_folder: Option<PathBuf>,
    pub send_email: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueResult {
    pub outcome: DkdmIssueOutcome,
    pub deliveries: Vec<StoredDelivery>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookingIssueResult {
    pub booking_id: BookingId,
    pub content_title: String,
    pub outcome: DkdmIssueOutcome,
    pub deliveries: Vec<StoredDelivery>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CinemaIssueResult {
    pub issued: Vec<BookingIssueResult>,
    pub failed: Vec<BookingIssueFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookingPlanRow {
    pub booking_id: BookingId,
    pub plan: IssuePlan,
}

pub fn list(
    database: &DistributionDatabase,
    now: DateTime<Utc>,
) -> Result<Vec<BookingRow>, String> {
    let cinemas = database.cinemas()?;
    let ending_soon = bookings_ending_within(database, now, ENDING_SOON_WITHIN)?;
    let booked_screen = |id: ScreenId, pending: bool| {
        cinemas.iter().find_map(|stored| {
            let index = stored.screen_ids.iter().position(|screen| *screen == id)?;
            Some(BookedScreen {
                id,
                cinema: stored.cinema.name.clone(),
                screen: stored.cinema.screens[index].name.clone(),
                pending,
            })
        })
    };
    database
        .bookings()?
        .into_iter()
        .map(|booking| {
            let title = database.title(booking.title_id)?;
            Ok(BookingRow {
                id: booking.id,
                title_id: booking.title_id,
                content_title: title.content_title,
                start: booking.window.start,
                end: booking.window.end,
                formulation: booking.formulation,
                screens: booking
                    .screen_ids
                    .iter()
                    .filter_map(|id| booked_screen(*id, booking.pending_screen_ids.contains(id)))
                    .collect(),
                ends_soon_at: ending_soon
                    .iter()
                    .filter(|ending| ending.booking_id == booking.id)
                    .map(|ending| ending.ends_at)
                    .min(),
            })
        })
        .collect()
}

pub fn add(
    database: &mut DistributionDatabase,
    booking: NewBooking,
    now: DateTime<Utc>,
) -> Result<BookingId, String> {
    database.add_booking(
        booking.title_id,
        &booking.screen_ids,
        LocalWindow {
            start: booking.start,
            end: booking.end,
        },
        booking.formulation,
        now,
    )
}

pub fn update(
    database: &mut DistributionDatabase,
    id: BookingId,
    change: BookingChange,
) -> Result<(), String> {
    database.update_booking(
        id,
        &change.screen_ids,
        LocalWindow {
            start: change.start,
            end: change.end,
        },
        change.formulation,
    )
}

pub fn plan(
    database: &DistributionDatabase,
    settings: &Settings,
    data_dir: &Path,
    id: BookingId,
    scope: IssueScope,
    now: DateTime<Utc>,
) -> Result<IssuePlan, String> {
    let issue_settings = settings.issue_settings(settings.output_dir(data_dir, None))?;
    database.plan_booking(id, scope, &issue_settings, now)
}

fn email_server(settings: &Settings, send_email: bool) -> Result<Option<&SmtpConfig>, String> {
    match (send_email, &settings.smtp) {
        (false, _) => Ok(None),
        (true, Some(smtp)) => Ok(Some(smtp)),
        (true, None) => Err("set the SMTP server in Settings before emailing".to_string()),
    }
}

pub fn issue(
    database: &mut DistributionDatabase,
    settings: &Settings,
    data_dir: &Path,
    id: BookingId,
    scope: IssueScope,
    destination: IssueDestination,
    now: DateTime<Utc>,
) -> Result<IssueResult, String> {
    let smtp = email_server(settings, destination.send_email)?;
    let issue_settings =
        settings.issue_settings(settings.output_dir(data_dir, destination.output_folder))?;
    let outcome = database.issue_booking(id, scope, &issue_settings, now)?;
    let deliveries = database.deliver_bundles(&outcome, Some(id), smtp, now)?;
    Ok(IssueResult {
        outcome,
        deliveries,
    })
}

pub fn plan_cinema_pending(
    database: &DistributionDatabase,
    settings: &Settings,
    data_dir: &Path,
    cinema_id: CinemaId,
    now: DateTime<Utc>,
) -> Result<Vec<BookingPlanRow>, String> {
    let issue_settings = settings.issue_settings(settings.output_dir(data_dir, None))?;
    Ok(database
        .plan_cinema_pending(cinema_id, &issue_settings, now)?
        .into_iter()
        .map(|planned| BookingPlanRow {
            booking_id: planned.booking_id,
            plan: planned.plan,
        })
        .collect())
}

// every booking's pending screens at the cinema, one ZIP per booking
pub fn issue_cinema_pending(
    database: &mut DistributionDatabase,
    settings: &Settings,
    data_dir: &Path,
    cinema_id: CinemaId,
    send_email: bool,
    now: DateTime<Utc>,
) -> Result<CinemaIssueResult, String> {
    let smtp = email_server(settings, send_email)?;
    let issue_settings = settings.issue_settings(settings.output_dir(data_dir, None))?;
    let outcome = database.issue_cinema_pending(cinema_id, &issue_settings, now)?;
    let issued = outcome
        .issued
        .into_iter()
        .map(|issued| {
            let deliveries =
                database.deliver_bundles(&issued.outcome, Some(issued.booking_id), smtp, now)?;
            Ok(BookingIssueResult {
                booking_id: issued.booking_id,
                content_title: issued.outcome.content_title.clone(),
                outcome: issued.outcome,
                deliveries,
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(CinemaIssueResult {
        issued,
        failed: outcome.failed,
    })
}

#[tauri::command(async)]
pub fn bookings_list(state: tauri::State<'_, AppState>) -> Result<Vec<BookingRow>, String> {
    state.with_database(|database| list(database, Utc::now()))
}

#[tauri::command(async)]
pub fn bookings_add(
    booking: NewBooking,
    state: tauri::State<'_, AppState>,
) -> Result<BookingId, String> {
    state.with_database(|database| add(database, booking, Utc::now()))
}

#[tauri::command(async)]
pub fn bookings_update(
    id: BookingId,
    change: BookingChange,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.with_database(|database| update(database, id, change))
}

#[tauri::command(async)]
pub fn bookings_remove(id: BookingId, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.with_database(|database| database.remove_booking(id))
}

#[tauri::command(async)]
pub fn bookings_plan(
    id: BookingId,
    scope: IssueScope,
    state: tauri::State<'_, AppState>,
) -> Result<IssuePlan, String> {
    let settings = state.settings()?;
    state
        .with_database(|database| plan(database, &settings, &state.data_dir, id, scope, Utc::now()))
}

#[tauri::command(async)]
pub fn bookings_issue(
    id: BookingId,
    scope: IssueScope,
    output_folder: Option<PathBuf>,
    send_email: bool,
    state: tauri::State<'_, AppState>,
) -> Result<IssueResult, String> {
    let settings = state.settings()?;
    state.with_database(|database| {
        issue(
            database,
            &settings,
            &state.data_dir,
            id,
            scope,
            IssueDestination {
                output_folder,
                send_email,
            },
            Utc::now(),
        )
    })
}

#[tauri::command(async)]
pub fn cinemas_plan_pending(
    id: CinemaId,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<BookingPlanRow>, String> {
    let settings = state.settings()?;
    state.with_database(|database| {
        plan_cinema_pending(database, &settings, &state.data_dir, id, Utc::now())
    })
}

#[tauri::command(async)]
pub fn cinemas_issue_pending(
    id: CinemaId,
    send_email: bool,
    state: tauri::State<'_, AppState>,
) -> Result<CinemaIssueResult, String> {
    let settings = state.settings()?;
    state.with_database(|database| {
        issue_cinema_pending(
            database,
            &settings,
            &state.data_dir,
            id,
            send_email,
            Utc::now(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{
        booked_database, dkdm, extended_flm, fixtures, local_window, signed_settings, DCNC_TITLE,
    };
    use postkit::kdm_distribution::cinema::read_flm_cinema;
    use postkit::kdm_distribution::database::DeliveryResult;
    use postkit::kdm_distribution::window::kdm_window_in_time_zone;

    #[test]
    fn a_booking_lists_its_title_and_screens_by_name() {
        let (database, booking) = booked_database(fixtures());
        let rows = list(&database, Utc::now()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, booking);
        let names: Vec<(&str, &str)> = rows[0]
            .screens
            .iter()
            .map(|screen| (screen.cinema.as_str(), screen.screen.as_str()))
            .collect();
        assert_eq!(names, vec![("Rex", "1"), ("Rex", "2")]);
    }

    const WRITE_ONLY: IssueDestination = IssueDestination {
        output_folder: None,
        send_email: false,
    };

    fn pending_screens(database: &DistributionDatabase) -> Vec<bool> {
        list(database, Utc::now()).unwrap()[0]
            .screens
            .iter()
            .map(|screen| screen.pending)
            .collect()
    }

    #[test]
    fn a_window_edit_lists_every_issued_screen_as_pending_and_removal_keeps_the_outbox() {
        let directory = tempfile::tempdir().unwrap();
        let (mut database, booking) = booked_database(fixtures());
        let settings = signed_settings(fixtures(), directory.path());
        assert_eq!(pending_screens(&database), vec![true, true]);
        issue(
            &mut database,
            &settings,
            directory.path(),
            booking,
            IssueScope::PendingScreens,
            WRITE_ONLY,
            Utc::now(),
        )
        .unwrap();
        let row = list(&database, Utc::now()).unwrap().remove(0);
        assert_eq!(pending_screens(&database), vec![false, false]);
        let screen_ids: Vec<ScreenId> = row.screens.iter().map(|screen| screen.id).collect();
        update(
            &mut database,
            booking,
            BookingChange {
                screen_ids: screen_ids.clone(),
                start: row.start,
                end: row.end + chrono::Duration::hours(2),
                formulation: None,
            },
        )
        .unwrap();
        assert_eq!(pending_screens(&database), vec![true, true]);

        let reissued = issue(
            &mut database,
            &settings,
            directory.path(),
            booking,
            IssueScope::PendingScreens,
            WRITE_ONLY,
            Utc::now(),
        )
        .unwrap();
        assert_eq!(reissued.outcome.bundles[0].kdms.len(), 2);
        let window_end = reissued.outcome.bundles[0].kdms[0].not_valid_after.clone();
        assert!(
            window_end.starts_with(
                &(row.end + chrono::Duration::hours(2))
                    .format("%Y-%m-%dT%H:%M:%S")
                    .to_string()
            ),
            "{window_end}"
        );
        assert_eq!(pending_screens(&database), vec![false, false]);
        let again = issue(
            &mut database,
            &settings,
            directory.path(),
            booking,
            IssueScope::AllScreens,
            WRITE_ONLY,
            Utc::now(),
        )
        .unwrap();
        assert_eq!(again.outcome.bundles[0].kdms.len(), 2, "issue all again");

        database.remove_booking(booking).unwrap();
        assert!(list(&database, Utc::now()).unwrap().is_empty());
        let outbox = crate::outbox::outbox(&database).unwrap();
        assert_eq!(outbox.issues.len(), 6);
        assert_eq!(outbox.deliveries.len(), 3);
    }

    #[test]
    fn the_plan_shows_each_screens_formulation_before_anything_is_written() {
        let directory = tempfile::tempdir().unwrap();
        let (database, booking) = booked_database(fixtures());
        let settings = signed_settings(fixtures(), directory.path());
        let plan = plan(
            &database,
            &settings,
            directory.path(),
            booking,
            IssueScope::AllScreens,
            Utc::now(),
        )
        .unwrap();
        let formulations: Vec<Option<KdmFormulation>> = plan
            .screens
            .iter()
            .map(|screen| screen.formulation)
            .collect();
        assert_eq!(
            formulations,
            vec![
                Some(KdmFormulation::MultipleModifiedTransitional1),
                Some(KdmFormulation::ModifiedTransitional1)
            ]
        );
        assert!(!directory.path().join("outbox").exists());
    }

    #[test]
    fn issuing_without_smtp_writes_zips_and_emailing_without_smtp_is_refused() {
        let directory = tempfile::tempdir().unwrap();
        let (mut database, booking) = booked_database(fixtures());
        let mut settings = signed_settings(fixtures(), directory.path());
        settings.smtp = None;
        let error = issue(
            &mut database,
            &settings,
            directory.path(),
            booking,
            IssueScope::PendingScreens,
            IssueDestination {
                output_folder: None,
                send_email: true,
            },
            Utc::now(),
        )
        .unwrap_err();
        assert_eq!(error, "set the SMTP server in Settings before emailing");

        let result = issue(
            &mut database,
            &settings,
            directory.path(),
            booking,
            IssueScope::PendingScreens,
            WRITE_ONLY,
            Utc::now(),
        )
        .unwrap();
        assert_eq!(result.deliveries.len(), 1);
        assert_eq!(result.deliveries[0].record.result, DeliveryResult::Written);
        assert!(result.outcome.bundles[0]
            .zip_path
            .starts_with(directory.path().join("outbox")));
        assert_eq!(database.issues().unwrap().len(), 2);
    }

    #[test]
    fn a_booking_ending_within_three_days_shows_its_earliest_end_across_its_cinemas() {
        let f = fixtures();
        let directory = tempfile::tempdir().unwrap();
        let mut database = DistributionDatabase::open_in_memory().unwrap();
        let mut screens = Vec::new();
        for (name, zone) in [("Rex", "Europe/London"), ("Kino", "Asia/Tokyo")] {
            let flm = directory.path().join(format!("{name}.xml"));
            std::fs::write(&flm, extended_flm(f, name, zone)).unwrap();
            let cinema = database
                .save_cinema(&read_flm_cinema(&flm).unwrap())
                .unwrap()
                .cinema_id;
            screens.push(database.cinema(cinema).unwrap().screen_ids[0]);
        }
        let title = database
            .add_title_from_dkdm(&dkdm(f, DCNC_TITLE, 30))
            .unwrap();
        let window = local_window();
        database
            .add_booking(title, &screens, window, None, Utc::now())
            .unwrap();
        let end_in = |zone| kdm_window_in_time_zone(&window, zone).unwrap().end;
        let (london_end, tokyo_end) = (end_in("Europe/London"), end_in("Asia/Tokyo"));

        let ends_soon_at = |now| list(&database, now).unwrap()[0].ends_soon_at;
        assert_eq!(ends_soon_at(Utc::now()), None);
        assert_eq!(
            ends_soon_at(london_end - Duration::days(1)),
            Some(tokyo_end)
        );
        assert_eq!(ends_soon_at(tokyo_end), Some(london_end));
        assert_eq!(ends_soon_at(london_end), None);
    }
}
