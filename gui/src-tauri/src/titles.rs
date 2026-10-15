use crate::state::AppState;
use postkit::certificate::parse_kdm;
use postkit::kdm_coverage::{check_kdm_coverage, find_cpl, read_cpl_keys, KdmCoverage};
use postkit::kdm_distribution::database::{DistributionDatabase, Title, TitleId};
use postkit::kdm_distribution::formulation::ContentStandard;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleRow {
    pub id: TitleId,
    pub cpl_id: String,
    pub content_title: String,
    pub standard: Option<ContentStandard>,
    pub dkdm_not_valid_before: String,
    pub dkdm_not_valid_after: String,
}

impl From<Title> for TitleRow {
    fn from(title: Title) -> Self {
        TitleRow {
            id: title.id,
            cpl_id: title.cpl_id,
            content_title: title.content_title,
            standard: title.standard,
            dkdm_not_valid_before: title.dkdm_not_valid_before,
            dkdm_not_valid_after: title.dkdm_not_valid_after,
        }
    }
}

pub fn list(database: &DistributionDatabase) -> Result<Vec<TitleRow>, String> {
    Ok(database.titles()?.into_iter().map(TitleRow::from).collect())
}

pub fn import_dkdm(database: &mut DistributionDatabase, path: &Path) -> Result<TitleRow, String> {
    let xml = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read DKDM {}: {error}", path.display()))?;
    let id = database.add_title_from_dkdm(&xml)?;
    Ok(database.title(id)?.into())
}

// a folder is searched for the DKDM's CPL, a file is read as the CPL
pub fn check_against_dcp(
    database: &DistributionDatabase,
    id: TitleId,
    path: &Path,
) -> Result<KdmCoverage, String> {
    let kdm = parse_kdm(&database.title(id)?.dkdm_xml)?;
    let cpl_path = if path.is_dir() {
        find_cpl(path, kdm.cpl_id)?
            .ok_or_else(|| format!("{} holds no CPL urn:uuid:{}", path.display(), kdm.cpl_id))?
    } else {
        path.to_path_buf()
    };
    let cpl = std::fs::read_to_string(&cpl_path)
        .map_err(|error| format!("cannot read CPL {}: {error}", cpl_path.display()))?;
    let keys = read_cpl_keys(&cpl).map_err(|error| format!("{}: {error}", cpl_path.display()))?;
    Ok(check_kdm_coverage(&kdm, &keys))
}

#[tauri::command(async)]
pub fn titles_list(state: tauri::State<'_, AppState>) -> Result<Vec<TitleRow>, String> {
    state.with_database(|database| list(database))
}

#[tauri::command(async)]
pub fn titles_import_dkdm(
    path: std::path::PathBuf,
    state: tauri::State<'_, AppState>,
) -> Result<TitleRow, String> {
    state.with_database(|database| import_dkdm(database, &path))
}

#[tauri::command(async)]
pub fn titles_set_standard(
    id: TitleId,
    standard: Option<ContentStandard>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.with_database(|database| database.set_title_standard(id, standard))
}

#[tauri::command(async)]
pub fn titles_check_dcp(
    id: TitleId,
    path: std::path::PathBuf,
    state: tauri::State<'_, AppState>,
) -> Result<KdmCoverage, String> {
    state.with_database(|database| check_against_dcp(database, id, &path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{dkdm, fixtures, CPL_ID, DCNC_TITLE};

    #[test]
    fn an_imported_dkdm_lists_with_its_cpl_standard_and_window() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("dkdm.xml");
        std::fs::write(&path, dkdm(fixtures(), DCNC_TITLE, 30)).unwrap();
        let mut database = DistributionDatabase::open_in_memory().unwrap();
        let row = import_dkdm(&mut database, &path).unwrap();
        assert_eq!(row.cpl_id, CPL_ID);
        assert_eq!(row.standard, Some(ContentStandard::Smpte));
        assert_eq!(row.dkdm_not_valid_before.len(), 25);
        assert_eq!(list(&database).unwrap(), vec![row]);
        let error = import_dkdm(&mut database, &directory.path().join("gone.xml")).unwrap_err();
        assert!(error.starts_with("cannot read DKDM"), "{error}");
    }
}
