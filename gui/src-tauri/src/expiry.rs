use crate::settings::Settings;
use crate::state::AppState;
use chrono::{DateTime, Utc};
use postkit::kdm_distribution::database::DistributionDatabase;
use postkit::kdm_distribution::expiry::{expiry_report, ExpiryReport};

pub fn report(
    database: &DistributionDatabase,
    settings: &Settings,
    now: DateTime<Utc>,
) -> Result<ExpiryReport, String> {
    let signer_chain: Vec<_> = settings
        .signer_certificate
        .iter()
        .chain(&settings.signer_chain)
        .cloned()
        .collect();
    expiry_report(database, &signer_chain, now)
}

#[tauri::command(async)]
pub fn expiry_list(state: tauri::State<'_, AppState>) -> Result<ExpiryReport, String> {
    let settings = state.settings()?;
    state.with_database(|database| report(database, &settings, Utc::now()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{
        dkdm, extended_flm_with_first_recipient, fixtures, local_window,
        short_lived_security_manager, signed_settings, DCNC_TITLE,
    };
    use postkit::kdm_distribution::cinema::read_flm_cinema;
    use postkit::kdm_distribution::expiry::ExpiringItem;

    const SHORT_LIVED_DAYS: u32 = 5;
    const DKDM_DAYS: i64 = 30;

    #[test]
    fn a_recipient_certificate_ending_before_the_booking_is_listed_with_its_screen() {
        let f = fixtures();
        let directory = tempfile::tempdir().unwrap();
        let short_lived = short_lived_security_manager(directory.path(), SHORT_LIVED_DAYS);
        let flm = directory.path().join("rex.xml");
        std::fs::write(
            &flm,
            extended_flm_with_first_recipient(f, "Rex", "Europe/London", &short_lived.certificate),
        )
        .unwrap();
        let mut database = DistributionDatabase::open_in_memory().unwrap();
        let cinema = database
            .save_cinema(&read_flm_cinema(&flm).unwrap())
            .unwrap()
            .cinema_id;
        let screens = database.cinema(cinema).unwrap().screen_ids;
        let title = database
            .add_title_from_dkdm(&dkdm(f, DCNC_TITLE, DKDM_DAYS))
            .unwrap();
        let booking = database
            .add_booking(title, &screens, local_window(), None, Utc::now())
            .unwrap();

        let expiry = report(&database, &signed_settings(f, directory.path()), Utc::now()).unwrap();
        assert_eq!(expiry.bookings.len(), 1, "{:#?}", expiry.bookings);
        let listed = &expiry.bookings[0];
        assert_eq!(listed.booking_id, booking);
        assert_eq!(listed.cinema, "Rex");
        assert_eq!(listed.screen.as_deref(), Some("1"));
        assert_eq!(listed.item, ExpiringItem::Recipient);
        assert!(
            listed.subject.contains("SM.Vendor.IMB.1009"),
            "{}",
            listed.subject
        );
        assert!(listed.expires_at < listed.booking_ends_at);
        assert_eq!(
            expiry.signer_chain.len(),
            3,
            "the signer and the two CAs above it"
        );
        assert!(expiry
            .signer_chain
            .iter()
            .all(|certificate| certificate.bookings_ending_after.is_empty()));
    }
}
