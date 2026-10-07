import { test } from 'node:test';
import assert from 'node:assert/strict';
import { bookingExpiryRow, nothingExpires, signerChainRow } from '../src/expiry-rows.js';

const recipient = {
  bookingId: 4,
  contentTitle: 'Courier_FTR',
  cinema: 'Rex',
  screen: '1',
  item: 'recipient',
  subject: 'CN=SM.Vendor.IMB.1001',
  expiresAt: '2026-10-12T10:00:00Z',
  bookingEndsAt: '2026-10-15T22:00:00Z',
};

test('a screen certificate names its screen and the DKDM covers every booked screen', () => {
  assert.deepEqual(bookingExpiryRow(recipient), {
    title: 'Courier_FTR',
    cinema: 'Rex',
    screen: '1',
    item: 'recipient certificate',
    subject: 'CN=SM.Vendor.IMB.1001',
    expiresAt: '2026-10-12T10:00:00Z',
    bookingEndsAt: '2026-10-15T22:00:00Z',
  });
  assert.equal(bookingExpiryRow({ ...recipient, item: 'authorizedDevice' }).item, 'authorized device certificate');
  const dkdm = bookingExpiryRow({ ...recipient, screen: null, item: 'dkdm', subject: 'urn:uuid:8a2b' });
  assert.equal(dkdm.screen, 'every booked screen');
  assert.equal(dkdm.item, 'DKDM');
});

test('a signer certificate lists the titles of the bookings that end after it once each', () => {
  const bookings = [
    { id: 4, contentTitle: 'Courier_FTR' },
    { id: 5, contentTitle: 'Courier_FTR' },
    { id: 6, contentTitle: 'Other_TLR' },
  ];
  const row = signerChainRow({ subject: 'CN=signer', expiresAt: '2026-10-20T00:00:00Z', bookingsEndingAfter: [4, 5, 6] }, bookings);
  assert.deepEqual(row, { subject: 'CN=signer', expiresAt: '2026-10-20T00:00:00Z', bookingsEndingAfter: 'Courier_FTR, Other_TLR' });
  assert.equal(signerChainRow({ subject: 'CN=root', expiresAt: '2046-10-20T00:00:00Z', bookingsEndingAfter: [] }, bookings).bookingsEndingAfter, '');
});

test('nothing expires when no booking entry and no signer certificate ends early', () => {
  const quietSigner = { subject: 'CN=root', expiresAt: '2046-10-20T00:00:00Z', bookingsEndingAfter: [] };
  assert.equal(nothingExpires({ bookings: [], signerChain: [quietSigner], notChecked: [] }), true);
  assert.equal(nothingExpires({ bookings: [recipient], signerChain: [quietSigner], notChecked: [] }), false);
  assert.equal(nothingExpires({ bookings: [], signerChain: [{ ...quietSigner, bookingsEndingAfter: [4] }], notChecked: [] }), false);
});
