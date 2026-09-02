//! mail-ical — reading and replying to meeting invitations (iTIP).
//!
//! PURE crate (zero I/O, zero clock): it turns an iCalendar text
//! (RFC 5545/5546) into an `Invitation` ready to display, and builds the
//! `METHOD:REPLY` of a reply. The parser is `calcard` (Stalwart, same house
//! as mail-parser) — decision D1 of PLAN-INVITATIONS, decided by spikes on
//! a common corpus.
//!
//! Time-zone guard (D1): a TZID outside calcard's IANA/Windows tables is
//! NEVER converted into an instant — the time is rendered
//! `When::Floating`, displayed as is by the UI ("organizer's local time"),
//! never a lying conversion. The trap was measured at the spike:
//! `resolve_or_default` falls back to floating-treated-as-UTC, a SILENT
//! one-hour offset — hence `resolve()` and the explicit handling of `None`.

use calcard::common::{PartialDateTime, timezone::Tz};
use calcard::icalendar::timezone::TzResolver;
use calcard::icalendar::{
    ICalendar, ICalendarComponent, ICalendarComponentType, ICalendarEntry, ICalendarMethod,
    ICalendarParameter, ICalendarParameterName, ICalendarParameterValue,
    ICalendarParticipationStatus, ICalendarProperty, ICalendarValue, ICalendarValueType,
};
use calcard::{Entry, Parser};
use chrono::Utc;
use thiserror::Error;

/// The iTIP method of the message (RFC 5546).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// `METHOD:REQUEST` — an invitation to answer.
    Request,
    /// `METHOD:CANCEL` — the meeting is cancelled.
    Cancel,
    /// `METHOD:REPLY` — the reply of an attendee (we are the organizer).
    Reply,
}

/// The participation status (PARTSTAT, RFC 5545 §3.2.12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Participation {
    /// `NEEDS-ACTION` — or PARTSTAT absent (the RFC default).
    NeedsAction,
    /// `ACCEPTED`.
    Accepted,
    /// `TENTATIVE`.
    Tentative,
    /// `DECLINED`.
    Declined,
}

/// A person of the invitation (organizer or attendee).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    /// The bare address, without `mailto:`.
    pub address: String,
    /// The display name (CN parameter), if given.
    pub name: Option<String>,
}

/// A time of the event — the resolution is stated, never guessed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum When {
    /// Instant resolved in UTC (epoch seconds).
    Instant(i64),
    /// Whole day (`VALUE=DATE`) — `YYYY-MM-DD`.
    Day(String),
    /// UNRESOLVED local time (unknown TZID or floating time) —
    /// `YYYY-MM-DDTHH:MM`, to display as is.
    Floating(String),
}

/// An invitation read from a `text/calendar` part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invitation {
    pub method: Method,
    /// The event's UID — the identity of the meeting across messages
    /// (REQUEST, CANCEL and REPLY share it).
    pub uid: String,
    /// SEQUENCE (0 if absent) — the revision number of the meeting.
    pub sequence: i64,
    /// Unescaped SUMMARY; empty string if absent.
    pub title: String,
    /// Unescaped LOCATION.
    pub location: Option<String>,
    pub organizer: Option<Person>,
    pub start: Option<When>,
    pub end: Option<When>,
    /// The event carries an RRULE — the card says "repeats", nothing more
    /// (scope refusal: no expansion).
    pub recurrent: bool,
    /// Our PARTSTAT in a REQUEST (None: we are not in the attendee list).
    pub our_participation: Option<Participation>,
    /// The attendee who replies, in a received REPLY.
    pub attendee: Option<Person>,
    /// Their PARTSTAT.
    pub attendee_participation: Option<Participation>,
}

/// The data needed for a `METHOD:REPLY` — all of it comes from the stored
/// `invitations` row, never from a re-parse.
#[derive(Debug, Clone)]
pub struct ReplyRequest<'a> {
    pub uid: &'a str,
    pub sequence: i64,
    pub organizer_address: &'a str,
    pub our_address: &'a str,
    pub participation: Participation,
    /// DTSTAMP in epoch seconds — provided by the caller (the crate has no
    /// clock).
    pub dtstamp_epoch: i64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IcalError {
    #[error("the text is not a readable iCalendar")]
    Unreadable,
    #[error("no VEVENT in the calendar")]
    NoEvent,
    #[error("the VEVENT carries no UID")]
    NoUid,
    #[error("iTIP method absent or unsupported")]
    UnknownMethod,
}

/// Reads a `text/calendar` part and extracts the invitation.
///
/// `our_address` is used to find OUR attendee in the list (case-insensitive
/// comparison — providers rewrite the case of addresses).
pub fn parse(ics: &str, our_address: &str) -> Result<Invitation, IcalError> {
    let ical = match Parser::new(ics).entry() {
        Entry::ICalendar(ical) => ical,
        _ => return Err(IcalError::Unreadable),
    };

    let method = calendar_method(&ical).ok_or(IcalError::UnknownMethod)?;
    let vevent = ical
        .components
        .iter()
        .find(|c| c.component_type == ICalendarComponentType::VEvent)
        .ok_or(IcalError::NoEvent)?;

    let uid = vevent.uid().map(str::to_string).ok_or(IcalError::NoUid)?;

    let organizer = vevent
        .property(&ICalendarProperty::Organizer)
        .and_then(person_of_entry);

    // Our attendee (REQUEST) and the replying attendee (REPLY) come out of
    // the same ATTENDEE list. A conforming REPLY carries only one — but
    // Exchange sometimes ECHOES the organizer at the head of the list
    // (review): the attendee is therefore the first ATTENDEE who is NOT the
    // organizer; failing that only, the first one.
    let organizer_address = organizer.as_ref().map(|o| o.address.clone());
    let mut our_participation = None;
    let mut attendee = None;
    let mut attendee_participation = None;
    let mut attendee_is_organizer = false;
    for att in vevent.properties(&ICalendarProperty::Attendee) {
        let status = participation_of_entry(att);
        if matches!(method, Method::Reply) {
            let is_organizer = match (&organizer_address, att.calendar_address()) {
                (Some(org), Some(address)) => address.eq_ignore_ascii_case(org),
                _ => false,
            };
            if attendee.is_none() || (attendee_is_organizer && !is_organizer) {
                attendee = person_of_entry(att);
                attendee_participation = Some(status);
                attendee_is_organizer = is_organizer;
            }
        }
        if att
            .calendar_address()
            .is_some_and(|a| a.eq_ignore_ascii_case(our_address))
        {
            our_participation = Some(status);
        }
    }
    // The time-zone resolver is built ONCE (it materializes the VTIMEZONE)
    // and serves start and end.
    let resolver = ical.build_tz_resolver();

    Ok(Invitation {
        method,
        uid,
        sequence: vevent
            .property(&ICalendarProperty::Sequence)
            .and_then(|e| e.values.first())
            .and_then(|v| v.as_integer())
            .unwrap_or(0),
        title: text_of(vevent, &ICalendarProperty::Summary).unwrap_or_default(),
        location: text_of(vevent, &ICalendarProperty::Location),
        organizer,
        start: vevent
            .property(&ICalendarProperty::Dtstart)
            .and_then(|e| when_of_entry(e, &resolver)),
        end: vevent
            .property(&ICalendarProperty::Dtend)
            .and_then(|e| when_of_entry(e, &resolver)),
        recurrent: vevent.has_property(&ICalendarProperty::Rrule),
        our_participation,
        attendee,
        attendee_participation,
    })
}

/// Builds the iCalendar text of a reply (`METHOD:REPLY`), CRLF, lines
/// folded at 75 bytes (calcard's writer folds natively — proven at the
/// spike).
pub fn itip_reply(request: &ReplyRequest<'_>) -> String {
    let mut vcal = ICalendarComponent::new(ICalendarComponentType::VCalendar);
    vcal.add_property(ICalendarProperty::Version, "2.0");
    vcal.add_property(ICalendarProperty::Prodid, "-//Wind//mail-ical//EN");
    vcal.add_property(
        ICalendarProperty::Method,
        ICalendarValue::Method(ICalendarMethod::Reply),
    );

    let mut vevent = ICalendarComponent::new(ICalendarComponentType::VEvent);
    vevent.add_uid(request.uid);
    vevent.add_sequence(request.sequence);
    vevent.add_dtstamp(PartialDateTime::from_utc_timestamp(request.dtstamp_epoch));
    vevent.add_property(
        ICalendarProperty::Organizer,
        format!("mailto:{}", request.organizer_address),
    );
    vevent.entries.push(
        ICalendarEntry::new(ICalendarProperty::Attendee)
            .with_param(ICalendarParameter::partstat(
                ICalendarParameterValue::Partstat(partstat_of(request.participation)),
            ))
            .with_value(format!("mailto:{}", request.our_address)),
    );

    vcal.component_ids = vec![1];
    ICalendar {
        components: vec![vcal, vevent],
    }
    .to_string()
}

fn calendar_method(ical: &ICalendar) -> Option<Method> {
    let vcal = ical
        .components
        .iter()
        .find(|c| c.component_type == ICalendarComponentType::VCalendar)?;
    match vcal.property(&ICalendarProperty::Method)?.values.first()? {
        ICalendarValue::Method(ICalendarMethod::Request) => Some(Method::Request),
        ICalendarValue::Method(ICalendarMethod::Cancel) => Some(Method::Cancel),
        ICalendarValue::Method(ICalendarMethod::Reply) => Some(Method::Reply),
        _ => None,
    }
}

fn text_of(comp: &ICalendarComponent, prop: &ICalendarProperty) -> Option<String> {
    comp.property(prop)
        .and_then(|e| e.values.first())
        .and_then(|v| v.as_text())
        .map(str::to_string)
}

fn person_of_entry(entry: &ICalendarEntry) -> Option<Person> {
    Some(Person {
        address: entry.calendar_address()?.to_string(),
        name: entry
            .parameter(&ICalendarParameterName::Cn)
            .and_then(|v| v.as_text())
            .map(str::to_string),
    })
}

fn participation_of_entry(entry: &ICalendarEntry) -> Participation {
    match entry.parameter(&ICalendarParameterName::Partstat) {
        Some(ICalendarParameterValue::Partstat(p)) => match p {
            ICalendarParticipationStatus::Accepted => Participation::Accepted,
            ICalendarParticipationStatus::Tentative => Participation::Tentative,
            ICalendarParticipationStatus::Declined => Participation::Declined,
            _ => Participation::NeedsAction,
        },
        // PARTSTAT absent: NEEDS-ACTION is the RFC default.
        _ => Participation::NeedsAction,
    }
}

fn partstat_of(participation: Participation) -> ICalendarParticipationStatus {
    match participation {
        Participation::Accepted => ICalendarParticipationStatus::Accepted,
        Participation::Tentative => ICalendarParticipationStatus::Tentative,
        Participation::Declined => ICalendarParticipationStatus::Declined,
        Participation::NeedsAction => ICalendarParticipationStatus::NeedsAction,
    }
}

fn is_date_only(entry: &ICalendarEntry) -> bool {
    entry
        .parameter(&ICalendarParameterName::Value)
        .is_some_and(|v| matches!(v, ICalendarParameterValue::Value(ICalendarValueType::Date)))
}

fn when_of_entry(entry: &ICalendarEntry, resolver: &TzResolver<&str>) -> Option<When> {
    let pdt = entry.values.first()?.as_partial_date_time()?;
    if is_date_only(entry) {
        return Some(When::Day(format!(
            "{:04}-{:02}-{:02}",
            pdt.year?, pdt.month?, pdt.day?
        )));
    }
    // The offset carried by the value itself (Z suffix) wins over any time
    // zone: calcard applies it whatever tz is passed.
    if pdt.tz_hour.is_some() {
        let dt = pdt.to_date_time_with_tz(Tz::Floating)?;
        return Some(When::Instant(dt.with_timezone(&Utc).timestamp()));
    }
    match entry.tz_id() {
        Some(tzid) => match resolver.resolve(tzid) {
            // `.single()` may return None in the gap of a DST change — we
            // then fall back to floating, never to a wrong value.
            Some(tz) => match pdt.to_date_time_with_tz(tz) {
                Some(dt) => Some(When::Instant(dt.with_timezone(&Utc).timestamp())),
                None => Some(floating(pdt)),
            },
            // TZID outside the tables: the D1 guard — time stated, not converted.
            None => Some(floating(pdt)),
        },
        // Neither offset nor TZID: floating time in the RFC sense.
        None => Some(floating(pdt)),
    }
}

fn floating(pdt: &PartialDateTime) -> When {
    When::Floating(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}",
        pdt.year.unwrap_or(0),
        pdt.month.unwrap_or(0),
        pdt.day.unwrap_or(0),
        pdt.hour.unwrap_or(0),
        pdt.minute.unwrap_or(0)
    ))
}
