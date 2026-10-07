import { test } from 'node:test';
import assert from 'node:assert/strict';
import { cinemaIssueStatus, deliveryText, failedBookingText, issuedStatus, outcomeRows } from '../src/issue-outcome.js';

test('issued screens list their file and refused ones their reasons', () => {
  const rows = outcomeRows({
    bundles: [{ kdms: [{ cinema: 'Rex', screen: '1', fileName: 'k_Title_1001.xml', warnings: [] }] }],
    refused: [{ cinema: 'Odeon', screen: 'A', reasons: ['Odeon / A: cinema has no time zone'] }],
  });
  assert.deepEqual(rows, [
    { screen: 'Rex / 1', status: 'issued', detail: 'k_Title_1001.xml', notes: [] },
    { screen: 'Odeon / A', status: 'refused', detail: '', notes: ['Odeon / A: cinema has no time zone'] },
  ]);
});

test('a delivery says where it went or why it did not', () => {
  assert.equal(deliveryText({ recipients: ['kdm@rex.test'], result: { kind: 'sent' } }), 'sent to kdm@rex.test');
  assert.equal(deliveryText({ recipients: [], result: { kind: 'failed', detail: 'smtp send to host failed' } }), 'not sent: smtp send to host failed');
  assert.equal(deliveryText({ recipients: [], result: { kind: 'written' } }), 'written to the folder, not emailed');
});

const twoZips = { bundles: [{ kdms: [] }, { kdms: [] }], refused: [{ reasons: [] }] };

test('the status counts the ZIPs and refused screens of every outcome', () => {
  assert.equal(issuedStatus([twoZips, { bundles: [{ kdms: [] }], refused: [] }]), 'Issued 3 ZIP(s), 1 screen(s) refused');
});

test('a cinema issue counts the bookings that failed and names each with its error', () => {
  const failed = { bookingId: 7, contentTitle: 'Other_TLR', error: 'the DKDM of Other_TLR cannot be read' };
  assert.equal(cinemaIssueStatus({ issued: [{ outcome: twoZips }], failed: [] }), 'Issued 2 ZIP(s), 1 screen(s) refused');
  assert.equal(cinemaIssueStatus({ issued: [{ outcome: twoZips }], failed: [failed] }), 'Issued 2 ZIP(s), 1 screen(s) refused, 1 booking(s) failed');
  assert.equal(failedBookingText(failed), 'Other_TLR: the DKDM of Other_TLR cannot be read');
});
