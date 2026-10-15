export const VENDORS = [
  { value: 'dolby', label: 'Dolby or Doremi', needsAccount: false },
  { value: 'qube', label: 'Qube', needsAccount: false },
  { value: 'christie', label: 'Christie', needsAccount: true },
  { value: 'gdc', label: 'GDC', needsAccount: true },
  { value: 'barco', label: 'Barco', needsAccount: true },
];

export function vendorLabel(vendor) {
  return VENDORS.find((entry) => entry.value === vendor)?.label ?? vendor;
}

// the certificates_fetch arguments, or the reason they cannot be sent
export function fetchRequest({ vendor, serial, qubeType }) {
  const trimmedSerial = serial.trim();
  if (!trimmedSerial) return { error: 'Type the serial' };
  const trimmedType = qubeType.trim();
  if (vendor === 'qube' && !trimmedType) return { error: 'Type the Qube device type, such as QXPD' };
  return {
    request: {
      vendor,
      serial: trimmedSerial,
      qubeType: vendor === 'qube' ? trimmedType : null,
    },
  };
}

export function fetchedText(certificate) {
  return `${certificate.subject}, valid until ${certificate.notAfter}`;
}
