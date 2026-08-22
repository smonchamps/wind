//! Le corpus des spikes de PLAN-INVITATIONS, versé en tests : six
//! invitations réalistes (Google/IANA, Outlook/TZID Windows, UTC nu,
//! journée entière, CANCEL, récurrence) + l'épreuve de génération du
//! METHOD:REPLY. La vérité de référence est celle d'ATTENDU.md des
//! spikes (2026-08-22), rejouée ici à chaque gate.

use mail_ical::{
    DemandeReponse, ErreurIcal, Invitation, Methode, Participation, Quand, analyser, reponse_itip,
};

const NOUS: &str = "nous@wind.example";

fn charge(nom: &str) -> String {
    let brut = match nom {
        "google" => include_str!("fixtures/google-request.ics"),
        "outlook" => include_str!("fixtures/outlook-request.ics"),
        "utc" => include_str!("fixtures/utc-request.ics"),
        "allday" => include_str!("fixtures/allday-request.ics"),
        "cancel" => include_str!("fixtures/cancel.ics"),
        "recurrence" => include_str!("fixtures/recurrence-request.ics"),
        autre => panic!("fixture inconnue : {autre}"),
    };
    // Les fixtures du dépôt sont en LF ; le fil réel est CRLF.
    brut.replace("\r\n", "\n").replace('\n', "\r\n")
}

fn epoch(s: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(s)
        .unwrap_or_else(|e| panic!("date de référence invalide {s} : {e}"))
        .timestamp()
}

#[test]
fn google_request_tzid_iana_ete() {
    let inv = analyser(&charge("google"), NOUS).unwrap();
    assert_eq!(inv.methode, Methode::Requete);
    assert_eq!(inv.uid, "7f3e9a2b1c4d5e6f@google.com");
    assert_eq!(inv.sequence, 0);
    assert_eq!(inv.titre, "Revue budgétaire T3 — comité de pilotage");
    // Virgule DÉSÉCHAPPÉE de LOCATION.
    assert_eq!(inv.lieu.as_deref(), Some("Salle Vosges, 3e étage"));
    let org = inv.organisateur.expect("organisateur");
    assert_eq!(org.adresse, "claire.martin@exemple.fr");
    assert_eq!(org.nom.as_deref(), Some("Claire Martin"));
    // Europe/Paris en été : 14:30 local = 12:30Z.
    assert_eq!(
        inv.debut,
        Some(Quand::Instant(epoch("2026-09-03T12:30:00Z")))
    );
    assert_eq!(inv.fin, Some(Quand::Instant(epoch("2026-09-03T13:30:00Z"))));
    assert!(!inv.recurrent);
    // Ligne ATTENDEE PLIÉE en plein paramètre (RSVP=\r\n TRUE).
    assert_eq!(inv.notre_participation, Some(Participation::SansReponse));
}

#[test]
fn outlook_request_tzid_windows_hiver() {
    let inv = analyser(&charge("outlook"), NOUS).unwrap();
    assert_eq!(inv.methode, Methode::Requete);
    assert_eq!(
        inv.uid,
        "040000008200E00074C5B7101A82E00800000000B0C3D4E5F6A7B8C9"
    );
    // Param LANGUAGE ignoré sans casser la valeur.
    assert_eq!(inv.titre, "Entretien annuel");
    assert_eq!(inv.lieu.as_deref(), Some("Bureau 204"));
    let org = inv.organisateur.expect("organisateur");
    assert_eq!(org.adresse, "paul.durand@contoso.com");
    // « Romance Standard Time » (TZID Windows) en hiver : 09:00 = 08:00Z.
    assert_eq!(
        inv.debut,
        Some(Quand::Instant(epoch("2026-12-10T08:00:00Z")))
    );
    assert_eq!(inv.fin, Some(Quand::Instant(epoch("2026-12-10T08:30:00Z"))));
    assert_eq!(inv.notre_participation, Some(Participation::SansReponse));
}

#[test]
fn utc_request_sans_tzid() {
    let inv = analyser(&charge("utc"), NOUS).unwrap();
    assert_eq!(inv.titre, "Appel fournisseur");
    assert_eq!(inv.lieu, None);
    assert_eq!(
        inv.debut,
        Some(Quand::Instant(epoch("2026-09-03T12:30:00Z")))
    );
    assert_eq!(inv.fin, Some(Quand::Instant(epoch("2026-09-03T13:00:00Z"))));
}

#[test]
fn journee_entiere_value_date() {
    let inv = analyser(&charge("allday"), NOUS).unwrap();
    assert_eq!(inv.titre, "Séminaire d'équipe");
    assert_eq!(inv.debut, Some(Quand::Jour("2026-09-07".into())));
    // DTEND exclusif, restitué tel quel — l'UI décide de l'affichage.
    assert_eq!(inv.fin, Some(Quand::Jour("2026-09-08".into())));
}

#[test]
fn cancel_meme_uid_sequence_1() {
    let inv = analyser(&charge("cancel"), NOUS).unwrap();
    assert_eq!(inv.methode, Methode::Annulation);
    // Même UID que google-request : c'est la même réunion.
    assert_eq!(inv.uid, "7f3e9a2b1c4d5e6f@google.com");
    assert_eq!(inv.sequence, 1);
}

#[test]
fn recurrence_presence_du_rrule() {
    let inv = analyser(&charge("recurrence"), NOUS).unwrap();
    assert!(inv.recurrent);
    // Première occurrence : mardi 8 sept. 10:00 Paris = 08:00Z.
    assert_eq!(
        inv.debut,
        Some(Quand::Instant(epoch("2026-09-08T08:00:00Z")))
    );
}

#[test]
fn tzid_inconnu_rend_flottant_jamais_une_heure_fausse() {
    // Le piège mesuré au spike : un TZID hors tables retomberait en
    // « flottant traité comme UTC » = décalage d'une heure SILENCIEUX.
    // La garde D1 : on rend Quand::Flottant, l'UI dit « heure locale de
    // l'organisateur ».
    let ics = charge("outlook").replace("Romance Standard Time", "Zone Perso Wind");
    let inv = analyser(&ics, NOUS).unwrap();
    assert_eq!(inv.debut, Some(Quand::Flottant("2026-12-10T09:00".into())));
    assert_eq!(inv.fin, Some(Quand::Flottant("2026-12-10T09:30".into())));
}

#[test]
fn notre_adresse_se_compare_sans_la_casse() {
    let ics = charge("utc").replace("mailto:nous@wind.example", "mailto:NOUS@Wind.Example");
    let inv = analyser(&ics, NOUS).unwrap();
    assert_eq!(inv.notre_participation, Some(Participation::SansReponse));
}

#[test]
fn reply_recu_donne_le_repondant() {
    // Nous sommes l'organisateur : un REPLY arrive avec le PARTSTAT du
    // répondant (D2 — l'état « X a accepté »).
    let demande = DemandeReponse {
        uid: "7f3e9a2b1c4d5e6f@google.com",
        sequence: 0,
        organisateur_adresse: NOUS,
        notre_adresse: "claire.martin@exemple.fr",
        participation: Participation::Accepte,
        dtstamp_epoch: epoch("2026-08-22T12:00:00Z"),
    };
    let reply = reponse_itip(&demande);
    let inv = analyser(&reply, NOUS).unwrap();
    assert_eq!(inv.methode, Methode::Reponse);
    let repondant = inv.repondant.expect("répondant");
    assert_eq!(repondant.adresse, "claire.martin@exemple.fr");
    assert_eq!(inv.participation_du_repondant, Some(Participation::Accepte));
}

#[test]
fn le_repondant_d_un_reply_n_est_pas_l_organisateur_echo() {
    // Exchange ÉCHO parfois l'organisateur en tête de la liste ATTENDEE
    // d'un REPLY (revue) : le répondant est le premier ATTENDEE qui
    // n'est PAS lui — jamais « nous avons répondu » à notre place.
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REPLY\r\n\
        BEGIN:VEVENT\r\nUID:r1@exemple.fr\r\n\
        ORGANIZER;CN=Nous:mailto:nous@wind.example\r\n\
        ATTENDEE;PARTSTAT=NEEDS-ACTION:mailto:nous@wind.example\r\n\
        ATTENDEE;PARTSTAT=ACCEPTED;CN=Paul Durand:mailto:paul@contoso.com\r\n\
        END:VEVENT\r\nEND:VCALENDAR\r\n";
    let inv = analyser(ics, NOUS).unwrap();
    let repondant = inv.repondant.expect("répondant");
    assert_eq!(repondant.adresse, "paul@contoso.com");
    assert_eq!(inv.participation_du_repondant, Some(Participation::Accepte));
}

#[test]
fn reply_genere_conforme_et_reparsable() {
    let inv = analyser(&charge("google"), NOUS).unwrap();
    let org = inv.organisateur.expect("organisateur");
    let demande = DemandeReponse {
        uid: &inv.uid,
        sequence: inv.sequence,
        organisateur_adresse: &org.adresse,
        notre_adresse: NOUS,
        participation: Participation::Refuse,
        dtstamp_epoch: epoch("2026-08-22T12:00:00Z"),
    };
    let reply = reponse_itip(&demande);
    // Forme du fil : CRLF uniquement, lignes ≤ 75 octets.
    assert!(
        !reply.replace("\r\n", "").contains('\n'),
        "LF nu dans la sortie"
    );
    let plus_longue = reply.split("\r\n").map(str::len).max().unwrap_or(0);
    assert!(plus_longue <= 75, "ligne de {plus_longue} octets");
    assert!(reply.contains("METHOD:REPLY"));
    // Re-parse : l'identité de la réunion et notre statut survivent.
    let relu = analyser(&reply, org.adresse.as_str()).unwrap();
    assert_eq!(relu.methode, Methode::Reponse);
    assert_eq!(relu.uid, "7f3e9a2b1c4d5e6f@google.com");
    assert_eq!(relu.sequence, 0);
    let repondant = relu.repondant.expect("répondant");
    assert_eq!(repondant.adresse, NOUS);
    assert_eq!(relu.participation_du_repondant, Some(Participation::Refuse));
}

#[test]
fn texte_illisible_dit_illisible() {
    assert_eq!(
        analyser("pas un calendrier", NOUS),
        Err(ErreurIcal::Illisible)
    );
}

#[test]
fn calendrier_sans_vevent_dit_sans_evenement() {
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REQUEST\r\nEND:VCALENDAR\r\n";
    assert_eq!(analyser(ics, NOUS), Err(ErreurIcal::SansEvenement));
}

#[test]
fn methode_absente_dit_methode_inconnue() {
    // Un .ics de pur export (sans METHOD) n'est pas une invitation.
    let ics = charge("utc").replace("METHOD:REQUEST\r\n", "");
    let _ = ics;
    assert_eq!(
        analyser(&ics, NOUS),
        Err(ErreurIcal::MethodeInconnue),
        "un calendrier sans METHOD n'est pas un message iTIP"
    );
}

#[test]
fn le_lf_nu_est_tolere() {
    // Certains producteurs livrent du LF nu ; l'extraction est identique.
    let brut = include_str!("fixtures/google-request.ics").replace("\r\n", "\n");
    let via_lf = analyser(&brut, NOUS).unwrap();
    let via_crlf = analyser(&charge("google"), NOUS).unwrap();
    assert_eq!(via_lf, via_crlf);
}

#[test]
fn invitation_est_clonable_et_comparable() {
    let inv: Invitation = analyser(&charge("google"), NOUS).unwrap();
    assert_eq!(inv.clone(), inv);
}
