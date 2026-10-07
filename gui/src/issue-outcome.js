import { screenLabel } from './booking-form.js';

export function outcomeRows(outcome) {
  const issued = outcome.bundles.flatMap((bundle) =>
    bundle.kdms.map((kdm) => ({
      screen: screenLabel(kdm),
      status: 'issued',
      detail: kdm.fileName,
      notes: kdm.warnings,
    })),
  );
  const refused = outcome.refused.map((refusal) => ({
    screen: screenLabel(refusal),
    status: 'refused',
    detail: '',
    notes: refusal.reasons,
  }));
  return [...issued, ...refused];
}

export function deliveryText(delivery) {
  switch (delivery.result.kind) {
    case 'sent':
      return `sent to ${delivery.recipients.join(', ')}`;
    case 'failed':
      return `not sent: ${delivery.result.detail}`;
    default:
      return 'written to the folder, not emailed';
  }
}

export function issuedStatus(outcomes) {
  const zips = outcomes.reduce((total, outcome) => total + outcome.bundles.length, 0);
  const refused = outcomes.reduce((total, outcome) => total + outcome.refused.length, 0);
  return `Issued ${zips} ZIP(s), ${refused} screen(s) refused`;
}

export function cinemaIssueStatus(result) {
  const issued = issuedStatus(result.issued.map((booking) => booking.outcome));
  if (result.failed.length === 0) return issued;
  return `${issued}, ${result.failed.length} booking(s) failed`;
}

export function failedBookingText(booking) {
  return `${booking.contentTitle}: ${booking.error}`;
}
