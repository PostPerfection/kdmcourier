use crate::settings::Settings;
use chrono::{Duration, Utc};
use postkit::certificate::{
    build_kdm, generate_certificate, generate_chain, CertOptions, CertType, KdmConfig,
    KdmContentKey,
};
use postkit::kdm_distribution::cinema::read_flm_cinema;
use postkit::kdm_distribution::database::{BookingId, DistributionDatabase};
use postkit::kdm_distribution::window::LocalWindow;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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
    }
}
