import { escapeHtml } from '../../extern/guikit/src/html.js';

const NONE_TEXT = '(none)';

function yesNo(value) {
  return value ? 'yes' : 'no';
}

function listOrNone(values) {
  return values.length === 0 ? NONE_TEXT : values.join(', ');
}

export function certificateFields(certificate) {
  const key = certificate.rsa_key_bits === null ? 'not RSA' : `${certificate.rsa_key_bits}-bit RSA`;
  const hasPathLength = certificate.is_certificate_authority && certificate.path_length !== null;
  const certificateAuthority = hasPathLength
    ? `yes, path length ${certificate.path_length}`
    : yesNo(certificate.is_certificate_authority);
  return [
    ['Subject', certificate.subject],
    ['Issuer', certificate.issuer],
    ['Serial', certificate.serial],
    ['Valid', `${certificate.not_before} to ${certificate.not_after}`],
    ['Expired', yesNo(certificate.expired)],
    ['Key', key],
    ['Certificate authority', certificateAuthority],
    ['Key usage', listOrNone(certificate.key_usage)],
    ['Signature algorithm', certificate.signature_algorithm],
    ['Thumbprint', certificate.thumbprint],
    ['dnQualifier', certificate.dn_qualifier ?? NONE_TEXT],
    ['Roles', listOrNone(certificate.roles)],
  ];
}

export function chainCheckLines(chain) {
  const check = chain.check;
  const passed = check.failures.length === 0;
  return [
    chain.ordering_error ? `Order: as found, ${chain.ordering_error}` : 'Order: leaf first',
    `Chain check: ${passed ? 'passed' : 'failed'}`,
    ...check.failures.map((finding) => `${finding.certificate}: ${finding.rule}: ${finding.detail}`),
    ...check.best_effort_failures.map(
      (finding) => `Best effort: ${finding.certificate}: ${finding.rule}: ${finding.detail}`,
    ),
    ...check.not_checked.map((skipped) => `Not checked: ${skipped.rule}: ${skipped.reason}`),
  ];
}

export function kdmFields(kdm) {
  const metadata = kdm.metadata;
  const keys = kdm.key_ids.map((key) => `${key.key_type ?? NONE_TEXT} urn:uuid:${key.key_id}`);
  const signature = kdm.signature.result === 'passed' ? 'passed' : `failed, ${kdm.signature.detail}`;
  const fields = [
    ['Content title', metadata.content_title],
    ['Annotation', metadata.annotation_text],
    ['CPL', `urn:uuid:${metadata.cpl_id}`],
    ['Valid', `${metadata.not_valid_before} to ${metadata.not_valid_after}`],
    ['Recipient', metadata.recipient_subject_name ?? NONE_TEXT],
    ['Content authenticator', metadata.content_authenticator ?? NONE_TEXT],
    [`Keys: ${keys.length}`, keys.join('\n')],
    ['Signature', signature],
  ];
  if (kdm.undecodable_certificates > 0) {
    fields.push(['Signer certificates that are not base64', String(kdm.undecodable_certificates)]);
  }
  return fields;
}

function fieldsTableHtml(fields) {
  const rows = fields
    .map(([label, value]) => `<tr><th>${escapeHtml(label)}</th><td class="distinguished-name">${escapeHtml(value)}</td></tr>`)
    .join('');
  return `<table class="jobs-table inspection-fields"><tbody>${rows}</tbody></table>`;
}

export function chainInspectionHtml(chain) {
  const count = chain.certificates.length;
  const certificates = chain.certificates
    .map((certificate, index) => `<h4>Certificate ${index + 1} of ${count}</h4>${fieldsTableHtml(certificateFields(certificate))}`)
    .join('');
  const statusClass = chain.check.failures.length === 0 ? 'status-issued' : 'status-refused';
  const lines = chainCheckLines(chain)
    .map((line) => `<li>${escapeHtml(line)}</li>`)
    .join('');
  return `${certificates}<ul class="notes ${statusClass}">${lines}</ul>`;
}

export function fileInspectionHtml(inspection) {
  if (inspection.kind === 'certificates') return chainInspectionHtml(inspection);
  return `<h3>KDM</h3>${fieldsTableHtml(kdmFields(inspection))}<h3>Signer chain</h3>${chainInspectionHtml(inspection.signer_chain)}`;
}
