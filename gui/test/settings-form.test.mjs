import { test } from 'node:test';
import assert from 'node:assert/strict';
import { passwordChange, pathsFromText, settingsUpdateFromFields, smtpFromFields } from '../src/settings-form.js';

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
  });
  assert.equal(update.databasePath, null);
  assert.equal(update.signerKey, null);
  assert.deepEqual(update.signerChain, ['/keys/root.pem']);
  assert.equal(update.creationFacility, 'DIS');
  assert.deepEqual(update.smtpPassword, { action: 'keep' });
});
