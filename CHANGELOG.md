# Changelog

## Unreleased

### Added
- A Quick KDM view issues one KDM for a stored title to a certificate file or a certificate fetched by vendor and serial, with optional trusted devices, a window in a chosen time zone and a formulation, to the KDM folder from Settings. The booking certificate checks run first and a refusal names the rule and writes nothing. The KDM is listed in the Outbox. Each screen in Cinemas can replace its certificate from a file or fetch it by serial, and Settings holds a Christie, GDC and Barco account whose passwords the page never receives.
- An Inspect view opens a KDM or a PEM certificate file and shows what it holds without any private key: the KDM's title, CPL, window, recipient, key ids with their types, signature result and signer chain, and every certificate's full subject and issuer, validity, key, CA flag, key usage, thumbprint and roles with the ST 430-2 chain check. A tampered KDM still shows, with its signature failing. Each screen in Cinemas has the same certificate details under Certificate details.
- Settings take a list of SMPTE ST 430-15 FLM feeds, each a name, a SiteList URL and no login, a user and password, or a client certificate. Cinemas has a Refresh feeds button that imports every new or changed FLM from them and lists one line per feed, then each screen change and the bookings to reissue as Import FLM does. A cinema renamed in its FLM is updated in place by its facility id, and a facility gone from a feed is listed and kept.

## [1.0.1] - 2026-10-15

### Changed
- The macOS download is the `.dmg` alone, the release no longer carries an `.app.tar.gz`.

## [1.0.0] - 2026-10-07

- A booking whose window ends within 3 days at any of its cinemas shows "ends in 2 days", or hours under a day, in the Bookings State column, and each cinema shows how many of its bookings end soon, so a holdover can be booked by extending the end date, which marks the issued screens for reissue.
- Releases ship Linux (AppImage, deb, rpm), macOS (.app in a dmg) and Windows (NSIS and msi) bundles.
- A cinema with booked screens that need a KDM shows their count and an Issue pending button. It plans every booking's pending screens at that cinema, shows each booking's plan in the cinema's block, and Issue writes one ZIP per booking to the KDM folder from Settings and emails it when asked. A booking that fails is listed with its title and error, and the others still issue.
- Re-importing an FLM for a known cinema lists each screen added, removed, or with a changed recipient or authorized device certificate, and the bookings whose issued KDMs no longer match, which then need issuing again.
- The Outbox lists each delivery's title, and Resend mails a delivery's ZIP again to the cinema's addresses as they are now, recorded as a new delivery with its SMTP result. It needs an SMTP server in Settings and the ZIP still on disk.
- An Expiry view lists, for every booking still running, each recipient certificate, authorized device certificate and DKDM that expires before the booking ends at a cinema, and the signer chain's certificates with the titles of the bookings that end after them. A booking at a cinema with no usable time zone is listed as not checked.
- Bookings can be edited (screens, dates, formulation) and removed. Each booked screen with no KDM yet, or whose KDM no longer matches the booking's window and formulation or the screen's certificates, is marked in the Screens column, and the State column counts them, as in "2 of 3 screens need a KDM". Issue covers those screens and Issue all again covers every booked screen, both to the folder in the field and by email when asked. Check plans the pending screens, or every screen when none are pending. A removed booking's KDMs and ZIPs stay in the Outbox. The issue folder is a field that takes a typed path as well as the folder picker's. The app has its own icon, a key on an envelope.
- Titles, Cinemas, Bookings, Outbox and Settings views over postkit's `kdm_distribution`. A DKDM imports as a title. FLM files (ST 430-16 and ST 430-7) and DCP Wizard's cinema database import as cinemas, listed with their screens, time zone, authorized devices and certificate status, and a cinema's emails and time zone can be edited. A booking takes a title, screens and a local start and end, and Check shows each screen's formulation, window, warnings and refusals before anything is written. Issue writes one ZIP per cinema to a folder and emails it when asked, then lists each screen as issued or refused with the rule, and the Outbox lists every KDM and ZIP with the SMTP result. Settings holds the signer chain and key, the DKDM key, the creation facility code, the SMTP server and the database location, and never sends the SMTP password to the page.
