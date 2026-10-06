# Changelog

## Unreleased

- Titles, Cinemas, Bookings, Outbox and Settings views over postkit's `kdm_distribution`. A DKDM imports as a title. FLM files (ST 430-16 and ST 430-7) and DCP Wizard's cinema database import as cinemas, listed with their screens, time zone, authorized devices and certificate status, and a cinema's emails and time zone can be edited. A booking takes a title, screens and a local start and end, and Check shows each screen's formulation, window, warnings and refusals before anything is written. Issue writes one ZIP per cinema to a folder and emails it when asked, then lists each screen as issued or refused with the rule, and the Outbox lists every KDM and ZIP with the SMTP result. Settings holds the signer chain and key, the DKDM key, the creation facility code, the SMTP server and the database location, and never sends the SMTP password to the page.
