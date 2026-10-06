import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { escapeHtml } from "../../extern/guikit/src/html.js";
import { formatDateTime } from "../../extern/guikit/src/time-format.js";
import { certificateStatusText, devicesText, emailsFromText } from "./cinema-rows.js";
import { newBooking, planRows, screenLabel } from "./booking-form.js";
import { deliveryText, outcomeRows } from "./issue-outcome.js";
import { settingsUpdateFromFields } from "./settings-form.js";

const FILE_FILTERS = {
  certificate: [{ name: "Certificate", extensions: ["pem", "crt", "cer"] }],
  key: [{ name: "Private key", extensions: ["pem", "key"] }],
  dkdm: [{ name: "DKDM", extensions: ["xml"] }],
  flm: [{ name: "FLM", extensions: ["xml"] }],
  json: [{ name: "DCP Wizard cinemas", extensions: ["json"] }],
  jsonl: [{ name: "DCP Wizard KDM history", extensions: ["jsonl"] }],
};
const FOLDER_FILTER = "folder";
const READY_STATUS = "Ready";
const SETTINGS_SAVED_STATUS = "Settings saved";
const ZONE_LIST_ID = "time-zone-names";

const statusBadge = document.getElementById("status");
let selectedBookingId = null;
let issueFolder = null;

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
    outbox: refreshOutbox,
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

function screenRowHtml(screen) {
  const certificate = screen.certificate;
  const statusClass = certificate.failures.length > 0 ? "status-refused" : "status-issued";
  return `<tr>
    <td>${escapeHtml(screen.name)}</td>
    <td>${escapeHtml(screen.deviceSerial ?? "")}</td>
    <td>${escapeHtml(devicesText(screen.devices))}</td>
    <td>${escapeHtml(certificate.subject)}</td>
    <td class="${statusClass}">${escapeHtml(certificateStatusText(certificate))}${notesList(certificate.warnings)}</td>
  </tr>`;
}

function cinemaHtml(cinema) {
  return `<div class="cinema-block" data-cinema="${cinema.id}">
    <h3>${escapeHtml(cinema.name)}</h3>
    <div class="cinema-edit">
      <input type="text" class="cinema-emails" value="${escapeHtml(cinema.emails.join(", "))}" placeholder="KDM email addresses">
      <input type="text" class="cinema-zone" list="${ZONE_LIST_ID}" value="${escapeHtml(cinema.timeZone ?? "")}" placeholder="IANA time zone">
      <button type="button" class="btn-sm cinema-save">Save</button>
    </div>
    <table class="jobs-table">
      <thead><tr><th>Screen</th><th>Media block</th><th>Authorized devices</th><th>Recipient</th><th>Certificates</th></tr></thead>
      <tbody>${cinema.screens.map(screenRowHtml).join("")}</tbody>
    </table>
  </div>`;
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
  return cinemas;
}

function showImportResults(lines) {
  document.getElementById("cinemas-import-results").innerHTML = lines
    .map((line) => `<li>${escapeHtml(line)}</li>`)
    .join("");
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

async function refreshBookings() {
  await fillBookingForm();
  const bookings = await invoke("bookings_list");
  const tbody = document.getElementById("bookings-tbody");
  tbody.innerHTML = bookings
    .map(
      (booking) => `<tr>
        <td>${escapeHtml(booking.contentTitle)}</td>
        <td>${escapeHtml(booking.start)}</td>
        <td>${escapeHtml(booking.end)}</td>
        <td>${escapeHtml(booking.screens.map(screenLabel).join(", "))}</td>
        <td><button class="btn-sm booking-open" data-booking="${booking.id}">Check</button></td>
      </tr>`,
    )
    .join("");
  tbody.querySelectorAll(".booking-open").forEach((button) => {
    button.addEventListener("click", () => run(() => showPlan(Number(button.dataset.booking))));
  });
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
  const plan = await invoke("bookings_plan", { id: bookingId });
  document.getElementById("booking-detail").hidden = false;
  document.getElementById("booking-detail-title").textContent = plan.contentTitle;
  document.getElementById("booking-plan-notes").textContent = plan.notChecked.join(" ");
  document.getElementById("booking-detail-tbody").innerHTML = detailRowsHtml(planRows(plan));
  document.getElementById("issue-deliveries").innerHTML = "";
}

document.getElementById("booking-form").addEventListener("submit", (event) => {
  event.preventDefault();
  run(async () => {
    const screenIds = [...document.querySelectorAll("#booking-screens input:checked")].map(
      (input) => input.value,
    );
    const { booking, error } = newBooking({
      titleId: document.getElementById("booking-title").value,
      screenIds,
      start: document.getElementById("booking-start").value,
      end: document.getElementById("booking-end").value,
      formulation: document.getElementById("booking-formulation").value,
    });
    document.getElementById("booking-error").textContent = error ?? "";
    if (error) return;
    const id = await invoke("bookings_add", { booking });
    await refreshBookings();
    await showPlan(id);
  });
});

document.getElementById("issue-folder").addEventListener("click", () =>
  run(async () => {
    issueFolder = (await pickFile(FOLDER_FILTER)) ?? issueFolder;
    document.getElementById("issue-folder-path").textContent = issueFolder ?? "";
  }),
);

document.getElementById("issue-run").addEventListener("click", () =>
  run(async () => {
    if (selectedBookingId === null) return;
    const result = await invoke("bookings_issue", {
      id: selectedBookingId,
      outputFolder: issueFolder,
      sendEmail: document.getElementById("issue-email").checked,
    });
    document.getElementById("booking-detail-tbody").innerHTML = detailRowsHtml(outcomeRows(result.outcome));
    document.getElementById("issue-deliveries").innerHTML = result.deliveries
      .map((delivery) => `<li>${escapeHtml(delivery.cinema)}: ${escapeHtml(delivery.zipPath)}, ${escapeHtml(deliveryText(delivery))}</li>`)
      .join("");
    showStatus(`Issued ${result.outcome.bundles.length} ZIP(s), ${result.outcome.refused.length} screen(s) refused`);
  }),
);

// outbox

async function refreshOutbox() {
  const outbox = await invoke("outbox_list");
  document.getElementById("deliveries-tbody").innerHTML = outbox.deliveries
    .map(
      (delivery) => `<tr>
        <td>${escapeHtml(formatDateTime(delivery.deliveredAt))}</td>
        <td>${escapeHtml(delivery.cinema)}</td>
        <td>${escapeHtml(delivery.zipPath)}</td>
        <td class="status-${delivery.result.kind}">${escapeHtml(deliveryText(delivery))}</td>
      </tr>`,
    )
    .join("");
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
