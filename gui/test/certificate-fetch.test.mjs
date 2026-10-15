import { test } from 'node:test';
import assert from 'node:assert/strict';
import { VENDORS, fetchRequest, fetchedText, vendorLabel } from '../src/certificate-fetch.js';

test('a fetch needs a serial, and Qube its device type, which no other vendor sends', () => {
  assert.deepEqual(fetchRequest({ vendor: 'dolby', serial: ' 218281828 ', qubeType: 'QXPD' }), {
    request: { vendor: 'dolby', serial: '218281828', qubeType: null },
  });
  assert.deepEqual(fetchRequest({ vendor: 'qube', serial: '54', qubeType: ' QXPD ' }).request, {
    vendor: 'qube',
    serial: '54',
    qubeType: 'QXPD',
  });
  assert.equal(fetchRequest({ vendor: 'qube', serial: '54', qubeType: '' }).error, 'Type the Qube device type, such as QXPD');
  assert.equal(fetchRequest({ vendor: 'barco', serial: '  ', qubeType: '' }).error, 'Type the serial');
});

test('only Christie, GDC and Barco need an account', () => {
  assert.deepEqual(
    VENDORS.filter((vendor) => vendor.needsAccount).map((vendor) => vendor.value),
    ['christie', 'gdc', 'barco'],
  );
  assert.equal(vendorLabel('gdc'), 'GDC');
  assert.equal(fetchedText({ subject: 'SM.Vendor.IMB.1001', notAfter: '2036-10-01' }), 'SM.Vendor.IMB.1001, valid until 2036-10-01');
});
