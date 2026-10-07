# KDM Courier

KDM Courier gets a working KDM to every screen a distributor books a film into. It holds a DKDM per composition, imports cinemas from Facility List Messages, checks every certificate before a KDM is written, and writes or emails one ZIP per cinema with a record of each KDM. It is a thin Tauri app over postkit's `kdm_distribution`, where all of the KDM logic lives.

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

1. Settings: the signer certificate, its key and the CA certificates above it, the key the DKDMs from the mastering facility are addressed to, a three letter creation facility code, and an SMTP server if KDMs go out by email.
2. Titles: Import DKDM adds a title with its CPL id, its standard read from the CPL title when it is an ISDCF name, and the DKDM's own window.
3. Cinemas: Import FLM reads SMPTE ST 430-16 and ST 430-7 FLMs. Each ST 430-16 suite is a screen, its SM device the KDM recipient and its other devices the ones the KDM lists. A cinema needs a time zone before it can be booked, from the FLM or typed in. Each screen shows its certificate chains checked against ST 430-2 rules 1 to 18 as ISDCF Doc 5 Annex A asks. Importing an FLM for a cinema already listed updates its screens by name and lists each screen added, removed or with changed certificates, and the bookings that need issuing again because of it. A cinema with booked screens that need a KDM shows their count, and Issue pending plans and then issues every booking's pending screens there, one ZIP per booking, to the KDM folder from Settings. A booking that fails is listed with its error and the others still issue. DCP Wizard's `cinemas.json` and `kdm-history.jsonl` import as they are.
4. Bookings: a title, the screens, and a start and end in each cinema's local time. Edit changes the screens, dates and formulation. A screen needs a KDM until one is issued, and again after a new window or formulation or a changed certificate, and the list marks those screens and counts them. A booking that ends within 3 days at any of its cinemas shows how many days or hours are left, and the Cinemas view counts those bookings per cinema, so a holdover can be decided while there is time to extend the end date and reissue. Remove deletes a booking and keeps its KDMs in the Outbox. Check shows each screen's formulation and window, any warning, and the screens that would be refused with the rule they fail, for the screens that need a KDM, or for every screen when none do. Issue covers the screens that need a KDM and Issue all again covers every screen. Both write to the folder in the field beside them, typed or picked, else to the KDM folder from Settings, one ZIP per cinema, named by the 2009 KDM Naming Convention with its KDMs beside it, and email it when asked, with the ZIP name as the subject.
5. Outbox: every KDM issued and every ZIP written or emailed, with its title and the SMTP result. Resend mails a ZIP again to the cinema's addresses as they are now.
6. Expiry: each recipient certificate, authorized device certificate and DKDM that expires before a running booking ends, and the signer chain's certificates with the bookings that end after them.

Settings are kept in `~/.config/kdmcourier/settings.json`, readable by the user only, with the SMTP password in it, as DCP Wizard keeps its SMTP file. The page never receives the password. The database is `~/.local/share/kdmcourier/kdmcourier.sqlite` unless Settings names another, and ZIPs go to `~/.local/share/kdmcourier/outbox` unless a folder is chosen. `XDG_CONFIG_HOME` and `XDG_DATA_HOME` move them.
