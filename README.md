# KDM Courier

KDM Courier gets a working KDM to every screen a distributor books a film into. It holds a DKDM per composition, imports cinemas from Facility List Messages, checks every certificate before a KDM is written, and writes or emails one ZIP per cinema with a record of each KDM. It is a thin Tauri app over postkit's `kdm_distribution`, where all of the KDM logic lives.

## Install

Download a package from the [latest release](https://github.com/PostPerfection/kdmcourier/releases/latest).

- Linux (x86_64): `sudo apt install ./KDM.Courier_*_amd64.deb` on Debian and Ubuntu, `sudo dnf install ./KDM.Courier-*.x86_64.rpm` on Fedora. The `.AppImage` needs `chmod +x` before it runs.
- macOS (Apple Silicon): open the `.dmg` and drag KDM Courier to Applications. The app is not notarized, so macOS blocks the first launch. Open System Settings, Privacy & Security, and click Open Anyway beside the KDM Courier message.
- Windows (x86_64): run the `-setup.exe` or the `.msi`. The installers are unsigned, so SmartScreen may stop them: click More info, then Run anyway.

## Build

Linux only for now. The submodules come first:

```bash
git submodule update --init --recursive
```

postkit links FFmpeg, which must be discoverable by pkg-config at build time, as for the wizards:

```bash
export PKG_CONFIG_PATH="/path/to/ffmpeg/lib/pkgconfig:$PKG_CONFIG_PATH"

cd gui
pnpm install
pnpm build
pnpm test

cd src-tauri
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
cargo deny check bans licenses sources
```

The icons come from `gui/icon-source/kdmcourier.svg`: `pnpm tauri icon icon-source/kdmcourier.svg -o <scratch folder>` in `gui`, then the desktop sizes listed in `tauri.conf.json` plus `icon.ico` and `icon.icns` copied into `gui/src-tauri/icons`.

`cargo build --release --features tauri/custom-protocol` in `gui/src-tauri` builds the release binary at `gui/src-tauri/target/release/kdmcourier-gui`. `cargo test` includes a check that drives the commands over IPC the way the page does: it imports a generated ST 430-16 FLM and the SMPTE example FLM, books a title from a generated DKDM, issues to a folder and unwraps the KDMs read back from the ZIP.

## Use

1. Settings: the signer certificate, its key and the CA certificates above it, the key the DKDMs from the mastering facility are addressed to, a three letter creation facility code, an SMTP server if KDMs go out by email, a Christie, GDC or Barco account for fetching their certificates, and the exhibitors' SMPTE ST 430-15 FLM feeds, each a SiteList URL with no login, a user and password, or a PEM client certificate and key.
2. Titles: Import DKDM adds a title with its CPL id, its standard read from the CPL title when it is an ISDCF name, and the DKDM's own window.
3. Cinemas: Import FLM reads SMPTE ST 430-16 and ST 430-7 FLMs. Each ST 430-16 suite is a screen, its SM device the KDM recipient and its other devices the ones the KDM lists. A cinema needs a time zone before it can be booked, from the FLM or typed in. Each screen shows its certificate chains checked against ST 430-2 rules 1 to 18 as ISDCF Doc 5 Annex A asks. Importing an FLM for a cinema already listed updates its screens by name and lists each screen added, removed or with changed certificates, and the bookings that need issuing again because of it. A cinema with booked screens that need a KDM shows their count, and Issue pending plans and then issues every booking's pending screens there, one ZIP per booking, to the KDM folder from Settings. A booking that fails is listed with its error and the others still issue. Refresh feeds reads every feed from Settings and imports each FLM that is new or changed since the last refresh, the same way as Import FLM. A cinema is matched by its facility id, so one renamed in its FLM is updated in place. It lists one line per feed, then the same screen and reissue lines as Import FLM. A facility gone from a feed is listed and its cinema kept. Feeds refresh only from this button. DCP Wizard's `cinemas.json` and `kdm-history.jsonl` import as they are. Each screen's File… replaces its recipient certificate with a picked PEM, and Fetch… downloads it by vendor and serial, with its chain where Dolby publishes one, both keeping the screen's devices and marking issued bookings for reissue. A certificate without its chain still gets KDMs, with a no chain warning on the screen and on each KDM, because the rules that need the issuer certificates cannot be checked.
4. Bookings: a title, the screens, and a start and end in each cinema's local time. Edit changes the screens, dates and formulation. A screen needs a KDM until one is issued, and again after a new window or formulation or a changed certificate, and the list marks those screens and counts them. A booking that ends within 3 days at any of its cinemas shows how many days or hours are left, and the Cinemas view counts those bookings per cinema, so a holdover can be decided while there is time to extend the end date and reissue. Remove deletes a booking and keeps its KDMs in the Outbox. Check shows each screen's formulation and window, any warning, and the screens that would be refused with the rule they fail, for the screens that need a KDM, or for every screen when none do. Issue covers the screens that need a KDM and Issue all again covers every screen. Both write to the folder in the field beside them, typed or picked, else to the KDM folder from Settings, one ZIP per cinema, named by the 2009 KDM Naming Convention with its KDMs beside it, and email it when asked, with the ZIP name as the subject.
5. Quick KDM: one KDM for a stored title to a certificate that is no screen in Cinemas, picked as a file or fetched by vendor and serial, with optional trusted device certificates, a start and end in a time zone and a formulation. The certificate checks a booking runs come first, and a refusal names each rule and writes nothing. The KDM goes to the KDM folder from Settings under the booking file name convention, and the Outbox lists it. Nothing is emailed.
6. Outbox: every KDM issued and every ZIP written or emailed, with its title and the SMTP result. Resend mails a ZIP again to the cinema's addresses as they are now.
7. Expiry: each recipient certificate, authorized device certificate and DKDM that expires before a running booking ends, and the signer chain's certificates with the bookings that end after them.
8. Inspect: open a KDM or a PEM certificate file to see what it holds without any private key. A KDM shows its title, CPL, window, recipient, key ids with their types, whether its signature verifies, and its signer chain. Every certificate shows its full subject and issuer, validity, key, CA flag, key usage, thumbprint and roles, with the ST 430-2 chain check. Each screen in Cinemas has the same certificate details under Certificate details.

Settings are kept in `kdmcourier/settings.json` under `~/.config` on Linux, `~/Library/Application Support` on macOS and `%APPDATA%` on Windows, with the SMTP, feed and vendor passwords in it, as DCP Wizard keeps its SMTP file. On Linux and macOS the file is readable by the user only. The page never receives a password. A feed's stored password is kept only while its SiteList URL stays the same. The database is `kdmcourier/kdmcourier.sqlite` under `~/.local/share` on Linux and the same folder as the settings on macOS and Windows, unless Settings names another. ZIPs go to `kdmcourier/outbox` beside it unless a folder is chosen. `XDG_CONFIG_HOME` moves the settings, and on Linux `XDG_DATA_HOME` moves the database and ZIPs.
