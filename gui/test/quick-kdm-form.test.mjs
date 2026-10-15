import { test } from 'node:test';
import assert from 'node:assert/strict';
import { quickKdmOutcome, quickKdmRequest } from '../src/quick-kdm-form.js';

const fields = {
  titleId: '2',
  recipient: { kind: 'file', path: '/certs/screen.pem' },
  trustedDevices: '/certs/ld.pem\n\n /certs/pr.pem ',
  mediaBlockSerial: ' ',
  start: '2026-11-01T18:00',
  end: '2026-11-08T23:00',
  timeZone: ' Europe/London ',
  formulation: '',
};

test('a quick KDM sends the title, certificate, devices, window, zone and formulation', () => {
  assert.deepEqual(quickKdmRequest(fields), {
    request: {
      titleId: 2,
      recipient: { kind: 'file', path: '/certs/screen.pem' },
      trustedDevices: ['/certs/ld.pem', '/certs/pr.pem'],
      mediaBlockSerial: null,
      start: '2026-11-01T18:00:00',
      end: '2026-11-08T23:00:00',
      timeZone: 'Europe/London',
      formulation: null,
    },
  });
  const fetched = quickKdmRequest({ ...fields, recipient: { kind: 'pem', pem: 'PEM' }, mediaBlockSerial: '218281828', formulation: 'dci-any' });
  assert.equal(fetched.request.mediaBlockSerial, '218281828');
  assert.equal(fetched.request.formulation, 'dci-any');
});

test('a quick KDM needs a title, a certificate, an end after its start and a zone', () => {
  assert.equal(quickKdmRequest({ ...fields, titleId: '' }).error, 'Pick a title');
  assert.equal(quickKdmRequest({ ...fields, recipient: null }).error, 'Pick or fetch the recipient certificate');
  assert.equal(quickKdmRequest({ ...fields, end: '2026-11-01T17:00' }).error, 'The end has to be after the start');
  assert.equal(quickKdmRequest({ ...fields, timeZone: '' }).error, 'Set the time zone');
});

test('an issued KDM shows its file and a refused one every rule it fails', () => {
  const issued = quickKdmOutcome({
    written: {
      path: '/kdms/k_Title.xml',
      kdm: { formulation: 'modified-transitional-1', notValidBefore: 'a', notValidAfter: 'b', warnings: ['short'] },
    },
    refusals: [],
    notChecked: ['rule 12 not checked'],
  });
  assert.deepEqual(issued, {
    status: 'issued',
    lines: ['Written to /kdms/k_Title.xml', 'modified-transitional-1, a to b', 'short', 'rule 12 not checked'],
  });
  const refused = quickKdmOutcome({ written: null, refusals: ['SM: ST 430-2 rule 8 (role)'], notChecked: [] });
  assert.deepEqual(refused, { status: 'refused', lines: ['SM: ST 430-2 rule 8 (role)'] });
});
