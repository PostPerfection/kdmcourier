const SECONDS_SUFFIX = ':00';
// a datetime-local field gives YYYY-MM-DDTHH:MM, the backend reads seconds too
const MINUTES_ONLY_LENGTH = 16;

export function bookingLocalTime(fieldValue) {
  if (!fieldValue) return null;
  return fieldValue.length === MINUTES_ONLY_LENGTH ? `${fieldValue}${SECONDS_SUFFIX}` : fieldValue;
}

// the booking to send, or the reason it cannot be sent
export function newBooking({ titleId, screenIds, start, end, formulation }) {
  if (!titleId) return { error: 'Pick a title' };
  if (screenIds.length === 0) return { error: 'Pick at least one screen' };
  const startTime = bookingLocalTime(start);
  const endTime = bookingLocalTime(end);
  if (!startTime || !endTime) return { error: 'Set the start and the end' };
  if (endTime <= startTime) return { error: 'The end has to be after the start' };
  return {
    booking: {
      titleId: Number(titleId),
      screenIds: screenIds.map(Number),
      start: startTime,
      end: endTime,
      formulation: formulation || null,
    },
  };
}

export function screenLabel(row) {
  return `${row.cinema} / ${row.screen}`;
}

// one row per screen: what it gets, or the checks it fails
export function planRows(plan) {
  return plan.screens.map((screen) => {
    const refused = screen.refusals.length > 0;
    return {
      screen: screenLabel(screen),
      status: refused ? 'refused' : 'issues',
      formulation: screen.formulation ?? '',
      window: refused ? '' : `${screen.notValidBefore} to ${screen.notValidAfter}`,
      notes: refused ? screen.refusals : screen.warnings,
    };
  });
}
