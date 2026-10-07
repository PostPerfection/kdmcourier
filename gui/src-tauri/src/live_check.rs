// the page's whole issue path through the registered commands, as JSON over IPC
use crate::state::AppState;
use crate::test_fixtures::{
    content_keys, dkdm, extended_flm, fixtures, local_window, CPL_ID, DCNC_TITLE, SMPTE_EXAMPLE_FLM,
};
use postkit::certificate::{parse_kdm, unwrap_kdm};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::io::Read;

const LOCAL_TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";

fn invoke<T: DeserializeOwned>(
    webview: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    command: &str,
    arguments: Value,
) -> Result<T, Value> {
    let request = tauri::webview::InvokeRequest {
        cmd: command.into(),
        callback: tauri::ipc::CallbackFn(0),
        error: tauri::ipc::CallbackFn(1),
        url: webview.url().unwrap(),
        body: tauri::ipc::InvokeBody::Json(arguments),
        headers: Default::default(),
        invoke_key: tauri::test::INVOKE_KEY.to_string(),
    };
    tauri::test::get_ipc_response(webview, request).map(|body| {
        body.deserialize::<T>()
            .expect("the reply reads as the type asked for")
    })
}

fn zip_entries(path: &str) -> Vec<(String, String)> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    (0..archive.len())
        .map(|index| {
            let mut entry = archive.by_index(index).unwrap();
            let mut content = String::new();
            entry.read_to_string(&mut content).unwrap();
            (entry.name().to_string(), content)
        })
        .collect()
}

#[test]
fn an_flm_a_dkdm_and_a_booking_issue_kdms_that_unwrap_to_the_content_keys() {
    let f = fixtures();
    let directory = tempfile::tempdir().unwrap();
    let data_dir = directory.path().join("data");
    let app = crate::with_commands(tauri::test::mock_builder())
        .manage(AppState::new(
            directory.path().join("config").join("settings.json"),
            data_dir.clone(),
        ))
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();

    let settings: Value = invoke(
        &webview,
        "settings_save",
        json!({ "update": {
            "databasePath": null,
            "signerCertificate": f.distributor_signer,
            "signerKey": f.distributor_signer_key,
            "signerChain": f.distributor_chain,
            "dkdmRecipientKey": f.distributor_signer_key,
            "creationFacility": "DIS",
            "outputFolder": null,
            "smtp": null,
            "smtpPassword": { "action": "keep" }
        }}),
    )
    .unwrap();
    assert_eq!(
        settings["databasePath"],
        json!(data_dir.join("kdmcourier.sqlite"))
    );

    let flm = directory.path().join("rex.xml");
    std::fs::write(&flm, extended_flm(f, "Rex", "Europe/London")).unwrap();
    let imported: Vec<String> = invoke(
        &webview,
        "cinemas_import_flm",
        json!({ "paths": [flm, SMPTE_EXAMPLE_FLM] }),
    )
    .unwrap();
    assert!(imported[0].ends_with("Rex (2 screens)"), "{imported:?}");

    let dkdm_path = directory.path().join("dkdm.xml");
    std::fs::write(&dkdm_path, dkdm(f, DCNC_TITLE, 30)).unwrap();
    let title: Value =
        invoke(&webview, "titles_import_dkdm", json!({ "path": dkdm_path })).unwrap();
    assert_eq!(title["cplId"], json!(CPL_ID));
    assert_eq!(title["standard"], json!("Smpte"));

    let cinemas: Value = invoke(&webview, "cinemas_list", json!({})).unwrap();
    let screen_ids: Vec<Value> = cinemas
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|cinema| cinema["screens"].as_array().unwrap().clone())
        .map(|screen| screen["id"].clone())
        .collect();
    assert_eq!(screen_ids.len(), 3);

    let window = local_window();
    let booking: Value = invoke(
        &webview,
        "bookings_add",
        json!({ "booking": {
            "titleId": title["id"],
            "screenIds": screen_ids,
            "start": window.start.format(LOCAL_TIME_FORMAT).to_string(),
            "end": window.end.format(LOCAL_TIME_FORMAT).to_string(),
            "formulation": null
        }}),
    )
    .unwrap();

    let plan: Value = invoke(
        &webview,
        "bookings_plan",
        json!({ "id": booking, "scope": "pendingScreens" }),
    )
    .unwrap();
    let planned = plan["screens"].as_array().unwrap();
    let planned_for = |cinema: &str, screen: &str| {
        planned
            .iter()
            .find(|row| row["cinema"] == json!(cinema) && row["screen"] == json!(screen))
            .unwrap_or_else(|| panic!("{cinema} / {screen} is planned"))
            .clone()
    };
    let refusals = planned_for("ExampleFacility Cinema", "1")["refusals"].to_string();
    assert!(refusals.contains("ST 430-2 rule 8 (role)"), "{refusals}");
    assert_eq!(
        planned_for("Rex", "1")["formulation"],
        json!("multiple-modified-transitional-1")
    );

    let output = directory.path().join("kdms");
    let issued: Value = invoke(
        &webview,
        "bookings_issue",
        json!({
            "id": booking, "scope": "pendingScreens", "outputFolder": output, "sendEmail": false
        }),
    )
    .unwrap();
    let outcome = &issued["outcome"];
    assert_eq!(outcome["refused"].as_array().unwrap().len(), 1);
    let bundles = outcome["bundles"].as_array().unwrap();
    assert_eq!(bundles.len(), 1);
    let zip_path = bundles[0]["zipPath"].as_str().unwrap();
    assert!(zip_path.starts_with(output.to_str().unwrap()));
    let entries = zip_entries(zip_path);
    assert_eq!(entries.len(), 2);
    let local_start = window.start.format(LOCAL_TIME_FORMAT).to_string();
    for ((name, xml), manager) in entries.iter().zip(&f.security_managers) {
        assert!(
            name.starts_with("k_Courier_FTR_EN-XX_51-HI-VI_100"),
            "{name}"
        );
        let metadata = parse_kdm(xml).unwrap();
        assert_eq!(metadata.cpl_id.to_string(), CPL_ID);
        assert!(metadata.not_valid_before.starts_with(&local_start));
        let unwrapped = unwrap_kdm(xml, &manager.key).unwrap();
        for key in content_keys() {
            assert_eq!(unwrapped.content_key(&key.key_id), Some(&key.content_key));
        }
    }
    assert_eq!(
        issued["deliveries"][0]["result"],
        json!({ "kind": "written" })
    );

    let outbox: Value = invoke(&webview, "outbox_list", json!({})).unwrap();
    assert_eq!(outbox["issues"].as_array().unwrap().len(), 2);
    assert_eq!(outbox["deliveries"].as_array().unwrap().len(), 1);

    let rex_screens: Vec<Value> = cinemas[1]["screens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|screen| screen["id"].clone())
        .collect();
    let _: Value = invoke(
        &webview,
        "bookings_update",
        json!({ "id": booking, "change": {
            "screenIds": rex_screens,
            "start": window.start.format(LOCAL_TIME_FORMAT).to_string(),
            "end": window.end.format(LOCAL_TIME_FORMAT).to_string(),
            "formulation": "modified-transitional-1"
        }}),
    )
    .unwrap();
    let bookings: Value = invoke(&webview, "bookings_list", json!({})).unwrap();
    let screens = bookings[0]["screens"].as_array().unwrap();
    assert_eq!(screens.len(), 2);
    assert!(
        screens
            .iter()
            .all(|screen| screen["pending"] == json!(true)),
        "a new formulation needs both issued screens reissued: {screens:?}"
    );
    let _: Value = invoke(&webview, "bookings_remove", json!({ "id": booking })).unwrap();
    let bookings: Value = invoke(&webview, "bookings_list", json!({})).unwrap();
    assert_eq!(bookings, json!([]));
    let outbox: Value = invoke(&webview, "outbox_list", json!({})).unwrap();
    assert_eq!(outbox["issues"].as_array().unwrap().len(), 2);

    let refused: Value = invoke::<Value>(
        &webview,
        "cinemas_update",
        json!({
            "id": cinemas[0]["id"], "emails": [], "timeZone": "Mars/Base"
        }),
    )
    .unwrap_err();
    assert!(
        refused.to_string().contains("not an IANA time zone"),
        "{refused}"
    );
}
