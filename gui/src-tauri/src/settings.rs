use postkit::certificate::{AudioForensicMarking, PictureForensicMarking};
use postkit::kdm_distribution::database::IssueSettings;
use postkit::kdm_distribution::email::{Security, SmtpConfig};
use postkit::kdm_distribution::issue::KdmSigner;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const SETTINGS_FILE: &str = "settings.json";
const DATABASE_FILE: &str = "kdmcourier.sqlite";
const OUTBOX_DIRECTORY: &str = "outbox";

pub fn settings_path() -> PathBuf {
    postkit::preferences::config_dir(crate::APP_DIRECTORY_NAME).join(SETTINGS_FILE)
}

// written with owner-only permissions, it holds the SMTP password as dcpwizard's smtp file does
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "action")]
pub enum PasswordChange {
    Keep,
    Set { password: String },
    Clear,
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
        }
    }

    pub fn updated(&self, update: SettingsUpdate) -> Settings {
        let kept_password = self.smtp.as_ref().and_then(|smtp| smtp.password.clone());
        let password = match update.smtp_password {
            PasswordChange::Keep => kept_password,
            PasswordChange::Set { password } => Some(password),
            PasswordChange::Clear => None,
        };
        let kept_subject = self
            .smtp
            .as_ref()
            .and_then(|smtp| smtp.subject_template.clone());
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
