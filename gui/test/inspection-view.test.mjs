import { test } from 'node:test';
import assert from 'node:assert/strict';
import { certificateFields, chainCheckLines, chainInspectionHtml, fileInspectionHtml, kdmFields } from '../src/inspection-view.js';

const leaf = {
  subject: 'dnQualifier=a\\+b=,C=US,O=Vendor,CN=SM.Vendor.IMB.1001',
  issuer: 'dnQualifier=c=,C=US,O=Vendor,CN=.Vendor.smpte-430-2.INTERMEDIATE',
  serial: '7',
  not_before: '2026-10-01T00:00:00Z',
  not_after: '2036-10-01T00:00:00Z',
  expired: false,
  rsa_key_bits: 2048,
  is_certificate_authority: false,
  path_length: null,
  key_usage: ['digitalSignature', 'keyEncipherment'],
  signature_algorithm: 'sha256WithRSAEncryption',
  thumbprint: 'ZYHzIvKjje6leNUUVzjar7Ad1jw=',
  dn_qualifier: 'a+b=',
  roles: ['SM'],
};
const root = {
  ...leaf,
  subject: 'dnQualifier=d=,C=US,O=Vendor,CN=.Vendor.smpte-430-2.ROOT',
  issuer: 'dnQualifier=d=,C=US,O=Vendor,CN=.Vendor.smpte-430-2.ROOT',
  is_certificate_authority: true,
  path_length: 3,
  key_usage: ['keyCertSign', 'cRLSign'],
  roles: [],
};
const revocationSkipped = { rule: 'Revocation', reason: 'no revocation list is held' };
const passingChain = {
  certificates: [leaf, root],
  ordering_error: null,
  check: { failures: [], best_effort_failures: [], not_checked: [revocationSkipped] },
};

test('a certificate shows its whole distinguished names and its role', () => {
  const fields = new Map(certificateFields(leaf));
  assert.equal(fields.get('Subject'), leaf.subject);
  assert.equal(fields.get('Issuer'), leaf.issuer);
  assert.equal(fields.get('Certificate authority'), 'no');
  assert.equal(fields.get('Key'), '2048-bit RSA');
  assert.equal(fields.get('Roles'), 'SM');
  const rootFields = new Map(certificateFields(root));
  assert.equal(rootFields.get('Certificate authority'), 'yes, path length 3');
  assert.equal(rootFields.get('Roles'), '(none)');
});

test('the chain check names its order, verdict and every finding', () => {
  assert.deepEqual(chainCheckLines(passingChain), [
    'Order: leaf first',
    'Chain check: passed',
    'Not checked: Revocation: no revocation list is held',
  ]);
  const failing = {
    ...passingChain,
    ordering_error: 'the certificates do not form one chain: 2 of them sign no other certificate',
    check: {
      failures: [{ rule: 'Issuer', certificate: 'CN=a', detail: 'chain broken' }],
      best_effort_failures: [{ rule: 'AuthorityKeyIdentifier', certificate: 'CN=b', detail: 'no match' }],
      not_checked: [],
    },
  };
  assert.deepEqual(chainCheckLines(failing), [
    'Order: as found, the certificates do not form one chain: 2 of them sign no other certificate',
    'Chain check: failed',
    'CN=a: Issuer: chain broken',
    'Best effort: CN=b: AuthorityKeyIdentifier: no match',
  ]);
});

test('every certificate of a chain is rendered in order', () => {
  const html = chainInspectionHtml(passingChain);
  assert.ok(html.indexOf('Certificate 1 of 2') < html.indexOf('Certificate 2 of 2'));
  assert.ok(html.indexOf('SM.Vendor.IMB.1001') < html.indexOf('CN=.Vendor.smpte-430-2.ROOT</td>'));
  assert.ok(html.includes('status-issued'));
});

test('a KDM shows its keys, its signature result and its signer chain', () => {
  const kdm = {
    kind: 'kdm',
    metadata: {
      cpl_id: '8a2b1c3d-4e5f-6071-8293-a4b5c6d7e8f9',
      content_title: 'Courier_FTR',
      annotation_text: '',
      not_valid_before: '2026-10-16T00:00:00+00:00',
      not_valid_after: '2026-11-16T00:00:00+00:00',
      key_ids: [],
      recipient_subject_name: null,
      content_authenticator: null,
    },
    key_ids: [
      { key_type: 'MDIK', key_id: '00000000-0000-0000-0000-000000001111' },
      { key_type: null, key_id: '00000000-0000-0000-0000-000000002222' },
    ],
    signature: { result: 'failed', detail: 'KDM signature is invalid: digest mismatch' },
    signer_chain: passingChain,
    undecodable_certificates: 0,
  };
  const fields = new Map(kdmFields(kdm));
  assert.equal(
    fields.get('Keys: 2'),
    'MDIK urn:uuid:00000000-0000-0000-0000-000000001111\n(none) urn:uuid:00000000-0000-0000-0000-000000002222',
  );
  assert.equal(fields.get('Signature'), 'failed, KDM signature is invalid: digest mismatch');
  assert.equal(fields.get('Recipient'), '(none)');
  const html = fileInspectionHtml(kdm);
  assert.ok(html.indexOf('<h3>KDM</h3>') < html.indexOf('<h3>Signer chain</h3>'));
  assert.ok(html.includes('Certificate 2 of 2'));
});
