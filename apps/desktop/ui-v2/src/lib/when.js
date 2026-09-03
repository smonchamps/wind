// The row's time, in the prototype's exact forms — now per
// language (PLAN-LANGUES, A15): months, days and templates come from
// the catalogue, the grammar does not move. In French: “09:12”
// today, “Hier”, “Lundi” from 2 to 6 days (debt D-3, closed at
// E2 of Settings), “5 août” within the year, “5 août 2024” beyond.
// In English, A15's transposition: "Yesterday", "Monday", "Aug 5",
// "Aug 5, 2024" — the time stays on 24h in both languages.
// Epoch 0 = unknown date -> empty.
import { t } from './text.svelte.js';

// “il y a 2 minutes” — the prototype's exact form for the status
// bar (PLAN-SYNCHRO E1). `now` (ms) comes from the caller: a
// `$state` re-clocked every 30 s, so that “N minutes ago” ages on
// screen without anyone clicking.
export function since(epoch, now) {
  const elapsed = Math.max(0, Math.floor(now / 1000) - epoch);
  if (elapsed < 60) return t('since.now');
  const minutes = Math.floor(elapsed / 60);
  if (minutes < 60) return t('since.minutes', { n: minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return t('since.hours', { n: hours });
  return t('since.days', { n: Math.floor(hours / 24) });
}

// The thread cards' long form: “Aujourd'hui, 09:12”,
// “Hier, 16:30”, “Lundi, 18:20”, “5 août, 10:12” — the one from the
// Classic mockup (field finding A45; the form from before v3, revived
// by the language: day and template come from the catalogue).
export function whenLong(epoch) {
  if (!epoch) return '';
  const date = new Date(epoch * 1000);
  const time = `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`;
  const short = when(epoch);
  const day = short === time ? t('when.today') : short;
  return t('when.long', { day, time });
}

export function when(epoch) {
  if (!epoch) return '';
  const date = new Date(epoch * 1000);
  const now = new Date();
  const day = (d) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const dayGap = Math.round((day(now) - day(date)) / 86400000);
  if (dayGap === 0) {
    return `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`;
  }
  if (dayGap === 1) return t('when.yesterday');
  // The prototype's sliding week: “Lundi, 18:20”.
  if (dayGap >= 2 && dayGap <= 6) return t('when.days')[date.getDay()];
  return absoluteDate(date.getFullYear(), date.getMonth(), date.getDate());
}

// The absolute date — “5 août” within the year, “5 août 2024” beyond,
// “1ᵉʳ” included. THE grammar lives here, once: the list (when) and
// the invitation card share it (PLAN-INVITATIONS review — a copy
// would drift apart at the first language added).
export function absoluteDate(year, monthIndex, day) {
  const ordinalDay = day === 1 ? t('when.first') : String(day);
  const month = t('when.month')[monthIndex];
  if (year === new Date().getFullYear()) {
    return t('when.inYear', { day: ordinalDay, month });
  }
  return t('when.beyond', { day: ordinalDay, month, year });
}
