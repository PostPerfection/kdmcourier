import { test } from 'node:test';
import assert from 'node:assert/strict';
import { coverageErrorHtml, coverageHtml, coverageLines, coveragePassed } from '../src/kdm-coverage-view.js';

const CPL_ID = '8a2b1c3d-4e5f-6071-8293-a4b5c6d7e8f9';
const covered = {
  kdm_cpl_id: CPL_ID,
  cpl_id: CPL_ID,
  content_title: 'Feature <OV>',
  encrypted_asset_count: 2,
  kdm_key_count: 2,
  missing_keys: [],
  unused_keys: [],
  key_type_mismatches: [],
};

test('a covered CPL passes and shows its counts', () => {
  assert.ok(coveragePassed(covered));
  assert.deepEqual(coverageLines(covered), [
    'KDM key check: passed',
    `CPL: urn:uuid:${CPL_ID} Feature <OV>`,
    'Encrypted assets: 2, KDM keys: 2',
  ]);
  const html = coverageHtml(covered);
  assert.ok(html.includes('status-issued'));
  assert.ok(html.includes('Feature &lt;OV&gt;'));
});

test('a missing key fails and every finding is named', () => {
  const coverage = {
    ...covered,
    missing_keys: [{ reel: 2, kind: 'MainSound', key_id: '00000000-0000-0000-0000-000000003333' }],
    unused_keys: [{ key_type: null, key_id: '00000000-0000-0000-0000-000000004444' }],
    key_type_mismatches: [
      {
        reel: 1,
        kind: 'MainPicture',
        key_id: '00000000-0000-0000-0000-000000002222',
        expected_type: 'MDIK',
        kdm_type: 'MDAK',
      },
    ],
  };
  assert.equal(coveragePassed(coverage), false);
  assert.deepEqual(coverageLines(coverage).slice(3), [
    'Error: reel 2 MainSound KeyId urn:uuid:00000000-0000-0000-0000-000000003333 has no key in the KDM',
    'Warning: KDM key (none) urn:uuid:00000000-0000-0000-0000-000000004444 is used by no asset in the CPL',
    'Warning: reel 1 MainPicture KeyId urn:uuid:00000000-0000-0000-0000-000000002222 has key type MDAK in the KDM, MDIK expected',
  ]);
  assert.ok(coverageHtml(coverage).includes('status-refused'));
});

test('a KDM for another CPL fails', () => {
  const coverage = { ...covered, kdm_cpl_id: '0c0c0000-0000-0000-0000-000000000000' };
  assert.equal(coveragePassed(coverage), false);
  assert.ok(coverageLines(coverage).includes('Error: the KDM is for CPL urn:uuid:0c0c0000-0000-0000-0000-000000000000'));
});

test('a check that could not run shows its error', () => {
  const html = coverageErrorHtml('/dcp holds no CPL urn:uuid:x');
  assert.ok(html.includes('status-refused'));
  assert.ok(html.includes('/dcp holds no CPL urn:uuid:x'));
});
