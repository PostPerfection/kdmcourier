const EXPIRING_ITEM_TEXT = {
  recipient: 'recipient certificate',
  authorizedDevice: 'authorized device certificate',
  dkdm: 'DKDM',
};
const EVERY_BOOKED_SCREEN = 'every booked screen';

export function bookingExpiryRow(entry) {
  return {
    title: entry.contentTitle,
    cinema: entry.cinema,
    screen: entry.screen ?? EVERY_BOOKED_SCREEN,
    item: EXPIRING_ITEM_TEXT[entry.item],
    subject: entry.subject,
    expiresAt: entry.expiresAt,
    bookingEndsAt: entry.bookingEndsAt,
  };
}

export function signerChainRow(certificate, bookings) {
  const titleById = new Map(bookings.map((booking) => [booking.id, booking.contentTitle]));
  const titles = new Set(certificate.bookingsEndingAfter.map((id) => titleById.get(id)));
  return {
    subject: certificate.subject,
    expiresAt: certificate.expiresAt,
    bookingsEndingAfter: [...titles].join(', '),
  };
}

export function nothingExpires(report) {
  return (
    report.bookings.length === 0 &&
    report.signerChain.every((certificate) => certificate.bookingsEndingAfter.length === 0)
  );
}
