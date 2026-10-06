import { test } from 'node:test';
import assert from 'node:assert/strict';
import { deliveryText, outcomeRows } from '../src/issue-outcome.js';

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
