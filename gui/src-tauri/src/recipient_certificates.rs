use crate::settings::Settings;
use crate::state::AppState;
use postkit::certificate::cert_info_from_pem;
use postkit::kdm_distribution::certificate_fetch::{
    fetch_pem, CertificateRequest, Vendor, VendorServers,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum CertificateSource {
    File { path: PathBuf },
    // a fetched certificate the page sends back
    Pem { pem: String },
}

impl CertificateSource {
    pub fn pem(&self) -> Result<String, String> {
        match self {
            CertificateSource::File { path } => std::fs::read_to_string(path)
                .map_err(|error| format!("cannot read certificate {}: {error}", path.display())),
            CertificateSource::Pem { pem } => Ok(pem.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchedCertificate {
    pub pem: String,
    pub subject: String,
    pub not_after: String,
}

pub fn fetch(
    settings: &Settings,
    servers: &VendorServers,
    vendor: Vendor,
    serial: &str,
    qube_type: Option<&str>,
) -> Result<FetchedCertificate, String> {
    let credentials = settings.vendor_credentials(vendor)?;
    let request = CertificateRequest {
        vendor,
        serial,
        qube_type,
        credentials: credentials.as_ref(),
    };
    let pem = fetch_pem(&request, servers)?;
    let info = cert_info_from_pem(&pem)?;
    Ok(FetchedCertificate {
        pem,
        subject: info.subject_cn,
        not_after: info.not_after,
    })
}

#[tauri::command(async)]
pub fn certificates_fetch(
    vendor: Vendor,
    serial: String,
    qube_type: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<FetchedCertificate, String> {
    fetch(
        &state.settings()?,
        &state.vendor_servers,
        vendor,
        &serial,
        qube_type.as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{PasswordChange, SettingsUpdate, VendorAccount};
    use crate::test_fixtures::{
        dolby_certificate_path, dolby_listing, empty_settings_update, fixtures, vendor_servers,
        FeedServer, DOLBY_LISTING_PATH, DOLBY_SERIAL,
    };

    #[test]
    fn a_dolby_serial_fetches_its_certificate_and_bytes_that_are_not_one_are_refused() {
        let pem = std::fs::read_to_string(&fixtures().security_managers[0].certificate).unwrap();
        let server = FeedServer::start();
        server.serve(DOLBY_LISTING_PATH, dolby_listing());
        server.serve(&dolby_certificate_path(), pem.clone());
        let servers = vendor_servers(&server);
        let fetched = fetch(
            &Settings::default(),
            &servers,
            Vendor::Dolby,
            DOLBY_SERIAL,
            None,
        )
        .unwrap();
        assert_eq!(fetched.pem, pem);
        assert_eq!(fetched.subject, "SM.Vendor.IMB.1001");

        server.serve(&dolby_certificate_path(), "<html>maintenance</html>".into());
        assert!(fetch(
            &Settings::default(),
            &servers,
            Vendor::Dolby,
            DOLBY_SERIAL,
            None
        )
        .is_err());
    }

    #[test]
    fn a_vendor_without_a_stored_password_is_refused_before_curl_runs() {
        let server = FeedServer::start();
        let settings = Settings::default().updated(SettingsUpdate {
            vendor_accounts: vec![VendorAccount {
                vendor: Vendor::Gdc,
                user: "projectionist".into(),
                password: PasswordChange::Keep,
            }],
            ..empty_settings_update()
        });
        let error = fetch(
            &settings,
            &vendor_servers(&server),
            Vendor::Gdc,
            "H1234",
            None,
        )
        .unwrap_err();
        assert_eq!(error, "set the GDC account in Settings before fetching");
        assert!(server.requested_paths().is_empty());
    }
}
