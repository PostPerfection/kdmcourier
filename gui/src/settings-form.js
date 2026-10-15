const DEFAULT_SMTP_PORT = 465;

// one path per line, blank lines dropped
export function pathsFromText(text) {
  return text
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

export function passwordChange(newPassword, clearPassword) {
  if (clearPassword) return { action: 'clear' };
  if (newPassword) return { action: 'set', password: newPassword };
  return { action: 'keep' };
}

// an empty host means no SMTP server, so the other SMTP fields are not saved
export function smtpFromFields({ host, port, security, username, from, bodyTemplate }) {
  if (!host.trim()) return null;
  return {
    host: host.trim(),
    port: Number(port) || DEFAULT_SMTP_PORT,
    security,
    username: username.trim() || null,
    from: from.trim(),
    bodyTemplate: bodyTemplate.trim() || null,
  };
}

const FEED_AUTHENTICATION_FROM_FIELDS = {
  none: () => ({ kind: 'none' }),
  basic: (fields) => ({
    kind: 'basic',
    username: fields.username.trim(),
    password: passwordChange(fields.newPassword, fields.clearPassword),
  }),
  clientCertificate: (fields) => ({
    kind: 'clientCertificate',
    certificateChain: fields.certificateChain,
    privateKey: fields.privateKey,
  }),
};

// a row with no SiteList URL is dropped, a blank name falls back to the URL
export function feedUpdateFromFields(fields) {
  const siteListUrl = fields.siteListUrl.trim();
  if (!siteListUrl) return null;
  return {
    name: fields.name.trim() || siteListUrl,
    siteListUrl,
    authentication: FEED_AUTHENTICATION_FROM_FIELDS[fields.kind](fields),
  };
}

export function settingsUpdateFromFields(fields) {
  return {
    databasePath: fields.databasePath.trim() || null,
    signerCertificate: fields.signerCertificate || null,
    signerKey: fields.signerKey || null,
    signerChain: pathsFromText(fields.signerChain),
    dkdmRecipientKey: fields.dkdmRecipientKey || null,
    creationFacility: fields.creationFacility.trim().toUpperCase(),
    outputFolder: fields.outputFolder || null,
    smtp: smtpFromFields(fields.smtp),
    smtpPassword: passwordChange(fields.newPassword, fields.clearPassword),
    feeds: fields.feeds.map(feedUpdateFromFields).filter((feed) => feed !== null),
  };
}
