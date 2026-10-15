import { escapeHtml } from '../../extern/guikit/src/html.js';

const NONE_TEXT = '(none)';

function cplMatches(coverage) {
  return coverage.kdm_cpl_id === coverage.cpl_id;
}

export function coveragePassed(coverage) {
  return cplMatches(coverage) && coverage.missing_keys.length === 0;
}

export function coverageLines(coverage) {
  const cplMismatch = cplMatches(coverage) ? [] : [`Error: the KDM is for CPL urn:uuid:${coverage.kdm_cpl_id}`];
  return [
    `KDM key check: ${coveragePassed(coverage) ? 'passed' : 'failed'}`,
    `CPL: urn:uuid:${coverage.cpl_id} ${coverage.content_title}`,
    `Encrypted assets: ${coverage.encrypted_asset_count}, KDM keys: ${coverage.kdm_key_count}`,
    ...cplMismatch,
    ...coverage.missing_keys.map(
      (key) => `Error: reel ${key.reel} ${key.kind} KeyId urn:uuid:${key.key_id} has no key in the KDM`,
    ),
    ...coverage.unused_keys.map(
      (key) => `Warning: KDM key ${key.key_type ?? NONE_TEXT} urn:uuid:${key.key_id} is used by no asset in the CPL`,
    ),
    ...coverage.key_type_mismatches.map(
      (mismatch) =>
        `Warning: reel ${mismatch.reel} ${mismatch.kind} KeyId urn:uuid:${mismatch.key_id} has key type ${mismatch.kdm_type} in the KDM, ${mismatch.expected_type} expected`,
    ),
  ];
}

function notesHtml(lines, statusClass) {
  const items = lines.map((line) => `<li>${escapeHtml(line)}</li>`).join('');
  return `<ul class="notes ${statusClass}">${items}</ul>`;
}

export function coverageHtml(coverage) {
  return notesHtml(coverageLines(coverage), coveragePassed(coverage) ? 'status-issued' : 'status-refused');
}

export function coverageErrorHtml(error) {
  return notesHtml([String(error)], 'status-refused');
}
