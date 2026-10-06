use crate::settings::Settings;
use crate::state::AppState;
use chrono::{DateTime, NaiveDateTime, Utc};
use postkit::certificate::KdmFormulation;
use postkit::kdm_distribution::database::{
    BookingId, DeliveryRecord, DistributionDatabase, ScreenId, TitleId,
};
use postkit::kdm_distribution::issue::{DkdmIssueOutcome, IssuePlan};
use postkit::kdm_distribution::window::LocalWindow;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookedScreen {
    pub id: ScreenId,
    pub cinema: String,
    pub screen: String,
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
    pub needs_reissue: bool,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueResult {
    pub outcome: DkdmIssueOutcome,
    pub deliveries: Vec<DeliveryRecord>,
}

pub fn list(database: &DistributionDatabase) -> Result<Vec<BookingRow>, String> {
    let cinemas = database.cinemas()?;
    let screen_names = |id: ScreenId| {
        cinemas.iter().find_map(|stored| {
            let index = stored.screen_ids.iter().position(|screen| *screen == id)?;
            Some(BookedScreen {
                id,
                cinema: stored.cinema.name.clone(),
                screen: stored.cinema.screens[index].name.clone(),
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
                    .into_iter()
                    .filter_map(screen_names)
                    .collect(),
                needs_reissue: booking.needs_reissue,
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
    now: DateTime<Utc>,
) -> Result<IssuePlan, String> {
    let issue_settings = settings.issue_settings(settings.output_dir(data_dir, None))?;
    database.plan_booking(id, &issue_settings, now)
}

pub fn issue(
    database: &mut DistributionDatabase,
    settings: &Settings,
    data_dir: &Path,
    id: BookingId,
    output_folder: Option<PathBuf>,
    send_email: bool,
    now: DateTime<Utc>,
) -> Result<IssueResult, String> {
    let smtp = match (send_email, &settings.smtp) {
        (false, _) => None,
        (true, Some(smtp)) => Some(smtp),
        (true, None) => return Err("set the SMTP server in Settings before emailing".to_string()),
    };
    let issue_settings = settings.issue_settings(settings.output_dir(data_dir, output_folder))?;
    let outcome = database.issue_booking(id, &issue_settings, now)?;
    let deliveries = database.deliver_bundles(&outcome, Some(id), smtp, now)?;
    Ok(IssueResult {
        outcome,
        deliveries,
    })
}

#[tauri::command(async)]
pub fn bookings_list(state: tauri::State<'_, AppState>) -> Result<Vec<BookingRow>, String> {
    state.with_database(|database| list(database))
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
    state: tauri::State<'_, AppState>,
) -> Result<IssuePlan, String> {
    let settings = state.settings()?;
    state.with_database(|database| plan(database, &settings, &state.data_dir, id, Utc::now()))
}

#[tauri::command(async)]
pub fn bookings_issue(
    id: BookingId,
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
            output_folder,
            send_email,
            Utc::now(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{booked_database, fixtures, signed_settings};
    use postkit::kdm_distribution::database::DeliveryResult;

    #[test]
    fn a_booking_lists_its_title_and_screens_by_name() {
        let (database, booking) = booked_database(fixtures());
        let rows = list(&database).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, booking);
        let names: Vec<(&str, &str)> = rows[0]
            .screens
            .iter()
            .map(|screen| (screen.cinema.as_str(), screen.screen.as_str()))
            .collect();
        assert_eq!(names, vec![("Rex", "1"), ("Rex", "2")]);
    }

    #[test]
    fn an_issued_booking_edited_lists_as_needing_a_reissue_and_removal_keeps_the_outbox() {
        let directory = tempfile::tempdir().unwrap();
        let (mut database, booking) = booked_database(fixtures());
        let settings = signed_settings(fixtures(), directory.path());
        issue(
            &mut database,
            &settings,
            directory.path(),
            booking,
            None,
            false,
            Utc::now(),
        )
        .unwrap();
        let row = list(&database).unwrap().remove(0);
        assert!(!row.needs_reissue);
        update(
            &mut database,
            booking,
            BookingChange {
                screen_ids: vec![row.screens[0].id],
                start: row.start,
                end: row.end + chrono::Duration::hours(2),
                formulation: None,
            },
        )
        .unwrap();
        let edited = list(&database).unwrap().remove(0);
        assert!(edited.needs_reissue);
        assert_eq!(edited.screens.len(), 1);

        database.remove_booking(booking).unwrap();
        assert!(list(&database).unwrap().is_empty());
        let outbox = crate::outbox::outbox(&database).unwrap();
        assert_eq!(outbox.issues.len(), 2);
        assert_eq!(outbox.deliveries.len(), 1);
    }

    #[test]
    fn the_plan_shows_each_screens_formulation_before_anything_is_written() {
        let directory = tempfile::tempdir().unwrap();
        let (database, booking) = booked_database(fixtures());
        let settings = signed_settings(fixtures(), directory.path());
        let plan = plan(&database, &settings, directory.path(), booking, Utc::now()).unwrap();
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
            None,
            true,
            Utc::now(),
        )
        .unwrap_err();
        assert_eq!(error, "set the SMTP server in Settings before emailing");

        let result = issue(
            &mut database,
            &settings,
            directory.path(),
            booking,
            None,
            false,
            Utc::now(),
        )
        .unwrap();
        assert_eq!(result.deliveries.len(), 1);
        assert_eq!(result.deliveries[0].result, DeliveryResult::Written);
        assert!(result.outcome.bundles[0]
            .zip_path
            .starts_with(directory.path().join("outbox")));
        assert_eq!(database.issues().unwrap().len(), 2);
    }
}
