use crate::settings::{PasswordChange, Settings, SettingsUpdate};
use chrono::{Duration, Utc};
use postkit::certificate::{
    build_kdm, generate_certificate, generate_chain, CertOptions, CertType, KdmConfig,
    KdmContentKey,
};
use postkit::kdm_distribution::certificate_fetch::VendorServers;
use postkit::kdm_distribution::cinema::read_flm_cinema;
use postkit::kdm_distribution::database::{BookingId, DistributionDatabase};
use postkit::kdm_distribution::window::LocalWindow;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};

pub const CPL_ID: &str = "8a2b1c3d-4e5f-6071-8293-a4b5c6d7e8f9";
pub const DCNC_TITLE: &str =
    "Courier_FTR-1_F-185_EN-XX_US-13_51-HI-VI_2K_STU_20261001_FAC_SMPTE_OV";
pub const SMPTE_EXAMPLE_FLM: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../extern/postkit/tests/fixtures/flm/st430-16b-2017.xml"
);
const VENDOR: &str = "Vendor";
const DISTRIBUTOR: &str = "Distributor";
const KDM_TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H:%M:%S+00:00";
const DEVICE_TYPE_SCOPE: &str =
    "http://www.smpte-ra.org/schemas/433/2008/dcmlTypes/#device-type-tokens";

pub struct Device {
    pub certificate: PathBuf,
    pub key: PathBuf,
}

pub struct Fixtures {
    _directory: tempfile::TempDir,
    pub vendor_root: PathBuf,
    pub vendor_intermediate: PathBuf,
    pub distributor_signer: PathBuf,
    pub distributor_signer_key: PathBuf,
    pub distributor_chain: Vec<PathBuf>,
    pub security_managers: Vec<Device>,
    pub link_decryptor: Device,
    pub projector: Device,
}

fn leaf(directory: &Path, stem: &str, common_name: &str) -> Device {
    let device = Device {
        certificate: directory.join(format!("{stem}.pem")),
        key: directory.join(format!("{stem}.key")),
    };
    let options = CertOptions {
        cert_type: CertType::Leaf,
        common_name: common_name.to_string(),
        organization: VENDOR.to_string(),
        output_cert: device.certificate.clone(),
        output_key: device.key.clone(),
        issuer_cert: directory.join("intermediate.pem"),
        issuer_key: directory.join("intermediate.key"),
        ..Default::default()
    };
    assert_eq!(generate_certificate(&options), 0, "{stem} certificate");
    device
}

// RSA key generation is slow, so every test shares one set
pub fn fixtures() -> &'static Fixtures {
    static FIXTURES: OnceLock<Fixtures> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        let directory = tempfile::tempdir().expect("tempdir");
        let vendor = directory.path().join("vendor");
        let distributor = directory.path().join("distributor");
        assert_eq!(generate_chain(VENDOR, &vendor), 0, "vendor chain");
        assert_eq!(
            generate_chain(DISTRIBUTOR, &distributor),
            0,
            "distributor chain"
        );
        Fixtures {
            vendor_root: vendor.join("root.pem"),
            vendor_intermediate: vendor.join("intermediate.pem"),
            distributor_signer: distributor.join("signer.pem"),
            distributor_signer_key: distributor.join("signer.key"),
            distributor_chain: vec![
                distributor.join("intermediate.pem"),
                distributor.join("root.pem"),
            ],
            security_managers: (1..=3)
                .map(|index| {
                    leaf(
                        &vendor,
                        &format!("sm{index}"),
                        &format!("SM.{VENDOR}.IMB.100{index}"),
                    )
                })
                .collect(),
            link_decryptor: leaf(&vendor, "ld", &format!("LD.{VENDOR}.LDB.2001")),
            projector: leaf(&vendor, "pr", &format!("PR.{VENDOR}.PRJ.3001")),
            _directory: directory,
        }
    })
}

pub fn short_lived_security_manager(directory: &Path, validity_days: u32) -> Device {
    let f = fixtures();
    let device = Device {
        certificate: directory.join("short-lived.pem"),
        key: directory.join("short-lived.key"),
    };
    let options = CertOptions {
        cert_type: CertType::Leaf,
        common_name: format!("SM.{VENDOR}.IMB.1009"),
        organization: VENDOR.to_string(),
        validity_days,
        output_cert: device.certificate.clone(),
        output_key: device.key.clone(),
        issuer_cert: f.vendor_intermediate.clone(),
        issuer_key: f.vendor_intermediate.with_extension("key"),
        ..Default::default()
    };
    assert_eq!(generate_certificate(&options), 0, "short-lived certificate");
    device
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

pub fn content_keys() -> Vec<KdmContentKey> {
    vec![
        KdmContentKey {
            key_type: *b"MDIK",
            key_id: uuid_from(0x1111),
            content_key: [0x11; 16],
        },
        KdmContentKey {
            key_type: *b"MDAK",
            key_id: uuid_from(0x2222),
            content_key: [0x22; 16],
        },
    ]
}

fn uuid_from(value: u128) -> uuid::Uuid {
    uuid::Uuid::from_u128(value)
}

// a DKDM to the distributor's own signer certificate, as a mastering facility would send
pub fn dkdm(f: &Fixtures, content_title: &str, days: i64) -> String {
    let start = Utc::now() + Duration::days(1);
    let end = start + Duration::days(days);
    build_kdm(&KdmConfig {
        cpl_id: CPL_ID.to_string(),
        content_title: content_title.to_string(),
        recipient_cert_file: f.distributor_signer.clone(),
        signer_cert_file: f.distributor_signer.clone(),
        signer_key_file: f.distributor_signer_key.clone(),
        signer_chain_files: f.distributor_chain.clone(),
        valid_from: start.format(KDM_TIMESTAMP_FORMAT).to_string(),
        valid_to: end.format(KDM_TIMESTAMP_FORMAT).to_string(),
        content_keys: content_keys(),
        ..Default::default()
    })
    .expect("DKDM")
    .xml
}

fn certificate_base64(pem: &str) -> String {
    pem.lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn device_xml(f: &Fixtures, device_type: &str, serial: &str, leaf: &Path) -> String {
    let certificates: String = [
        leaf,
        f.vendor_intermediate.as_path(),
        f.vendor_root.as_path(),
    ]
    .iter()
    .map(|path| {
        format!(
            "<ds:X509Data><ds:X509Certificate>{}</ds:X509Certificate></ds:X509Data>",
            certificate_base64(&read(path))
        )
    })
    .collect();
    format!(
        r#"<Device>
  <DeviceTypeID scope="{DEVICE_TYPE_SCOPE}">{device_type}</DeviceTypeID>
  <DeviceIdentifier idtype="DeviceUID">urn:uuid:{identifier}</DeviceIdentifier>
  <DeviceSerial>{serial}</DeviceSerial>
  <Manufacturer>{VENDOR}</Manufacturer>
  <ModelNumber>M-1</ModelNumber>
  <IsActive>true</IsActive>
  <KeyInfoList><ds:KeyInfo>{certificates}</ds:KeyInfo></KeyInfoList>
  <Capabilities/>
</Device>"#,
        identifier = uuid::Uuid::new_v4(),
    )
}

// a ST 430-16 facility: auditorium 1 is an SM with a link decryptor and projector, auditorium 2 an SM
pub fn extended_flm(f: &Fixtures, facility_name: &str, time_zone: &str) -> String {
    extended_flm_with_first_recipient(
        f,
        facility_name,
        time_zone,
        &f.security_managers[0].certificate,
    )
}

pub fn extended_flm_with_first_recipient(
    f: &Fixtures,
    facility_name: &str,
    time_zone: &str,
    first_recipient: &Path,
) -> String {
    let first_suite = [
        device_xml(f, "SM", "1001", first_recipient),
        device_xml(f, "LD", "2001", &f.link_decryptor.certificate),
        device_xml(f, "PR", "3001", &f.projector.certificate),
    ]
    .concat();
    let second_suite = device_xml(f, "SM", "1002", &f.security_managers[1].certificate);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<FacilityListMessage xmlns="http://www.smpte-ra.org/ns/430-16/2017/FLM" xmlns:ds="http://www.w3.org/2000/09/xmldsig#">
  <MessageId>urn:uuid:{message_id}</MessageId>
  <IssueDate>2026-10-01T10:00:00+00:00</IssueDate>
  <FacilityInfo>
    <FacilityID>urn:x-facilityID:example.com:{facility_name}</FacilityID>
    <FacilityName>{facility_name}</FacilityName>
    <FacilityTimeZone>{time_zone}</FacilityTimeZone>
    <Circuit>Independent</Circuit>
    <AddressList>
      <Physical>
        <StreetAddress>1 Screen Street</StreetAddress>
        <City>London</City>
        <Province>Greater London</Province>
        <Country>GB</Country>
      </Physical>
    </AddressList>
    <Capabilities>
      <KDMDeliveryMethodList><DeliveryMethod><Email><EmailAddress>kdm@rex.test</EmailAddress></Email></DeliveryMethod></KDMDeliveryMethodList>
    </Capabilities>
  </FacilityInfo>
  <AuditoriumList>
    <Auditorium><AuditoriumNumberOrName>1</AuditoriumNumberOrName><SuiteList><Suite>{first_suite}</Suite></SuiteList></Auditorium>
    <Auditorium><AuditoriumNumberOrName>2</AuditoriumNumberOrName><SuiteList><Suite>{second_suite}</Suite></SuiteList></Auditorium>
  </AuditoriumList>
</FacilityListMessage>"#,
        message_id = uuid::Uuid::new_v4(),
    )
}

// a certificate minted today cannot sign a window starting today, so bookings start in two days
pub fn local_window() -> LocalWindow {
    let today = Utc::now().date_naive();
    LocalWindow {
        start: (today + Duration::days(2)).and_hms_opt(18, 0, 0).unwrap(),
        end: (today + Duration::days(9)).and_hms_opt(23, 0, 0).unwrap(),
    }
}

// Rex from the generated FLM, booked on both screens for a title from a generated DKDM
pub fn booked_database(f: &Fixtures) -> (DistributionDatabase, BookingId) {
    let directory = tempfile::tempdir().unwrap();
    let flm = directory.path().join("rex.xml");
    std::fs::write(&flm, extended_flm(f, "Rex", "Europe/London")).unwrap();
    let mut database = DistributionDatabase::open_in_memory().unwrap();
    let cinema = database
        .save_cinema(&read_flm_cinema(&flm).unwrap())
        .unwrap()
        .cinema_id;
    let screens = database.cinema(cinema).unwrap().screen_ids;
    let title = database
        .add_title_from_dkdm(&dkdm(f, DCNC_TITLE, 30))
        .unwrap();
    let booking = database
        .add_booking(title, &screens, local_window(), None, Utc::now())
        .unwrap();
    (database, booking)
}

pub fn signed_settings(f: &Fixtures, data_dir: &Path) -> Settings {
    Settings {
        database_path: Some(data_dir.join("kdmcourier.sqlite")),
        signer_certificate: Some(f.distributor_signer.clone()),
        signer_key: Some(f.distributor_signer_key.clone()),
        signer_chain: f.distributor_chain.clone(),
        dkdm_recipient_key: Some(f.distributor_signer_key.clone()),
        creation_facility: "DIS".to_string(),
        output_folder: None,
        smtp: None,
        feeds: Vec::new(),
        vendor_accounts: Vec::new(),
    }
}

pub fn empty_settings_update() -> SettingsUpdate {
    SettingsUpdate {
        database_path: None,
        signer_certificate: None,
        signer_key: None,
        signer_chain: Vec::new(),
        dkdm_recipient_key: None,
        creation_facility: String::new(),
        output_folder: None,
        smtp: None,
        smtp_password: PasswordChange::Keep,
        feeds: Vec::new(),
        vendor_accounts: Vec::new(),
    }
}

pub const DOLBY_SERIAL: &str = "218281828";
pub const DOLBY_LISTING_PATH: &str = "/dolby/218xxx/";
const DOLBY_CERTIFICATE_FILE: &str = "Dolby-DCP2000-218281828.cert.sha256.pem";

// every vendor's base URL on the local server, dolby under /dolby
pub fn vendor_servers(server: &FeedServer) -> VendorServers {
    VendorServers {
        dolby: server.url("/dolby"),
        qube: server.url(""),
        christie: server.url(""),
        gdc: server.url(""),
        barco: server.url(""),
    }
}

pub fn dolby_listing() -> String {
    format!("Dolby-DCP2000-218281828.cert.mpeg.pem\n{DOLBY_CERTIFICATE_FILE}\n")
}

pub fn dolby_certificate_path() -> String {
    format!("{DOLBY_LISTING_PATH}{DOLBY_CERTIFICATE_FILE}")
}

pub const FEED_SITE_LIST_PATH: &str = "/feed/sitelist.xml";

// a SMPTE ST 430-15 SiteList linking each facility to /flm/<facility name>.xml
pub fn site_list(facilities: &[(&str, &str)]) -> String {
    let entries: String = facilities
        .iter()
        .map(|(facility_name, modified)| {
            format!(
                r#"<Facility id="urn:x-facilityID:example.com:{facility_name}" modified="{modified}" xlink:href="/flm/{facility_name}.xml" xlink:type="simple"/>"#
            )
        })
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<SiteList xmlns="http://www.smpte-ra.org/ns/430-15/2017/SiteList" xmlns:xlink="http://www.w3.org/1999/xlink">
  <Originator>https://feed.example.com/feed/sitelist.xml</Originator>
  <SystemName>Exhibitor TMS</SystemName>
  <DateTimeCreated>2026-10-01T10:00:00+00:00</DateTimeCreated>
  <FacilityList>{entries}</FacilityList>
</SiteList>"#
    )
}

// plain http on localhost, each path answers its stored body and anything else 404
pub struct FeedServer {
    origin: String,
    bodies: Arc<Mutex<HashMap<String, String>>>,
    requested_paths: Arc<Mutex<Vec<String>>>,
    held_response: Arc<Mutex<Option<ResponseGate>>>,
}

struct ResponseGate {
    arrived: Sender<()>,
    release: Receiver<()>,
}

// the server says when the held request arrived and answers it once released
pub struct HeldResponse {
    pub arrived: Receiver<()>,
    pub release: Sender<()>,
}

impl FeedServer {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let bodies = Arc::new(Mutex::new(HashMap::<String, String>::new()));
        let held_response = Arc::new(Mutex::new(None::<ResponseGate>));
        let requested_paths = Arc::new(Mutex::new(Vec::new()));
        let (served, gate, requested) = (
            bodies.clone(),
            held_response.clone(),
            requested_paths.clone(),
        );
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let mut request_line = String::new();
                let mut reader = BufReader::new(&mut stream);
                if reader.read_line(&mut request_line).is_err() {
                    continue;
                }
                let mut header = String::new();
                while reader.read_line(&mut header).is_ok_and(|read| read > 2) {
                    header.clear();
                }
                if let Some(gate) = gate.lock().unwrap().take() {
                    let _ = gate.arrived.send(());
                    let _ = gate.release.recv();
                }
                let path = request_line.split(' ').nth(1).unwrap_or_default();
                requested.lock().unwrap().push(path.to_string());
                let body = served.lock().unwrap().get(path).cloned();
                let (status, body) = match body {
                    Some(body) => ("200 OK", body),
                    None => ("404 Not Found", String::new()),
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        Self {
            origin,
            bodies,
            requested_paths,
            held_response,
        }
    }

    pub fn requested_paths(&self) -> Vec<String> {
        self.requested_paths.lock().unwrap().clone()
    }

    pub fn hold_next_response(&self) -> HeldResponse {
        let (arrived_sender, arrived) = channel();
        let (release, release_receiver) = channel();
        *self.held_response.lock().unwrap() = Some(ResponseGate {
            arrived: arrived_sender,
            release: release_receiver,
        });
        HeldResponse { arrived, release }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.origin)
    }

    pub fn serve(&self, path: &str, body: String) {
        self.bodies.lock().unwrap().insert(path.to_string(), body);
    }
}
