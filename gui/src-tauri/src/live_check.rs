// the page's whole issue path through the registered commands, as JSON over IPC
use crate::state::AppState;
use crate::test_fixtures::{
    content_keys, dkdm, dolby_certificate_path, dolby_listing, extended_flm, fixtures,
    local_window, site_list, vendor_servers, FeedServer, CPL_ID, DCNC_TITLE, DOLBY_LISTING_PATH,
    DOLBY_SERIAL, FEED_SITE_LIST_PATH, SMPTE_EXAMPLE_FLM,
};
use postkit::certificate::{parse_kdm, unwrap_kdm};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::io::Read;

const LOCAL_TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";

fn invoke<T: DeserializeOwned>(
    webview: &MockWebview,
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

type MockWebview = tauri::WebviewWindow<tauri::test::MockRuntime>;

fn app_state(directory: &std::path::Path) -> AppState {
    AppState::new(
        directory.join("config").join("settings.json"),
        directory.join("data"),
    )
}

fn mock_app(directory: &std::path::Path) -> (tauri::App<tauri::test::MockRuntime>, MockWebview) {
    mock_app_with(app_state(directory))
}

// the app must outlive its webview
fn mock_app_with(state: AppState) -> (tauri::App<tauri::test::MockRuntime>, MockWebview) {
    let app = crate::with_commands(tauri::test::mock_builder())
        .manage(state)
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    (app, webview)
}

#[test]
fn an_flm_a_dkdm_and_a_booking_issue_kdms_that_unwrap_to_the_content_keys() {
    let f = fixtures();
    let directory = tempfile::tempdir().unwrap();
    let data_dir = directory.path().join("data");
    let (_app, webview) = mock_app(directory.path());

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
            "smtpPassword": { "action": "keep" },
            "feeds": [],
            "vendorAccounts": []
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

#[test]
fn feeds_saved_from_the_page_refresh_into_cinemas_and_never_echo_the_password() {
    let directory = tempfile::tempdir().unwrap();
    let (_app, webview) = mock_app(directory.path());
    let server = FeedServer::start();
    server.serve(
        "/flm/Rex.xml",
        extended_flm(fixtures(), "Rex", "Europe/London"),
    );
    server.serve(
        FEED_SITE_LIST_PATH,
        site_list(&[("Rex", "2026-10-01T10:00:00Z")]),
    );
    let locked_url = server.url("/locked/sitelist.xml");

    let settings: Value = invoke(
        &webview,
        "settings_save",
        json!({ "update": {
            "databasePath": null,
            "signerCertificate": null,
            "signerKey": null,
            "signerChain": [],
            "dkdmRecipientKey": null,
            "creationFacility": "DIS",
            "outputFolder": null,
            "smtp": null,
            "smtpPassword": { "action": "keep" },
            "feeds": [
                {
                    "name": "Exhibitor",
                    "siteListUrl": server.url(FEED_SITE_LIST_PATH),
                    "authentication": { "kind": "none" }
                },
                {
                    "name": "Locked",
                    "siteListUrl": locked_url,
                    "authentication": {
                        "kind": "basic",
                        "username": "distributor",
                        "password": { "action": "set", "password": "hunter2" }
                    }
                }
            ],
            "vendorAccounts": []
        }}),
    )
    .unwrap();
    assert!(!settings.to_string().contains("hunter2"), "{settings}");
    assert_eq!(
        settings["feeds"][1]["authentication"],
        json!({ "kind": "basic", "username": "distributor", "password": { "set": true } })
    );

    let lines: Vec<String> = invoke(&webview, "cinemas_refresh_feeds", json!({})).unwrap();
    assert_eq!(
        lines,
        [
            "Exhibitor: 1 FLMs imported, 0 failed, 0 no longer listed".to_string(),
            "Rex: new cinema (2 screens)".to_string(),
            format!(
                "Locked: the feed {locked_url} has credentials, which are only sent over https"
            ),
        ]
    );
    let cinemas: Value = invoke(&webview, "cinemas_list", json!({})).unwrap();
    assert_eq!(cinemas[0]["name"], json!("Rex"));
}

#[test]
fn a_kdm_and_a_certificate_file_inspect_over_ipc() {
    let f = fixtures();
    let directory = tempfile::tempdir().unwrap();
    let (_app, webview) = mock_app(directory.path());
    let kdm_path = directory.path().join("dkdm.xml");
    std::fs::write(&kdm_path, dkdm(f, DCNC_TITLE, 30)).unwrap();
    let pem_path = directory.path().join("chain.pem");
    let chain_pem: String = std::iter::once(&f.distributor_signer)
        .chain(&f.distributor_chain)
        .map(|path| std::fs::read_to_string(path).unwrap())
        .collect();
    std::fs::write(&pem_path, chain_pem).unwrap();

    let kdm: Value = invoke(&webview, "inspect_file", json!({ "path": kdm_path })).unwrap();
    assert_eq!(kdm["kind"], json!("kdm"));
    assert_eq!(kdm["metadata"]["content_title"], json!(DCNC_TITLE));
    assert_eq!(kdm["signature"]["result"], json!("passed"));
    assert_eq!(kdm["key_ids"][0]["key_type"], json!("MDIK"));
    let signer_chain = kdm["signer_chain"]["certificates"].as_array().unwrap();
    assert_eq!(signer_chain.len(), 3);
    assert_eq!(signer_chain[0]["roles"], json!(["CS"]));

    let certificates: Value =
        invoke(&webview, "inspect_file", json!({ "path": pem_path })).unwrap();
    assert_eq!(certificates["kind"], json!("certificates"));
    assert_eq!(certificates["certificates"].as_array().unwrap().len(), 3);
    assert_eq!(certificates["check"]["failures"], json!([]));
}

fn vendor_chain_pem(leaf: &std::path::Path) -> String {
    let f = fixtures();
    [
        leaf,
        f.vendor_intermediate.as_path(),
        f.vendor_root.as_path(),
    ]
    .iter()
    .map(|path| std::fs::read_to_string(path).unwrap())
    .collect()
}

#[test]
fn a_fetched_lone_certificate_gets_a_quick_kdm_with_a_warning_and_a_wrong_one_is_refused() {
    let f = fixtures();
    let directory = tempfile::tempdir().unwrap();
    let server = FeedServer::start();
    server.serve(DOLBY_LISTING_PATH, dolby_listing());
    server.serve(
        &dolby_certificate_path(),
        std::fs::read_to_string(&f.security_managers[0].certificate).unwrap(),
    );
    let (_app, webview) =
        mock_app_with(app_state(directory.path()).with_vendor_servers(vendor_servers(&server)));
    let output = directory.path().join("kdms");
    let _: Value = invoke(
        &webview,
        "settings_save",
        json!({ "update": {
            "databasePath": null,
            "signerCertificate": f.distributor_signer,
            "signerKey": f.distributor_signer_key,
            "signerChain": f.distributor_chain,
            "dkdmRecipientKey": f.distributor_signer_key,
            "creationFacility": "DIS",
            "outputFolder": output,
            "smtp": null,
            "smtpPassword": { "action": "keep" },
            "feeds": [],
            "vendorAccounts": [
                { "vendor": "christie", "user": "projectionist", "password": { "action": "keep" } }
            ]
        }}),
    )
    .unwrap();
    let dkdm_path = directory.path().join("dkdm.xml");
    std::fs::write(&dkdm_path, dkdm(f, DCNC_TITLE, 30)).unwrap();
    let title: Value =
        invoke(&webview, "titles_import_dkdm", json!({ "path": dkdm_path })).unwrap();

    let refused = invoke::<Value>(
        &webview,
        "certificates_fetch",
        json!({ "vendor": "christie", "serial": "218281", "qubeType": null }),
    )
    .unwrap_err();
    assert_eq!(
        refused,
        json!("set the Christie account in Settings before fetching")
    );
    assert!(server.requested_paths().is_empty());

    let fetched: Value = invoke(
        &webview,
        "certificates_fetch",
        json!({ "vendor": "dolby", "serial": DOLBY_SERIAL, "qubeType": null }),
    )
    .unwrap();
    assert_eq!(fetched["subject"], json!("SM.Vendor.IMB.1001"));

    let window = local_window();
    let quick_kdm = |recipient: Value| {
        json!({ "request": {
            "titleId": title["id"],
            "recipient": recipient,
            "trustedDevices": [],
            "mediaBlockSerial": DOLBY_SERIAL,
            "start": window.start.format(LOCAL_TIME_FORMAT).to_string(),
            "end": window.end.format(LOCAL_TIME_FORMAT).to_string(),
            "timeZone": "Europe/London",
            "formulation": null
        }})
    };
    let issued: Value = invoke(
        &webview,
        "quick_kdm_issue",
        quick_kdm(json!({ "kind": "pem", "pem": fetched["pem"] })),
    )
    .unwrap();
    assert_eq!(issued["refusals"], json!([]));
    let warnings = issued["written"]["kdm"]["warnings"].to_string();
    assert!(warnings.contains("no chain"), "{warnings}");
    let path = issued["written"]["path"].as_str().unwrap();
    assert!(path.starts_with(output.to_str().unwrap()), "{path}");
    assert!(
        path.ends_with(&format!(
            "k_Courier_FTR_EN-XX_51-HI-VI_{DOLBY_SERIAL}_{}_{}_DIS_OV_US-13.xml",
            window.start.format("%Y%m%d"),
            window.end.format("%Y%m%d")
        )),
        "{path}"
    );
    let xml = std::fs::read_to_string(path).unwrap();
    let unwrapped = unwrap_kdm(&xml, &f.security_managers[0].key).unwrap();
    for key in content_keys() {
        assert_eq!(unwrapped.content_key(&key.key_id), Some(&key.content_key));
    }

    let wrong_role = directory.path().join("link-decryptor.pem");
    std::fs::write(&wrong_role, vendor_chain_pem(&f.link_decryptor.certificate)).unwrap();
    let refused: Value = invoke(
        &webview,
        "quick_kdm_issue",
        quick_kdm(json!({ "kind": "file", "path": wrong_role })),
    )
    .unwrap();
    assert_eq!(refused["written"], Value::Null);
    let reasons = refused["refusals"].to_string();
    assert!(reasons.contains("ST 430-2 rule 8 (role)"), "{reasons}");
    assert_eq!(std::fs::read_dir(&output).unwrap().count(), 1);

    let outbox: Value = invoke(&webview, "outbox_list", json!({})).unwrap();
    let issues = outbox["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0]["bookingId"], Value::Null);
    assert_eq!(issues[0]["fileName"], issued["written"]["kdm"]["fileName"]);
}

const OTHER_CPL_ID: &str = "0c0c0000-0000-0000-0000-000000000000";

// one reel per picture and sound KeyId pair
fn cpl_with_keys(cpl_id: &str, reels: &[(u128, u128)]) -> String {
    use postkit::packaging::{ns, DcpCpl, DcpCplReel};
    let key = |value: u128| Some(uuid::Uuid::from_u128(value).to_string());
    DcpCpl {
        uuid: cpl_id.to_string(),
        namespace: ns::CPL_SMPTE.into(),
        title: DCNC_TITLE.into(),
        reels: reels
            .iter()
            .map(|&(picture_key, sound_key)| DcpCplReel {
                reel_id: uuid::Uuid::new_v4().to_string(),
                picture_id: uuid::Uuid::new_v4().to_string(),
                picture_edit_rate_num: 24,
                picture_edit_rate_den: 1,
                picture_duration: 24,
                picture_width: 1998,
                picture_height: 1080,
                picture_key_id: key(picture_key),
                sound_id: Some(uuid::Uuid::new_v4().to_string()),
                sound_edit_rate_num: 24,
                sound_edit_rate_den: 1,
                sound_duration: 24,
                sound_key_id: key(sound_key),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
    .to_xml()
}

fn dcp_folder(directory: &std::path::Path, cpl_id: &str, reels: &[(u128, u128)]) {
    use postkit::packaging::{ns, AssetMap, AssetMapAsset};
    std::fs::create_dir_all(directory).unwrap();
    std::fs::write(directory.join("cpl.xml"), cpl_with_keys(cpl_id, reels)).unwrap();
    let assetmap = AssetMap {
        uuid: uuid::Uuid::new_v4().to_string(),
        namespace: ns::AM_SMPTE.into(),
        assets: vec![AssetMapAsset {
            id: cpl_id.to_string(),
            path: "cpl.xml".into(),
            packing_list: false,
        }],
        ..Default::default()
    };
    std::fs::write(directory.join("ASSETMAP.xml"), assetmap.to_xml()).unwrap();
}

#[test]
fn a_title_checks_its_dkdm_against_a_dcp_folder_or_a_cpl_file() {
    let f = fixtures();
    let directory = tempfile::tempdir().unwrap();
    let (_app, webview) = mock_app(directory.path());
    let dkdm_path = directory.path().join("dkdm.xml");
    std::fs::write(&dkdm_path, dkdm(f, DCNC_TITLE, 30)).unwrap();
    let title: Value =
        invoke(&webview, "titles_import_dkdm", json!({ "path": dkdm_path })).unwrap();
    let [picture_key, sound_key] = [content_keys()[0].key_id, content_keys()[1].key_id];
    let unknown_key = uuid::Uuid::from_u128(0x3333);

    let folder = directory.path().join("dcp");
    dcp_folder(
        &folder,
        CPL_ID,
        &[
            (picture_key.as_u128(), sound_key.as_u128()),
            (picture_key.as_u128(), unknown_key.as_u128()),
        ],
    );
    let coverage: Value = invoke(
        &webview,
        "titles_check_dcp",
        json!({ "id": title["id"], "path": folder }),
    )
    .unwrap();
    assert_eq!(coverage["cpl_id"], json!(CPL_ID));
    assert_eq!(coverage["encrypted_asset_count"], json!(4));
    assert_eq!(
        coverage["missing_keys"],
        json!([{ "reel": 2, "kind": "MainSound", "key_id": unknown_key }])
    );

    let covered = directory.path().join("covered.xml");
    std::fs::write(
        &covered,
        cpl_with_keys(CPL_ID, &[(picture_key.as_u128(), sound_key.as_u128())]),
    )
    .unwrap();
    let coverage: Value = invoke(
        &webview,
        "titles_check_dcp",
        json!({ "id": title["id"], "path": covered }),
    )
    .unwrap();
    assert_eq!(coverage["missing_keys"], json!([]));
    assert_eq!(coverage["unused_keys"], json!([]));
    assert_eq!(coverage["key_type_mismatches"], json!([]));

    let other = directory.path().join("other");
    dcp_folder(
        &other,
        OTHER_CPL_ID,
        &[(picture_key.as_u128(), sound_key.as_u128())],
    );
    let error = invoke::<Value>(
        &webview,
        "titles_check_dcp",
        json!({ "id": title["id"], "path": other }),
    )
    .unwrap_err();
    assert!(
        error
            .as_str()
            .unwrap()
            .contains(&format!("holds no CPL urn:uuid:{CPL_ID}")),
        "{error}"
    );
}
