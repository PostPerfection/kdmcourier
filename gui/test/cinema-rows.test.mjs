import { test } from 'node:test';
import assert from 'node:assert/strict';
import { certificateStatusText, cinemaEndingSoonText, cinemaPendingText, devicesText, emailsFromText } from '../src/cinema-rows.js';

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

test('a cinema counts its booked screens that need a KDM', () => {
  assert.equal(cinemaPendingText(0), '');
  assert.equal(cinemaPendingText(1), '1 booked screen needs a KDM');
  assert.equal(cinemaPendingText(3), '3 booked screens need a KDM');
});

test('a cinema counts its bookings ending soon', () => {
  assert.equal(cinemaEndingSoonText(0), '');
  assert.equal(cinemaEndingSoonText(1), '1 booking ends soon');
  assert.equal(cinemaEndingSoonText(2), '2 bookings end soon');
});
