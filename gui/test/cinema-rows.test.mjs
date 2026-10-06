import { test } from 'node:test';
import assert from 'node:assert/strict';
import { certificateStatusText, devicesText, emailsFromText } from '../src/cinema-rows.js';

test('addresses split on commas, spaces and new lines', () => {
  assert.deepEqual(emailsFromText('kdm@rex.test, booth@rex.test\nops@rex.test;'), ['kdm@rex.test', 'booth@rex.test', 'ops@rex.test']);
  assert.deepEqual(emailsFromText('  '), []);
});

test('devices read as type and serial', () => {
  assert.equal(devicesText([]), 'none');
  assert.equal(devicesText([{ deviceType: 'LD', serial: '2001' }, { deviceType: 'PR', serial: null }]), 'LD 2001, PR');
});

test('a failing chain shows its first rule failure and how many more there are', () => {
  const valid = { failures: [], notAfter: 'Oct  1 12:00:00 2036 +00:00' };
  assert.equal(certificateStatusText(valid), 'valid until Oct  1 12:00:00 2036 +00:00');
  const failing = {
    failures: ['Rex / 1: recipient certificate CN=LD: ST 430-2 rule 8 (role): no SM', 'Rex / 1: recipient certificate CN=LD: ST 430-2 rule 9 (desired time): expired'],
    notAfter: '',
  };
  assert.equal(certificateStatusText(failing), 'Rex / 1: recipient certificate CN=LD: ST 430-2 rule 8 (role): no SM (and 1 more)');
});
