import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { escapeHtml } from "../../extern/guikit/src/html.js";
import { formatDateTime } from "../../extern/guikit/src/time-format.js";
import {
  certificateStatusText,
  cinemaEndingSoonText,
  cinemaPendingText,
  devicesText,
  emailsFromText,
} from "./cinema-rows.js";
import {
  ISSUE_SCOPE,
  bookedScreenText,
  bookingChange,
  bookingEndingText,
  bookingFields,
  bookingPendingText,
  newBooking,
  pendingScreenCount,
  planRows,
  planScope,
} from "./booking-form.js";
import { VENDORS, fetchRequest, fetchedText, vendorLabel } from "./certificate-fetch.js";
import { bookingExpiryRow, nothingExpires, signerChainRow } from "./expiry-rows.js";
import { cinemaIssueStatus, deliveryText, failedBookingText, issuedStatus, outcomeRows } from "./issue-outcome.js";
import { chainInspectionHtml, fileInspectionHtml } from "./inspection-view.js";
import { quickKdmOutcome, quickKdmRequest } from "./quick-kdm-form.js";
import { settingsUpdateFromFields } from "./settings-form.js";

const FILE_FILTERS = {
  certificate: [{ name: "Certificate", extensions: ["pem", "crt", "cer"] }],
  key: [{ name: "Private key", extensions: ["pem", "key"] }],
  dkdm: [{ name: "DKDM", extensions: ["xml"] }],
  flm: [{ name: "FLM", extensions: ["xml"] }],
  json: [{ name: "DCP Wizard cinemas", extensions: ["json"] }],
  jsonl: [{ name: "DCP Wizard KDM history", extensions: ["jsonl"] }],
  inspect: [{ name: "KDM or certificate", extensions: ["xml", "pem", "crt", "cer"] }],
};
const FOLDER_FILTER = "folder";
const READY_STATUS = "Ready";
const SETTINGS_SAVED_STATUS = "Settings saved";
const REFRESHING_FEEDS_STATUS = "Refreshing feeds";
const NO_FEEDS_LINE = "No FLM feeds, add one in Settings";
const NEW_FEED = { name: "", siteListUrl: "", authentication: { kind: "none" } };
const ZONE_LIST_ID = "time-zone-names";
const SCREEN_COLUMNS = 6;
const NO_CERTIFICATE_CHANGE_LINE = "The certificate is the one already stored";

const statusBadge = document.getElementById("status");
let selectedBookingId = null;
let listedBookings = [];
const issueFolderInput = document.getElementById("issue-folder-path");
// the booking the form edits, null while it makes a new one
let editingBooking = null;

function showStatus(text) {
  statusBadge.textContent = text;
}

// every failure reaches the page as the backend wrote it, screen and rule included
async function run(work) {
  try {
    return await work();
  } catch (error) {
    showStatus(String(error));
    return undefined;
  }
}

function notesList(notes) {
  if (notes.length === 0) return "";
  return `<ul class="notes">${notes.map((note) => `<li>${escapeHtml(note)}</li>`).join("")}</ul>`;
}

async function pickFile(filterName, multiple = false) {
  if (filterName === FOLDER_FILTER) return open({ directory: true });
  return open({ multiple, filters: FILE_FILTERS[filterName] });
}

// onFetched gets the certificate and the serial it was fetched by
function attachCertificateFetch(container, serial, onFetched) {
  const options = VENDORS.map((vendor) => `<option value="${vendor.value}">${escapeHtml(vendor.label)}</option>`).join("");
  container.innerHTML = `<select class="fetch-vendor">${options}</select>
    <input type="text" class="fetch-serial" placeholder="Serial" value="${escapeHtml(serial)}" spellcheck="false">
    <input type="text" class="fetch-qube-type" placeholder="Qube device type, such as QXPD" spellcheck="false" hidden>
    <button type="button" class="btn-sm fetch-run">Fetch by serial</button>`;
  const vendor = container.querySelector(".fetch-vendor");
  const qubeType = container.querySelector(".fetch-qube-type");
  vendor.addEventListener("change", () => {
    qubeType.hidden = vendor.value !== "qube";
  });
  container.querySelector(".fetch-run").addEventListener("click", () =>
    run(async () => {
      const { request, error } = fetchRequest({
        vendor: vendor.value,
        serial: container.querySelector(".fetch-serial").value,
        qubeType: qubeType.value,
      });
      if (error) {
        showStatus(error);
        return;
      }
      showStatus(`Fetching from ${vendorLabel(request.vendor)}`);
      const certificate = await invoke("certificates_fetch", request);
      showStatus(`Fetched ${fetchedText(certificate)}`);
      await onFetched(certificate, request.serial);
    }),
  );
}

// views

document.querySelectorAll(".sidebar-btn[data-view]").forEach((button) => {
  button.addEventListener("click", () => {
    document.querySelectorAll(".sidebar-btn").forEach((other) => other.classList.remove("active"));
    document.querySelectorAll(".view").forEach((view) => view.classList.remove("active"));
    button.classList.add("active");
    document.getElementById(`view-${button.dataset.view}`).classList.add("active");
    refreshView(button.dataset.view);
  });
});

document.getElementById("theme-toggle").addEventListener("click", () => {
  document.body.classList.toggle("light");
});

function refreshView(view) {
  const refreshers = {
    titles: refreshTitles,
    cinemas: refreshCinemas,
    bookings: refreshBookings,
    "quick-kdm": fillQuickKdmForm,
    outbox: refreshOutbox,
    expiry: refreshExpiry,
    inspect: async () => {},
    settings: loadSettings,
  };
  run(refreshers[view]);
}

// titles

async function refreshTitles() {
  const titles = await invoke("titles_list");
  document.getElementById("titles-tbody").innerHTML = titles
    .map(
      (title) => `<tr>
        <td>${escapeHtml(title.contentTitle)}</td>
        <td>${escapeHtml(title.cplId)}</td>
        <td>${escapeHtml(title.standard ?? "unknown")}</td>
        <td>${escapeHtml(title.dkdmNotValidBefore)}</td>
        <td>${escapeHtml(title.dkdmNotValidAfter)}</td>
      </tr>`,
    )
    .join("");
  return titles;
}

document.getElementById("titles-import").addEventListener("click", () =>
  run(async () => {
    const path = await pickFile("dkdm");
    if (!path) return;
    const title = await invoke("titles_import_dkdm", { path });
    showStatus(`Imported ${title.contentTitle}`);
    await refreshTitles();
  }),
);

// cinemas

// the first failure in the row, every failure one click away
function certificateStatusHtml(certificate) {
  const summary = escapeHtml(certificateStatusText(certificate));
  if (certificate.failures.length < 2) return summary;
  return `<details><summary>${summary}</summary>${notesList(certificate.failures)}</details>`;
}

function screenCertificatesHtml(certificate) {
  if (!certificate.chain) return "";
  return `<tr>
    <td colspan="${SCREEN_COLUMNS}"><details><summary>Certificate details</summary>${chainInspectionHtml(certificate.chain)}</details></td>
  </tr>`;
}

function screenRowHtml(screen) {
  const certificate = screen.certificate;
  const statusClass = certificate.failures.length > 0 ? "status-refused" : "status-issued";
  return `<tr>
    <td>${escapeHtml(screen.name)}</td>
    <td>${escapeHtml(screen.deviceSerial ?? "")}</td>
    <td>${escapeHtml(devicesText(screen.devices))}</td>
    <td>${escapeHtml(certificate.subject)}</td>
    <td class="${statusClass}">${certificateStatusHtml(certificate)}${notesList(certificate.warnings)}</td>
    <td>
      <button type="button" class="btn-sm screen-certificate-file" title="Replace the recipient certificate">File…</button>
      <button type="button" class="btn-sm screen-certificate-fetch" title="Replace it with the vendor's certificate">Fetch…</button>
    </td>
  </tr>
  <tr class="screen-fetch-row" data-screen="${screen.id}" data-serial="${escapeHtml(screen.deviceSerial ?? "")}" hidden>
    <td colspan="${SCREEN_COLUMNS}"><div class="certificate-fetch"></div></td>
  </tr>${screenCertificatesHtml(certificate)}`;
}

function cinemaPendingHtml(cinema) {
  if (cinema.pendingScreens === 0) return "";
  return `<div class="cinema-pending-summary">
    <span class="status-refused">${escapeHtml(cinemaPendingText(cinema.pendingScreens))}</span>
    <button type="button" class="btn-sm cinema-plan-pending">Issue pending</button>
  </div>`;
}

function cinemaEndingSoonHtml(cinema) {
  if (cinema.bookingsEndingSoon === 0) return "";
  return `<div class="status-ending-soon">${escapeHtml(cinemaEndingSoonText(cinema.bookingsEndingSoon))}</div>`;
}

function cinemaHtml(cinema) {
  return `<div class="cinema-block" data-cinema="${cinema.id}">
    <h3>${escapeHtml(cinema.name)}</h3>
    ${cinemaPendingHtml(cinema)}
    ${cinemaEndingSoonHtml(cinema)}
    <div class="cinema-edit">
      <input type="text" class="cinema-emails" value="${escapeHtml(cinema.emails.join(", "))}" placeholder="KDM email addresses">
      <input type="text" class="cinema-zone" list="${ZONE_LIST_ID}" value="${escapeHtml(cinema.timeZone ?? "")}" placeholder="IANA time zone">
      <button type="button" class="btn-sm cinema-save">Save</button>
    </div>
    <table class="jobs-table">
      <thead><tr><th>Screen</th><th>Media block</th><th>Authorized devices</th><th>Recipient</th><th>Certificates</th><th></th></tr></thead>
      <tbody>${cinema.screens.map(screenRowHtml).join("")}</tbody>
    </table>
    <div class="cinema-pending"></div>
  </div>`;
}

function cinemaPlansHtml(plans) {
  const bookings = plans
    .map(
      ({ plan }) => `<h4>${escapeHtml(plan.contentTitle)}</h4>
        ${notesList(plan.notChecked)}
        ${detailTableHtml(planRows(plan))}`,
    )
    .join("");
  return `${bookings}
    <div class="panel-actions">
      <label><input type="checkbox" class="cinema-issue-email"> Email one ZIP per booking</label>
      <button type="button" class="btn-sm btn-primary cinema-issue-pending" title="Written to the KDM folder from Settings">Issue</button>
    </div>`;
}

function cinemaOutcomesHtml(result) {
  const issued = result.issued
    .map(
      (booking) => `<h4>${escapeHtml(booking.contentTitle)}</h4>
        ${detailTableHtml(outcomeRows(booking.outcome))}
        <ul class="notes">${deliveriesHtml(booking.deliveries)}</ul>`,
    )
    .join("");
  const failed = result.failed.map((booking) => `<li>${escapeHtml(failedBookingText(booking))}</li>`).join("");
  return `${issued}<ul class="notes status-failed">${failed}</ul>`;
}

async function issueCinemaPending(cinemaId, sendEmail) {
  const result = await invoke("cinemas_issue_pending", { id: cinemaId, sendEmail });
  await refreshCinemas();
  const block = document.querySelector(`.cinema-block[data-cinema="${cinemaId}"]`);
  block.querySelector(".cinema-pending").innerHTML = cinemaOutcomesHtml(result);
  showStatus(cinemaIssueStatus(result));
  block.scrollIntoView();
}

async function planCinemaPending(block) {
  const cinemaId = Number(block.dataset.cinema);
  const plans = await invoke("cinemas_plan_pending", { id: cinemaId });
  const pending = block.querySelector(".cinema-pending");
  pending.innerHTML = cinemaPlansHtml(plans);
  pending.querySelector(".cinema-issue-pending").addEventListener("click", () =>
    run(() => issueCinemaPending(cinemaId, pending.querySelector(".cinema-issue-email").checked)),
  );
}

async function refreshCinemas() {
  const cinemas = await invoke("cinemas_list");
  const list = document.getElementById("cinemas-list");
  list.innerHTML = cinemas.map(cinemaHtml).join("");
  list.querySelectorAll(".cinema-save").forEach((button) => {
    button.addEventListener("click", () =>
      run(async () => {
        const block = button.closest(".cinema-block");
        const timeZone = block.querySelector(".cinema-zone").value.trim();
        await invoke("cinemas_update", {
          id: Number(block.dataset.cinema),
          emails: emailsFromText(block.querySelector(".cinema-emails").value),
          timeZone: timeZone || null,
        });
        showStatus("Cinema saved");
      }),
    );
  });
  list.querySelectorAll(".screen-certificate-file").forEach((button) => {
    const fetchRow = button.closest("tr").nextElementSibling;
    button.addEventListener("click", () =>
      run(async () => {
        const path = await pickFile("certificate");
        if (path) await setScreenCertificate(Number(fetchRow.dataset.screen), { kind: "file", path });
      }),
    );
  });
  list.querySelectorAll(".screen-certificate-fetch").forEach((button) => {
    const fetchRow = button.closest("tr").nextElementSibling;
    button.addEventListener("click", () => {
      fetchRow.hidden = false;
      attachCertificateFetch(fetchRow.querySelector(".certificate-fetch"), fetchRow.dataset.serial, (certificate) =>
        setScreenCertificate(Number(fetchRow.dataset.screen), { kind: "pem", pem: certificate.pem }),
      );
    });
  });
  list.querySelectorAll(".cinema-plan-pending").forEach((button) => {
    button.addEventListener("click", () => run(() => planCinemaPending(button.closest(".cinema-block"))));
  });
  return cinemas;
}

async function setScreenCertificate(screenId, certificate) {
  const lines = await invoke("cinemas_set_screen_certificate", { screenId, certificate });
  showImportResults(lines.length > 0 ? lines : [NO_CERTIFICATE_CHANGE_LINE]);
  showStatus("Screen certificate saved");
  await refreshCinemas();
}

// each import adds its lines under the ones before
function showImportResults(lines) {
  document.getElementById("cinemas-import-results").insertAdjacentHTML(
    "beforeend",
    lines.map((line) => `<li>${escapeHtml(line)}</li>`).join(""),
  );
}

document.getElementById("cinemas-import-flm").addEventListener("click", () =>
  run(async () => {
    const picked = await pickFile("flm", true);
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked : [picked];
    showImportResults(await invoke("cinemas_import_flm", { paths }));
    await refreshCinemas();
  }),
);

const refreshFeedsButton = document.getElementById("cinemas-refresh-feeds");
refreshFeedsButton.addEventListener("click", () =>
  run(async () => {
    refreshFeedsButton.disabled = true;
    showStatus(REFRESHING_FEEDS_STATUS);
    try {
      const lines = await invoke("cinemas_refresh_feeds");
      showImportResults(lines.length > 0 ? lines : [NO_FEEDS_LINE]);
    } finally {
      refreshFeedsButton.disabled = false;
    }
    showStatus(READY_STATUS);
    await refreshCinemas();
  }),
);

document.getElementById("cinemas-import-dcpwizard").addEventListener("click", () =>
  run(async () => {
    const path = await pickFile("json");
    if (!path) return;
    const report = await invoke("cinemas_import_dcpwizard", { path });
    showImportResults([
      `${report.cinemas} cinemas, ${report.screens} screens imported`,
      ...report.skipped.map((skipped) => `skipped ${skipped}`),
    ]);
    await refreshCinemas();
  }),
);

document.getElementById("history-import-dcpwizard").addEventListener("click", () =>
  run(async () => {
    const path = await pickFile("jsonl");
    if (!path) return;
    const count = await invoke("history_import_dcpwizard", { path });
    showImportResults([`${count} history records imported`]);
  }),
);

// bookings

async function fillBookingForm() {
  const [titles, cinemas] = await Promise.all([invoke("titles_list"), invoke("cinemas_list")]);
  document.getElementById("booking-title").innerHTML = titles
    .map((title) => `<option value="${title.id}">${escapeHtml(title.contentTitle)}</option>`)
    .join("");
  document.getElementById("booking-screens").innerHTML = cinemas
    .flatMap((cinema) =>
      cinema.screens.map(
        (screen) =>
          `<label><input type="checkbox" value="${screen.id}"> ${escapeHtml(cinema.name)} / ${escapeHtml(screen.name)}</label>`,
      ),
    )
    .join("");
}

function bookedScreenHtml(screen) {
  const text = escapeHtml(bookedScreenText(screen));
  return screen.pending ? `<span class="status-refused">${text}</span>` : text;
}

function bookingStateHtml(booking, now) {
  return [
    ["status-refused", bookingPendingText(booking)],
    ["status-ending-soon", bookingEndingText(booking, now)],
  ]
    .filter(([, text]) => text)
    .map(([className, text]) => `<div class="${className}">${escapeHtml(text)}</div>`)
    .join("");
}

function selectedBooking() {
  return listedBookings.find((booking) => booking.id === selectedBookingId);
}

function updateIssueButtons() {
  const booking = selectedBooking();
  document.getElementById("issue-run").disabled = !booking || pendingScreenCount(booking) === 0;
}

async function refreshBookings() {
  await fillBookingForm();
  const bookings = await invoke("bookings_list");
  listedBookings = bookings;
  updateIssueButtons();
  const tbody = document.getElementById("bookings-tbody");
  const now = new Date();
  tbody.innerHTML = bookings
    .map(
      (booking) => `<tr>
        <td>${escapeHtml(booking.contentTitle)}</td>
        <td>${escapeHtml(booking.start)}</td>
        <td>${escapeHtml(booking.end)}</td>
        <td>${booking.screens.map(bookedScreenHtml).join(", ")}</td>
        <td>${bookingStateHtml(booking, now)}</td>
        <td>
          <button class="btn-sm booking-open" data-booking="${booking.id}">Check</button>
          <button class="btn-sm booking-edit" data-booking="${booking.id}">Edit</button>
          <button class="btn-sm booking-remove" data-booking="${booking.id}">Remove</button>
        </td>
      </tr>`,
    )
    .join("");
  const bookingFor = (button) => bookings.find((booking) => booking.id === Number(button.dataset.booking));
  tbody.querySelectorAll(".booking-open").forEach((button) => {
    button.addEventListener("click", () => run(() => showPlan(Number(button.dataset.booking))));
  });
  tbody.querySelectorAll(".booking-edit").forEach((button) => {
    button.addEventListener("click", () => startEditing(bookingFor(button)));
  });
  tbody.querySelectorAll(".booking-remove").forEach((button) => {
    button.addEventListener("click", () =>
      run(async () => {
        await invoke("bookings_remove", { id: Number(button.dataset.booking) });
        if (selectedBookingId === Number(button.dataset.booking)) {
          document.getElementById("booking-detail").hidden = true;
          selectedBookingId = null;
        }
        showStatus("Booking removed, its KDMs stay in the Outbox");
        await refreshBookings();
      }),
    );
  });
}

function setEditing(booking) {
  editingBooking = booking;
  document.getElementById("booking-form-title").textContent = booking ? `Edit ${booking.contentTitle}` : "New Booking";
  document.getElementById("booking-submit").textContent = booking ? "Save Booking" : "Book";
  document.getElementById("booking-cancel-edit").hidden = !booking;
  document.getElementById("booking-title").disabled = Boolean(booking);
}

function startEditing(booking) {
  const fields = bookingFields(booking);
  setEditing(booking);
  document.getElementById("booking-title").value = fields.titleId;
  document.getElementById("booking-start").value = fields.start;
  document.getElementById("booking-end").value = fields.end;
  document.getElementById("booking-formulation").value = fields.formulation;
  document.querySelectorAll("#booking-screens input").forEach((input) => {
    input.checked = fields.screenIds.includes(input.value);
  });
}

document.getElementById("booking-cancel-edit").addEventListener("click", () => setEditing(null));

function detailTableHtml(rows) {
  return `<table class="jobs-table">
    <thead>
      <tr><th>Screen</th><th>Result</th><th>Formulation or file</th><th>Window</th><th>Rule, warning or reason</th></tr>
    </thead>
    <tbody>${detailRowsHtml(rows)}</tbody>
  </table>`;
}

function deliveriesHtml(deliveries) {
  return deliveries
    .map((delivery) => `<li>${escapeHtml(delivery.cinema)}: ${escapeHtml(delivery.zipPath)}, ${escapeHtml(deliveryText(delivery))}</li>`)
    .join("");
}

function detailRowsHtml(rows) {
  return rows
    .map(
      (row) => `<tr>
        <td>${escapeHtml(row.screen)}</td>
        <td class="status-${row.status}">${escapeHtml(row.status)}</td>
        <td>${escapeHtml(row.formulation ?? row.detail)}</td>
        <td>${escapeHtml(row.window ?? "")}</td>
        <td>${notesList(row.notes)}</td>
      </tr>`,
    )
    .join("");
}

async function showPlan(bookingId) {
  selectedBookingId = bookingId;
  updateIssueButtons();
  const plan = await invoke("bookings_plan", { id: bookingId, scope: planScope(selectedBooking()) });
  document.getElementById("booking-detail").hidden = false;
  document.getElementById("booking-detail-title").textContent = plan.contentTitle;
  document.getElementById("booking-plan-notes").textContent = plan.notChecked.join(" ");
  document.getElementById("booking-detail-tbody").innerHTML = detailRowsHtml(planRows(plan));
  document.getElementById("issue-deliveries").innerHTML = "";
  document.getElementById("booking-detail").scrollIntoView();
}

document.getElementById("booking-form").addEventListener("submit", (event) => {
  event.preventDefault();
  run(async () => {
    const screenIds = [...document.querySelectorAll("#booking-screens input:checked")].map(
      (input) => input.value,
    );
    const fields = {
      titleId: document.getElementById("booking-title").value,
      screenIds,
      start: document.getElementById("booking-start").value,
      end: document.getElementById("booking-end").value,
      formulation: document.getElementById("booking-formulation").value,
    };
    const edited = editingBooking;
    const { booking, change, error } = edited ? bookingChange(fields) : newBooking(fields);
    document.getElementById("booking-error").textContent = error ?? "";
    if (error) return;
    const id = edited ? edited.id : await invoke("bookings_add", { booking });
    if (edited) await invoke("bookings_update", { id, change });
    setEditing(null);
    await refreshBookings();
    await showPlan(id);
  });
});

document.getElementById("issue-folder").addEventListener("click", () =>
  run(async () => {
    const folder = await pickFile(FOLDER_FILTER);
    if (folder) issueFolderInput.value = folder;
  }),
);

async function issueSelectedBooking(scope) {
  if (selectedBookingId === null) return;
  const result = await invoke("bookings_issue", {
    id: selectedBookingId,
    scope,
    outputFolder: issueFolderInput.value.trim() || null,
    sendEmail: document.getElementById("issue-email").checked,
  });
  document.getElementById("booking-detail-tbody").innerHTML = detailRowsHtml(outcomeRows(result.outcome));
  document.getElementById("issue-deliveries").innerHTML = deliveriesHtml(result.deliveries);
  showStatus(issuedStatus([result.outcome]));
  await refreshBookings();
  document.getElementById("booking-detail").scrollIntoView();
}

document.getElementById("issue-run").addEventListener("click", () =>
  run(() => issueSelectedBooking(ISSUE_SCOPE.pendingScreens)),
);
document.getElementById("issue-run-all").addEventListener("click", () =>
  run(() => issueSelectedBooking(ISSUE_SCOPE.allScreens)),
);

// quick kdm

// the certificate the next quick KDM goes to, a picked file or a fetched PEM
let quickRecipient = null;
const quickZoneInput = document.getElementById("quick-zone");
const quickSerialInput = document.getElementById("quick-serial");

function setQuickRecipient(recipient, text) {
  quickRecipient = recipient;
  document.getElementById("quick-recipient").textContent = text;
}

async function fillQuickKdmForm() {
  const titles = await invoke("titles_list");
  const select = document.getElementById("quick-title");
  const chosen = select.value;
  select.innerHTML = titles
    .map((title) => `<option value="${title.id}">${escapeHtml(title.contentTitle)}</option>`)
    .join("");
  if (chosen) select.value = chosen;
  if (!quickZoneInput.value) quickZoneInput.value = Intl.DateTimeFormat().resolvedOptions().timeZone;
}

attachCertificateFetch(document.getElementById("quick-fetch"), "", (certificate, serial) => {
  setQuickRecipient({ kind: "pem", pem: certificate.pem }, fetchedText(certificate));
  if (!quickSerialInput.value) quickSerialInput.value = serial;
});

document.getElementById("quick-pick").addEventListener("click", () =>
  run(async () => {
    const path = await pickFile("certificate");
    if (path) setQuickRecipient({ kind: "file", path }, path);
  }),
);

document.getElementById("quick-devices-pick").addEventListener("click", () =>
  run(async () => {
    const picked = await pickFile("certificate", true);
    if (!picked) return;
    const devices = document.getElementById("quick-devices");
    const paths = Array.isArray(picked) ? picked : [picked];
    devices.value = [devices.value.trim(), ...paths].filter(Boolean).join("\n");
  }),
);

document.getElementById("quick-form").addEventListener("submit", (event) => {
  event.preventDefault();
  run(async () => {
    const { request, error } = quickKdmRequest({
      titleId: document.getElementById("quick-title").value,
      recipient: quickRecipient,
      trustedDevices: document.getElementById("quick-devices").value,
      mediaBlockSerial: quickSerialInput.value,
      start: document.getElementById("quick-start").value,
      end: document.getElementById("quick-end").value,
      timeZone: quickZoneInput.value,
      formulation: document.getElementById("quick-formulation").value,
    });
    document.getElementById("quick-error").textContent = error ?? "";
    if (error) return;
    const { status, lines } = quickKdmOutcome(await invoke("quick_kdm_issue", { request }));
    const result = document.getElementById("quick-result");
    result.className = `notes status-${status}`;
    result.innerHTML = lines.map((line) => `<li>${escapeHtml(line)}</li>`).join("");
    showStatus(status === "issued" ? "KDM issued" : "KDM refused by the certificate checks");
  });
});

// outbox

async function refreshOutbox() {
  const outbox = await invoke("outbox_list");
  document.getElementById("deliveries-tbody").innerHTML = outbox.deliveries
    .map(
      (delivery) => `<tr>
        <td>${escapeHtml(formatDateTime(delivery.deliveredAt))}</td>
        <td>${escapeHtml(delivery.contentTitle)}</td>
        <td>${escapeHtml(delivery.cinema)}</td>
        <td>${escapeHtml(delivery.zipPath)}</td>
        <td class="status-${delivery.result.kind}">${escapeHtml(deliveryText(delivery))}</td>
        <td><button class="btn-sm delivery-resend" data-delivery="${delivery.id}">Resend</button></td>
      </tr>`,
    )
    .join("");
  document.querySelectorAll("#deliveries-tbody .delivery-resend").forEach((button) => {
    button.addEventListener("click", () =>
      run(async () => {
        const delivery = await invoke("outbox_resend", { id: Number(button.dataset.delivery) });
        showStatus(`${delivery.cinema}: ${deliveryText(delivery)}`);
        await refreshOutbox();
      }),
    );
  });
  document.getElementById("issues-tbody").innerHTML = outbox.issues
    .map(
      (issue) => `<tr>
        <td>${escapeHtml(formatDateTime(issue.issuedAt))}</td>
        <td>${escapeHtml(issue.contentTitle)}</td>
        <td>${escapeHtml([issue.cinema, issue.screen].filter(Boolean).join(" / "))}</td>
        <td>${escapeHtml(issue.recipientSubject)}</td>
        <td>${escapeHtml(`${issue.validFrom} to ${issue.validTo}`)}</td>
        <td>${escapeHtml(issue.formulation ?? "")}</td>
        <td>${escapeHtml(issue.fileName)}</td>
      </tr>`,
    )
    .join("");
}

document.getElementById("outbox-refresh").addEventListener("click", () => run(refreshOutbox));

// expiry

async function refreshExpiry() {
  const [report, bookings] = await Promise.all([invoke("expiry_list"), invoke("bookings_list")]);
  document.getElementById("expiry-empty").hidden = !nothingExpires(report);
  document.getElementById("expiry-bookings-tbody").innerHTML = report.bookings
    .map(bookingExpiryRow)
    .map(
      (row) => `<tr>
        <td>${escapeHtml(row.title)}</td>
        <td>${escapeHtml(row.cinema)}</td>
        <td>${escapeHtml(row.screen)}</td>
        <td>${escapeHtml(row.item)}</td>
        <td class="distinguished-name">${escapeHtml(row.subject)}</td>
        <td class="status-refused">${escapeHtml(formatDateTime(row.expiresAt))}</td>
        <td>${escapeHtml(formatDateTime(row.bookingEndsAt))}</td>
      </tr>`,
    )
    .join("");
  document.getElementById("expiry-signer-tbody").innerHTML = report.signerChain
    .map((certificate) => signerChainRow(certificate, bookings))
    .map(
      (row) => `<tr>
        <td class="distinguished-name">${escapeHtml(row.subject)}</td>
        <td class="${row.bookingsEndingAfter ? "status-refused" : ""}">${escapeHtml(formatDateTime(row.expiresAt))}</td>
        <td>${escapeHtml(row.bookingsEndingAfter)}</td>
      </tr>`,
    )
    .join("");
  document.getElementById("expiry-not-checked").innerHTML = report.notChecked
    .map((line) => `<li>${escapeHtml(line)}</li>`)
    .join("");
}

document.getElementById("expiry-refresh").addEventListener("click", () => run(refreshExpiry));

// inspect

document.getElementById("inspect-open").addEventListener("click", () =>
  run(async () => {
    const path = await pickFile("inspect");
    if (!path) return;
    const inspection = await invoke("inspect_file", { path });
    document.getElementById("inspect-title").textContent = path;
    document.getElementById("inspect-result").innerHTML = fileInspectionHtml(inspection);
  }),
);

// settings

const settingsFields = {
  signerCertificate: document.getElementById("set-signer-certificate"),
  signerKey: document.getElementById("set-signer-key"),
  signerChain: document.getElementById("set-signer-chain"),
  dkdmRecipientKey: document.getElementById("set-dkdm-key"),
  creationFacility: document.getElementById("set-creation-facility"),
  outputFolder: document.getElementById("set-output-folder"),
  databasePath: document.getElementById("set-database-path"),
  smtpHost: document.getElementById("set-smtp-host"),
  smtpPort: document.getElementById("set-smtp-port"),
  smtpSecurity: document.getElementById("set-smtp-security"),
  smtpUsername: document.getElementById("set-smtp-username"),
  smtpPassword: document.getElementById("set-smtp-password"),
  smtpPasswordClear: document.getElementById("set-smtp-password-clear"),
  smtpFrom: document.getElementById("set-smtp-from"),
  smtpBody: document.getElementById("set-smtp-body"),
};

const feedList = document.getElementById("set-feeds");
const vendorAccountList = document.getElementById("set-vendor-accounts");

function vendorAccountRowHtml(account) {
  const passwordPlaceholder = account.password.set ? "Stored, type to replace" : "";
  return `<div class="vendor-account-row" data-vendor="${escapeHtml(account.vendor)}">
    <div class="prop-field">
      <label>${escapeHtml(vendorLabel(account.vendor))} User</label>
      <input type="text" class="vendor-user" value="${escapeHtml(account.user)}" spellcheck="false" autocomplete="off">
    </div>
    <div class="prop-field">
      <label>Password</label>
      <input type="password" class="vendor-password" autocomplete="new-password" placeholder="${passwordPlaceholder}">
      <label><input type="checkbox" class="vendor-password-clear"> Remove the stored password</label>
    </div>
  </div>`;
}

function vendorAccountFields(row) {
  return {
    vendor: row.dataset.vendor,
    user: row.querySelector(".vendor-user").value,
    newPassword: row.querySelector(".vendor-password").value,
    clearPassword: row.querySelector(".vendor-password-clear").checked,
  };
}

function feedRowHtml(feed) {
  const authentication = feed.authentication;
  const passwordPlaceholder = authentication.password?.set ? "Stored, type to replace" : "";
  return `<div class="feed-row" data-kind="${escapeHtml(authentication.kind)}">
    <div class="prop-field">
      <label>Name</label>
      <input type="text" class="feed-name" value="${escapeHtml(feed.name)}" spellcheck="false">
    </div>
    <div class="prop-field">
      <label>SiteList URL</label>
      <input type="url" class="feed-url" value="${escapeHtml(feed.siteListUrl)}" spellcheck="false">
    </div>
    <div class="prop-field">
      <label>Authentication</label>
      <select class="feed-kind">
        <option value="none">None</option>
        <option value="basic">User and password</option>
        <option value="clientCertificate">Client certificate</option>
      </select>
    </div>
    <div class="prop-field feed-basic">
      <label>User</label>
      <input type="text" class="feed-username" value="${escapeHtml(authentication.username ?? "")}" spellcheck="false" autocomplete="off">
    </div>
    <div class="prop-field feed-basic">
      <label>Password</label>
      <input type="password" class="feed-password" autocomplete="new-password" placeholder="${passwordPlaceholder}">
      <label><input type="checkbox" class="feed-password-clear"> Remove the stored password</label>
    </div>
    <div class="prop-field feed-client-certificate">
      <label>Client Certificate Chain, PEM</label>
      <div class="input-with-btn">
        <input type="text" class="feed-certificate" value="${escapeHtml(authentication.certificateChain ?? "")}" readonly>
        <button type="button" class="btn-sm feed-browse" data-target="feed-certificate" data-filter="certificate">…</button>
      </div>
    </div>
    <div class="prop-field feed-client-certificate">
      <label>Client Private Key, PEM</label>
      <div class="input-with-btn">
        <input type="text" class="feed-key" value="${escapeHtml(authentication.privateKey ?? "")}" readonly>
        <button type="button" class="btn-sm feed-browse" data-target="feed-key" data-filter="key">…</button>
      </div>
    </div>
    <button type="button" class="btn-sm feed-remove">Remove feed</button>
  </div>`;
}

function addFeedRow(feed) {
  feedList.insertAdjacentHTML("beforeend", feedRowHtml(feed));
  const row = feedList.lastElementChild;
  const kind = row.querySelector(".feed-kind");
  kind.value = feed.authentication.kind;
  kind.addEventListener("change", () => {
    row.dataset.kind = kind.value;
  });
  row.querySelectorAll(".feed-browse").forEach((button) => {
    button.addEventListener("click", () =>
      run(async () => {
        const path = await pickFile(button.dataset.filter);
        if (path) row.querySelector(`.${button.dataset.target}`).value = path;
      }),
    );
  });
  row.querySelector(".feed-remove").addEventListener("click", () => row.remove());
}

function feedFields(row) {
  const value = (selector) => row.querySelector(selector).value;
  return {
    name: value(".feed-name"),
    siteListUrl: value(".feed-url"),
    kind: value(".feed-kind"),
    username: value(".feed-username"),
    newPassword: value(".feed-password"),
    clearPassword: row.querySelector(".feed-password-clear").checked,
    certificateChain: value(".feed-certificate"),
    privateKey: value(".feed-key"),
  };
}

document.getElementById("set-feed-add").addEventListener("click", () => addFeedRow(NEW_FEED));

function fillSettings(settings) {
  const fields = settingsFields;
  fields.signerCertificate.value = settings.signerCertificate ?? "";
  fields.signerKey.value = settings.signerKey ?? "";
  fields.signerChain.value = settings.signerChain.join("\n");
  fields.dkdmRecipientKey.value = settings.dkdmRecipientKey ?? "";
  fields.creationFacility.value = settings.creationFacility;
  fields.outputFolder.value = settings.outputFolder ?? "";
  fields.databasePath.value = settings.databasePath;
  const smtp = settings.smtp;
  fields.smtpHost.value = smtp?.host ?? "";
  fields.smtpPort.value = smtp?.port ?? "";
  fields.smtpSecurity.value = smtp?.security ?? "tls";
  fields.smtpUsername.value = smtp?.username ?? "";
  fields.smtpFrom.value = smtp?.from ?? "";
  fields.smtpBody.value = smtp?.bodyTemplate ?? "";
  fields.smtpPassword.value = "";
  fields.smtpPassword.placeholder = settings.smtpPasswordSet ? "Stored, type to replace" : "";
  fields.smtpPasswordClear.checked = false;
  feedList.innerHTML = "";
  settings.feeds.forEach(addFeedRow);
  vendorAccountList.innerHTML = settings.vendorAccounts.map(vendorAccountRowHtml).join("");
}

async function loadSettings() {
  fillSettings(await invoke("settings_load"));
}

document.querySelectorAll("[data-browse]").forEach((button) => {
  button.addEventListener("click", () =>
    run(async () => {
      const path = await pickFile(button.dataset.filter);
      if (path) document.getElementById(button.dataset.browse).value = path;
    }),
  );
});

document.getElementById("settings-form").addEventListener("submit", (event) => {
  event.preventDefault();
  run(async () => {
    const fields = settingsFields;
    const update = settingsUpdateFromFields({
      databasePath: fields.databasePath.value,
      signerCertificate: fields.signerCertificate.value,
      signerKey: fields.signerKey.value,
      signerChain: fields.signerChain.value,
      dkdmRecipientKey: fields.dkdmRecipientKey.value,
      creationFacility: fields.creationFacility.value,
      outputFolder: fields.outputFolder.value,
      smtp: {
        host: fields.smtpHost.value,
        port: fields.smtpPort.value,
        security: fields.smtpSecurity.value,
        username: fields.smtpUsername.value,
        from: fields.smtpFrom.value,
        bodyTemplate: fields.smtpBody.value,
      },
      newPassword: fields.smtpPassword.value,
      clearPassword: fields.smtpPasswordClear.checked,
      feeds: [...feedList.querySelectorAll(".feed-row")].map(feedFields),
      vendorAccounts: [...vendorAccountList.querySelectorAll(".vendor-account-row")].map(vendorAccountFields),
    });
    fillSettings(await invoke("settings_save", { update }));
    showStatus(SETTINGS_SAVED_STATUS);
  });
});

async function fillTimeZones() {
  const list = document.createElement("datalist");
  list.id = ZONE_LIST_ID;
  list.innerHTML = (await invoke("time_zones"))
    .map((zone) => `<option value="${escapeHtml(zone)}">`)
    .join("");
  document.body.appendChild(list);
}

run(async () => {
  await fillTimeZones();
  await refreshTitles();
  showStatus(READY_STATUS);
});
