import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  feedUpdateFromFields,
  passwordChange,
  pathsFromText,
  settingsUpdateFromFields,
  smtpFromFields,
  vendorAccountUpdate,
} from '../src/settings-form.js';

const smtp = { host: ' smtp.dist.test ', port: '587', security: 'starttls', username: '', from: 'kdm@dist.test', bodyTemplate: '' };

test('an untouched password field keeps the stored password, a typed one replaces it and the box clears it', () => {
  assert.deepEqual(passwordChange('', false), { action: 'keep' });
  assert.deepEqual(passwordChange('hunter2', false), { action: 'set', password: 'hunter2' });
  assert.deepEqual(passwordChange('hunter2', true), { action: 'clear' });
});

test('no SMTP host means no SMTP server', () => {
  assert.equal(smtpFromFields({ ...smtp, host: '  ' }), null);
  assert.deepEqual(smtpFromFields(smtp), {
    host: 'smtp.dist.test',
    port: 587,
    security: 'starttls',
    username: null,
    from: 'kdm@dist.test',
    bodyTemplate: null,
  });
});

test('the chain is one path per line and the facility code is upper case', () => {
  assert.deepEqual(pathsFromText('/keys/intermediate.pem\n\n /keys/root.pem \n'), ['/keys/intermediate.pem', '/keys/root.pem']);
  const update = settingsUpdateFromFields({
    databasePath: '',
    signerCertificate: '/keys/signer.pem',
    signerKey: '',
    signerChain: '/keys/root.pem',
    dkdmRecipientKey: '/keys/signer.key',
    creationFacility: 'dis ',
    outputFolder: '',
    smtp,
    newPassword: '',
    clearPassword: false,
    feeds: [],
    vendorAccounts: [],
  });
  assert.equal(update.databasePath, null);
  assert.equal(update.signerKey, null);
  assert.deepEqual(update.signerChain, ['/keys/root.pem']);
  assert.equal(update.creationFacility, 'DIS');
  assert.deepEqual(update.smtpPassword, { action: 'keep' });
});

const feed = {
  name: ' Exhibitor ',
  siteListUrl: ' https://tms.exhibitor.test/sitelist.xml ',
  kind: 'basic',
  username: ' distributor ',
  newPassword: '',
  clearPassword: false,
  certificateChain: '/keys/client.pem',
  privateKey: '/keys/client.key',
};

test('a feed sends only the fields of its authentication, and its password the way SMTP does', () => {
  assert.deepEqual(feedUpdateFromFields(feed), {
    name: 'Exhibitor',
    siteListUrl: 'https://tms.exhibitor.test/sitelist.xml',
    authentication: { kind: 'basic', username: 'distributor', password: { action: 'keep' } },
  });
  assert.deepEqual(feedUpdateFromFields({ ...feed, newPassword: 'hunter2' }).authentication.password, {
    action: 'set',
    password: 'hunter2',
  });
  assert.deepEqual(feedUpdateFromFields({ ...feed, kind: 'clientCertificate' }).authentication, {
    kind: 'clientCertificate',
    certificateChain: '/keys/client.pem',
    privateKey: '/keys/client.key',
  });
  assert.deepEqual(feedUpdateFromFields({ ...feed, kind: 'none' }).authentication, { kind: 'none' });
});

test('a feed row without a SiteList URL is dropped and a blank name becomes the URL', () => {
  assert.equal(feedUpdateFromFields({ ...feed, siteListUrl: '  ' }), null);
  assert.equal(feedUpdateFromFields({ ...feed, name: '' }).name, 'https://tms.exhibitor.test/sitelist.xml');
  const update = settingsUpdateFromFields({
    databasePath: '',
    signerCertificate: '',
    signerKey: '',
    signerChain: '',
    dkdmRecipientKey: '',
    creationFacility: '',
    outputFolder: '',
    smtp,
    newPassword: '',
    clearPassword: false,
    feeds: [feed, { ...feed, siteListUrl: '' }],
    vendorAccounts: [],
  });
  assert.equal(update.feeds.length, 1);
});

test('a vendor account sends its user trimmed and its password the way SMTP does', () => {
  const row = { vendor: 'barco', user: ' projectionist ', newPassword: '', clearPassword: false };
  assert.deepEqual(vendorAccountUpdate(row), { vendor: 'barco', user: 'projectionist', password: { action: 'keep' } });
  assert.deepEqual(vendorAccountUpdate({ ...row, newPassword: 'hunter2' }).password, { action: 'set', password: 'hunter2' });
  assert.deepEqual(vendorAccountUpdate({ ...row, newPassword: 'hunter2', clearPassword: true }).password, { action: 'clear' });
});
