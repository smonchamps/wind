//! L'invitation de réunion d'un message — la ligne `invitations` du cache.
//!
//! Le pont entre `mail-ical` (le parseur pur) et le stockage : une partie
//! `text/calendar` devient une `InvitationRow` prête à écrire, au moment
//! où le MIME passe sous les yeux du moteur (`save_body_full`), ou à
//! l'ouverture pour un message d'avant la fonctionnalité (adoption,
//! invariant §6.7 — re-fetch à la demande puis write-back, jamais de
//! migration de masse).
//!
//! Les valeurs stockées sont des chaînes STABLES (`request`, `accepte`…),
//! découplées des enums de `mail-ical` : la base survit aux refontes du
//! parseur.

use mail_ical::{Invitation, Method, Participation, When};

/// La ligne `invitations` : l'invitation d'UN message, prête à afficher.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InvitationRow {
    /// `request` | `cancel` | `reply`.
    pub methode: String,
    /// L'UID iCalendar de la réunion (partagé par REQUEST/CANCEL/REPLY).
    pub event_uid: String,
    pub sequence: i64,
    pub titre: String,
    pub lieu: Option<String>,
    pub organisateur_adresse: Option<String>,
    pub organisateur_nom: Option<String>,
    /// Début résolu en UTC — `None` si journée entière ou TZID irrésolu.
    pub debut_epoch: Option<i64>,
    pub fin_epoch: Option<i64>,
    /// La forme TEXTE quand l'epoch manque : `AAAA-MM-JJ` (journée
    /// entière) ou `AAAA-MM-JJTHH:MM` (heure flottante, affichée telle
    /// quelle — garde D1, jamais une conversion mensongère).
    pub debut_texte: Option<String>,
    pub fin_texte: Option<String>,
    pub journee_entiere: bool,
    pub recurrent: bool,
    /// NOTRE statut lu du REQUEST : `sans_reponse` | `accepte` |
    /// `provisoire` | `refuse`. `None` : nous ne sommes pas invités.
    pub partstat: Option<String>,
    /// Le répondant d'un REPLY reçu (nous sommes l'organisateur).
    pub repondant_adresse: Option<String>,
    pub repondant_nom: Option<String>,
    pub repondant_statut: Option<String>,
    /// La réunion est annulée (terrain R6) : vrai sur un CANCEL, et
    /// CROISÉ sur le REQUEST de la même réunion (même `event_uid`,
    /// même compte) — posé par le stockage à l'écriture, quel que soit
    /// l'ordre d'arrivée des scans. Une invitation annulée n'offre plus
    /// de réponse.
    pub annule: bool,
}

/// Une invitation stockée, relue avec NOTRE réponse locale (D6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitationStockee {
    pub row: InvitationRow,
    /// `accepte` | `provisoire` | `refuse` — la dernière réponse partie
    /// par la boîte d'envoi. `None` : pas encore répondu depuis Wind.
    pub reponse: Option<String>,
    pub reponse_epoch: Option<i64>,
}

/// Tire la ligne d'invitation d'une partie `text/calendar`.
///
/// `None` si le texte n'est pas un message iTIP lisible — un `.ics`
/// d'export sans METHOD n'est PAS une invitation, il reste une simple
/// pièce jointe.
pub fn extraire_invitation(ics: &str, notre_adresse: &str) -> Option<InvitationRow> {
    let invitation = mail_ical::parse(ics, notre_adresse).ok()?;
    Some(row_de(invitation))
}

fn row_de(invitation: Invitation) -> InvitationRow {
    let (debut_epoch, debut_texte, journee_entiere) = decompose(invitation.start);
    let (fin_epoch, fin_texte, _) = decompose(invitation.end);
    InvitationRow {
        methode: methode_stable(invitation.method).to_string(),
        event_uid: invitation.uid,
        sequence: invitation.sequence,
        titre: invitation.title,
        lieu: invitation.location,
        organisateur_adresse: invitation.organizer.as_ref().map(|o| o.address.clone()),
        organisateur_nom: invitation.organizer.and_then(|o| o.name),
        debut_epoch,
        fin_epoch,
        debut_texte,
        fin_texte,
        journee_entiere,
        recurrent: invitation.recurrent,
        partstat: invitation
            .our_participation
            .map(|p| statut_stable(p).to_string()),
        repondant_adresse: invitation.attendee.as_ref().map(|r| r.address.clone()),
        repondant_nom: invitation.attendee.and_then(|r| r.name),
        repondant_statut: invitation
            .attendee_participation
            .map(|p| statut_stable(p).to_string()),
        // Un CANCEL est annulé par nature ; le croisement vers le
        // REQUEST de la même réunion appartient au stockage.
        annule: matches!(invitation.method, Method::Cancel),
    }
}

fn decompose(quand: Option<When>) -> (Option<i64>, Option<String>, bool) {
    match quand {
        Some(When::Instant(epoch)) => (Some(epoch), None, false),
        Some(When::Day(jour)) => (None, Some(jour), true),
        Some(When::Floating(texte)) => (None, Some(texte), false),
        None => (None, None, false),
    }
}

fn methode_stable(methode: Method) -> &'static str {
    match methode {
        Method::Request => "request",
        Method::Cancel => "cancel",
        Method::Reply => "reply",
    }
}

fn statut_stable(participation: Participation) -> &'static str {
    match participation {
        Participation::NeedsAction => "sans_reponse",
        Participation::Accepted => "accepte",
        Participation::Tentative => "provisoire",
        Participation::Declined => "refuse",
    }
}

/// La `Participation` d'une chaîne stable de la base — pour construire la
/// réponse iTIP depuis la ligne stockée.
pub fn participation_de_stable(stable: &str) -> Option<Participation> {
    match stable {
        "accepte" => Some(Participation::Accepted),
        "provisoire" => Some(Participation::Tentative),
        "refuse" => Some(Participation::Declined),
        "sans_reponse" => Some(Participation::NeedsAction),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUEST: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REQUEST\r\n\
        BEGIN:VEVENT\r\nUID:reunion-1@exemple.fr\r\nSEQUENCE:2\r\n\
        SUMMARY:Point projet\r\nLOCATION:Salle A\r\n\
        DTSTART:20260903T123000Z\r\nDTEND:20260903T130000Z\r\n\
        ORGANIZER;CN=Claire Martin:mailto:claire@exemple.fr\r\n\
        ATTENDEE;PARTSTAT=NEEDS-ACTION:mailto:nous@wind.example\r\n\
        END:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn un_request_devient_une_ligne_complete() {
        let row = extraire_invitation(REQUEST, "nous@wind.example").expect("invitation");
        assert_eq!(row.methode, "request");
        assert_eq!(row.event_uid, "reunion-1@exemple.fr");
        assert_eq!(row.sequence, 2);
        assert_eq!(row.titre, "Point projet");
        assert_eq!(row.lieu.as_deref(), Some("Salle A"));
        assert_eq!(
            row.organisateur_adresse.as_deref(),
            Some("claire@exemple.fr")
        );
        assert_eq!(row.organisateur_nom.as_deref(), Some("Claire Martin"));
        assert!(row.debut_epoch.is_some());
        assert!(row.fin_epoch.is_some());
        assert!(!row.journee_entiere);
        assert_eq!(row.partstat.as_deref(), Some("sans_reponse"));
    }

    #[test]
    fn une_journee_entiere_porte_le_texte_pas_l_epoch() {
        let ics = REQUEST
            .replace("DTSTART:20260903T123000Z", "DTSTART;VALUE=DATE:20260907")
            .replace("DTEND:20260903T130000Z", "DTEND;VALUE=DATE:20260908");
        let row = extraire_invitation(&ics, "nous@wind.example").expect("invitation");
        assert_eq!(row.debut_epoch, None);
        assert_eq!(row.debut_texte.as_deref(), Some("2026-09-07"));
        assert!(row.journee_entiere);
    }

    #[test]
    fn un_export_sans_method_n_est_pas_une_invitation() {
        let ics = REQUEST.replace("METHOD:REQUEST\r\n", "");
        assert_eq!(extraire_invitation(&ics, "nous@wind.example"), None);
    }

    #[test]
    fn un_reply_recu_porte_le_repondant() {
        let ics = REQUEST.replace("METHOD:REQUEST", "METHOD:REPLY").replace(
            "ATTENDEE;PARTSTAT=NEEDS-ACTION:mailto:nous@wind.example",
            "ATTENDEE;PARTSTAT=ACCEPTED;CN=Paul Durand:mailto:paul@contoso.com",
        );
        let row = extraire_invitation(&ics, "nous@wind.example").expect("invitation");
        assert_eq!(row.methode, "reply");
        assert_eq!(row.repondant_adresse.as_deref(), Some("paul@contoso.com"));
        assert_eq!(row.repondant_nom.as_deref(), Some("Paul Durand"));
        assert_eq!(row.repondant_statut.as_deref(), Some("accepte"));
        // Nous ne sommes pas dans la liste : pas de partstat à nous.
        assert_eq!(row.partstat, None);
    }

    #[test]
    fn les_chaines_stables_font_l_aller_retour() {
        for (stable, participation) in [
            ("accepte", Participation::Accepted),
            ("provisoire", Participation::Tentative),
            ("refuse", Participation::Declined),
            ("sans_reponse", Participation::NeedsAction),
        ] {
            assert_eq!(participation_de_stable(stable), Some(participation));
        }
        assert_eq!(participation_de_stable("autre"), None);
    }
}
