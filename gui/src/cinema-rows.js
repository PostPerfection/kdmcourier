// addresses separated by commas, spaces or new lines
export function emailsFromText(text) {
  return text
    .split(/[\s,;]+/)
    .map((address) => address.trim())
    .filter((address) => address.length > 0);
}

export function devicesText(devices) {
  if (devices.length === 0) return 'none';
  return devices
    .map((device) => (device.serial ? `${device.deviceType} ${device.serial}` : device.deviceType))
    .join(', ');
}

// the first failure names the screen, the certificate and the rule, the count says how many more
export function certificateStatusText(certificate) {
  const [first, ...rest] = certificate.failures;
  if (!first) return `valid until ${certificate.notAfter}`;
  if (rest.length === 0) return first;
  return `${first} (and ${rest.length} more)`;
}

export function cinemaPendingText(pendingScreens) {
  if (pendingScreens === 0) return '';
  return `${pendingScreens} booked ${pendingScreens === 1 ? 'screen needs' : 'screens need'} a KDM`;
}
