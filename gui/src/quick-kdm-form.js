import { bookingLocalTime } from './booking-form.js';
import { pathsFromText } from './settings-form.js';

// the quick_kdm_issue request, or the reason it cannot be sent
export function quickKdmRequest(fields) {
  if (!fields.titleId) return { error: 'Pick a title' };
  if (!fields.recipient) return { error: 'Pick or fetch the recipient certificate' };
  const start = bookingLocalTime(fields.start);
  const end = bookingLocalTime(fields.end);
  if (!start || !end) return { error: 'Set the start and the end' };
  if (end <= start) return { error: 'The end has to be after the start' };
  const timeZone = fields.timeZone.trim();
  if (!timeZone) return { error: 'Set the time zone' };
  return {
    request: {
      titleId: Number(fields.titleId),
      recipient: fields.recipient,
      trustedDevices: pathsFromText(fields.trustedDevices),
      mediaBlockSerial: fields.mediaBlockSerial.trim() || null,
      start,
      end,
      timeZone,
      formulation: fields.formulation || null,
    },
  };
}

// where the KDM went and its warnings, or every reason the checks refused it
export function quickKdmOutcome(outcome) {
  const notes = outcome.notChecked;
  if (!outcome.written) {
    return { status: 'refused', lines: [...outcome.refusals, ...notes] };
  }
  const kdm = outcome.written.kdm;
  return {
    status: 'issued',
    lines: [
      `Written to ${outcome.written.path}`,
      `${kdm.formulation}, ${kdm.notValidBefore} to ${kdm.notValidAfter}`,
      ...kdm.warnings,
      ...notes,
    ],
  };
}
