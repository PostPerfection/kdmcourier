use postkit::certificate::{AudioForensicMarking, PictureForensicMarking};
use postkit::kdm_distribution::certificate_fetch::{Credentials, Vendor};
use postkit::kdm_distribution::database::IssueSettings;
use postkit::kdm_distribution::email::{Security, SmtpConfig};
use postkit::kdm_distribution::flm_exchange::{Feed, FeedAuthentication};
use postkit::kdm_distribution::issue::KdmSigner;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const SETTINGS_FILE: &str = "settings.json";
const DATABASE_FILE: &str = "kdmcourier.sqlite";
const OUTBOX_DIRECTORY: &str = "outbox";
const CREDENTIALED_VENDORS: [Vendor; 3] = [Vendor::Christie, Vendor::Gdc, Vendor::Barco];

pub fn settings_path() -> PathBuf {
    postkit::preferences::config_dir(crate::APP_DIRECTORY_NAME).join(SETTINGS_FILE)
}

// written with owner-only permissions, it holds the SMTP, feed and vendor passwords as dcpwizard's smtp file does
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub database_path: Option<PathBuf>,
    pub signer_certificate: Option<PathBuf>,
    pub signer_key: Option<PathBuf>,
    // the CA certificates above the signer, intermediate first
    pub signer_chain: Vec<PathBuf>,
    // the private key the DKDMs from the mastering facility are addressed to
    pub dkdm_recipient_key: Option<PathBuf>,
    pub creation_facility: String,
    pub output_folder: Option<PathBuf>,
    pub smtp: Option<SmtpConfig>,
    pub feeds: Vec<StoredFeed>,
    pub vendor_accounts: Vec<StoredVendorAccount>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VendorAccount<Password> {
    pub vendor: Vendor,
    pub user: String,
    pub password: Password,
}

pub type StoredVendorAccount = VendorAccount<Option<String>>;
pub type VendorAccountView = VendorAccount<PasswordStatus>;
pub type VendorAccountUpdate = VendorAccount<PasswordChange>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedSetting<Password> {
    pub name: String,
    pub site_list_url: String,
    pub authentication: FeedAuthenticationSetting<Password>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum FeedAuthenticationSetting<Password> {
    None,
    Basic {
        username: String,
        password: Password,
    },
    ClientCertificate {
        certificate_chain: PathBuf,
        private_key: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PasswordStatus {
    pub set: bool,
}

pub type StoredFeed = FeedSetting<Option<String>>;
pub type FeedView = FeedSetting<PasswordStatus>;
pub type FeedUpdate = FeedSetting<PasswordChange>;

impl<Password> FeedSetting<Password> {
    fn with_password<Other>(self, password: impl FnOnce(Password) -> Other) -> FeedSetting<Other> {
        let authentication = match self.authentication {
            FeedAuthenticationSetting::None => FeedAuthenticationSetting::None,
            FeedAuthenticationSetting::Basic {
                username,
                password: current,
            } => FeedAuthenticationSetting::Basic {
                username,
                password: password(current),
            },
            FeedAuthenticationSetting::ClientCertificate {
                certificate_chain,
                private_key,
            } => FeedAuthenticationSetting::ClientCertificate {
                certificate_chain,
                private_key,
            },
        };
        FeedSetting {
            name: self.name,
            site_list_url: self.site_list_url,
            authentication,
        }
    }
}

impl StoredFeed {
    fn password(&self) -> Option<&String> {
        match &self.authentication {
            FeedAuthenticationSetting::Basic { password, .. } => password.as_ref(),
            _ => None,
        }
    }

    pub fn feed(&self) -> Result<Feed, String> {
        let authentication = match &self.authentication {
            FeedAuthenticationSetting::None => FeedAuthentication::None,
            FeedAuthenticationSetting::Basic { username, password } => FeedAuthentication::Basic {
                username: username.clone(),
                password: password
                    .clone()
                    .ok_or("set the feed's password in Settings before refreshing")?,
            },
            FeedAuthenticationSetting::ClientCertificate {
                certificate_chain,
                private_key,
            } => FeedAuthentication::ClientCertificate {
                certificate_chain: certificate_chain.clone(),
                private_key: private_key.clone(),
            },
        };
        Ok(Feed {
            site_list_url: self.site_list_url.clone(),
            authentication,
        })
    }
}

// the SMTP settings the page sees and edits, never the password
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmtpFields {
    pub host: String,
    pub port: u16,
    pub security: Security,
    pub username: Option<String>,
    pub from: String,
    pub body_template: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub database_path: PathBuf,
    pub signer_certificate: Option<PathBuf>,
    pub signer_key: Option<PathBuf>,
    pub signer_chain: Vec<PathBuf>,
    pub dkdm_recipient_key: Option<PathBuf>,
    pub creation_facility: String,
    pub output_folder: Option<PathBuf>,
    pub smtp: Option<SmtpFields>,
    pub smtp_password_set: bool,
    pub feeds: Vec<FeedView>,
    // one per vendor that needs an account, set or not
    pub vendor_accounts: Vec<VendorAccountView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "action")]
pub enum PasswordChange {
    Keep,
    Set { password: String },
    Clear,
}

impl PasswordChange {
    fn applied_to(self, kept: Option<String>) -> Option<String> {
        match self {
            PasswordChange::Keep => kept,
            PasswordChange::Set { password } => Some(password),
            PasswordChange::Clear => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdate {
    pub database_path: Option<PathBuf>,
    pub signer_certificate: Option<PathBuf>,
    pub signer_key: Option<PathBuf>,
    pub signer_chain: Vec<PathBuf>,
    pub dkdm_recipient_key: Option<PathBuf>,
    pub creation_facility: String,
    pub output_folder: Option<PathBuf>,
    pub smtp: Option<SmtpFields>,
    pub smtp_password: PasswordChange,
    pub feeds: Vec<FeedUpdate>,
    pub vendor_accounts: Vec<VendorAccountUpdate>,
}

impl Settings {
    pub fn load(path: &Path) -> Result<Settings, String> {
        let text = postkit::preferences::read_preferences_file(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let Some(text) = text else {
            return Ok(Settings::default());
        };
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        postkit::preferences::write_preferences_file(path, &json)
            .map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn database_path(&self, data_dir: &Path) -> PathBuf {
        self.database_path
            .clone()
            .unwrap_or_else(|| data_dir.join(DATABASE_FILE))
    }

    pub fn view(&self, data_dir: &Path) -> SettingsView {
        SettingsView {
            database_path: self.database_path(data_dir),
            signer_certificate: self.signer_certificate.clone(),
            signer_key: self.signer_key.clone(),
            signer_chain: self.signer_chain.clone(),
            dkdm_recipient_key: self.dkdm_recipient_key.clone(),
            creation_facility: self.creation_facility.clone(),
            output_folder: self.output_folder.clone(),
            smtp: self.smtp.as_ref().map(|smtp| SmtpFields {
                host: smtp.host.clone(),
                port: smtp.port,
                security: smtp.security,
                username: smtp.username.clone(),
                from: smtp.from.clone(),
                body_template: smtp.body_template.clone(),
            }),
            smtp_password_set: self
                .smtp
                .as_ref()
                .is_some_and(|smtp| smtp.password.is_some()),
            feeds: self
                .feeds
                .iter()
                .map(|feed| {
                    feed.clone().with_password(|password| PasswordStatus {
                        set: password.is_some(),
                    })
                })
                .collect(),
            vendor_accounts: CREDENTIALED_VENDORS
                .into_iter()
                .map(|vendor| {
                    let stored = self.vendor_account(vendor);
                    VendorAccount {
                        vendor,
                        user: stored
                            .map(|account| account.user.clone())
                            .unwrap_or_default(),
                        password: PasswordStatus {
                            set: stored.is_some_and(|account| account.password.is_some()),
                        },
                    }
                })
                .collect(),
        }
    }

    fn vendor_account(&self, vendor: Vendor) -> Option<&StoredVendorAccount> {
        self.vendor_accounts
            .iter()
            .find(|account| account.vendor == vendor)
    }

    // None for a vendor whose server needs no account
    pub fn vendor_credentials(&self, vendor: Vendor) -> Result<Option<Credentials>, String> {
        if !CREDENTIALED_VENDORS.contains(&vendor) {
            return Ok(None);
        }
        let missing = || format!("set the {vendor} account in Settings before fetching");
        let account = self.vendor_account(vendor).ok_or_else(missing)?;
        let password = account.password.clone().ok_or_else(missing)?;
        Ok(Some(Credentials {
            user: account.user.clone(),
            password,
        }))
    }

    pub fn updated(&self, update: SettingsUpdate) -> Settings {
        let password = update
            .smtp_password
            .applied_to(self.smtp.as_ref().and_then(|smtp| smtp.password.clone()));
        let kept_subject = self
            .smtp
            .as_ref()
            .and_then(|smtp| smtp.subject_template.clone());
        let feeds = update
            .feeds
            .into_iter()
            .map(|feed| {
                // a password never follows a feed to another server
                let kept_password = self
                    .feeds
                    .iter()
                    .find(|stored| stored.site_list_url == feed.site_list_url)
                    .and_then(StoredFeed::password)
                    .cloned();
                feed.with_password(|change| change.applied_to(kept_password))
            })
            .collect();
        // an account with no user is removed, its password with it
        let vendor_accounts = update
            .vendor_accounts
            .into_iter()
            .filter(|account| !account.user.trim().is_empty())
            .map(|account| {
                let kept_password = self
                    .vendor_account(account.vendor)
                    .and_then(|stored| stored.password.clone());
                VendorAccount {
                    vendor: account.vendor,
                    user: account.user.trim().to_string(),
                    password: account.password.applied_to(kept_password),
                }
            })
            .collect();
        Settings {
            database_path: update.database_path,
            signer_certificate: update.signer_certificate,
            signer_key: update.signer_key,
            signer_chain: update.signer_chain,
            dkdm_recipient_key: update.dkdm_recipient_key,
            creation_facility: update.creation_facility,
            output_folder: update.output_folder,
            smtp: update.smtp.map(|fields| SmtpConfig {
                host: fields.host,
                port: fields.port,
                security: fields.security,
                username: fields.username,
                password,
                from: fields.from,
                subject_template: kept_subject,
                body_template: fields.body_template,
            }),
            feeds,
            vendor_accounts,
        }
    }

    // the folder an issue writes to when the page names none
    pub fn output_dir(&self, data_dir: &Path, chosen: Option<PathBuf>) -> PathBuf {
        chosen
            .or_else(|| self.output_folder.clone())
            .unwrap_or_else(|| data_dir.join(OUTBOX_DIRECTORY))
    }

    pub fn issue_settings(&self, output_dir: PathBuf) -> Result<IssueSettings, String> {
        let missing = |what: &str| format!("set the {what} in Settings before issuing");
        let certificate = self
            .signer_certificate
            .clone()
            .ok_or_else(|| missing("signer certificate"))?;
        let key = self
            .signer_key
            .clone()
            .ok_or_else(|| missing("signer key"))?;
        let dkdm_recipient_key = self
            .dkdm_recipient_key
            .clone()
            .ok_or_else(|| missing("key the DKDMs are addressed to"))?;
        Ok(IssueSettings {
            signer: KdmSigner {
                certificate,
                key,
                chain: self.signer_chain.clone(),
            },
            dkdm_recipient_key,
            creation_facility: self.creation_facility.clone(),
            output_dir,
            picture_forensic_marking: PictureForensicMarking::default(),
            audio_forensic_marking: AudioForensicMarking::default(),
        })
    }
}

#[tauri::command(async)]
pub fn settings_load(
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<SettingsView, String> {
    Ok(state.settings()?.view(&state.data_dir))
}

#[tauri::command(async)]
pub fn settings_save(
    update: SettingsUpdate,
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<SettingsView, String> {
    state.save_settings(update)
}

#[tauri::command(async)]
pub fn time_zones() -> Vec<String> {
    postkit::kdm_distribution::window::time_zone_names()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn smtp_fields() -> SmtpFields {
        SmtpFields {
            host: "smtp.example.test".into(),
            port: 587,
            security: Security::Starttls,
            username: Some("kdm".into()),
            from: "kdm@distributor.test".into(),
            body_template: None,
        }
    }

    fn update(password: PasswordChange) -> SettingsUpdate {
        SettingsUpdate {
            database_path: None,
            signer_certificate: Some(PathBuf::from("/keys/signer.pem")),
            signer_key: Some(PathBuf::from("/keys/signer.key")),
            signer_chain: vec![PathBuf::from("/keys/intermediate.pem")],
            dkdm_recipient_key: Some(PathBuf::from("/keys/signer.key")),
            creation_facility: "DIS".into(),
            output_folder: None,
            smtp: Some(smtp_fields()),
            smtp_password: password,
            feeds: Vec::new(),
            vendor_accounts: Vec::new(),
        }
    }

    const FEED_URL: &str = "https://tms.exhibitor.test/sitelist.xml";

    fn basic_feed(site_list_url: &str, password: PasswordChange) -> FeedUpdate {
        FeedSetting {
            name: "Exhibitor".into(),
            site_list_url: site_list_url.into(),
            authentication: FeedAuthenticationSetting::Basic {
                username: "distributor".into(),
                password,
            },
        }
    }

    fn with_feeds(feeds: Vec<FeedUpdate>) -> SettingsUpdate {
        SettingsUpdate {
            feeds,
            ..update(PasswordChange::Keep)
        }
    }

    fn feed_password(settings: &Settings) -> Option<&str> {
        settings.feeds[0].password().map(String::as_str)
    }

    #[test]
    fn a_feed_password_is_stored_never_shown_and_kept_only_for_the_same_site_list() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(SETTINGS_FILE);
        let saved = Settings::default().updated(with_feeds(vec![basic_feed(
            FEED_URL,
            PasswordChange::Set {
                password: "hunter2".into(),
            },
        )]));
        saved.save(&path).unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert_eq!(loaded, saved);
        assert_eq!(feed_password(&loaded), Some("hunter2"));

        let view = loaded.view(directory.path());
        let view_json = serde_json::to_string(&view).unwrap();
        assert!(!view_json.contains("hunter2"), "{view_json}");
        assert_eq!(
            view.feeds[0].authentication,
            FeedAuthenticationSetting::Basic {
                username: "distributor".into(),
                password: PasswordStatus { set: true },
            }
        );

        let kept = loaded.updated(with_feeds(vec![basic_feed(FEED_URL, PasswordChange::Keep)]));
        assert_eq!(feed_password(&kept), Some("hunter2"));
        let moved = kept.updated(with_feeds(vec![basic_feed(
            "https://other.example.test/sitelist.xml",
            PasswordChange::Keep,
        )]));
        assert_eq!(feed_password(&moved), None);
        let cleared = kept.updated(with_feeds(vec![basic_feed(
            FEED_URL,
            PasswordChange::Clear,
        )]));
        assert_eq!(feed_password(&cleared), None);
        let error = cleared.feeds[0].feed().unwrap_err();
        assert!(error.contains("password"), "{error}");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn the_password_is_stored_but_never_shown_and_kept_until_changed() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(SETTINGS_FILE);
        let saved = Settings::default().updated(update(PasswordChange::Set {
            password: "hunter2".into(),
        }));
        saved.save(&path).unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert_eq!(loaded, saved);

        let view = serde_json::to_string(&loaded.view(directory.path())).unwrap();
        assert!(!view.contains("hunter2"), "{view}");
        assert!(loaded.view(directory.path()).smtp_password_set);

        let kept = loaded.updated(update(PasswordChange::Keep));
        assert_eq!(
            kept.smtp.as_ref().unwrap().password.as_deref(),
            Some("hunter2")
        );
        let cleared = kept.updated(update(PasswordChange::Clear));
        assert_eq!(cleared.smtp.unwrap().password, None);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    fn christie_account(password: PasswordChange) -> SettingsUpdate {
        SettingsUpdate {
            vendor_accounts: vec![VendorAccount {
                vendor: Vendor::Christie,
                user: " projectionist ".into(),
                password,
            }],
            ..update(PasswordChange::Keep)
        }
    }

    #[test]
    fn a_vendor_password_is_stored_never_shown_and_kept_until_changed() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(SETTINGS_FILE);
        let saved = Settings::default().updated(christie_account(PasswordChange::Set {
            password: "hunter2".into(),
        }));
        saved.save(&path).unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert_eq!(loaded, saved);
        let credentials = loaded
            .vendor_credentials(Vendor::Christie)
            .unwrap()
            .unwrap();
        assert_eq!(
            (credentials.user.as_str(), credentials.password.as_str()),
            ("projectionist", "hunter2")
        );

        let view = loaded.view(directory.path());
        let view_json = serde_json::to_string(&view).unwrap();
        assert!(!view_json.contains("hunter2"), "{view_json}");
        let shown: Vec<(Vendor, &str, bool)> = view
            .vendor_accounts
            .iter()
            .map(|account| (account.vendor, account.user.as_str(), account.password.set))
            .collect();
        assert_eq!(
            shown,
            [
                (Vendor::Christie, "projectionist", true),
                (Vendor::Gdc, "", false),
                (Vendor::Barco, "", false),
            ]
        );

        let kept = loaded.updated(christie_account(PasswordChange::Keep));
        assert!(kept.vendor_credentials(Vendor::Christie).unwrap().is_some());
        assert!(kept.vendor_credentials(Vendor::Dolby).unwrap().is_none());
        let error = kept.vendor_credentials(Vendor::Barco).unwrap_err();
        assert_eq!(error, "set the Barco account in Settings before fetching");
        let cleared = kept.updated(christie_account(PasswordChange::Clear));
        assert!(cleared.vendor_credentials(Vendor::Christie).is_err());
        let removed = kept.updated(update(PasswordChange::Keep));
        assert!(removed.vendor_accounts.is_empty());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn issuing_without_a_signer_names_the_missing_setting() {
        let error = Settings::default()
            .issue_settings(PathBuf::from("/out"))
            .unwrap_err();
        assert_eq!(
            error,
            "set the signer certificate in Settings before issuing"
        );
    }

    #[test]
    fn the_output_folder_is_the_chosen_one_then_the_setting_then_the_outbox() {
        let data_dir = Path::new("/data");
        let mut settings = Settings::default();
        assert_eq!(
            settings.output_dir(data_dir, None),
            data_dir.join(OUTBOX_DIRECTORY)
        );
        settings.output_folder = Some(PathBuf::from("/kdms"));
        assert_eq!(settings.output_dir(data_dir, None), PathBuf::from("/kdms"));
        assert_eq!(
            settings.output_dir(data_dir, Some(PathBuf::from("/today"))),
            PathBuf::from("/today")
        );
    }
}
