use crate::settings::{Settings, SettingsUpdate, SettingsView};
use postkit::kdm_distribution::database::DistributionDatabase;
use std::path::PathBuf;
use std::sync::Mutex;

struct OpenDatabase {
    path: PathBuf,
    database: DistributionDatabase,
}

pub struct AppState {
    pub settings_path: PathBuf,
    pub data_dir: PathBuf,
    database: Mutex<Option<OpenDatabase>>,
}

impl AppState {
    pub fn new(settings_path: PathBuf, data_dir: PathBuf) -> Self {
        Self {
            settings_path,
            data_dir,
            database: Mutex::new(None),
        }
    }

    pub fn settings(&self) -> Result<Settings, String> {
        Settings::load(&self.settings_path)
    }

    pub fn save_settings(&self, update: SettingsUpdate) -> Result<SettingsView, String> {
        let settings = self.settings()?.updated(update);
        settings.save(&self.settings_path)?;
        Ok(settings.view(&self.data_dir))
    }

    // the database the settings name, opened again when that location changes
    pub fn with_database<T>(
        &self,
        work: impl FnOnce(&mut DistributionDatabase) -> Result<T, String>,
    ) -> Result<T, String> {
        let path = self.settings()?.database_path(&self.data_dir);
        let mut open = self
            .database
            .lock()
            .map_err(|_| "the database is unusable after an earlier failure".to_string())?;
        let mut current = match open.take() {
            Some(current) if current.path == path => current,
            _ => OpenDatabase {
                database: DistributionDatabase::open(&path)?,
                path,
            },
        };
        let result = work(&mut current.database);
        *open = Some(current);
        result
    }
}
