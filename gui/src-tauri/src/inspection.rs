use postkit::certificate::FileInspection;
use std::path::PathBuf;

#[tauri::command(async)]
pub fn inspect_file(path: PathBuf) -> Result<FileInspection, String> {
    postkit::certificate::inspect_file(&path, chrono::Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{dkdm, fixtures, DCNC_TITLE};
    use postkit::certificate::SignatureVerification;

    #[test]
    fn a_tampered_kdm_inspects_with_its_signature_failing() {
        let f = fixtures();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tampered.xml");
        let edited = dkdm(f, DCNC_TITLE, 30).replacen(
            &format!(">{DCNC_TITLE}</ContentTitleText>"),
            ">Edited</ContentTitleText>",
            1,
        );
        std::fs::write(&path, edited).unwrap();

        let FileInspection::Kdm(inspection) = inspect_file(path).unwrap() else {
            panic!("a KDM must inspect as a KDM");
        };

        assert_eq!(inspection.metadata.content_title, "Edited");
        assert!(matches!(
            inspection.signature,
            SignatureVerification::Failed(_)
        ));
        assert_eq!(inspection.signer_chain.certificates.len(), 3);
    }
}
