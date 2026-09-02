//! Composition d'un message sortant : la frontière de validation.
//!
//! Tout ce que NOUS produisons est strict (à l'inverse de l'affichage,
//! qui tolère le monde réel) : adresses validées une à une, sujet ramené
//! sur une seule ligne (aucune injection d'en-têtes possible), Message-ID
//! généré par nous AVANT l'envoi — c'est lui qui rend un envoi interrompu
//! corrélable au message réellement parti (règle « jamais de fantôme »).

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::address::EmailAddress;
use crate::error::Error;

/// Un message prêt à entrer dans la boîte d'envoi : tout y est validé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// Message-ID RFC 5322, chevrons compris, généré par nous.
    pub message_id: String,
    pub from: String,
    /// Destinataires validés — jamais vide.
    pub to: Vec<String>,
    /// Copie carbone, validée — peut être vide.
    pub cc: Vec<String>,
    /// Copie carbone INVISIBLE, validée — peut être vide. Ne paraît
    /// JAMAIS dans les en-têtes du message servi aux autres (l'envoi la
    /// porte dans l'enveloppe SMTP seule, mail-smtp) : c'est tout son sens.
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_text: String,
    /// Corps riche (PLAN-COMPOSITION-HTML), déjà assaini par l'appelant
    /// (la frontière ammonia vit côté app, seule à dépendre de
    /// mail-render). `None` = envoi texte seul, chemin historique.
    /// `compose()` rend `None` ; l'appelant qui a du HTML le pose —
    /// le texte reste TOUJOURS peuplé, c'est lui le repli.
    pub body_html: Option<String>,
    /// Message-ID du message auquel on répond (fil de discussion).
    pub in_reply_to: Option<String>,
    /// La chaîne `References` (RFC 5322 §3.6.4) : celles du parent + son
    /// Message-ID — lue en base par `Store::references_de` (E7). `None`
    /// = le parent seul (`in_reply_to`), chemin d'avant.
    pub references: Option<String>,
    /// Marqué « important » par l'expéditeur (PLAN-RETOURS-6, R3) :
    /// l'envoi portera les en-têtes `X-Priority: 1` + `Importance:
    /// high` — la paire que posent les clients mûrs. `compose()` rend
    /// `false` ; l'appelant qui a le geste le pose.
    pub important: bool,
    /// La réponse iTIP d'une invitation (PLAN-INVITATIONS) : le texte
    /// `METHOD:REPLY` que la remise portera en partie
    /// `text/calendar; method=REPLY`. `compose()` rend `None` ;
    /// l'appelant qui répond à une invitation le pose.
    pub ics_reply: Option<String>,
}

/// Valide et assemble un brouillon prêt à journaliser.
///
/// `to_raw`/`cc_raw`/`bcc_raw` acceptent plusieurs adresses séparées par
/// des virgules ou des points-virgules ; chacune doit être valide, sinon
/// tout est refusé (fail fast à la frontière). `to_raw` ne peut pas être
/// vide ; Cc et Cci le peuvent. `in_reply_to` est le Message-ID du
/// message d'origine tel que rapporté par le serveur — normalisé ici.
pub fn compose(
    from: &str,
    to_raw: &str,
    cc_raw: &str,
    bcc_raw: &str,
    subject: &str,
    body_text: &str,
    in_reply_to: Option<&str>,
) -> Result<Draft, Error> {
    let from = EmailAddress::parse(from)?;
    let to = parse_recipients(to_raw)?;
    if to.is_empty() {
        return Err(Error::InvalidEmailAddress(to_raw.to_string()));
    }
    let cc = parse_recipients(cc_raw)?;
    let bcc = parse_recipients(bcc_raw)?;
    Ok(Draft {
        message_id: generate_message_id(&from),
        from: from.to_string(),
        to,
        cc,
        bcc,
        subject: single_line(subject),
        body_text: body_text.to_string(),
        body_html: None,
        in_reply_to: in_reply_to.and_then(normalize_message_id),
        references: None,
        important: false,
        ics_reply: None,
    })
}

/// Valide une liste d'adresses (virgules ou points-virgules) : chacune
/// stricte, une seule invalide refuse tout. Rend une liste vide sur une
/// entrée vide — l'appelant décide si le vide est permis (À : non ; Cc,
/// Cci : oui).
fn parse_recipients(raw: &str) -> Result<Vec<String>, Error> {
    raw.split([',', ';'])
        .filter(|part| !part.trim().is_empty())
        .map(|part| EmailAddress::parse(part).map(|address| address.to_string()))
        .collect()
}

/// Sujet pré-rempli d'une réponse : « Re: » sans empilement — jamais de
/// « Re: Re: », y compris face au « RE : » à la française d'Outlook.
pub fn reply_subject(original: Option<&str>) -> String {
    match original
        .map(str::trim)
        .filter(|subject| !subject.is_empty())
    {
        Some(subject) => {
            let lower = subject.to_lowercase();
            if lower.starts_with("re:") || lower.starts_with("re :") {
                subject.to_string()
            } else {
                format!("Re: {subject}")
            }
        }
        None => "Re:".to_string(),
    }
}

/// Sujet pré-rempli d'un transfert : « Fwd: » sans empilement, tolérant
/// aux variantes du terrain (« Tr : » d'Outlook français, « Fw: »…).
pub fn forward_subject(original: Option<&str>) -> String {
    match original
        .map(str::trim)
        .filter(|subject| !subject.is_empty())
    {
        Some(subject) => {
            let lower = subject.to_lowercase();
            let already = ["fwd:", "fwd :", "fw:", "fw :", "tr:", "tr :"]
                .iter()
                .any(|prefix| lower.starts_with(prefix));
            if already {
                subject.to_string()
            } else {
                format!("Fwd: {subject}")
            }
        }
        None => "Fwd:".to_string(),
    }
}

/// Destinataires d'un « Répondre à tous », À et Cc SÉPARÉS (verdict CE
/// D3, PLAN-RETOURS-2) : le champ À reçoit l'expéditeur puis les À
/// d'origine ; le champ Cc reçoit les Cc d'origine — les Cc restent des
/// Cc au lieu d'être aplatis dans le À. Sans doublon (comparaison
/// insensible à la casse), sans sa propre adresse (s'écrire à soi-même
/// serait du bruit), et sans remettre en Cc quelqu'un déjà placé en À.
/// Chaque liste peut être VIDE : un message qu'on s'est envoyé à soi seul
/// n'a personne d'autre — l'appelant tranche.
pub fn reply_all_split(
    sender: Option<&str>,
    to: &[String],
    cc: &[String],
    own_address: &str,
) -> (Vec<String>, Vec<String>) {
    let own = own_address.trim().to_lowercase();
    let mut vus: Vec<String> = Vec::new();
    let mut ajouter = |dest: &mut Vec<String>, candidat: &str| {
        let adresse = candidat.trim();
        if adresse.is_empty() {
            return;
        }
        let cle = adresse.to_lowercase();
        if cle == own || vus.contains(&cle) {
            return;
        }
        vus.push(cle);
        dest.push(adresse.to_string());
    };
    let mut to_out = Vec::new();
    for candidat in sender.into_iter().chain(to.iter().map(String::as_str)) {
        ajouter(&mut to_out, candidat);
    }
    let mut cc_out = Vec::new();
    for candidat in cc {
        ajouter(&mut cc_out, candidat);
    }
    (to_out, cc_out)
}

/// À qui adresse une réponse SIMPLE (« Répondre ») — R4, PLAN-RETOURS-3,
/// constat terrain du 2026-08-18.
///
/// Sur un message REÇU : l'expéditeur, comme toujours. Sur NOTRE PROPRE
/// message (l'expéditeur est le compte), répondre à l'expéditeur nous
/// écrirait à nous-mêmes ; on vise donc les destinataires d'ORIGINE (le
/// À du message), c'est-à-dire réécrire au même groupe. La liste peut être
/// VIDE — message reçu sans expéditeur connu, ou envoi ancien dont les
/// destinataires ne sont pas encore en base : l'appelant tranche le repli
/// (relève serveur, ou échec franc).
pub fn reply_to(
    is_own: bool,
    sender: Option<&str>,
    to_addrs: &[String],
    reply_to: Option<&str>,
) -> Vec<String> {
    // `Reply-To` dit où l'expéditeur veut la réponse (listes,
    // notifications) — il prime sur `From` (PLAN-AUDIT-V2 E5), sauf sur
    // son propre message, où l'on réécrit au même groupe.
    let reply_to = reply_to
        .map(str::trim)
        .filter(|adresse| !adresse.is_empty());
    if !is_own && let Some(adresse) = reply_to {
        return vec![adresse.to_string()];
    }
    if is_own {
        to_addrs
            .iter()
            .map(|adresse| adresse.trim())
            .filter(|adresse| !adresse.is_empty())
            .map(str::to_string)
            .collect()
    } else {
        sender
            .map(str::trim)
            .filter(|adresse| !adresse.is_empty())
            .map(|adresse| vec![adresse.to_string()])
            .unwrap_or_default()
    }
}

/// La ligne d'attribution d'une citation — l'autorité UNIQUE des deux
/// variantes (texte et riche) : un libellé qui bouge bouge partout.
fn attribution(sender: Option<&str>, date: Option<&str>) -> String {
    let sender = sender.unwrap_or("(expéditeur inconnu)");
    match date {
        Some(date) => format!("Le {date}, {sender} a écrit :"),
        None => format!("{sender} a écrit :"),
    }
}

/// L'en-tête d'un transfert (séparateur, De/Date/Objet) — même règle :
/// une seule source pour les variantes texte et riche.
fn entete_transfert(sender: Option<&str>, date: Option<&str>, subject: Option<&str>) -> String {
    let mut entete = String::from("---------- Message transféré ----------\n");
    entete.push_str(&format!(
        "De : {}\n",
        sender.unwrap_or("(expéditeur inconnu)")
    ));
    if let Some(date) = date {
        entete.push_str(&format!("Date : {date}\n"));
    }
    entete.push_str(&format!("Objet : {}", subject.unwrap_or("(sans objet)")));
    entete
}

/// Bloc de citation d'une réponse, à placer SOUS le curseur (top-posting) :
/// une ligne d'attribution puis chaque ligne du texte préfixée de « > ».
pub fn quote_reply(sender: Option<&str>, date: Option<&str>, body_text: &str) -> String {
    if body_text.trim().is_empty() {
        return String::new();
    }
    let quoted: String = body_text
        .lines()
        .map(|line| format!("> {line}\n"))
        .collect();
    format!("\n\n{}\n{}", attribution(sender, date), quoted.trim_end())
}

/// Citation riche d'une réponse (PLAN-COMPOSITION-HTML) : l'attribution
/// ÉCHAPPÉE (l'expéditeur vient du monde réel), puis le corps — déjà
/// assaini par l'appelant — dans un `<blockquote>` au filet gauche,
/// la forme que tous les clients mûrs donnent au texte cité. Le style
/// inline traverse `clean_style` (ni url ni exécution) et l'allowlist.
pub fn quote_reply_html(sender: Option<&str>, date: Option<&str>, body_html: &str) -> String {
    if body_html.trim().is_empty() {
        return String::new();
    }
    format!(
        "<br><br>{}<blockquote style=\"margin:0 0 0 0.8ex;border-left:2px solid #ccc;padding-left:1ex\">{body_html}</blockquote>",
        crate::echo::texte_en_html(&attribution(sender, date)),
    )
}

/// Bloc riche d'un transfert : l'en-tête d'origine (De/Date/Objet)
/// échappé, puis le corps HTML tel quel — un transfert transmet.
/// L'attribut qui marque le bloc transféré dans un corps composé
/// (PLAN-AUDIT-V2 E10, D8). Sa valeur nomme la source
/// (`compte/uid/boîte`) : à l'envoi, [`substituer_transfert`] remplace
/// tout ce qui suit par le rendu AVEC les images distantes — le composeur,
/// lui, n'en a chargé aucune.
pub const MARQUEUR_TRANSFERT: &str = "data-wind-transfert";

pub fn quote_forward_html(
    sender: Option<&str>,
    date: Option<&str>,
    subject: Option<&str>,
    body_html: &str,
    source: Option<&str>,
) -> String {
    let ouverture = match source {
        Some(source) => format!(
            "<div {MARQUEUR_TRANSFERT}=\"{}\">",
            echapper_attribut(source)
        ),
        None => "<div>".to_string(),
    };
    // La ligne vide éditable APRÈS le bloc (terrain STOP 2, 2026-09-02) :
    // dans un contenteditable, le curseur posé après le dernier bloc
    // tombe DEDANS — un mot tapé « après » vivait dans le `<div>` marqué
    // et la substitution à l'envoi l'emportait. Ici, le curseur a un
    // dehors.
    format!(
        "<br><br>{}<br>{ouverture}{body_html}</div><div><br></div>",
        crate::echo::texte_en_html(&entete_transfert(sender, date, subject)),
    )
}

fn echapper_attribut(valeur: &str) -> String {
    valeur
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// D'où vient un bloc transféré : le compte, l'UID et la boîte du
/// message d'origine — ce que le marqueur porte (`compte/uid/boîte`, la
/// boîte en dernier parce qu'elle peut contenir des `/`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceTransfert {
    pub account_id: i64,
    pub uid: crate::Uid,
    pub mailbox: String,
}

impl SourceTransfert {
    /// La valeur du marqueur — l'inverse exact de [`source_du_transfert`].
    pub fn cle(&self) -> String {
        format!("{}/{}/{}", self.account_id, self.uid, self.mailbox)
    }
}

/// La source du bloc transféré d'un corps composé, telle que le
/// marqueur la porte — `None` sans marqueur (réponse, message neuf) ou
/// si la valeur n'a pas la forme attendue. Décision pure : le shell ne
/// fait que relire la source et substituer (revue, STANDARD §4).
pub fn source_du_transfert(body_html: &str) -> Option<SourceTransfert> {
    let cle = format!("{MARQUEUR_TRANSFERT}=\"");
    let debut = body_html.find(&cle)? + cle.len();
    let fin = body_html[debut..].find('"')? + debut;
    let valeur = body_html[debut..fin]
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    let (compte, reste) = valeur.split_once('/')?;
    let (uid, mailbox) = reste.split_once('/')?;
    Some(SourceTransfert {
        account_id: compte.parse().ok()?,
        uid: uid.parse().ok()?,
        mailbox: mailbox.to_string(),
    })
}

/// Remplace le bloc transféré — le `<div>` marqué, jusqu'à SA fermeture
/// (les `<div>` imbriqués du courrier cité sont comptés) — par
/// `corps_frais` (le rendu avec ses images distantes). Ce que
/// l'utilisateur a tapé avant ET après le bloc reste ; une retouche DANS
/// le bloc est perdue (limite dite : on transmet, on ne commente pas
/// ligne à ligne). Un bloc jamais fermé se remplace jusqu'à la fin.
pub fn substituer_transfert(body_html: &str, corps_frais: &str) -> String {
    // Par l'ATTRIBUT, puis la balise qui le porte (revue) : un éditeur
    // qui pose `style` ou `class` avant le marqueur ne le fait pas rater.
    let cle = format!(" {MARQUEUR_TRANSFERT}=");
    let Some(attribut) = body_html.find(&cle) else {
        return body_html.to_string();
    };
    let Some(debut) = body_html[..attribut].rfind('<') else {
        return body_html.to_string();
    };
    let fin = fin_du_bloc(&body_html[debut..]).map_or(body_html.len(), |l| debut + l);
    format!(
        "{}<div>{corps_frais}</div>{}",
        &body_html[..debut],
        &body_html[fin..]
    )
}

/// La longueur du premier élément `<div …>…</div>` de `html`, fermetures
/// imbriquées comprises — `None` s'il n'est jamais fermé. Insensible à
/// la casse (le composeur peut réécrire `<DIV>`).
fn fin_du_bloc(html: &str) -> Option<usize> {
    // Sur les OCTETS, jamais `str[i..]` : l'avance octet par octet
    // tombait au milieu d'un « é » et paniquait (andon de gate,
    // 2026-09-02). Les balises cherchées sont ASCII : l'index rendu
    // est une frontière de caractère.
    let bas = html.to_ascii_lowercase();
    let bas = bas.as_bytes();
    let mut profondeur = 0usize;
    let mut i = 0;
    while i < bas.len() {
        if bas[i..].starts_with(b"</div") {
            profondeur = profondeur.checked_sub(1)?;
            let ferme = bas[i..].iter().position(|&c| c == b'>')? + i + 1;
            if profondeur == 0 {
                return Some(ferme);
            }
            i = ferme;
        } else if bas[i..].starts_with(b"<div") {
            profondeur += 1;
            i += 4;
        } else {
            i += 1;
        }
    }
    None
}

/// Bloc d'un transfert : l'en-tête d'origine (De/Date/Objet) puis le texte
/// tel quel — un transfert transmet, il ne commente pas ligne à ligne.
pub fn quote_forward(
    sender: Option<&str>,
    date: Option<&str>,
    subject: Option<&str>,
    body_text: &str,
) -> String {
    format!(
        "\n\n{}\n\n{}",
        entete_transfert(sender, date, subject),
        body_text.trim_end(),
    )
}

/// Un sujet vit sur une seule ligne : tout caractère de contrôle devient
/// une espace — la voie de l'injection d'en-têtes est coupée à la source.
fn single_line(subject: &str) -> String {
    subject
        .trim()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Un Message-ID s'utilise chevrons compris (RFC 5322) ; certains serveurs
/// les omettent dans leurs réponses ENVELOPE — normaliser ici, une fois.
fn normalize_message_id(id: &str) -> Option<String> {
    let bare = id.trim().trim_matches(['<', '>']);
    if bare.is_empty() {
        None
    } else {
        Some(format!("<{bare}>"))
    }
}

/// Message-ID unique, généré AVANT toute tentative d'envoi.
///
/// `RandomState` est semé aléatoirement par le système à chaque instance :
/// combiné à l'horloge en nanosecondes, l'unicité est assurée sans
/// dépendance supplémentaire.
fn generate_message_id(from: &EmailAddress) -> String {
    let domain = from.as_str().rsplit('@').next().unwrap_or("localhost");
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let random = RandomState::new().build_hasher().finish();
    format!("<{}.{:016x}@{}>", epoch.as_nanos(), random, domain)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compose_simple(to_raw: &str) -> Result<Draft, Error> {
        compose("moi@exemple.fr", to_raw, "", "", "Sujet", "Corps", None)
    }

    #[test]
    fn valide_cc_et_bcc_comme_le_champ_a() {
        let draft = compose(
            "moi@exemple.fr",
            "a@exemple.fr",
            "b@exemple.fr, c@exemple.fr",
            "secret@exemple.fr",
            "Sujet",
            "Corps",
            None,
        )
        .unwrap();
        assert_eq!(draft.to, vec!["a@exemple.fr"]);
        assert_eq!(draft.cc, vec!["b@exemple.fr", "c@exemple.fr"]);
        assert_eq!(draft.bcc, vec!["secret@exemple.fr"]);
    }

    #[test]
    fn cc_et_bcc_vides_valent_liste_vide() {
        let draft = compose_simple("a@exemple.fr").unwrap();
        assert!(draft.cc.is_empty());
        assert!(draft.bcc.is_empty());
    }

    #[test]
    fn une_adresse_cc_ou_bcc_invalide_refuse_tout() {
        assert!(
            compose(
                "moi@exemple.fr",
                "a@exemple.fr",
                "pas-une-adresse",
                "",
                "s",
                "c",
                None
            )
            .is_err()
        );
        assert!(
            compose(
                "moi@exemple.fr",
                "a@exemple.fr",
                "",
                "x\r\ny@z.fr",
                "s",
                "c",
                None
            )
            .is_err()
        );
    }

    #[test]
    fn composes_multiple_recipients_from_comma_or_semicolon_list() {
        let draft = compose_simple(" a@exemple.fr , b@exemple.fr ; c@exemple.fr ").unwrap();
        assert_eq!(
            draft.to,
            vec!["a@exemple.fr", "b@exemple.fr", "c@exemple.fr"]
        );
        assert_eq!(draft.from, "moi@exemple.fr");
    }

    #[test]
    fn rejects_the_whole_draft_if_any_recipient_is_invalid() {
        assert!(compose_simple("a@exemple.fr, pas-une-adresse").is_err());
    }

    #[test]
    fn rejects_empty_recipient_list() {
        assert!(compose_simple("").is_err());
        assert!(compose_simple("  ,  ; ").is_err());
    }

    #[test]
    fn rejects_invalid_sender() {
        assert!(compose("pas-une-adresse", "a@exemple.fr", "", "", "s", "c", None).is_err());
    }

    #[test]
    fn generates_unique_well_formed_message_ids() {
        let first = compose_simple("a@exemple.fr").unwrap();
        let second = compose_simple("a@exemple.fr").unwrap();
        assert_ne!(first.message_id, second.message_id);
        assert!(first.message_id.starts_with('<'));
        assert!(first.message_id.ends_with("@exemple.fr>"));
    }

    /// L'injection d'en-têtes par le sujet est neutralisée à la source.
    #[test]
    fn subject_is_flattened_to_a_single_line() {
        let draft = compose(
            "moi@exemple.fr",
            "a@exemple.fr",
            "",
            "",
            "Alerte\r\nBcc: espion@mal.example",
            "Corps",
            None,
        )
        .unwrap();
        assert!(!draft.subject.contains('\r'));
        assert!(!draft.subject.contains('\n'));
        assert!(draft.subject.contains("Bcc: espion@mal.example"));
    }

    #[test]
    fn body_newlines_are_preserved_verbatim() {
        let draft = compose(
            "moi@exemple.fr",
            "a@exemple.fr",
            "",
            "",
            "s",
            "ligne 1\nligne 2",
            None,
        )
        .unwrap();
        assert_eq!(draft.body_text, "ligne 1\nligne 2");
    }

    #[test]
    fn normalizes_in_reply_to_with_angle_brackets() {
        let with = compose(
            "moi@exemple.fr",
            "a@exemple.fr",
            "",
            "",
            "s",
            "c",
            Some("<id@x.y>"),
        )
        .unwrap();
        assert_eq!(with.in_reply_to.as_deref(), Some("<id@x.y>"));
        let without = compose(
            "moi@exemple.fr",
            "a@exemple.fr",
            "",
            "",
            "s",
            "c",
            Some("id@x.y"),
        )
        .unwrap();
        assert_eq!(without.in_reply_to.as_deref(), Some("<id@x.y>"));
        let blank = compose(
            "moi@exemple.fr",
            "a@exemple.fr",
            "",
            "",
            "s",
            "c",
            Some("  "),
        )
        .unwrap();
        assert_eq!(blank.in_reply_to, None);
    }

    #[test]
    fn reply_subject_prefixes_exactly_once() {
        assert_eq!(reply_subject(Some("Réunion")), "Re: Réunion");
        assert_eq!(reply_subject(Some("Re: Réunion")), "Re: Réunion");
        assert_eq!(reply_subject(Some("RE : Réunion")), "RE : Réunion");
        assert_eq!(reply_subject(Some("  ")), "Re:");
        assert_eq!(reply_subject(None), "Re:");
    }

    #[test]
    fn forward_subject_prefixes_exactly_once() {
        assert_eq!(forward_subject(Some("Réunion")), "Fwd: Réunion");
        assert_eq!(forward_subject(Some("Fwd: Réunion")), "Fwd: Réunion");
        assert_eq!(forward_subject(Some("TR : Réunion")), "TR : Réunion");
        assert_eq!(forward_subject(Some("Fw: Réunion")), "Fw: Réunion");
        assert_eq!(forward_subject(None), "Fwd:");
    }

    /// Un « Re: » n'est pas un « Fwd: » : transférer une réponse préfixe.
    #[test]
    fn forward_subject_still_prefixes_a_reply_subject() {
        assert_eq!(forward_subject(Some("Re: Réunion")), "Fwd: Re: Réunion");
    }

    #[test]
    fn reply_all_split_ordonne_sender_puis_a_puis_cc_sans_soi() {
        let to = vec!["moi@exemple.fr".to_string(), "bob@exemple.fr".to_string()];
        let cc = vec!["carole@exemple.fr".to_string()];
        let (to_out, cc_out) =
            reply_all_split(Some("alice@exemple.fr"), &to, &cc, "moi@exemple.fr");
        assert_eq!(to_out, vec!["alice@exemple.fr", "bob@exemple.fr"]);
        assert_eq!(cc_out, vec!["carole@exemple.fr"]);
    }

    /// D3 : les Cc d'origine restent en Cc ; un Cc déjà placé en À (bob)
    /// n'y est pas remis.
    #[test]
    fn reply_all_split_garde_les_cc_en_cc() {
        let to = vec!["moi@exemple.fr".to_string(), "bob@exemple.fr".to_string()];
        let cc = vec![
            "carole@exemple.fr".to_string(),
            "bob@exemple.fr".to_string(),
        ];
        let (to_out, cc_out) =
            reply_all_split(Some("alice@exemple.fr"), &to, &cc, "moi@exemple.fr");
        assert_eq!(to_out, vec!["alice@exemple.fr", "bob@exemple.fr"]);
        assert_eq!(cc_out, vec!["carole@exemple.fr"]);
    }

    /// La casse ne fait pas deux adresses : « Bob@ » et « bob@ » ne
    /// produisent qu'un destinataire, et « MOI@ » reste soi.
    #[test]
    fn reply_all_split_dedoublonne_insensible_a_la_casse() {
        let to = vec!["Bob@Exemple.fr".to_string(), "MOI@exemple.fr".to_string()];
        let cc = vec!["bob@exemple.fr".to_string(), "alice@exemple.fr".to_string()];
        let (to_out, cc_out) =
            reply_all_split(Some("alice@exemple.fr"), &to, &cc, "moi@exemple.fr");
        assert_eq!(to_out, vec!["alice@exemple.fr", "Bob@Exemple.fr"]);
        assert!(cc_out.is_empty(), "bob et alice sont déjà en À");
    }

    /// Message envoyé à soi seul : personne d'autre — les deux listes sont
    /// vides, c'est à l'appelant de trancher (retomber sur l'expéditeur).
    #[test]
    fn reply_all_split_peut_etre_vide_quand_seul() {
        let to = vec!["moi@exemple.fr".to_string()];
        let (to_out, cc_out) = reply_all_split(Some("moi@exemple.fr"), &to, &[], "moi@exemple.fr");
        assert!(to_out.is_empty());
        assert!(cc_out.is_empty());
    }

    #[test]
    fn reply_all_split_ignore_les_vides_et_l_expediteur_absent() {
        let to = vec!["  ".to_string(), "bob@exemple.fr".to_string()];
        let (to_out, _) = reply_all_split(None, &to, &[], "moi@exemple.fr");
        assert_eq!(to_out, vec!["bob@exemple.fr"]);
    }

    /// Réponse simple à un message REÇU : l'expéditeur, un seul.
    /// PLAN-AUDIT-V2 E5 : « Répondre » partait vers `From` même quand le
    /// message portait un `Reply-To` (listes, notifications) — l'en-tête
    /// était jeté par l'adaptateur. Sur son PROPRE message, le `Reply-To`
    /// ne change rien : on réécrit au même groupe.
    #[test]
    fn repondre_vise_reply_to() {
        assert_eq!(
            reply_to(false, Some("liste@x.fr"), &[], Some("bob@y.fr")),
            vec!["bob@y.fr".to_string()]
        );
        let to = vec!["groupe@x.fr".to_string()];
        assert_eq!(
            reply_to(true, Some("moi@exemple.fr"), &to, Some("bob@y.fr")),
            to
        );
        assert_eq!(
            reply_to(false, Some("liste@x.fr"), &[], Some("  ")),
            vec!["liste@x.fr".to_string()],
            "un Reply-To vide ne vaut rien"
        );
    }

    /// PLAN-AUDIT-V2 E10, décision CE D8 : « Transférer » ne charge
    /// AUCUNE image distante dans le composeur (le pixel de suivi partait
    /// au clic, « Annuler » ne le rattrapait pas) ; à l'envoi, le bloc
    /// transféré est remplacé par le rendu AVEC les vraies URL — le
    /// destinataire reçoit le même message.
    /// Andon de gate (2026-09-02) : « transféré » dans le bloc — un octet
    /// non ASCII faisait PANIQUER `fin_du_bloc` (découpe d'une `str` hors
    /// frontière de caractère) ; la tâche async tombait et `queue_send`
    /// ne répondait jamais : composition figée, ni toast ni erreur. Le
    /// test de la revue n'avait que de l'ASCII.
    #[test]
    fn un_bloc_accentue_se_substitue_sans_paniquer() {
        let corps = format!(
            "<p>Salut é</p><div {MARQUEUR_TRANSFERT}=\"1/17/INBOX\">             <p>Message transféré — été</p><div>à</div></div><p>après</p>"
        );
        assert_eq!(
            substituer_transfert(&corps, "Z"),
            "<p>Salut é</p><div>Z</div><p>après</p>"
        );
    }

    #[test]
    fn un_transfert_n_embarque_aucune_image_distante_a_la_composition_mais_les_rend_a_l_envoi() {
        let bloque = r#"<p>lettre</p><img src="data:image/gif;base64,R0lGOD" alt="">"#;
        let source = SourceTransfert {
            account_id: 3,
            uid: 42,
            mailbox: "INBOX".to_string(),
        };
        let compose = quote_forward_html(
            Some("Alice"),
            Some("2026-09-02"),
            Some("Lettre"),
            bloque,
            Some(&source.cle()),
        );
        assert!(!compose.contains("https://"), "{compose}");
        assert!(
            compose.contains(r#"data-wind-transfert="3/42/INBOX""#),
            "{compose}"
        );
        assert_eq!(source_du_transfert(&compose), Some(source));

        // Tapé AVANT et APRÈS le bloc : les deux restent (revue — la
        // première version tronquait tout après le marqueur).
        let edite = format!("<div>mon mot</div>{compose}<div>et ma conclusion</div>");
        let frais = r#"<p>lettre</p><img src="https://x.example/p.gif" alt="">"#;
        let envoye = substituer_transfert(&edite, frais);
        assert!(envoye.starts_with("<div>mon mot</div><br><br>"), "{envoye}");
        assert!(
            envoye.ends_with("</div><div>et ma conclusion</div>"),
            "{envoye}"
        );
        assert!(envoye.contains("https://x.example/p.gif"), "{envoye}");
        assert!(!envoye.contains("data:image"), "{envoye}");
        assert!(!envoye.contains("data-wind-transfert"), "{envoye}");
        // Un courrier cité plein de <div> imbriqués : la fermeture est la
        // BONNE, pas la première venue.
        let imbrique = quote_forward_html(
            None,
            None,
            None,
            "<div><div>a</div><div>b</div></div>",
            Some("1/1/INBOX"),
        );
        let envoye = substituer_transfert(&format!("{imbrique}<p>fin</p>"), "X");
        // Entre le bloc et « fin » : la ligne vide éditable du transfert
        // (terrain STOP 2) — elle vit HORS du bloc, elle survit.
        assert!(
            envoye.ends_with("<div>X</div><div><br></div><p>fin</p>"),
            "{envoye}"
        );
        // L'éditeur a posé un attribut AVANT le marqueur : trouvé quand même.
        let reserialise = imbrique.replace(
            "<div data-wind-transfert",
            r#"<div style="x" data-wind-transfert"#,
        );
        assert!(substituer_transfert(&reserialise, "Y").contains("<div>Y</div>"));
        // Sans marqueur (une réponse, un message neuf) : rien ne bouge.
        assert_eq!(substituer_transfert("<p>a</p>", frais), "<p>a</p>");
        assert_eq!(source_du_transfert("<p>a</p>"), None);
        // Un nom de boîte avec des guillemets ET des `/` survit à
        // l'aller-retour ; une valeur mal formée ne vaut rien.
        let bizarre =
            quote_forward_html(None, None, None, "", Some(r#"1/2/[Gmail]/Dossier "cité""#));
        assert_eq!(
            source_du_transfert(&bizarre),
            Some(SourceTransfert {
                account_id: 1,
                uid: 2,
                mailbox: r#"[Gmail]/Dossier "cité""#.to_string()
            })
        );
        assert_eq!(
            source_du_transfert(r#"<div data-wind-transfert="x/y/z">"#),
            None
        );
    }

    #[test]
    fn reply_to_recu_vise_l_expediteur() {
        assert_eq!(
            reply_to(
                false,
                Some("alice@exemple.fr"),
                &["bob@exemple.fr".to_string()],
                None
            ),
            vec!["alice@exemple.fr"],
        );
    }

    /// LE constat terrain (R4) : sur NOTRE propre message, répondre vise
    /// les destinataires d'origine — jamais l'expéditeur (nous).
    #[test]
    fn reply_to_propre_vise_les_destinataires_pas_soi() {
        let to = vec!["client@vantis.fr".to_string(), "chef@vantis.fr".to_string()];
        assert_eq!(
            reply_to(true, Some("moi@exemple.fr"), &to, None),
            vec!["client@vantis.fr", "chef@vantis.fr"],
        );
    }

    /// Notre propre message SANS destinataires en base (envoi ancien non
    /// rattrapé) : liste vide — l'appelant relève le serveur.
    #[test]
    fn reply_to_propre_sans_destinataires_est_vide() {
        assert!(reply_to(true, Some("moi@exemple.fr"), &[], None).is_empty());
    }

    /// Message reçu sans expéditeur connu : vide — l'appelant échoue franc.
    #[test]
    fn reply_to_recu_sans_expediteur_est_vide() {
        assert!(reply_to(false, None, &["x@y.fr".to_string()], None).is_empty());
    }

    /// PLAN-COMPOSITION-HTML E2 : la citation riche d'une réponse — une
    /// attribution ÉCHAPPÉE puis le corps assaini dans un blockquote.
    /// L'expéditeur vient du monde réel : un nom à chevrons ne doit
    /// jamais s'injecter dans notre HTML.
    #[test]
    fn quote_reply_html_attributes_then_blockquotes() {
        let quote = quote_reply_html(
            Some("Alice <alice@ex.fr>"),
            Some("2026-08-19 10:23"),
            "<p>première</p>",
        );
        assert!(
            quote.contains("Le 2026-08-19 10:23, Alice &lt;alice@ex.fr&gt; a écrit :"),
            "{quote}"
        );
        assert!(
            quote.contains("<blockquote"),
            "le corps cité vit dans un blockquote : {quote}"
        );
        assert!(
            quote.contains("<p>première</p></blockquote>"),
            "le HTML d'origine est cité tel quel : {quote}"
        );
        let attribution = quote.find("a écrit :").unwrap();
        let bloc = quote.find("<blockquote").unwrap();
        assert!(
            attribution < bloc,
            "l'attribution précède le bloc : {quote}"
        );
    }

    #[test]
    fn quote_reply_html_degrades_gracefully_without_metadata() {
        let quote = quote_reply_html(None, None, "<p>texte</p>");
        assert!(quote.contains("(expéditeur inconnu) a écrit :"), "{quote}");
    }

    #[test]
    fn quote_reply_html_of_empty_body_is_empty() {
        assert_eq!(quote_reply_html(Some("Alice"), None, "  \n "), "");
    }

    /// Le bloc riche d'un transfert : l'en-tête d'origine (De/Date/Objet)
    /// échappé, puis le corps HTML tel quel — un transfert transmet.
    #[test]
    fn quote_forward_html_carries_headers_and_body() {
        let block = quote_forward_html(
            Some("Alice <alice@ex.fr>"),
            Some("2026-08-19 10:23"),
            Some("Devis <urgent>"),
            "<p>le corps</p>",
            None,
        );
        assert!(
            block.contains("---------- Message transféré ----------"),
            "{block}"
        );
        assert!(block.contains("De : Alice &lt;alice@ex.fr&gt;"), "{block}");
        assert!(block.contains("Date : 2026-08-19 10:23"), "{block}");
        assert!(block.contains("Objet : Devis &lt;urgent&gt;"), "{block}");
        assert!(
            block.contains("<p>le corps</p>"),
            "le HTML d'origine suit tel quel : {block}"
        );
    }

    /// Terrain STOP 2 PLAN-AUDIT-V2 (2026-09-02) : « un mot tapé APRÈS le
    /// bloc a disparu à l'envoi ». Dans un contenteditable, le curseur
    /// posé après le dernier bloc tombe DEDANS ; le mot vivait donc dans
    /// le `<div>` marqué, et la substitution l'emportait. Le bloc se
    /// termine par une ligne vide éditable — le curseur a un dehors.
    #[test]
    fn un_transfert_laisse_une_ligne_editable_apres_le_bloc() {
        let block = quote_forward_html(None, None, None, "<p>corps</p>", Some("1/2/INBOX"));
        assert!(block.ends_with("</div><div><br></div>"), "{block}");
        let sans_source = quote_forward_html(None, None, None, "<p>corps</p>", None);
        assert!(
            sans_source.ends_with("</div><div><br></div>"),
            "{sans_source}"
        );
    }

    #[test]
    fn quote_forward_html_uses_placeholders_for_missing_metadata() {
        let block = quote_forward_html(None, None, None, "<p>corps</p>", None);
        assert!(block.contains("De : (expéditeur inconnu)"), "{block}");
        assert!(!block.contains("Date :"), "{block}");
        assert!(block.contains("Objet : (sans objet)"), "{block}");
    }

    #[test]
    fn quote_reply_attributes_and_prefixes_every_line() {
        let quote = quote_reply(
            Some("Alice Martin"),
            Some("2026-07-17 10:23"),
            "première ligne\n\nseconde ligne",
        );
        assert!(quote.starts_with("\n\nLe 2026-07-17 10:23, Alice Martin a écrit :\n"));
        assert!(quote.contains("> première ligne"));
        assert!(quote.contains("> seconde ligne"));
        assert!(
            !quote.contains("\n\n>"),
            "les lignes vides restent citées : {quote:?}"
        );
    }

    #[test]
    fn quote_reply_degrades_gracefully_without_metadata() {
        let quote = quote_reply(None, None, "texte");
        assert!(quote.contains("(expéditeur inconnu) a écrit :"));
        assert!(quote.contains("> texte"));
    }

    #[test]
    fn quote_reply_of_empty_body_is_empty() {
        assert_eq!(quote_reply(Some("Alice"), None, "  \n "), "");
    }

    #[test]
    fn quote_forward_carries_original_headers_and_text() {
        let block = quote_forward(
            Some("Alice Martin"),
            Some("2026-07-17 10:23"),
            Some("Réunion"),
            "le corps\nsur deux lignes\n",
        );
        assert!(block.contains("---------- Message transféré ----------"));
        assert!(block.contains("De : Alice Martin"));
        assert!(block.contains("Date : 2026-07-17 10:23"));
        assert!(block.contains("Objet : Réunion"));
        assert!(block.ends_with("le corps\nsur deux lignes"));
    }

    #[test]
    fn quote_forward_uses_placeholders_for_missing_metadata() {
        let block = quote_forward(None, None, None, "corps");
        assert!(block.contains("De : (expéditeur inconnu)"));
        assert!(!block.contains("Date :"));
        assert!(block.contains("Objet : (sans objet)"));
    }
}
