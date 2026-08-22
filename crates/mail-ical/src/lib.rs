//! mail-ical — lecture et réponse aux invitations de réunion (iTIP).
//!
//! Crate PURE (zéro I/O, zéro horloge) : elle transforme un texte
//! iCalendar (RFC 5545/5546) en une `Invitation` prête à afficher, et
//! construit le `METHOD:REPLY` d'une réponse. Le parseur est `calcard`
//! (Stalwart, même maison que mail-parser) — décision D1 de
//! PLAN-INVITATIONS, départagée par spikes sur corpus commun.
//!
//! Garde des fuseaux (D1) : un TZID hors des tables IANA/Windows de
//! calcard n'est JAMAIS converti en instant — l'heure est rendue
//! `Quand::Flottant`, affichée telle quelle par l'UI (« heure locale de
//! l'organisateur »), jamais une conversion mensongère. Le piège a été
//! mesuré au spike : `resolve_or_default` retombe en flottant-traité-
//! comme-UTC, un décalage d'une heure SILENCIEUX — d'où `resolve()` et
//! le traitement explicite du `None`.

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

/// La méthode iTIP du message (RFC 5546).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Methode {
    /// `METHOD:REQUEST` — une invitation à répondre.
    Requete,
    /// `METHOD:CANCEL` — la réunion est annulée.
    Annulation,
    /// `METHOD:REPLY` — la réponse d'un participant (nous sommes
    /// l'organisateur).
    Reponse,
}

/// Le statut de participation (PARTSTAT, RFC 5545 §3.2.12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Participation {
    /// `NEEDS-ACTION` — ou PARTSTAT absent (le défaut de la RFC).
    SansReponse,
    /// `ACCEPTED`.
    Accepte,
    /// `TENTATIVE`.
    Provisoire,
    /// `DECLINED`.
    Refuse,
}

/// Une personne de l'invitation (organisateur ou répondant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Personne {
    /// L'adresse nue, sans `mailto:`.
    pub adresse: String,
    /// Le nom d'affichage (paramètre CN), s'il est donné.
    pub nom: Option<String>,
}

/// Un horaire de l'évènement — la résolution est dite, jamais devinée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Quand {
    /// Instant résolu en UTC (secondes epoch).
    Instant(i64),
    /// Journée entière (`VALUE=DATE`) — `AAAA-MM-JJ`.
    Jour(String),
    /// Heure locale NON résolue (TZID inconnu ou heure flottante) —
    /// `AAAA-MM-JJTHH:MM`, à afficher telle quelle.
    Flottant(String),
}

/// Une invitation lue d'une partie `text/calendar`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invitation {
    pub methode: Methode,
    /// L'UID de l'évènement — l'identité de la réunion à travers les
    /// messages (REQUEST, CANCEL et REPLY le partagent).
    pub uid: String,
    /// SEQUENCE (0 si absent) — le numéro de révision de la réunion.
    pub sequence: i64,
    /// SUMMARY déséchappé ; chaîne vide si absent.
    pub titre: String,
    /// LOCATION déséchappée.
    pub lieu: Option<String>,
    pub organisateur: Option<Personne>,
    pub debut: Option<Quand>,
    pub fin: Option<Quand>,
    /// L'évènement porte un RRULE — la carte dit « se répète », rien
    /// de plus (refus de périmètre : pas d'expansion).
    pub recurrent: bool,
    /// Notre PARTSTAT dans un REQUEST (None : nous ne sommes pas dans
    /// la liste des participants).
    pub notre_participation: Option<Participation>,
    /// Le participant qui répond, dans un REPLY reçu.
    pub repondant: Option<Personne>,
    /// Son PARTSTAT.
    pub participation_du_repondant: Option<Participation>,
}

/// Les données nécessaires à un `METHOD:REPLY` — toutes viennent de la
/// ligne `invitations` stockée, jamais d'un re-parse.
#[derive(Debug, Clone)]
pub struct DemandeReponse<'a> {
    pub uid: &'a str,
    pub sequence: i64,
    pub organisateur_adresse: &'a str,
    pub notre_adresse: &'a str,
    pub participation: Participation,
    /// DTSTAMP en secondes epoch — fourni par l'appelant (la crate n'a
    /// pas d'horloge).
    pub dtstamp_epoch: i64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ErreurIcal {
    #[error("le texte n'est pas un iCalendar lisible")]
    Illisible,
    #[error("aucun VEVENT dans le calendrier")]
    SansEvenement,
    #[error("le VEVENT ne porte pas d'UID")]
    SansUid,
    #[error("méthode iTIP absente ou non gérée")]
    MethodeInconnue,
}

/// Lit une partie `text/calendar` et en tire l'invitation.
///
/// `notre_adresse` sert à retrouver NOTRE participant dans la liste
/// (comparaison insensible à la casse — les fournisseurs réécrivent la
/// casse des adresses).
pub fn analyser(ics: &str, notre_adresse: &str) -> Result<Invitation, ErreurIcal> {
    let ical = match Parser::new(ics).entry() {
        Entry::ICalendar(ical) => ical,
        _ => return Err(ErreurIcal::Illisible),
    };

    let methode = methode_du_calendrier(&ical).ok_or(ErreurIcal::MethodeInconnue)?;
    let vevent = ical
        .components
        .iter()
        .find(|c| c.component_type == ICalendarComponentType::VEvent)
        .ok_or(ErreurIcal::SansEvenement)?;

    let uid = vevent
        .uid()
        .map(str::to_string)
        .ok_or(ErreurIcal::SansUid)?;

    let organisateur = vevent
        .property(&ICalendarProperty::Organizer)
        .and_then(personne_de_entree);

    // Notre participant (REQUEST) et le répondant (REPLY) sortent de la
    // même liste ATTENDEE. Un REPLY conforme n'en porte qu'un — mais
    // Exchange ÉCHO parfois l'organisateur en tête de liste (revue) :
    // le répondant est donc le premier ATTENDEE qui n'est PAS
    // l'organisateur ; à défaut seulement, le premier venu.
    let organisateur_adresse = organisateur.as_ref().map(|o| o.adresse.clone());
    let mut notre_participation = None;
    let mut repondant = None;
    let mut participation_du_repondant = None;
    let mut repondant_est_organisateur = false;
    for att in vevent.properties(&ICalendarProperty::Attendee) {
        let statut = participation_de_entree(att);
        if matches!(methode, Methode::Reponse) {
            let est_organisateur = match (&organisateur_adresse, att.calendar_address()) {
                (Some(org), Some(adresse)) => adresse.eq_ignore_ascii_case(org),
                _ => false,
            };
            if repondant.is_none() || (repondant_est_organisateur && !est_organisateur) {
                repondant = personne_de_entree(att);
                participation_du_repondant = Some(statut);
                repondant_est_organisateur = est_organisateur;
            }
        }
        if att
            .calendar_address()
            .is_some_and(|a| a.eq_ignore_ascii_case(notre_adresse))
        {
            notre_participation = Some(statut);
        }
    }
    // Le résolveur de fuseaux se construit UNE fois (il matérialise les
    // VTIMEZONE) et sert début et fin.
    let resolveur = ical.build_tz_resolver();

    Ok(Invitation {
        methode,
        uid,
        sequence: vevent
            .property(&ICalendarProperty::Sequence)
            .and_then(|e| e.values.first())
            .and_then(|v| v.as_integer())
            .unwrap_or(0),
        titre: texte_de(vevent, &ICalendarProperty::Summary).unwrap_or_default(),
        lieu: texte_de(vevent, &ICalendarProperty::Location),
        organisateur,
        debut: vevent
            .property(&ICalendarProperty::Dtstart)
            .and_then(|e| quand_de_entree(e, &resolveur)),
        fin: vevent
            .property(&ICalendarProperty::Dtend)
            .and_then(|e| quand_de_entree(e, &resolveur)),
        recurrent: vevent.has_property(&ICalendarProperty::Rrule),
        notre_participation,
        repondant,
        participation_du_repondant,
    })
}

/// Construit le texte iCalendar d'une réponse (`METHOD:REPLY`),
/// CRLF, lignes pliées à 75 octets (le writer de calcard plie
/// nativement — prouvé au spike).
pub fn reponse_itip(demande: &DemandeReponse<'_>) -> String {
    let mut vcal = ICalendarComponent::new(ICalendarComponentType::VCalendar);
    vcal.add_property(ICalendarProperty::Version, "2.0");
    vcal.add_property(ICalendarProperty::Prodid, "-//Wind//mail-ical//FR");
    vcal.add_property(
        ICalendarProperty::Method,
        ICalendarValue::Method(ICalendarMethod::Reply),
    );

    let mut vevent = ICalendarComponent::new(ICalendarComponentType::VEvent);
    vevent.add_uid(demande.uid);
    vevent.add_sequence(demande.sequence);
    vevent.add_dtstamp(PartialDateTime::from_utc_timestamp(demande.dtstamp_epoch));
    vevent.add_property(
        ICalendarProperty::Organizer,
        format!("mailto:{}", demande.organisateur_adresse),
    );
    vevent.entries.push(
        ICalendarEntry::new(ICalendarProperty::Attendee)
            .with_param(ICalendarParameter::partstat(
                ICalendarParameterValue::Partstat(partstat_de(demande.participation)),
            ))
            .with_value(format!("mailto:{}", demande.notre_adresse)),
    );

    vcal.component_ids = vec![1];
    ICalendar {
        components: vec![vcal, vevent],
    }
    .to_string()
}

fn methode_du_calendrier(ical: &ICalendar) -> Option<Methode> {
    let vcal = ical
        .components
        .iter()
        .find(|c| c.component_type == ICalendarComponentType::VCalendar)?;
    match vcal.property(&ICalendarProperty::Method)?.values.first()? {
        ICalendarValue::Method(ICalendarMethod::Request) => Some(Methode::Requete),
        ICalendarValue::Method(ICalendarMethod::Cancel) => Some(Methode::Annulation),
        ICalendarValue::Method(ICalendarMethod::Reply) => Some(Methode::Reponse),
        _ => None,
    }
}

fn texte_de(comp: &ICalendarComponent, prop: &ICalendarProperty) -> Option<String> {
    comp.property(prop)
        .and_then(|e| e.values.first())
        .and_then(|v| v.as_text())
        .map(str::to_string)
}

fn personne_de_entree(entree: &ICalendarEntry) -> Option<Personne> {
    Some(Personne {
        adresse: entree.calendar_address()?.to_string(),
        nom: entree
            .parameter(&ICalendarParameterName::Cn)
            .and_then(|v| v.as_text())
            .map(str::to_string),
    })
}

fn participation_de_entree(entree: &ICalendarEntry) -> Participation {
    match entree.parameter(&ICalendarParameterName::Partstat) {
        Some(ICalendarParameterValue::Partstat(p)) => match p {
            ICalendarParticipationStatus::Accepted => Participation::Accepte,
            ICalendarParticipationStatus::Tentative => Participation::Provisoire,
            ICalendarParticipationStatus::Declined => Participation::Refuse,
            _ => Participation::SansReponse,
        },
        // PARTSTAT absent : NEEDS-ACTION est le défaut de la RFC.
        _ => Participation::SansReponse,
    }
}

fn partstat_de(participation: Participation) -> ICalendarParticipationStatus {
    match participation {
        Participation::Accepte => ICalendarParticipationStatus::Accepted,
        Participation::Provisoire => ICalendarParticipationStatus::Tentative,
        Participation::Refuse => ICalendarParticipationStatus::Declined,
        Participation::SansReponse => ICalendarParticipationStatus::NeedsAction,
    }
}

fn est_date_seule(entree: &ICalendarEntry) -> bool {
    entree
        .parameter(&ICalendarParameterName::Value)
        .is_some_and(|v| matches!(v, ICalendarParameterValue::Value(ICalendarValueType::Date)))
}

fn quand_de_entree(entree: &ICalendarEntry, resolveur: &TzResolver<&str>) -> Option<Quand> {
    let pdt = entree.values.first()?.as_partial_date_time()?;
    if est_date_seule(entree) {
        return Some(Quand::Jour(format!(
            "{:04}-{:02}-{:02}",
            pdt.year?, pdt.month?, pdt.day?
        )));
    }
    // L'offset porté par la valeur elle-même (suffixe Z) prime sur tout
    // fuseau : calcard l'applique quel que soit le tz passé.
    if pdt.tz_hour.is_some() {
        let dt = pdt.to_date_time_with_tz(Tz::Floating)?;
        return Some(Quand::Instant(dt.with_timezone(&Utc).timestamp()));
    }
    match entree.tz_id() {
        Some(tzid) => match resolveur.resolve(tzid) {
            // `.single()` peut rendre None dans le trou d'un changement
            // d'heure — on retombe alors en flottant, jamais à faux.
            Some(tz) => match pdt.to_date_time_with_tz(tz) {
                Some(dt) => Some(Quand::Instant(dt.with_timezone(&Utc).timestamp())),
                None => Some(flottant(pdt)),
            },
            // TZID hors tables : la garde D1 — heure dite, pas convertie.
            None => Some(flottant(pdt)),
        },
        // Ni offset ni TZID : heure flottante au sens de la RFC.
        None => Some(flottant(pdt)),
    }
}

fn flottant(pdt: &PartialDateTime) -> Quand {
    Quand::Flottant(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}",
        pdt.year.unwrap_or(0),
        pdt.month.unwrap_or(0),
        pdt.day.unwrap_or(0),
        pdt.hour.unwrap_or(0),
        pdt.minute.unwrap_or(0)
    ))
}
