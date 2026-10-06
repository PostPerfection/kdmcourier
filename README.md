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
3. Cinemas: Import FLM reads SMPTE ST 430-16 and ST 430-7 FLMs. Each ST 430-16 suite is a screen, its SM device the KDM recipient and its other devices the ones the KDM lists. A cinema needs a time zone before it can be booked, from the FLM or typed in. Each screen shows its certificate chains checked against ST 430-2 rules 1 to 18 as ISDCF Doc 5 Annex A asks. DCP Wizard's `cinemas.json` and `kdm-history.jsonl` import as they are.
4. Bookings: a title, the screens, and a start and end in each cinema's local time. Edit changes the screens, dates and formulation, and a booking edited after its KDMs went out is marked to issue again. Remove deletes a booking and keeps its KDMs in the Outbox. Check shows each screen's formulation and window, any warning, and the screens that would be refused with the rule they fail. Issue writes to the folder in the field beside it, typed or picked, else to the KDM folder from Settings, one ZIP per cinema, named by the 2009 KDM Naming Convention with its KDMs beside it, and emails it when asked, with the ZIP name as the subject.
5. Outbox: every KDM issued and every ZIP written or emailed, with the SMTP result.

Settings are kept in `~/.config/kdmcourier/settings.json`, readable by the user only, with the SMTP password in it, as DCP Wizard keeps its SMTP file. The page never receives the password. The database is `~/.local/share/kdmcourier/kdmcourier.sqlite` unless Settings names another, and ZIPs go to `~/.local/share/kdmcourier/outbox` unless a folder is chosen. `XDG_CONFIG_HOME` and `XDG_DATA_HOME` move them.
