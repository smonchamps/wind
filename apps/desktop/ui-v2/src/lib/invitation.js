// The invitation card (PLAN-INVITATIONS): PURE formatting of the
// view served by the core (it travels with the body, BodyView) —
// times, statuses, lines. The language comes from the catalogue; the
// date grammar is when.js's (absoluteDate, when.month abbreviated).
//
// D1 guard, per ENDPOINT: a FLOATING time (TZID unknown to the core)
// is shown as is, suffixed “organizer's local time” — never
// converted; a resolved-start/floating-end pair never compacts into a
// range that would lie (review).
import { t } from './text.svelte.js';
import { absoluteDate } from './when.js';

const pad = (n) => String(n).padStart(2, '0');

// The local components of a view's time: resolved epoch → the
// workstation's time; otherwise the core's text form (YYYY-MM-DD or
// YYYY-MM-DDTHH:MM), read as is — `floating` says so.
function components(inv, which) {
  const epoch = inv[`${which}_epoch`];
  if (epoch != null) {
    const d = new Date(epoch * 1000);
    return {
      year: d.getFullYear(), month: d.getMonth(), day: d.getDate(),
      time: `${pad(d.getHours())}:${pad(d.getMinutes())}`,
      floating: false,
    };
  }
  const text = inv[`${which}_text`];
  if (!text) return null;
  const [date, time] = text.split('T');
  const [year, month, day] = date.split('-').map(Number);
  return { year, month: month - 1, day, time: time ?? null, floating: time != null };
}

// The date tile: abbreviated month + day number — the card's visual
// marker, drawn in --tile/--tileInk (the current mailbox's pair).
export function invitationTile(inv) {
  const start = components(inv, 'start');
  if (!start) return null;
  return { month: t('when.month')[start.month], day: String(start.day) };
}

// “Jeudi 3 sept.” — weekday + when.js's grammar.
function longDate(c) {
  const week = t('when.days')[new Date(c.year, c.month, c.day).getDay()];
  return `${week} ${absoluteDate(c.year, c.month, c.day)}`;
}

const sameDay = (a, b) =>
  b && a.year === b.year && a.month === b.month && a.day === b.day;

export function whenInvitation(inv) {
  const start = components(inv, 'start');
  if (!start) return '';
  const end = components(inv, 'end');
  let line;
  if (inv.all_day || !start.time) {
    line = longDate(start);
  } else if (end?.time && sameDay(start, end) && start.floating === end.floating) {
    // The compact range requires endpoints of the SAME resolution:
    // mixing a converted time and a floating time would manufacture
    // “14:30 – 13:30” — the lying conversion that D1 forbids.
    line = `${longDate(start)} · ${start.time} – ${end.time}`;
  } else if (end?.time) {
    line = `${longDate(start)}, ${start.time} – ${longDate(end)}, ${end.time}`;
  } else {
    line = `${longDate(start)} · ${start.time}`;
  }
  if (start.floating || end?.floating) line += ` (${t('inv.localTime')})`;
  if (inv.recurrent) line += ` · ${t('inv.repeats')}`;
  return line;
}

export function invitationKicker(inv) {
  // Cancellation takes priority (cross-link R6): the REQUEST of a
  // cancelled meeting says “Invitation cancelled” just like the
  // CANCEL itself.
  if (inv.cancelled) return t('inv.kickerCancelled');
  if (inv.method === 'reply') return t('inv.kickerReply');
  return t('inv.kicker');
}

// The reply icons (R7/R9): the round counterpart of check_circle
// to decline (`cancel` — never `close`, which keeps its meaning
// “close”, A3), the question mark for tentative. The tone
// carries the color (accent / neutral / alert) — the text always
// doubles the icon (A8).
export const REPLY_ICONS = {
  accepted: 'check_circle',
  tentative: 'question_mark',
  declined: 'cancel',
};

// The list row's chip (R11): cancellation takes priority, otherwise
// the given reply. `null` = nothing to say (the gestures take the place).
export function invitationChip(badge) {
  if (badge.cancelled) {
    return { text: t('inv.chip_cancelled'), icon: null, tone: 'cancelled' };
  }
  if (REPLY_ICONS[badge.reply]) {
    return {
      text: t(`inv.chip_${badge.reply}`),
      icon: REPLY_ICONS[badge.reply],
      tone: badge.reply,
    };
  }
  return null;
}

// The status of an invitation to reply to — D6: the view already
// carries the LAST reply sent from Wind, otherwise the PARTSTAT read
// from the message.
export function invitationStatus(inv) {
  if (inv.method !== 'request') return '';
  return inv.status && inv.status !== 'no_reply'
    ? t(`inv.you_${inv.status}`)
    : t('inv.noReply');
}

// “Sofia Nardi a accepté” — the REPLY received when we organize.
export function attendeeLine(inv) {
  if (inv.method !== 'reply' || !inv.attendee) return '';
  const status = ['accepted', 'tentative', 'declined'].includes(inv.attendee_status)
    ? inv.attendee_status
    : 'no_reply';
  return t(`inv.attendee_${status}`, { who: inv.attendee });
}

export function organizerLocation(inv) {
  if (inv.location && inv.organizer) {
    return t('inv.organizerLocation', { location: inv.location, who: inv.organizer });
  }
  if (inv.location) return inv.location;
  if (inv.organizer) return t('inv.organizedBy', { who: inv.organizer });
  return '';
}
