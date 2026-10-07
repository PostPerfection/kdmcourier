const SECONDS_SUFFIX = ':00';
// a datetime-local field gives YYYY-MM-DDTHH:MM, the backend reads seconds too
const MINUTES_ONLY_LENGTH = 16;

export function bookingLocalTime(fieldValue) {
  if (!fieldValue) return null;
  return fieldValue.length === MINUTES_ONLY_LENGTH ? `${fieldValue}${SECONDS_SUFFIX}` : fieldValue;
}

// the screens, window and formulation to send, or the reason they cannot be sent
export function bookingChange({ screenIds, start, end, formulation }) {
  if (screenIds.length === 0) return { error: 'Pick at least one screen' };
  const startTime = bookingLocalTime(start);
  const endTime = bookingLocalTime(end);
  if (!startTime || !endTime) return { error: 'Set the start and the end' };
  if (endTime <= startTime) return { error: 'The end has to be after the start' };
  return {
    change: {
      screenIds: screenIds.map(Number),
      start: startTime,
      end: endTime,
      formulation: formulation || null,
    },
  };
}

export function newBooking(fields) {
  if (!fields.titleId) return { error: 'Pick a title' };
  const { change, error } = bookingChange(fields);
  if (error) return { error };
  return { booking: { titleId: Number(fields.titleId), ...change } };
}

// a stored booking as the form shows it, datetime-local takes minutes
export function bookingFields(booking) {
  return {
    titleId: String(booking.titleId),
    screenIds: booking.screens.map((screen) => String(screen.id)),
    start: booking.start.slice(0, MINUTES_ONLY_LENGTH),
    end: booking.end.slice(0, MINUTES_ONLY_LENGTH),
    formulation: booking.formulation ?? '',
  };
}

export const ISSUE_SCOPE = { allScreens: 'allScreens', pendingScreens: 'pendingScreens' };

export function pendingScreenCount(booking) {
  return booking.screens.filter((screen) => screen.pending).length;
}

export function bookingPendingText(booking) {
  const pending = pendingScreenCount(booking);
  if (pending === 0) return '';
  const total = booking.screens.length;
  return `${pending} of ${total} ${total === 1 ? 'screen' : 'screens'} ${pending === 1 ? 'needs' : 'need'} a KDM`;
}

const HOUR_MILLISECONDS = 60 * 60 * 1000;
const HOURS_PER_DAY = 24;

function countText(count, unit) {
  return `${count} ${unit}${count === 1 ? '' : 's'}`;
}

export function bookingEndingText(booking, now) {
  if (!booking.endsSoonAt) return '';
  const hours = Math.max(1, Math.floor((Date.parse(booking.endsSoonAt) - now.getTime()) / HOUR_MILLISECONDS));
  if (hours < HOURS_PER_DAY) return `ends in ${countText(hours, 'hour')}`;
  return `ends in ${countText(Math.floor(hours / HOURS_PER_DAY), 'day')}`;
}

export function planScope(booking) {
  return pendingScreenCount(booking) > 0 ? ISSUE_SCOPE.pendingScreens : ISSUE_SCOPE.allScreens;
}

export function screenLabel(row) {
  return `${row.cinema} / ${row.screen}`;
}

export function bookedScreenText(screen) {
  return screen.pending ? `${screenLabel(screen)} (needs a KDM)` : screenLabel(screen);
}

// one row per screen: what it gets, or the checks it fails
export function planRows(plan) {
  return plan.screens.map((screen) => {
    const refused = screen.refusals.length > 0;
    return {
      screen: screenLabel(screen),
      status: refused ? 'refused' : 'ready',
      formulation: screen.formulation ?? '',
      window: refused ? '' : `${screen.notValidBefore} to ${screen.notValidAfter}`,
      notes: refused ? screen.refusals : screen.warnings,
    };
  });
}
