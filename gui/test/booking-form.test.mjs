import { test } from 'node:test';
import assert from 'node:assert/strict';
import { bookingChange, bookingFields, bookingLocalTime, bookingStateText, newBooking, planRows } from '../src/booking-form.js';

test('a datetime-local value gains seconds and an empty one is no time', () => {
  assert.equal(bookingLocalTime('2026-11-01T18:00'), '2026-11-01T18:00:00');
  assert.equal(bookingLocalTime('2026-11-01T18:00:30'), '2026-11-01T18:00:30');
  assert.equal(bookingLocalTime(''), null);
});

test('a booking needs a title, a screen and an end after its start', () => {
  const fields = { titleId: '3', screenIds: ['1', '4'], start: '2026-11-01T18:00', end: '2026-11-08T23:00', formulation: '' };
  assert.deepEqual(newBooking(fields), {
    booking: { titleId: 3, screenIds: [1, 4], start: '2026-11-01T18:00:00', end: '2026-11-08T23:00:00', formulation: null },
  });
  assert.equal(newBooking({ ...fields, titleId: '' }).error, 'Pick a title');
  assert.equal(newBooking({ ...fields, screenIds: [] }).error, 'Pick at least one screen');
  assert.equal(newBooking({ ...fields, end: '2026-11-01T17:00' }).error, 'The end has to be after the start');
  assert.equal(newBooking({ ...fields, formulation: 'dci-specific' }).booking.formulation, 'dci-specific');
});

test('a refused screen lists its reasons and an issued one its window and warnings', () => {
  const rows = planRows({
    screens: [
      { cinema: 'Rex', screen: '1', notValidBefore: '2026-11-01T18:00:00+00:00', notValidAfter: '2026-11-08T23:00:00+00:00', formulation: 'modified-transitional-1', refusals: [], warnings: ['assume trust on 2 devices'] },
      { cinema: 'Rex', screen: '3', notValidBefore: null, notValidAfter: null, formulation: null, refusals: ['Rex / 3: ST 430-2 rule 8 (role)'], warnings: [] },
    ],
  });
  assert.deepEqual(rows, [
    { screen: 'Rex / 1', status: 'ready', formulation: 'modified-transitional-1', window: '2026-11-01T18:00:00+00:00 to 2026-11-08T23:00:00+00:00', notes: ['assume trust on 2 devices'] },
    { screen: 'Rex / 3', status: 'refused', formulation: '', window: '', notes: ['Rex / 3: ST 430-2 rule 8 (role)'] },
  ]);
});

test('an edit sends the screens, window and formulation and keeps the title', () => {
  assert.deepEqual(bookingChange({ screenIds: ['2'], start: '2026-11-01T18:00', end: '2026-11-09T23:00', formulation: '' }), {
    change: { screenIds: [2], start: '2026-11-01T18:00:00', end: '2026-11-09T23:00:00', formulation: null },
  });
  assert.equal(bookingChange({ screenIds: [], start: '2026-11-01T18:00', end: '2026-11-09T23:00', formulation: '' }).error, 'Pick at least one screen');
});

test('a stored booking fills the form to the minute and says when it needs issuing again', () => {
  const booking = {
    titleId: 3,
    screens: [{ id: 1 }, { id: 4 }],
    start: '2026-11-01T18:00:00',
    end: '2026-11-08T23:00:00',
    formulation: null,
    needsReissue: true,
  };
  assert.deepEqual(bookingFields(booking), {
    titleId: '3',
    screenIds: ['1', '4'],
    start: '2026-11-01T18:00',
    end: '2026-11-08T23:00',
    formulation: '',
  });
  assert.equal(bookingStateText(booking), 'edited since its KDMs were issued, issue again');
  assert.equal(bookingStateText({ ...booking, needsReissue: false }), '');
});
