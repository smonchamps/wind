//! Stockage local SQLite : enveloppes et état de synchro, multi-boîtes.
//!
//! Structure concrète (pas de trait) : SQLite est une décision produit gelée
//! (PHASE0.md §2.1) et les tests utilisent une base en mémoire — l'abstraction
//! du réseau ([`crate::MailServer`]) est la seule frontière nécessaire.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ops::ControlFlow;
use std::path::Path;

use chrono::DateTime;
use rusqlite::{Connection, OptionalExtension, params};

use crate::action::{Action, PendingAction};
use crate::attachment::Attachment;
use crate::envelope::{Envelope, Uid};
use crate::error::Error;
use crate::invitation::{InvitationRow, InvitationStockee};
use crate::remote::Folder;
use crate::remote::SpecialUse;
use crate::search;
use crate::thread;

const SCHEMA: &str = "
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS accounts (
    id       INTEGER PRIMARY KEY,
    email    TEXT NOT NULL UNIQUE,
    provider TEXT NOT NULL DEFAULT 'gmail',
    -- Le dossier des envois, sous son nom RESEAU, quand le serveur en
    -- expose un. Il complete la portee du regroupement (ADR 0009), et son
    -- nom varie d'un serveur a l'autre — il ne peut donc pas etre en dur.
    --
    -- Porte par le COMPTE et non deduit a la volee : la boite « Envoyes »
    -- est CREEE par la boucle de synchronisation, donc elle n'existe pas
    -- encore quand on declare la portee. Sans cette memoire, elle naitrait
    -- hors portee et ses messages resteraient sans fil jusqu'au prochain
    -- demarrage — le piege de l'adoption differee.
    sent_mailbox TEXT
);
CREATE TABLE IF NOT EXISTS mailboxes (
    id             INTEGER PRIMARY KEY,
    account_id     INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name           TEXT NOT NULL,
    uid_validity   INTEGER NOT NULL,
    last_uid       INTEGER NOT NULL DEFAULT 0,
    highest_modseq INTEGER,
    -- La boite a-t-elle DEJA ete synchronisee une fois (PLAN-AUDIT-V1
    -- E2) ? C'est CE drapeau qui decide initiale / incrementale — jamais
    -- `last_uid == 0` : une boite VIDEE (tout archive) a un max(uid) nul
    -- et redevenait « initiale », donc muette (aucune bulle) et chere.
    initialisee    INTEGER NOT NULL DEFAULT 0,
    -- Derniere releve REUSSIE de la boite (epoch), posee par update_state :
    -- le balayage des echos d envoi exige que les Envoyes aient ete releves
    -- APRES l envoi (PLAN-AUDIT-V2 E5).
    relevee_epoch  INTEGER,
    -- Cette boite participe-t-elle au REGROUPEMENT en fils ?
    --
    -- Depuis l'ADR 0010 on synchronise TOUTES les boites, mais la portee
    -- d'un fil reste INBOX + Envoyes (ADR 0009). Sans ce drapeau, un spam
    -- ou un message archive rejoindrait le fil tout seul — `thread::attach`
    -- travaille par COMPTE — et ferait remonter la conversation en tete de
    -- liste. Defaut de correction, pas d'ergonomie.
    --
    -- DEFAUT A 1 : c'est la reponse de la MIGRATION, pas celle du produit.
    -- Une base d'avant l'ADR 0010 ne contient qu'INBOX et « Envoyes »,
    -- toutes deux dans la portee ; les mettre a 0 viderait la liste au
    -- premier lancement. `create_mailbox` ecrit toujours la valeur
    -- explicitement, donc ce defaut ne decide jamais pour une boite neuve.
    threaded       INTEGER NOT NULL DEFAULT 1,
    -- Combien de messages le SERVEUR annonce dans cette boite (EXISTS),
    -- au dernier passage. Denominateur de l'avancement (ADR 0010 §5).
    --
    -- 0 = jamais selectionnee. Ce n'est PAS « boite vide » : les deux se
    -- distinguent parce que l'avancement doit se taire quand il ne sait
    -- pas, au lieu d'afficher « 0 % » ou « 100 % ».
    remote_total   INTEGER NOT NULL DEFAULT 0,
    UNIQUE (account_id, name)
);
CREATE TABLE IF NOT EXISTS envelopes (
    mailbox_id     INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid            INTEGER NOT NULL,
    subject        TEXT,
    sender         TEXT,
    sender_address TEXT,
    -- Destinataires A / Cc, un par saut de ligne, NULL quand l'ENVELOPE
    -- n'en porte pas (R4, PLAN-RETOURS-MAIL). Ils viennent de la MEME
    -- ENVELOPE que l'expediteur : dans un dossier d'envois l'expediteur
    -- est SOI, seul le destinataire dit a qui le message est parti.
    to_addrs       TEXT,
    cc_addrs       TEXT,
    -- Reply-To, premiere adresse, de la meme ENVELOPE (PLAN-AUDIT-V2 E5).
    reply_to       TEXT,
    message_id     TEXT,
    -- Les deux en-tetes du regroupement en fils. `in_reply_to` vient de
    -- l'ENVELOPE (gratuit) ; `refs` vient d'une passe separee sur les
    -- en-tetes complets, et reste NULL en attendant.
    in_reply_to    TEXT,
    refs           TEXT,
    thread_id      INTEGER,
    date_epoch     INTEGER,
    seen           INTEGER NOT NULL DEFAULT 0,
    flagged        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (mailbox_id, uid)
);
-- `uid` en TROISIEME colonne, et ce n'est pas de l'ornement : sans lui
-- l'index ne COUVRE pas les requetes du rattrapage, qui filtrent par date
-- puis sondent `bodies` par (mailbox_id, uid). SQLite devait alors aller
-- chercher la LIGNE d'enveloppe pour y lire l'uid, une fois par message.
-- Mesure du 2026-08-26 sur la base du terrain : `pending_total` 521,9 ms
-- avec l'index a deux colonnes, 107,9 ms avec celui-ci (pire dossier,
-- 87 117 enveloppes : 400,5 -> 46,3 ms). L'ordre DESC de la date reste
-- celui de la pagination ; uid ne le derange pas, il le complete.
CREATE INDEX IF NOT EXISTS idx_envelopes_date ON envelopes(mailbox_id, date_epoch DESC, uid);
CREATE TABLE IF NOT EXISTS bodies (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    html       TEXT NOT NULL,
    -- VESTIGIALE depuis le 2026-08-26 (PLAN-DEMARRAGE, decision D8) :
    -- ecrite a 1 par save_body_full, plus JAMAIS LUE. Elle marquait les
    -- corps rapatries AVANT que les pieces jointes existent, dont le MIME
    -- n'avait jamais ete inspecte ; le rattrapage les reprenait. La lire
    -- coutait 251 k rappels de ligne grasse dans 11,4 Go — 20 839 ms a
    -- froid contre 396 ms sans (mesure du 2026-08-26) — pour proteger
    -- ZERO ligne : la passe d'heritage est soldee sur toute la flotte, et
    -- rien en production n'ecrit plus 0. La retirer demanderait une
    -- reecriture de 11,4 Go : elle partira avec le chantier qui touchera
    -- `bodies` de toute facon.
    scanned    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (mailbox_id, uid)
);
-- Métadonnées seules : jamais les octets. Ils se retéléchargent à la
-- demande (ADR 0007 — le budget disque ne survivrait pas aux fichiers).
CREATE TABLE IF NOT EXISTS attachments (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    idx        INTEGER NOT NULL,
    name       TEXT NOT NULL,
    mime       TEXT NOT NULL,
    size       INTEGER NOT NULL,
    PRIMARY KEY (mailbox_id, uid, idx)
);
-- Liste des dossiers, mise en cache comme les enveloppes : choisir une
-- destination doit marcher HORS LIGNE, sinon le tri s'arrête avec le
-- réseau. Rafraîchie à chaque synchro.
CREATE TABLE IF NOT EXISTS folders (
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    wire       TEXT NOT NULL,
    display    TEXT NOT NULL,
    selectable INTEGER NOT NULL DEFAULT 1,
    -- Le role RFC 6154 (all, archive, drafts, junk, sent, trash), NULL
    -- quand le serveur n en annonce pas (PLAN-AUDIT-V2 E5).
    special_use TEXT,
    PRIMARY KEY (account_id, wire)
);
CREATE TABLE IF NOT EXISTS pending_actions (
    id         INTEGER PRIMARY KEY,
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    kind       TEXT NOT NULL,
    -- PLAN-AUDIT-V1 E3 : une action que le serveur refuse (NO/BAD) ou qui
    -- echoue SEUIL_QUARANTAINE fois entre en QUARANTAINE (refusee = 1) :
    -- elle sort de la file active — plus rien ne bloque les suivantes —
    -- mais reste visible (fente d'avis, D2) avec son motif.
    attempts   INTEGER NOT NULL DEFAULT 0,
    refusee    INTEGER NOT NULL DEFAULT 0,
    last_error TEXT
);
CREATE INDEX IF NOT EXISTS idx_pending_actions_message ON pending_actions(mailbox_id, uid);
CREATE TABLE IF NOT EXISTS drafts (
    id            INTEGER PRIMARY KEY,
    account_id    INTEGER NOT NULL DEFAULT 1,
    to_raw        TEXT NOT NULL,
    -- Cc et Cci bruts, non validés (comme to_raw) — la validation stricte
    -- n'intervient qu'à l'envoi (compose). Vides par défaut : un brouillon
    -- d'avant ces colonnes n'a ni l'un ni l'autre.
    cc_raw        TEXT NOT NULL DEFAULT '',
    bcc_raw       TEXT NOT NULL DEFAULT '',
    subject       TEXT NOT NULL,
    body          TEXT NOT NULL,
    -- Corps riche du brouillon (PLAN-COMPOSITION-HTML). NULL = brouillon
    -- texte (d'avant la colonne, ou rapatrié du serveur) ; `body` reste
    -- TOUJOURS peuplé — le texte dérivé sert d'aperçu et de repli.
    body_html     TEXT,
    reply_to_uid  INTEGER,
    -- La boîte qui donne son sens à reply_to_uid (ADR 0009) — le lien
    -- brouillon -> conversation (PLAN-BROUILLONS, B-D2). NULL avant la
    -- colonne : ces brouillons restent sans fil, jamais mal reliés.
    reply_to_mailbox TEXT,
    -- Marqué « important » (R3, PLAN-RETOURS-6) : l'état suit le
    -- brouillon jusqu'à l'envoi.
    important     INTEGER NOT NULL DEFAULT 0,
    updated_epoch INTEGER NOT NULL,
    remote_uid    INTEGER,
    pushed_epoch  INTEGER
);
-- Les octets des pièces d'un brouillon, copiés AU GESTE (PLAN-PIECES-JOINTES,
-- PJ-D1) : jamais de chemin nu en base — un fichier déplacé ou supprimé
-- après le geste ne peut plus rien casser. À l'inverse de `attachments`
-- (réception, métadonnées seules), ici les octets sont à NOUS : c'est le
-- message qu'on promet d'envoyer.
CREATE TABLE IF NOT EXISTS draft_attachments (
    id       INTEGER PRIMARY KEY,
    draft_id INTEGER NOT NULL REFERENCES drafts(id) ON DELETE CASCADE,
    name     TEXT NOT NULL,
    mime     TEXT NOT NULL,
    size     INTEGER NOT NULL,
    bytes    BLOB NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_draft_attachments_draft ON draft_attachments(draft_id);
CREATE TABLE IF NOT EXISTS draft_tombstones (
    account_id INTEGER NOT NULL,
    remote_uid INTEGER NOT NULL,
    PRIMARY KEY (account_id, remote_uid)
);
CREATE TABLE IF NOT EXISTS drafts_remote (
    account_id   INTEGER PRIMARY KEY,
    uid_validity INTEGER NOT NULL
);
-- Préférences de l'application persistées EN BASE (pas localStorage) :
-- elles doivent être lisibles par le shell Rust — la garde des bulles
-- d'arrivée se joue à l'émission, côté Rust (PLAN-REGLAGES, R-D2).
CREATE TABLE IF NOT EXISTS prefs (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS outbox (
    id           INTEGER PRIMARY KEY,
    account_id   INTEGER NOT NULL DEFAULT 1,
    message_id   TEXT NOT NULL,
    sender       TEXT NOT NULL,
    recipients   TEXT NOT NULL,
    cc_addrs     TEXT NOT NULL DEFAULT '',
    bcc_addrs    TEXT NOT NULL DEFAULT '',
    subject      TEXT NOT NULL,
    body_text    TEXT NOT NULL,
    -- Corps riche de l'envoi (PLAN-COMPOSITION-HTML) : ce que porte la
    -- partie text/html du multipart/alternative. NULL = envoi texte seul
    -- (chemin historique, octet pour octet inchangé).
    body_html    TEXT,
    in_reply_to  TEXT,
    -- E7 : la chaine References complete (RFC 5322 §3.6.4), NULL = le
    -- parent seul (chemin d'avant). `refs` comme dans envelopes :
    -- REFERENCES est un mot reserve de SQLite.
    refs         TEXT,
    -- Marqué « important » (R3) : la remise posera les en-têtes de
    -- priorité (X-Priority + Importance).
    important    INTEGER NOT NULL DEFAULT 0,
    -- Envoi différé (R2, PLAN-RETOURS-6) : l'époque (secondes) avant
    -- laquelle la vidange ne doit PAS prendre ce message. NULL = tout
    -- de suite (chemin historique).
    send_at_epoch INTEGER,
    -- Réponse iTIP d'une invitation (PLAN-INVITATIONS) : la remise la
    -- porte en partie text/calendar; method=REPLY. NULL = envoi
    -- ordinaire (chemin historique, octet pour octet inchangé).
    ics_reply    TEXT,
    state        TEXT NOT NULL DEFAULT 'queued',
    attempts     INTEGER NOT NULL DEFAULT 0,
    last_error   TEXT,
    queued_epoch INTEGER NOT NULL
);
-- Les pièces du journal d'envoi, copiées de `draft_attachments` dans la
-- transaction du geste d'envoi (PJ-D2 : « jamais d'envoi perdu » couvre
-- les octets). `bytes` passe à NULL au passage à `sent` (PJ-D7) : les
-- métadonnées restent lisibles, la quarantaine et le refus gardent leurs
-- octets — le renvoi sur décision de l'utilisateur doit rester entier.
CREATE TABLE IF NOT EXISTS outbox_attachments (
    id        INTEGER PRIMARY KEY,
    outbox_id INTEGER NOT NULL REFERENCES outbox(id) ON DELETE CASCADE,
    name      TEXT NOT NULL,
    mime      TEXT NOT NULL,
    size      INTEGER NOT NULL,
    bytes     BLOB
);
CREATE INDEX IF NOT EXISTS idx_outbox_attachments_outbox ON outbox_attachments(outbox_id);
-- L'echo local d'un geste (PLAN-REACTIVITE E3, R-D1 « < 1 s ») : la
-- copie de DESTINATION d'une suppression, d'un archivage ou d'un envoi,
-- visible en liste AVANT que le serveur ait suivi. JAMAIS dans
-- `envelopes` : un UID invente forgerait la cle (mailbox, uid) sur
-- laquelle tout repose. L'echo meurt a la reconciliation (la vraie
-- ligne entre, meme message_id) ou au balayage (le serveur dement).
-- `destination` est une categorie canonique : 'envoyes' | 'archives' |
-- 'corbeille'. `origin_action_id` (geste journalise) et
-- `origin_outbox_id` (envoi) disent l'INTENTION dont l'echo est le
-- reflet — un echo sans intention n'existe pas.
CREATE TABLE IF NOT EXISTS echos (
    id               INTEGER PRIMARY KEY,
    account_id       INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    destination      TEXT NOT NULL,
    message_id       TEXT NOT NULL,
    sender           TEXT,
    sender_address   TEXT,
    subject          TEXT,
    date_epoch       INTEGER,
    preview          TEXT,
    html             TEXT,
    attachment_count INTEGER NOT NULL DEFAULT 0,
    -- PLAN-RETOURS-5 : les destinataires de l'echo, au format des
    -- enveloppes (adresses jointes par un saut de ligne) — la liste
    -- d'Envoyes dit « A : X », jamais le slug de destination. NULL sur
    -- l'existant (echos morts a la reconciliation de toute facon).
    -- D-36 : JAMAIS de sequence d'echappement Rust (antislash-n) dans ce
    -- commentaire SQL — elle y devenait un vrai saut de ligne et SQLite
    -- avalait la suite comme une colonne fantome (base neuve, 2026-08-26).
    to_addrs         TEXT,
    origin_action_id INTEGER,
    origin_outbox_id INTEGER,
    created_epoch    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_echos_destination ON echos(destination, account_id);
-- L'annuaire des correspondants (PLAN-RETOURS-5, D4) : appris du
-- courrier vu (expediteurs hors indesirables/corbeille, destinataires
-- de NOS envois), jamais un carnet edite. Adresse en minuscules
-- (dedoublonnage), le nom d'affichage le plus recent gagne. Table
-- PETITE interrogee a la frappe — jamais un parcours d'envelopes par
-- frappe dans la file serialisee (lecon PLAN-DEFILEMENT-PROFOND).
CREATE TABLE IF NOT EXISTS correspondants (
    address    TEXT PRIMARY KEY,
    name       TEXT,
    last_epoch INTEGER NOT NULL DEFAULT 0,
    hits       INTEGER NOT NULL DEFAULT 0
);
-- L'epingle LOCALE d'une conversation (PLAN-RETOURS-7, R4) : cle
-- d'ENVELOPPE, pas de fil — les tables de fils se DROPent a l'adoption
-- (thread::drop_if_outdated), une epingle portee par `threads` mourrait
-- a la migration suivante. Le fil se retrouve par jointure. JAMAIS la
-- colonne `flagged` : elle est ecrasee par la verite serveur a chaque
-- synchro (upsert_envelopes), et l'etoile IMAP est une autre semantique.
-- Locale par decision (D-refus) : IMAP n'a pas ce concept.
CREATE TABLE IF NOT EXISTS pins (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    epoch      INTEGER NOT NULL,
    PRIMARY KEY (mailbox_id, uid)
);
-- Mis de côté (PLAN-MODE-ORGANISE E5) : la pile du mode organisé —
-- copie du patron `pins` (clé d'ENVELOPPE : survit à la reconstruction
-- des fils, meurt avec sa boîte — purges `reset_mailbox`/`remove_local`
-- comprises, leçon RETOURS-11). Un fil mis de côté quitte TOUTES les
-- vues organisées ; « Terminé » (DELETE) le rend d'où il vient. Le
-- classique n'en sait rien.
CREATE TABLE IF NOT EXISTS mis_de_cote (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    epoch      INTEGER NOT NULL,
    PRIMARY KEY (mailbox_id, uid)
);
-- La mémoire « lu » du Kiosque (RETOURS-13 R10) : une carte défilée
-- jusqu'en bas est lue — copie du patron `pins`/`mis_de_cote` (clé
-- d'enveloppe, locale au poste, meurt avec sa boîte et son message).
-- Le lu IMAP (`seen`) est une autre sémantique : il est écrasé par la
-- vérité serveur à chaque synchro, et le Kiosque ne « traite » pas.
CREATE TABLE IF NOT EXISTS kiosque_lus (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    epoch      INTEGER NOT NULL,
    PRIMARY KEY (mailbox_id, uid)
);
-- La mémoire de la garde d'images (PLAN-RETOURS-11, R1 — D1 renverse
-- l'invariant A43) : deux exceptions EXPLICITES au blocage par défaut,
-- jamais un réglage global. Par MESSAGE : clé d'enveloppe, patron de
-- `pins` (survit à la reconstruction des fils, meurt avec sa boîte).
-- Par EXPÉDITEUR : adresse exacte en minuscules (normalisée par le
-- Rust, comme `correspondants`), GLOBALE au poste (D3 — survit au
-- retrait d'un compte).
CREATE TABLE IF NOT EXISTS images_messages (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    epoch      INTEGER NOT NULL,
    PRIMARY KEY (mailbox_id, uid)
);
CREATE TABLE IF NOT EXISTS images_expediteurs (
    address TEXT PRIMARY KEY,
    epoch   INTEGER NOT NULL
);
-- Le routage du Mode organise (PLAN-MODE-ORGANISE E1, decision D1 :
-- routage LOCAL seul — la destination est une PRESENTATION, jamais un
-- deplacement IMAP ; les autres clients voient le courrier inchange).
-- Cle : adresse exacte en minuscules (la MEME autorite de
-- normalisation que la garde d'images), GLOBALE au poste comme
-- images_expediteurs — le verdict sur un expediteur survit au retrait
-- d'un compte. `regle` : l'automatisme du Non (spam/archive/corbeille
-- — D4 : JAMAIS une suppression definitive), NULL = ecarte sans
-- regle ; une regle n'existe que sur un expediteur `ecarte`.
-- « Reintegrer » a l'historique du Portier = DELETE de la ligne. Le
-- vocabulaire est verifie en Rust AVANT l'ecriture ; les CHECK ne
-- sont que la ceinture.
CREATE TABLE IF NOT EXISTS routage_expediteurs (
    address     TEXT PRIMARY KEY,
    destination TEXT NOT NULL CHECK (destination IN ('reception','kiosque','registre','ecarte')),
    regle       TEXT CHECK (regle IN ('spam','archive','corbeille')),
    epoch       INTEGER NOT NULL
);
-- L'attente du Portier (PLAN-MODE-ORGANISE E2, D3 « arrivées
-- seules ») : les expéditeurs SANS ligne de routage dont le courrier
-- n'existe QU'APRÈS l'époque d'activation. MATÉRIALISÉE et entretenue
-- à l'arrivée (spike S2-bis : la calculer dans la requête chaude coûte
-- 299 ms à l'offset profond, la sonde PK est gratuite ; l'entretien
-- vaut 7 µs/message). DÉRIVÉE du courrier — jamais une décision : elle
-- se défait quand le courrier ancien arrive (backfill) ou disparaît
-- (réinitialisation), et meurt au verdict (la ligne de routage prend
-- le relais). Une seule colonne : l'appartenance EST la donnée — tout
-- le reste (dernier message, comptes) se lit du courrier.
CREATE TABLE IF NOT EXISTS portier_attente (
    address TEXT PRIMARY KEY
);
-- La session du Nettoyage de printemps (PLAN-HORIZON-NETTOYAGE volet
-- B, D8 : persistee — un nettoyage entame reprend apres redemarrage).
-- UNE ligne au plus (id = 1). La borne est FIGEE au demarrage
-- (borne_epoch, derivee de la plage choisie) ; les verdicts vivent
-- dans routage_expediteurs — la session ne porte que la plage, le
-- perimetre et la progression (total de groupes au depart, traites).
CREATE TABLE IF NOT EXISTS nettoyage_session (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    plage       TEXT NOT NULL,
    perimetre   TEXT NOT NULL,
    borne_epoch INTEGER NOT NULL,
    total       INTEGER NOT NULL,
    traites     INTEGER NOT NULL DEFAULT 0
);
-- L'invitation de reunion d'un message (PLAN-INVITATIONS) : le CACHE de
-- la partie text/calendar, extrait au scan du corps (save_body_full) ou
-- a l'ouverture pour un message d'avant la fonctionnalite (write-back,
-- invariant d'adoption 6.7 — jamais de migration de masse). Cle
-- d'enveloppe, comme `attachments` ; le MIME brut n'est jamais stocke.
-- `partstat` = notre statut LU du REQUEST ; `reponse` = notre derniere
-- reponse PARTIE par la boite d'envoi (D6) — deux verites distinctes.
-- Les epochs sont UTC ; quand un horaire n'est pas resolu (journee
-- entiere, TZID inconnu), la forme TEXTE fait foi et l'epoch reste NULL
-- (garde D1 : jamais une conversion mensongere).
CREATE TABLE IF NOT EXISTS invitations (
    mailbox_id           INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid                  INTEGER NOT NULL,
    methode              TEXT NOT NULL,
    event_uid            TEXT NOT NULL,
    sequence             INTEGER NOT NULL DEFAULT 0,
    titre                TEXT NOT NULL DEFAULT '',
    lieu                 TEXT,
    organisateur_adresse TEXT,
    organisateur_nom     TEXT,
    debut_epoch          INTEGER,
    fin_epoch            INTEGER,
    debut_texte          TEXT,
    fin_texte            TEXT,
    journee_entiere      INTEGER NOT NULL DEFAULT 0,
    recurrent            INTEGER NOT NULL DEFAULT 0,
    partstat             TEXT,
    repondant_adresse    TEXT,
    repondant_nom        TEXT,
    repondant_statut     TEXT,
    -- Le lien d'annulation CROISE (terrain R6, 2026-08-22) : un CANCEL
    -- etaint le REQUEST de la meme reunion (meme event_uid, meme
    -- compte), quel que soit l'ordre d'arrivee des scans — sans lui,
    -- l'annulation arrivait dans une conversation neuve et l'invitation
    -- d'origine continuait d'offrir Accepter.
    annule               INTEGER NOT NULL DEFAULT 0,
    reponse              TEXT,
    reponse_epoch        INTEGER,
    PRIMARY KEY (mailbox_id, uid)
);
";

/// Écrit (ou remplace) la ligne d'invitation d'un message, en PRÉSERVANT
/// notre réponse locale : `reponse`/`reponse_epoch` ne sont jamais
/// touchées ici (D6) — le PARTSTAT relu du message et la réponse partie
/// de Wind sont deux vérités distinctes.
fn ecrire_invitation(
    conn: &Connection,
    mailbox_id: i64,
    uid: Uid,
    row: &InvitationRow,
) -> Result<(), Error> {
    // Le lien d'annulation croisé (terrain R6), dans les DEUX ordres
    // d'arrivée : un REQUEST écrit APRÈS le CANCEL de sa réunion naît
    // annulé ; un CANCEL écrit APRÈS éteint les REQUEST existants.
    // La réunion est identifiée par (event_uid, compte) — jamais
    // l'event_uid seul : deux comptes peuvent recevoir la même réunion.
    let annule = row.annule
        || (row.methode == "request"
            && conn
                .prepare(
                    "SELECT 1 FROM invitations i
                      JOIN mailboxes m ON m.id = i.mailbox_id
                     WHERE i.event_uid = ?1 AND i.methode = 'cancel'
                       AND m.account_id =
                           (SELECT account_id FROM mailboxes WHERE id = ?2)",
                )?
                .exists(params![row.event_uid, mailbox_id])?);
    conn.execute(
        "INSERT INTO invitations (mailbox_id, uid, methode, event_uid, sequence, titre,
             lieu, organisateur_adresse, organisateur_nom, debut_epoch, fin_epoch,
             debut_texte, fin_texte, journee_entiere, recurrent, partstat,
             repondant_adresse, repondant_nom, repondant_statut, annule)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
             ?16, ?17, ?18, ?19, ?20)
         ON CONFLICT(mailbox_id, uid) DO UPDATE SET
             methode = excluded.methode, event_uid = excluded.event_uid,
             sequence = excluded.sequence, titre = excluded.titre,
             lieu = excluded.lieu,
             organisateur_adresse = excluded.organisateur_adresse,
             organisateur_nom = excluded.organisateur_nom,
             debut_epoch = excluded.debut_epoch, fin_epoch = excluded.fin_epoch,
             debut_texte = excluded.debut_texte, fin_texte = excluded.fin_texte,
             journee_entiere = excluded.journee_entiere,
             recurrent = excluded.recurrent, partstat = excluded.partstat,
             repondant_adresse = excluded.repondant_adresse,
             repondant_nom = excluded.repondant_nom,
             repondant_statut = excluded.repondant_statut,
             annule = excluded.annule",
        params![
            mailbox_id,
            uid,
            row.methode,
            row.event_uid,
            row.sequence,
            row.titre,
            row.lieu,
            row.organisateur_adresse,
            row.organisateur_nom,
            row.debut_epoch,
            row.fin_epoch,
            row.debut_texte,
            row.fin_texte,
            row.journee_entiere,
            row.recurrent,
            row.partstat,
            row.repondant_adresse,
            row.repondant_nom,
            row.repondant_statut,
            annule
        ],
    )?;
    if row.methode == "cancel" {
        conn.execute(
            "UPDATE invitations SET annule = 1
             WHERE event_uid = ?1 AND methode = 'request' AND annule = 0
               AND mailbox_id IN
                   (SELECT id FROM mailboxes WHERE account_id =
                        (SELECT account_id FROM mailboxes WHERE id = ?2))",
            params![row.event_uid, mailbox_id],
        )?;
    }
    Ok(())
}

/// Avancement de l'adoption d'une base héritée, pour l'affichage.
///
/// `total` est un MAJORANT déclaré d'emblée (il ne bouge jamais en cours
/// de passe : une barre qui recule est pire qu'une barre imprécise), et
/// `fait == total` n'est annoncé qu'une fois la passe COMMISE — jamais
/// avant, c'est l'exigence « un signal doit être observable » (§9 de la
/// passation). L'affichage passe par [`crate::sync_percent`], qui porte
/// déjà les cas dégénérés.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdoptionProgress {
    pub done: u64,
    pub total: u64,
}

/// État de synchro persisté d'une boîte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncState {
    pub mailbox_id: i64,
    pub uid_validity: u32,
    pub last_uid: Uid,
    pub highest_modseq: Option<u64>,
    /// La boîte a déjà été synchronisée une fois : c'est ce qui décide
    /// initiale / incrémentale (E2), jamais `last_uid`.
    pub initialisee: bool,
}

/// Un compte connecté au client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: i64,
    pub email: String,
    pub provider: String,
}

/// Une ligne de la boîte unifiée : l'enveloppe ET son compte — un UID
/// seul n'identifie plus un message en multi-comptes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedRow {
    pub account_id: i64,
    pub account_email: String,
    /// La boîte qui CONTIENT ce message, sous son nom réseau.
    ///
    /// Sans elle, `(account_id, uid)` n'identifie plus rien depuis que les
    /// fils réunissent plusieurs boîtes ([ADR 0009]) : les UID sont
    /// attribués par boîte et repartent de 1, donc le message n°1 d'INBOX
    /// et le n°1 d'« Envoyés » sont deux messages différents du même
    /// compte. Toute lecture et toute action doivent la porter.
    pub mailbox: String,
    pub envelope: Envelope,
    /// Le message porte-t-il au moins une piece jointe ?
    ///
    /// Faux tant que son corps n'a pas ete lu — meme condition que la
    /// recherche dans le texte. Le trombone apparait donc au fil du
    /// rattrapage, jamais a tort.
    pub has_attachment: bool,
    /// COMBIEN de pièces jointes — la puce du prototype dit « 2
    /// fichiers », pas « des fichiers ». 0 tant que le corps n'a pas été
    /// lu, comme `has_attachment`.
    pub attachment_count: u32,
    /// L'aperçu texte sous l'objet (écran 02) — calculé à l'écriture du
    /// corps, `None` tant que le corps n'est pas rapatrié.
    pub preview: Option<String>,
    /// Le fil auquel ce message appartient. `None` seulement pendant la
    /// fenêtre où une base héritée n'a pas encore été adoptée.
    pub thread_id: Option<i64>,
    /// Nombre de messages du fil, **reçus et envoyés confondus**.
    /// 1 = message isolé.
    ///
    /// Depuis l'ADR 0009, un fil appartient au COMPTE et non à une boîte :
    /// nos propres réponses en font partie. Le compteur doit donc les
    /// inclure, sans quoi il contredirait à l'écran le bandeau de
    /// conversation, qui montre l'échange entier.
    pub thread_size: u32,
    /// Non-lus du fil. Un fil se montre non lu tant qu'il en reste un,
    /// même si son dernier message est lu.
    pub thread_unseen: u32,
    /// L'invitation du fil (terrain R10/R11) — posée par
    /// [`Store::enrichir_lignes`] sur la PAGE servie, jamais par la
    /// requête chaude. `None` dans tous les autres chemins.
    pub invitation: Option<InvitationRang>,
}

/// L'invitation d'une ligne de liste (terrain R10/R11) : ce que le rang
/// de puces affiche (réponse donnée, annulation) et la clé pour
/// répondre DEPUIS la liste — la boîte et l'UID du message
/// d'invitation, qui n'est pas forcément la tête affichée du fil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitationRang {
    pub mailbox: String,
    pub uid: Uid,
    /// Le titre de la réunion — le sujet de la réponse se construit de
    /// lui, jamais du sujet de la tête du fil (« Re : … »).
    pub titre: String,
    /// Notre dernière réponse partie (`accepte`|`provisoire`|`refuse`).
    pub reponse: Option<String>,
    pub annulee: bool,
    pub peut_repondre: bool,
}

/// Colonnes du SELECT unifié, partagées par [`Store::unified_recent`] et
/// [`Store::search`] — l'ordre est celui de [`row_to_unified`].
/// La derniere colonne est un EXISTS sur `attachments` : la liste doit
/// pouvoir afficher le trombone sans une requete par ligne. La cle
/// primaire (mailbox_id, uid, idx) rend ce test indexe.
// Exige les alias `e` (envelopes), `m` (mailboxes), `a` (accounts) ET la
// jointure `LEFT JOIN bodies b` — l'aperçu de liste vient de là, NULL
// tant que le corps n'est pas rapatrié. Le COUNT de pièces jointes
// remplace l'ancien EXISTS : la puce du prototype dit « 2 fichiers »,
// pas « des fichiers ». Les deux ne s'exécutent que sur les lignes
// RETENUES par la pagination (gate P1).
pub(crate) const SELECT_UNIFIED: &str = "SELECT a.id, a.email, e.uid, e.subject, e.sender, e.sender_address, e.message_id, e.date_epoch, e.seen, e.flagged, (SELECT COUNT(*) FROM attachments att WHERE att.mailbox_id = e.mailbox_id AND att.uid = e.uid), e.thread_id, e.in_reply_to, m.name, b.preview, e.to_addrs, e.cc_addrs";

/// Le SELECT de la liste groupée : les colonnes ci-dessus, plus l'agrégat
/// du fil. Il exige la jointure sur `threads` (alias `t`), que la
/// recherche n'a pas — un résultat de recherche est UN message, pas une
/// conversation. Vient APRÈS `to_addrs`/`cc_addrs` de [`SELECT_UNIFIED`] :
/// `t.size`/`t.unseen` sont donc aux index 17/18.
pub(crate) const THREAD_AGGREGATE: &str = ", t.size, t.unseen";

/// Les fils ÉPINGLÉS (R4, PLAN-RETOURS-7) — la sous-requête partagée
/// par la page (exclusion, D5), le comptage et le service à part.
/// Matérialisée UNE fois par requête (LIST SUBQUERY), petite par
/// construction (quelques épingles au plus) — mais SEULEMENT si `pins`
/// est la table extérieure : sans `ANALYZE` (jamais exécuté ici),
/// SQLite choisit `envelopes` en extérieure et paie un scan COMPLET de
/// la table la plus large sur le chemin le plus chaud (revue
/// 2026-08-21, mesuré au banc : ~24 ms la page à 200 k). Le
/// `CROSS JOIN` est la directive d'ordre de SQLite : `pins` se
/// parcourt, `envelopes` se sonde par sa clé primaire. La garde de
/// plan `la_boite_unifiee_ne_materialise_pas_son_tri` le prouve.
pub(crate) const PINNED_THREADS: &str = "SELECT pe.thread_id FROM pins p CROSS JOIN envelopes pe ON pe.mailbox_id = p.mailbox_id AND pe.uid = p.uid WHERE pe.thread_id IS NOT NULL";

/// Les fils MIS DE CÔTÉ (E5) — le jumeau de [`PINNED_THREADS`], mêmes
/// raisons : liste matérialisée une fois, petite par construction, et
/// `CROSS JOIN` directif (sans ANALYZE, SQLite choisirait `envelopes`
/// en extérieure — le scan complet sur le chemin le plus chaud).
pub(crate) const MIS_DE_COTE_THREADS: &str = "SELECT ce.thread_id FROM mis_de_cote c CROSS JOIN envelopes ce ON ce.mailbox_id = c.mailbox_id AND ce.uid = c.uid WHERE ce.thread_id IS NOT NULL";

/// L'exclusion de la Réception ORGANISÉE — LA seule écriture (revue
/// E4/E5 : le fragment vivait en quatre copies, la prochaine exclusion
/// — E6 les groupes — en aurait oublié une, exactement la panne
/// « pastille à 2 devant une liste vide » que la capture E5 a payée) :
/// les fils retenus/routés (drapeau) et les fils MIS DE CÔTÉ (pile).
pub(crate) fn exclusion_organisee() -> String {
    format!(" AND organise_hors = 0 AND id NOT IN ({MIS_DE_COTE_THREADS})")
}

/// La queue de la liste unifiée — jointures et tri final — partagée
/// par la page ([`unified_page_sql`]) et la section épinglée
/// ([`Store::pinned_unified_scoped`]) : UNE écriture, les deux
/// requêtes ne peuvent plus dériver (revue 2026-08-21 — la copie du
/// squelette aurait décalé les colonnes au premier ajout).
pub(crate) const UNIFIED_JOINS: &str = "
         JOIN envelopes e ON e.mailbox_id = t.last_mailbox_id AND e.uid = t.last_uid
         JOIN mailboxes m ON m.id = e.mailbox_id
         JOIN accounts a ON a.id = t.account_id
         LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid";

/// Le tri du flot unifié — la date seule (classique) ou les SECTIONS de
/// la Réception organisée (E4, verdict S1/A2) : les non-lus d'abord —
/// « Nouveau pour vous » — puis le reste — « Déjà consulté » —, la date
/// à l'intérieur de chaque section. UN flot, UN offset : l'ordre porte
/// les sections, la couture est le COUNT des non-lus (0,37 ms mesurés).
pub(crate) fn unified_join_tail(sections: bool) -> String {
    let ordre = if sections {
        "ORDER BY (t.unseen > 0) DESC, t.last_epoch DESC, t.last_uid DESC, a.id"
    } else {
        "ORDER BY t.last_epoch DESC, t.last_uid DESC, a.id"
    };
    format!("{UNIFIED_JOINS}\n         {ordre}")
}

/// Les préfixes des prefs suffixées par compte (`{prefixe}.{account_id}`).
/// LA liste que `delete_account` purge : `accounts.id` est un INTEGER
/// PRIMARY KEY sans AUTOINCREMENT — SQLite réutilise le plus grand rowid
/// libéré, et un compte ajouté après un retrait hériterait sinon de
/// l'identité de l'ancien (revue PLAN-RETOURS-8). Toute pref par compte
/// neuve s'ajoute ICI, pas dans un site d'appel (revue 2026-08-23 : la
/// liste vivait en dur dans la requête, à un crate de distance des
/// helpers qui frappent les clés).
pub const PREFS_PAR_COMPTE: &[&str] = &[
    "signature",
    "signature_replies",
    "repere_icone",
    "repere_teinte",
    "nom_compte",
    "horizon_import",
];

/// Les vocabulaires FERMÉS du Nettoyage de printemps (volet B, D6) —
/// vérifiés côté cœur avant toute écriture, comme le routage.
pub const PLAGES_NETTOYAGE: &[&str] = &["3m", "6m", "1a", "2a", "5a", "tout"];
pub const PERIMETRES_NETTOYAGE: &[&str] =
    &["reception", "dossiers", "dossiersArchives", "archives"];

/// La session de nettoyage en cours (une seule, persistée — D8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionNettoyage {
    pub plage: String,
    pub perimetre: String,
    pub borne_epoch: i64,
    pub total: u64,
    pub traites: u64,
}

/// Un groupe du Nettoyage : un expéditeur × son courrier de la plage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupeNettoyage {
    pub address: String,
    pub qui: Option<String>,
    pub messages: u64,
    pub dernier_epoch: i64,
    pub dernier_objet: Option<String>,
}

pub struct Store(Connection);

impl Store {
    /// Accès réservé aux modules du crate qui étendent le stockage
    /// (boîte d'envoi, dans `outbox.rs`) sans grossir ce fichier.
    pub(crate) fn conn(&self) -> &Connection {
        &self.0
    }

    pub fn open(path: &Path) -> Result<Self, Error> {
        Self::init(Connection::open(path)?)
    }

    /// Ouvre en rendant l'adoption d'une base héritée VISIBLE et
    /// INTERRUPTIBLE (Phase 5, chantier arbitré — passation §8).
    ///
    /// `on_progress` est appelé pendant la passe d'adoption avec
    /// l'avancement `(fait, total)`. Répondre [`ControlFlow::Break`]
    /// annule : **tout est défait** (`ROLLBACK`), `PRAGMA user_version`
    /// reste inchangé, et l'ouverture rend [`Error::Interrupted`] — la
    /// passe entière se rejouera au prochain lancement. Jamais d'adoption
    /// partielle persistée : la liste part de `threads`, une base à
    /// moitié adoptée serait une boîte à moitié vide.
    ///
    /// Sur une base à jour, `on_progress` n'est JAMAIS appelé : rien à
    /// adopter, rien à raconter — pas de faux bandeau à chaque lancement.
    pub fn open_with_progress(
        path: &Path,
        mut on_progress: impl FnMut(AdoptionProgress) -> ControlFlow<()>,
    ) -> Result<Self, Error> {
        Self::init_with(Connection::open(path)?, &mut on_progress)
    }

    pub fn open_in_memory() -> Result<Self, Error> {
        Self::init(Connection::open_in_memory()?)
    }

    /// Une adoption de base héritée attend-elle ici ? Sonde en **lecture
    /// seule** : rien n'est déclenché, rien n'est créé — c'est ce qui
    /// permet au desktop d'afficher l'écran de migration AVANT la
    /// première vraie ouverture, celle qui paiera la passe.
    ///
    /// Rend le nombre de messages concernés (`None` = rien à faire).
    /// C'est un ordre de grandeur pour l'écran d'attente, pas le
    /// dénominateur de l'avancement : celui-ci arrive par
    /// [`Store::open_with_progress`], seule à connaître la portée exacte.
    pub fn pending_adoption(path: &Path) -> Result<Option<u64>, Error> {
        if !path.exists() {
            // Première installation : rien d'hérité, et ouvrir créerait
            // le fichier — une sonde ne laisse pas de trace.
            return Ok(None);
        }
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        // Deux passes distinctes peuvent réclamer l'écran, indépendamment :
        // l'adoption des fils (base d'avant ADR 0008) ET la reconstruction
        // de l'index de recherche (schéma FTS d'avant la colonne
        // `recipients`). La seconde touche des bases DÉJÀ à jour côté fils —
        // sans cette détection, elle gèlerait le démarrage en silence, hors
        // de tout écran (constat terrain 2026-08-17).
        let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        let threads_pending = version < thread::THREADING_VERSION;
        let search_pending = {
            let fts_sql: Option<String> = conn
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'search_fts'",
                    [],
                    |row| row.get(0),
                )
                .optional()?;
            fts_sql
                .as_deref()
                .is_some_and(|sql| !sql.contains("recipients"))
        };
        if !threads_pending && !search_pending {
            return Ok(None);
        }
        // Une base d'avant les fils peut ne pas avoir la table : le COUNT
        // direct échouerait, et la sonde doit répondre, pas expliquer.
        let has_envelopes: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'envelopes'",
            [],
            |row| row.get(0),
        )?;
        if has_envelopes == 0 {
            return Ok(None);
        }
        // La reconstruction de l'index parcourt TOUTES les enveloppes ;
        // l'adoption des fils, seulement la portée du regroupement (ADR 0010 :
        // INBOX + Envoyés, très en dessous du total — « 256 312 » pour une
        // passe qui en rattache 7 500 ne désignerait pas ce qu'il dit). On
        // annonce la passe la plus large en attente ; ce n'est qu'un ordre de
        // grandeur, le vrai dénominateur vient d'`open_with_progress`.
        let messages: i64 = if search_pending {
            conn.query_row("SELECT COUNT(*) FROM envelopes", [], |row| row.get(0))?
        } else if table_columns(&conn, "mailboxes")?.contains("threaded") {
            conn.query_row(
                "SELECT COUNT(*) FROM envelopes e
                 JOIN mailboxes m ON m.id = e.mailbox_id
                 WHERE m.threaded = 1",
                [],
                |row| row.get(0),
            )?
        } else {
            conn.query_row("SELECT COUNT(*) FROM envelopes", [], |row| row.get(0))?
        };
        if messages == 0 {
            Ok(None)
        } else {
            Ok(Some(messages as u64))
        }
    }

    /// Lit une préférence SANS ouvrir la base — sonde en **lecture
    /// seule**, sœur de [`Store::pending_adoption`] : rien n'est
    /// déclenché, rien n'est créé. C'est ce qui permet au desktop de
    /// restaurer la langue AVANT l'écran de migration (ADR 0012) —
    /// avec l'ouverture pleine, l'adoption d'une base héritée se payait
    /// en silence au chargement de la langue, sans modale (constat
    /// terrain 2026-08-15).
    ///
    /// Limite assumée (revue du même jour) : après un arrêt brutal, un
    /// `-wal` chaud peut rendre l'ouverture en lecture seule impossible
    /// — la sonde échoue alors au lieu de récupérer le journal comme le
    /// ferait l'ouverture pleine. L'UI traite cet échec comme un repli
    /// de SESSION (langue du système), jamais comme une absence de
    /// préférence : rien ne se persiste sur la foi d'une sonde muette.
    pub fn text_pref_readonly(path: &Path, key: &str) -> Result<Option<String>, Error> {
        if !path.exists() {
            // Première installation : rien à lire, et ouvrir créerait
            // le fichier — une sonde ne laisse pas de trace.
            return Ok(None);
        }
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        // Le même budget d'attente que l'ouverture pleine : une base
        // héritée d'AVANT le WAL est en mode rollback, où un écrivain
        // bloque les lecteurs — sans ce budget, la sonde mourrait en
        // SQLITE_BUSY au premier essai (tard vaut mieux que mort).
        conn.busy_timeout(std::time::Duration::from_secs(30))?;
        // Une base d'avant les préférences peut ne pas avoir la table :
        // la sonde doit répondre (« pas de préférence »), pas expliquer.
        let has_prefs: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'prefs'",
            [],
            |row| row.get(0),
        )?;
        if has_prefs == 0 {
            return Ok(None);
        }
        let value = conn
            .query_row(
                "SELECT value FROM prefs WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value)
    }

    fn init(conn: Connection) -> Result<Self, Error> {
        Self::init_with(conn, &mut |_| ControlFlow::Continue(()))
    }

    /// Oublie l'initialisation d'UN chemin — pour les tests qui
    /// REMBOBINENT une base à la main entre deux ouvertures (le décor
    /// d'une base d'avant), ce que la mono-instance interdit en
    /// production. Un chemin, jamais tout le registre : les tests tournent
    /// en parallèle, et vider le registre sous les pieds d'un autre lui
    /// fait rejouer un schéma qu'il prouve justement ne pas rejouer.
    #[cfg(test)]
    pub(crate) fn oublier_initialisation(path: &Path) {
        // La MÊME clé que le registre : celle que SQLite donne au fichier.
        if let Some(cle) = Connection::open(path)
            .ok()
            .and_then(|conn| cle_fichier(&conn))
        {
            registre_initialisees().verrou().remove(&cle);
        }
    }

    fn init_with(
        conn: Connection,
        on_progress: &mut dyn FnMut(AdoptionProgress) -> ControlFlow<()>,
    ) -> Result<Self, Error> {
        // Plusieurs commandes ouvrent chacune leur connexion : patienter
        // plutôt que d'échouer en SQLITE_BUSY sur une écriture concurrente.
        // 30 s et non 5 (terrain 2026-08-15) : sous forte charge machine,
        // un lot d'écriture de la synchronisation peut tenir le verrou
        // au-delà de 5 s — un geste UI (`delete_draft` d'un brouillon
        // vidé) mourait alors en BUSY et son échec, tu par l'UI d'époque,
        // laissait un fantôme au dossier. En WAL les lectures ne
        // patientent jamais ; seule une écriture derrière une écriture
        // attend — tard vaut mieux que mort.
        conn.busy_timeout(std::time::Duration::from_secs(30))?;
        // WAL (ADR 0011) : une lecture ne bloque plus jamais une écriture,
        // ni l'inverse. Le mode rollback tenait tant que les écritures
        // duraient quelques secondes ; la synchronisation intégrale
        // (ADR 0010) les étire en minutes, et le PREMIER essai terrain a
        // produit « database is locked » — le sondage d'avancement et la
        // liste, en lisant, faisaient expirer le busy_timeout de la passe
        // d'en-têtes.
        //
        // `query_row` et non `pragma_update` : ce PRAGMA répond une ligne
        // (le mode effectif). Une base en mémoire répond « memory » — ce
        // n'est pas un échec, les tests y vivent très bien sans WAL. Le
        // mode est PERSISTANT : écrit une fois dans l'en-tête du fichier,
        // relu à chaque ouverture, bases héritées comprises.
        conn.query_row("PRAGMA journal_mode = wal", [], |row| {
            row.get::<_, String>(0)
        })?;
        // PLAN-AUDIT-V2 E1 — la porte rapide : chaque commande du shell
        // ouvre SA connexion (103 sites) ; rejouer ici le schéma, une
        // vingtaine de `table_xinfo` et les migrations coûtait 36 ms sur
        // 200 k enveloppes À CHAQUE commande. Une fois l'initialisation
        // complète RÉUSSIE sur un chemin dans ce processus, les ouvertures
        // suivantes ne font que les deux réglages ci-dessus. Sûr parce que
        // la mono-instance (PLAN-AUDIT-V1 E1) garantit qu'aucun autre
        // processus ne migre la base entre-temps, et que l'inscription
        // n'a lieu qu'après COMMIT de l'adoption (une annulation, un
        // échec : rien d'inscrit, la passe entière se rejoue). Une base
        // mémoire n'a pas de chemin : jamais inscrite.
        // Les clés étrangères sont un réglage PAR CONNEXION : `SCHEMA` les
        // active en tête, et la porte rapide ne rejoue pas `SCHEMA`. La
        // revue de la vague 2 y a vu des cascades perdues ; le test qui
        // devait le prouver est resté VERT sans cette ligne — rusqlite
        // `bundled` compile SQLite avec `SQLITE_DEFAULT_FOREIGN_KEYS=1`.
        // La ligne reste, avant la porte : une ceinture qui ne dépend
        // pas d'un drapeau de compilation (le test la garde).
        conn.execute_batch("PRAGMA foreign_keys = ON")?;
        conn.execute_batch("PRAGMA foreign_keys = ON")?;
        let cle = cle_fichier(&conn);
        if let Some(cle) = &cle
            && registre_initialisees().contains(cle)
        {
            return Ok(Self(conn));
        }
        conn.execute_batch(SCHEMA)?;
        // Les migrations légères d'abord : colonnes, index. La
        // reconstruction de l'index de recherche vit ICI mais n'est PAS
        // légère sur une base fournie (relecture des corps) : elle est
        // donc visible et interruptible via `on_progress`, et
        // `pending_adoption` la fait précéder d'un écran (sinon, gel muet
        // du démarrage — constat terrain 2026-08-17). L'adoption des fils,
        // juste dessous, a besoin des colonnes qu'ajoutent ces migrations
        // (`thread_id`, `in_reply_to`, `refs`).
        migrate(&conn, on_progress)?;
        // ——— L'unité des fils, d'un seul tenant (passation §8). ———
        // Du DROP conditionnel jusqu'à `user_version`, tout vit dans UNE
        // transaction : annuler pendant l'adoption rembobine TOUT — une
        // adoption partielle persistée serait une boîte à moitié vide,
        // la liste partant de `threads`. Le BEGIN est DEFERRED : sur une
        // base à jour rien n'écrit, la transaction reste lectrice et ne
        // rencontre jamais l'écrivain d'une synchro longue (ADR 0011).
        conn.execute_batch("BEGIN")?;
        let unit = (|| {
            // AVANT le schéma des fils, jamais après : si la règle de
            // regroupement a changé, les deux tables doivent DISPARAÎTRE
            // pour que le `CREATE TABLE IF NOT EXISTS` juste dessous les
            // recrée dans leur forme neuve. Sans cela l'ouverture échoue —
            // voir `thread::drop_if_outdated`.
            thread::drop_if_outdated(&conn)?;
            conn.execute_batch(thread::SCHEMA)?;
            thread::migrate_threads_with(&conn, on_progress)
        })();
        let announced = match unit {
            Ok(announced) => {
                conn.execute_batch("COMMIT")?;
                announced
            }
            Err(err) => {
                // L'échec du retour arrière n'apprendrait rien de plus que
                // l'erreur d'origine, qui est celle qu'il faut remonter —
                // l'annulation volontaire comprise.
                let _ = conn.execute_batch("ROLLBACK");
                return Err(err);
            }
        };
        if let Some(total) = announced {
            // « Fini » ne se dit qu'une fois la passe COMMISE — jamais
            // avant (un signal doit être observable, passation §9). Trop
            // tard pour annuler : la réponse est ignorée.
            let _ = on_progress(AdoptionProgress { done: total, total });
        }
        let store = Self(conn);
        // L'annuaire des correspondants se rattrape UNE fois sur
        // l'existant (PLAN-RETOURS-5) : set-based, marque en `prefs` —
        // sur une base à jour, un SELECT et rien d'autre.
        store.rattraper_correspondants()?;
        if let Some(cle) = cle {
            registre_initialisees().insert(cle);
        }
        Ok(store)
    }

    /// Préférence booléenne persistée en base. Absente = `default` : une
    /// préférence jamais touchée n'écrit rien — la base ne porte que les
    /// choix explicites.
    pub fn bool_pref(&self, key: &str, default: bool) -> Result<bool, Error> {
        let value: Option<String> = self
            .0
            .query_row(
                "SELECT value FROM prefs WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.map_or(default, |v| v == "1"))
    }

    pub fn set_bool_pref(&self, key: &str, value: bool) -> Result<(), Error> {
        self.0.execute(
            "INSERT INTO prefs (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, if value { "1" } else { "0" }],
        )?;
        Ok(())
    }

    /// Préférence texte persistée en base — le pendant de `bool_pref`
    /// pour les valeurs nommées (la langue de l'interface, PLAN-LANGUES).
    /// Absente = `None` : une préférence jamais touchée n'écrit rien,
    /// c'est l'appelant qui connaît son défaut.
    pub fn text_pref(&self, key: &str) -> Result<Option<String>, Error> {
        let value = self
            .0
            .query_row(
                "SELECT value FROM prefs WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value)
    }

    pub fn set_text_pref(&self, key: &str, value: &str) -> Result<(), Error> {
        self.0.execute(
            "INSERT INTO prefs (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// L'horizon d'import d'un compte (PLAN-HORIZON-NETTOYAGE D1/D3) —
    /// pref `horizon_import.{id}`, vocabulaire
    /// [`crate::backfill::HORIZONS_IMPORT`]. Sans pref, ou sur une valeur
    /// hors vocabulaire : « tout » (D4 — un compte d'avant le réglage, ou
    /// une pref corrompue, importe tout ; jamais une perte silencieuse).
    pub fn horizon_import(&self, account_id: i64) -> Result<String, Error> {
        Ok(self
            .text_pref(&format!("horizon_import.{account_id}"))?
            .filter(|v| crate::backfill::HORIZONS_IMPORT.contains(&v.as_str()))
            .unwrap_or_else(|| "tout".to_string()))
    }

    /// Pose l'horizon d'import — la porte valide le vocabulaire AVANT
    /// d'écrire (même règle que `valider_routage` : un vocabulaire troué
    /// ne se cache pas derrière un autre refus).
    pub fn set_horizon_import(&self, account_id: i64, valeur: &str) -> Result<(), Error> {
        if !crate::backfill::HORIZONS_IMPORT.contains(&valeur) {
            return Err(Error::Corrupt(format!("horizon inconnu : {valeur:?}")));
        }
        self.set_text_pref(&format!("horizon_import.{account_id}"), valeur)
    }

    /// Plusieurs préférences texte d'un COUP, transactionnelles : des
    /// clés qui n'ont de sens qu'ensemble (l'icône ET la teinte d'un
    /// repère de compte) ne doivent jamais se retrouver à moitié
    /// écrites — un échec entre les deux laisserait une paire que
    /// personne n'a choisie (revue PLAN-RETOURS-8, 2026-08-22).
    pub fn set_text_prefs(&mut self, prefs: &[(&str, &str)]) -> Result<(), Error> {
        let tx = self.0.transaction()?;
        for (key, value) in prefs {
            tx.execute(
                "INSERT INTO prefs (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn sync_state(&self, account_id: i64, mailbox: &str) -> Result<Option<SyncState>, Error> {
        let state = self
            .0
            .query_row(
                "SELECT id, uid_validity, last_uid, highest_modseq, initialisee
                 FROM mailboxes WHERE account_id = ?1 AND name = ?2",
                params![account_id, mailbox],
                |row| {
                    Ok(SyncState {
                        mailbox_id: row.get(0)?,
                        uid_validity: row.get(1)?,
                        last_uid: row.get(2)?,
                        highest_modseq: row.get::<_, Option<i64>>(3)?.map(|m| m as u64),
                        initialisee: row.get::<_, i64>(4)? != 0,
                    })
                },
            )
            .optional()?;
        Ok(state)
    }

    /// Enregistre une boite. Elle n'entre dans la portee du regroupement
    /// que si c'est la boite de reception : le dossier « Envoyes » y entre
    /// aussi, mais son NOM varie d'un serveur a l'autre (ADR 0009 §7), donc
    /// seul l'appelant qui l'a decouvert peut le declarer —
    /// [`Store::set_thread_scope`].
    ///
    /// Toutes les autres — Archive, Corbeille, Spam, dossiers utilisateur —
    /// sont stockees et indexees, jamais regroupees (ADR 0010 §3).
    pub fn create_mailbox(
        &self,
        account_id: i64,
        mailbox: &str,
        uid_validity: u32,
    ) -> Result<i64, Error> {
        // `COALESCE` : sans lui, un compte sans dossier d'envois connu
        // rendrait `?2 = NULL` — donc NULL — et `faux OR NULL` vaut NULL en
        // SQL. La colonne étant NOT NULL, l'insertion échouerait pour tout
        // dossier ordinaire d'un compte qui n'a pas encore découvert ses
        // envois. C'est-à-dire au tout premier passage.
        self.0.execute(
            "INSERT INTO mailboxes (account_id, name, uid_validity, threaded)
             VALUES (?1, ?2, ?3,
                     ?2 = ?4 OR COALESCE(
                         ?2 = (SELECT sent_mailbox FROM accounts WHERE id = ?1), 0))",
            params![account_id, mailbox, uid_validity, thread::RECEIVED_MAILBOX],
        )?;
        Ok(self.0.last_insert_rowid())
    }

    /// Les boites d'un compte, dans l'ordre ou le rattrapage doit les
    /// servir : la reception d'abord (c'est elle que la liste montre et
    /// que la recherche du quotidien interroge), les envois ensuite (ils
    /// completent les fils), le reste par nom — deterministe, donc
    /// reprenable d'une session a l'autre.
    ///
    /// Miroir HORS LIGNE de `sync_order` : meme priorite, mais la source
    /// est la base et non le serveur — le rattrapage ne doit pas payer un
    /// LIST pour savoir quoi pomper.
    pub fn mailbox_names(&self, account_id: i64) -> Result<Vec<String>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT name FROM mailboxes WHERE account_id = ?1
             ORDER BY (name = ?2) DESC,
                      (name = (SELECT sent_mailbox FROM accounts WHERE id = ?1)) DESC,
                      name",
        )?;
        let names = stmt
            .query_map(params![account_id, thread::RECEIVED_MAILBOX], |row| {
                row.get(0)
            })?
            .collect::<Result<_, _>>()?;
        Ok(names)
    }

    /// Combien de messages ce COMPTE possede en base, toutes boites
    /// confondues. C'est le « deja fait » que la garde d'espace disque
    /// soustrait de l'annonce des serveurs (ADR 0010 §4) : sans lui, une
    /// boite deja rapatriee aux trois quarts serait refusee comme si tout
    /// restait a telecharger.
    pub fn account_message_count(&self, account_id: i64) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1",
            [account_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Le UIDNEXT vu au relevé qui a précédé la dernière relève soldée
    /// (ADR 0017) — `None` tant qu'aucune relève gardée n'a eu lieu.
    pub fn remote_uidnext(&self, mailbox_id: i64) -> Result<Option<u32>, Error> {
        Ok(self.0.query_row(
            "SELECT remote_uidnext FROM mailboxes WHERE id = ?1",
            params![mailbox_id],
            |row| row.get(0),
        )?)
    }

    /// Pose le UIDNEXT vu — APRÈS que la relève a été soldée, jamais
    /// avant : un repère posé sur une relève interrompue ferait sauter
    /// un dossier pas encore rattrapé.
    pub fn set_remote_uidnext(&self, mailbox_id: i64, uidnext: u32) -> Result<(), Error> {
        self.0.execute(
            "UPDATE mailboxes SET remote_uidnext = ?2 WHERE id = ?1",
            params![mailbox_id, uidnext],
        )?;
        Ok(())
    }

    /// Messages en base pour CE dossier — le pendant local du MESSAGES
    /// du STATUS, comparés par `faut_relever` (ADR 0017).
    pub fn envelope_count(&self, mailbox_id: i64) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM envelopes WHERE mailbox_id = ?1",
            params![mailbox_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Des actions locales attendent-elles leur rejeu dans ce dossier ?
    /// EXISTS et non la liste : la question est fermée, la réponse aussi.
    pub fn has_pending_actions(&self, mailbox_id: i64) -> Result<bool, Error> {
        Ok(self.0.query_row(
            "SELECT EXISTS(SELECT 1 FROM pending_actions WHERE mailbox_id = ?1 AND refusee = 0)",
            params![mailbox_id],
            |row| row.get(0),
        )?)
    }

    /// Releve ce que le serveur annonce dans cette boite (EXISTS).
    pub fn record_remote_total(&self, mailbox_id: i64, exists: u32) -> Result<(), Error> {
        self.0.execute(
            "UPDATE mailboxes SET remote_total = ?2 WHERE id = ?1",
            params![mailbox_id, exists],
        )?;
        Ok(())
    }

    /// Avancement de la synchronisation, toutes boites et tous comptes
    /// confondus : (messages en base, messages annonces par les serveurs).
    ///
    /// Ne compte QUE les boites deja selectionnees au moins une fois
    /// (`remote_total > 0`). Sinon un compte dont la moitie des dossiers
    /// n'a pas encore ete visitee afficherait un avancement qui RECULE a
    /// mesure qu'on les decouvre — le denominateur grandissant plus vite
    /// que le numerateur. Un avancement qui recule est pire que pas
    /// d'avancement du tout.
    ///
    /// Le denominateur s'ajuste des DEPARTS EN ATTENTE de rejeu
    /// (archive, suppression, deplacement — `pending_actions`) : le
    /// geste retire la ligne locale immediatement (echo, E3) mais
    /// `remote_total` date du dernier SELECT — sans l'ajustement, un
    /// seul triage figeait l'avancement a 99 % (jamais 100 tant que
    /// local < remote) et le trait de la barre d'etat avec lui, pour
    /// toute la duree du rejeu (terrain 2026-08-15, PLAN-GELS). Les
    /// marquages (lu, etoile) ne retirent rien : ils ne touchent pas le
    /// denominateur. Borne a zero par boite : un `remote_total` en
    /// retard ne fait pas reculer les autres.
    pub fn sync_progress(&self) -> Result<(u64, u64), Error> {
        let (local, remote): (i64, i64) = self.0.query_row(
            "SELECT COALESCE(SUM(
                        (SELECT COUNT(*) FROM envelopes e WHERE e.mailbox_id = m.id)), 0),
                    COALESCE(SUM(MAX(0, m.remote_total -
                        (SELECT COUNT(*) FROM pending_actions p
                          WHERE p.mailbox_id = m.id AND p.refusee = 0
                            AND (p.kind IN ('archive', 'delete')
                                 OR p.kind LIKE 'move_to:%')))), 0)
             FROM mailboxes m WHERE m.remote_total > 0",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok((local as u64, remote as u64))
    }

    /// Declare la portee du regroupement d'un compte : la boite de
    /// reception, plus le dossier des envois quand le serveur en expose un.
    ///
    /// Le dossier des envois du compte, sous son nom RÉSEAU — `None`
    /// tant que la découverte des dossiers ne l'a pas mémorisé. C'est la
    /// cible de la relève ciblée d'après-envoi : la copie qu'ajoute le
    /// serveur d'envoi doit se voir sans attendre le cycle complet
    /// (terrain 0.1.4, 2026-08-14 : 4 minutes sans copie visible).
    pub fn sent_mailbox(&self, account_id: i64) -> Result<Option<String>, Error> {
        Ok(self
            .0
            .query_row(
                "SELECT sent_mailbox FROM accounts WHERE id = ?1",
                [account_id],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    /// Appele APRES la decouverte des dossiers, a chaque synchronisation :
    /// un serveur peut renommer son dossier d'envois, et un compte peut
    /// n'en avoir aucun — auquel cas les fils ne regroupent que les recus,
    /// exactement comme avant l'ADR 0009. Idempotent.
    pub fn set_thread_scope(&self, account_id: i64, sent: Option<&str>) -> Result<(), Error> {
        // E4 : compte et boîtes d'accord ou rien — une seule transaction.
        let tx = self.0.unchecked_transaction()?;
        // Mémorisé sur le compte d'ABORD : c'est cette mémoire que
        // `create_mailbox` consultera pour les boîtes que la boucle de
        // synchronisation n'a pas encore créées.
        self.0.execute(
            "UPDATE accounts SET sent_mailbox = ?2 WHERE id = ?1",
            params![account_id, sent],
        )?;
        self.0.execute(
            "UPDATE mailboxes SET threaded = (name = ?2 OR (?3 IS NOT NULL AND name = ?3))
             WHERE account_id = ?1",
            params![account_id, thread::RECEIVED_MAILBOX, sent],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Enregistre un compte, ou revendique le compte « en attente
    /// d'adoption » créé par la migration Phase 2 → 3 (email vide) : la
    /// première connexion après la mise à jour est, en pratique, le même
    /// compte Gmail qu'avant — ses données l'attendent.
    pub fn adopt_or_create_account(&self, email: &str, provider: &str) -> Result<i64, Error> {
        if let Some(id) = self.account_id(email)? {
            return Ok(id);
        }
        let claimed = self.0.execute(
            "UPDATE accounts SET email = ?1, provider = ?2
             WHERE email = '' AND id = (SELECT MIN(id) FROM accounts WHERE email = '')",
            params![email, provider],
        )?;
        if claimed == 0 {
            self.0.execute(
                "INSERT INTO accounts (email, provider) VALUES (?1, ?2)",
                params![email, provider],
            )?;
            return Ok(self.0.last_insert_rowid());
        }
        self.account_id(email)?
            .ok_or_else(|| Error::Corrupt("compte revendiqué introuvable".to_string()))
    }

    fn account_id(&self, email: &str) -> Result<Option<i64>, Error> {
        let id = self
            .0
            .query_row("SELECT id FROM accounts WHERE email = ?1", [email], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(id)
    }

    /// Les comptes connus — sans l'éventuel compte en attente d'adoption.
    pub fn accounts(&self) -> Result<Vec<Account>, Error> {
        let mut stmt = self
            .0
            .prepare("SELECT id, email, provider FROM accounts WHERE email != '' ORDER BY id")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Account {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    provider: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Configuration serveur d'un compte (Gmail ou IMAP générique).
    pub fn account_config(&self, account_id: i64) -> Result<AccountConfig, Error> {
        let config = self
            .0
            .query_row(
                "SELECT imap_host, imap_port, smtp_host, smtp_port, username
                 FROM accounts WHERE id = ?1",
                [account_id],
                |row| {
                    Ok(AccountConfig {
                        imap_host: row.get(0)?,
                        imap_port: row.get(1)?,
                        smtp_host: row.get(2)?,
                        smtp_port: row.get(3)?,
                        username: row.get(4)?,
                    })
                },
            )
            .optional()?
            .unwrap_or(AccountConfig {
                imap_host: None,
                imap_port: None,
                smtp_host: None,
                smtp_port: None,
                username: None,
            });
        Ok(config)
    }

    /// Crée ou met à jour un compte IMAP/SMTP générique.
    pub fn create_generic_account(
        &self,
        email: &str,
        username: &str,
        imap_host: &str,
        imap_port: u16,
        smtp_host: &str,
        smtp_port: u16,
    ) -> Result<i64, Error> {
        self.0.execute(
            "INSERT INTO accounts (email, provider, username, imap_host, imap_port, smtp_host, smtp_port)
             VALUES (?1, 'imap', ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(email) DO UPDATE SET
                provider = 'imap',
                username = excluded.username,
                imap_host = excluded.imap_host,
                imap_port = excluded.imap_port,
                smtp_host = excluded.smtp_host,
                smtp_port = excluded.smtp_port",
            params![
                email,
                username,
                imap_host,
                imap_port,
                smtp_host,
                smtp_port
            ],
        )?;
        // JAMAIS `last_insert_rowid()` : sur le chemin UPDATE (ré-ajout),
        // aucune ligne n'est insérée et il renverrait 0 (ou un id d'une
        // autre écriture de la connexion). L'id fait toujours foi en base.
        self.account_id(email)?.ok_or_else(|| {
            Error::Corrupt("compte générique introuvable après écriture".to_string())
        })
    }

    /// Supprime un compte et TOUT ce qui s'y rattache, en une transaction.
    ///
    /// Les préfixes des prefs suffixées par compte vivent dans
    /// [`PREFS_PAR_COMPTE`] — l'auteur d'une pref neuve l'ajoute LÀ.
    ///
    /// Les cascades du schéma emportent boîtes, enveloppes, corps, pièces
    /// jointes, actions en attente, dossiers et fils. Trois familles n'ont
    /// PAS de clé étrangère et se vident à la main : l'index de recherche
    /// (boîte par boîte, AVANT que la cascade ne fasse disparaître les
    /// boîtes), les brouillons (avec pierres tombales et repère distant)
    /// et la boîte d'envoi. Rien ne doit survivre au compte — un reste
    /// orphelin ne serait jamais relu, mais continuerait de sortir en
    /// recherche ou de partir à la prochaine vidange.
    pub fn delete_account(&mut self, account_id: i64) -> Result<(), Error> {
        let tx = self.0.transaction()?;
        let mailboxes: Vec<i64> = {
            let mut stmt = tx.prepare("SELECT id FROM mailboxes WHERE account_id = ?1")?;
            stmt.query_map([account_id], |row| row.get(0))?
                .collect::<Result<Vec<i64>, _>>()?
        };
        for mailbox_id in mailboxes {
            search::deindex_mailbox(&tx, mailbox_id)?;
        }
        tx.execute("DELETE FROM drafts WHERE account_id = ?1", [account_id])?;
        tx.execute(
            "DELETE FROM draft_tombstones WHERE account_id = ?1",
            [account_id],
        )?;
        tx.execute(
            "DELETE FROM drafts_remote WHERE account_id = ?1",
            [account_id],
        )?;
        tx.execute("DELETE FROM outbox WHERE account_id = ?1", [account_id])?;
        // Les préférences suffixées par l'id (signature, repère de
        // compte) meurent avec lui : `accounts.id` est un INTEGER
        // PRIMARY KEY sans AUTOINCREMENT — SQLite réutilise le plus
        // grand rowid libéré, et un compte ajouté après le retrait
        // hériterait sinon de l'identité de l'ancien (revue
        // PLAN-RETOURS-8, 2026-08-22).
        for prefixe in PREFS_PAR_COMPTE {
            tx.execute(
                "DELETE FROM prefs WHERE key = ?1",
                [format!("{prefixe}.{account_id}")],
            )?;
        }
        tx.execute("DELETE FROM accounts WHERE id = ?1", [account_id])?;
        // L'attente du Portier suit le courrier (E2) : les rangs que
        // les cascades viennent de vider meurent avec le compte. Le
        // routage, lui, est GLOBAL au poste et survit (patron
        // `images_expediteurs`).
        purger_attente_orpheline(&tx)?;
        tx.commit()?;
        Ok(())
    }

    /// Repart de zéro pour une boîte dont l'UIDVALIDITY a changé : les UIDs
    /// ne veulent plus rien dire — corps et actions en attente compris (une
    /// intention sur un UID invalidé est irréalisable par construction).
    pub fn reset_mailbox(&self, mailbox_id: i64, uid_validity: u32) -> Result<(), Error> {
        // PLAN-AUDIT-V1 E4 : UNE transaction — neuf écritures en
        // autocommit laissaient, sur un crash entre deux, des fils sans
        // enveloppes (la « pastille devant une liste vide »). Prouvé par
        // un déclencheur qui refuse la suppression des enveloppes.
        let tx = self.0.unchecked_transaction()?;
        search::deindex_mailbox(&self.0, mailbox_id)?;
        // Les actions en attente : une intention sur un UID invalidé est
        // irréalisable par construction.
        self.0.execute(
            "DELETE FROM pending_actions WHERE mailbox_id = ?1",
            [mailbox_id],
        )?;
        // Les tables par message suivent TOUTES (revue PLAN-INVITATIONS,
        // R1 RETOURS-11, E5, R10 RETOURS-13) : après un changement
        // d'UIDVALIDITY les UIDs ne veulent plus rien dire — une
        // invitation, un accord d'images, une mise de côté ou un « lu » qui
        // survivraient colleraient au message qui recycle l'UID. LA liste
        // est `TABLES_PAR_MESSAGE` (revue PLAN-AUDIT-V1 : plus de copie).
        for table in TABLES_PAR_MESSAGE {
            self.0.execute(
                &format!("DELETE FROM {table} WHERE mailbox_id = ?1"),
                [mailbox_id],
            )?;
        }
        // L'attente du Portier est DÉRIVÉE du courrier (E2) : un rang
        // qui ne s'appuie plus sur rien meurt avec la boîte — un UID
        // recyclé n'hérite d'aucune attente (A43/A89).
        purger_attente_orpheline(&self.0)?;
        self.0.execute(
            "UPDATE mailboxes
             SET uid_validity = ?2, last_uid = 0, highest_modseq = NULL
             WHERE id = ?1",
            params![mailbox_id, uid_validity],
        )?;
        // APRÈS la suppression des enveloppes, jamais avant : un fil se
        // recalcule à partir de ce qui reste. Le refaire d'abord le
        // ferait pointer sur des messages qu'on s'apprête à effacer.
        //
        // Et sur le COMPTE, pas la boîte : depuis l'ADR 0009 un fil peut
        // réunir INBOX et « Envoyés », donc réinitialiser l'une oblige à
        // reconsidérer les deux.
        let account_id: i64 = self.0.query_row(
            "SELECT account_id FROM mailboxes WHERE id = ?1",
            [mailbox_id],
            |row| row.get(0),
        )?;
        thread::rebuild_account(&self.0, account_id)?;
        tx.commit()?;
        Ok(())
    }

    pub fn update_state(
        &self,
        mailbox_id: i64,
        last_uid: Uid,
        highest_modseq: Option<u64>,
    ) -> Result<(), Error> {
        // `initialisee = 1` : une passe qui s'est soldée — la prochaine
        // est incrémentale quoi qu'il reste en base (E2).
        self.0.execute(
            "UPDATE mailboxes SET last_uid = ?2, highest_modseq = ?3, initialisee = 1,
                                  relevee_epoch = unixepoch()
             WHERE id = ?1",
            params![mailbox_id, last_uid, highest_modseq.map(|m| m as i64)],
        )?;
        Ok(())
    }

    pub fn upsert_envelopes(
        &mut self,
        mailbox_id: i64,
        envelopes: &[Envelope],
    ) -> Result<(), Error> {
        // Ce que l'annuaire des correspondants apprend de CETTE boîte —
        // résolu une fois par lot, comme le fil (PLAN-RETOURS-5, D4).
        let (noter_expediteurs, noter_destinataires) = self.role_annuaire(mailbox_id)?;
        // L'époque du Portier (E2) — lue AVANT la transaction (une
        // pref, jamais réécrite pendant un lot). None = le mode n'a
        // jamais été activé, la décision d'arrivée ne coûte rien.
        let epoque_portier = self.mode_organise_epoch()?;
        // Les règles du Non (E3, D2) : elles ne jouent que le mode
        // ACTIF — désactivé, elles DORMENT (le verdict reste posé).
        let regles_actives = self.mode_organise()?;
        // Résolu UNE fois : la boîte ne change pas dans un lot, et le fil
        // se raisonne désormais au compte (ADR 0009). Le faire par message
        // ajouterait une requête par enveloppe sur le chemin le plus chaud
        // de la synchronisation.
        // Même raison pour la portée : elle est propre à la boîte, pas au
        // message. Hors portée, on stocke et on indexe sans regrouper —
        // `thread_id` reste NULL (ADR 0010 §3).
        let (account_id, threaded, nom_boite): (i64, bool, String) = self.0.query_row(
            "SELECT account_id, threaded, name FROM mailboxes WHERE id = ?1",
            [mailbox_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        // Le dossier indésirable du compte, résolu AVANT la transaction
        // (la règle `spam` en a besoin) ; None = pas de dossier reconnu,
        // la règle spam ne fait RIEN — jamais une destination inventée.
        // Le message dégrade alors en « Non nu » : caché du mode
        // organisé (drapeau), jamais déplacé — limite dite au PLAN.
        let indesirables = if regles_actives {
            self.canonical_folders(account_id)?.indesirables
        } else {
            None
        };
        // Les retraits locaux décidés pendant le lot — appliqués APRÈS
        // le commit (`remove_local` recalcule fil et index dans sa
        // propre transaction). L'ACTION, elle, est journalisée DANS la
        // transaction du lot (revue E3) : un crash entre les deux ne
        // perd rien — l'intention est en base, le rejeu l'applique au
        // serveur et la copie locale part à la réconciliation suivante.
        let mut retraits_du_non: Vec<Uid> = Vec::new();
        let tx = self.0.transaction()?;
        // Le Portier ne juge que les ARRIVÉES (E2, D3) : la boîte du
        // courrier entrant, comme `inbox_size`. Un expéditeur vu
        // d'abord aux Indésirables ou en Archives n'attend pas au
        // guichet — mais son courrier, où qu'il soit, compte comme
        // « connu avant l'époque ».
        let arrivee = nom_boite == thread::RECEIVED_MAILBOX;
        // Les adresses dont l'attente s'est DÉFAITE dans ce lot (leur
        // courrier ancien arrive après coup) : leurs fils des lots
        // précédents recalculent leur drapeau après la boucle.
        let mut attente_defaite: BTreeSet<String> = BTreeSet::new();
        // Un ENSEMBLE, pas une liste : même défaut quadratique que celui
        // mesuré dans l'adoption (`Vec::contains` est linéaire). Borné ici
        // par la taille du lot, donc moins spectaculaire — mais c'est le
        // même chemin chaud, et la même correction.
        let mut touched: BTreeSet<i64> = BTreeSet::new();
        {
            // `INSERT OR REPLACE` remettrait à NULL toute colonne absente
            // de la liste — et `refs` comme `thread_id` sont écrits par
            // d'AUTRES chemins que la synchro. Une re-synchronisation
            // effacerait donc silencieusement le travail de rattrapage des
            // en-têtes, exactement comme elle aurait effacé les pièces
            // jointes. On énumère donc les colonnes que la synchro possède,
            // et elles seules.
            let mut stmt = tx.prepare(
                "INSERT INTO envelopes
                 (mailbox_id, uid, subject, sender, sender_address, message_id,
                  in_reply_to, date_epoch, seen, flagged, to_addrs, cc_addrs, reply_to)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT (mailbox_id, uid) DO UPDATE SET
                     subject = excluded.subject,
                     sender = excluded.sender,
                     sender_address = excluded.sender_address,
                     message_id = excluded.message_id,
                     in_reply_to = excluded.in_reply_to,
                     date_epoch = excluded.date_epoch,
                     seen = excluded.seen,
                     flagged = excluded.flagged,
                     to_addrs = excluded.to_addrs,
                     cc_addrs = excluded.cc_addrs,
                     reply_to = excluded.reply_to",
            )?;
            let mut body_stmt =
                tx.prepare("SELECT html FROM bodies WHERE mailbox_id = ?1 AND uid = ?2")?;
            let mut refs_stmt =
                tx.prepare("SELECT refs FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2")?;
            let mut deja_stmt = tx.prepare(
                "SELECT subject, sender, sender_address, to_addrs, cc_addrs
                 FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
            )?;
            for envelope in envelopes {
                // L'annuaire n'apprend que des messages NEUFS : une
                // re-synchronisation (drapeaux CONDSTORE, re-relève) ne
                // gonfle pas la fréquence d'un correspondant. La même
                // lecture dit si les champs INDEXÉS ont changé : sinon,
                // l'index ne bouge pas (PLAN-AUDIT-V2 E2 — avant, chaque
                // enveloppe relue faisait relire et re-tokeniser son corps
                // sous le verrou d'écriture).
                let to_field = join_addrs(&envelope.to_addrs);
                let cc_field = join_addrs(&envelope.cc_addrs);
                let deja: Option<ChampsIndexes> = deja_stmt
                    .query_row(params![mailbox_id, envelope.uid], |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    })
                    .optional()?;
                let nouveau = deja.is_none();
                // Comparaison par référence (revue) : cinq clones par
                // enveloppe relue, c'était 25 000 allocations pour rien
                // sur 5 000 deltas CONDSTORE.
                let a_reindexer = deja.as_ref().is_none_or(|deja| {
                    deja.0.as_deref() != envelope.subject.as_deref()
                        || deja.1.as_deref() != envelope.sender.as_deref()
                        || deja.2.as_deref() != envelope.sender_address.as_deref()
                        || deja.3.as_deref() != to_field.as_deref()
                        || deja.4.as_deref() != cc_field.as_deref()
                });
                stmt.execute(params![
                    mailbox_id,
                    envelope.uid,
                    envelope.subject,
                    envelope.sender,
                    envelope.sender_address,
                    envelope.message_id,
                    envelope.in_reply_to,
                    envelope.date.map(|d| d.timestamp()),
                    envelope.seen,
                    envelope.flagged,
                    join_addrs(&envelope.to_addrs),
                    join_addrs(&envelope.cc_addrs),
                    envelope.reply_to,
                ])?;

                // La décision d'arrivée du Portier (E2) — sondes par
                // clés, toutes cachées, 7 µs/message mesurées
                // (S2-bis) : un inconnu (aucune ligne de routage, aucun
                // courrier avant l'époque, jamais soi) entre en attente
                // à son premier message d'arrivée postérieur à
                // l'époque ; le courrier ANCIEN qui arrive après coup
                // (désordre de synchro) prouve le connu et DÉFAIT
                // l'attente posée à tort. Un message SANS date ne
                // prouve JAMAIS le connu (revue E2) : le spam sans
                // en-tête Date contournerait sinon le guichet même —
                // il est traité comme une arrivée d'aujourd'hui.
                // La règle du Non (E3) : un message qui ARRIVE d'un
                // expéditeur écarté avec règle, POSTÉRIEUR au verdict
                // (« ses prochains messages » — un backfill d'historique
                // n'archive ni ne jette jamais ; sans date = arrivée
                // d'aujourd'hui ; limite DITE : un en-tête Date falsifié
                // antérieur au verdict esquive la règle — le message
                // reste caché du mode organisé par le drapeau, c'est le
                // serveur qu'il n'atteint pas). L'action est journalisée
                // ICI, dans la transaction du lot (revue E3 — jamais une
                // fenêtre de crash entre le commit et l'intention) ;
                // `corbeille` → Delete, la corbeille du serveur, JAMAIS
                // une suppression définitive (D4). La garde anti-doublon
                // couvre la re-livraison (le retrait local fait reculer
                // `max_uid`, un rejeu en échec re-présentait le message —
                // une seconde action identique coincerait la file).
                if nouveau
                    && arrivee
                    && regles_actives
                    && let Some(adresse) = adresse_images(envelope.sender_address.clone())
                    && let Some((regle, verdict)) = tx
                        .prepare_cached(
                            "SELECT regle, epoch FROM routage_expediteurs
                              WHERE address = ?1 AND destination = 'ecarte'
                                AND regle IS NOT NULL",
                        )?
                        .query_row(params![adresse], |row| {
                            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                        })
                        .optional()?
                    && envelope
                        .date
                        .map(|d| d.timestamp())
                        .is_none_or(|date| date > verdict)
                {
                    let action = match regle.as_str() {
                        "archive" => Some(Action::Archive),
                        "corbeille" => Some(Action::Delete),
                        "spam" => indesirables.clone().map(Action::MoveTo),
                        _ => None,
                    };
                    if let Some(action) = action {
                        let deja_en_file = tx
                            .prepare_cached(
                                "SELECT 1 FROM pending_actions
                                  WHERE mailbox_id = ?1 AND uid = ?2 AND refusee = 0",
                            )?
                            .exists(params![mailbox_id, envelope.uid])?;
                        if !deja_en_file {
                            tx.prepare_cached(
                                "INSERT INTO pending_actions (mailbox_id, uid, kind)
                                 VALUES (?1, ?2, ?3)",
                            )?
                            .execute(params![
                                mailbox_id,
                                envelope.uid,
                                action.to_kind()
                            ])?;
                        }
                        retraits_du_non.push(envelope.uid);
                    }
                }

                if nouveau
                    && let Some(epoque) = epoque_portier
                    && let Some(adresse) = adresse_images(envelope.sender_address.clone())
                {
                    let date = envelope.date.map(|d| d.timestamp());
                    if let Some(date) = date
                        && date <= epoque
                    {
                        if tx
                            .prepare_cached("DELETE FROM portier_attente WHERE address = ?1")?
                            .execute(params![adresse])?
                            > 0
                        {
                            attente_defaite.insert(adresse);
                        }
                    } else if arrivee {
                        let deja = tx
                            .prepare_cached("SELECT 1 FROM portier_attente WHERE address = ?1")?
                            .exists(params![adresse])?
                            || tx
                                .prepare_cached(
                                    "SELECT 1 FROM routage_expediteurs WHERE address = ?1",
                                )?
                                .exists(params![adresse])?
                            || adresse_d_un_compte(&tx, &adresse)?
                            || connu_avant_epoque(&tx, &adresse, epoque)?;
                        if !deja {
                            tx.prepare_cached("INSERT INTO portier_attente (address) VALUES (?1)")?
                                .execute(params![adresse])?;
                        }
                    }
                }

                if nouveau {
                    let date = envelope.date.map(|d| d.timestamp()).unwrap_or(0);
                    if noter_expediteurs && let Some(adresse) = envelope.sender_address.as_deref() {
                        crate::correspondants::noter(
                            &tx,
                            adresse,
                            envelope.sender.as_deref(),
                            date,
                        )?;
                    }
                    if noter_destinataires {
                        for adresse in envelope.to_addrs.iter().chain(envelope.cc_addrs.iter()) {
                            crate::correspondants::noter(&tx, adresse, None, date)?;
                        }
                    }
                }

                // Les `References` déjà acquises comptent dans le
                // rattachement : une re-synchronisation ne doit pas
                // dégrouper un fil que la passe d'en-têtes avait recollé.
                let references: Option<String> = refs_stmt
                    .query_row(params![mailbox_id, envelope.uid], |row| row.get(0))
                    .optional()?
                    .flatten();
                if threaded {
                    let thread = thread::attach(
                        &tx,
                        account_id,
                        envelope.message_id.as_deref(),
                        envelope.in_reply_to.as_deref(),
                        references.as_deref(),
                        &adresses_de(envelope),
                    )?;
                    tx.execute(
                        "UPDATE envelopes SET thread_id = ?3 WHERE mailbox_id = ?1 AND uid = ?2",
                        params![mailbox_id, envelope.uid, thread],
                    )?;
                    touched.insert(thread);
                }

                if a_reindexer {
                    let html: Option<String> = body_stmt
                        .query_row(params![mailbox_id, envelope.uid], |row| row.get(0))
                        .optional()?;
                    search::index_message(
                        &tx,
                        mailbox_id,
                        envelope.uid,
                        search::Indexed {
                            subject: envelope.subject.as_deref(),
                            sender: envelope.sender.as_deref(),
                            sender_address: envelope.sender_address.as_deref(),
                            to_addrs: to_field.as_deref(),
                            cc_addrs: cc_field.as_deref(),
                            body_html: html.as_deref(),
                        },
                    )?;
                }
            }
            // Les fils des lots PRÉCÉDENTS d'une attente défaite : leur
            // drapeau de rétention date d'un état démenti — ils entrent
            // dans la même passe de recalcul.
            for adresse in &attente_defaite {
                touched.extend(fils_de(&tx, adresse)?);
            }
            // Après la boucle, et une seule fois par fil : recalculer à
            // chaque message ferait N fois le travail sur une conversation
            // de N messages arrivant dans le même lot.
            for thread in &touched {
                thread::refresh(&tx, *thread)?;
            }
        }
        tx.commit()?;
        // Le retrait local des messages traités (E3) — SANS écho (pas
        // un geste utilisateur ; l'historique du Portier dit déjà la
        // règle). L'action, elle, est DÉJÀ commise avec le lot : un
        // échec ici laisse la copie locale, que la réconciliation
        // serveur emportera après le rejeu — jamais un message qui
        // échappe à sa règle.
        if !retraits_du_non.is_empty() {
            // E4 : en UNE transaction (le patron de `nettoyage_verdict`) —
            // un retrait par autocommit payait huit fsync par message.
            let tx = self.0.unchecked_transaction()?;
            let mut fils: BTreeSet<i64> = BTreeSet::new();
            for uid in retraits_du_non {
                if let Some(thread) = purger_message(&tx, mailbox_id, uid)? {
                    fils.insert(thread);
                }
            }
            for thread in &fils {
                thread::refresh(&tx, *thread)?;
            }
            tx.commit()?;
        }
        Ok(())
    }

    /// Enregistre les en-têtes de fil lus dans le bloc d'en-têtes complet,
    /// et recolle le fil s'il y a lieu.
    ///
    /// `references` vaut `""` quand le message n'en porte pas : c'est la
    /// marque « déjà lu, rien à y trouver ». Écrire NULL le ferait
    /// redemander à chaque passage, indéfiniment.
    ///
    /// Retourne `true` si le rattachement a changé — l'appelant sait alors
    /// que la liste affichée n'est plus à jour.
    pub fn set_thread_headers(
        &mut self,
        mailbox_id: i64,
        uid: Uid,
        in_reply_to: Option<&str>,
        references: &str,
    ) -> Result<bool, Error> {
        let tx = self.0.transaction()?;
        let before: Option<i64> = tx
            .query_row(
                "SELECT thread_id FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let context: Option<(Option<String>, Option<String>, Vec<String>)> = tx
            .query_row(
                "SELECT message_id, in_reply_to, sender_address, to_addrs, cc_addrs
                 FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| {
                    let mut adresses: Vec<String> = Vec::new();
                    adresses.extend(row.get::<_, Option<String>>(2)?);
                    adresses.extend(split_addrs(row.get(3)?));
                    adresses.extend(split_addrs(row.get(4)?));
                    Ok((row.get(0)?, row.get(1)?, adresses))
                },
            )
            .optional()?;
        let Some((message_id, known_parent, adresses)) = context else {
            // Le message a disparu entre la lecture des en-têtes et leur
            // écriture (archivé, supprimé) : il n'y a plus rien à rattacher.
            return Ok(false);
        };

        // `COALESCE` : le bloc d'en-têtes fait autorité quand il dit
        // quelque chose, mais un `In-Reply-To` déjà donné par l'ENVELOPE ne
        // doit pas être effacé par une lecture qui n'en trouve pas.
        tx.execute(
            "UPDATE envelopes SET refs = ?3, in_reply_to = COALESCE(?4, in_reply_to)
             WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid, references, in_reply_to],
        )?;
        let parent = in_reply_to.map(str::to_string).or(known_parent);
        // Le COMPTE, pas la boîte (ADR 0009). Les deux sont des `i64` :
        // le compilateur ne peut pas distinguer l'un de l'autre, et se
        // tromper ici ne casserait rien — cela rattacherait simplement les
        // messages au mauvais espace de fils, en silence.
        let (account_id, threaded): (i64, bool) = tx.query_row(
            "SELECT account_id, threaded FROM mailboxes WHERE id = ?1",
            [mailbox_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        // Hors portée, les en-têtes sont conservés — ils servent la
        // recherche, et ils resserviront si la boîte entre un jour dans la
        // portée — mais ils ne rattachent rien (ADR 0010 §3).
        if !threaded {
            tx.commit()?;
            return Ok(false);
        }
        let thread = thread::attach(
            &tx,
            account_id,
            message_id.as_deref(),
            parent.as_deref(),
            Some(references),
            &adresses,
        )?;
        tx.execute(
            "UPDATE envelopes SET thread_id = ?3 WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid, thread],
        )?;
        thread::refresh(&tx, thread)?;
        if let Some(previous) = before.filter(|previous| *previous != thread) {
            thread::refresh(&tx, previous)?;
        }
        tx.commit()?;
        Ok(before != Some(thread))
    }

    /// Supprime les enveloppes absentes du serveur ; retourne leur nombre.
    /// Les UID qu'une boîte porte déjà en base — ce qu'une synchro
    /// initiale reprise ne redemande pas (PLAN-AUDIT-V2 E5).
    pub fn uids_connus(&self, mailbox_id: i64) -> Result<HashSet<Uid>, Error> {
        Ok(self
            .0
            .prepare_cached("SELECT uid FROM envelopes WHERE mailbox_id = ?1")?
            .query_map([mailbox_id], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    }

    pub fn remove_absent(
        &mut self,
        mailbox_id: i64,
        present: &HashSet<Uid>,
    ) -> Result<usize, Error> {
        let stale: Vec<Uid> = self
            .uids_connus(mailbox_id)?
            .into_iter()
            .filter(|uid| !present.contains(uid))
            .collect();
        let tx = self.0.transaction()?;
        {
            // E4 : LA liste des tables par message (`purger_message`) —
            // avant, trois tables sur sept : pièces, invitation, mémoire
            // d'images, mise de côté et « lu » du Kiosque restaient
            // orphelins (aucune clé étrangère sur `envelopes`). Un message
            // parti du serveur emporte aussi ses actions en attente : une
            // intention sur un UID disparu est irréalisable.
            let mut actions =
                tx.prepare("DELETE FROM pending_actions WHERE mailbox_id = ?1 AND uid = ?2")?;
            let mut touched: BTreeSet<i64> = BTreeSet::new();
            for uid in &stale {
                if let Some(thread) = purger_message(&tx, mailbox_id, *uid)? {
                    touched.insert(thread);
                }
                actions.execute(params![mailbox_id, uid])?;
            }
            // UNE fois par fil touché, jamais par message.
            for thread in &touched {
                thread::refresh(&tx, *thread)?;
            }
        }
        tx.commit()?;
        Ok(stale.len())
    }

    /// Retire localement une enveloppe et son corps (archivage/suppression
    /// optimiste) ; le serveur suivra via la file d'actions — les actions
    /// en attente ne sont PAS touchées, c'est elles qui portent le geste.
    ///
    /// Atomique (E4) : dans la transaction de l'appelant s'il en a une
    /// (`geste_avec_echo`, `nettoyage_verdict`, `upsert_envelopes`),
    /// sinon dans la sienne — jamais huit écritures en autocommit.
    pub fn remove_local(&self, mailbox_id: i64, uid: Uid) -> Result<(), Error> {
        if self.0.is_autocommit() {
            let tx = self.0.unchecked_transaction()?;
            if let Some(thread) = purger_message(&tx, mailbox_id, uid)? {
                thread::refresh(&tx, thread)?;
            }
            tx.commit()?;
            Ok(())
        } else {
            if let Some(thread) = purger_message(&self.0, mailbox_id, uid)? {
                thread::refresh(&self.0, thread)?;
            }
            Ok(())
        }
    }

    /// Applique localement un changement lu/non-lu (optimisme UI).
    /// Retourne `false` si l'enveloppe était déjà dans cet état.
    pub fn set_seen_local(&self, mailbox_id: i64, uid: Uid, seen: bool) -> Result<bool, Error> {
        let changed = self.0.execute(
            "UPDATE envelopes SET seen = ?3
             WHERE mailbox_id = ?1 AND uid = ?2 AND seen != ?3",
            params![mailbox_id, uid, seen],
        )?;
        if changed > 0 {
            // Le compteur de non-lus du fil vient de bouger. L'oublier
            // laisserait une conversation en gras alors qu'on vient de
            // lire son dernier message non lu.
            if let Some(thread) = thread::thread_of(&self.0, mailbox_id, uid)? {
                thread::refresh(&self.0, thread)?;
            }
        }
        Ok(changed > 0)
    }

    /// Applique localement l'étoile (optimisme UI).
    /// Retourne `false` si l'enveloppe était déjà dans cet état.
    pub fn set_flagged_local(
        &self,
        mailbox_id: i64,
        uid: Uid,
        flagged: bool,
    ) -> Result<bool, Error> {
        let changed = self.0.execute(
            "UPDATE envelopes SET flagged = ?3
             WHERE mailbox_id = ?1 AND uid = ?2 AND flagged != ?3",
            params![mailbox_id, uid, flagged],
        )?;
        Ok(changed > 0)
    }

    /// Journalise une intention à rejouer vers le serveur. Un geste neuf
    /// sur un message REMPLACE ses refusées (revue PLAN-AUDIT-V1) : sans
    /// cela, une action en quarantaine y restait pour toujours et la
    /// ligne de la fente ne pouvait que croître.
    pub fn enqueue_action(&self, mailbox_id: i64, uid: Uid, action: Action) -> Result<(), Error> {
        oublier_les_refusees(&self.0, mailbox_id, uid)?;
        self.0.execute(
            "INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (?1, ?2, ?3)",
            params![mailbox_id, uid, action.to_kind()],
        )?;
        Ok(())
    }

    /// La file ACTIVE d'actions, dans l'ordre d'émission — les refusées
    /// (quarantaine, E3) n'y sont plus. Une ligne au `kind` illisible
    /// (version future, corruption) est mise en quarantaine avec son
    /// motif, jamais fatale : avant E3 elle faisait échouer TOUTE la file.
    pub fn pending_actions(&self, mailbox_id: i64) -> Result<Vec<PendingAction>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT id, uid, kind FROM pending_actions
              WHERE mailbox_id = ?1 AND refusee = 0 ORDER BY id",
        )?;
        let rows = stmt
            .query_map([mailbox_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get(1)?, row.get::<_, String>(2)?))
            })?
            .collect::<Result<Vec<(i64, Uid, String)>, _>>()?;
        let mut file = Vec::with_capacity(rows.len());
        for (id, uid, kind) in rows {
            match Action::parse(&kind) {
                Some(action) => file.push(PendingAction { id, uid, action }),
                None => self.refuser_action(id, &format!("action illisible : {kind}"))?,
            }
        }
        Ok(file)
    }

    /// Échecs transitoires consécutifs au-delà desquels une action entre
    /// en quarantaine (D2 : cinq cycles).
    pub const SEUIL_QUARANTAINE: i64 = 5;

    /// Un échec TRANSITOIRE de plus sur cette action. Rend `true` quand
    /// le seuil est atteint : l'action vient d'entrer en quarantaine.
    pub fn noter_echec_action(&self, action_id: i64, erreur: &str) -> Result<bool, Error> {
        let refusee: i64 = self.0.query_row(
            "UPDATE pending_actions
                SET attempts = attempts + 1,
                    last_error = ?2,
                    refusee = CASE WHEN attempts + 1 >= ?3 THEN 1 ELSE refusee END
              WHERE id = ?1
              RETURNING refusee",
            params![action_id, erreur, Self::SEUIL_QUARANTAINE],
            |row| row.get(0),
        )?;
        Ok(refusee != 0)
    }

    /// Refus DÉFINITIF : quarantaine immédiate, avec le motif.
    pub fn refuser_action(&self, action_id: i64, erreur: &str) -> Result<(), Error> {
        self.0.execute(
            "UPDATE pending_actions SET refusee = 1, last_error = ?2 WHERE id = ?1",
            params![action_id, erreur],
        )?;
        Ok(())
    }

    /// Combien d'actions sont en quarantaine, tous comptes confondus —
    /// la ligne de la fente d'avis (D2).
    pub fn actions_refusees(&self) -> Result<u64, Error> {
        let n: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM pending_actions WHERE refusee = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(u64::try_from(n).unwrap_or(0))
    }

    pub fn remove_action(&self, action_id: i64) -> Result<(), Error> {
        self.0
            .execute("DELETE FROM pending_actions WHERE id = ?1", [action_id])?;
        Ok(())
    }

    /// Corps HTML brut (pré-assainissement) d'un message, s'il est en cache.
    pub fn body(&self, account_id: i64, mailbox: &str, uid: Uid) -> Result<Option<String>, Error> {
        let body = self
            .0
            .query_row(
                "SELECT b.html FROM bodies b JOIN mailboxes m ON m.id = b.mailbox_id
                 WHERE m.account_id = ?1 AND m.name = ?2 AND b.uid = ?3",
                params![account_id, mailbox, uid],
                |row| row.get(0),
            )
            .optional()?;
        Ok(body)
    }

    /// Enregistre un corps, son index de recherche et la description de
    /// ses pièces jointes — **dans une seule transaction**.
    ///
    /// Les trois se lisent dans les mêmes octets et n'ont de sens
    /// qu'ensemble : un corps sans son index sortirait des recherches, un
    /// corps sans ses pièces jointes les rendrait invisibles jusqu'au
    /// prochain re-téléchargement. Un crash entre deux écritures ne doit
    /// jamais pouvoir produire cet état.
    pub fn save_body(
        &self,
        mailbox_id: i64,
        uid: Uid,
        html: &str,
        attachments: &[Attachment],
    ) -> Result<(), Error> {
        self.save_body_full(mailbox_id, uid, html, attachments, None)
    }

    /// [`Store::save_body`], plus la ligne d'invitation quand la partie
    /// `text/calendar` accompagnait le corps (PLAN-INVITATIONS). Même
    /// transaction que le corps, remplacement intégral comme les pièces :
    /// un re-scan sans partie calendrier efface la ligne.
    pub fn save_body_full(
        &self,
        mailbox_id: i64,
        uid: Uid,
        html: &str,
        attachments: &[Attachment],
        invitation: Option<&InvitationRow>,
    ) -> Result<(), Error> {
        // Même règle que le rattrapage des aperçus : le parsing HTML se
        // paie AVANT d'ouvrir la transaction — jamais de CPU dans la
        // fenêtre du verrou d'écriture.
        let apercu = crate::body::extraire_apercu(html);
        let tx = self.0.unchecked_transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO bodies (mailbox_id, uid, html, scanned, preview)
             VALUES (?1, ?2, ?3, 1, ?4)",
            params![mailbox_id, uid, html, apercu],
        )?;
        // Remplacement intégral : un message re-téléchargé dont une pièce
        // aurait disparu ne doit pas garder l'ancienne ligne fantôme.
        tx.execute(
            "DELETE FROM attachments WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid],
        )?;
        for attachment in attachments {
            tx.execute(
                "INSERT INTO attachments (mailbox_id, uid, idx, name, mime, size)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    mailbox_id,
                    uid,
                    attachment.index as i64,
                    attachment.name,
                    attachment.mime,
                    attachment.size as i64
                ],
            )?;
        }
        match invitation {
            Some(row) => ecrire_invitation(&tx, mailbox_id, uid, row)?,
            // Même règle que les pièces : un re-scan SANS partie
            // calendrier ne garde pas une carte fantôme.
            None => {
                tx.execute(
                    "DELETE FROM invitations WHERE mailbox_id = ?1 AND uid = ?2",
                    params![mailbox_id, uid],
                )?;
            }
        }
        if let Some((subject, sender, sender_address, to_field, cc_field)) = tx
            .query_row(
                "SELECT subject, sender, sender_address, to_addrs, cc_addrs
                 FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                },
            )
            .optional()?
        {
            search::index_message(
                &tx,
                mailbox_id,
                uid,
                search::Indexed {
                    subject: subject.as_deref(),
                    sender: sender.as_deref(),
                    sender_address: sender_address.as_deref(),
                    to_addrs: to_field.as_deref(),
                    cc_addrs: cc_field.as_deref(),
                    body_html: Some(html),
                },
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// L'invitation d'un message, avec notre réponse locale — lecture
    /// LOCALE, jamais de réseau. `None` : ce message n'en porte pas (ou
    /// son MIME n'a pas encore été inspecté).
    pub fn invitation(
        &self,
        account_id: i64,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Option<InvitationStockee>, Error> {
        let stockee = self
            .0
            .query_row(
                // Colonnes lues PAR NOM : une colonne ajoutée au milieu du
                // SELECT ne décale jamais les champs — dix-neuf Options du
                // même type, un décalage positionnel serait silencieux et
                // enverrait la réponse iTIP à la mauvaise adresse (revue).
                "SELECT i.* FROM invitations i JOIN mailboxes m ON m.id = i.mailbox_id
                 WHERE m.account_id = ?1 AND m.name = ?2 AND i.uid = ?3",
                params![account_id, mailbox, uid],
                |row| {
                    Ok(InvitationStockee {
                        row: InvitationRow {
                            methode: row.get("methode")?,
                            event_uid: row.get("event_uid")?,
                            sequence: row.get("sequence")?,
                            titre: row.get("titre")?,
                            lieu: row.get("lieu")?,
                            organisateur_adresse: row.get("organisateur_adresse")?,
                            organisateur_nom: row.get("organisateur_nom")?,
                            debut_epoch: row.get("debut_epoch")?,
                            fin_epoch: row.get("fin_epoch")?,
                            debut_texte: row.get("debut_texte")?,
                            fin_texte: row.get("fin_texte")?,
                            journee_entiere: row.get("journee_entiere")?,
                            recurrent: row.get("recurrent")?,
                            partstat: row.get("partstat")?,
                            repondant_adresse: row.get("repondant_adresse")?,
                            repondant_nom: row.get("repondant_nom")?,
                            repondant_statut: row.get("repondant_statut")?,
                            annule: row.get("annule")?,
                        },
                        reponse: row.get("reponse")?,
                        reponse_epoch: row.get("reponse_epoch")?,
                    })
                },
            )
            .optional()?;
        Ok(stockee)
    }

    /// L'adresse d'un compte par son id — la clé de lecture des
    /// invitations (notre PARTSTAT se cherche par adresse). Adresse
    /// VIDE = compte à moitié provisionné : dit `None`, comme
    /// [`Store::accounts`] qui filtre ces lignes.
    pub fn account_email(&self, account_id: i64) -> Result<Option<String>, Error> {
        let email: Option<String> = self
            .0
            .query_row(
                "SELECT email FROM accounts WHERE id = ?1 AND email != ''",
                params![account_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(email)
    }

    /// Remplace la liste des dossiers connus d'un compte.
    ///
    /// Remplacement intégral et transactionnel : un dossier supprimé côté
    /// serveur ne doit pas rester proposé comme destination — le
    /// déplacement échouerait au rejeu, longtemps après le clic.
    pub fn replace_folders(&self, account_id: i64, folders: &[Folder]) -> Result<(), Error> {
        let tx = self.0.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM folders WHERE account_id = ?1",
            params![account_id],
        )?;
        for folder in folders {
            tx.execute(
                "INSERT OR REPLACE INTO folders (account_id, wire, display, selectable, special_use)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    account_id,
                    folder.wire,
                    folder.display,
                    folder.selectable,
                    folder.special_use.map(SpecialUse::code)
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Les dossiers connus d'un compte — lecture LOCALE, jamais de réseau.
    pub fn folders(&self, account_id: i64) -> Result<Vec<Folder>, Error> {
        let mut statement = self.0.prepare(
            "SELECT wire, display, selectable, special_use FROM folders
             WHERE account_id = ?1 ORDER BY display",
        )?;
        let rows = statement.query_map(params![account_id], |row| {
            Ok(Folder {
                wire: row.get(0)?,
                display: row.get(1)?,
                selectable: row.get(2)?,
                special_use: row
                    .get::<_, Option<String>>(3)?
                    .as_deref()
                    .and_then(SpecialUse::from_code),
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Rattrape l'aperçu des corps écrits AVANT la colonne `preview` —
    /// par lots bornés. Appelé par le shell au fil de son sondage :
    /// jamais sur le chemin d'ouverture (budget démarrage < 1 s, leçon
    /// de la chasse aux orphelins), jamais au défilement. Rend le nombre
    /// de retardataires restants — zéro quand la passe est soldée.
    pub fn preview_catchup(&self, limit: usize) -> Result<u64, Error> {
        // Par sous-lots de 100 corps (PLAN-AUDIT-V2 E2) : le shell demande
        // 500 d'un coup, et 500 corps HTML entiers en RAM pesaient ~28 Mo
        // au 56 ko moyen — cinq fois moins par sous-lot, même contrat.
        const SOUS_LOT: usize = 100;
        let mut restes = limit;
        while restes > 0 {
            let pris = self.rattraper_apercus_lot(restes.min(SOUS_LOT))?;
            if pris == 0 {
                break;
            }
            restes -= pris;
        }
        let restants: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM bodies WHERE preview IS NULL",
            [],
            |row| row.get(0),
        )?;
        Ok(restants as u64)
    }

    /// Un sous-lot de [`Store::preview_catchup`] ; rend le nombre de corps
    /// traités (zéro = plus de retardataire).
    fn rattraper_apercus_lot(&self, limit: usize) -> Result<usize, Error> {
        let lot: Vec<(i64, Uid, String)> = self
            .0
            .prepare("SELECT mailbox_id, uid, html FROM bodies WHERE preview IS NULL LIMIT ?1")?
            .query_map(params![limit as i64], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<_, _>>()?;
        if !lot.is_empty() {
            // Le CPU HORS de la fenêtre du verrou (terrain 2026-08-15) :
            // extraire les aperçus DANS la transaction tenait le verrou
            // d'écriture pendant tout le parsing du lot (2 000 corps HTML
            // au sondage du shell) — une écriture UI concurrente
            // (`delete_draft` d'un brouillon vidé) expirait son
            // busy_timeout et échouait en BUSY. On parse d'abord, la
            // transaction ne fait plus qu'écrire — courte par
            // construction.
            let apercus: Vec<(i64, Uid, String)> = lot
                .iter()
                .map(|(mailbox_id, uid, html)| {
                    (*mailbox_id, *uid, crate::body::extraire_apercu(html))
                })
                .collect();
            let tx = self.0.unchecked_transaction()?;
            for (mailbox_id, uid, apercu) in &apercus {
                tx.execute(
                    "UPDATE bodies SET preview = ?3 WHERE mailbox_id = ?1 AND uid = ?2",
                    params![mailbox_id, uid, apercu],
                )?;
            }
            tx.commit()?;
        }
        Ok(lot.len())
    }

    /// Les pièces jointes connues d'un message, dans l'ordre du MIME.
    ///
    /// Vide tant que le corps n'a pas été rapatrié : c'est la même
    /// condition que la recherche dans le texte, et le rattrapage la
    /// lève pour tout l'horizon de récence.
    pub fn attachments(
        &self,
        account_id: i64,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Vec<Attachment>, Error> {
        let mut statement = self.0.prepare(
            "SELECT a.idx, a.name, a.mime, a.size
             FROM attachments a
             JOIN mailboxes m ON m.id = a.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2 AND a.uid = ?3
             ORDER BY a.idx",
        )?;
        let rows = statement.query_map(params![account_id, mailbox, uid], |row| {
            Ok(Attachment {
                index: row.get::<_, i64>(0)? as usize,
                name: row.get(1)?,
                mime: row.get(2)?,
                size: row.get::<_, i64>(3)? as u64,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Les messages NON LUS arrivés après `uid_gt`, du plus ancien au
    /// plus récent — la matière des notifications.
    ///
    /// Le critère est l'UID, pas la date : c'est l'ordre d'arrivée que
    /// le serveur garantit, et c'est lui qui distingue « nouveau » de
    /// « ancien mais récemment daté ». Les messages déjà lus ailleurs
    /// sont exclus : notifier un message que l'utilisateur vient de lire
    /// sur son téléphone est du bruit pur.
    pub fn new_unread_after(
        &self,
        account_id: i64,
        mailbox: &str,
        uid_gt: Uid,
        limit: usize,
    ) -> Result<Vec<Envelope>, Error> {
        let mut statement = self.0.prepare(
            "SELECT e.uid, e.subject, e.sender, e.sender_address, e.message_id,
                    e.date_epoch, e.seen, e.flagged, e.in_reply_to, e.to_addrs, e.cc_addrs
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND e.uid > ?3 AND e.seen = 0
             ORDER BY e.uid
             LIMIT ?4",
        )?;
        let rows = statement.query_map(
            params![account_id, mailbox, uid_gt, limit as i64],
            row_to_envelope,
        )?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Les messages RÉCENTS dont le corps manque encore, du plus récent au
    /// plus ancien — le travail du rattrapage ([ADR 0007](../../../docs/adr/0007-rattrapage-des-corps.md)).
    ///
    /// `since_epoch` borne le coût : c'est l'horizon de récence. L'ordre
    /// décroissant rend la reprise après coupure naturelle — on redemande
    /// simplement la liste, les corps déjà écrits n'y sont plus.
    ///
    /// Un message sans date est TOUJOURS éligible — révisé par
    /// l'ADR 0010. L'ancienne règle l'excluait comme « non situable dans
    /// l'horizon » ; depuis que la production ne borne plus rien
    /// ([`crate::NO_HORIZON`]), l'exclure serait un trou silencieux : un
    /// message dont la date ne se lit pas n'aurait jamais de corps, donc
    /// jamais de recherche, sans que rien ne le signale. Il passe en
    /// dernier (les NULL ferment un tri DESC) : le doute ne coûte que son
    /// rang.
    pub fn bodies_to_backfill(
        &self,
        account_id: i64,
        mailbox: &str,
        since_epoch: i64,
        limit: usize,
    ) -> Result<Vec<Uid>, Error> {
        let mut stmt = self.0.prepare(&bodies_to_backfill_sql())?;
        let uids = stmt
            .query_map(
                params![account_id, mailbox, since_epoch, limit as i64],
                |row| row.get(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(uids)
    }

    /// Les messages dont les en-têtes de fil n'ont pas encore été lus, du
    /// plus récent au plus ancien.
    ///
    /// `refs IS NULL` = jamais lu. Un message sans `References` reçoit
    /// `""` et sort définitivement de cette liste.
    pub fn thread_headers_to_backfill(
        &self,
        account_id: i64,
        mailbox: &str,
        since_epoch: i64,
        limit: usize,
    ) -> Result<Vec<Uid>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT e.uid
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND (e.date_epoch IS NULL OR e.date_epoch >= ?3)
               AND e.refs IS NULL
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT ?4",
        )?;
        let uids = stmt
            .query_map(
                params![account_id, mailbox, since_epoch, limit as i64],
                |row| row.get(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(uids)
    }

    /// Les messages d'une boîte dont les destinataires n'ont pas encore
    /// été lus, du plus récent au plus ancien (R4, rattrapage des envois
    /// D2).
    ///
    /// `to_addrs IS NULL` = jamais lu. Un message sans À reçoit `""`
    /// (chaîne vide, PAS NULL) et sort définitivement de cette liste —
    /// même sentinelle que `refs` pour les en-têtes de fil, sans quoi la
    /// pompe le redemanderait indéfiniment (enseignement de convergence,
    /// PASSATION §9).
    pub fn recipients_to_backfill(
        &self,
        account_id: i64,
        mailbox: &str,
        limit: usize,
    ) -> Result<Vec<Uid>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT e.uid
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND e.to_addrs IS NULL
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT ?3",
        )?;
        let uids = stmt
            .query_map(params![account_id, mailbox, limit as i64], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(uids)
    }

    /// Combien d'envois attendent encore leurs destinataires.
    pub fn recipients_pending_count(&self, account_id: i64, mailbox: &str) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*)
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2 AND e.to_addrs IS NULL",
            params![account_id, mailbox],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Écrit les destinataires À/Cc d'un message déjà stocké (rattrapage
    /// des envois). Écrit `""` — jamais NULL — quand la liste est vide :
    /// c'est la marque « lu, aucun » qui fait converger la pompe. Ne
    /// touche à AUCUNE autre colonne (ni fil, ni refs).
    pub fn set_recipients(
        &self,
        mailbox_id: i64,
        uid: Uid,
        to: &[String],
        cc: &[String],
    ) -> Result<(), Error> {
        // E4 : destinataires et annuaire d'accord ou rien.
        let tx = self.0.unchecked_transaction()?;
        self.0.execute(
            "UPDATE envelopes SET to_addrs = ?3, cc_addrs = ?4
             WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid, to.join("\n"), cc.join("\n")],
        )?;
        // PLAN-RETOURS-5 (D4, revue) : ces destinataires rattrapés sont
        // ceux de NOS envois d'avant le stockage des À/Cc — sans ceci,
        // ils n'entreraient jamais dans l'annuaire (le rattrapage
        // d'ouverture est passé avant eux). Le surcoût (deux lectures)
        // est invisible derrière l'aller-retour serveur qui précède.
        let (_, noter_destinataires) = self.role_annuaire(mailbox_id)?;
        if noter_destinataires && (!to.is_empty() || !cc.is_empty()) {
            let date: Option<i64> = self
                .0
                .query_row(
                    "SELECT date_epoch FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                    params![mailbox_id, uid],
                    |row| row.get(0),
                )
                .optional()?
                .flatten();
            for adresse in to.iter().chain(cc.iter()) {
                crate::correspondants::noter(self.conn(), adresse, None, date.unwrap_or(0))?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Combien de messages attendent encore leurs en-têtes de fil.
    pub fn thread_headers_pending_count(
        &self,
        account_id: i64,
        mailbox: &str,
        since_epoch: i64,
    ) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*)
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND (e.date_epoch IS NULL OR e.date_epoch >= ?3)
               AND e.refs IS NULL",
            params![account_id, mailbox, since_epoch],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Combien de messages attendent encore leur corps dans l'horizon —
    /// de quoi afficher un avancement honnête.
    pub fn bodies_pending_count(
        &self,
        account_id: i64,
        mailbox: &str,
        since_epoch: i64,
    ) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            &bodies_pending_count_sql(),
            params![account_id, mailbox, since_epoch],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Le corpus EN PORTÉE : tous les messages qui PEUVENT porter un corps
    /// (même filtre que [`Self::bodies_pending_count`], sans la clause du
    /// corps manquant). C'est le dénominateur du pourcentage de rattrapage
    /// (R1, PLAN-RETOURS-3) — `total - pending` donne les corps présents.
    /// Plus léger que le compte des manquants : pas de sous-requête
    /// `NOT EXISTS`.
    pub fn bodies_total_count(
        &self,
        account_id: i64,
        mailbox: &str,
        since_epoch: i64,
    ) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*)
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND (e.date_epoch IS NULL OR e.date_epoch >= ?3)",
            params![account_id, mailbox, since_epoch],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Une page d'enveloppes d'UN compte, les plus récentes d'abord.
    pub fn recent(
        &self,
        account_id: i64,
        mailbox: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Envelope>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT e.uid, e.subject, e.sender, e.sender_address, e.message_id,
                    e.date_epoch, e.seen, e.flagged, e.in_reply_to, e.to_addrs, e.cc_addrs
             FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT ?3 OFFSET ?4",
        )?;
        let rows = stmt
            .query_map(
                params![account_id, mailbox, limit as i64, offset as i64],
                row_to_envelope,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// La boîte unifiée : la même boîte (INBOX) de TOUS les comptes,
    /// fusionnée par date — le cœur produit du multi-comptes. Chaque
    /// ligne porte son compte : un UID seul n'identifie plus un message.
    pub fn unified_recent(&self, offset: usize, limit: usize) -> Result<Vec<UnifiedRow>, Error> {
        // Une ligne par CONVERSATION, représentée par son dernier message.
        //
        // Le départ se fait depuis `threads`, pas depuis `envelopes` : un
        // `GROUP BY thread_id` avec un `MAX(date)` obligerait SQLite à
        // parcourir puis trier les 200 000 enveloppes à chaque page de
        // défilement. Ici l'index `idx_threads_date_globale` porte à la
        // fois le tri et la pagination — le coût d'une page ne dépend
        // plus de la taille de la boîte. C'est l'agrégat matérialisé qui
        // paie ça, et il se maintient dans la transaction d'écriture.
        //
        // La pagination vit dans une SOUS-REQUÊTE sur `threads` seul :
        // voir `unified_page_sql`.
        let mut stmt = self.0.prepare(&unified_page_sql(false, false, false))?;
        let rows = stmt
            .query_map(params![limit as i64, offset as i64], row_to_threaded)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Total de la boîte unifiée — en CONVERSATIONS, puisque c'est ce que
    /// la liste affiche. Compter les messages ferait défiler dans le vide.
    /// Les fils ÉPINGLÉS n'y comptent pas (R4, D5) — la page les exclut,
    /// le total DOIT décrire le même ensemble qu'elle (revue 2026-08-21 :
    /// la paire page/total désaccordée fabriquerait des lignes fantômes).
    //
    // (`unified_page_sql`, plus bas, porte la requête de la page.)
    pub fn unified_count(&self) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            &format!(
                "SELECT COUNT(*) FROM threads
                  WHERE inbox_size > 0 AND id NOT IN ({PINNED_THREADS})"
            ),
            [],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// R4 (PLAN-RETOURS-7) : épingle ou désépingle la CONVERSATION du
    /// message donné — rend le nouvel état. Poser l'épingle enregistre
    /// la clé d'enveloppe du geste ; la retirer libère le fil ENTIER
    /// (toutes les clés qui y mènent), sans quoi une épingle posée hier
    /// depuis une autre tête du fil resterait accrochée. Le fil se
    /// résout UNE fois — l'état et l'écriture regardent le même
    /// (revue 2026-08-21 : deux résolutions pouvaient diverger si une
    /// synchro re-filait entre elles).
    pub fn toggle_pin(&self, mailbox_id: i64, uid: Uid, epoch: i64) -> Result<bool, Error> {
        let fil = thread::thread_of(&self.0, mailbox_id, uid)?;
        if self.pin_state_du_fil(fil, mailbox_id, uid)? {
            match fil {
                Some(fil) => self.0.execute(
                    "DELETE FROM pins WHERE (mailbox_id, uid) IN
                       (SELECT mailbox_id, uid FROM envelopes WHERE thread_id = ?1)",
                    params![fil],
                )?,
                None => self.0.execute(
                    "DELETE FROM pins WHERE mailbox_id = ?1 AND uid = ?2",
                    params![mailbox_id, uid],
                )?,
            };
            Ok(false)
        } else {
            self.0.execute(
                "INSERT OR REPLACE INTO pins (mailbox_id, uid, epoch) VALUES (?1, ?2, ?3)",
                params![mailbox_id, uid, epoch],
            )?;
            Ok(true)
        }
    }

    /// La conversation du message est-elle épinglée ? L'état se lit par
    /// le FIL : une épingle posée sur n'importe quel message du fil vaut
    /// pour sa tête courante — la barre du fil dit vrai même quand une
    /// réponse a déplacé la tête depuis le geste.
    pub fn pin_state(&self, mailbox_id: i64, uid: Uid) -> Result<bool, Error> {
        let fil = thread::thread_of(&self.0, mailbox_id, uid)?;
        self.pin_state_du_fil(fil, mailbox_id, uid)
    }

    fn pin_state_du_fil(&self, fil: Option<i64>, mailbox_id: i64, uid: Uid) -> Result<bool, Error> {
        let epingle = match fil {
            Some(fil) => self
                .0
                .prepare(
                    "SELECT 1 FROM pins p JOIN envelopes e
                       ON e.mailbox_id = p.mailbox_id AND e.uid = p.uid
                     WHERE e.thread_id = ?1",
                )?
                .exists(params![fil])?,
            None => self
                .0
                .prepare("SELECT 1 FROM pins WHERE mailbox_id = ?1 AND uid = ?2")?
                .exists(params![mailbox_id, uid])?,
        };
        Ok(epingle)
    }

    /// E5 — Mis de côté : le MÊME contrat que l'épingle (patron
    /// `toggle_pin`, résolution du fil UNE fois) — posé sur un message,
    /// l'état vaut pour le fil entier ; « Terminé » depuis n'importe
    /// quelle tête libère tout. Rend l'état APRÈS le geste.
    pub fn toggle_mis_de_cote(&self, mailbox_id: i64, uid: Uid, epoch: i64) -> Result<bool, Error> {
        let fil = thread::thread_of(&self.0, mailbox_id, uid)?;
        if self.mis_de_cote_du_fil(fil, mailbox_id, uid)? {
            match fil {
                Some(fil) => self.0.execute(
                    "DELETE FROM mis_de_cote WHERE (mailbox_id, uid) IN
                       (SELECT mailbox_id, uid FROM envelopes WHERE thread_id = ?1)",
                    params![fil],
                )?,
                None => self.0.execute(
                    "DELETE FROM mis_de_cote WHERE mailbox_id = ?1 AND uid = ?2",
                    params![mailbox_id, uid],
                )?,
            };
            Ok(false)
        } else {
            self.0.execute(
                "INSERT OR REPLACE INTO mis_de_cote (mailbox_id, uid, epoch) VALUES (?1, ?2, ?3)",
                params![mailbox_id, uid, epoch],
            )?;
            Ok(true)
        }
    }

    /// Le fil de ce message est-il mis de côté ? — l'état par le FIL,
    /// tête nouvelle comprise (même règle que `pin_state`).
    pub fn etat_mis_de_cote(&self, mailbox_id: i64, uid: Uid) -> Result<bool, Error> {
        let fil = thread::thread_of(&self.0, mailbox_id, uid)?;
        self.mis_de_cote_du_fil(fil, mailbox_id, uid)
    }

    fn mis_de_cote_du_fil(
        &self,
        fil: Option<i64>,
        mailbox_id: i64,
        uid: Uid,
    ) -> Result<bool, Error> {
        let cote = match fil {
            Some(fil) => self
                .0
                .prepare(
                    "SELECT 1 FROM mis_de_cote c JOIN envelopes e
                       ON e.mailbox_id = c.mailbox_id AND e.uid = c.uid
                     WHERE e.thread_id = ?1",
                )?
                .exists(params![fil])?,
            None => self
                .0
                .prepare("SELECT 1 FROM mis_de_cote WHERE mailbox_id = ?1 AND uid = ?2")?
                .exists(params![mailbox_id, uid])?,
        };
        Ok(cote)
    }

    /// La pile (E5) : les têtes des fils mis de côté, au squelette
    /// unifié — l'ordre des listes (la date), l'éventail et le tableau
    /// s'en servent tels quels. Petite par construction.
    pub fn pile_mis_de_cote(&self) -> Result<Vec<UnifiedRow>, Error> {
        let queue = unified_join_tail(false);
        let sql = format!(
            "{SELECT_UNIFIED}{THREAD_AGGREGATE}
             FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
                     FROM threads
                    WHERE inbox_size > 0 AND id IN ({MIS_DE_COTE_THREADS})) t{queue}"
        );
        let mut stmt = self.0.prepare(&sql)?;
        let rows = stmt
            .query_map([], row_to_threaded)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// R1 (PLAN-RETOURS-11, D1-D2) : le choix « Afficher les images »
    /// du message — clé d'enveloppe, patron de `pins`. Rejouer le
    /// geste ne change rien (REPLACE).
    pub fn allow_images_message(&self, mailbox_id: i64, uid: Uid, epoch: i64) -> Result<(), Error> {
        self.0.execute(
            "INSERT OR REPLACE INTO images_messages (mailbox_id, uid, epoch)
             VALUES (?1, ?2, ?3)",
            params![mailbox_id, uid, epoch],
        )?;
        Ok(())
    }

    /// D3 : pose la règle « toujours afficher les images de cet
    /// expéditeur » DEPUIS un message — l'adresse est lue de
    /// l'enveloppe (jamais de l'UI), normalisée en minuscules. Rend
    /// l'adresse posée ; None si l'enveloppe n'a pas d'adresse (jamais
    /// une règle vide). N'écrit PAS de choix par message : la règle
    /// d'expéditeur doit suffire seule, et sa révocation tout défaire.
    pub fn allow_images_sender_of(
        &self,
        mailbox_id: i64,
        uid: Uid,
        epoch: i64,
    ) -> Result<Option<String>, Error> {
        let adresse: Option<String> = self
            .0
            .query_row(
                "SELECT sender_address FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let Some(adresse) = adresse_images(adresse) else {
            return Ok(None);
        };
        self.0.execute(
            "INSERT OR REPLACE INTO images_expediteurs (address, epoch) VALUES (?1, ?2)",
            params![adresse, epoch],
        )?;
        Ok(Some(adresse))
    }

    /// D4 : retire une règle d'expéditeur (la porte de sortie du
    /// « toujours »). La normalisation passe par LA même autorité que
    /// la pose — sinon une règle posée deviendrait irrévocable le jour
    /// où `adresse_images` évolue.
    pub fn revoke_images_sender(&self, address: &str) -> Result<(), Error> {
        let Some(adresse) = adresse_images(Some(address.to_string())) else {
            return Ok(());
        };
        self.0.execute(
            "DELETE FROM images_expediteurs WHERE address = ?1",
            params![adresse],
        )?;
        Ok(())
    }

    /// Les règles d'expéditeur, pour la liste des Réglages (D4) —
    /// ordre alphabétique : l'œil y cherche une adresse.
    pub fn images_senders(&self) -> Result<Vec<String>, Error> {
        let mut stmt = self
            .0
            .prepare("SELECT address FROM images_expediteurs ORDER BY address")?;
        let adresses = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(adresses)
    }

    /// La porte du RENDU (message_body) : ce message a-t-il droit aux
    /// images distantes ? Choix par message OU règle d'expéditeur —
    /// l'adresse de l'enveloppe est normalisée par le MÊME chemin que
    /// l'écriture (une seule autorité, jamais le lower() ASCII de
    /// SQLite).
    pub fn images_allowed(&self, mailbox_id: i64, uid: Uid) -> Result<bool, Error> {
        if self
            .0
            .prepare("SELECT 1 FROM images_messages WHERE mailbox_id = ?1 AND uid = ?2")?
            .exists(params![mailbox_id, uid])?
        {
            return Ok(true);
        }
        let adresse: Option<String> = self
            .0
            .query_row(
                "SELECT sender_address FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        match adresse_images(adresse) {
            Some(adresse) => Ok(self
                .0
                .prepare("SELECT 1 FROM images_expediteurs WHERE address = ?1")?
                .exists(params![adresse])?),
            None => Ok(false),
        }
    }

    /// L'état du Mode organisé (PLAN-MODE-ORGANISE E1, D2 amendée :
    /// `prefs` SQLite — le cœur doit savoir, les règles du Non
    /// s'éteignent avec le mode). Éteint tant que rien n'est posé.
    pub fn mode_organise(&self) -> Result<bool, Error> {
        Ok(self.text_pref(PREF_MODE_ORGANISE)?.as_deref() == Some("1"))
    }

    /// L'époque de PREMIÈRE activation du mode — la borne de la
    /// rétention du Portier (D3 « arrivées seules »). None tant que le
    /// mode n'a jamais été activé.
    pub fn mode_organise_epoch(&self) -> Result<Option<i64>, Error> {
        Ok(self
            .text_pref(PREF_MODE_ORGANISE_EPOCH)?
            .and_then(|v| v.parse().ok()))
    }

    /// Bascule le mode. À la PREMIÈRE activation, l'état et l'époque
    /// s'écrivent ENSEMBLE (transaction — jamais un mode actif sans sa
    /// borne) ; l'époque ne se réécrit JAMAIS ensuite : la réécrire à
    /// une réactivation déverserait au Portier (ou en Réception) du
    /// courrier arrivé entre-temps, en silence.
    pub fn set_mode_organise(&mut self, actif: bool, epoch: i64) -> Result<(), Error> {
        if actif && self.text_pref(PREF_MODE_ORGANISE_EPOCH)?.is_none() {
            self.set_text_prefs(&[
                (PREF_MODE_ORGANISE, "1"),
                (PREF_MODE_ORGANISE_EPOCH, &epoch.to_string()),
            ])
        } else {
            self.set_text_pref(PREF_MODE_ORGANISE, if actif { "1" } else { "0" })
        }
    }

    /// RETOURS-13 R10 — marque une carte du Kiosque comme lue (le bas
    /// de son élévation a été affiché). Idempotente ; clé d'enveloppe,
    /// patron `pins` — jamais le `seen` IMAP (autre sémantique,
    /// écrasée par la synchro).
    pub fn marquer_kiosque_lu(&self, mailbox_id: i64, uid: Uid, epoch: i64) -> Result<(), Error> {
        self.0.execute(
            "INSERT OR IGNORE INTO kiosque_lus (mailbox_id, uid, epoch) VALUES (?1, ?2, ?3)",
            params![mailbox_id, uid, epoch],
        )?;
        Ok(())
    }

    /// Une carte du Kiosque a-t-elle déjà été lue ? (sonde PK)
    pub fn kiosque_lu(&self, mailbox_id: i64, uid: Uid) -> Result<bool, Error> {
        Ok(self
            .0
            .prepare("SELECT 1 FROM kiosque_lus WHERE mailbox_id = ?1 AND uid = ?2")?
            .exists(params![mailbox_id, uid])?)
    }

    /// RETOURS-13 R5/R9 — les actions par défaut des boutons Oui/Non du
    /// Portier. Livrées : Oui → `reception`, Non → `corbeille`. Une
    /// valeur hors vocabulaire en base (écrite hors porte) retombe au
    /// défaut : le clic nu ne pose JAMAIS un verdict troué.
    pub fn portier_defauts(&self) -> Result<(String, String), Error> {
        let oui = self
            .text_pref(PREF_PORTIER_DEFAUT_OUI)?
            .filter(|v| defaut_portier_oui_valide(v))
            .unwrap_or_else(|| "reception".to_string());
        let non = self
            .text_pref(PREF_PORTIER_DEFAUT_NON)?
            .filter(|v| defaut_portier_non_valide(v))
            .unwrap_or_else(|| "corbeille".to_string());
        Ok((oui, non))
    }

    /// Règle les défauts du Portier — vocabulaire FERMÉ, vérifié avant
    /// toute écriture (décision pure) : le Oui prend une destination
    /// (jamais `ecarte`), le Non une règle ou « écarter sans déplacer ».
    pub fn set_portier_defauts(&mut self, oui: &str, non: &str) -> Result<(), Error> {
        if !defaut_portier_oui_valide(oui) {
            return Err(Error::InvalidRouting(format!(
                "défaut du Oui inconnu : {oui:?}"
            )));
        }
        if !defaut_portier_non_valide(non) {
            return Err(Error::InvalidRouting(format!(
                "défaut du Non inconnu : {non:?}"
            )));
        }
        self.set_text_prefs(&[
            (PREF_PORTIER_DEFAUT_OUI, oui),
            (PREF_PORTIER_DEFAUT_NON, non),
        ])
    }

    /// Pose le verdict du Mode organisé sur un expéditeur
    /// (PLAN-MODE-ORGANISE E1, D1 : routage LOCAL seul). Un seul
    /// verdict par adresse — la pose écrase la décision précédente
    /// (changer d'avis est un droit, patron du Portier). Le vocabulaire
    /// est fermé et vérifié AVANT l'écriture (décision pure) ; une
    /// règle du Non n'a de sens que sur un expéditeur écarté.
    pub fn router_expediteur(
        &self,
        address: &str,
        destination: &str,
        regle: Option<&str>,
        epoch: i64,
    ) -> Result<(), Error> {
        valider_routage(destination, regle)?;
        let Some(adresse) = adresse_images(Some(address.to_string())) else {
            return Err(Error::InvalidEmailAddress(address.to_string()));
        };
        // Verdict, sortie de l'attente et drapeaux des fils dans UNE
        // transaction (E2) : un verdict à moitié appliqué laisserait un
        // expéditeur au Portier ET dans sa vue.
        let tx = self.0.unchecked_transaction()?;
        poser_verdict(&tx, &adresse, destination, regle, epoch)?;
        tx.commit()?;
        Ok(())
    }

    /// « Déplacer vers… » (E1) : pose le verdict DEPUIS un message —
    /// l'adresse est lue en base (jamais de l'UI), normalisée et
    /// validée par [`Store::router_expediteur`], LA porte unique.
    ///
    /// Revue E1 : la ligne servie est la TÊTE du fil — le dernier
    /// message toutes boîtes confondues, Envoyés compris. S'ancrer
    /// dessus routerait la PROPRE adresse de l'utilisateur dès qu'il a
    /// répondu en dernier. L'adresse routée est donc celle du dernier
    /// message du fil qui ne vient PAS du compte (les alias d'envoi
    /// restent hors de cette garde — limite dite) ; un message hors
    /// fil retombe sur sa propre enveloppe. Rend l'adresse routée ;
    /// None si rien ne porte d'adresse (jamais un verdict fantôme).
    pub fn router_expediteur_of(
        &self,
        mailbox_id: i64,
        uid: Uid,
        destination: &str,
        regle: Option<&str>,
        epoch: i64,
    ) -> Result<Option<String>, Error> {
        // La porte de validation d'abord — un vocabulaire troué ne se
        // cache jamais derrière « message sans adresse » (revue E1).
        valider_routage(destination, regle)?;
        let du_fil: Option<String> = self
            .0
            .query_row(
                "SELECT te.sender_address
                   FROM envelopes te
                  WHERE te.thread_id = (SELECT thread_id FROM envelopes
                                         WHERE mailbox_id = ?1 AND uid = ?2)
                    AND te.sender_address IS NOT NULL
                    AND lower(trim(te.sender_address)) <> (
                          SELECT lower(trim(a.email)) FROM accounts a
                            JOIN mailboxes m ON m.account_id = a.id
                           WHERE m.id = ?1)
                  ORDER BY te.date_epoch DESC, te.uid DESC
                  LIMIT 1",
                params![mailbox_id, uid],
                |row| row.get(0),
            )
            .optional()?;
        let adresse = match du_fil {
            Some(a) => Some(a),
            None => self.sender_address_of(mailbox_id, uid)?,
        };
        let Some(adresse) = adresse_images(adresse) else {
            return Ok(None);
        };
        self.router_expediteur(&adresse, destination, regle, epoch)?;
        Ok(Some(adresse))
    }

    /// L'adresse d'expéditeur d'UNE enveloppe — la lecture partagée
    /// des portes qui résolvent côté cœur (garde d'images, routage) :
    /// une seule copie, jamais une divergence (leçon A80).
    fn sender_address_of(&self, mailbox_id: i64, uid: Uid) -> Result<Option<String>, Error> {
        Ok(self
            .0
            .query_row(
                "SELECT sender_address FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    /// « Réintégrer » à l'historique du Portier : le verdict disparaît,
    /// l'expéditeur redevient inconnu. La normalisation passe par LA
    /// même autorité que la pose — sinon un verdict deviendrait
    /// irrévocable le jour où elle évolue (leçon `revoke_images_sender`).
    pub fn retirer_routage(&self, address: &str) -> Result<(), Error> {
        let Some(adresse) = adresse_images(Some(address.to_string())) else {
            return Ok(());
        };
        let epoque = self.mode_organise_epoch()?;
        let tx = self.0.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM routage_expediteurs WHERE address = ?1",
            params![adresse],
        )?;
        // « Réintégrer » (E2) : un INCONNU — aucun courrier avant
        // l'époque — redevient un expéditeur en attente, ses messages
        // réapparaissent au Portier ; un ancien est simplement rendu à
        // la Réception, jamais au guichet (D3 : son historique fait
        // foi). Jamais soi (leçon E1). La porte de sortie suit la MÊME
        // règle que l'arrivée (revue E2) : seul du courrier ARRIVÉ
        // après l'époque réintègre — un expéditeur vu seulement en
        // Archives ou aux Indésirables n'a jamais passé le guichet.
        if let Some(epoque) = epoque
            && !adresse_d_un_compte(&tx, &adresse)?
            && !connu_avant_epoque(&tx, &adresse, epoque)?
            && tx
                .prepare(
                    "SELECT 1 FROM envelopes e
                       JOIN mailboxes m ON m.id = e.mailbox_id
                      WHERE e.sender_norm = ?1
                        AND (e.date_epoch > ?2 OR e.date_epoch IS NULL)
                        AND m.name = ?3 LIMIT 1",
                )?
                .exists(params![adresse, epoque, thread::RECEIVED_MAILBOX])?
        {
            tx.execute(
                "INSERT OR IGNORE INTO portier_attente (address) VALUES (?1)",
                params![adresse],
            )?;
        }
        rafraichir_fils_de(&tx, &adresse)?;
        tx.commit()?;
        Ok(())
    }

    /// Le verdict posé sur un expéditeur, s'il existe.
    pub fn routage_de(&self, address: &str) -> Result<Option<Routage>, Error> {
        let Some(adresse) = adresse_images(Some(address.to_string())) else {
            return Ok(None);
        };
        let routage = self
            .0
            .query_row(
                "SELECT address, destination, regle, epoch
                 FROM routage_expediteurs WHERE address = ?1",
                params![adresse],
                lire_routage,
            )
            .optional()?;
        Ok(routage)
    }

    /// L'historique du Portier : toutes les décisions, la plus récente
    /// en tête — l'œil y cherche le dernier verdict.
    pub fn routages(&self) -> Result<Vec<Routage>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT address, destination, regle, epoch
             FROM routage_expediteurs ORDER BY epoch DESC, address",
        )?;
        let routages = stmt
            .query_map([], lire_routage)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(routages)
    }

    /// Le guichet du Portier (E2) : un rang par expéditeur en attente —
    /// l'adresse normalisée (la clé que le verdict prendra) et sa
    /// DERNIÈRE arrivée postérieure à l'époque, au format des rangées
    /// de la liste. Le plus récent en tête. Vide tant que le mode n'a
    /// jamais été activé. Le guichet ne dit que les ARRIVÉES (revue
    /// E2) : un message du même expéditeur déjà jeté ou archivé n'est
    /// ni le rang ni le compte — la boîte du rang, c'est l'INBOX. Les
    /// sondes suivent `idx_envelopes_sender` (0,32 ms à 200 k,
    /// S2-bis) ; un rang dont le courrier a disparu ne se sert pas.
    pub fn portier_attente(&self) -> Result<Vec<RangPortier>, Error> {
        let Some(epoque) = self.mode_organise_epoch()? else {
            return Ok(Vec::new());
        };
        // `COALESCE` sur l'agrégat : le rang montre UN message — si son
        // fil n'existe pas (boîte hors portée), il compte pour lui-même.
        let sql = format!(
            "{SELECT_UNIFIED}, COALESCE(t.size, 1), COALESCE(t.unseen, 1 - e.seen), pa.address
             FROM portier_attente pa
             JOIN envelopes e ON e.rowid = (
                  SELECT e2.rowid FROM envelopes e2
                    JOIN mailboxes m2 ON m2.id = e2.mailbox_id AND m2.name = ?2
                   WHERE e2.sender_norm = pa.address
                     AND (e2.date_epoch > ?1 OR e2.date_epoch IS NULL)
                   ORDER BY e2.date_epoch DESC, e2.uid DESC LIMIT 1)
             JOIN mailboxes m ON m.id = e.mailbox_id
             JOIN accounts a ON a.id = m.account_id
             LEFT JOIN threads t ON t.id = e.thread_id
             LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
             ORDER BY e.date_epoch DESC, e.uid DESC"
        );
        let mut stmt = self.0.prepare(&sql)?;
        let rangs = stmt
            .query_map(params![epoque, thread::RECEIVED_MAILBOX], |row| {
                Ok(RangPortier {
                    ligne: row_to_threaded(row)?,
                    address: row.get(19)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rangs)
    }

    /// La pastille du Portier : combien de MESSAGES attendent — les
    /// arrivées postérieures à l'époque des expéditeurs en attente, la
    /// même portée que le guichet (jamais un compte que la page ne
    /// saurait montrer). Somme d'intervalles d'index, 0,26 ms à 200 k.
    pub fn portier_total(&self) -> Result<u64, Error> {
        let Some(epoque) = self.mode_organise_epoch()? else {
            return Ok(0);
        };
        let total: i64 = self.0.query_row(
            "SELECT COALESCE(SUM((SELECT COUNT(*) FROM envelopes e
                     JOIN mailboxes m ON m.id = e.mailbox_id AND m.name = ?2
                     WHERE e.sender_norm = pa.address
                       AND (e.date_epoch > ?1 OR e.date_epoch IS NULL))), 0)
             FROM portier_attente pa",
            params![epoque, thread::RECEIVED_MAILBOX],
            |row| row.get(0),
        )?;
        Ok(total as u64)
    }

    /// RETOURS-14 R4 (revue) — les ADRESSES seules du guichet, pour le
    /// badge « En attente au Portier » du fil : `portier_attente()`
    /// construit une rangée complète par expéditeur, ce que le badge
    /// n'a que faire — et le guichet n'est pas borné.
    pub fn portier_adresses(&self) -> Result<Vec<String>, Error> {
        let mut stmt = self
            .0
            .prepare("SELECT address FROM portier_attente ORDER BY address")?;
        let adresses = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(adresses)
    }

    // -----------------------------------------------------------------
    // Le Nettoyage de printemps (PLAN-HORIZON-NETTOYAGE volet B).
    // -----------------------------------------------------------------

    /// Les boîtes qu'un périmètre couvre (D6, vocabulaire CE) — résolu
    /// par compte depuis les canoniques. Envoyés, Brouillons,
    /// Indésirables et Corbeille sont TOUJOURS hors périmètre (on ne
    /// trie pas ce qui est déjà traité ou écrit par soi). L'archive
    /// INTÉGRALE (« Tous les messages ») rejouerait toute la boîte :
    /// hors périmètre — limite dite au PLAN.
    fn boites_du_perimetre(&self, perimetre: &str) -> Result<Vec<i64>, Error> {
        let dossiers_inclus = matches!(perimetre, "dossiers" | "dossiersArchives");
        let archives_incluses = matches!(perimetre, "archives" | "dossiersArchives");
        let mut ids = Vec::new();
        let mut stmt = self
            .0
            .prepare_cached("SELECT id, name FROM mailboxes WHERE account_id = ?1")?;
        for account in self.accounts()? {
            let canon = self.canonical_folders(account.id)?;
            let boites = stmt
                .query_map([account.id], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (id, name) in boites {
                let est = |canonique: &Option<String>| canonique.as_deref() == Some(name.as_str());
                let inclue = if name == canon.reception {
                    true
                } else if est(&canon.archives) {
                    archives_incluses && !canon.archives_integrale
                } else if est(&canon.envoyes)
                    || est(&canon.brouillons)
                    || est(&canon.indesirables)
                    || est(&canon.corbeille)
                {
                    false
                } else {
                    dossiers_inclus
                };
                if inclue {
                    ids.push(id);
                }
            }
        }
        Ok(ids)
    }

    /// La liste SQL des ids de boîtes du périmètre — UNE écriture
    /// (revue 2026-08-30 : trois copies divergeaient en germe). Les ids
    /// viennent de NOTRE base — jamais d'une saisie.
    fn liste_ids(ids: &[i64]) -> String {
        if ids.is_empty() {
            "NULL".to_string()
        } else {
            ids.iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",")
        }
    }

    /// Le critère partagé des groupes : le courrier de la plage dans le
    /// périmètre, hors expéditeurs déjà routés (D7), hors soi, hors
    /// enveloppes sans adresse. LA seule définition de « la plage dans
    /// le périmètre » — groupes, stock du verdict et vue d'un groupe la
    /// partagent. Un message SANS date compte dans toute plage
    /// (précédent A98 : « sans date = aujourd'hui » — limite dite au
    /// PLAN : il suit aussi les règles du stock).
    fn nettoyage_critere(ids: &[i64]) -> String {
        let liste = Self::liste_ids(ids);
        format!(
            "e.mailbox_id IN ({liste})
               AND (e.date_epoch > ?1 OR e.date_epoch IS NULL)
               AND e.sender_norm IS NOT NULL
               AND e.sender_norm NOT IN (SELECT address FROM routage_expediteurs WHERE address IS NOT NULL)
               AND e.sender_norm NOT IN (SELECT lower(trim(email)) FROM accounts WHERE email IS NOT NULL)"
        )
    }

    /// Le SQL des groupes — UNE passe : avec un max() SEUL, SQLite
    /// garantit que les colonnes nues (sender, subject) viennent de la
    /// ligne du max — le rang montre l'objet du dernier message DE LA
    /// PORTÉE (revue 2026-08-30 : deux sous-requêtes corrélées non
    /// bornées pouvaient afficher l'objet d'un message hors session, et
    /// repayaient le tri de l'expéditeur quatre fois par groupe).
    ///
    /// En DEUX phases sur l'index des expéditeurs (PLAN-AUDIT-V2 E4) :
    /// l'agrégat est COUVERT par `idx_envelopes_sender` (expéditeur,
    /// date, boîte — jamais une ligne de table lue), puis l'objet et le
    /// nom du dernier message se cherchent par le même index. Mesuré sur
    /// 200 k enveloppes et 5 000 expéditeurs : 380 ms → moins de 100
    /// (l'ancienne passe parcourait l'index de DATE puis un B-tree
    /// temporaire). `INDEXED BY` : le planificateur préférait l'index de
    /// date — le test de plan `les_groupes_du_nettoyage_se_lisent_par_
    /// l_index_des_expediteurs` tient la promesse. Le `GROUP BY` externe
    /// absorbe une égalité de date (deux messages d'un expéditeur à la
    /// même seconde) : un rang par groupe, jamais deux.
    fn nettoyage_groupes_sql(ids: &[i64]) -> String {
        let critere = Self::nettoyage_critere(ids);
        let liste = Self::liste_ids(ids);
        format!(
            "SELECT g.sender_norm, g.n, g.dernier, e.sender, e.subject
               FROM (SELECT e.sender_norm AS sender_norm, COUNT(*) AS n,
                            MAX(e.date_epoch) AS dernier
                       FROM envelopes e INDEXED BY {INDEX_EXPEDITEURS}
                      WHERE {critere}
                      GROUP BY e.sender_norm) g
               CROSS JOIN envelopes e INDEXED BY {INDEX_EXPEDITEURS}
                 ON e.sender_norm = g.sender_norm
                AND e.date_epoch IS g.dernier
                AND e.mailbox_id IN ({liste})
              GROUP BY g.sender_norm
              ORDER BY g.dernier DESC, g.sender_norm"
        )
    }

    /// Le courrier d'un groupe — LE critère partagé : la vue montre
    /// exactement ce que le verdict traitera. `INDEXED BY` (PLAN-AUDIT-V2
    /// E4) : sans lui, le SQLite embarqué préférait l'index de date et
    /// balayait la boîte entière pour 40 lignes — 116 ms sur 200 k.
    fn nettoyage_messages_sql(ids: &[i64]) -> String {
        let critere = Self::nettoyage_critere(ids);
        format!(
            "{SELECT_UNIFIED}, COALESCE(t.size, 1), COALESCE(t.unseen, 1 - e.seen)
             FROM envelopes e INDEXED BY {INDEX_EXPEDITEURS}
             JOIN mailboxes m ON m.id = e.mailbox_id
             JOIN accounts a ON a.id = m.account_id
             LEFT JOIN threads t ON t.id = e.thread_id
             LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
             WHERE e.sender_norm = ?2 AND {critere}
             ORDER BY e.date_epoch DESC, e.uid DESC"
        )
    }

    fn nettoyage_compter_groupes(&self, ids: &[i64], borne: i64) -> Result<u64, Error> {
        let critere = Self::nettoyage_critere(ids);
        let total: i64 = self.0.query_row(
            &format!(
                "SELECT COUNT(*) FROM (SELECT 1 FROM envelopes e INDEXED BY {INDEX_EXPEDITEURS}
                  WHERE {critere} GROUP BY e.sender_norm)"
            ),
            params![borne],
            |row| row.get(0),
        )?;
        Ok(total as u64)
    }

    /// La session en cours — `None` : aucun nettoyage entamé.
    pub fn nettoyage_etat(&self) -> Result<Option<SessionNettoyage>, Error> {
        Ok(self
            .0
            .query_row(
                "SELECT plage, perimetre, borne_epoch, total, traites
                   FROM nettoyage_session WHERE id = 1",
                [],
                |row| {
                    Ok(SessionNettoyage {
                        plage: row.get(0)?,
                        perimetre: row.get(1)?,
                        borne_epoch: row.get(2)?,
                        total: row.get::<_, i64>(3)? as u64,
                        traites: row.get::<_, i64>(4)? as u64,
                    })
                },
            )
            .optional()?)
    }

    /// Démarre un nettoyage (remplace la session en cours) : la borne
    /// est FIGÉE ici — une session ne glisse pas avec l'horloge — et le
    /// total de groupes devient le dénominateur de la progression.
    pub fn nettoyage_demarrer(
        &self,
        plage: &str,
        perimetre: &str,
        now: i64,
    ) -> Result<SessionNettoyage, Error> {
        if !PLAGES_NETTOYAGE.contains(&plage) {
            return Err(Error::Corrupt(format!("plage inconnue : {plage:?}")));
        }
        if !PERIMETRES_NETTOYAGE.contains(&perimetre) {
            return Err(Error::Corrupt(format!("périmètre inconnu : {perimetre:?}")));
        }
        let borne = crate::backfill::horizon_epoch(plage, now);
        let ids = self.boites_du_perimetre(perimetre)?;
        let total = self.nettoyage_compter_groupes(&ids, borne)?;
        self.0.execute(
            "INSERT OR REPLACE INTO nettoyage_session
               (id, plage, perimetre, borne_epoch, total, traites)
             VALUES (1, ?1, ?2, ?3, ?4, 0)",
            params![plage, perimetre, borne, total as i64],
        )?;
        Ok(SessionNettoyage {
            plage: plage.to_string(),
            perimetre: perimetre.to_string(),
            borne_epoch: borne,
            total,
            traites: 0,
        })
    }

    /// Les groupes restants de la session : un expéditeur × son
    /// courrier de la plage, le plus récent en tête. Vide sans session.
    pub fn nettoyage_groupes(&self) -> Result<Vec<GroupeNettoyage>, Error> {
        let Some(session) = self.nettoyage_etat()? else {
            return Ok(Vec::new());
        };
        let ids = self.boites_du_perimetre(&session.perimetre)?;
        let mut stmt = self.0.prepare(&Self::nettoyage_groupes_sql(&ids))?;
        let groupes = stmt
            .query_map(params![session.borne_epoch], |row| {
                Ok(GroupeNettoyage {
                    address: row.get(0)?,
                    messages: row.get::<_, i64>(1)? as u64,
                    dernier_epoch: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    qui: row.get(3)?,
                    dernier_objet: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(groupes)
    }

    /// Le verdict de GROUPE (D5 : le stock ET l'avenir) — la porte du
    /// Portier pour l'avenir (routage, sortie d'attente, drapeaux),
    /// plus l'application de la règle au stock DE LA PLAGE : une action
    /// par message en `pending_actions`, DANS la transaction du verdict
    /// (patron E3 — jamais une fenêtre de crash entre le courrier et
    /// l'intention), garde anti-doublon, `corbeille` → la corbeille du
    /// serveur, JAMAIS une suppression définitive (D4) ; `spam` sans
    /// dossier résolu ne fait RIEN (jamais une destination inventée).
    /// Rend le nombre de messages du stock traités.
    pub fn nettoyage_verdict(
        &mut self,
        address: &str,
        destination: &str,
        regle: Option<&str>,
        epoch: i64,
    ) -> Result<usize, Error> {
        let Some(session) = self.nettoyage_etat()? else {
            return Err(Error::Corrupt("aucun nettoyage en cours".to_string()));
        };
        valider_routage(destination, regle)?;
        let Some(adresse) = adresse_images(Some(address.to_string())) else {
            return Err(Error::InvalidEmailAddress(address.to_string()));
        };
        let ids = self.boites_du_perimetre(&session.perimetre)?;
        // Le dossier indésirable de CHAQUE compte, résolu AVANT la
        // transaction (même règle que l'arrivée E3).
        let mut indesirables: BTreeMap<i64, Option<String>> = BTreeMap::new();
        if destination == "ecarte" && regle == Some("spam") {
            for account in self.accounts()? {
                indesirables.insert(account.id, self.canonical_folders(account.id)?.indesirables);
            }
        }
        let mut retraits: Vec<(i64, Uid)> = Vec::new();
        let tx = self.0.unchecked_transaction()?;
        if destination == "ecarte"
            && let Some(regle) = regle
        {
            // Le stock : LE critère partagé (même définition que les
            // groupes et la vue), restreint à l'adresse — lu AVANT
            // `poser_verdict`, qui ferait sortir l'expéditeur du
            // critère (D7 exclut les routés).
            let critere = Self::nettoyage_critere(&ids);
            let stock: Vec<(i64, Uid, i64)> = {
                let mut stmt = tx.prepare(&format!(
                    "SELECT e.mailbox_id, e.uid, m.account_id
                       FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
                      WHERE e.sender_norm = ?2 AND {critere}"
                ))?;
                stmt.query_map(params![session.borne_epoch, adresse], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?
                .collect::<Result<Vec<_>, _>>()?
            };
            for (mailbox_id, uid, account_id) in stock {
                let action = match regle {
                    "archive" => Some(Action::Archive),
                    "corbeille" => Some(Action::Delete),
                    "spam" => indesirables
                        .get(&account_id)
                        .cloned()
                        .flatten()
                        .map(Action::MoveTo),
                    _ => None,
                };
                let Some(action) = action else { continue };
                // Une action DÉJÀ en file (un geste utilisateur d'il y a
                // quelques secondes — mark_seen, archivage) : on ne
                // journalise PAS la nôtre ET on ne retire PAS la copie
                // locale (revue 2026-08-30 : le patron d'arrivée E3
                // suppose un message NEUF sans action possible ; sur du
                // stock, retirer sans avoir posé l'intention ferait
                // croire au nettoyage un message que le serveur garde —
                // il reviendrait à la relève suivante). Le message reste
                // visible, cohérent avec le serveur — limite dite.
                let deja = tx
                    .prepare_cached(
                        "SELECT 1 FROM pending_actions WHERE mailbox_id = ?1 AND uid = ?2",
                    )?
                    .exists(params![mailbox_id, uid])?;
                if deja {
                    continue;
                }
                tx.prepare_cached(
                    "INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (?1, ?2, ?3)",
                )?
                .execute(params![mailbox_id, uid, action.to_kind()])?;
                retraits.push((mailbox_id, uid));
            }
        }
        poser_verdict(&tx, &adresse, destination, regle, epoch)?;
        tx.execute(
            "UPDATE nettoyage_session SET traites = traites + 1 WHERE id = 1",
            [],
        )?;
        tx.commit()?;
        // Le retrait local APRÈS le commit (patron E3) : l'intention est
        // en base, un crash ici ne perd rien — la copie locale partira à
        // la réconciliation suivante. En UNE transaction (revue
        // 2026-08-30 : un retrait par autocommit payait un fsync par
        // message — des secondes sur un gros groupe, sous le verrou des
        // commandes).
        let traites = retraits.len();
        if !retraits.is_empty() {
            let tx = self.0.unchecked_transaction()?;
            // UNE fois par fil touché, jamais par message (PLAN-AUDIT-V2
            // E4, le patron de `remove_absent`) : un groupe de N messages
            // d'un même expéditeur vit souvent dans quelques fils —
            // `remove_local` par message rafraîchissait chacun N fois.
            let mut touched: BTreeSet<i64> = BTreeSet::new();
            for (mailbox_id, uid) in retraits {
                if let Some(thread) = purger_message(&tx, mailbox_id, uid)? {
                    touched.insert(thread);
                }
            }
            for thread in &touched {
                thread::refresh(&tx, *thread)?;
            }
            tx.commit()?;
        }
        Ok(traites)
    }

    /// Le courrier d'un groupe, dans la plage et le périmètre de la
    /// session — la lecture que l'écran de tri offre quand on entre
    /// dans un groupe (voir, jamais trier au message : le verdict
    /// reste au groupe, refus de périmètre du PLAN). Le plus récent en
    /// tête. Vide sans session.
    pub fn nettoyage_messages(&self, address: &str) -> Result<Vec<UnifiedRow>, Error> {
        let Some(session) = self.nettoyage_etat()? else {
            return Ok(Vec::new());
        };
        let Some(adresse) = adresse_images(Some(address.to_string())) else {
            return Ok(Vec::new());
        };
        let ids = self.boites_du_perimetre(&session.perimetre)?;
        // LE critère partagé : la vue d'un groupe montre exactement ce
        // que le verdict traitera.
        let sql = Self::nettoyage_messages_sql(&ids);
        let mut stmt = self.0.prepare(&sql)?;
        let rangs = stmt
            .query_map(params![session.borne_epoch, adresse], row_to_threaded)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rangs)
    }

    /// Clôt la session (la progression s'efface ; les verdicts, eux,
    /// restent posés — ils vivent dans le routage).
    pub fn nettoyage_terminer(&self) -> Result<(), Error> {
        self.0
            .execute("DELETE FROM nettoyage_session WHERE id = 1", [])?;
        Ok(())
    }

    /// Les messages d'une conversation, du plus ancien au plus récent —
    /// l'ordre de lecture d'un échange.
    /// Les messages d'un fil, en TROIS colonnes (compte, boîte, UID) —
    /// ce qu'un geste de masse a besoin de savoir, sans hydrater les
    /// rangées entières (revue de la vague 2 : `thread_messages` joignait
    /// corps et fils pour trois scalaires).
    pub fn messages_du_fil(&self, thread_id: i64) -> Result<Vec<(i64, String, Uid)>, Error> {
        let mut stmt = self.0.prepare_cached(
            "SELECT m.account_id, m.name, e.uid FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE e.thread_id = ?1 ORDER BY e.date_epoch DESC, e.uid DESC",
        )?;
        let rows = stmt
            .query_map([thread_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn thread_messages(&self, thread_id: i64) -> Result<Vec<UnifiedRow>, Error> {
        // Jointure sur `threads`, et non le mapping « message seul » :
        // chaque message doit repartir en connaissant la taille de SON
        // fil. Sans elle il vaudrait 1, et l'écran conclurait qu'il n'y a
        // pas de conversation à montrer — au moment précis où on la
        // parcourt.
        let mut stmt = self.0.prepare(&format!(
            "{SELECT_UNIFIED}{THREAD_AGGREGATE}
             FROM envelopes e
             JOIN threads t ON t.id = e.thread_id
             JOIN mailboxes m ON m.id = e.mailbox_id
             JOIN accounts a ON a.id = m.account_id
             LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
             WHERE e.thread_id = ?1
             ORDER BY e.date_epoch ASC, e.uid ASC"
        ))?;
        let rows = stmt
            .query_map([thread_id], row_to_threaded)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Une enveloppe précise — le contexte nécessaire pour répondre
    /// (adresse brute de l'expéditeur, Message-ID du fil).
    /// Le `Reply-To` d'un message, s'il en porte un — lu à la demande
    /// (« Répondre »), jamais dans les lignes de liste.
    pub fn reply_to_de(
        &self,
        account_id: i64,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Option<String>, Error> {
        Ok(self
            .0
            .query_row(
                "SELECT e.reply_to FROM envelopes e
                 JOIN mailboxes m ON m.id = e.mailbox_id
                 WHERE m.account_id = ?1 AND m.name = ?2 AND e.uid = ?3",
                params![account_id, mailbox, uid],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten()
            .map(|adresse| adresse.trim().to_string())
            .filter(|adresse| !adresse.is_empty()))
    }

    pub fn envelope(
        &self,
        account_id: i64,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Option<Envelope>, Error> {
        let envelope = self
            .0
            .query_row(
                "SELECT e.uid, e.subject, e.sender, e.sender_address, e.message_id,
                        e.date_epoch, e.seen, e.flagged, e.in_reply_to, e.to_addrs, e.cc_addrs
                 FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
                 WHERE m.account_id = ?1 AND m.name = ?2 AND e.uid = ?3",
                params![account_id, mailbox, uid],
                row_to_envelope,
            )
            .optional()?;
        Ok(envelope)
    }

    /// La chaîne `References` qu'une réponse à ce message doit porter
    /// (RFC 5322 §3.6.4) : les `References` du parent + son `Message-ID`.
    /// `None` : message inconnu ou sans Message-ID. E7 : avant, l'envoi ne
    /// portait que le parent et cassait le fil chez le destinataire.
    pub fn references_de(
        &self,
        account_id: i64,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Option<String>, Error> {
        let ligne: Option<(Option<String>, Option<String>)> = self
            .0
            .query_row(
                "SELECT e.refs, e.message_id
                 FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
                 WHERE m.account_id = ?1 AND m.name = ?2 AND e.uid = ?3",
                params![account_id, mailbox, uid],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(ligne.and_then(|(refs, message_id)| {
            let message_id = message_id?;
            let refs = refs.unwrap_or_default();
            let refs = refs.trim();
            Some(if refs.is_empty() {
                message_id
            } else {
                format!("{refs} {message_id}")
            })
        }))
    }

    pub fn count(&self, mailbox_id: i64) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM envelopes WHERE mailbox_id = ?1",
            [mailbox_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    pub fn max_uid(&self, mailbox_id: i64) -> Result<Uid, Error> {
        let max: Uid = self.0.query_row(
            "SELECT COALESCE(MAX(uid), 0) FROM envelopes WHERE mailbox_id = ?1",
            [mailbox_id],
            |row| row.get(0),
        )?;
        Ok(max)
    }

    /// Combien d'enveloppes d'une boîte portent un UID STRICTEMENT
    /// au-dessus du repère — les ARRIVÉES d'une relève qui vient de se
    /// solder (PLAN-REACTIVITE E4, terrain du 2026-08-14). Le `fetched`
    /// du rapport ne sait pas les compter : un delta CONDSTORE y mêle
    /// tous les drapeaux glissés — et Gmail fait glisser le modseq à
    /// chaque étiquette. Seul l'UID sépare le neuf du retouché.
    pub fn arrivees_depuis(&self, account_id: i64, mailbox: &str, uid: Uid) -> Result<u64, Error> {
        let Some(state) = self.sync_state(account_id, mailbox)? else {
            return Ok(0);
        };
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM envelopes WHERE mailbox_id = ?1 AND uid > ?2",
            params![state.mailbox_id, uid],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }
}

/// Fait évoluer en place une base d'une version précédente : les colonnes
/// s'ajoutent sans perdre ce qui est déjà là, et la bascule multi-comptes
/// (Phase 3) reconstruit les tables dont les contraintes changent.
/// Configuration serveur d'un compte IMAP/SMTP générique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountConfig {
    pub imap_host: Option<String>,
    pub imap_port: Option<u16>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub username: Option<String>,
}

/// Le prédicat « ce message attend encore son corps », partagé par le
/// COMPTE ([`Store::bodies_pending_count`]) et la LISTE de travail
/// ([`Store::bodies_to_backfill`]).
///
/// UNE écriture : les deux ne peuvent plus diverger — et c'est cette
/// écriture-là, jamais une copie, que la garde de plan interroge (même
/// raison qu'[`unified_page_sql`], et même leçon payée).
///
/// **Il ne lit AUCUNE colonne de `bodies`, et c'est tout le chantier.**
/// L'existence de la ligne se tranche dans l'auto-index de la clé
/// primaire `(mailbox_id, uid)` — donc sans jamais rappeler la ligne,
/// qui pèse 56 ko en moyenne au terrain. Y lire ne serait-ce qu'un bit
/// coûtait 251 k lectures aléatoires dans 11,4 Go : **20 839 ms à froid
/// contre 396 ms sans** (mesuré le 2026-08-26 sur la base du terrain).
///
/// Ce prédicat portait `AND b.scanned = 1` — la trace des corps
/// rapatriés AVANT que les pièces jointes n'existent, dont le MIME
/// n'avait jamais été inspecté. **Retiré le 2026-08-26 (PLAN-DEMARRAGE,
/// décision D8)** sur trois faits mesurés : la production n'écrit
/// JAMAIS `scanned = 0` ([`Store::save_body_full`] pose un `1` en dur),
/// les deux postes de la flotte portent **zéro** ligne à `scanned = 0`,
/// et le critère coûtait le gel de 8 870 ms du démarrage pour protéger
/// zéro ligne. La colonne survit, vestigiale : la retirer demanderait
/// une réécriture de 11,4 Go — elle partira avec le chantier qui
/// touchera `bodies` de toute façon (l'aperçu, dette).
///
/// **Exige l'alias `e`** pour `envelopes` chez qui l'emploie — comme
/// [`SELECT_UNIFIED`] exige les siens. Le fragment est une chaine : un
/// autre alias compile et echoue au `prepare`, sur un chemin dont l'UI
/// n'affiche rien (le `catch` du rattrapage est un `console.error`).
pub(crate) const CORPS_ABSENT: &str = "NOT EXISTS (
                   SELECT 1 FROM bodies b
                    WHERE b.mailbox_id = e.mailbox_id AND b.uid = e.uid
               )";

/// Le COMPTE des corps manquants d'une boîte : `?1` le compte, `?2` la
/// boîte, `?3` l'horizon.
pub(crate) fn bodies_pending_count_sql() -> String {
    format!(
        "SELECT COUNT(*)
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND (e.date_epoch IS NULL OR e.date_epoch >= ?3)
               AND {CORPS_ABSENT}"
    )
}

/// La LISTE de travail du rattrapage — mêmes paramètres, plus `?4`, la
/// borne du lot.
pub(crate) fn bodies_to_backfill_sql() -> String {
    format!(
        "SELECT e.uid
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND (e.date_epoch IS NULL OR e.date_epoch >= ?3)
               AND {CORPS_ABSENT}
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT ?4"
    )
}

/// La requête d'une page de la boîte unifiée.
///
/// Isolée pour qu'un test puisse interroger **son** plan d'exécution, et
/// non une copie qui divergerait le jour où l'une des deux change. Le
/// coût de cette requête est le chemin le plus chaud du produit.
/// `organise` (E2) : la Réception ORGANISÉE — le MÊME squelette plus le
/// drapeau de rétention, sous la forme EXACTE de l'index partiel
/// `idx_threads_date_organise` qui porte alors tri, filtre et
/// pagination (S2-bis : l'offset saute des entrées d'index, jamais des
/// lignes sondées). UNE écriture pour les deux modes — la revue E1
/// avait isolé cette requête précisément pour qu'aucune copie ne
/// diverge.
pub(crate) fn unified_page_sql(par_compte: bool, non_lues: bool, organise: bool) -> String {
    // La pagination (`LIMIT`/`OFFSET`) s'applique dans une sous-requête
    // sur `threads` SEUL, pas sur la jointure : `OFFSET` produit puis
    // jette chaque ligne sautée, donc tout ce qui se calcule par ligne —
    // la triple jointure et l'`EXISTS` corrélé sur `attachments` de
    // SELECT_UNIFIED — se payait pour les 200 000 lignes d'un saut
    // profond. Mesuré (gate P1 de la refonte, 205 050 conversations) :
    // 252,6 ms à l'offset 200 000, croissance linéaire. Avec le squelette
    // en sous-requête, le saut ne parcourt que l'index partiel
    // `idx_threads_date_globale` — qui porte la clé de tri COMPLÈTE
    // (last_epoch DESC, last_uid DESC, account_id) et le filtre
    // `inbox_size > 0` — et les jointures ne s'exécutent que sur les
    // `limit` lignes retenues.
    //
    // Le ORDER BY externe re-trie les lignes retenues avec la même clé :
    // il garantit l'ordre final quelle que soit la stratégie de jointure,
    // pour le prix d'un tri de `limit` lignes.
    // `par_compte` ajoute le filtre `account_id = ?3` de la nav v2
    // (« Boîtes » de l'écran 02) : même squelette, l'index préfixé
    // `idx_threads_date (account_id, …)` porte alors tri et pagination.
    // `non_lues` est l'onglet « Non lus » du prototype — filtré ICI, pas
    // côté client : 331 conversations sur 2 929 au terrain, une page ne
    // doit transporter que ce qu'elle affiche.
    let filtre = if par_compte {
        " AND account_id = ?3"
    } else {
        ""
    };
    let non_lues_seulement = if non_lues { " AND unseen > 0" } else { "" };
    // E5 : en mode organisé, les fils MIS DE CÔTÉ quittent le flot —
    // ils vivent dans la pile (exclusion partagée, patron pins). Le
    // classique n'exclut rien.
    let retenue = if organise {
        exclusion_organisee()
    } else {
        String::new()
    };
    // E4 : l'ordre INTERNE (celui que l'index partiel porte) suit les
    // sections en mode organisé — même clé que la queue de jointures.
    let tri = if organise {
        "ORDER BY (unseen > 0) DESC, last_epoch DESC, last_uid DESC, account_id"
    } else {
        "ORDER BY last_epoch DESC, last_uid DESC, account_id"
    };
    let queue = unified_join_tail(organise);
    // R4 (PLAN-RETOURS-7, D5) : les conversations ÉPINGLÉES quittent le
    // flot paginé — elles se servent À PART, en tête de page 0
    // (`pinned_unified_scoped`) ; la liste ne montre jamais deux fois le
    // même message. `NOT IN` sur la sous-requête des épingles : liste
    // matérialisée une fois, minuscule par construction.
    format!(
        "{SELECT_UNIFIED}{THREAD_AGGREGATE}
         FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
                 FROM threads
                WHERE inbox_size > 0{retenue} AND id NOT IN ({PINNED_THREADS}){filtre}{non_lues_seulement}
                {tri}
                LIMIT ?1 OFFSET ?2) t{queue}"
    )
}

/// Le calcul du drapeau `organise_hors` d'UN fil (E2) — LE fragment
/// partagé par `thread::refresh` (entretien) et le rattrapage de
/// migration : une seule écriture de la règle, jamais deux copies qui
/// divergent. `param_fil` désigne le fil (paramètre ou colonne).
///
/// Règle d'or (revue E2) — jamais perdre de courrier :
/// - un expéditeur routé vers une VUE (kiosque/registre) éjecte le fil
///   dès UN message — le fil vit dans sa vue (miroir de
///   [`fil_route_sql`]), rien n'est perdu ;
/// - un écarté ou un expéditeur en attente n'a PAS de vue : le fil ne
///   se cache que s'il est ENTIÈREMENT à eux — un fil mêlé (un intrus
///   écarté répond dans le fil d'un connu) RESTE en Réception.
///
/// Premier WHEN : les deux tables vides (mode jamais employé) — deux
/// sondes O(1), l'adoption d'une base héritée ne paie rien.
pub(crate) fn organise_hors_sql(param_fil: &str) -> String {
    format!(
        "CASE
           WHEN NOT EXISTS (SELECT 1 FROM routage_expediteurs LIMIT 1)
            AND NOT EXISTS (SELECT 1 FROM portier_attente LIMIT 1) THEN 0
           WHEN EXISTS (
             SELECT 1 FROM envelopes te
               JOIN routage_expediteurs r
                 ON r.address = te.sender_norm
                AND r.destination IN ('kiosque', 'registre')
              WHERE te.thread_id = {param_fil}) THEN 1
           WHEN NOT EXISTS (
             SELECT 1 FROM envelopes o
              WHERE o.thread_id = {param_fil}
                AND NOT EXISTS (SELECT 1 FROM portier_attente pa
                                 WHERE pa.address = o.sender_norm)
                AND NOT EXISTS (SELECT 1 FROM routage_expediteurs re
                                 WHERE re.address = o.sender_norm
                                   AND re.destination = 'ecarte')) THEN 1
           ELSE 0 END"
    )
}

/// Le filtre du Kiosque et du Registre (PLAN-MODE-ORGANISE E1, revue) :
/// un fil appartient à la destination si N'IMPORTE LEQUEL de ses
/// messages vient d'un expéditeur routé là — jamais la seule TÊTE, qui
/// est le dernier message toutes boîtes confondues : y répondre la
/// déplace en Envoyés et le fil s'éjectait de sa destination (prouvé
/// RED). Sonde par `idx_envelopes_thread` puis PK routage (spike S2),
/// posée DANS le squelette paginé — jamais après le LIMIT (pages
/// courtes, réserve S2). `sender_norm` (colonne générée, E2) EST le
/// `lower(trim(sender_address))` d'origine — une seule expression,
/// définie une fois ; sa divergence avec `adresse_images` (Rust) sur
/// le non-ASCII reste la limite assumée E1 : une adresse réelle est
/// ASCII.
pub(crate) fn fil_route_sql(param_destination: &str) -> String {
    format!(
        "EXISTS (
                   SELECT 1 FROM envelopes te
                     JOIN routage_expediteurs r
                       ON r.address = te.sender_norm
                      AND r.destination = {param_destination}
                    WHERE te.thread_id = threads.id
               )"
    )
}

/// La page du Kiosque/Registre — le squelette EXACT de
/// [`unified_page_sql`] plus [`fil_route_sql`] : même tri, mêmes
/// jointures. Les ÉPINGLÉES ne sont PAS exclues (revue E1) : leur
/// section préposée n'existe qu'en Réception — les exclure ici ferait
/// disparaître un fil épinglé routé de TOUTES les vues organisées.
/// `?1` limit, `?2` offset, `?3` destination, `?4` compte (si
/// `par_compte`).
pub(crate) fn routage_page_sql(par_compte: bool, non_lues: bool) -> String {
    let filtre = if par_compte {
        " AND account_id = ?4"
    } else {
        ""
    };
    let non_lues_seulement = if non_lues { " AND unseen > 0" } else { "" };
    let fil_route = fil_route_sql("?3");
    let queue = unified_join_tail(false);
    // E5 : le Kiosque et le Registre sont des vues ORGANISÉES — un fil
    // mis de côté les quitte aussi (il vit dans la pile).
    let hors_pile = format!(" AND id NOT IN ({MIS_DE_COTE_THREADS})");
    format!(
        "{SELECT_UNIFIED}{THREAD_AGGREGATE}
         FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
                 FROM threads
                WHERE inbox_size > 0
                  AND {fil_route}{hors_pile}{filtre}{non_lues_seulement}
                ORDER BY last_epoch DESC, last_uid DESC, account_id
                LIMIT ?1 OFFSET ?2) t{queue}"
    )
}

fn migrate(
    conn: &Connection,
    on_progress: &mut dyn FnMut(AdoptionProgress) -> ControlFlow<()>,
) -> Result<(), Error> {
    migrate_multi_account(conn)?;
    add_missing_columns(
        conn,
        "drafts",
        &[
            ("account_id", "INTEGER NOT NULL DEFAULT 1"),
            ("reply_to_mailbox", "TEXT"),
        ],
    )?;
    // ADR 0010 : la portee du regroupement devient explicite. Les boites
    // deja en base sont INBOX et « Envoyes » — toutes deux dedans, d'ou
    // le defaut a 1. Une base heritee garde donc exactement les fils
    // qu'elle avait : la migration ne change rien a ce qui est affiche.
    add_missing_columns(
        conn,
        "mailboxes",
        &[("threaded", "INTEGER NOT NULL DEFAULT 1")],
    )?;
    add_missing_columns(conn, "accounts", &[("sent_mailbox", "TEXT")])?;
    add_missing_columns(conn, "folders", &[("special_use", "TEXT")])?;
    add_missing_columns(conn, "mailboxes", &[("relevee_epoch", "INTEGER")])?;
    add_missing_columns(
        conn,
        "mailboxes",
        &[("remote_total", "INTEGER NOT NULL DEFAULT 0")],
    )?;
    // ADR 0017 : le UIDNEXT vu au dernier relevé — NULL tant qu'aucune
    // relève gardée n'a eu lieu, donc une base héritée relève tout à son
    // premier cycle (conservateur), puis devient sobre.
    add_missing_columns(conn, "mailboxes", &[("remote_uidnext", "INTEGER")])?;
    // PLAN-AUDIT-V1 E3 : la quarantaine des actions refusées.
    add_missing_columns(
        conn,
        "pending_actions",
        &[
            ("attempts", "INTEGER NOT NULL DEFAULT 0"),
            ("refusee", "INTEGER NOT NULL DEFAULT 0"),
            ("last_error", "TEXT"),
        ],
    )?;
    // PLAN-AUDIT-V1 E2 : le drapeau d'initialisation. Sur une base
    // héritée, UNE fois, à la pose de la colonne : toute boîte qui a déjà
    // un repère est réputée initialisée — les lignes à 0 gardent le
    // comportement d'avant (première passe = initiale).
    if !table_columns(conn, "mailboxes")?.contains("initialisee") {
        add_missing_columns(
            conn,
            "mailboxes",
            &[("initialisee", "INTEGER NOT NULL DEFAULT 0")],
        )?;
        conn.execute(
            "UPDATE mailboxes SET initialisee = 1 WHERE last_uid > 0",
            [],
        )?;
    }
    add_missing_columns(
        conn,
        "outbox",
        &[
            ("account_id", "INTEGER NOT NULL DEFAULT 1"),
            ("refs", "TEXT"),
            ("reply_to", "TEXT"),
        ],
    )?;
    add_missing_columns(
        conn,
        "envelopes",
        &[
            ("sender_address", "TEXT"),
            ("message_id", "TEXT"),
            ("flagged", "INTEGER NOT NULL DEFAULT 0"),
            ("in_reply_to", "TEXT"),
            ("refs", "TEXT"),
            // NULL = « pas encore rattaché ». C'est ce que
            // `thread::migrate_threads` cherche, plus bas.
            ("thread_id", "INTEGER"),
            // R4 : les destinataires arrivent NULL sur l'existant — le
            // rattrapage des envois (D2) les peuple, la synchro les ecrit
            // desormais sur tout message neuf.
            ("to_addrs", "TEXT"),
            ("cc_addrs", "TEXT"),
        ],
    )?;
    add_missing_columns(
        conn,
        "drafts",
        &[
            ("remote_uid", "INTEGER"),
            ("pushed_epoch", "INTEGER"),
            // Cc/Cci d'un brouillon — vides sur l'existant (PLAN-RETOURS-2).
            ("cc_raw", "TEXT NOT NULL DEFAULT ''"),
            ("bcc_raw", "TEXT NOT NULL DEFAULT ''"),
            // Corps riche — NULL sur l'existant, chemin texte intact
            // (PLAN-COMPOSITION-HTML).
            ("body_html", "TEXT"),
        ],
    )?;
    // Cc/Cci du journal d'envoi — vides sur l'existant (PLAN-RETOURS-2).
    add_missing_columns(
        conn,
        "outbox",
        &[
            ("cc_addrs", "TEXT NOT NULL DEFAULT ''"),
            ("bcc_addrs", "TEXT NOT NULL DEFAULT ''"),
            // Corps riche — NULL sur l'existant (PLAN-COMPOSITION-HTML).
            ("body_html", "TEXT"),
        ],
    )?;
    // Les corps deja en base valent 0 : ils datent d'avant les pieces
    // jointes, et le rattrapage devra les relire une fois.
    add_missing_columns(conn, "bodies", &[("scanned", "INTEGER NOT NULL DEFAULT 0")])?;
    // Destinataires de l'echo — NULL sur l'existant (PLAN-RETOURS-5).
    add_missing_columns(conn, "echos", &[("to_addrs", "TEXT")])?;
    // « Important » et envoi différé (PLAN-RETOURS-6) : l'existant
    // n'est ni marqué ni programmé.
    add_missing_columns(
        conn,
        "drafts",
        &[("important", "INTEGER NOT NULL DEFAULT 0")],
    )?;
    add_missing_columns(
        conn,
        "outbox",
        &[
            ("important", "INTEGER NOT NULL DEFAULT 0"),
            ("send_at_epoch", "INTEGER"),
        ],
    )?;
    // Réponse iTIP (PLAN-INVITATIONS) — NULL sur l'existant, chemin
    // d'envoi historique inchangé.
    add_missing_columns(conn, "outbox", &[("ics_reply", "TEXT")])?;
    // Le lien d'annulation croisé (terrain R6) — les bases nées pendant
    // le chantier ont la table sans la colonne.
    add_missing_columns(
        conn,
        "invitations",
        &[("annule", "INTEGER NOT NULL DEFAULT 0")],
    )?;
    // L'aperçu de liste (écran 02 de la refonte) se calcule à l'ÉCRITURE
    // du corps ; les corps antérieurs le rattrapent PAR LOTS
    // (`preview_catchup`, appelé par le shell au fil du sondage) — jamais
    // sur le chemin d'ouverture ni au défilement. L'index partiel rend la
    // sonde « des retardataires ? » gratuite une fois la passe soldée.
    add_missing_columns(conn, "bodies", &[("preview", "TEXT")])?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_bodies_apercu_manquant
             ON bodies(mailbox_id, uid) WHERE preview IS NULL;",
    )?;
    // L'index de date des enveloppes gagne `uid` (voir le commentaire du
    // SCHEMA). `CREATE INDEX IF NOT EXISTS` ne suffit PAS : sur une base
    // existante l'index porte deja ce nom, la creation est un no-op muet
    // et le defaut survivrait. On lit donc sa DEFINITION et on le
    // reconstruit s'il lui manque la colonne — meme patron que la sonde
    // `recipients` de l'index de recherche.
    //
    // Sans ecran : la reconstruction ne lit que `envelopes` (47 Mo au
    // terrain), jamais les corps — 0,332 s mesurees sur la base du CE,
    // contre les 18 s qu'aurait coutees un index sur `bodies`. C'est
    // toute la difference entre une migration muette acceptable et le gel
    // du 2026-08-17.
    //
    // La relecture et la reconstruction vivent dans UNE transaction, et
    // ce n'est pas de la prudence de principe (revue a regard neuf du
    // 2026-08-26) : `connect_accounts` appelle `Store::open` DIRECTEMENT,
    // hors du verrou global des commandes (commands.rs), donc deux
    // `migrate()` tournent pour de vrai en parallele au demarrage. Sans
    // transaction, les deux lisent l'index a deux colonnes avant que l'un
    // n'ecrive, et le reconstruisent chacun leur tour : ~3,5 s de gel au
    // lieu de 1,77 s. `BEGIN IMMEDIATE` prend le verrou d'ecriture des la
    // lecture — le second arrivant relit APRES le premier, trouve `uid`,
    // et ne fait rien.
    // DOUBLE VERIFICATION, et le premier temps compte autant que le
    // second : `migrate()` tourne a CHAQUE `Store::open`, donc des
    // dizaines de fois par demarrage. Une lecture nue de `sqlite_master`
    // ne prend aucun verrou ; ouvrir une transaction d'ecriture juste
    // pour verifier couterait le verrou d'ecriture a chaque commande.
    reconstruire_index_si_ancien(
        conn,
        "idx_envelopes_date",
        "uid",
        "CREATE INDEX idx_envelopes_date ON envelopes(mailbox_id, date_epoch DESC, uid);",
    )?;
    // La sonde d'exclusion des intégrales (nav, catégorie Archives sur
    // Gmail) cherche par message_id : sans cet index, chaque ligne de
    // « Tous les messages » paierait un parcours de table.
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_envelopes_message
             ON envelopes(message_id) WHERE message_id IS NOT NULL;",
    )?;
    // Réparation des aperçus extraits par le premier décodeur, qui
    // laissait passer les entités numériques (&#233;) et nommées
    // (&eacute;, &zwnj;…) — défaut vu au terrain. Remettre à NULL suffit :
    // le rattrapage par lots les recalcule avec le décodeur complet, hors
    // du chemin d'ouverture. Le critère est LE scanner du décodeur
    // lui-même (pas un motif SQL approximatif). UNE seule passe, tenue
    // par un marqueur : un corps double-encodé (« &amp;gt; ») produit
    // légitimement « &gt; » dans l'aperçu neuf — sans le marqueur, la
    // réparation le remettrait à NULL à chaque ouverture, pour rien.
    conn.execute_batch("CREATE TABLE IF NOT EXISTS reparations (nom TEXT PRIMARY KEY);")?;
    let deja_faite: bool = conn
        .prepare("SELECT 1 FROM reparations WHERE nom = 'apercus-entites'")?
        .exists([])?;
    if !deja_faite {
        let mut stmt = conn.prepare(
            "SELECT mailbox_id, uid, preview FROM bodies
                 WHERE preview IS NOT NULL AND preview LIKE '%&%'",
        )?;
        let pollues: Vec<(i64, u32)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .filter_map(Result::ok)
            .filter(|(_, _, p)| crate::body::contient_entite_residuelle(p))
            .map(|(m, u, _)| (m, u))
            .collect();
        drop(stmt);
        for (mailbox_id, uid) in pollues {
            conn.execute(
                "UPDATE bodies SET preview = NULL WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
            )?;
        }
        conn.execute_batch("INSERT INTO reparations (nom) VALUES ('apercus-entites');")?;
    }
    // Réparation des corps mutilés au décodage — défaut vu au terrain
    // (25 corps sur la base de mesure). Deux causes, corrigées côté
    // mail-imap : les charsets multi-octets (gb2312…) exigeaient la
    // feature `full_encoding` de mail-parser, et un charset absent
    // tombait en UTF-8 avec remplacement au lieu du windows-1252 de fait.
    // Supprimer la ligne suffit : le rattrapage (`bodies_to_backfill`)
    // retélécharge tout message sans corps, et `save_body` refait au
    // passage l'aperçu, l'index de recherche et les pièces. Les U+FFFD
    // authentiques (envoyés tels quels) reviendront identiques — c'est un
    // retéléchargement pour rien, mais UNE seule fois, tenu par le
    // marqueur.
    let deja_faite: bool = conn
        .prepare("SELECT 1 FROM reparations WHERE nom = 'corps-fffd'")?
        .exists([])?;
    if !deja_faite {
        conn.execute_batch(
            "DELETE FROM bodies WHERE html LIKE '%' || char(65533) || '%';
             INSERT INTO reparations (nom) VALUES ('corps-fffd');",
        )?;
    }
    // Réparation des messages à partie calendrier scannés AVANT
    // PLAN-INVITATIONS. Deux raisons, un seul remède : (1) le filtre
    // `est_calendrier_inline` (mail-imap) a changé la numérotation des
    // pièces — les `idx` stockés comptaient la partie calendrier, la
    // relecture des octets ne la compte plus : cliquer une pièce
    // servirait le MAUVAIS fichier en silence ; (2) ces messages n'ont
    // pas de ligne `invitations` — leur carte doit naître (adoption,
    // invariant §6.7). Supprimer corps ET lignes de pièces suffit : le
    // rattrapage (`bodies_to_backfill`) relit le message, et
    // `save_body_full` refait pièces (indices neufs), aperçu, index de
    // recherche et invitation d'un coup. UNE fois, tenu par le marqueur.
    let deja_faite: bool = conn
        .prepare("SELECT 1 FROM reparations WHERE nom = 'pieces-calendrier'")?
        .exists([])?;
    if !deja_faite {
        conn.execute_batch(
            "CREATE TEMP TABLE reparation_calendrier AS
                 SELECT DISTINCT mailbox_id, uid FROM attachments
                 WHERE mime IN ('text/calendar', 'application/ics')
                    OR LOWER(name) LIKE '%.ics';
             DELETE FROM bodies WHERE (mailbox_id, uid) IN
                 (SELECT mailbox_id, uid FROM reparation_calendrier);
             DELETE FROM attachments WHERE (mailbox_id, uid) IN
                 (SELECT mailbox_id, uid FROM reparation_calendrier);
             DROP TABLE reparation_calendrier;
             INSERT INTO reparations (nom) VALUES ('pieces-calendrier');",
        )?;
    }
    // R2 (PLAN-RETOURS-MAIL) : les enveloppes synchronisées AVANT le
    // correctif portent les backslash-escapes des `quoted-string` IMAP que
    // `imap-proto` laisse dans le contenu (objet « Test \"Envoyés\" », nom
    // d'expéditeur, adresse). Le décodage neuf les retire à la synchro,
    // mais l'existant reste parasité : on le répare UNE fois. Le contenu
    // stocké est déjà RFC 2047-décodé ; il ne reste que la couche d'escape
    // IMAP, donc dé-échapper la valeur stockée équivaut au nouveau décodage
    // (un encoded-word ne porte ni `"` ni `\`). L'index FTS n'a pas à
    // bouger : son tokeniseur écarte déjà le backslash, la recherche
    // donnait les mêmes résultats. char(92) = `\`.
    let deja_faite: bool = conn
        .prepare("SELECT 1 FROM reparations WHERE nom = 'objets-escapes'")?
        .exists([])?;
    if !deja_faite {
        let mut stmt = conn.prepare(
            "SELECT mailbox_id, uid, subject, sender, sender_address FROM envelopes
                 WHERE instr(subject, char(92)) > 0
                    OR instr(sender, char(92)) > 0
                    OR instr(sender_address, char(92)) > 0",
        )?;
        #[allow(clippy::type_complexity)]
        let parasites: Vec<(i64, u32, Option<String>, Option<String>, Option<String>)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .filter_map(Result::ok)
            .collect();
        drop(stmt);
        for (mailbox_id, uid, subject, sender, sender_address) in parasites {
            let propre =
                |v: Option<String>| v.map(|s| crate::unescape_imap_quoted_str(&s).into_owned());
            conn.execute(
                "UPDATE envelopes SET subject = ?3, sender = ?4, sender_address = ?5
                     WHERE mailbox_id = ?1 AND uid = ?2",
                params![
                    mailbox_id,
                    uid,
                    propre(subject),
                    propre(sender),
                    propre(sender_address),
                ],
            )?;
        }
        conn.execute_batch("INSERT INTO reparations (nom) VALUES ('objets-escapes');")?;
    }
    add_missing_columns(
        conn,
        "accounts",
        &[
            ("imap_host", "TEXT"),
            ("imap_port", "INTEGER"),
            ("smtp_host", "TEXT"),
            ("smtp_port", "INTEGER"),
            ("username", "TEXT"),
        ],
    )?;
    search::migrate_search(conn, on_progress)?;
    // L'index vient APRÈS `add_missing_columns`, pas dans `SCHEMA` : sur
    // une base héritée, `CREATE TABLE IF NOT EXISTS envelopes` ne fait
    // rien et la colonne `thread_id` n'existe pas encore au moment où le
    // schéma s'exécute. Deux tests de migration l'ont prouvé.
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_envelopes_thread
             ON envelopes(thread_id, date_epoch DESC);",
    )?;
    // L'adresse d'expéditeur NORMALISÉE, en colonne générée (Mode
    // organisé E2, spike S2-bis) : SQLite n'emploie un index
    // d'EXPRESSION que contre un littéral — dans une jointure
    // (`= r.address`), il scanne (2,3 s mesurées à 200 k). La colonne
    // VIRTUAL ne stocke rien (ALTER 14 ms) ; l'index réel (188 ms à
    // 200 k, une fois) rend SEARCH toutes les sondes par expéditeur du
    // routage et du Portier. Même expression que `fil_route_sql` —
    // divergence connue avec `adresse_images` (Rust) sur le non-ASCII,
    // limite assumée E1 : une adresse réelle est ASCII.
    add_missing_columns(
        conn,
        "envelopes",
        &[(
            "sender_norm",
            "TEXT GENERATED ALWAYS AS (lower(trim(sender_address))) VIRTUAL",
        )],
    )?;
    // Trois colonnes (PLAN-AUDIT-V2 E4) : l'agrégat du Nettoyage est
    // COUVERT — expéditeur, date, boîte — sans lire une ligne de table ;
    // les sondes par expéditeur (Portier, stock d'un verdict) le servent
    // toujours par son préfixe. Une base du parc portait l'index à deux
    // colonnes : reconstruit, même patron que l'index de date.
    let creation = format!(
        "CREATE INDEX {INDEX_EXPEDITEURS} ON envelopes(sender_norm, date_epoch, mailbox_id);"
    );
    conn.execute_batch(&creation.replace("CREATE INDEX", "CREATE INDEX IF NOT EXISTS"))?;
    reconstruire_index_si_ancien(conn, INDEX_EXPEDITEURS, "mailbox_id", &creation)?;
    // Le drapeau de rétention des fils (E2, verdict S2-bis : V4 —
    // entretenu par `thread::refresh` comme `size`/`unseen`, servi par
    // l'index partiel miroir). Sur une base héritée, `threads` existe
    // déjà sans la colonne — et son index partiel, créé par
    // `thread::SCHEMA` APRÈS ce point, échouerait sans elle : c'est le
    // piège documenté de `drop_if_outdated`. Une base neuve n'a pas
    // encore la table : le schéma des fils la crée complète.
    // E4 : l'index de la Réception organisée gagne les SECTIONS dans sa
    // clé — un index d'E2 (sans l'expression `unseen`) ne porterait
    // plus le tri et chaque page paierait un tri matérialisé (S1 :
    // 548 ms). Même patron que la reconstruction d'idx_envelopes_date :
    // le nom ne suffit pas, on lit la DÉFINITION. Le schéma des fils
    // (appliqué après) recrée la forme neuve.
    let organise_sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master
              WHERE type = 'index' AND name = 'idx_threads_date_organise'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if organise_sql.is_some_and(|sql| !sql.contains("unseen")) {
        conn.execute_batch("DROP INDEX idx_threads_date_organise;")?;
    }
    let colonnes_threads = table_columns(conn, "threads")?;
    if colonnes_threads.contains("id") && !colonnes_threads.contains("organise_hors") {
        add_missing_columns(
            conn,
            "threads",
            &[("organise_hors", "INTEGER NOT NULL DEFAULT 0")],
        )?;
        // Rattrapage UNIQUE d'une base d'AVANT E2 où le mode a déjà
        // servi (terrain E1 : l'époque a pu être gravée et des inconnus
        // arriver AVANT cette mise à jour — sans rattrapage ils
        // passeraient le guichet pour toujours, en silence). D'abord
        // l'attente (la définition de l'arrivée, rejouée sur le stock :
        // 21 ms mesurées à 200 k), puis les drapeaux des fils touchés,
        // par LE fragment partagé — jamais une copie de la règle.
        let epoque: Option<i64> = conn
            .query_row(
                "SELECT value FROM prefs WHERE key = 'mode_organise_epoch'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .and_then(|v| v.parse().ok());
        if let Some(epoque) = epoque {
            conn.execute(
                "INSERT OR IGNORE INTO portier_attente (address)
                 SELECT e.sender_norm FROM envelopes e
                   JOIN mailboxes m ON m.id = e.mailbox_id AND m.name = ?2
                  WHERE (e.date_epoch > ?1 OR e.date_epoch IS NULL)
                    AND e.sender_norm IS NOT NULL
                  GROUP BY e.sender_norm
                 HAVING NOT EXISTS (SELECT 1 FROM routage_expediteurs r
                                     WHERE r.address = e.sender_norm)
                    AND NOT EXISTS (SELECT 1 FROM envelopes v
                                     WHERE v.sender_norm = e.sender_norm
                                       AND v.date_epoch <= ?1)
                    AND NOT EXISTS (SELECT 1 FROM accounts a
                                     WHERE lower(trim(a.email)) = e.sender_norm)",
                params![epoque, thread::RECEIVED_MAILBOX],
            )?;
        }
        conn.execute(
            &format!(
                "UPDATE threads SET organise_hors = {}
                  WHERE id IN (
                    SELECT DISTINCT te.thread_id FROM envelopes te
                     WHERE te.thread_id IS NOT NULL
                       AND (EXISTS (SELECT 1 FROM routage_expediteurs r
                                     WHERE r.address = te.sender_norm)
                            OR EXISTS (SELECT 1 FROM portier_attente pa
                                        WHERE pa.address = te.sender_norm)))",
                organise_hors_sql("threads.id")
            ),
            [],
        )?;
    }
    // L'adoption des fils ne vit PAS ici : elle appartient à l'unité
    // transactionnelle de `init_with`, pour être rembobinable (§8). Elle
    // vient après ce module — la colonne et l'index doivent exister avant
    // d'adopter les messages hérités.
    Ok(())
}

/// Bascule Phase 2 → 3 : les contraintes de trois tables changent
/// (UNIQUE et clés par compte) — SQLite exige une reconstruction. Les
/// données existantes sont adoptées par un compte « en attente » (email
/// vide) que la première connexion revendiquera : en pratique, le même
/// compte Gmail qu'avant la mise à jour. Zéro perte, prouvé par test.
fn migrate_multi_account(conn: &Connection) -> Result<(), Error> {
    if table_columns(conn, "mailboxes")?.contains("account_id") {
        return Ok(());
    }
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;
         BEGIN;
         INSERT INTO accounts (id, email, provider) VALUES (1, '', 'gmail');

         CREATE TABLE mailboxes_v3 (
             id             INTEGER PRIMARY KEY,
             account_id     INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
             name           TEXT NOT NULL,
             uid_validity   INTEGER NOT NULL,
             last_uid       INTEGER NOT NULL DEFAULT 0,
             highest_modseq INTEGER,
             UNIQUE (account_id, name)
         );
         INSERT INTO mailboxes_v3 (id, account_id, name, uid_validity, last_uid, highest_modseq)
             SELECT id, 1, name, uid_validity, last_uid, highest_modseq FROM mailboxes;
         DROP TABLE mailboxes;
         ALTER TABLE mailboxes_v3 RENAME TO mailboxes;

         CREATE TABLE drafts_remote_v3 (
             account_id   INTEGER PRIMARY KEY,
             uid_validity INTEGER NOT NULL
         );
         INSERT INTO drafts_remote_v3 (account_id, uid_validity)
             SELECT 1, uid_validity FROM drafts_remote;
         DROP TABLE drafts_remote;
         ALTER TABLE drafts_remote_v3 RENAME TO drafts_remote;

         CREATE TABLE draft_tombstones_v3 (
             account_id INTEGER NOT NULL,
             remote_uid INTEGER NOT NULL,
             PRIMARY KEY (account_id, remote_uid)
         );
         INSERT INTO draft_tombstones_v3 (account_id, remote_uid)
             SELECT 1, remote_uid FROM draft_tombstones;
         DROP TABLE draft_tombstones;
         ALTER TABLE draft_tombstones_v3 RENAME TO draft_tombstones;

         COMMIT;
         PRAGMA foreign_keys = ON;",
    )?;
    Ok(())
}

/// L'index des expéditeurs (expéditeur, date, boîte) — nommé UNE fois :
/// les requêtes du Nettoyage l'exigent par `INDEXED BY` (revue : quatre
/// copies du nom, un renommage en aurait oublié une en silence).
pub(crate) const INDEX_EXPEDITEURS: &str = "idx_envelopes_sender";

/// Les champs d'une enveloppe qui vivent dans l'index de recherche —
/// tels que relus en base, pour savoir si une re-synchronisation les a
/// changés (sujet, expéditeur, adresse, destinataires, copies).
type ChampsIndexes = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// Le chemin d'une connexion sur FICHIER — `None` pour une base mémoire
/// (SQLite répond un nom vide), qui ne s'inscrit jamais au registre.
fn cle_fichier(conn: &Connection) -> Option<std::path::PathBuf> {
    conn.path()
        .filter(|chemin| !chemin.is_empty())
        .map(std::path::PathBuf::from)
}

/// Le registre des chemins dont l'initialisation complète a RÉUSSI dans
/// ce processus (PLAN-AUDIT-V2 E1). Un verrou empoisonné est repris :
/// perdre le registre ferait rejouer les migrations, jamais les sauter.
struct RegistreInitialisees(std::sync::Mutex<HashSet<std::path::PathBuf>>);

impl RegistreInitialisees {
    fn contains(&self, cle: &std::path::Path) -> bool {
        self.verrou().contains(cle)
    }

    fn insert(&self, cle: std::path::PathBuf) {
        self.verrou().insert(cle);
    }

    fn verrou(&self) -> std::sync::MutexGuard<'_, HashSet<std::path::PathBuf>> {
        match self.0.lock() {
            Ok(garde) => garde,
            Err(empoisonne) => empoisonne.into_inner(),
        }
    }
}

fn registre_initialisees() -> &'static RegistreInitialisees {
    static REGISTRE: std::sync::OnceLock<RegistreInitialisees> = std::sync::OnceLock::new();
    REGISTRE.get_or_init(|| RegistreInitialisees(std::sync::Mutex::new(HashSet::new())))
}

/// Reconstruit un index dont la définition en base ne porte pas encore
/// `marqueur` (une colonne gagnée après coup). DOUBLE VÉRIFICATION, et le
/// premier temps compte autant que le second : une lecture nue de
/// `sqlite_master` ne prend aucun verrou ; puis, sous `BEGIN IMMEDIATE`,
/// relecture — deux `migrate()` peuvent tourner en parallèle au démarrage
/// (`connect_accounts` ouvre hors du verrou des commandes) : le second
/// arrivant relit APRÈS le premier, trouve le marqueur, et ne fait rien.
fn reconstruire_index_si_ancien(
    conn: &Connection,
    nom: &str,
    marqueur: &str,
    creation: &str,
) -> Result<(), Error> {
    let definition = |conn: &Connection| -> Result<Option<String>, Error> {
        Ok(conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'index' AND name = ?1",
                [nom],
                |row| row.get(0),
            )
            .optional()?)
    };
    let ancien = |sql: Option<String>| sql.is_some_and(|sql| !sql.contains(marqueur));
    if !ancien(definition(conn)?) {
        return Ok(());
    }
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let travail = (|| -> Result<(), Error> {
        if ancien(definition(conn)?) {
            conn.execute_batch(&format!("DROP INDEX {nom}; {creation}"))?;
        }
        Ok(())
    })();
    match travail {
        Ok(()) => conn.execute_batch("COMMIT")?,
        Err(err) => {
            // L'échec du retour arrière n'apprendrait rien de plus que
            // l'erreur d'origine — même choix qu'à l'unité des fils.
            let _ = conn.execute_batch("ROLLBACK");
            return Err(err);
        }
    }
    Ok(())
}

fn table_columns(conn: &Connection, table: &str) -> Result<HashSet<String>, Error> {
    // `table_xinfo`, pas `table_info` : le second MASQUE les colonnes
    // générées (`sender_norm`) — la sonde d'existence les recréait à
    // chaque réouverture, « duplicate column name » (prouvé rouge E2).
    let mut stmt = conn.prepare(&format!("PRAGMA table_xinfo({table})"))?;
    let columns = stmt
        .query_map([], |row| row.get(1))?
        .collect::<Result<_, _>>()?;
    Ok(columns)
}

fn add_missing_columns(
    conn: &Connection,
    table: &str,
    columns: &[(&str, &str)],
) -> Result<(), Error> {
    let existing = table_columns(conn, table)?;
    for (column, ddl) in columns {
        if !existing.contains(*column) {
            conn.execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {ddl}"),
                [],
            )?;
        }
    }
    Ok(())
}

/// Destinataires stockés sur une ligne — un par `\n`, NULL quand vide
/// (R4). `join`/`split` sont réciproques ; une adresse ne contient jamais
/// de retour ligne (c'est `mailbox@host`).
/// Les adresses qu'une enveloppe porte (expéditeur, À, Cc) — jamais des
/// identifiants de fil, même entre chevrons (PLAN-AUDIT-V2 E5).
fn adresses_de(envelope: &Envelope) -> Vec<String> {
    let mut adresses: Vec<String> = Vec::new();
    adresses.extend(envelope.sender_address.clone());
    adresses.extend(envelope.to_addrs.iter().cloned());
    adresses.extend(envelope.cc_addrs.iter().cloned());
    adresses
}

fn join_addrs(addrs: &[String]) -> Option<String> {
    if addrs.is_empty() {
        None
    } else {
        Some(addrs.join("\n"))
    }
}

fn split_addrs(raw: Option<String>) -> Vec<String> {
    raw.map(|s| {
        s.split('\n')
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default()
}

/// Mapping partagé par toutes les lectures d'enveloppes — l'ordre des
/// colonnes est celui des SELECT ci-dessus (`to_addrs`/`cc_addrs` en
/// queue, index 9/10).
/// L'autorité UNIQUE de normalisation d'une adresse pour la mémoire
/// d'images (R1, PLAN-RETOURS-11) : minuscules Unicode côté Rust —
/// écriture (`allow_images_sender_of`, `revoke_images_sender`) et
/// lecture (`images_allowed`) passent toutes par ici.
fn adresse_images(adresse: Option<String>) -> Option<String> {
    adresse
        .map(|a| a.trim().to_lowercase())
        .filter(|a| !a.is_empty())
}

/// Les vocabulaires FERMÉS du routage (PLAN-MODE-ORGANISE E1) — la
/// même table sert la validation Rust et, en ceinture, les CHECK du
/// schéma. `ecarte` est la seule destination qui accepte une règle.
const DESTINATIONS_ROUTAGE: [&str; 4] = ["reception", "kiosque", "registre", "ecarte"];
const REGLES_ROUTAGE: [&str; 3] = ["spam", "archive", "corbeille"];

/// Les clés `prefs` du Mode organisé — l'état, et la borne de
/// rétention du Portier (première activation, jamais réécrite).
const PREF_MODE_ORGANISE: &str = "mode_organise";
const PREF_MODE_ORGANISE_EPOCH: &str = "mode_organise_epoch";

/// RETOURS-13 R5/R9 — les défauts des boutons du Portier : le Oui
/// prend une destination (jamais `ecarte`), le Non une règle du
/// vocabulaire d'écarté ou `ecarte` nu (« écarter sans déplacer »).
/// DÉRIVÉS des tables de routage — jamais une seconde copie du
/// vocabulaire (revue : une destination ajoutée à DESTINATIONS_ROUTAGE
/// aurait laissé le sélecteur des Réglages la refuser en silence).
const PREF_PORTIER_DEFAUT_OUI: &str = "portier_defaut_oui";
const PREF_PORTIER_DEFAUT_NON: &str = "portier_defaut_non";
fn defaut_portier_oui_valide(v: &str) -> bool {
    v != "ecarte" && DESTINATIONS_ROUTAGE.contains(&v)
}
fn defaut_portier_non_valide(v: &str) -> bool {
    v == "ecarte" || REGLES_ROUTAGE.contains(&v)
}

/// La porte UNIQUE de validation du vocabulaire de routage — appelée
/// avant toute écriture ET avant toute résolution d'adresse (un
/// vocabulaire troué ne se cache jamais derrière un autre refus).
fn valider_routage(destination: &str, regle: Option<&str>) -> Result<(), Error> {
    if !DESTINATIONS_ROUTAGE.contains(&destination) {
        return Err(Error::InvalidRouting(format!(
            "destination inconnue : {destination:?}"
        )));
    }
    if let Some(r) = regle {
        if destination != "ecarte" {
            return Err(Error::InvalidRouting(format!(
                "une règle du Non exige un expéditeur écarté, pas {destination:?}"
            )));
        }
        if !REGLES_ROUTAGE.contains(&r) {
            return Err(Error::InvalidRouting(format!("règle inconnue : {r:?}")));
        }
    }
    Ok(())
}

/// Le verdict du Portier sur un expéditeur — une ligne de
/// `routage_expediteurs`, telle que l'historique la montre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routage {
    pub address: String,
    pub destination: String,
    pub regle: Option<String>,
    pub epoch: i64,
}

fn lire_routage(row: &rusqlite::Row<'_>) -> rusqlite::Result<Routage> {
    Ok(Routage {
        address: row.get(0)?,
        destination: row.get(1)?,
        regle: row.get(2)?,
        epoch: row.get(3)?,
    })
}

/// Un rang du guichet du Portier (E2) : l'adresse EN ATTENTE —
/// normalisée, la clé que le verdict prendra — et son dernier message.
#[derive(Debug)]
pub struct RangPortier {
    pub address: String,
    pub ligne: UnifiedRow,
}

/// Les fils d'UN expéditeur — LA définition unique, partagée par le
/// recalcul des verdicts et la défaite d'attente du chemin de synchro.
fn fils_de(conn: &Connection, adresse: &str) -> Result<Vec<i64>, Error> {
    let fils = conn
        .prepare_cached(
            "SELECT DISTINCT thread_id FROM envelopes
              WHERE sender_norm = ?1 AND thread_id IS NOT NULL",
        )?
        .query_map(params![adresse], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(fils)
}

/// Le CŒUR transactionnel du verdict — LA porte unique, partagée par
/// [`Store::router_expediteur`] (Portier, « Déplacer vers… ») et
/// [`Store::nettoyage_verdict`] (revue 2026-08-30 : le Nettoyage en
/// recopiait le corps ; un futur ajout à « poser un verdict » aurait
/// divergé selon l'écran d'origine). L'appelant valide le vocabulaire
/// et normalise l'adresse AVANT.
fn poser_verdict(
    tx: &Connection,
    adresse: &str,
    destination: &str,
    regle: Option<&str>,
    epoch: i64,
) -> Result<(), Error> {
    tx.execute(
        "INSERT OR REPLACE INTO routage_expediteurs (address, destination, regle, epoch)
         VALUES (?1, ?2, ?3, ?4)",
        params![adresse, destination, regle, epoch],
    )?;
    // RETOURS-14 R8 (terrain 2026-08-31) : un OUI vaut confiance — le
    // verdict pose AUSSI la règle « toujours afficher les images de
    // cet expéditeur » (même table, même normalisation que la garde
    // R1 ; révocable aux Réglages > Affichage). Un Non ne touche pas
    // à la garde — elle a sa propre porte de sortie.
    if destination != "ecarte" {
        tx.execute(
            "INSERT OR REPLACE INTO images_expediteurs (address, epoch) VALUES (?1, ?2)",
            params![adresse, epoch],
        )?;
    }
    // Le verdict prend le relais de l'attente — Oui comme Non.
    tx.execute(
        "DELETE FROM portier_attente WHERE address = ?1",
        params![adresse],
    )?;
    rafraichir_fils_de(tx, adresse)
}

/// Recalcule les drapeaux des fils d'UN expéditeur par LA porte unique
/// (`thread::refresh`) — après un verdict ou une réintégration. Borné
/// aux fils de l'adresse (63 ms mesurées sur un expéditeur de 10 000
/// fils, geste unique).
fn rafraichir_fils_de(conn: &Connection, adresse: &str) -> Result<(), Error> {
    for fil in fils_de(conn, adresse)? {
        thread::refresh(conn, fil)?;
    }
    Ok(())
}

/// L'adresse est-elle celle d'UN de nos comptes ? Jamais soi au
/// Portier (leçon E1 : la propre adresse de l'utilisateur n'est jamais
/// un expéditeur à trier). `prepare_cached` : la sonde vit sur le
/// chemin chaud de la synchro (revue E2).
fn adresse_d_un_compte(conn: &Connection, adresse: &str) -> Result<bool, Error> {
    Ok(conn
        .prepare_cached("SELECT 1 FROM accounts WHERE lower(trim(email)) = ?1")?
        .exists(params![adresse])?)
}

/// L'expéditeur a-t-il du courrier ANTÉRIEUR à l'époque d'activation ?
/// C'est LA définition du « connu » de D3 (arrivées seules) — une seule
/// écriture, partagée par la décision d'arrivée et la réintégration :
/// deux copies divergeraient sur le sens même du guichet. Toutes
/// boîtes confondues : un historique en Archives ou aux Indésirables
/// est un historique.
fn connu_avant_epoque(conn: &Connection, adresse: &str, epoque: i64) -> Result<bool, Error> {
    Ok(conn
        .prepare_cached(
            "SELECT 1 FROM envelopes
              WHERE sender_norm = ?1 AND date_epoch <= ?2 LIMIT 1",
        )?
        .exists(params![adresse, epoque])?)
}

/// Purge les rangs du Portier qui ne s'appuient plus sur AUCUN
/// courrier (E2) : l'attente est DÉRIVÉE — un UID recyclé n'hérite
/// d'aucune décision (A43/A89). Partagée par le retrait de compte et
/// la réinitialisation de boîte.
/// LA liste des tables « par message », pour les trois purges
/// (`remove_local`, `remove_absent`, `reset_mailbox`) — PLAN-AUDIT-V1 E4.
/// Avant : trois copies divergentes, `remove_absent` en oubliait cinq.
/// Les actions en attente ne sont PAS dans la liste : selon la purge,
/// elles portent le geste (`remove_local`) ou sont irréalisables
/// (`remove_absent`, `reset_mailbox` — qui les retire à part).
pub(crate) const TABLES_PAR_MESSAGE: [&str; 7] = [
    "bodies",
    "invitations",
    "attachments",
    "images_messages",
    "mis_de_cote",
    "kiosque_lus",
    "envelopes",
];

/// Purge UN message de toutes ses tables et rend son fil, RELEVÉ AVANT
/// la suppression (après, le lien est perdu) — sans le rafraîchir :
/// c'est l'appelant qui rafraîchit, UNE fois par fil touché (revue
/// PLAN-AUDIT-V1 : un rafraîchissement par message coûtait ~500× sur un
/// fil de 500 disparus).
pub(crate) fn purger_message(
    conn: &Connection,
    mailbox_id: i64,
    uid: Uid,
) -> Result<Option<thread::ThreadId>, Error> {
    let thread = thread::thread_of(conn, mailbox_id, uid)?;
    search::deindex_message(conn, mailbox_id, uid)?;
    for table in TABLES_PAR_MESSAGE {
        conn.execute(
            &format!("DELETE FROM {table} WHERE mailbox_id = ?1 AND uid = ?2"),
            params![mailbox_id, uid],
        )?;
    }
    Ok(thread)
}

/// Les refusées d'un message (quarantaine E3) : un geste neuf de
/// l'utilisateur les remplace.
fn oublier_les_refusees(conn: &Connection, mailbox_id: i64, uid: Uid) -> Result<(), Error> {
    conn.execute(
        "DELETE FROM pending_actions WHERE mailbox_id = ?1 AND uid = ?2 AND refusee = 1",
        params![mailbox_id, uid],
    )?;
    Ok(())
}

fn purger_attente_orpheline(conn: &Connection) -> Result<(), Error> {
    conn.execute(
        "DELETE FROM portier_attente WHERE NOT EXISTS (
             SELECT 1 FROM envelopes e WHERE e.sender_norm = portier_attente.address)",
        [],
    )?;
    Ok(())
}

fn row_to_envelope(row: &rusqlite::Row<'_>) -> rusqlite::Result<Envelope> {
    Ok(Envelope {
        reply_to: None,
        uid: row.get(0)?,
        subject: row.get(1)?,
        sender: row.get(2)?,
        sender_address: row.get(3)?,
        message_id: row.get(4)?,
        date: row
            .get::<_, Option<i64>>(5)?
            .and_then(|epoch| DateTime::from_timestamp(epoch, 0)),
        seen: row.get(6)?,
        flagged: row.get(7)?,
        in_reply_to: row.get(8)?,
        to_addrs: split_addrs(row.get(9)?),
        cc_addrs: split_addrs(row.get(10)?),
    })
}

/// Mapping partagé par les lectures de la boîte unifiée — l'ordre des
/// colonnes est celui de [`SELECT_UNIFIED`].
pub(crate) fn row_to_unified(row: &rusqlite::Row<'_>) -> rusqlite::Result<UnifiedRow> {
    let attachment_count = row.get::<_, i64>(10)?.max(0) as u32;
    Ok(UnifiedRow {
        account_id: row.get(0)?,
        account_email: row.get(1)?,
        envelope: Envelope {
            reply_to: None,
            uid: row.get(2)?,
            subject: row.get(3)?,
            sender: row.get(4)?,
            sender_address: row.get(5)?,
            message_id: row.get(6)?,
            date: row
                .get::<_, Option<i64>>(7)?
                .and_then(|epoch| DateTime::from_timestamp(epoch, 0)),
            seen: row.get(8)?,
            flagged: row.get(9)?,
            in_reply_to: row.get(12)?,
            to_addrs: split_addrs(row.get(15)?),
            cc_addrs: split_addrs(row.get(16)?),
        },
        mailbox: row.get(13)?,
        has_attachment: attachment_count > 0,
        attachment_count,
        preview: row.get(14)?,
        thread_id: row.get(11)?,
        // Valeurs d'un message vu SEUL — c'est le cas de la recherche, qui
        // ne joint pas `threads`. La liste groupée les écrase avec
        // l'agrégat réel via [`row_to_threaded`].
        thread_size: 1,
        thread_unseen: u32::from(!row.get::<_, bool>(8)?),
        // Posée par la passe de PAGE (`enrichir_lignes`), jamais ici.
        invitation: None,
    })
}

/// Mapping de la liste groupée : les colonnes unifiées, puis l'agrégat du
/// fil ajouté par [`THREAD_AGGREGATE`].
pub(crate) fn row_to_threaded(row: &rusqlite::Row<'_>) -> rusqlite::Result<UnifiedRow> {
    Ok(UnifiedRow {
        // `to_addrs`/`cc_addrs` ont repoussé l'agrégat aux index 17/18.
        thread_size: row.get(17)?,
        thread_unseen: row.get(18)?,
        ..row_to_unified(row)?
    })
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn envelope(uid: Uid, subject: &str, epoch: i64, seen: bool) -> Envelope {
        Envelope {
            reply_to: None,
            uid,
            subject: Some(subject.to_string()),
            sender: Some("Alice Martin".to_string()),
            sender_address: Some("alice@example.com".to_string()),
            message_id: Some(format!("<m{uid}@example.com>")),
            in_reply_to: None,
            date: Some(Utc.timestamp_opt(epoch, 0).unwrap()),
            seen,
            flagged: uid.is_multiple_of(2),
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        }
    }

    fn test_account(store: &Store) -> i64 {
        store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap()
    }

    fn store_with_mailbox() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = test_account(&store);
        let id = store.create_mailbox(account, "INBOX", 1).unwrap();
        (store, id)
    }

    /// Toutes les tables « par message » garnies pour un UID : ce que
    /// chaque purge doit emporter (PLAN-AUDIT-V1 E4).
    fn garnir_message(store: &mut Store, inbox: i64, uid: Uid) {
        store
            .upsert_envelopes(inbox, &[envelope(uid, "sujet", 100, false)])
            .unwrap();
        store.save_body(inbox, uid, "<p>corps</p>", &[]).unwrap();
        let conn = store.conn();
        conn.execute(
            "INSERT INTO attachments (mailbox_id, uid, idx, name, mime, size) VALUES (?1, ?2, 0, 'a.pdf', 'application/pdf', 1)",
            params![inbox, uid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO invitations (mailbox_id, uid, methode, event_uid) VALUES (?1, ?2, 'REQUEST', 'evt')",
            params![inbox, uid],
        )
        .unwrap();
        for table in ["images_messages", "mis_de_cote", "kiosque_lus"] {
            conn.execute(
                &format!("INSERT INTO {table} (mailbox_id, uid, epoch) VALUES (?1, ?2, 1)"),
                params![inbox, uid],
            )
            .unwrap();
        }
    }

    /// Combien de lignes, toutes tables par message confondues, portent
    /// encore cet UID.
    fn lignes_du_message(store: &Store, inbox: i64, uid: Uid) -> Vec<(&'static str, i64)> {
        [
            "envelopes",
            "bodies",
            "attachments",
            "invitations",
            "images_messages",
            "mis_de_cote",
            "kiosque_lus",
        ]
        .into_iter()
        .map(|table| {
            let n: i64 = store
                .conn()
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE mailbox_id = ?1 AND uid = ?2"),
                    params![inbox, uid],
                    |row| row.get(0),
                )
                .unwrap();
            (table, n)
        })
        .filter(|(_, n)| *n > 0)
        .collect()
    }

    /// Audit 2026-09-01 S2 (E4) : `remove_absent` ne purgeait que 3 tables
    /// sur 7 — un message disparu du serveur laissait pièces, invitation,
    /// mémoire d'images, mise de côté et « lu » du Kiosque orphelins (aucune
    /// clé étrangère sur `envelopes`). UNE liste, la même pour les trois
    /// purges.
    #[test]
    fn un_message_disparu_du_serveur_ne_laisse_aucun_orphelin() {
        let (mut store, inbox) = store_with_mailbox();
        garnir_message(&mut store, inbox, 1);
        assert_eq!(lignes_du_message(&store, inbox, 1).len(), 7, "décor garni");

        let retires = store.remove_absent(inbox, &HashSet::new()).unwrap();

        assert_eq!(retires, 1);
        assert_eq!(
            lignes_du_message(&store, inbox, 1),
            Vec::<(&str, i64)>::new(),
            "aucune ligne ne doit survivre au message"
        );
    }

    /// Un déclencheur SQLite qui refuse la suppression des enveloppes
    /// simule une panne au milieu de la purge : tout ce qui précédait
    /// (corps, actions…) doit être REMBOBINÉ. Avant E4, `reset_mailbox`
    /// enchaînait neuf écritures en autocommit — un crash entre deux
    /// laissait des fils sans enveloppes (la « pastille devant une liste
    /// vide » déjà payée à E5 du mode organisé).
    fn bloquer_les_suppressions_d_enveloppes(store: &Store) {
        store
            .conn()
            .execute_batch(
                "CREATE TEMP TRIGGER panne BEFORE DELETE ON envelopes
                 BEGIN SELECT RAISE(ABORT, 'panne simulee'); END;",
            )
            .unwrap();
    }

    #[test]
    fn reset_mailbox_est_atomique() {
        let (mut store, inbox) = store_with_mailbox();
        garnir_message(&mut store, inbox, 1);
        store.enqueue_action(inbox, 1, Action::MarkSeen).unwrap();
        bloquer_les_suppressions_d_enveloppes(&store);

        assert!(
            store.reset_mailbox(inbox, 2).is_err(),
            "la panne doit remonter"
        );

        assert_eq!(
            lignes_du_message(&store, inbox, 1).len(),
            7,
            "rien n'a été effacé avant la panne : une seule transaction"
        );
        assert_eq!(store.pending_actions(inbox).unwrap().len(), 1);
        assert_eq!(
            store
                .sync_state(test_account(&store), "INBOX")
                .unwrap()
                .unwrap()
                .uid_validity,
            1,
            "l'UIDVALIDITY n'a pas bougé non plus"
        );
    }

    #[test]
    fn remove_local_est_atomique() {
        let (mut store, inbox) = store_with_mailbox();
        garnir_message(&mut store, inbox, 1);
        bloquer_les_suppressions_d_enveloppes(&store);

        assert!(store.remove_local(inbox, 1).is_err());

        assert_eq!(
            lignes_du_message(&store, inbox, 1).len(),
            7,
            "corps, pièces, invitation… tous encore là : rembobinés avec l'enveloppe"
        );
    }

    /// Revue PLAN-AUDIT-V1 : une refusée n'est pas éternelle — un geste
    /// neuf de l'utilisateur sur le même message la remplace, et la ligne
    /// de la fente retombe.
    #[test]
    fn un_nouveau_geste_remplace_l_ancienne_refusee() {
        let (store, id) = store_with_mailbox();
        store
            .enqueue_action(id, 1, Action::MoveTo("Disparu".to_string()))
            .unwrap();
        let refusee = store.pending_actions(id).unwrap().remove(0).id;
        store.refuser_action(refusee, "[TRYCREATE]").unwrap();
        assert_eq!(store.actions_refusees().unwrap(), 1);

        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();

        assert_eq!(store.actions_refusees().unwrap(), 0, "remplacée");
        let file = store.pending_actions(id).unwrap();
        assert_eq!(file.len(), 1);
        assert_eq!(file[0].action, Action::MarkSeen);
    }

    /// Audit 2026-09-01 (PLAN-AUDIT-V1 E3) : une ligne `pending_actions`
    /// au `kind` illisible (version future, corruption) faisait échouer
    /// TOUT `pending_actions(mailbox_id)` — la file entière coincée par
    /// une ligne. Elle est mise en quarantaine avec son motif, la file
    /// continue.
    #[test]
    fn une_ligne_illisible_ne_fait_pas_echouer_la_file() {
        let (store, id) = store_with_mailbox();
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
        store
            .conn()
            .execute(
                "INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (?1, 2, 'teleporter')",
                [id],
            )
            .unwrap();
        store.enqueue_action(id, 3, Action::Archive).unwrap();

        let file = store.pending_actions(id).unwrap();
        assert_eq!(
            file.iter().map(|p| p.uid).collect::<Vec<_>>(),
            vec![1, 3],
            "les lisibles passent, l'illisible est écartée"
        );
        assert_eq!(store.actions_refusees().unwrap(), 1);
        // Idempotent : une seconde lecture ne la recompte pas.
        store.pending_actions(id).unwrap();
        assert_eq!(store.actions_refusees().unwrap(), 1);
    }

    /// D-36 (soldée à l'audit du 2026-09-01) : un `\n` dans un
    /// commentaire `--` du littéral `SCHEMA` devenait un vrai saut de
    /// ligne, SQLite avalait la suite du commentaire comme une COLONNE,
    /// et toute base NEUVE naissait avec une colonne fantôme dans
    /// `echos`. Le filet manquant : chaque colonne de chaque table d'une
    /// base neuve porte un nom sain — un identifiant, jamais un bout de
    /// phrase.
    #[test]
    fn une_base_neuve_n_a_aucune_colonne_fantome() {
        let store = Store::open_in_memory().unwrap();
        let conn = store.conn();
        let mut tables = conn
            .prepare(
                "SELECT name FROM sqlite_master \
                 WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            )
            .unwrap();
        let noms: Vec<String> = tables
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(noms.iter().any(|t| t == "echos"), "la table echos manque");
        for table in noms {
            let mut colonnes = conn
                .prepare(&format!("PRAGMA table_info(\"{table}\")"))
                .unwrap();
            let noms_colonnes: Vec<String> = colonnes
                .query_map([], |row| row.get(1))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            for colonne in &noms_colonnes {
                assert!(
                    colonne
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                    "colonne fantôme « {colonne} » dans {table} : {noms_colonnes:?}"
                );
            }
        }
    }

    fn recent(store: &Store, offset: usize, limit: usize) -> Vec<Envelope> {
        store
            .recent(test_account(store), "INBOX", offset, limit)
            .unwrap()
    }

    /// R4 : les destinataires À/Cc écrits à la synchro se relisent tels
    /// quels — c'est ce que le dossier d'envois affiche (l'expéditeur y
    /// est SOI) et ce que « Répondre à tous » relit hors ligne. Le cas
    /// « Test PJ 3 » : un envoi à une adresse tierce.
    #[test]
    fn upsert_persiste_les_destinataires() {
        let (mut store, id) = store_with_mailbox();
        let mut env = envelope(1, "Test PJ 3", 1_700_000_000, true);
        env.to_addrs = vec!["sebastien.monchamps@gmail.com".to_string()];
        env.cc_addrs = vec![
            "copie1@exemple.fr".to_string(),
            "copie2@exemple.fr".to_string(),
        ];
        store
            .upsert_envelopes(id, std::slice::from_ref(&env))
            .unwrap();
        assert_eq!(recent(&store, 0, 10), vec![env]);
    }

    /// Une préférence jamais posée répond le défaut demandé ; posée, elle
    /// se relit telle quelle et s'écrase sans doublon.
    #[test]
    fn bool_pref_default_then_roundtrip() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.bool_pref("arrival_bubbles", true).unwrap());
        assert!(!store.bool_pref("arrival_bubbles", false).unwrap());
        store.set_bool_pref("arrival_bubbles", false).unwrap();
        assert!(!store.bool_pref("arrival_bubbles", true).unwrap());
        store.set_bool_pref("arrival_bubbles", true).unwrap();
        assert!(store.bool_pref("arrival_bubbles", false).unwrap());
    }

    /// Le repère de la relève gardée (ADR 0017) : jamais posé -> `None`
    /// (une base héritée relève tout à son premier cycle), posé -> relu.
    #[test]
    fn remote_uidnext_absent_puis_pose() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        // NULL tant qu'aucune relève gardée n'a eu lieu : une base
        // héritée relève tout à son premier cycle (ADR 0017).
        assert_eq!(store.remote_uidnext(mailbox).unwrap(), None);
        store.set_remote_uidnext(mailbox, 101).unwrap();
        assert_eq!(store.remote_uidnext(mailbox).unwrap(), Some(101));
        assert_eq!(store.envelope_count(mailbox).unwrap(), 0);
        assert!(!store.has_pending_actions(mailbox).unwrap());
    }

    /// Un départ en attente de rejeu (archive, suppression, déplacement)
    /// ne compte plus dans le dénominateur de l'avancement : le geste
    /// retire la ligne locale immédiatement (écho, PLAN-REACTIVITE E3)
    /// mais `remote_total` date du dernier SELECT — sans l'ajustement,
    /// UN SEUL triage suffisait à figer l'avancement à 99 % et le trait
    /// hitofude de la barre d'état avec lui (terrain 2026-08-15,
    /// PLAN-GELS : 5 archives + 1 suppression en attente = 99 % pour
    /// toute la durée du rejeu). Le vrai chemin du geste est appelé
    /// (`geste_avec_echo`), jamais une simulation.
    #[test]
    fn un_depart_en_attente_ne_compte_plus_dans_le_denominateur() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "reste", 100, true),
                    envelope(2, "part en archive", 200, true),
                    envelope(3, "reste aussi", 300, false),
                ],
            )
            .unwrap();
        store.record_remote_total(id, 3).unwrap();
        assert_eq!(store.sync_progress().unwrap(), (3, 3));
        // Le triage : l'écho retire la ligne, l'action attend son rejeu.
        store
            .geste_avec_echo(id, 2, Action::Archive, Some("archives"))
            .unwrap();
        assert_eq!(
            store.sync_progress().unwrap(),
            (2, 2),
            "le message archivé localement ne doit plus être attendu"
        );
        // Un marquage en attente ne retire rien de la boîte : il ne
        // touche pas le dénominateur.
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
        assert_eq!(store.sync_progress().unwrap(), (2, 2));
        // Un déplacement retire aussi ; et le dénominateur ne descend
        // jamais sous zéro même si `remote_total` est en retard.
        store
            .geste_avec_echo(id, 3, Action::MoveTo("Factures".into()), None)
            .unwrap();
        store.record_remote_total(id, 1).unwrap();
        assert_eq!(store.sync_progress().unwrap(), (1, 0));
    }

    /// Le pendant texte : jamais posée -> `None` (le défaut appartient à
    /// l'appelant), posée -> relue telle quelle, écrasée sans doublon.
    #[test]
    fn text_pref_none_then_roundtrip() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.text_pref("lang").unwrap(), None);
        store.set_text_pref("lang", "en").unwrap();
        assert_eq!(store.text_pref("lang").unwrap(), Some("en".to_string()));
        store.set_text_pref("lang", "fr").unwrap();
        assert_eq!(store.text_pref("lang").unwrap(), Some("fr".to_string()));
    }

    /// Le lot transactionnel : tout écrit, tout relu — le pendant
    /// multi-clés de `text_pref_none_then_roundtrip`.
    #[test]
    fn set_text_prefs_ecrit_le_lot_entier() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .set_text_prefs(&[("repere_icone.1", "home"), ("repere_teinte.1", "bleu")])
            .unwrap();
        assert_eq!(
            store.text_pref("repere_icone.1").unwrap(),
            Some("home".to_string())
        );
        assert_eq!(
            store.text_pref("repere_teinte.1").unwrap(),
            Some("bleu".to_string())
        );
    }

    #[test]
    fn roundtrips_all_envelope_fields() {
        let (mut store, id) = store_with_mailbox();
        let original = envelope(7, "Sujet accentué : été", 1_700_000_000, true);
        store
            .upsert_envelopes(id, std::slice::from_ref(&original))
            .unwrap();
        assert_eq!(recent(&store, 0, 10), vec![original]);
    }

    #[test]
    fn roundtrips_envelope_without_optional_fields() {
        let (mut store, id) = store_with_mailbox();
        let bare = Envelope {
            reply_to: None,
            uid: 1,
            subject: None,
            sender: None,
            sender_address: None,
            message_id: None,
            in_reply_to: None,
            date: None,
            seen: false,
            flagged: false,
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        };
        store
            .upsert_envelopes(id, std::slice::from_ref(&bare))
            .unwrap();
        assert_eq!(recent(&store, 0, 10), vec![bare]);
    }

    /// L'ordre du rattrapage est un choix de PRODUIT, pas un accident de
    /// tri SQL : INBOX d'abord, les envois ensuite, le reste par nom. Un
    /// serveur qui liste « Archive » avant INBOX ne doit pas faire
    /// rattraper 80 000 corps d'archive avant le courrier que la liste
    /// affiche.
    #[test]
    fn les_boites_se_rattrapent_reception_d_abord() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        store.create_mailbox(account, "Archive", 1).unwrap();
        store.create_mailbox(account, "Corbeille", 1).unwrap();
        store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .create_mailbox(account, "Messages envoyés", 1)
            .unwrap();
        store
            .set_thread_scope(account, Some("Messages envoyés"))
            .unwrap();

        assert_eq!(
            store.mailbox_names(account).unwrap(),
            vec!["INBOX", "Messages envoyés", "Archive", "Corbeille"]
        );
    }

    /// L'horizon d'import (PLAN-HORIZON-NETTOYAGE, D1-D4) : pref par
    /// compte au vocabulaire FERMÉ ; sans pref, « tout » — un compte
    /// d'avant le réglage garde l'import intégral (D4) ; la valeur meurt
    /// avec le compte, et le rowid réutilisé n'en hérite pas
    /// (PREFS_PAR_COMPTE).
    #[test]
    fn horizon_import_defaut_tout_vocabulaire_ferme_purge_au_retrait() {
        let mut store = Store::open_in_memory().unwrap();
        let id = store
            .adopt_or_create_account("h@exemple.fr", "gmail")
            .unwrap();

        assert_eq!(store.horizon_import(id).unwrap(), "tout");
        store.set_horizon_import(id, "1a").unwrap();
        assert_eq!(store.horizon_import(id).unwrap(), "1a");
        assert!(store.set_horizon_import(id, "42 jours").is_err());
        assert_eq!(store.horizon_import(id).unwrap(), "1a");

        store.delete_account(id).unwrap();
        let heritier = store
            .adopt_or_create_account("h2@exemple.fr", "gmail")
            .unwrap();
        assert_eq!(heritier, id, "décor : le rowid doit se réutiliser");
        assert_eq!(store.horizon_import(heritier).unwrap(), "tout");
    }

    /// Le retrait d'un compte ne laisse RIEN derrière lui : ni les lignes
    /// en cascade (boîtes, enveloppes, corps), ni celles sans clé
    /// étrangère (brouillons, boîte d'envoi, index de recherche) — et le
    /// compte voisin garde tout, recherche comprise.
    #[test]
    fn delete_account_efface_tout_et_ne_touche_pas_le_voisin() {
        let mut store = Store::open_in_memory().unwrap();
        let parti = store
            .adopt_or_create_account("part@exemple.fr", "gmail")
            .unwrap();
        let voisin = store
            .adopt_or_create_account("reste@exemple.fr", "gmail")
            .unwrap();
        for (account, sujet) in [(parti, "Facture du départ"), (voisin, "Devis qui reste")] {
            let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(mailbox, &[envelope(1, sujet, 100, false)])
                .unwrap();
            store.save_body(mailbox, 1, "<p>corps</p>", &[]).unwrap();
            store
                .save_draft(
                    account,
                    None,
                    None,
                    crate::DraftContent {
                        to_raw: "a@b.fr",
                        cc_raw: "",
                        bcc_raw: "",
                        body_html: None,
                        subject: sujet,
                        body: "brouillon",
                        reply_to_uid: None,
                        reply_to_mailbox: None,
                        important: false,
                    },
                )
                .unwrap();
            store
                .enqueue_outbox(
                    account,
                    &crate::compose::Draft {
                        message_id: format!("<sortant-{account}@exemple.fr>"),
                        from: "moi@exemple.fr".to_string(),
                        to: vec!["a@b.fr".to_string()],
                        cc: Vec::new(),
                        bcc: Vec::new(),
                        subject: sujet.to_string(),
                        body_text: "corps".to_string(),
                        body_html: None,
                        in_reply_to: None,
                        references: None,
                        important: false,
                        ics_reply: None,
                    },
                )
                .unwrap();
        }

        // Les préférences suffixées par l'id (signature, repère, nom) :
        // un id SQLite réutilisé après retrait ferait sinon hériter au
        // compte suivant l'identité de l'ancien (revue PLAN-RETOURS-8 ;
        // nom personnalisé : PLAN-RETOURS-9).
        for (account, teinte) in [(parti, "rouge"), (voisin, "bleu")] {
            store
                .set_text_pref(&format!("signature.{account}"), "<p>sig</p>")
                .unwrap();
            store
                .set_text_pref(&format!("repere_icone.{account}"), "home")
                .unwrap();
            store
                .set_text_pref(&format!("repere_teinte.{account}"), teinte)
                .unwrap();
            store
                .set_text_pref(&format!("nom_compte.{account}"), "Perso")
                .unwrap();
        }

        store.delete_account(parti).unwrap();

        let comptes = store.accounts().unwrap();
        assert_eq!(comptes.len(), 1);
        assert_eq!(comptes[0].email, "reste@exemple.fr");
        for cle in ["signature", "repere_icone", "repere_teinte", "nom_compte"] {
            assert_eq!(
                store.text_pref(&format!("{cle}.{parti}")).unwrap(),
                None,
                "{cle} du compte parti : la pref doit mourir avec lui"
            );
            assert!(
                store
                    .text_pref(&format!("{cle}.{voisin}"))
                    .unwrap()
                    .is_some(),
                "{cle} du voisin : intacte"
            );
        }
        for table in [
            "mailboxes",
            "envelopes",
            "bodies",
            "drafts",
            "outbox",
            "search_docs",
        ] {
            let total: i64 = store
                .0
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(total, 1, "{table} : seule la ligne du voisin doit rester");
        }
        assert!(
            store.search("départ", 10).unwrap().is_empty(),
            "le courrier du compte parti ne doit plus sortir en recherche"
        );
        assert_eq!(
            store.search("reste", 10).unwrap().len(),
            1,
            "la recherche du voisin doit survivre au retrait"
        );
    }

    /// ADR 0010 : un message SANS date reste éligible au rattrapage, même
    /// sous un horizon borné. L'ancienne règle l'excluait (« non situable
    /// dans l'horizon ») — un trou silencieux : jamais de corps, donc
    /// jamais de recherche, et rien à l'écran pour le signaler. Le doute
    /// ne coûte plus que son rang : les NULL ferment le tri.
    #[test]
    fn un_message_sans_date_reste_a_rattraper() {
        let (mut store, id) = store_with_mailbox();
        let sans_date = Envelope {
            reply_to: None,
            uid: 9,
            subject: None,
            sender: None,
            sender_address: None,
            message_id: None,
            in_reply_to: None,
            date: None,
            seen: false,
            flagged: false,
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        };
        store
            .upsert_envelopes(id, std::slice::from_ref(&sans_date))
            .unwrap();

        let account = test_account(&store);
        let uids = store
            .bodies_to_backfill(account, "INBOX", 1_000_000, 10)
            .unwrap();
        assert_eq!(uids, vec![9], "l'horizon borné n'exclut plus les sans-date");
        assert_eq!(
            store
                .bodies_pending_count(account, "INBOX", 1_000_000)
                .unwrap(),
            1,
            "le compteur d'avancement le voit aussi — sinon la barre mentirait"
        );
    }

    #[test]
    fn upsert_replaces_existing_envelope() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "avant", 100, false)])
            .unwrap();
        store
            .upsert_envelopes(id, &[envelope(1, "après", 100, true)])
            .unwrap();
        let rows = recent(&store, 0, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].subject.as_deref(), Some("après"));
        assert!(rows[0].seen);
    }

    #[test]
    fn recent_orders_by_date_then_uid_descending() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "ancien", 100, false),
                    envelope(3, "récent", 300, false),
                    envelope(2, "milieu", 200, false),
                ],
            )
            .unwrap();
        let uids: Vec<Uid> = recent(&store, 0, 2).iter().map(|e| e.uid).collect();
        assert_eq!(uids, vec![3, 2]);
    }

    #[test]
    fn remove_absent_deletes_only_missing_uids() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "a", 100, false),
                    envelope(2, "b", 200, false),
                    envelope(3, "c", 300, false),
                ],
            )
            .unwrap();
        let present: HashSet<Uid> = [1, 3].into_iter().collect();
        assert_eq!(store.remove_absent(id, &present).unwrap(), 1);
        assert_eq!(store.count(id).unwrap(), 2);
    }

    #[test]
    fn sync_state_roundtrips_including_modseq() {
        let (store, id) = store_with_mailbox();
        assert_eq!(
            store.sync_state(test_account(&store), "INBOX").unwrap(),
            Some(SyncState {
                mailbox_id: id,
                uid_validity: 1,
                last_uid: 0,
                highest_modseq: None,
                initialisee: false,
            })
        );
        store.update_state(id, 42, Some(9000)).unwrap();
        let state = store
            .sync_state(test_account(&store), "INBOX")
            .unwrap()
            .unwrap();
        assert_eq!(state.last_uid, 42);
        assert_eq!(state.highest_modseq, Some(9000));
    }

    #[test]
    fn sync_state_is_none_for_unknown_mailbox() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(
            store.sync_state(test_account(&store), "INBOX").unwrap(),
            None
        );
    }

    #[test]
    fn reset_mailbox_clears_envelopes_and_state() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();
        store.update_state(id, 1, Some(5)).unwrap();
        store.reset_mailbox(id, 2).unwrap();
        assert_eq!(store.count(id).unwrap(), 0);
        let state = store
            .sync_state(test_account(&store), "INBOX")
            .unwrap()
            .unwrap();
        assert_eq!(state.uid_validity, 2);
        assert_eq!(state.last_uid, 0);
        assert_eq!(state.highest_modseq, None);
    }

    #[test]
    fn max_uid_is_zero_for_empty_mailbox() {
        let (store, id) = store_with_mailbox();
        assert_eq!(store.max_uid(id).unwrap(), 0);
    }

    #[test]
    fn recent_pages_with_offset() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &(1..=5)
                    .map(|uid| envelope(uid, "sujet", 100 * i64::from(uid), false))
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let page: Vec<Uid> = recent(&store, 2, 2).iter().map(|e| e.uid).collect();
        assert_eq!(page, vec![3, 2], "offset 2 saute les deux plus récents");
        assert!(recent(&store, 10, 5).is_empty());
    }

    #[test]
    fn action_queue_roundtrips_in_emission_order() {
        let (store, id) = store_with_mailbox();
        store.enqueue_action(id, 5, Action::MarkSeen).unwrap();
        store.enqueue_action(id, 3, Action::MarkUnseen).unwrap();

        let queued = store.pending_actions(id).unwrap();
        assert_eq!(queued.len(), 2);
        assert_eq!(
            (queued[0].uid, queued[0].action.clone()),
            (5, Action::MarkSeen)
        );
        assert_eq!(
            (queued[1].uid, queued[1].action.clone()),
            (3, Action::MarkUnseen)
        );

        store.remove_action(queued[0].id).unwrap();
        assert_eq!(store.pending_actions(id).unwrap().len(), 1);
    }

    #[test]
    fn set_seen_local_updates_and_reports_actual_change() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();

        assert!(store.set_seen_local(id, 1, true).unwrap());
        assert!(recent(&store, 0, 1)[0].seen);
        assert!(
            !store.set_seen_local(id, 1, true).unwrap(),
            "déjà lu : aucun changement à journaliser"
        );
    }

    #[test]
    fn set_flagged_local_updates_and_reports_actual_change() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();

        assert!(store.set_flagged_local(id, 1, true).unwrap());
        assert!(recent(&store, 0, 1)[0].flagged);
        assert!(
            !store.set_flagged_local(id, 1, true).unwrap(),
            "déjà étoilé : aucun changement à journaliser"
        );
    }

    /// E4 (PLAN-REACTIVITE, 1ᵉʳ terrain) : les ARRIVÉES se comptent à
    /// l'UID, jamais au `fetched` du rapport — un delta CONDSTORE y mêle
    /// tous les drapeaux glissés (Gmail à chaque étiquette), et la borne
    /// des corps « débordait » à chaque arrivée.
    #[test]
    fn les_arrivees_se_comptent_a_l_uid() {
        let (mut store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "ancien", 100, true),
                    envelope(2, "ancien aussi", 200, true),
                ],
            )
            .unwrap();
        assert_eq!(store.arrivees_depuis(account, "INBOX", 2).unwrap(), 0);

        // Deux arrivées + un vieux drapeau retouché (upsert du même
        // uid 1) : le compte ne bouge que pour les UID neufs.
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "ancien", 100, false),
                    envelope(3, "neuf", 300, false),
                    envelope(4, "neuf aussi", 400, false),
                ],
            )
            .unwrap();
        assert_eq!(store.arrivees_depuis(account, "INBOX", 2).unwrap(), 2);
        // Boîte inconnue : zéro, jamais une erreur — la relève d'un
        // compte jamais synchronisé ne doit pas casser sur ce compte.
        assert_eq!(store.arrivees_depuis(account, "Ailleurs", 0).unwrap(), 0);
    }

    #[test]
    fn remove_local_drops_envelope_and_body() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();
        store.save_body(id, 1, "<p>x</p>", &[]).unwrap();

        store.remove_local(id, 1).unwrap();

        assert!(recent(&store, 0, 10).is_empty());
        assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
    }

    #[test]
    fn reset_mailbox_clears_pending_actions() {
        let (store, id) = store_with_mailbox();
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
        store.reset_mailbox(id, 2).unwrap();
        assert!(store.pending_actions(id).unwrap().is_empty());
    }

    #[test]
    fn body_roundtrips_and_is_none_when_absent() {
        let (store, id) = store_with_mailbox();
        assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
        store.save_body(id, 1, "<p>bonjour</p>", &[]).unwrap();
        assert_eq!(
            store
                .body(test_account(&store), "INBOX", 1)
                .unwrap()
                .as_deref(),
            Some("<p>bonjour</p>")
        );
    }

    fn pdf(index: usize, name: &str) -> Attachment {
        Attachment {
            index,
            name: name.to_string(),
            mime: "application/pdf".to_string(),
            size: 2048,
        }
    }

    /// Ce que le rattrapage cherche, depuis le 2026-08-26 : **les corps
    /// ABSENTS**, et rien d'autre.
    ///
    /// Il a longtemps cherché aussi les corps rapatriés AVANT que les
    /// pièces jointes n'existent — `bodies.scanned = 0`, un MIME jamais
    /// inspecté, non récupérable depuis le HTML stocké. Ce critère est
    /// **retiré** (PLAN-DEMARRAGE, décision D8 du CE) : il obligeait
    /// SQLite à rappeler la ligne du corps pour lire un bit, ce qui
    /// tenait le verrou global **8 870 ms à chaque démarrage** sur la
    /// base du terrain.
    ///
    /// Les trois faits qui l'ont permis, tous mesurés le 2026-08-26 :
    /// la production n'écrit **jamais** `scanned = 0` ([`Store::save_body_full`]
    /// pose un `1` en dur) ; les **deux** postes de la flotte portent
    /// **zéro** ligne à `scanned = 0` ; et la passe d'héritage qui les
    /// produisait est soldée partout. Le critère protégeait zéro ligne.
    ///
    /// Ce que ce test garde donc : un corps présent sort le message du
    /// rattrapage, et **rien ne l'y ramène**. Plus l'invariant d'écriture
    /// qui a rendu le retrait sûr — si un jour quelque chose écrivait
    /// `scanned = 0`, il faudrait rouvrir la décision, et ce test le dira.
    #[test]
    fn un_corps_present_sort_le_message_du_rattrapage_et_rien_ne_l_y_ramene() {
        let (mut store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .upsert_envelopes(id, &[envelope(1, "sujet", 100, false)])
            .unwrap();

        // Sans corps : le message attend.
        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            vec![1]
        );
        assert_eq!(store.bodies_pending_count(account, "INBOX", 0).unwrap(), 1);

        store.save_body(id, 1, "<p>corps</p>", &[]).unwrap();

        // Corps present : plus rien a faire, definitivement.
        assert!(
            store
                .bodies_to_backfill(account, "INBOX", 0, 10)
                .unwrap()
                .is_empty()
        );
        assert_eq!(store.bodies_pending_count(account, "INBOX", 0).unwrap(), 0);

        // L'INVARIANT qui a rendu le retrait du critere sur : la
        // production pose `scanned = 1`, toujours. La colonne n'est plus
        // lue par le rattrapage — si elle devait le redevenir, elle dit
        // encore la verite.
        let scanne: i64 = store
            .conn()
            .query_row("SELECT scanned FROM bodies", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            scanne, 1,
            "la production doit toujours ecrire scanned = 1 — sinon la decision D8 de PLAN-DEMARRAGE est a rouvrir"
        );
    }

    /// R1 (PLAN-RETOURS-3) : le dénominateur du pourcentage. Le total ne
    /// bouge PAS quand un corps arrive — seul le nombre de manquants
    /// diminue ; `total - pending` donne les corps présents, base du
    /// pourcentage affiché.
    #[test]
    fn le_total_du_corpus_ne_compte_pas_les_corps_mais_les_messages() {
        let (mut store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "un", 100, false),
                    envelope(2, "deux", 200, false),
                    envelope(3, "trois", 300, false),
                ],
            )
            .unwrap();

        // Trois messages en portée, aucun corps encore lu.
        assert_eq!(store.bodies_total_count(account, "INBOX", 0).unwrap(), 3);
        assert_eq!(store.bodies_pending_count(account, "INBOX", 0).unwrap(), 3);

        // Un corps arrive : le total tient, le reste baisse d'un.
        store.save_body(id, 2, "<p>corps</p>", &[]).unwrap();
        assert_eq!(
            store.bodies_total_count(account, "INBOX", 0).unwrap(),
            3,
            "le total est le corpus, pas les corps rapatriés"
        );
        assert_eq!(store.bodies_pending_count(account, "INBOX", 0).unwrap(), 2);
    }

    /// Un message deja lu ailleurs — telephone, webmail — ne doit pas
    /// declencher de bulle : c'est du bruit pur, et c'est ce qui fait
    /// couper les notifications.
    #[test]
    fn only_genuinely_new_and_unread_messages_are_notifiable() {
        let (mut store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(10, "ancien", 100, false),
                    envelope(11, "deja lu", 200, true),
                    envelope(12, "vraiment nouveau", 300, false),
                ],
            )
            .unwrap();

        let arrivals = store.new_unread_after(account, "INBOX", 10, 20).unwrap();
        let subjects: Vec<_> = arrivals
            .iter()
            .map(|e| e.subject.clone().unwrap_or_default())
            .collect();
        assert_eq!(subjects, vec!["vraiment nouveau".to_string()]);
    }

    fn folder(wire: &str, display: &str) -> Folder {
        Folder {
            wire: wire.to_string(),
            display: display.to_string(),
            selectable: true,
            special_use: None,
        }
    }

    /// Choisir une destination doit fonctionner HORS LIGNE : la liste est
    /// donc lue localement, comme les enveloppes. Le nom réseau et le nom
    /// lisible sont conservés tous les deux — perdre le premier rendrait
    /// le déplacement irréalisable au rejeu.
    #[test]
    fn folders_are_cached_locally_with_both_names() {
        let (store, _) = store_with_mailbox();
        let account = test_account(&store);
        assert!(store.folders(account).unwrap().is_empty());

        store
            .replace_folders(account, &[folder("Archiv&AOk-s", "Archivés")])
            .unwrap();

        let cached = store.folders(account).unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].wire, "Archiv&AOk-s");
        assert_eq!(cached[0].display, "Archivés");
    }

    /// Un dossier supprimé côté serveur ne doit plus être proposé : le
    /// déplacement échouerait au rejeu, longtemps après le clic — et
    /// l'utilisateur ne ferait plus le lien.
    #[test]
    fn refreshing_folders_drops_the_ones_that_disappeared() {
        let (store, _) = store_with_mailbox();
        let account = test_account(&store);
        store
            .replace_folders(
                account,
                &[folder("Ancien", "Ancien"), folder("Reste", "Reste")],
            )
            .unwrap();

        store
            .replace_folders(account, &[folder("Reste", "Reste")])
            .unwrap();

        let cached = store.folders(account).unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].wire, "Reste");
    }

    #[test]
    fn attachments_are_saved_with_the_body_and_read_back_in_order() {
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        assert!(
            store.attachments(account, "INBOX", 1).unwrap().is_empty(),
            "rien tant que le corps n'est pas rapatrié"
        );

        store
            .save_body(
                id,
                1,
                "<p>ci-joint</p>",
                &[pdf(0, "un.pdf"), pdf(1, "deux.pdf")],
            )
            .unwrap();

        let found = store.attachments(account, "INBOX", 1).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "un.pdf");
        assert_eq!(found[1].name, "deux.pdf");
        assert_eq!(found[1].size, 2048);
    }

    /// Un message re-téléchargé dont une pièce a disparu ne doit pas
    /// garder l'ancienne ligne : l'utilisateur cliquerait sur un fichier
    /// que le serveur ne sert plus, et l'échec n'arriverait qu'au
    /// téléchargement — loin de la cause.
    #[test]
    fn re_saving_replaces_the_attachment_list_instead_of_accumulating() {
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .save_body(id, 1, "<p>x</p>", &[pdf(0, "un.pdf"), pdf(1, "deux.pdf")])
            .unwrap();

        store
            .save_body(id, 1, "<p>x</p>", &[pdf(0, "un.pdf")])
            .unwrap();

        let found = store.attachments(account, "INBOX", 1).unwrap();
        assert_eq!(found.len(), 1, "la pièce disparue doit l'être aussi ici");
        assert_eq!(found[0].name, "un.pdf");
    }

    /// Les pièces jointes appartiennent à un message d'un COMPTE : la
    /// même paire (boîte, uid) chez un autre compte ne doit rien voir.
    #[test]
    fn attachments_never_leak_across_accounts() {
        let (store, id) = store_with_mailbox();
        store
            .save_body(id, 1, "<p>x</p>", &[pdf(0, "prive.pdf")])
            .unwrap();

        let other = store
            .adopt_or_create_account("autre@exemple.fr", "gmail")
            .unwrap();
        store.create_mailbox(other, "INBOX", 1).unwrap();

        assert!(store.attachments(other, "INBOX", 1).unwrap().is_empty());
    }

    fn invitation_projet() -> crate::InvitationRow {
        crate::InvitationRow {
            methode: "request".into(),
            event_uid: "reunion-1@exemple.fr".into(),
            sequence: 2,
            titre: "Point projet".into(),
            lieu: Some("Salle A".into()),
            organisateur_adresse: Some("claire@exemple.fr".into()),
            organisateur_nom: Some("Claire Martin".into()),
            debut_epoch: Some(1_788_400_200),
            fin_epoch: Some(1_788_402_000),
            partstat: Some("sans_reponse".into()),
            ..Default::default()
        }
    }

    #[test]
    fn une_invitation_s_ecrit_avec_le_corps_et_se_relit() {
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .save_body_full(id, 1, "<p>x</p>", &[], Some(&invitation_projet()))
            .unwrap();

        let stockee = store
            .invitation(account, "INBOX", 1)
            .unwrap()
            .expect("ligne");
        assert_eq!(stockee.row, invitation_projet());
        assert_eq!(stockee.reponse, None, "pas encore répondu");
    }

    /// Même règle que les pièces : un message re-téléchargé SANS partie
    /// calendrier ne garde pas une carte fantôme.
    #[test]
    fn un_rescan_sans_calendrier_efface_la_ligne() {
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .save_body_full(id, 1, "<p>x</p>", &[], Some(&invitation_projet()))
            .unwrap();
        store.save_body(id, 1, "<p>x</p>", &[]).unwrap();
        assert_eq!(store.invitation(account, "INBOX", 1).unwrap(), None);
    }

    fn brouillon_reponse() -> crate::compose::Draft {
        let mut draft = crate::compose(
            "moi@exemple.fr",
            "claire@exemple.fr",
            "",
            "",
            "Accepté : Point projet",
            "Accepté : Point projet",
            None,
        )
        .unwrap();
        draft.ics_reply = Some("BEGIN:VCALENDAR\r\nMETHOD:REPLY\r\nEND:VCALENDAR\r\n".into());
        draft
    }

    /// D6 : l'email iTIP se journalise ET la réponse se consigne — UNE
    /// transaction ; la réponse survit au re-scan du corps (deux vérités
    /// distinctes — le PARTSTAT lu du message ne l'écrase pas).
    #[test]
    fn la_reponse_se_journalise_avec_son_email_et_survit_au_rescan() {
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .save_body_full(id, 1, "<p>x</p>", &[], Some(&invitation_projet()))
            .unwrap();

        let outbox_id = store
            .enqueue_reponse_invitation(
                account,
                &brouillon_reponse(),
                "INBOX",
                1,
                "accepte",
                1_755_900_000,
            )
            .unwrap();
        assert!(outbox_id.is_some(), "email journalisé");
        store
            .save_body_full(id, 1, "<p>x</p>", &[], Some(&invitation_projet()))
            .unwrap();

        let stockee = store
            .invitation(account, "INBOX", 1)
            .unwrap()
            .expect("ligne");
        assert_eq!(stockee.reponse.as_deref(), Some("accepte"));
        assert_eq!(stockee.reponse_epoch, Some(1_755_900_000));
        assert_eq!(store.outbox_to_send(account).unwrap().len(), 1);
    }

    /// La ligne a disparu entre l'affichage et le clic (expurgé, boîte
    /// réinitialisée) : RIEN ne part — un email en file devant une carte
    /// « pas répondu » inviterait au double envoi (revue).
    #[test]
    fn une_reponse_sans_ligne_ne_journalise_rien() {
        let (store, _id) = store_with_mailbox();
        let account = test_account(&store);
        assert_eq!(
            store
                .enqueue_reponse_invitation(account, &brouillon_reponse(), "INBOX", 9, "accepte", 1)
                .unwrap(),
            None
        );
        assert!(
            store.outbox_to_send(account).unwrap().is_empty(),
            "la transaction s'est rembobinée : aucun email en file"
        );
    }

    /// La revue PLAN-INVITATIONS : après un changement d'UIDVALIDITY,
    /// les UIDs ne veulent plus rien dire — une carte (et sa réponse !)
    /// qui survivrait collerait à un message sans rapport.
    #[test]
    fn reset_mailbox_efface_invitations_et_pieces() {
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .save_body_full(
                id,
                1,
                "<p>x</p>",
                &[pdf(0, "un.pdf")],
                Some(&invitation_projet()),
            )
            .unwrap();

        store.reset_mailbox(id, 2).unwrap();

        assert_eq!(store.invitation(account, "INBOX", 1).unwrap(), None);
        assert!(store.attachments(account, "INBOX", 1).unwrap().is_empty());
    }

    /// La réparation `pieces-calendrier` : un message scanné AVANT
    /// PLAN-INVITATIONS avec une partie calendrier a des indices de
    /// pièces DÉCALÉS (l'ancienne numérotation la comptait) et pas de
    /// carte. À l'ouverture suivante de la base, corps et pièces de ces
    /// messages sont jetés : le rattrapage les relira avec la
    /// numérotation neuve — et la carte naîtra du même scan (adoption,
    /// invariant §6.7). Sur base FICHIER : c'est la réouverture qui
    /// répare. Les messages sans calendrier ne bougent pas.
    #[test]
    fn la_reparation_pieces_calendrier_fait_relire_les_messages_touches() {
        let path = std::env::temp_dir().join(format!(
            "wind-test-reparation-cal-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("moi@exemple.fr", "gmail")
                .unwrap();
            let id = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(
                    id,
                    &[
                        envelope(1, "invitation", 100, true),
                        envelope(2, "simple", 90, true),
                    ],
                )
                .unwrap();
            // L'état d'AVANT : la partie calendrier comptée en pièce 0.
            store
                .save_body(
                    id,
                    1,
                    "<p>invitation</p>",
                    &[
                        Attachment {
                            index: 0,
                            name: "piece-jointe.calendar".into(),
                            mime: "text/calendar".into(),
                            size: 2048,
                        },
                        pdf(1, "contrat.pdf"),
                    ],
                )
                .unwrap();
            store
                .save_body(id, 2, "<p>simple</p>", &[pdf(0, "note.pdf")])
                .unwrap();
            // Retire le marqueur posé à l'ouverture (base née réparée) :
            // on rejoue l'arrivée d'une base d'AVANT la réparation.
            store
                .conn()
                .execute(
                    "DELETE FROM reparations WHERE nom = 'pieces-calendrier'",
                    [],
                )
                .unwrap();
        }

        Store::oublier_initialisation(&path);
        let store = Store::open(&path).unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        assert_eq!(
            store.body(account, "INBOX", 1).unwrap(),
            None,
            "le message à calendrier sera relu"
        );
        assert!(store.attachments(account, "INBOX", 1).unwrap().is_empty());
        assert_eq!(
            store.body(account, "INBOX", 2).unwrap().as_deref(),
            Some("<p>simple</p>"),
            "le message ordinaire ne bouge pas"
        );
        assert_eq!(store.attachments(account, "INBOX", 2).unwrap().len(), 1);
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// Terrain R6 : un CANCEL éteint le REQUEST de la même réunion
    /// (même event_uid, même compte), dans les DEUX ordres d'arrivée —
    /// l'annulation arrive souvent dans une conversation neuve, c'est
    /// la carte d'ORIGINE qui doit le dire.
    #[test]
    fn un_cancel_eteint_le_request_de_la_meme_reunion_dans_les_deux_ordres() {
        let mut cancel = invitation_projet();
        cancel.methode = "cancel".to_string();
        cancel.annule = true;

        // Ordre 1 : le REQUEST d'abord, le CANCEL ensuite.
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .save_body_full(id, 1, "<p>i</p>", &[], Some(&invitation_projet()))
            .unwrap();
        store
            .save_body_full(id, 2, "<p>a</p>", &[], Some(&cancel))
            .unwrap();
        assert!(
            store
                .invitation(account, "INBOX", 1)
                .unwrap()
                .expect("ligne")
                .row
                .annule,
            "le REQUEST est éteint par le CANCEL"
        );

        // Une AUTRE réunion du même compte ne bouge pas.
        let mut autre = invitation_projet();
        autre.event_uid = "autre-reunion@exemple.fr".to_string();
        store
            .save_body_full(id, 3, "<p>x</p>", &[], Some(&autre))
            .unwrap();
        assert!(
            !store
                .invitation(account, "INBOX", 3)
                .unwrap()
                .expect("ligne")
                .row
                .annule
        );

        // Ordre 2 : le CANCEL scanné AVANT (rattrapage dans le
        // désordre) — le REQUEST naît annulé.
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .save_body_full(id, 2, "<p>a</p>", &[], Some(&cancel))
            .unwrap();
        store
            .save_body_full(id, 1, "<p>i</p>", &[], Some(&invitation_projet()))
            .unwrap();
        assert!(
            store
                .invitation(account, "INBOX", 1)
                .unwrap()
                .expect("ligne")
                .row
                .annule
        );
    }

    #[test]
    fn une_invitation_ne_fuit_pas_entre_comptes() {
        let (store, id) = store_with_mailbox();
        store
            .save_body_full(id, 1, "<p>x</p>", &[], Some(&invitation_projet()))
            .unwrap();

        let autre = store
            .adopt_or_create_account("autre@exemple.fr", "gmail")
            .unwrap();
        store.create_mailbox(autre, "INBOX", 1).unwrap();

        assert_eq!(store.invitation(autre, "INBOX", 1).unwrap(), None);
    }

    #[test]
    fn reset_mailbox_clears_bodies_too() {
        let (store, id) = store_with_mailbox();
        store.save_body(id, 1, "<p>x</p>", &[]).unwrap();
        store.reset_mailbox(id, 2).unwrap();
        assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
    }

    #[test]
    fn envelope_returns_reply_context_fields() {
        let (mut store, id) = store_with_mailbox();
        let original = envelope(7, "sujet", 100, false);
        store
            .upsert_envelopes(id, std::slice::from_ref(&original))
            .unwrap();

        assert_eq!(
            store.envelope(test_account(&store), "INBOX", 7).unwrap(),
            Some(original)
        );
        assert_eq!(
            store.envelope(test_account(&store), "INBOX", 99).unwrap(),
            None
        );
    }

    /// ADR 0011 : sur une base FICHIER, l'ouverture passe en WAL — et le
    /// mode persiste, une base héritée en rollback est convertie. C'est ce
    /// qui empêche « database is locked » quand la jauge d'avancement lit
    /// pendant qu'une synchronisation intégrale écrit — le premier défaut
    /// que le terrain ait rendu sur l'ADR 0010.
    ///
    /// Sur base fichier et non en mémoire, comme le terrain : une base en
    /// mémoire répond « memory » à ce PRAGMA, et le test validerait un
    /// modèle faux.
    #[test]
    fn une_base_fichier_s_ouvre_en_wal() {
        let path = std::env::temp_dir().join(format!("wind-test-wal-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);

        // Une base héritée, née AVANT le WAL : mode rollback (delete).
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("CREATE TABLE heritage (id INTEGER)")
                .unwrap();
        }

        {
            let _store = Store::open(&path).unwrap();
            let conn = Connection::open(&path).unwrap();
            let mode: String = conn
                .query_row("PRAGMA journal_mode", [], |row| row.get(0))
                .unwrap();
            assert_eq!(mode.to_lowercase(), "wal", "la base héritée est convertie");
        }
        let _ = std::fs::remove_file(&path);
    }

    /// PLAN-AUDIT-V2 E1 : chaque commande du shell ouvre SA connexion —
    /// 103 sites — et chacune rejouait le schéma, une vingtaine de
    /// `table_xinfo` et les migrations (36 ms sur 200 k enveloppes, à
    /// CHAQUE commande). Une fois l'initialisation complète RÉUSSIE sur un
    /// chemin, les ouvertures suivantes du même processus ne la rejouent
    /// pas. Preuve sans espion dans le code de production : on retire un
    /// index derrière le dos du Store ; si le schéma était rejoué, le
    /// `CREATE INDEX IF NOT EXISTS` le recréerait.
    #[test]
    fn une_seconde_ouverture_du_meme_chemin_ne_rejoue_pas_le_schema() {
        let path =
            std::env::temp_dir().join(format!("wind-test-porte-rapide-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        drop(Store::open(&path).unwrap());

        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("DROP INDEX idx_pending_actions_message")
                .unwrap();
        }
        drop(Store::open(&path).unwrap());

        let conn = Connection::open(&path).unwrap();
        let recree: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_pending_actions_message'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(recree, 0, "la seconde ouverture a rejoué le schéma");
        let _ = std::fs::remove_file(&path);
    }

    /// La reconstruction de l'index de recherche doit faire afficher l'écran
    /// de migration (ADR 0012) même sur une base DÉJÀ à jour côté fils : sans
    /// cette détection dans `pending_adoption`, elle gèlerait le démarrage en
    /// silence (constat terrain 2026-08-17). Sur base fichier, car la sonde
    /// ouvre en lecture seule — une base en mémoire n'a pas de chemin.
    #[test]
    fn pending_adoption_sees_an_old_search_index() {
        let path =
            std::env::temp_dir().join(format!("wind-test-search-migr-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let mut store = Store::open(&path).unwrap();
            let account = test_account(&store);
            let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(mailbox, &[envelope(1, "Sujet", 100, false)])
                .unwrap();
            // Rétrograde l'index vers l'ancien schéma à trois colonnes : les
            // fils restent adoptés (`user_version` inchangé), seul l'index est
            // d'avant ce chantier — exactement l'état du terrain.
            store
                .conn()
                .execute_batch(
                    "DROP TABLE search_fts;
                     DROP TABLE search_docs;
                     CREATE TABLE search_docs (
                        docid      INTEGER PRIMARY KEY,
                        mailbox_id INTEGER NOT NULL,
                        uid        INTEGER NOT NULL,
                        UNIQUE (mailbox_id, uid)
                     );
                     CREATE VIRTUAL TABLE search_fts USING fts5(
                        subject, sender, body,
                        content='', contentless_delete=1,
                        tokenize='unicode61 remove_diacritics 2'
                     );",
                )
                .unwrap();
        } // fermeture propre → checkpoint du WAL, la sonde lecture seule lit.

        assert_eq!(
            Store::pending_adoption(&path).unwrap(),
            Some(1),
            "l'ancien schéma FTS fait afficher l'écran, fils déjà adoptés"
        );

        // Une ouverture pleine reconstruit ; ensuite, plus rien à annoncer.
        Store::oublier_initialisation(&path);
        {
            Store::open(&path).unwrap();
        }
        assert_eq!(
            Store::pending_adoption(&path).unwrap(),
            None,
            "reconstruit → l'écran ne se réaffiche pas"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// Une base Phase 1 (sans les colonnes de réponse) doit s'ouvrir et
    /// s'enrichir sans perdre les enveloppes déjà synchronisées.
    #[test]
    fn opens_and_migrates_a_phase1_database() {
        let path =
            std::env::temp_dir().join(format!("wind-test-migration-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE mailboxes (
                    id             INTEGER PRIMARY KEY,
                    name           TEXT NOT NULL UNIQUE,
                    uid_validity   INTEGER NOT NULL,
                    last_uid       INTEGER NOT NULL DEFAULT 0,
                    highest_modseq INTEGER
                );
                CREATE TABLE envelopes (
                    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                    uid        INTEGER NOT NULL,
                    subject    TEXT,
                    sender     TEXT,
                    date_epoch INTEGER,
                    seen       INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (mailbox_id, uid)
                );
                INSERT INTO mailboxes (id, name, uid_validity) VALUES (1, 'INBOX', 1);
                INSERT INTO envelopes (mailbox_id, uid, subject, sender, date_epoch, seen)
                VALUES (1, 42, 'hérité de la phase 1', 'Alice', 100, 1);",
            )
            .unwrap();
        }

        let store = Store::open(&path).unwrap();
        let rows = recent(&store, 0, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].uid, 42);
        assert_eq!(rows[0].subject.as_deref(), Some("hérité de la phase 1"));
        assert_eq!(
            rows[0].sender_address, None,
            "colonne ajoutée par migration : valeur inconnue pour l'existant"
        );
        assert!(
            !rows[0].flagged,
            "étoile absente par défaut après migration"
        );

        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// R2 (PLAN-RETOURS-MAIL) : une enveloppe synchronisée AVANT le
    /// correctif porte les backslash-escapes IMAP dans son objet et le nom
    /// de son expéditeur ; la migration les retire une fois. Le cas terrain
    /// « Test \"Envoyés\" ».
    #[test]
    fn migration_retire_les_escapes_imap_des_objets_existants() {
        let path =
            std::env::temp_dir().join(format!("wind-test-escapes-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE mailboxes (
                    id             INTEGER PRIMARY KEY,
                    name           TEXT NOT NULL UNIQUE,
                    uid_validity   INTEGER NOT NULL,
                    last_uid       INTEGER NOT NULL DEFAULT 0,
                    highest_modseq INTEGER
                );
                CREATE TABLE envelopes (
                    mailbox_id     INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                    uid            INTEGER NOT NULL,
                    subject        TEXT,
                    sender         TEXT,
                    sender_address TEXT,
                    date_epoch     INTEGER,
                    seen           INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (mailbox_id, uid)
                );
                INSERT INTO mailboxes (id, name, uid_validity) VALUES (1, 'INBOX', 1);",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO envelopes
                    (mailbox_id, uid, subject, sender, sender_address, date_epoch, seen)
                 VALUES (1, 7, ?1, ?2, ?3, 100, 1)",
                params![r#"Test \"Envoyes\""#, r#"Societe \"ACME\""#, "info@acme.fr"],
            )
            .unwrap();
            // Un objet propre, sans escape : il doit traverser intact.
            conn.execute(
                "INSERT INTO envelopes (mailbox_id, uid, subject, sender, date_epoch, seen)
                 VALUES (1, 8, 'Reunion de demain', 'Alice', 90, 1)",
                [],
            )
            .unwrap();
        }

        let store = Store::open(&path).unwrap();
        let rows = recent(&store, 0, 10);
        let sept = rows.iter().find(|e| e.uid == 7).unwrap();
        assert_eq!(sept.subject.as_deref(), Some(r#"Test "Envoyes""#));
        assert_eq!(sept.sender.as_deref(), Some(r#"Societe "ACME""#));
        let huit = rows.iter().find(|e| e.uid == 8).unwrap();
        assert_eq!(huit.subject.as_deref(), Some("Reunion de demain"));

        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// Migration Phase 2 → 3 sur une base complète : toutes les données
    /// (enveloppes, corps, actions, brouillons, tombstones, boîte d'envoi)
    /// sont adoptées par le compte en attente — zéro perte, et la première
    /// connexion revendique le tout.
    #[test]
    fn migrates_a_full_phase2_database_and_adopts_everything() {
        let path =
            std::env::temp_dir().join(format!("wind-test-migration-p2-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE mailboxes (
                    id             INTEGER PRIMARY KEY,
                    name           TEXT NOT NULL UNIQUE,
                    uid_validity   INTEGER NOT NULL,
                    last_uid       INTEGER NOT NULL DEFAULT 0,
                    highest_modseq INTEGER
                );
                CREATE TABLE envelopes (
                    mailbox_id     INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                    uid            INTEGER NOT NULL,
                    subject        TEXT,
                    sender         TEXT,
                    sender_address TEXT,
                    message_id     TEXT,
                    date_epoch     INTEGER,
                    seen           INTEGER NOT NULL DEFAULT 0,
                    flagged        INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (mailbox_id, uid)
                );
                CREATE TABLE bodies (
                    mailbox_id INTEGER NOT NULL,
                    uid        INTEGER NOT NULL,
                    html       TEXT NOT NULL,
                    PRIMARY KEY (mailbox_id, uid)
                );
                CREATE TABLE pending_actions (
                    id INTEGER PRIMARY KEY, mailbox_id INTEGER NOT NULL,
                    uid INTEGER NOT NULL, kind TEXT NOT NULL
                );
                CREATE TABLE drafts (
                    id INTEGER PRIMARY KEY, to_raw TEXT NOT NULL,
                    subject TEXT NOT NULL, body TEXT NOT NULL,
                    reply_to_uid INTEGER, updated_epoch INTEGER NOT NULL,
                    remote_uid INTEGER, pushed_epoch INTEGER
                );
                CREATE TABLE draft_tombstones (remote_uid INTEGER PRIMARY KEY);
                CREATE TABLE drafts_remote (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    uid_validity INTEGER NOT NULL
                );
                CREATE TABLE outbox (
                    id INTEGER PRIMARY KEY, message_id TEXT NOT NULL,
                    sender TEXT NOT NULL, recipients TEXT NOT NULL,
                    subject TEXT NOT NULL, body_text TEXT NOT NULL,
                    in_reply_to TEXT, state TEXT NOT NULL DEFAULT 'queued',
                    attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT,
                    queued_epoch INTEGER NOT NULL
                );
                INSERT INTO mailboxes (id, name, uid_validity) VALUES (1, 'INBOX', 7);
                INSERT INTO envelopes (mailbox_id, uid, subject, seen, flagged)
                    VALUES (1, 42, 'hérité', 1, 1);
                INSERT INTO bodies VALUES (1, 42, '<p>corps</p>');
                INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (1, 42, 'mark_seen');
                INSERT INTO drafts (to_raw, subject, body, updated_epoch, remote_uid, pushed_epoch)
                    VALUES ('x@y.fr', 'précieux', 'texte', 10, 77, 10);
                INSERT INTO draft_tombstones VALUES (99);
                INSERT INTO drafts_remote VALUES (1, 1234);
                INSERT INTO outbox (message_id, sender, recipients, subject, body_text, queued_epoch)
                    VALUES ('<m@x>', 'moi@y.fr', 'toi@y.fr', 's', 'b', 20);",
            )
            .unwrap();
        }

        let store = Store::open(&path).unwrap();
        let account = store
            .adopt_or_create_account("legacy@exemple.fr", "gmail")
            .unwrap();
        assert_eq!(account, 1, "la revendication prend le compte en attente");

        assert_eq!(store.recent(account, "INBOX", 0, 10).unwrap()[0].uid, 42);
        assert_eq!(
            store.body(1, "INBOX", 42).unwrap().as_deref(),
            Some("<p>corps</p>")
        );
        let drafts = store.drafts().unwrap();
        assert_eq!(drafts[0].account_id, 1);
        assert_eq!(drafts[0].remote_uid, Some(77));
        assert_eq!(store.draft_tombstones(1).unwrap(), vec![99]);
        assert!(
            !store.align_drafts_uidvalidity(1, 1234).unwrap(),
            "l'UIDVALIDITY des brouillons a survécu : pas de réinitialisation"
        );
        assert_eq!(store.outbox_to_send(1).unwrap().len(), 1);
        assert_eq!(store.accounts().unwrap().len(), 1);

        let second = store
            .adopt_or_create_account("deux@exemple.fr", "gmail")
            .unwrap();
        assert_ne!(second, 1, "le placeholder ne se revendique qu'une fois");

        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// PLAN-COMPOSITION-HTML E1 : une base héritée (d'avant le corps
    /// HTML) gagne les colonnes `body_html` de `drafts` et `outbox` à
    /// l'ouverture — NULL sur l'existant, le chemin texte intact.
    /// Sur base de FICHIER : c'est la passe réelle qui est prouvée,
    /// pas un schéma neuf (invariant #7).
    #[test]
    fn legacy_database_gains_body_html_columns_with_null_on_existing_rows() {
        let path =
            std::env::temp_dir().join(format!("wind-test-body-html-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE drafts (
                    id INTEGER PRIMARY KEY, to_raw TEXT NOT NULL,
                    subject TEXT NOT NULL, body TEXT NOT NULL,
                    reply_to_uid INTEGER, updated_epoch INTEGER NOT NULL,
                    remote_uid INTEGER, pushed_epoch INTEGER
                );
                CREATE TABLE outbox (
                    id INTEGER PRIMARY KEY, message_id TEXT NOT NULL,
                    sender TEXT NOT NULL, recipients TEXT NOT NULL,
                    subject TEXT NOT NULL, body_text TEXT NOT NULL,
                    in_reply_to TEXT, state TEXT NOT NULL DEFAULT 'queued',
                    attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT,
                    queued_epoch INTEGER NOT NULL
                );
                INSERT INTO drafts (to_raw, subject, body, updated_epoch)
                    VALUES ('x@y.fr', 's', 'texte brut', 10);
                INSERT INTO outbox (message_id, sender, recipients, subject, body_text, queued_epoch)
                    VALUES ('<m@x>', 'moi@y.fr', 'toi@y.fr', 's', 'b', 20);",
            )
            .unwrap();
        }

        let store = Store::open(&path).unwrap();
        for table in ["drafts", "outbox"] {
            assert!(
                table_columns(store.conn(), table)
                    .unwrap()
                    .contains("body_html"),
                "{table} doit gagner body_html à l'ouverture"
            );
        }
        let ancien: Option<String> = store
            .conn()
            .query_row("SELECT body_html FROM drafts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(ancien, None, "l'existant reste NULL : chemin texte intact");
        let ancien: Option<String> = store
            .conn()
            .query_row("SELECT body_html FROM outbox", [], |row| row.get(0))
            .unwrap();
        assert_eq!(ancien, None);

        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// Le cœur produit du multi-comptes : la même boîte de tous les
    /// comptes, fusionnée par date — chaque ligne connaît son compte.
    #[test]
    fn unified_recent_merges_accounts_by_date() {
        let store = Store::open_in_memory().unwrap();
        let first = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let second = store
            .adopt_or_create_account("b@exemple.fr", "gmail")
            .unwrap();
        let inbox_a = store.create_mailbox(first, "INBOX", 1).unwrap();
        let inbox_b = store.create_mailbox(second, "INBOX", 1).unwrap();

        let mut store = store;
        store
            .upsert_envelopes(
                inbox_a,
                &[
                    envelope(1, "a-ancien", 100, false),
                    envelope(2, "a-récent", 300, false),
                ],
            )
            .unwrap();
        store
            .upsert_envelopes(
                inbox_b,
                &[
                    envelope(1, "b-milieu", 200, false),
                    envelope(2, "b-dernier", 400, false),
                ],
            )
            .unwrap();

        let rows = store.unified_recent(0, 10).unwrap();
        let order: Vec<(&str, &str)> = rows
            .iter()
            .map(|row| {
                (
                    row.account_email.as_str(),
                    row.envelope.subject.as_deref().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            order,
            vec![
                ("b@exemple.fr", "b-dernier"),
                ("a@exemple.fr", "a-récent"),
                ("b@exemple.fr", "b-milieu"),
                ("a@exemple.fr", "a-ancien"),
            ],
            "fusion par date, chaque ligne porte son compte"
        );
        assert_eq!(store.unified_count().unwrap(), 4);
        // Même UID dans deux comptes : deux messages distincts.
        assert!(store.envelope(first, "INBOX", 1).unwrap().is_some());
        assert!(store.envelope(second, "INBOX", 1).unwrap().is_some());
    }

    #[test]
    fn remove_absent_drops_orphaned_bodies() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();
        store.save_body(id, 1, "<p>x</p>", &[]).unwrap();
        assert_eq!(store.remove_absent(id, &HashSet::new()).unwrap(), 1);
        assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
    }

    /// Réparation `corps-fffd` : un corps mutilé au décodage (U+FFFD) est
    /// purgé pour que le rattrapage le retélécharge avec le décodeur
    /// corrigé ; un corps sain reste en place.
    #[test]
    fn reparation_corps_fffd_purge_les_corps_mutiles() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[envelope(1, "a", 100, false), envelope(2, "b", 100, false)],
            )
            .unwrap();
        store
            .save_body(id, 1, "<p>journ\u{FFFD}es</p>", &[])
            .unwrap();
        store.save_body(id, 2, "<p>sain</p>", &[]).unwrap();
        // Simule une base d'avant la réparation : le marqueur disparaît,
        // et la migration se rejoue comme à la prochaine ouverture.
        store
            .conn()
            .execute("DELETE FROM reparations WHERE nom = 'corps-fffd'", [])
            .unwrap();
        migrate(store.conn(), &mut |_| ControlFlow::Continue(())).unwrap();
        let account = test_account(&store);
        assert_eq!(
            store.body(account, "INBOX", 1).unwrap(),
            None,
            "corps mutilé purgé"
        );
        assert!(
            store.body(account, "INBOX", 2).unwrap().is_some(),
            "corps sain conservé"
        );
        // Le message purgé redevient une cible du rattrapage.
        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            vec![1]
        );
    }

    /// Régression (bug #2) : ré-ajouter un compte générique déjà connu
    /// doit renvoyer le MÊME id et appliquer la nouvelle configuration.
    /// Sur le chemin UPDATE de l'upsert, `last_insert_rowid()` renvoyait
    /// 0 — un id fantôme que l'UI récupérait pour la pastille et la
    /// sélection. Chaque commande ouvre SA connexion : on modélise donc
    /// le ré-ajout par deux `Store` distincts sur la même base fichier,
    /// car c'est la connexion fraîche (sans INSERT préalable) qui emprunte
    /// le chemin UPDATE et exhibe le 0.
    #[test]
    fn re_adding_a_generic_account_returns_the_same_id_and_updates_config() {
        let path = std::env::temp_dir().join(format!(
            "wind-test-generic-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let first = {
            let store = Store::open(&path).unwrap();
            store
                .create_generic_account(
                    "compte@exemple.fr",
                    "compte",
                    "imap.a.fr",
                    993,
                    "smtp.a.fr",
                    465,
                )
                .unwrap()
        };
        let second = {
            let store = Store::open(&path).unwrap();
            store
                .create_generic_account(
                    "compte@exemple.fr",
                    "login",
                    "imap.b.fr",
                    143,
                    "smtp.b.fr",
                    587,
                )
                .unwrap()
        };
        let (count, config) = {
            let store = Store::open(&path).unwrap();
            (
                store.accounts().unwrap().len(),
                store.account_config(first).unwrap(),
            )
        };
        // Nettoyage avant les assertions : un échec ne doit pas laisser de
        // fichier temporaire derrière lui.
        let _ = std::fs::remove_file(&path);

        assert!(first > 0, "la primo-création doit renvoyer un id réel");
        assert_eq!(
            second, first,
            "le ré-ajout doit renvoyer l'id existant, jamais 0"
        );
        assert_eq!(count, 1, "un seul compte, pas un doublon");
        assert_eq!(config.username.as_deref(), Some("login"));
        assert_eq!(config.imap_host.as_deref(), Some("imap.b.fr"));
        assert_eq!(config.imap_port, Some(143));
        assert_eq!(config.smtp_host.as_deref(), Some("smtp.b.fr"));
        assert_eq!(config.smtp_port, Some(587));
    }

    /// Le rattrapage vise les messages RÉCENTS sans corps, du plus récent
    /// au plus ancien : c'est l'ordre où la recherche a le plus de valeur,
    /// et celui qui rend la reprise après coupure naturelle.
    #[test]
    fn backfill_lists_recent_bodyless_messages_newest_first() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "ancien", 1_000, false),
                    envelope(2, "milieu", 2_000, false),
                    envelope(3, "récent", 3_000, false),
                ],
            )
            .unwrap();
        let account = test_account(&store);

        let todo = store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap();
        assert_eq!(todo, vec![3, 2, 1]);
    }

    #[test]
    fn backfill_skips_messages_that_already_have_a_body() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "sans corps", 1_000, false),
                    envelope(2, "avec corps", 2_000, false),
                ],
            )
            .unwrap();
        store.save_body(id, 2, "<p>déjà là</p>", &[]).unwrap();
        let account = test_account(&store);

        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            vec![1]
        );
    }

    /// L'horizon de récence est ce qui BORNE le coût (ADR 0007) : au-delà,
    /// on ne rapatrie rien.
    #[test]
    fn backfill_respects_the_recency_horizon() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "hors horizon", 1_000, false),
                    envelope(2, "dans l'horizon", 5_000, false),
                ],
            )
            .unwrap();
        let account = test_account(&store);

        assert_eq!(
            store
                .bodies_to_backfill(account, "INBOX", 4_000, 10)
                .unwrap(),
            vec![2]
        );
    }

    #[test]
    fn backfill_honours_the_batch_limit() {
        let (mut store, id) = store_with_mailbox();
        let envelopes: Vec<Envelope> = (1..=10)
            .map(|uid| envelope(uid, "message", uid as i64 * 100, false))
            .collect();
        store.upsert_envelopes(id, &envelopes).unwrap();
        let account = test_account(&store);

        assert_eq!(
            store
                .bodies_to_backfill(account, "INBOX", 0, 3)
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn backfill_never_leaks_another_accounts_messages() {
        let (mut store, mine) = store_with_mailbox();
        let other = store
            .adopt_or_create_account("autre@exemple.fr", "gmail")
            .unwrap();
        let theirs = store.create_mailbox(other, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(mine, &[envelope(1, "à moi", 1_000, false)])
            .unwrap();
        store
            .upsert_envelopes(theirs, &[envelope(1, "à l'autre", 2_000, false)])
            .unwrap();
        let account = test_account(&store);

        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            vec![1],
            "un seul message : celui du compte demandé"
        );
        assert_eq!(
            store.bodies_to_backfill(other, "INBOX", 0, 10).unwrap(),
            vec![1]
        );
    }

    // -----------------------------------------------------------------
    // Regroupement en conversations
    // -----------------------------------------------------------------

    /// Une réponse à `parent`, dans le format de [`envelope`] — dont le
    /// `Message-ID` est `<m{uid}@example.com>`.
    fn reply(uid: Uid, subject: &str, epoch: i64, seen: bool, parent: Uid) -> Envelope {
        Envelope {
            in_reply_to: Some(format!("<m{parent}@example.com>")),
            ..envelope(uid, subject, epoch, seen)
        }
    }

    fn unified(store: &Store) -> Vec<UnifiedRow> {
        store.unified_recent(0, 50).unwrap()
    }

    fn uids(rows: &[UnifiedRow]) -> Vec<Uid> {
        rows.iter().map(|row| row.envelope.uid).collect()
    }

    /// Le cœur du chantier : deux messages, une seule ligne.
    #[test]
    fn la_liste_montre_une_ligne_par_conversation() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Devis", 100, true),
                    reply(2, "Re: Devis", 200, true, 1),
                ],
            )
            .unwrap();

        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "un fil, et non deux messages");
        assert_eq!(rows[0].thread_size, 2);
        assert_eq!(
            rows[0].envelope.uid, 2,
            "la ligne montre le DERNIER message"
        );
        assert_eq!(
            store.unified_count().unwrap(),
            1,
            "le défilement compte des conversations, sinon il défile dans le vide"
        );
    }

    #[test]
    fn une_reponse_fait_remonter_tout_le_fil() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Devis", 100, true),
                    envelope(2, "Facture", 200, true),
                ],
            )
            .unwrap();
        assert_eq!(uids(&unified(&store)), vec![2, 1]);

        store
            .upsert_envelopes(id, &[reply(3, "Re: Devis", 300, true, 1)])
            .unwrap();

        let rows = unified(&store);
        assert_eq!(
            uids(&rows),
            vec![3, 2],
            "le devis repasse devant la facture"
        );
        assert_eq!(rows[0].thread_size, 2);
    }

    /// Un fil dont le dernier message est lu, mais qui garde un non-lu
    /// plus haut, doit rester en gras. Lire l'état du seul message affiché
    /// donnerait la réponse inverse.
    #[test]
    fn un_fil_reste_non_lu_tant_qu_un_de_ses_messages_l_est() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Devis", 100, false),
                    reply(2, "Re: Devis", 200, true, 1),
                ],
            )
            .unwrap();

        let rows = unified(&store);
        assert!(rows[0].envelope.seen, "le dernier message est lu…");
        assert_eq!(rows[0].thread_unseen, 1, "…mais le fil garde un non-lu");

        store.set_seen_local(id, 1, true).unwrap();
        assert_eq!(
            unified(&store)[0].thread_unseen,
            0,
            "lire le message manquant éteint le fil"
        );
    }

    /// Le cas qui justifie la passe sur les en-têtes complets : dans une
    /// boîte de réception, le message du milieu d'un échange est celui
    /// qu'on a soi-même ENVOYÉ — il n'y est pas. `In-Reply-To` seul laisse
    /// donc deux fils ; `References`, qui porte aussi la racine, les
    /// recolle.
    #[test]
    fn les_references_recollent_deux_moities_de_fil() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Devis", 100, true),
                    // Répond à <m2@…> : notre propre réponse, absente.
                    reply(3, "Re: Devis", 300, true, 2),
                ],
            )
            .unwrap();
        assert_eq!(unified(&store).len(), 2, "deux fils, faute du chaînon");

        assert!(
            store
                .set_thread_headers(id, 3, None, "<m1@example.com> <m2@example.com>")
                .unwrap(),
            "le rattachement a changé"
        );

        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "les deux moitiés se rejoignent");
        assert_eq!(rows[0].thread_size, 2);
        assert_eq!(rows[0].envelope.uid, 3);
    }

    /// Une re-synchronisation réécrit l'enveloppe. Si elle écrasait les
    /// `References` déjà acquises, elle DÉGROUPERAIT un fil recollé : le
    /// regroupement se déferait tout seul, sans que rien ne le signale.
    /// C'est le piège qui avait coûté les pièces jointes.
    #[test]
    fn une_resynchronisation_ne_degroupe_pas_un_fil_recolle() {
        let (mut store, id) = store_with_mailbox();
        let arrivee = [
            envelope(1, "Devis", 100, true),
            reply(3, "Re: Devis", 300, true, 2),
        ];
        store.upsert_envelopes(id, &arrivee).unwrap();
        store
            .set_thread_headers(id, 3, None, "<m1@example.com> <m2@example.com>")
            .unwrap();
        assert_eq!(unified(&store).len(), 1);

        store.upsert_envelopes(id, &arrivee).unwrap();

        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "le fil tient la re-synchronisation");
        assert_eq!(rows[0].thread_size, 2);
    }

    /// Le piège des pièces jointes appliqué aux fils : une base d'avant le
    /// regroupement a `thread_id` NULL partout. La liste part de
    /// `threads` — sans adoption, elle serait VIDE à la première
    /// ouverture, et pour toujours.
    #[test]
    fn une_base_heritee_voit_tous_ses_messages_adoptes() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Devis", 100, true),
                    envelope(2, "Facture", 200, true),
                ],
            )
            .unwrap();

        // On rembobine à l'état d'une base d'avant les fils.
        store
            .conn()
            .execute_batch(
                "UPDATE envelopes SET thread_id = NULL;
                 DELETE FROM thread_links;
                 DELETE FROM threads;",
            )
            .unwrap();
        assert!(
            unified(&store).is_empty(),
            "sans adoption, la boîte entière disparaît de l'écran"
        );

        crate::thread::migrate_threads(store.conn()).unwrap();

        assert_eq!(uids(&unified(&store)), vec![2, 1]);
        assert_eq!(store.unified_count().unwrap(), 2);
    }

    /// Le désordre d'arrivée ne doit rien changer : ici la réponse précède
    /// son parent dans le même lot.
    #[test]
    fn un_fil_se_lit_du_plus_ancien_au_plus_recent() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    reply(2, "Re: Devis", 200, true, 1),
                    envelope(1, "Devis", 100, true),
                ],
            )
            .unwrap();

        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "l'ordre d'arrivée ne casse pas le fil");
        let thread = rows[0].thread_id.unwrap();
        let messages = store.thread_messages(thread).unwrap();
        assert_eq!(uids(&messages), vec![1, 2]);
        // Chaque message repart en connaissant la taille de SON fil :
        // sinon l'écran qui le rouvre conclurait qu'il est seul.
        assert!(messages.iter().all(|m| m.thread_size == 2));
    }

    #[test]
    fn retirer_les_messages_d_un_fil_le_fait_disparaitre() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Devis", 100, true),
                    reply(2, "Re: Devis", 200, true, 1),
                ],
            )
            .unwrap();

        store.remove_local(id, 2).unwrap();
        let rows = unified(&store);
        assert_eq!(uids(&rows), vec![1], "le fil retombe sur ce qui reste");
        assert_eq!(rows[0].thread_size, 1);

        store.remove_local(id, 1).unwrap();
        assert!(unified(&store).is_empty());
        assert_eq!(store.unified_count().unwrap(), 0);
    }

    /// Le défaut du terrain, de bout en bout : deux messages étrangers
    /// dont l'`In-Reply-To` est une PHRASE — pas un identifiant — doivent
    /// rester deux conversations.
    ///
    /// Avant correction, chaque mot de la phrase devenait une ancre
    /// commune et les réunissait. Sur une vraie boîte, cela donnait un
    /// fil de 43 messages sans rapport les uns avec les autres.
    #[test]
    fn deux_messages_dont_l_en_tete_est_en_prose_ne_fusionnent_pas() {
        let (mut store, id) = store_with_mailbox();
        let prose = "Votre message du 3 janvier";
        store
            .upsert_envelopes(
                id,
                &[
                    Envelope {
                        in_reply_to: Some(prose.to_string()),
                        ..envelope(1, "Promotion", 100, true)
                    },
                    Envelope {
                        in_reply_to: Some(prose.to_string()),
                        ..envelope(2, "Autre promotion", 200, true)
                    },
                ],
            )
            .unwrap();

        assert_eq!(unified(&store).len(), 2, "aucun lien entre ces deux-là");
    }

    /// Une base regroupée par l'ancienne règle porte des fils FAUX, et
    /// corriger le code ne les répare pas tout seul. Le marqueur de
    /// version les fait refaire à l'ouverture — sans réseau, les en-têtes
    /// bruts étant intacts en base.
    #[test]
    fn une_base_mal_regroupee_est_refaite_a_l_ouverture() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Promotion", 100, true),
                    envelope(2, "Autre promotion", 200, true),
                ],
            )
            .unwrap();
        assert_eq!(unified(&store).len(), 2);

        // On rejoue l'état que produisait la règle permissive : un seul
        // fil pour deux messages étrangers, et la version d'avant.
        store
            .conn()
            .execute_batch(
                "DELETE FROM thread_links WHERE thread_id = (SELECT MAX(id) FROM threads);
                 UPDATE envelopes SET thread_id = (SELECT MIN(id) FROM threads);
                 DELETE FROM threads WHERE id = (SELECT MAX(id) FROM threads);
                 UPDATE threads SET size = 2, last_uid = 2, last_epoch = 200;
                 PRAGMA user_version = 0;",
            )
            .unwrap();
        assert_eq!(unified(&store).len(), 1, "l'état fautif est bien reproduit");

        crate::thread::migrate_threads(store.conn()).unwrap();

        assert_eq!(unified(&store).len(), 2, "les fils sont refaits");
        let version: i64 = store
            .conn()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        // Contre la CONSTANTE, jamais contre un littéral : chaque
        // changement de règle de regroupement l'incrémente, et un « 1 »
        // en dur ferait échouer ce test pour une raison qui n'est pas la
        // sienne.
        assert_eq!(
            version,
            crate::thread::THREADING_VERSION,
            "et la reconstruction ne se rejoue pas"
        );
    }

    /// UIDVALIDITY invalidée : les fils partent avec le reste, et
    /// l'annuaire ne doit pas empêcher une repopulation propre.
    #[test]
    fn reset_mailbox_efface_aussi_les_fils() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Devis", 100, true),
                    reply(2, "Re: Devis", 200, true, 1),
                ],
            )
            .unwrap();
        store.reset_mailbox(id, 2).unwrap();
        assert!(unified(&store).is_empty());

        store
            .upsert_envelopes(id, &[envelope(1, "Devis", 100, true)])
            .unwrap();
        assert_eq!(unified(&store).len(), 1, "la boîte se repeuple sans butée");
    }

    /// Rejoue sur `path` les tables telles que la version 1 des fils les
    /// créait — le seul décor où la passe d'adoption a du travail réel.
    /// Partagé par le test d'ouverture ci-dessous et par ceux du
    /// rembobinage (chantier Phase 5).
    fn rembobine_au_schema_v1(path: &Path) {
        // Une base rembobinée à la main est une base d'AVANT : le registre
        // de la porte rapide (E1) ne doit plus la connaître.
        Store::oublier_initialisation(path);
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "DROP TABLE thread_links;
             DROP TABLE threads;
             CREATE TABLE threads (
                 id         INTEGER PRIMARY KEY,
                 mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                 last_uid   INTEGER NOT NULL DEFAULT 0,
                 last_epoch INTEGER,
                 size       INTEGER NOT NULL DEFAULT 0,
                 unseen     INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX idx_threads_date
                 ON threads(mailbox_id, last_epoch DESC, last_uid DESC);
             CREATE TABLE thread_links (
                 mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                 message_id TEXT NOT NULL,
                 thread_id  INTEGER NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
                 PRIMARY KEY (mailbox_id, message_id)
             );
             CREATE INDEX idx_thread_links_thread ON thread_links(thread_id);
             UPDATE envelopes SET thread_id = NULL;
             PRAGMA user_version = 1;",
        )
        .unwrap();
    }

    /// Défaut trouvé au TERRAIN, pas ici : une base créée par la version
    /// précédente porte une table `threads` sans `inbox_size`.
    /// `CREATE TABLE IF NOT EXISTS` ne la touche pas — mais l'index
    /// partiel, lui, n'existe pas encore, donc SQLite le crée vraiment :
    /// il échoue sur une colonne absente, et **l'ouverture entière est
    /// refusée** (« no such column: inbox_size »). L'application ne
    /// démarrait plus.
    ///
    /// Aucun test ne pouvait l'attraper : ils créent tous une base neuve,
    /// donc déjà au schéma courant. Celui-ci REMBOBINE une vraie base au
    /// schéma d'avant — le seul décor où le défaut existe.
    #[test]
    fn une_base_au_schema_des_fils_precedent_s_ouvre_et_se_migre() {
        let path =
            std::env::temp_dir().join(format!("wind-test-fils-v1-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);

        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("moi@exemple.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            let mut premier = envelope(1, "Devis", 100, true);
            premier.message_id = Some("<a@exemple.fr>".to_string());
            let mut second = envelope(2, "Re: Devis", 200, true);
            second.message_id = Some("<b@exemple.fr>".to_string());
            second.in_reply_to = Some("<a@exemple.fr>".to_string());
            store.upsert_envelopes(inbox, &[premier, second]).unwrap();
            assert_eq!(unified(&store).len(), 1, "décor : un fil de deux messages");
        }

        // Rembobinage : les tables telles que la version 1 les créait.
        rembobine_au_schema_v1(&path);

        // C'est CETTE ouverture qui refusait de se faire.
        let store = Store::open(&path).unwrap();
        let lignes = unified(&store);
        assert_eq!(lignes.len(), 1, "le fil est refait, et la liste le montre");
        assert_eq!(lignes[0].thread_size, 2, "avec son compteur");

        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// LE test du chantier Phase 5 (passation §8) : l'adoption n'est PAS
    /// fractionnable — la liste part de `threads`, une adoption partielle
    /// persistée serait une boîte à moitié vide. « Interruptible » veut
    /// donc dire : annuler AU MILIEU de la passe défait TOUT et laisse
    /// `user_version` inchangé, pour que la passe entière se rejoue au
    /// prochain lancement — où la liste est complète.
    #[test]
    fn annuler_l_adoption_defait_tout_et_laisse_user_version_inchangee() {
        let path =
            std::env::temp_dir().join(format!("wind-test-rembobinage-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);

        // Assez de messages pour que l'annulation tombe en PLEINE passe :
        // l'avancement se rapporte par paliers, il faut en franchir un.
        const MESSAGES: u32 = 1_200;
        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("moi@exemple.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            let decor: Vec<Envelope> = (1..=MESSAGES)
                .map(|uid| envelope(uid, "Sujet", 100 + i64::from(uid), true))
                .collect();
            store.upsert_envelopes(inbox, &decor).unwrap();
        }
        rembobine_au_schema_v1(&path);

        // Annuler dès que 1 000 messages sont passés — au milieu, pas au
        // seuil de la porte : le rembobinage doit défaire du travail réel.
        let mut plus_haut_fait = 0;
        let result = Store::open_with_progress(&path, |p| {
            plus_haut_fait = plus_haut_fait.max(p.done);
            if p.done >= 1_000 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });
        assert!(
            matches!(result, Err(Error::Interrupted)),
            "annuler doit rendre Error::Interrupted, pas un Store"
        );
        assert!(
            plus_haut_fait >= 1_000,
            "le décor doit exercer une annulation EN COURS de passe \
             (relevé le plus haut : {plus_haut_fait})"
        );

        // Tout est défait : la base est revenue à l'état d'AVANT
        // l'ouverture annulée.
        {
            let conn = Connection::open(&path).unwrap();
            let version: i64 = conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(version, 1, "user_version inchangé : la passe se rejouera");
            let forme_neuve: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('threads')
                     WHERE name = 'inbox_size'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                forme_neuve, 0,
                "la table v1 est intacte : le DROP aussi est rembobiné"
            );
            let enveloppes: i64 = conn
                .query_row("SELECT COUNT(*) FROM envelopes", [], |row| row.get(0))
                .unwrap();
            assert_eq!(enveloppes, i64::from(MESSAGES), "aucun message perdu");
        }

        // Le prochain lancement rejoue la passe ENTIÈRE : liste complète.
        {
            let store = Store::open(&path).unwrap();
            let sans_fil: i64 = store
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM envelopes WHERE thread_id IS NULL",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(sans_fil, 0, "tous les messages hérités sont adoptés");
            let version: i64 = store
                .conn()
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(version, crate::thread::THREADING_VERSION);
        }
        let _ = std::fs::remove_file(&path);
    }

    /// L'avancement est OBSERVABLE (enseignement §9) : le total s'annonce
    /// d'emblée et ne bouge plus, l'avancement ne recule jamais, et
    /// « fini » ne se dit qu'à la fin — jamais avant.
    #[test]
    fn l_adoption_annonce_son_avancement_du_depart_a_la_fin() {
        let path = std::env::temp_dir().join(format!(
            "wind-test-avancement-adoption-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("moi@exemple.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(
                    inbox,
                    &[
                        envelope(1, "Devis", 100, true),
                        reply(2, "Re: Devis", 200, true, 1),
                    ],
                )
                .unwrap();
        }
        rembobine_au_schema_v1(&path);

        let mut releves: Vec<AdoptionProgress> = Vec::new();
        let store = Store::open_with_progress(&path, |p| {
            releves.push(p);
            ControlFlow::Continue(())
        })
        .unwrap();

        assert!(!releves.is_empty(), "une adoption muette n'est pas visible");
        assert_eq!(releves[0].done, 0, "le départ se dit tout de suite");
        assert!(releves[0].total > 0, "le total est annoncé d'emblée");
        for paire in releves.windows(2) {
            assert!(paire[1].done >= paire[0].done, "l'avancement ne recule pas");
            assert_eq!(
                paire[1].total, paire[0].total,
                "le total ne bouge pas en route — une barre qui recule \
                 est pire qu'une barre imprécise"
            );
        }
        let dernier = releves.last().unwrap();
        assert_eq!(
            dernier.done, dernier.total,
            "le dernier relevé dit « fini »"
        );
        assert!(
            releves[..releves.len() - 1]
                .iter()
                .all(|p| p.done < p.total),
            "et il est le SEUL : jamais « 100 % » avant la fin"
        );

        let lignes = unified(&store);
        assert_eq!(lignes.len(), 1, "le fil est refait");
        assert_eq!(lignes[0].thread_size, 2, "avec son compteur");
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// La sonde répond sans rien déclencher : le desktop l'appelle AVANT
    /// la première vraie ouverture pour décider d'afficher l'écran de
    /// migration — si elle migrait elle-même, l'écran arriverait après
    /// la bataille.
    #[test]
    fn la_sonde_dit_quand_une_adoption_attend_sans_la_declencher() {
        let path = std::env::temp_dir().join(format!(
            "wind-test-sonde-adoption-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        // Fichier absent : première installation, rien d'hérité — et la
        // sonde ne doit PAS créer le fichier.
        assert_eq!(Store::pending_adoption(&path).unwrap(), None);
        assert!(!path.exists(), "une sonde ne laisse pas de trace");

        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("moi@exemple.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(
                    inbox,
                    &[
                        envelope(1, "Devis", 100, true),
                        reply(2, "Re: Devis", 200, true, 1),
                    ],
                )
                .unwrap();
            // Un message HORS portée (ADR 0010 §3) : la passe ne
            // l'adoptera jamais, la sonde ne doit pas l'annoncer.
            let spam = store.create_mailbox(account, "Spam", 1).unwrap();
            store
                .upsert_envelopes(spam, &[envelope(1, "Gagné !", 300, true)])
                .unwrap();
        }
        // Base à jour : rien à annoncer.
        assert_eq!(Store::pending_adoption(&path).unwrap(), None);

        rembobine_au_schema_v1(&path);
        assert_eq!(
            Store::pending_adoption(&path).unwrap(),
            Some(2),
            "une base héritée annonce ses messages à adopter — la PORTÉE, \
             pas la base entière : un chiffre doit désigner ce qu'il dit"
        );
        // Et RIEN n'a été déclenché : la version n'a pas bougé.
        {
            let conn = Connection::open(&path).unwrap();
            let version: i64 = conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(version, 1, "la sonde n'a pas migré à notre place");
        }
        let _ = std::fs::remove_file(&path);
    }

    /// La langue se restaure AVANT le premier rendu, donc AVANT l'écran
    /// de migration (constat terrain 2026-08-15) : sa lecture doit être
    /// une sonde en lecture seule — avec l'ouverture pleine, l'adoption
    /// d'une base héritée se payait en silence au chargement de la
    /// langue, sans modale, sans avancement, sans annulation — tout ce
    /// que l'ADR 0012 interdit. Le décor REMBOBINE une vraie base de
    /// fichier (invariant §6.7) : le seul où la faute existe.
    #[test]
    fn la_langue_se_lit_sans_adopter_la_base() {
        let path =
            std::env::temp_dir().join(format!("wind-test-langue-sonde-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);

        // Fichier absent : première installation — et la sonde ne doit
        // PAS créer le fichier.
        assert_eq!(Store::text_pref_readonly(&path, "lang").unwrap(), None);
        assert!(!path.exists(), "une sonde ne laisse pas de trace");

        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("moi@exemple.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(
                    inbox,
                    &[
                        envelope(1, "Devis", 100, true),
                        reply(2, "Re: Devis", 200, true, 1),
                    ],
                )
                .unwrap();
            store.set_text_pref("lang", "en").unwrap();
        }
        rembobine_au_schema_v1(&path);

        // La préférence se lit…
        assert_eq!(
            Store::text_pref_readonly(&path, "lang").unwrap(),
            Some("en".to_string())
        );
        // …et RIEN n'a été déclenché : la version n'a pas bougé, la
        // modale trouvera l'adoption toujours en attente.
        {
            let conn = Connection::open(&path).unwrap();
            let version: i64 = conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(version, 1, "lire la langue n'a pas migré à notre place");
        }
        assert_eq!(
            Store::pending_adoption(&path).unwrap(),
            Some(2),
            "l'écran de migration garde sa raison d'être"
        );

        // Une base héritée d'AVANT le WAL vit en mode rollback (delete) —
        // c'est la forme réelle du terrain, pas celle que Store::open
        // laisse derrière lui : la sonde doit y répondre aussi.
        Connection::open(&path)
            .unwrap()
            .query_row("PRAGMA journal_mode = delete", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap();
        assert_eq!(
            Store::text_pref_readonly(&path, "lang").unwrap(),
            Some("en".to_string()),
            "la sonde répond aussi sur une base en mode rollback"
        );

        // Une base d'avant les préférences (pas de table `prefs`) : la
        // sonde répond « pas de préférence », elle n'échoue pas.
        Connection::open(&path)
            .unwrap()
            .execute_batch("DROP TABLE prefs")
            .unwrap();
        assert_eq!(Store::text_pref_readonly(&path, "lang").unwrap(), None);
        let _ = std::fs::remove_file(&path);
    }

    /// Sur une base à jour il n'y a RIEN à adopter — et donc rien à dire.
    /// Un bandeau de migration à chaque lancement serait un faux signal,
    /// et chaque commande du desktop ouvre sa propre connexion.
    #[test]
    fn une_base_a_jour_s_ouvre_sans_annoncer_de_migration() {
        let path = std::env::temp_dir().join(format!(
            "wind-test-adoption-muette-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("moi@exemple.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(
                    inbox,
                    &[
                        envelope(1, "Devis", 100, true),
                        reply(2, "Re: Devis", 200, true, 1),
                    ],
                )
                .unwrap();
        }

        let mut appels = 0;
        let store = Store::open_with_progress(&path, |_| {
            appels += 1;
            ControlFlow::Continue(())
        })
        .unwrap();
        assert_eq!(appels, 0, "rien à adopter, rien à raconter");
        assert_eq!(unified(&store).len(), 1, "et la liste est là");
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// LE point du chantier de l'[ADR 0009] : un message reçu et la
    /// réponse qu'on lui a faite appartiennent au même échange, donc au
    /// même fil — bien qu'ils vivent dans **deux boîtes différentes**.
    ///
    /// Avant, les fils étaient cloisonnés par boîte : cette réponse aurait
    /// formé son propre fil dans son propre espace d'identifiants, et
    /// synchroniser « Envoyés » aurait coûté sans rien rapporter.
    ///
    /// Le décor donne le même UID (1) aux deux messages, à dessein :
    /// l'identité d'un message est `(compte, boîte, UID)`, et un
    /// regroupement qui confondrait deux UID égaux se verrait ici.
    #[test]
    fn une_reponse_dans_envoyes_rejoint_le_fil_du_message_recu() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let envoyes = store.create_mailbox(account, "Sent", 1).unwrap();
        // Le décor doit DÉCLARER la portée qu'il exerce : depuis l'ADR 0010
        // une boîte ne regroupe que si on l'a dite dedans, et le nom du
        // dossier des envois varie d'un serveur à l'autre.
        store.set_thread_scope(account, Some("Sent")).unwrap();

        // Alice écrit.
        let mut recu = envelope(1, "Devis", 100, true);
        recu.message_id = Some("<alice-1@exemple.fr>".to_string());
        store.upsert_envelopes(inbox, &[recu]).unwrap();

        // Je réponds : le message part dans « Envoyés » et cite le premier.
        let mut reponse = envelope(1, "Re: Devis", 200, true);
        reponse.message_id = Some("<moi-1@exemple.fr>".to_string());
        reponse.in_reply_to = Some("<alice-1@exemple.fr>".to_string());
        store.upsert_envelopes(envoyes, &[reponse]).unwrap();

        let lignes = unified(&store);
        assert_eq!(lignes.len(), 1, "un seul fil, pas deux");
        assert_eq!(
            lignes[0].thread_size, 2,
            "le compteur couvre tout l'échange, envoyés compris"
        );
        assert_eq!(
            lignes[0].envelope.subject.as_deref(),
            Some("Re: Devis"),
            "le fil est représenté par son message le plus récent, \
             même quand c'est notre propre réponse"
        );
    }

    /// Deux messages du MÊME compte peuvent porter le MÊME UID dès qu'ils
    /// vivent dans deux boîtes — c'est la règle et non l'exception, les
    /// UID étant attribués par boîte et repartant de 1.
    ///
    /// Chaque ligne doit donc dire **où elle habite**. Sans cela, ouvrir
    /// notre réponse depuis le bandeau de conversation afficherait le
    /// message reçu à sa place, et le marquerait lu — l'invariant §6.2 de
    /// la passation, corrigé ici pour deux boîtes.
    #[test]
    fn chaque_ligne_dit_dans_quelle_boite_elle_habite() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let envoyes = store.create_mailbox(account, "Sent", 1).unwrap();
        // Le décor doit DÉCLARER la portée qu'il exerce : depuis l'ADR 0010
        // une boîte ne regroupe que si on l'a dite dedans, et le nom du
        // dossier des envois varie d'un serveur à l'autre.
        store.set_thread_scope(account, Some("Sent")).unwrap();

        let mut recu = envelope(1, "Devis", 100, true);
        recu.message_id = Some("<alice-9@exemple.fr>".to_string());
        store.upsert_envelopes(inbox, &[recu]).unwrap();
        let mut reponse = envelope(1, "Re: Devis", 200, true);
        reponse.message_id = Some("<moi-9@exemple.fr>".to_string());
        reponse.in_reply_to = Some("<alice-9@exemple.fr>".to_string());
        store.upsert_envelopes(envoyes, &[reponse]).unwrap();

        let fil = unified(&store)[0].thread_id.unwrap();
        let messages = store.thread_messages(fil).unwrap();

        assert_eq!(messages.len(), 2);
        assert!(
            messages.iter().all(|ligne| ligne.envelope.uid == 1),
            "le décor a bien deux messages de même UID : c'est tout l'objet"
        );
        let boites: Vec<&str> = messages.iter().map(|l| l.mailbox.as_str()).collect();
        assert!(boites.contains(&"INBOX"), "boîtes vues : {boites:?}");
        assert!(boites.contains(&"Sent"), "boîtes vues : {boites:?}");
    }

    /// L'autre face de la même règle : écrire à quelqu'un qui ne répond
    /// jamais ne crée PAS de conversation dans la boîte de réception.
    /// C'est ce que le compteur `inbox_size` protège, et c'est aussi ce
    /// qui rend l'index partiel possible (ADR 0009 §2 et §4).
    #[test]
    fn un_fil_purement_sortant_n_a_pas_de_ligne() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        store.create_mailbox(account, "INBOX", 1).unwrap();
        let envoyes = store.create_mailbox(account, "Sent", 1).unwrap();
        // Le décor doit DÉCLARER la portée qu'il exerce : depuis l'ADR 0010
        // une boîte ne regroupe que si on l'a dite dedans, et le nom du
        // dossier des envois varie d'un serveur à l'autre.
        store.set_thread_scope(account, Some("Sent")).unwrap();

        let mut sortant = envelope(1, "Ma proposition", 100, true);
        sortant.message_id = Some("<moi-2@exemple.fr>".to_string());
        store.upsert_envelopes(envoyes, &[sortant]).unwrap();

        assert!(
            unified(&store).is_empty(),
            "rien n'a été reçu : la boîte de réception reste vide"
        );
        assert_eq!(store.unified_count().unwrap(), 0);
    }

    /// [ADR 0010] §3 — on STOCKE tout, on ne REGROUPE que dans la portée.
    ///
    /// Depuis l'[ADR 0009] un fil appartient au COMPTE. Dès que la
    /// synchronisation intégrale verse Archive, Corbeille et Spam dans ce
    /// même compte, leurs messages rejoindraient les fils **tout seuls** —
    /// et trois agrégats se corrompraient sans qu'aucun test ne le voie :
    ///
    /// - `size` : « 12 messages » sur un fil qui en montre 3 ;
    /// - `unseen` : un fil éternellement non lu à cause d'un spam ;
    /// - `last_epoch` : **la conversation remonte en tête de liste parce
    ///   qu'un spam s'y est accroché**.
    ///
    /// Le troisième est un défaut de CORRECTION : la liste mentirait sur
    /// l'ordre des échanges, sans recours pour l'utilisateur. Même motif de
    /// refus que le regroupement par sujet (ADR 0008 §2).
    ///
    /// Le compilateur ne protège rien ici — une boîte est une chaîne comme
    /// une autre (passation §6.2). C'est ce test qui tient l'invariant.
    #[test]
    fn un_message_hors_portee_ne_rejoint_pas_le_fil() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let spam = store.create_mailbox(account, "Spam", 1).unwrap();

        let mut recu = envelope(1, "Devis", 100, true);
        recu.message_id = Some("<alice-10@exemple.fr>".to_string());
        store.upsert_envelopes(inbox, &[recu]).unwrap();

        // Le spam cite le message reçu — c'est exactement ce qui le ferait
        // rejoindre le fil. Il est PLUS RÉCENT et NON LU : s'il entrait,
        // les trois agrégats bougeraient d'un coup.
        let mut indesirable = envelope(1, "GAGNEZ 1000 EUROS", 300, false);
        indesirable.message_id = Some("<spam-1@ailleurs.example>".to_string());
        indesirable.in_reply_to = Some("<alice-10@exemple.fr>".to_string());
        store.upsert_envelopes(spam, &[indesirable]).unwrap();

        let lignes = unified(&store);
        assert_eq!(lignes.len(), 1, "un seul fil");
        assert_eq!(
            lignes[0].thread_size, 1,
            "le spam ne compte pas dans l'échange"
        );
        assert_eq!(
            lignes[0].envelope.subject.as_deref(),
            Some("Devis"),
            "le fil reste représenté par le message reçu, pas par le spam \
             qui s'y est accroché"
        );
        assert_eq!(
            lignes[0].thread_unseen, 0,
            "un spam jamais ouvert ne rend pas la conversation non lue"
        );

        // L'autre moitié de l'ADR 0010 : hors portée ne veut pas dire
        // absent. Le message est stocké — donc cherchable.
        assert!(
            store.envelope(account, "Spam", 1).unwrap().is_some(),
            "le spam est bien en base : on stocke tout, on ne regroupe pas tout"
        );
    }

    /// La portée déclarée AVANT que la boîte n'existe doit valoir quand
    /// même — c'est le cas normal, pas le cas limite.
    ///
    /// La boucle de synchronisation de l'[ADR 0010] **crée** le dossier des
    /// envois : au moment où l'on déclare la portée, il n'y a aucune ligne
    /// à mettre à jour. Si la portée ne vivait que sur `mailboxes`, cette
    /// déclaration serait perdue, la boîte naîtrait hors portée, et ses
    /// messages resteraient sans fil jusqu'au prochain démarrage — la liste
    /// afficherait un échange amputé de nos réponses, sans rien signaler.
    ///
    /// D'où la mémoire portée par le COMPTE, que ce test garde.
    #[test]
    fn une_portee_declaree_avant_la_creation_de_la_boite_vaut_quand_meme() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();

        // On déclare « Envoyés » AVANT de l'avoir créé — l'ordre réel.
        store.set_thread_scope(account, Some("Sent")).unwrap();
        let envoyes = store.create_mailbox(account, "Sent", 1).unwrap();

        let mut recu = envelope(1, "Devis", 100, true);
        recu.message_id = Some("<alice-11@exemple.fr>".to_string());
        store.upsert_envelopes(inbox, &[recu]).unwrap();
        let mut reponse = envelope(1, "Re: Devis", 200, true);
        reponse.message_id = Some("<moi-11@exemple.fr>".to_string());
        reponse.in_reply_to = Some("<alice-11@exemple.fr>".to_string());
        store.upsert_envelopes(envoyes, &[reponse]).unwrap();

        let lignes = unified(&store);
        assert_eq!(lignes.len(), 1, "un seul fil");
        assert_eq!(
            lignes[0].thread_size, 2,
            "la réponse a rejoint le fil dès son écriture, sans attendre \
             un redémarrage"
        );
    }

    /// La promesse de l'[ADR 0008] §4 — « le coût d'une page ne dépend
    /// plus de la taille de la boîte » — repose ENTIÈREMENT sur un index
    /// qui porte le tri. Si SQLite matérialise l'ordre dans un B-arbre
    /// temporaire, elle est rompue : silencieusement, et seulement à
    /// l'échelle, là où plus aucun test fonctionnel ne regarde.
    ///
    /// C'est arrivé. Le gate 3 a mesuré **987 ms** pour une page à
    /// 160 000 conversations, contre 0,66 ms une fois l'index posé.
    /// L'index d'origine était préfixé par `mailbox_id` : il servait une
    /// boîte, mais pas la **boîte unifiée**, qui les couvre toutes et qui
    /// est la vue par défaut du produit. Deux comptes suffisent à le
    /// reproduire — d'où ce décor.
    ///
    /// On interroge le plan plutôt qu'un chronomètre : une durée dépend
    /// de la machine, un plan d'exécution non.
    #[test]
    fn la_boite_unifiee_ne_materialise_pas_son_tri() {
        let mut store = Store::open_in_memory().unwrap();
        for (email, uids) in [("un@exemple.fr", 1..60u32), ("deux@exemple.fr", 60..120)] {
            let account = store.adopt_or_create_account(email, "gmail").unwrap();
            let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            let envelopes: Vec<Envelope> = uids
                .map(|uid| envelope(uid, "Sujet", 1_600_000_000 + i64::from(uid), true))
                .collect();
            store.upsert_envelopes(mailbox, &envelopes).unwrap();
        }

        let mut stmt = store
            .0
            .prepare(&format!(
                "EXPLAIN QUERY PLAN {}",
                unified_page_sql(false, false, false)
            ))
            .unwrap();
        let plan: Vec<String> = stmt
            .query_map(params![200i64, 0i64], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        // « FOR LAST TERM OF ORDER BY » est acceptable : ce tri-là ne
        // départage que les ex æquo de date ET d'UID. C'est le tri
        // COMPLET qui coûte, et lui seul est interdit ici.
        assert!(
            !plan
                .iter()
                .any(|etape| etape.contains("TEMP B-TREE FOR ORDER BY")),
            "la page de la boîte unifiée matérialise son tri — le coût \
             redevient proportionnel à la taille de la boîte.\nPlan :\n{}",
            plan.join("\n")
        );
        // R4 : la sous-requête des épingles (PINNED_THREADS) doit partir
        // de `pins` (minuscule) et SONDER `envelopes` par sa clé — sans
        // le CROSS JOIN directif, SQLite (sans ANALYZE, le cas de
        // production) scanne `envelopes` ENTIÈRE à chaque page : ~24 ms
        // mesurés à 200 k, sur le chemin le plus chaud (revue
        // 2026-08-21).
        assert!(
            !plan.iter().any(|etape| etape.contains("SCAN pe")),
            "la sous-requête des épingles scanne `envelopes` — l'ordre \
             de jointure a perdu sa directive.\nPlan :\n{}",
            plan.join("\n")
        );
        assert!(
            plan.iter().any(|etape| etape.contains("SCAN p")),
            "la sous-requête des épingles ne part plus de `pins`.\nPlan :\n{}",
            plan.join("\n")
        );
    }

    /// PLAN-AUDIT-V2 E4 : les groupes du Nettoyage (un expéditeur × son
    /// courrier) coûtaient 380 ms sur 200 k enveloppes et 5 000 expéditeurs
    /// — un parcours par l'index de DATE puis un B-tree temporaire de
    /// regroupement. L'index des expéditeurs, étendu à la boîte, COUVRE
    /// l'agrégat : le plan doit passer par lui, jamais par l'index de date
    /// (un test de plan d'exécution, leçon STANDARD §9).
    #[test]
    fn les_groupes_du_nettoyage_se_lisent_par_l_index_des_expediteurs() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let sql = Store::nettoyage_groupes_sql(&[inbox]);
        let plan: Vec<String> = store
            .conn()
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap()
            .query_map(params![0i64], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            plan.iter()
                .any(|ligne| ligne.contains("idx_envelopes_sender")),
            "l'agrégat ne passe pas par l'index des expéditeurs : {plan:?}"
        );
        assert!(
            !plan
                .iter()
                .any(|ligne| ligne.contains("idx_envelopes_date")),
            "l'agrégat parcourt l'index de date : {plan:?}"
        );
        // Le courrier d'un groupe, même exigence (116 ms sur 200 k sinon).
        let sql = Store::nettoyage_messages_sql(&[inbox]);
        let plan: Vec<String> = store
            .conn()
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap()
            .query_map(params![0i64, "x@y.fr"], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            plan.iter()
                .any(|ligne| ligne.contains("idx_envelopes_sender (sender_norm=?)")),
            "le courrier d'un groupe ne se cherche pas par l'expéditeur : {plan:?}"
        );
    }

    /// Revue de la vague 2 : `PRAGMA foreign_keys = ON` vit dans `SCHEMA`
    /// et vaut PAR CONNEXION — la porte rapide ne rejoue pas le schéma.
    /// Ce test est resté vert AVANT la ligne posée dans `init_with` :
    /// rusqlite `bundled` active les clés par défaut à la compilation.
    /// Il garde la ceinture : sur base FICHIER (une base mémoire n'entre
    /// jamais au registre), la seconde ouverture efface encore les boîtes
    /// d'un compte supprimé, quel que soit le drapeau de compilation.
    #[test]
    fn la_porte_rapide_garde_les_cles_etrangeres() {
        let path =
            std::env::temp_dir().join(format!("wind-test-porte-fk-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        drop(Store::open(&path).unwrap());

        let mut store = Store::open(&path).unwrap();
        let actives: i64 = store
            .conn()
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            actives, 1,
            "clés étrangères éteintes sur la seconde connexion"
        );
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        store.create_mailbox(account, "INBOX", 1).unwrap();
        store.delete_account(account).unwrap();
        let boites: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM mailboxes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(boites, 0, "la cascade du compte supprimé n'a pas joué");
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// Une base du parc porte l'index des expéditeurs à DEUX colonnes ;
    /// à la réouverture il gagne la boîte (même patron que l'index de
    /// date ci-dessous).
    #[test]
    fn l_index_des_expediteurs_herite_gagne_la_boite_a_la_reouverture() {
        let path =
            std::env::temp_dir().join(format!("wind-test-idx-sender-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let lire_sql = |conn: &Connection| -> String {
            conn.query_row(
                "SELECT sql FROM sqlite_master WHERE name = 'idx_envelopes_sender'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        };
        {
            let store = Store::open(&path).unwrap();
            store
                .conn()
                .execute_batch(
                    "DROP INDEX idx_envelopes_sender;
                     CREATE INDEX idx_envelopes_sender
                         ON envelopes(sender_norm, date_epoch);",
                )
                .unwrap();
            assert!(!lire_sql(store.conn()).contains("mailbox_id"));
        }
        Store::oublier_initialisation(&path);
        let store = Store::open(&path).unwrap();
        assert!(
            lire_sql(store.conn()).contains("mailbox_id"),
            "l'index hérité n'a pas été reconstruit"
        );
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// PLAN-DEMARRAGE, E1-bis — l'index de date des enveloppes gagne
    /// `uid`, et **`CREATE INDEX IF NOT EXISTS` ne suffit PAS** : sur une
    /// base existante l'index porte déjà ce nom, la création est un no-op
    /// muet, et le défaut survivrait à la mise à jour. La migration lit
    /// donc sa DÉFINITION, pas son nom.
    ///
    /// Sans ce test, la branche de reconstruction n'est **jamais jouée** :
    /// toute base née d'un `Store::open` porte l'index à jour dès le
    /// `SCHEMA`, et `migrate()` n'a plus rien à faire. Il faut donc
    /// rétrograder l'index à la main pour exercer le chemin du parc.
    #[test]
    fn l_index_de_date_herite_gagne_uid_a_la_reouverture() {
        let path =
            std::env::temp_dir().join(format!("wind-test-idx-date-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let lire_sql = |store: &Store| -> String {
            store
                .conn()
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE name = 'idx_envelopes_date'",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
        };

        {
            let store = Store::open(&path).unwrap();
            // Rétrograde l'index à sa forme d'avant le chantier — l'état
            // exact de toute base du parc au moment de la mise à jour.
            store
                .conn()
                .execute_batch(
                    "DROP INDEX idx_envelopes_date;
                     CREATE INDEX idx_envelopes_date
                         ON envelopes(mailbox_id, date_epoch DESC);",
                )
                .unwrap();
            assert!(
                !lire_sql(&store).contains("uid"),
                "le décor doit partir de l'index COURT, sinon le test ne prouve rien"
            );
        }

        Store::oublier_initialisation(&path);
        let store = Store::open(&path).unwrap();
        let sql = lire_sql(&store);
        assert!(
            sql.contains("uid"),
            "l'index hérité n'a pas été reconstruit à l'ouverture — la sonde de définition ne fait rien, et le parc garderait le défaut.
SQL : {sql}"
        );
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// PLAN-DEMARRAGE, défaut 01 — la sonde « combien de corps
    /// manquent ? » tenait le VERROU GLOBAL des commandes **8 870 ms à
    /// chaque démarrage** (20 839 ms en SQL pur à froid), mesuré le
    /// 2026-08-26 sur la base du terrain : 251 466 corps, 11,4 Go.
    ///
    /// La cause n'était pas la jointure. C'était la lecture d'une
    /// COLONNE de `bodies` : absente de l'auto-index de la clé primaire,
    /// elle forçait SQLite à rappeler la LIGNE — 56 ko en moyenne — pour
    /// lire un bit. 251 k lectures aléatoires dans 11,4 Go.
    ///
    /// Le plan le dit d'un seul mot : `COVERING`. Tant que la
    /// sous-requête ne lit AUCUNE colonne de `bodies`, l'existence de la
    /// ligne se tranche dans l'index seul. Qu'on y rajoute une colonne un
    /// jour, et le mot disparaît — c'est cela, et rien d'autre, que ce
    /// test garde.
    ///
    /// On interroge le plan plutôt qu'un chronomètre : une durée dépend
    /// de la machine, un plan d'exécution non.
    #[test]
    fn les_sondes_de_corps_manquants_ne_rappellent_jamais_la_ligne_grasse() {
        let (mut store, inbox) = store_with_mailbox();
        let envelopes: Vec<Envelope> = (1..=40u32)
            .map(|uid| envelope(uid, "Sujet", 1_600_000_000 + i64::from(uid), true))
            .collect();
        store.upsert_envelopes(inbox, &envelopes).unwrap();
        // Des corps pour les trois quarts : la sous-requête doit avoir
        // des lignes à trouver ET des lignes à ne pas trouver.
        for uid in 1..=30u32 {
            store.save_body(inbox, uid, "<p>corps</p>", &[]).unwrap();
        }

        let mut compte = store
            .0
            .prepare(&format!(
                "EXPLAIN QUERY PLAN {}",
                bodies_pending_count_sql()
            ))
            .unwrap();
        let plan_compte: Vec<String> = compte
            .query_map(params![1i64, "INBOX", 0i64], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        let mut liste = store
            .0
            .prepare(&format!("EXPLAIN QUERY PLAN {}", bodies_to_backfill_sql()))
            .unwrap();
        let plan_liste: Vec<String> = liste
            .query_map(params![1i64, "INBOX", 0i64, 10i64], |row| {
                row.get::<_, String>(3)
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        for (quoi, plan) in [
            ("le compte des manquants", &plan_compte),
            ("la liste de travail du rattrapage", &plan_liste),
        ] {
            for (alias, table) in [(" e ", "envelopes"), (" b ", "bodies")] {
                let etape = plan
                    .iter()
                    .find(|etape| etape.contains(alias))
                    .unwrap_or_else(|| {
                        panic!(
                            "{quoi} : aucune etape ne touche `{table}`.\nPlan :\n{}",
                            plan.join("\n")
                        )
                    });
                assert!(
                    etape.contains("COVERING"),
                    "{quoi} : l'acces a `{table}` n'est PAS couvert par son \
index — SQLite rappelle la ligne pour y lire une colonne que l'index ne \
porte pas. C'est le defaut de PLAN-DEMARRAGE, des DEUX cotes : 8 870 ms \
de verrou tenu cote `bodies`, 521,9 ms de sonde cote `envelopes`.\n\
Etape : {etape}\nPlan :\n{}",
                    plan.join("\n")
                );
            }
        }
    }

    /// R4 (PLAN-RETOURS-7) : une conversation épinglée se sert À PART
    /// (`pinned_unified_scoped`) et QUITTE le flot paginé comme son
    /// comptage (décision D5 : la liste ne montre jamais deux fois le
    /// même message). Désépingler la rend au flot. L'épingle est bornée
    /// au compte et suit l'onglet « Non lus » comme la page.
    #[test]
    fn une_epingle_sert_sa_conversation_a_part_et_hors_du_flot() {
        let (mut store, inbox) = store_with_mailbox();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "ancien", 100, true),
                    envelope(2, "milieu", 200, true),
                    envelope(3, "récent", 300, true),
                ],
            )
            .unwrap();
        assert!(
            store
                .pinned_unified_scoped(None, false, false)
                .unwrap()
                .is_empty()
        );

        assert!(store.toggle_pin(inbox, 1, 1_000).unwrap(), "épinglé");
        let epingles = store.pinned_unified_scoped(None, false, false).unwrap();
        assert_eq!(epingles.len(), 1);
        assert_eq!(epingles[0].envelope.uid, 1);
        let flot = store.unified_recent_scoped(None, false, 0, 10).unwrap();
        assert!(
            flot.iter().all(|row| row.envelope.uid != 1),
            "la conversation épinglée quitte le flot"
        );
        assert_eq!(flot.len(), 2);
        assert_eq!(store.unified_count_scoped(None, false).unwrap(), 2);
        // Bornes de la portée : un AUTRE compte n'a pas cette épingle,
        // et l'onglet « Non lus » ne la montre pas (tout est lu ici).
        assert!(
            store
                .pinned_unified_scoped(Some(999), false, false)
                .unwrap()
                .is_empty()
        );
        assert!(
            store
                .pinned_unified_scoped(None, true, false)
                .unwrap()
                .is_empty()
        );

        assert!(!store.toggle_pin(inbox, 1, 1_001).unwrap(), "désépinglé");
        assert!(
            store
                .pinned_unified_scoped(None, false, false)
                .unwrap()
                .is_empty()
        );
        assert_eq!(store.unified_count_scoped(None, false).unwrap(), 3);
    }

    /// R1 (PLAN-RETOURS-11, D1-D2) : le choix « Afficher les images »
    /// est une exception EXPLICITE écrite en base, par MESSAGE (clé
    /// d'enveloppe, patron de `pins`) — rouvrir le message ne
    /// redemande pas, et le message voisin n'hérite de rien.
    #[test]
    fn le_choix_d_images_par_message_persiste_et_ne_deteint_pas() {
        let (mut store, inbox) = store_with_mailbox();
        store
            .upsert_envelopes(
                inbox,
                &[envelope(1, "a", 100, true), envelope(2, "b", 200, true)],
            )
            .unwrap();
        assert!(
            !store.images_allowed(inbox, 1).unwrap(),
            "bloqué par défaut"
        );
        store.allow_images_message(inbox, 1, 1_000).unwrap();
        assert!(store.images_allowed(inbox, 1).unwrap());
        assert!(
            !store.images_allowed(inbox, 2).unwrap(),
            "le choix est PAR message"
        );
    }

    /// R1 (D3-D4) : la règle d'expéditeur se pose DEPUIS un message —
    /// l'adresse est lue de l'ENVELOPPE (jamais de l'UI), normalisée
    /// en minuscules — couvre tous ses messages, se liste et se
    /// révoque.
    #[test]
    fn la_regle_d_expediteur_couvre_ses_messages_et_se_revoque() {
        let (mut store, inbox) = store_with_mailbox();
        let mut expediteur = envelope(1, "a", 100, true);
        expediteur.sender_address = Some("No-Reply@Registrar.FR".to_string());
        let mut pareil = envelope(2, "b", 200, true);
        pareil.sender_address = Some("no-reply@registrar.fr".to_string());
        let tiers = envelope(3, "c", 300, true); // alice@example.com
        store
            .upsert_envelopes(inbox, &[expediteur, pareil, tiers])
            .unwrap();

        let posee = store.allow_images_sender_of(inbox, 1, 1_000).unwrap();
        assert_eq!(
            posee.as_deref(),
            Some("no-reply@registrar.fr"),
            "l'adresse posée est normalisée"
        );
        assert!(store.images_allowed(inbox, 1).unwrap());
        assert!(
            store.images_allowed(inbox, 2).unwrap(),
            "tous les messages de l'expéditeur, quelle que soit la casse"
        );
        assert!(!store.images_allowed(inbox, 3).unwrap(), "jamais un tiers");
        assert_eq!(
            store.images_senders().unwrap(),
            vec!["no-reply@registrar.fr".to_string()]
        );

        store.revoke_images_sender("no-reply@registrar.fr").unwrap();
        assert!(store.images_senders().unwrap().is_empty());
        assert!(
            !store.images_allowed(inbox, 1).unwrap(),
            "révoquée — la garde revient"
        );
    }

    /// R1 (revue 2026-08-28) : l'accord d'images PAR MESSAGE meurt au
    /// changement d'UIDVALIDITY — un UID recyclé ne doit JAMAIS
    /// hériter d'un consentement (le pixel espion d'un inconnu
    /// partirait sans bandeau ni geste). Même contrat que
    /// `invitations`/`attachments` dans `reset_mailbox`.
    #[test]
    fn le_reset_uidvalidity_purge_la_memoire_d_images_par_message() {
        let (mut store, inbox) = store_with_mailbox();
        store
            .upsert_envelopes(inbox, &[envelope(1, "a", 100, true)])
            .unwrap();
        store.allow_images_message(inbox, 1, 1_000).unwrap();
        assert!(store.images_allowed(inbox, 1).unwrap());

        store.reset_mailbox(inbox, 2).unwrap();
        store
            .upsert_envelopes(inbox, &[envelope(1, "tout autre", 200, true)])
            .unwrap();
        assert!(
            !store.images_allowed(inbox, 1).unwrap(),
            "un UID recyclé n'hérite d'aucun accord"
        );
    }

    /// R1 : une enveloppe SANS adresse d'expéditeur ne pose RIEN —
    /// jamais une règle vide qui accorderait on ne sait quoi.
    #[test]
    fn pas_d_adresse_d_expediteur_pas_de_regle() {
        let (mut store, inbox) = store_with_mailbox();
        let mut sans = envelope(1, "a", 100, true);
        sans.sender_address = None;
        store.upsert_envelopes(inbox, &[sans]).unwrap();
        assert_eq!(store.allow_images_sender_of(inbox, 1, 1_000).unwrap(), None);
        assert!(store.images_senders().unwrap().is_empty());
        assert!(!store.images_allowed(inbox, 1).unwrap());
    }

    /// R4 : l'épingle suit le FIL — posée sur un message, elle tient
    /// quand une réponse déplace la tête de la conversation ;
    /// `pin_state` répond par le fil, et désépingler depuis la tête
    /// NOUVELLE libère le fil entier.
    #[test]
    fn une_epingle_suit_le_fil_et_sa_tete_nouvelle() {
        let (mut store, inbox) = store_with_mailbox();
        store
            .upsert_envelopes(inbox, &[envelope(1, "sujet", 100, true)])
            .unwrap();
        assert!(store.toggle_pin(inbox, 1, 1_000).unwrap());

        let mut reponse = envelope(2, "Re: sujet", 400, true);
        reponse.in_reply_to = Some("<m1@example.com>".to_string());
        store.upsert_envelopes(inbox, &[reponse]).unwrap();

        let epingles = store.pinned_unified_scoped(None, false, false).unwrap();
        assert_eq!(epingles.len(), 1, "un fil épinglé = UNE ligne");
        assert_eq!(epingles[0].envelope.uid, 2, "la ligne est la tête du fil");
        assert_eq!(epingles[0].thread_size, 2);
        assert!(
            store.pin_state(inbox, 2).unwrap(),
            "l'état se lit par le fil"
        );

        assert!(
            !store.toggle_pin(inbox, 2, 1_001).unwrap(),
            "désépinglé depuis la tête nouvelle"
        );
        assert!(
            store
                .pinned_unified_scoped(None, false, false)
                .unwrap()
                .is_empty()
        );
        assert!(!store.pin_state(inbox, 1).unwrap());
        assert_eq!(store.unified_count_scoped(None, false).unwrap(), 1);
    }

    /// PLAN-MODE-ORGANISE E1 (D1 : routage LOCAL seul, patron
    /// `images_expediteurs`). La pose normalise l'adresse par LA même
    /// autorité que la garde d'images, écrase la décision précédente
    /// (un seul verdict par expéditeur), et « Réintégrer » = DELETE —
    /// quelle que soit la casse fournie par l'appelant.
    #[test]
    fn routage_pose_normalise_ecrase_et_se_retire() {
        let store = Store::open_in_memory().unwrap();
        store
            .router_expediteur("  Ada@Exemple.FR ", "kiosque", None, 1_700_000_000)
            .unwrap();
        let r = store.routage_de("ada@exemple.fr").unwrap().unwrap();
        assert_eq!(
            (r.destination.as_str(), r.regle.as_deref()),
            ("kiosque", None)
        );
        store
            .router_expediteur("ada@exemple.fr", "ecarte", Some("corbeille"), 1_700_000_100)
            .unwrap();
        let r = store.routage_de("ADA@EXEMPLE.FR").unwrap().unwrap();
        assert_eq!(
            (r.destination.as_str(), r.regle.as_deref()),
            ("ecarte", Some("corbeille"))
        );
        store.retirer_routage(" ada@EXEMPLE.fr ").unwrap();
        assert!(store.routage_de("ada@exemple.fr").unwrap().is_none());
    }

    /// Le vocabulaire est FERMÉ : une destination ou une règle hors
    /// table est refusée AVANT toute écriture (décision pure, jamais un
    /// CHECK SQLite en première ligne) ; une règle n'a de sens que sur
    /// un expéditeur écarté ; une adresse vide n'écrit jamais une règle
    /// fantôme.
    #[test]
    fn routage_refuse_hors_vocabulaire() {
        let store = Store::open_in_memory().unwrap();
        assert!(
            store
                .router_expediteur("a@b.fr", "poubelle", None, 1)
                .is_err()
        );
        assert!(
            store
                .router_expediteur("a@b.fr", "ecarte", Some("suppression-definitive"), 1)
                .is_err()
        );
        assert!(
            store
                .router_expediteur("a@b.fr", "kiosque", Some("corbeille"), 1)
                .is_err(),
            "une règle du Non sur une destination servie n'a pas de sens"
        );
        assert!(store.router_expediteur("   ", "kiosque", None, 1).is_err());
        assert!(store.routages().unwrap().is_empty(), "rien n'a été écrit");
    }

    /// PLAN-MODE-ORGANISE E1 : une page du Kiosque ou du Registre —
    /// le flot unifié de la Réception, borné aux fils dont la TÊTE
    /// vient d'un expéditeur routé vers cette destination. Même
    /// squelette, mêmes exclusions (épingles), même tri que la
    /// Réception ; la sonde est PK → PK (spike S2 : 0,209 ms à 200 k,
    /// jamais un scan).
    #[test]
    fn le_kiosque_ne_sert_que_les_expediteurs_routes() {
        let (mut store, inbox) = store_with_mailbox();
        let mut lettre = envelope(1, "La lettre", 100, true);
        lettre.sender_address = Some("Lettre@infolettre.fr".to_string());
        lettre.message_id = Some("<l1@infolettre.fr>".to_string());
        let ordinaire = envelope(2, "Bonjour", 200, false);
        store.upsert_envelopes(inbox, &[lettre, ordinaire]).unwrap();
        store
            .router_expediteur("lettre@infolettre.fr", "kiosque", None, 300)
            .unwrap();

        let kiosque = store
            .routage_unified_scoped("kiosque", None, false, 0, 10)
            .unwrap();
        assert_eq!(kiosque.len(), 1);
        assert_eq!(kiosque[0].envelope.uid, 1);
        assert_eq!(
            store.routage_count_scoped("kiosque", None, false).unwrap(),
            1
        );
        // Le Registre est vide : la destination filtre vraiment.
        assert!(
            store
                .routage_unified_scoped("registre", None, false, 0, 10)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store.routage_count_scoped("registre", None, false).unwrap(),
            0
        );
        // La Réception, elle, montre TOUJOURS tout (E1 : le retrait du
        // flot est l'affaire de l'étape E2 — rétention du Portier).
        assert_eq!(store.unified_count_scoped(None, false).unwrap(), 2);
    }

    /// La garde de plan du service du Kiosque (leçon `pins`) : la sonde
    /// de routage se joue par CLÉS (envelopes PK, routage PK) — jamais
    /// un parcours d'`envelopes`.
    #[test]
    fn le_kiosque_ne_scanne_jamais_les_enveloppes() {
        let store = Store::open_in_memory().unwrap();
        let plan: Vec<String> = store
            .0
            .prepare(&format!(
                "EXPLAIN QUERY PLAN {}",
                routage_page_sql(false, false)
            ))
            .unwrap()
            .query_map(params![10, 0, "kiosque"], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let scans: Vec<&String> = plan
            .iter()
            .filter(|l| l.starts_with("SCAN") && l.contains("envelopes"))
            .collect();
        assert!(scans.is_empty(), "plan avec scan d'envelopes : {plan:?}");
    }

    /// Revue E1 : la TÊTE d'un fil est le dernier message TOUTES
    /// boîtes confondues — Envoyés compris. Le geste et le filtre ne
    /// doivent jamais s'ancrer dessus : (1) « Déplacer vers… » depuis
    /// un fil où l'utilisateur a répondu en dernier doit router le
    /// CORRESPONDANT, jamais soi ; (2) un fil routé au Kiosque n'en
    /// sort pas parce qu'on y a répondu ; (3) un fil épinglé routé
    /// reste visible dans sa destination (les épingles ne se préposent
    /// qu'en Réception — l'exclure ici le ferait disparaître partout).
    #[test]
    fn le_routage_ignore_sa_propre_reponse_et_garde_les_epingles() {
        let (mut store, inbox) = store_with_mailbox();
        // Les envois entrent dans la portée du regroupement (ADR 0009)
        // — sans quoi la réponse resterait hors fil et le décor ne
        // rejouerait pas la racine (tête = Envoyés).
        store
            .set_thread_scope(test_account(&store), Some("Envoyes"))
            .unwrap();
        let envoyes = store
            .create_mailbox(test_account(&store), "Envoyes", 1)
            .unwrap();
        let mut lettre = envelope(1, "La lettre", 100, true);
        lettre.sender_address = Some("lettre@infolettre.fr".to_string());
        lettre.message_id = Some("<l1@infolettre.fr>".to_string());
        store.upsert_envelopes(inbox, &[lettre]).unwrap();
        // La réponse de l'utilisateur, en Envoyés — elle devient la
        // TÊTE du fil (date la plus récente).
        let mut reponse = envelope(1, "Re: La lettre", 500, true);
        reponse.sender_address = Some("test@exemple.fr".to_string());
        reponse.message_id = Some("<r1@exemple.fr>".to_string());
        reponse.in_reply_to = Some("<l1@infolettre.fr>".to_string());
        store.upsert_envelopes(envoyes, &[reponse]).unwrap();

        // (1) Le geste depuis la tête (la propre réponse) route le
        // correspondant, jamais soi.
        let adresse = store
            .router_expediteur_of(envoyes, 1, "kiosque", None, 600)
            .unwrap();
        assert_eq!(adresse.as_deref(), Some("lettre@infolettre.fr"));
        // (2) Le fil est au Kiosque malgré sa tête « Envoyés ».
        let kiosque = store
            .routage_unified_scoped("kiosque", None, false, 0, 10)
            .unwrap();
        assert_eq!(kiosque.len(), 1);
        assert_eq!(
            store.routage_count_scoped("kiosque", None, false).unwrap(),
            1
        );
        // (3) Épinglé, il reste visible au Kiosque — page ET total.
        assert!(store.toggle_pin(inbox, 1, 700).unwrap());
        assert_eq!(
            store
                .routage_unified_scoped("kiosque", None, false, 0, 10)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store.routage_count_scoped("kiosque", None, false).unwrap(),
            1
        );
    }

    /// « Déplacer vers… » (E1) : l'adresse est résolue de l'ENVELOPPE
    /// côté cœur — l'UI ne parse jamais une adresse (patron
    /// `allow_images_sender_of`). Rend l'adresse routée ; None si
    /// l'enveloppe n'a pas d'adresse (jamais un verdict fantôme).
    #[test]
    fn le_routage_depuis_l_enveloppe_resout_l_adresse_au_coeur() {
        let (mut store, inbox) = store_with_mailbox();
        let mut env = envelope(1, "sujet", 100, true);
        env.sender_address = Some("  ADA@Exemple.FR ".to_string());
        let mut sans_adresse = envelope(2, "anonyme", 200, true);
        sans_adresse.sender_address = None;
        store.upsert_envelopes(inbox, &[env, sans_adresse]).unwrap();

        let adresse = store
            .router_expediteur_of(inbox, 1, "registre", None, 300)
            .unwrap();
        assert_eq!(adresse.as_deref(), Some("ada@exemple.fr"));
        assert_eq!(
            store
                .routage_de("ada@exemple.fr")
                .unwrap()
                .unwrap()
                .destination,
            "registre"
        );
        assert_eq!(
            store
                .router_expediteur_of(inbox, 2, "kiosque", None, 400)
                .unwrap(),
            None
        );
        assert_eq!(
            store.routages().unwrap().len(),
            1,
            "rien d'écrit sans adresse"
        );
    }

    /// Le mode organisé vit en `prefs` SQLite (D2 amendée : le Rust
    /// doit lire l'état — les règles du Non s'éteignent avec lui) et
    /// l'ÉPOQUE DE PREMIÈRE ACTIVATION ne bouge JAMAIS (D3 « arrivées
    /// seules » : c'est elle qui borne la rétention du Portier ; la
    /// réécrire à chaque bascule déverserait ou retiendrait du courrier
    /// en silence). Éteint par défaut, l'état et l'époque s'écrivent
    /// ENSEMBLE à la première activation (jamais l'un sans l'autre).
    #[test]
    fn mode_organise_garde_l_epoque_de_premiere_activation() {
        let mut store = Store::open_in_memory().unwrap();
        assert!(!store.mode_organise().unwrap());
        assert_eq!(store.mode_organise_epoch().unwrap(), None);
        store.set_mode_organise(true, 100).unwrap();
        assert!(store.mode_organise().unwrap());
        assert_eq!(store.mode_organise_epoch().unwrap(), Some(100));
        store.set_mode_organise(false, 200).unwrap();
        assert!(!store.mode_organise().unwrap());
        store.set_mode_organise(true, 300).unwrap();
        assert_eq!(
            store.mode_organise_epoch().unwrap(),
            Some(100),
            "l'époque de PREMIÈRE activation est gravée"
        );
    }

    /// RETOURS-13 R10 — la mémoire « lu » du Kiosque (patron
    /// `pins`/`mis_de_cote` : clé d'enveloppe, locale au poste). Une
    /// carte lue jusqu'en bas se marque ; la marque est idempotente,
    /// meurt avec sa boîte (`reset_mailbox`) et avec son message
    /// (`remove_local`) — un UID recyclé n'hérite d'aucune lecture.
    #[test]
    fn kiosque_lu_se_marque_et_meurt_avec_sa_boite_et_son_message() {
        let (mut store, inbox) = store_with_mailbox();
        store
            .upsert_envelopes(inbox, &[envelope(1, "lettre", 1_000, false)])
            .unwrap();
        store
            .upsert_envelopes(inbox, &[envelope(2, "autre", 1_100, false)])
            .unwrap();
        assert!(!store.kiosque_lu(inbox, 1).unwrap());
        store.marquer_kiosque_lu(inbox, 1, 2_000).unwrap();
        store.marquer_kiosque_lu(inbox, 1, 2_100).unwrap(); // idempotent
        assert!(store.kiosque_lu(inbox, 1).unwrap());
        store.marquer_kiosque_lu(inbox, 2, 2_200).unwrap();
        // Le message part : sa marque aussi.
        store.remove_local(inbox, 1).unwrap();
        assert!(!store.kiosque_lu(inbox, 1).unwrap());
        // La boîte se réinitialise : plus aucune marque.
        store.reset_mailbox(inbox, 2).unwrap();
        assert!(!store.kiosque_lu(inbox, 2).unwrap());
    }

    /// RETOURS-14 R8 (terrain 2026-08-31) — un OUI au Portier vaut
    /// confiance : le verdict pose AUSSI la règle « toujours afficher
    /// les images de cet expéditeur » (table `images_expediteurs`,
    /// révocable aux Réglages > Affichage comme toute règle). Un Non
    /// ne pose rien et ne retire rien — la garde d'images a sa propre
    /// porte de sortie.
    #[test]
    fn un_oui_au_portier_autorise_les_images_de_l_expediteur() {
        let (mut store, inbox) = store_with_mailbox();
        let mut bienvenu = envelope(1, "Bonjour", 100, false);
        bienvenu.sender_address = Some("Ami@exemple.fr".to_string());
        bienvenu.message_id = Some("<a1@exemple.fr>".to_string());
        let mut intrus = envelope(2, "Promo", 200, false);
        intrus.sender_address = Some("promo@exemple.fr".to_string());
        intrus.message_id = Some("<p1@exemple.fr>".to_string());
        store.upsert_envelopes(inbox, &[bienvenu, intrus]).unwrap();
        assert!(!store.images_allowed(inbox, 1).unwrap());

        // Le Oui (toute destination servie) pose la règle — adresse
        // normalisée par LA porte (adresse_images).
        store
            .router_expediteur("ami@exemple.fr", "reception", None, 300)
            .unwrap();
        assert!(store.images_allowed(inbox, 1).unwrap());
        // Le Non n'autorise rien.
        store
            .router_expediteur("promo@exemple.fr", "ecarte", Some("spam"), 300)
            .unwrap();
        assert!(!store.images_allowed(inbox, 2).unwrap());
        // La porte de sortie existante défait la règle posée par le Oui.
        store.revoke_images_sender("ami@exemple.fr").unwrap();
        assert!(!store.images_allowed(inbox, 1).unwrap());
    }

    /// RETOURS-14 R6 (D7) — le Registre se regroupe par EXPÉDITEUR,
    /// les groupes triés par récence du dernier message (patron du
    /// Nettoyage), et la page d'UN groupe rend les fils de ce seul
    /// expéditeur, au tri de la vue.
    #[test]
    fn le_registre_se_groupe_par_expediteur_a_la_recence() {
        let (mut store, inbox) = store_with_mailbox();
        let mut ancien = envelope(1, "Reçu A", 100, true);
        ancien.sender_address = Some("recu@boutique.fr".to_string());
        ancien.message_id = Some("<r1@boutique.fr>".to_string());
        let mut recent = envelope(2, "Avis B", 300, true);
        recent.sender_address = Some("avis@banque.fr".to_string());
        recent.message_id = Some("<b1@banque.fr>".to_string());
        let mut second = envelope(3, "Reçu C", 200, true);
        second.sender_address = Some("recu@boutique.fr".to_string());
        second.message_id = Some("<r2@boutique.fr>".to_string());
        let hors = envelope(4, "Bonjour", 400, false);
        store
            .upsert_envelopes(inbox, &[ancien, recent, second, hors])
            .unwrap();
        store
            .router_expediteur("recu@boutique.fr", "registre", None, 500)
            .unwrap();
        store
            .router_expediteur("avis@banque.fr", "registre", None, 500)
            .unwrap();

        let groupes = store.registre_groupes(None).unwrap();
        assert_eq!(groupes.len(), 2, "un groupe par expéditeur routé");
        // La récence d'abord (D7) : banque (300) avant boutique (200).
        assert_eq!(groupes[0].address, "avis@banque.fr");
        assert_eq!(groupes[0].fils, 1);
        assert_eq!(groupes[1].address, "recu@boutique.fr");
        assert_eq!(groupes[1].fils, 2);
        assert_eq!(groupes[1].dernier_epoch, 200);
        assert_eq!(groupes[1].dernier_objet.as_deref(), Some("Reçu C"));

        // La page d'un groupe : les fils de CE seul expéditeur, les
        // plus récents en tête.
        let page = store
            .registre_groupe_scoped("recu@boutique.fr", None, 0, 10)
            .unwrap();
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].envelope.uid, 3);
        assert_eq!(page[1].envelope.uid, 1);
        // Le filtre de compte borne comme partout.
        let autre = store
            .registre_groupe_scoped("recu@boutique.fr", Some(999), 0, 10)
            .unwrap();
        assert!(autre.is_empty());
    }

    /// RETOURS-14 R7 (D8) — la pastille nav du Kiosque compte les
    /// cartes PAS ENCORE OUVERTES (mémoire `kiosque_lus`), jamais le
    /// `seen` IMAP : c'est la sémantique de la page elle-même (les
    /// sections Non lus / Lus précédemment). Le décor est vu côté
    /// serveur (`seen = true`) : si la requête comptait l'`unseen`,
    /// elle rendrait zéro.
    #[test]
    fn la_pastille_du_kiosque_compte_les_cartes_jamais_ouvertes() {
        let (mut store, inbox) = store_with_mailbox();
        let mut a = envelope(1, "Lettre A", 100, true);
        a.sender_address = Some("lettre@infolettre.fr".to_string());
        a.message_id = Some("<a@infolettre.fr>".to_string());
        let mut b = envelope(2, "Lettre B", 200, true);
        b.sender_address = Some("lettre@infolettre.fr".to_string());
        b.message_id = Some("<b@infolettre.fr>".to_string());
        let ordinaire = envelope(3, "Bonjour", 300, false);
        store.upsert_envelopes(inbox, &[a, b, ordinaire]).unwrap();
        store
            .router_expediteur("lettre@infolettre.fr", "kiosque", None, 400)
            .unwrap();

        // Deux cartes au Kiosque, aucune ouverte — le seen IMAP (true)
        // ne compte pas ; le message non routé non plus.
        assert_eq!(store.kiosque_non_ouverts(None).unwrap(), 2);
        // Le filtre de compte se prouve PENDANT qu'il reste du non-lu
        // (revue : à zéro partout, un filtre ignoré passerait vert) :
        // le bon compte voit 2, un compte étranger 0.
        let compte = test_account(&store);
        assert_eq!(store.kiosque_non_ouverts(Some(compte)).unwrap(), 2);
        assert_eq!(store.kiosque_non_ouverts(Some(compte + 1)).unwrap(), 0);
        // Ouvrir une carte la retire du compte.
        store.marquer_kiosque_lu(inbox, 2, 500).unwrap();
        assert_eq!(store.kiosque_non_ouverts(None).unwrap(), 1);
        store.marquer_kiosque_lu(inbox, 1, 600).unwrap();
        assert_eq!(store.kiosque_non_ouverts(None).unwrap(), 0);
    }

    /// RETOURS-13 R5/R9 — les actions PAR DÉFAUT des boutons du
    /// Portier : livrées Oui → Réception, Non → Corbeille ; réglables
    /// dans un vocabulaire FERMÉ (les destinations du Oui, les règles
    /// du Non plus « écarter sans déplacer ») ; une pref corrompue
    /// retombe au défaut — jamais un verdict au vocabulaire troué.
    #[test]
    fn portier_defauts_livres_puis_reglables_au_vocabulaire_ferme() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(
            store.portier_defauts().unwrap(),
            ("reception".to_string(), "corbeille".to_string()),
            "les défauts livrés : Oui → Réception, Non → Corbeille"
        );
        store.set_portier_defauts("kiosque", "archive").unwrap();
        assert_eq!(
            store.portier_defauts().unwrap(),
            ("kiosque".to_string(), "archive".to_string())
        );
        store.set_portier_defauts("reception", "ecarte").unwrap();
        assert_eq!(store.portier_defauts().unwrap().1, "ecarte");
        // Le vocabulaire est fermé : « ecarte » n'est pas un Oui, une
        // destination n'est pas une règle du Non.
        assert!(store.set_portier_defauts("ecarte", "corbeille").is_err());
        assert!(store.set_portier_defauts("reception", "registre").is_err());
        // Une pref corrompue (écrite hors porte) retombe au défaut.
        store
            .set_text_pref("portier_defaut_oui", "poubelle")
            .unwrap();
        assert_eq!(store.portier_defauts().unwrap().0, "reception");
    }

    /// PLAN-MODE-ORGANISE E2 — la rétention du Portier (D3 « arrivées
    /// seules »). Un expéditeur SANS ligne de routage dont le courrier
    /// n'existe QU'APRÈS l'époque d'activation attend au Portier : son
    /// fil quitte le flot ET les totaux de la Réception organisée
    /// (exclusion partagée, leçon `pins`). L'historique d'un connu
    /// reste en Réception, et le mode CLASSIQUE ne bouge pas d'un
    /// message.
    #[test]
    fn un_inconnu_apres_l_epoque_attend_au_portier_hors_flot_et_totaux() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        // L'ancien : du courrier avant ET après l'époque.
        let mut avant = envelope(1, "d'hier", 500, true);
        avant.sender_address = Some("ancien@exemple.fr".to_string());
        let mut apres = envelope(2, "d'aujourd'hui", 1_500, false);
        apres.sender_address = Some("ancien@exemple.fr".to_string());
        // L'inconnu : premier message POSTÉRIEUR à l'époque.
        let mut inconnu = envelope(3, "premiere fois", 1_600, false);
        inconnu.sender = Some("Nouvelle Venue".to_string());
        inconnu.sender_address = Some("Nouv@Exemple.FR".to_string());
        store
            .upsert_envelopes(inbox, &[avant, apres, inconnu])
            .unwrap();

        let page = store
            .reception_organisee_scoped(None, false, 0, 10)
            .unwrap();
        assert_eq!(
            page.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
            vec![2, 1],
            "la Réception organisée ne sert que l'ancien"
        );
        assert_eq!(
            store.reception_organisee_count_scoped(None, false).unwrap(),
            2,
            "le total suit le flot (exclusion partagée)"
        );
        assert_eq!(
            store.unified_count_scoped(None, false).unwrap(),
            3,
            "le mode classique montre TOUJOURS tout"
        );
        let attente = store.portier_attente().unwrap();
        assert_eq!(attente.len(), 1);
        assert_eq!(attente[0].address, "nouv@exemple.fr");
        assert_eq!(
            attente[0].ligne.envelope.uid, 3,
            "le rang porte son dernier message"
        );
        assert_eq!(store.portier_total().unwrap(), 1);
    }

    /// Le guichet du Portier : le Oui nu rend l'expéditeur à la
    /// Réception, le Non avec règle l'écarte — dans les DEUX cas il
    /// quitte l'attente, et l'historique dit la règle choisie.
    #[test]
    fn le_oui_libere_le_non_ecarte_et_l_attente_se_vide() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        let mut a = envelope(1, "bonjour", 1_500, false);
        a.sender_address = Some("a@exemple.fr".to_string());
        let mut b = envelope(2, "offre", 1_600, false);
        b.sender_address = Some("b@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[a, b]).unwrap();
        assert_eq!(store.portier_attente().unwrap().len(), 2);
        assert_eq!(
            store.reception_organisee_count_scoped(None, false).unwrap(),
            0
        );

        // Oui nu → Réception : le fil revient, page ET total.
        store
            .router_expediteur("a@exemple.fr", "reception", None, 2_000)
            .unwrap();
        assert_eq!(
            store
                .portier_attente()
                .unwrap()
                .iter()
                .map(|r| r.address.as_str())
                .collect::<Vec<_>>(),
            vec!["b@exemple.fr"]
        );
        let page = store
            .reception_organisee_scoped(None, false, 0, 10)
            .unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].envelope.uid, 1);
        assert_eq!(
            store.reception_organisee_count_scoped(None, false).unwrap(),
            1
        );

        // Non avec règle → écarté : hors Réception, hors vues servies,
        // et l'historique porte la règle.
        store
            .router_expediteur("b@exemple.fr", "ecarte", Some("archive"), 2_100)
            .unwrap();
        assert!(store.portier_attente().unwrap().is_empty());
        assert_eq!(store.portier_total().unwrap(), 0);
        assert_eq!(
            store.reception_organisee_count_scoped(None, false).unwrap(),
            1,
            "l'écarté ne revient pas en Réception"
        );
        assert!(
            store
                .routage_unified_scoped("kiosque", None, false, 0, 10)
                .unwrap()
                .is_empty(),
            "écarté n'est pas une vue servie"
        );
        let verdict = store.routage_de("b@exemple.fr").unwrap().unwrap();
        assert_eq!(
            (verdict.destination.as_str(), verdict.regle.as_deref()),
            ("ecarte", Some("archive"))
        );
    }

    /// « Réintégrer » à l'historique = DELETE de la ligne : un inconnu
    /// écarté REVIENT au Portier (ses messages réapparaissent), un
    /// ancien routé revient simplement en Réception — jamais au
    /// Portier, son courrier d'avant l'époque fait foi.
    #[test]
    fn la_reintegration_rend_l_inconnu_au_portier_et_l_ancien_a_la_reception() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        let mut ancien = envelope(1, "d'hier", 500, true);
        ancien.sender_address = Some("ancien@exemple.fr".to_string());
        let mut inconnu = envelope(2, "premiere fois", 1_500, false);
        inconnu.sender_address = Some("nouv@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[ancien, inconnu]).unwrap();
        store
            .router_expediteur("nouv@exemple.fr", "ecarte", Some("spam"), 2_000)
            .unwrap();
        store
            .router_expediteur("ancien@exemple.fr", "kiosque", None, 2_000)
            .unwrap();
        assert!(store.portier_attente().unwrap().is_empty());
        assert_eq!(
            store.reception_organisee_count_scoped(None, false).unwrap(),
            0
        );

        store.retirer_routage("nouv@exemple.fr").unwrap();
        let attente = store.portier_attente().unwrap();
        assert_eq!(attente.len(), 1, "l'inconnu réintégré re-attend au Portier");
        assert_eq!(attente[0].address, "nouv@exemple.fr");

        store.retirer_routage("ancien@exemple.fr").unwrap();
        assert_eq!(
            store.portier_attente().unwrap().len(),
            1,
            "l'ancien ne passe JAMAIS au Portier : son courrier d'avant l'époque fait foi"
        );
        assert_eq!(
            store.reception_organisee_count_scoped(None, false).unwrap(),
            1,
            "l'ancien est rendu à la Réception"
        );
    }

    /// Règle d'or — jamais perdre de courrier : un fil MÊLÉ (un inconnu
    /// répond dans le fil d'un connu) RESTE en Réception ; l'inconnu
    /// attend quand même au Portier. La rétention ne prend un fil que
    /// s'il est ENTIÈREMENT à des expéditeurs en attente.
    #[test]
    fn un_fil_mele_reste_en_reception_et_l_inconnu_attend_quand_meme() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        let mut hier = envelope(1, "hier", 500, true);
        hier.sender_address = Some("connu@exemple.fr".to_string());
        let mut racine = envelope(2, "projet", 1_500, false);
        racine.sender_address = Some("connu@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[hier, racine]).unwrap();
        let mut intrus = envelope(3, "Re: projet", 1_600, false);
        intrus.sender_address = Some("nouv@exemple.fr".to_string());
        intrus.in_reply_to = Some("<m2@example.com>".to_string());
        store.upsert_envelopes(inbox, &[intrus]).unwrap();

        assert_eq!(
            store.reception_organisee_count_scoped(None, false).unwrap(),
            2,
            "le fil mêlé et le fil d'hier restent en Réception"
        );
        let attente = store.portier_attente().unwrap();
        assert_eq!(
            attente
                .iter()
                .map(|r| r.address.as_str())
                .collect::<Vec<_>>(),
            vec!["nouv@exemple.fr"],
            "l'inconnu attend au Portier même si son fil est mêlé"
        );
    }

    /// Jamais soi au Portier (leçon E1 « jamais sa propre adresse »),
    /// et jamais une attente sans adresse.
    #[test]
    fn jamais_soi_ni_sans_adresse_au_portier() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        let mut soi = envelope(1, "note a moi-meme", 1_500, false);
        soi.sender_address = Some("Test@Exemple.FR".to_string());
        let mut muet = envelope(2, "anonyme", 1_600, false);
        muet.sender_address = None;
        store.upsert_envelopes(inbox, &[soi, muet]).unwrap();
        assert!(store.portier_attente().unwrap().is_empty());
        assert_eq!(
            store.reception_organisee_count_scoped(None, false).unwrap(),
            2,
            "rien n'est retenu : ni soi, ni un message sans adresse"
        );
    }

    /// La synchro n'arrive pas dans l'ordre : si le courrier ANCIEN
    /// d'un expéditeur (antérieur à l'époque) arrive APRÈS son courrier
    /// neuf, l'attente posée à tort se défait et le fil est libéré —
    /// l'expéditeur était connu, la base ne le savait pas encore.
    #[test]
    fn le_courrier_ancien_qui_arrive_apres_coup_defait_l_attente() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        let mut neuf = envelope(1, "recent", 1_500, false);
        neuf.sender_address = Some("connu@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[neuf]).unwrap();
        assert_eq!(store.portier_attente().unwrap().len(), 1);

        let mut ancien = envelope(2, "l'historique arrive", 500, true);
        ancien.sender_address = Some("connu@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[ancien]).unwrap();
        assert!(
            store.portier_attente().unwrap().is_empty(),
            "le courrier d'avant l'époque prouve le connu"
        );
        assert_eq!(
            store.reception_organisee_count_scoped(None, false).unwrap(),
            2,
            "ses fils sont libérés, page et totaux"
        );
    }

    /// L'attente est DÉRIVÉE du courrier : quand la boîte se
    /// réinitialise (UIDVALIDITY), les rangs du Portier qui ne
    /// s'appuient plus sur rien meurent avec elle (leçon A43/A89 — un
    /// UID recyclé ne doit hériter d'aucune décision).
    #[test]
    fn l_attente_meurt_avec_le_courrier_qui_la_portait() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        let mut inconnu = envelope(1, "premiere fois", 1_500, false);
        inconnu.sender_address = Some("nouv@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[inconnu]).unwrap();
        assert_eq!(store.portier_attente().unwrap().len(), 1);

        store.reset_mailbox(inbox, 2).unwrap();
        assert!(
            store.portier_attente().unwrap().is_empty(),
            "plus de courrier, plus d'attente"
        );
        assert_eq!(store.portier_total().unwrap(), 0);
    }

    /// Revue E2, règle d'or — jamais perdre de courrier : le Non sur un
    /// INTRUS (un écarté qui a répondu dans le fil d'un connu) ne cache
    /// pas le fil du connu. `ecarte` n'a AUCUNE vue servie : cacher le
    /// fil mêlé le ferait disparaître de partout. Seul un fil
    /// ENTIÈREMENT aux écartés/attente se cache.
    #[test]
    fn le_non_sur_un_intrus_ne_cache_pas_le_fil_du_connu() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        let mut hier = envelope(1, "hier", 500, true);
        hier.sender_address = Some("connu@exemple.fr".to_string());
        let mut racine = envelope(2, "projet", 1_500, false);
        racine.sender_address = Some("connu@exemple.fr".to_string());
        let mut intrus = envelope(3, "Re: projet", 1_600, false);
        intrus.sender_address = Some("spam@exemple.fr".to_string());
        intrus.in_reply_to = Some("<m2@example.com>".to_string());
        store
            .upsert_envelopes(inbox, &[hier, racine, intrus])
            .unwrap();
        // Un inconnu SEUL, écarté lui aussi : son fil, entièrement à
        // lui, se cache — le contraste qui prouve la règle.
        let mut seul = envelope(4, "offre", 1_700, false);
        seul.sender_address = Some("promo@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[seul]).unwrap();

        store
            .router_expediteur("spam@exemple.fr", "ecarte", Some("spam"), 2_000)
            .unwrap();
        store
            .router_expediteur("promo@exemple.fr", "ecarte", None, 2_000)
            .unwrap();
        let page = store
            .reception_organisee_scoped(None, false, 0, 10)
            .unwrap();
        assert_eq!(
            page.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
            vec![3, 1],
            "le fil mêlé du connu RESTE (tête intruse comprise), le fil du promo seul se cache"
        );
        assert_eq!(
            store.reception_organisee_count_scoped(None, false).unwrap(),
            2
        );
        assert!(
            store
                .routage_unified_scoped("kiosque", None, false, 0, 10)
                .unwrap()
                .is_empty(),
            "écarté n'est pas une vue servie"
        );
    }

    /// Un message SANS en-tête Date ne prouve JAMAIS le connu : le
    /// traiter comme antérieur à l'époque ferait contourner le guichet
    /// par les expéditeurs mêmes qu'il existe pour trier (le spam sans
    /// Date est courant) — et défairait une attente légitime.
    #[test]
    fn un_message_sans_date_n_est_jamais_une_preuve_de_connu() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        let mut sans_date = envelope(1, "sans date", 0, false);
        sans_date.sender_address = Some("nouv@exemple.fr".to_string());
        sans_date.date = None;
        store.upsert_envelopes(inbox, &[sans_date]).unwrap();
        assert_eq!(
            store.portier_attente().unwrap().len(),
            1,
            "l'inconnu sans date attend au guichet — jamais un contournement"
        );

        let mut datee = envelope(2, "datee", 1_500, false);
        datee.sender_address = Some("autre@exemple.fr".to_string());
        let mut sans_date2 = envelope(3, "re-sans date", 0, false);
        sans_date2.sender_address = Some("autre@exemple.fr".to_string());
        sans_date2.date = None;
        store.upsert_envelopes(inbox, &[datee, sans_date2]).unwrap();
        assert_eq!(
            store
                .portier_attente()
                .unwrap()
                .iter()
                .filter(|r| r.address == "autre@exemple.fr")
                .count(),
            1,
            "un second message sans date ne défait pas l'attente"
        );
    }

    /// La réintégration suit la MÊME règle que l'arrivée (D3) : seul un
    /// expéditeur dont du courrier est ARRIVÉ (INBOX) après l'époque
    /// re-attend au Portier — un expéditeur vu seulement en Archives ou
    /// aux Indésirables n'a jamais passé le guichet, il n'y entre pas
    /// par la porte de sortie.
    #[test]
    fn la_reintegration_n_admet_que_les_arrivees() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        let archives = store
            .create_mailbox(test_account(&store), "Archives", 1)
            .unwrap();
        let mut hors_guichet = envelope(1, "vu en archives", 1_500, true);
        hors_guichet.sender_address = Some("ailleurs@exemple.fr".to_string());
        store.upsert_envelopes(archives, &[hors_guichet]).unwrap();
        let mut arrive = envelope(1, "arrive", 1_600, false);
        arrive.sender_address = Some("guichet@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[arrive]).unwrap();

        store
            .router_expediteur("ailleurs@exemple.fr", "ecarte", None, 2_000)
            .unwrap();
        store
            .router_expediteur("guichet@exemple.fr", "ecarte", None, 2_000)
            .unwrap();
        store.retirer_routage("ailleurs@exemple.fr").unwrap();
        store.retirer_routage("guichet@exemple.fr").unwrap();
        assert_eq!(
            store
                .portier_attente()
                .unwrap()
                .iter()
                .map(|r| r.address.as_str())
                .collect::<Vec<_>>(),
            vec!["guichet@exemple.fr"],
            "seule l'arrivée réintègre au guichet"
        );
    }

    /// La pastille et le guichet ne disent que les ARRIVÉES : un
    /// message du même expéditeur vivant ailleurs (corbeille,
    /// archives) n'est ni compté ni servi comme rang.
    #[test]
    fn le_guichet_ne_compte_que_les_arrivees() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        let corbeille = store
            .create_mailbox(test_account(&store), "Corbeille", 1)
            .unwrap();
        let mut arrive = envelope(1, "arrive", 1_500, false);
        arrive.sender_address = Some("nouv@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[arrive]).unwrap();
        let mut jetee = envelope(1, "deja jetee", 1_600, false);
        jetee.sender_address = Some("nouv@exemple.fr".to_string());
        store.upsert_envelopes(corbeille, &[jetee]).unwrap();

        assert_eq!(
            store.portier_total().unwrap(),
            1,
            "la corbeille ne compte pas"
        );
        let attente = store.portier_attente().unwrap();
        assert_eq!(attente.len(), 1);
        assert_eq!(
            attente[0].ligne.envelope.uid, 1,
            "le rang montre l'arrivée, jamais le message jeté"
        );
        assert_eq!(attente[0].ligne.mailbox, "INBOX");
    }

    /// L'exclusion partagée s'étend aux ÉPINGLES et au compteur de la
    /// nav : en Réception organisée, un fil épinglé routé au Kiosque ne
    /// se prépose plus (il vit dans sa vue), et le non-lu d'un retenu ne
    /// gonfle pas la pastille de la Réception — le classique, lui, ne
    /// bouge pas.
    #[test]
    fn les_epingles_et_la_pastille_suivent_l_exclusion_partagee() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        let mut lettre = envelope(1, "la lettre", 500, false);
        lettre.sender_address = Some("lettre@exemple.fr".to_string());
        let ordinaire = envelope(2, "bonjour", 600, false);
        store.upsert_envelopes(inbox, &[lettre, ordinaire]).unwrap();
        assert!(store.toggle_pin(inbox, 1, 700).unwrap());
        store
            .router_expediteur("lettre@exemple.fr", "kiosque", None, 2_000)
            .unwrap();
        let mut retenu = envelope(3, "premiere fois", 1_500, false);
        retenu.sender_address = Some("nouv@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[retenu]).unwrap();

        assert!(
            store
                .pinned_unified_scoped(None, false, true)
                .unwrap()
                .is_empty(),
            "l'épingle d'un fil routé ne se prépose plus en Réception organisée"
        );
        assert_eq!(
            store
                .pinned_unified_scoped(None, false, false)
                .unwrap()
                .len(),
            1,
            "le classique garde son épingle"
        );
        let compte = test_account(&store);
        let dossiers = store.canonical_folders(compte).unwrap();
        let (organise, _) = store.nav_unread_counts(compte, &dossiers, true).unwrap();
        assert_eq!(
            organise, 1,
            "seul l'ordinaire non-lu compte (le routé épinglé et le retenu, non)"
        );
        let (classique, _) = store.nav_unread_counts(compte, &dossiers, false).unwrap();
        assert_eq!(classique, 3);
    }

    /// E1 → E2 au terrain : le mode a pu être ACTIVÉ avant cette
    /// version (terrain E1 sur les postes du CE) — les inconnus
    /// arrivés entre l'activation et la mise à jour se rattrapent à la
    /// migration, sinon ils passeraient le guichet pour toujours, en
    /// silence. Décor : une base E2 dont on efface les artefacts E2
    /// (colonne + attente) pour rejouer l'état E1 exact, puis une
    /// réouverture.
    #[test]
    fn la_migration_rattrape_l_attente_d_une_base_d_avant_e2() {
        let path = std::env::temp_dir().join(format!(
            "wind-test-rattrapage-portier-{}.db",
            std::process::id()
        ));
        for suffixe in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffixe}", path.display()));
        }
        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("test@exemple.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store.set_mode_organise(true, 1_000).unwrap();
            let mut ancien = envelope(1, "d'hier", 500, true);
            ancien.sender_address = Some("ancien@exemple.fr".to_string());
            let mut inconnu = envelope(2, "premiere fois", 1_500, false);
            inconnu.sender_address = Some("nouv@exemple.fr".to_string());
            store.upsert_envelopes(inbox, &[ancien, inconnu]).unwrap();
            // Rejoue l'état E1 : ni colonne de drapeau, ni attente.
            // Reconstruction (pas de DROP COLUMN : SQLite bute sur les
            // commentaires du SQL stocké — « incomplete input »).
            store
                .0
                .execute_batch(
                    "DELETE FROM portier_attente;
                     PRAGMA foreign_keys = OFF;
                     CREATE TABLE threads_e1 AS
                       SELECT id, account_id, last_mailbox_id, last_uid,
                              last_epoch, size, unseen, inbox_size FROM threads;
                     DROP TABLE threads;
                     ALTER TABLE threads_e1 RENAME TO threads;
                     PRAGMA foreign_keys = ON;",
                )
                .unwrap();
        }
        Store::oublier_initialisation(&path);
        let store = Store::open(&path).unwrap();
        let attente = store.portier_attente().unwrap();
        assert_eq!(
            attente
                .iter()
                .map(|r| r.address.as_str())
                .collect::<Vec<_>>(),
            vec!["nouv@exemple.fr"],
            "l'inconnu d'avant la mise à jour re-attend au guichet"
        );
        assert_eq!(
            store.reception_organisee_count_scoped(None, false).unwrap(),
            1,
            "son fil est retenu, celui de l'ancien reste"
        );
        drop(store);
        for suffixe in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffixe}", path.display()));
        }
    }

    /// PLAN-MODE-ORGANISE E3 — les règles du Non à la synchro. Un
    /// message qui ARRIVE d'un expéditeur écarté AVEC règle est traité
    /// PLAN-HORIZON-NETTOYAGE volet B (D5-D8) — la session de
    /// nettoyage : une seule, persistée ; démarrer fige la borne et
    /// compte les groupes ; le verdict de GROUPE route l'avenir ET
    /// traite le stock DE LA PLAGE (jamais l'antérieur) ; la
    /// progression avance ; terminer efface la session.
    #[test]
    fn nettoyage_session_groupes_verdicts_et_progression() {
        const JOUR: i64 = 86_400;
        let now = 100 * JOUR;
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();

        let sème = |uid, sujet: &str, epoch, adresse: &str| {
            let mut e = envelope(uid, sujet, epoch, true);
            e.sender_address = Some(adresse.to_string());
            e
        };
        store
            .upsert_envelopes(
                inbox,
                &[
                    sème(1, "lettre", now - 2 * JOUR, "un@exemple.fr"),
                    sème(2, "relance", now - JOUR, "un@exemple.fr"),
                    sème(3, "offre", now - 3 * JOUR, "deux@exemple.fr"),
                    // Le stock ANTÉRIEUR à la plage du même expéditeur :
                    // jamais touché par le verdict.
                    sème(5, "tres vieille offre", 500, "deux@exemple.fr"),
                    // Un expéditeur entièrement hors plage : pas un groupe.
                    sème(4, "archives", 1_000, "vieux@exemple.fr"),
                    // Déjà routé (D7) : jamais re-demandé.
                    sème(6, "news", now - JOUR, "route@exemple.fr"),
                    // Soi-même : jamais un groupe.
                    sème(7, "note a moi", now - JOUR, "test@exemple.fr"),
                ],
            )
            .unwrap();
        store
            .router_expediteur("route@exemple.fr", "kiosque", None, 2_000)
            .unwrap();

        assert!(store.nettoyage_etat().unwrap().is_none());
        assert!(
            store
                .nettoyage_demarrer("un siecle", "reception", now)
                .is_err(),
            "le vocabulaire des plages est fermé"
        );
        assert!(
            store.nettoyage_demarrer("3m", "le grenier", now).is_err(),
            "le vocabulaire des périmètres est fermé"
        );

        let session = store.nettoyage_demarrer("3m", "reception", now).unwrap();
        assert_eq!((session.total, session.traites), (2, 0));
        let groupes = store.nettoyage_groupes().unwrap();
        assert_eq!(
            groupes
                .iter()
                .map(|g| (g.address.as_str(), g.messages))
                .collect::<Vec<_>>(),
            vec![("un@exemple.fr", 2), ("deux@exemple.fr", 1)],
            "les groupes de la plage, le plus récent en tête — routés, soi et hors-plage exclus"
        );

        // Oui de groupe : routage seul, aucune action serveur.
        store
            .nettoyage_verdict("un@exemple.fr", "reception", None, now)
            .unwrap();
        assert!(store.pending_actions(inbox).unwrap().is_empty());
        let etat = store.nettoyage_etat().unwrap().unwrap();
        assert_eq!((etat.total, etat.traites), (2, 1));
        assert_eq!(store.nettoyage_groupes().unwrap().len(), 1);

        // Naviguer dans un groupe : SES messages de la plage, jamais
        // l'antérieur — la lecture que l'écran de tri offre au clic.
        let dedans = store.nettoyage_messages("deux@exemple.fr").unwrap();
        assert_eq!(
            dedans.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
            vec![3],
            "le groupe montre son courrier de la plage seulement"
        );

        // Non + corbeille : le stock DE LA PLAGE part (uid 3), jamais
        // l'antérieur (uid 5) ; l'action est la corbeille du serveur.
        store
            .nettoyage_verdict("deux@exemple.fr", "ecarte", Some("corbeille"), now)
            .unwrap();
        let actions = store.pending_actions(inbox).unwrap();
        assert_eq!(
            actions
                .iter()
                .map(|a| (a.uid, a.action.clone()))
                .collect::<Vec<_>>(),
            vec![(3, Action::Delete)],
            "le stock de la plage seulement — D4 : jamais une suppression définitive"
        );
        let compte = test_account(&store);
        assert!(
            store.envelope(compte, "INBOX", 5).unwrap().is_some(),
            "l'antérieur à la plage reste en base"
        );
        assert!(
            store.envelope(compte, "INBOX", 3).unwrap().is_none(),
            "le stock traité quitte la copie locale"
        );
        let etat = store.nettoyage_etat().unwrap().unwrap();
        assert_eq!((etat.total, etat.traites), (2, 2));

        store.nettoyage_terminer().unwrap();
        assert!(store.nettoyage_etat().unwrap().is_none());
        assert!(
            store
                .nettoyage_verdict("vieux@exemple.fr", "reception", None, now)
                .is_err(),
            "un verdict sans session en cours se refuse"
        );
    }

    /// D6 (CE, mot pour mot) : le périmètre se choisit — « Réception
    /// seule » ignore les dossiers utilisateur, « Réception +
    /// Dossiers » les couvre.
    #[test]
    fn nettoyage_perimetre_reception_ou_dossiers() {
        const JOUR: i64 = 86_400;
        let now = 100 * JOUR;
        let (mut store, inbox) = store_with_mailbox();
        let account = test_account(&store);
        store.set_mode_organise(true, 1_000).unwrap();
        let projets = store.create_mailbox(account, "Projets", 1).unwrap();

        let mut boite = envelope(1, "bonjour", now - JOUR, true);
        boite.sender_address = Some("un@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[boite]).unwrap();
        let mut range = envelope(1, "range", now - JOUR, true);
        range.sender_address = Some("proj@exemple.fr".to_string());
        store.upsert_envelopes(projets, &[range]).unwrap();

        let session = store.nettoyage_demarrer("tout", "reception", now).unwrap();
        assert_eq!(session.total, 1, "Réception seule : le dossier n'entre pas");
        store.nettoyage_terminer().unwrap();

        let session = store.nettoyage_demarrer("tout", "dossiers", now).unwrap();
        assert_eq!(session.total, 2, "Réception + Dossiers : les deux groupes");
        let adresses: Vec<_> = store
            .nettoyage_groupes()
            .unwrap()
            .into_iter()
            .map(|g| g.address)
            .collect();
        assert!(adresses.contains(&"proj@exemple.fr".to_string()));
    }

    /// par le chemin des gestes : action journalisée (`pending_actions`,
    /// rejouée en tête de chaque synchro) + disparition locale — sans
    /// écho (ce n'est pas un geste utilisateur). `archive` → Archive,
    /// `corbeille` → Delete (la corbeille du serveur, JAMAIS une
    /// suppression définitive — D4).
    #[test]
    fn la_regle_du_non_s_execute_a_l_arrivee() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        store
            .router_expediteur("promo@exemple.fr", "ecarte", Some("archive"), 2_000)
            .unwrap();
        store
            .router_expediteur("pub@exemple.fr", "ecarte", Some("corbeille"), 2_000)
            .unwrap();
        let mut offre = envelope(1, "offre", 2_500, false);
        offre.sender_address = Some("promo@exemple.fr".to_string());
        let mut relance = envelope(2, "relance", 2_600, false);
        relance.sender_address = Some("pub@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[offre, relance]).unwrap();

        assert_eq!(
            store.count(inbox).unwrap(),
            0,
            "les deux ont quitté la boîte locale"
        );
        let actions = store.pending_actions(inbox).unwrap();
        assert_eq!(
            actions
                .iter()
                .map(|a| (a.uid, a.action.clone()))
                .collect::<Vec<_>>(),
            vec![(1, Action::Archive), (2, Action::Delete)],
            "archive → Archive, corbeille → Delete (jamais définitive)"
        );
    }

    /// La règle `spam` part vers le dossier indésirable RÉSOLU du compte
    /// (`canonical_folders`, comme le geste) ; sans dossier reconnu, on
    /// ne fait RIEN — jamais une destination inventée (règle d'or).
    #[test]
    fn la_regle_spam_va_au_dossier_indesirable_resolu() {
        let (mut store, inbox) = store_with_mailbox();
        let account = test_account(&store);
        store.set_mode_organise(true, 1_000).unwrap();
        store
            .router_expediteur("arnaque@exemple.fr", "ecarte", Some("spam"), 2_000)
            .unwrap();
        // Sans dossier indésirable reconnu : le message RESTE.
        let mut avant = envelope(1, "avant", 2_500, false);
        avant.sender_address = Some("arnaque@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[avant]).unwrap();
        assert_eq!(
            store.count(inbox).unwrap(),
            1,
            "sans dossier reconnu, rien ne bouge"
        );
        assert!(store.pending_actions(inbox).unwrap().is_empty());

        store
            .replace_folders(
                account,
                &[crate::Folder {
                    wire: "Junk".to_string(),
                    display: "Junk".to_string(),
                    selectable: true,
                    special_use: None,
                }],
            )
            .unwrap();
        let mut apres = envelope(2, "apres", 2_600, false);
        apres.sender_address = Some("arnaque@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[apres]).unwrap();
        assert_eq!(
            store.count(inbox).unwrap(),
            1,
            "le nouveau est parti, l'ancien reste"
        );
        assert_eq!(
            store
                .pending_actions(inbox)
                .unwrap()
                .iter()
                .map(|a| (a.uid, a.action.clone()))
                .collect::<Vec<_>>(),
            vec![(2, Action::MoveTo("Junk".to_string()))]
        );
    }

    /// D2 — les règles du Non S'ÉTEIGNENT avec le mode : mode désactivé,
    /// un message d'un écarté avec règle arrive et RESTE. Et un écarté
    /// SANS règle ne déclenche jamais rien (le Non nu ne fait que
    /// cacher).
    #[test]
    fn les_regles_du_non_s_eteignent_avec_le_mode() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        store
            .router_expediteur("promo@exemple.fr", "ecarte", Some("archive"), 2_000)
            .unwrap();
        store
            .router_expediteur("muet@exemple.fr", "ecarte", None, 2_000)
            .unwrap();
        store.set_mode_organise(false, 3_000).unwrap();
        let mut pendant_off = envelope(1, "pendant off", 3_500, false);
        pendant_off.sender_address = Some("promo@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[pendant_off]).unwrap();
        assert_eq!(
            store.count(inbox).unwrap(),
            1,
            "mode éteint : la règle dort"
        );
        assert!(store.pending_actions(inbox).unwrap().is_empty());

        store.set_mode_organise(true, 4_000).unwrap();
        let mut sans_regle = envelope(2, "sans regle", 4_500, false);
        sans_regle.sender_address = Some("muet@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[sans_regle]).unwrap();
        assert_eq!(store.count(inbox).unwrap(), 2, "le Non nu ne traite rien");
        assert!(store.pending_actions(inbox).unwrap().is_empty());
    }

    /// Re-livraison (revue E3) : le retrait local fait reculer
    /// `max_uid` — si le rejeu échoue, la synchro suivante re-présente
    /// le même uid. La règle re-retire localement mais ne JOURNALISE
    /// jamais deux fois : une seconde action identique sur un uid déjà
    /// parti du serveur coincerait toute la file du rejeu derrière un
    /// échec permanent.
    #[test]
    fn une_re_livraison_ne_journalise_jamais_deux_fois() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        store
            .router_expediteur("promo@exemple.fr", "ecarte", Some("archive"), 2_000)
            .unwrap();
        let mut offre = envelope(1, "offre", 2_500, false);
        offre.sender_address = Some("promo@exemple.fr".to_string());
        store
            .upsert_envelopes(inbox, std::slice::from_ref(&offre))
            .unwrap();
        // Le serveur re-présente le même uid (rejeu pas encore passé).
        store.upsert_envelopes(inbox, &[offre]).unwrap();
        assert_eq!(store.count(inbox).unwrap(), 0, "re-retiré localement");
        assert_eq!(
            store
                .pending_actions(inbox)
                .unwrap()
                .iter()
                .map(|a| (a.uid, a.action.clone()))
                .collect::<Vec<_>>(),
            vec![(1, Action::Archive)],
            "UNE seule action journalisée"
        );
    }

    /// « Ses PROCHAINS messages » (les toasts du guichet) : la règle ne
    /// touche que le courrier POSTÉRIEUR au verdict — un backfill de
    /// courrier ancien (ajout d'un compte, désordre de synchro)
    /// n'archive ni ne jette jamais l'historique. Un message SANS date
    /// est une arrivée d'aujourd'hui : la règle s'applique.
    #[test]
    fn la_regle_ne_touche_jamais_le_courrier_anterieur_au_verdict() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        store
            .router_expediteur("promo@exemple.fr", "ecarte", Some("corbeille"), 2_000)
            .unwrap();
        let mut ancien = envelope(1, "d'avant le verdict", 1_500, true);
        ancien.sender_address = Some("promo@exemple.fr".to_string());
        let mut sans_date = envelope(2, "sans date", 0, false);
        sans_date.sender_address = Some("promo@exemple.fr".to_string());
        sans_date.date = None;
        store.upsert_envelopes(inbox, &[ancien, sans_date]).unwrap();
        assert_eq!(
            store.count(inbox).unwrap(),
            1,
            "l'antérieur au verdict reste ; le sans-date (arrivée d'aujourd'hui) est traité"
        );
        assert_eq!(
            store
                .pending_actions(inbox)
                .unwrap()
                .iter()
                .map(|a| (a.uid, a.action.clone()))
                .collect::<Vec<_>>(),
            vec![(2, Action::Delete)]
        );
    }

    /// PLAN-MODE-ORGANISE E4 — les sections de la Réception organisée
    /// (verdict S1, variante A2) : UN flot ordonné « non-lus d'abord,
    /// puis la date » — « Nouveau pour vous » puis « Déjà consulté »
    /// sont DEUX bornes de la même source paginée, la couture est le
    /// COUNT des non-lus. Le classique, lui, ne bouge pas d'un rang.
    #[test]
    fn la_reception_organisee_sert_les_non_lus_en_tete() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "lu ancien", 100, true),
                    envelope(2, "nonlu recent", 200, false),
                    envelope(3, "lu recent", 300, true),
                    envelope(4, "nonlu ancien", 150, false),
                ],
            )
            .unwrap();
        let organise = store
            .reception_organisee_scoped(None, false, 0, 10)
            .unwrap();
        assert_eq!(
            organise.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
            vec![2, 4, 3, 1],
            "les non-lus d'abord (par date), puis les lus (par date)"
        );
        let compte = test_account(&store);
        let borne = store
            .reception_organisee_scoped(Some(compte), false, 0, 10)
            .unwrap();
        assert_eq!(
            borne.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
            vec![2, 4, 3, 1],
            "même ordre borné à un compte"
        );
        assert_eq!(
            store.reception_organisee_count_scoped(None, true).unwrap(),
            2,
            "la couture : le COUNT des non-lus dit où la seconde section commence"
        );
        // Le classique, INTACT : la date seule.
        let classique = store.unified_recent_scoped(None, false, 0, 10).unwrap();
        assert_eq!(
            classique.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
            vec![3, 2, 4, 1]
        );
    }

    /// PLAN-MODE-ORGANISE E5 — Mis de côté (patron `pins` : clé
    /// d'ENVELOPPE qui survit à la reconstruction des fils, état par
    /// FIL). Un fil mis de côté quitte TOUTES les vues organisées —
    /// Réception, sa vue de routage, les épingles préposées — et vit
    /// dans la pile ; « Terminé » le rend d'où il vient. Le mode
    /// CLASSIQUE ne bouge pas d'un message.
    #[test]
    fn un_fil_mis_de_cote_vit_dans_la_pile_et_revient_termine() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        let mut lettre = envelope(1, "la lettre", 100, false);
        lettre.sender_address = Some("lettre@exemple.fr".to_string());
        let ordinaire = envelope(2, "bonjour", 200, false);
        store.upsert_envelopes(inbox, &[lettre, ordinaire]).unwrap();
        store
            .router_expediteur("lettre@exemple.fr", "kiosque", None, 300)
            .unwrap();

        assert!(store.toggle_mis_de_cote(inbox, 2, 1_000).unwrap());
        assert!(store.etat_mis_de_cote(inbox, 2).unwrap());
        assert!(
            store
                .reception_organisee_scoped(None, false, 0, 10)
                .unwrap()
                .is_empty(),
            "le fil mis de côté quitte la Réception organisée"
        );
        assert_eq!(
            store.reception_organisee_count_scoped(None, false).unwrap(),
            0,
            "le total suit (exclusion partagée)"
        );
        assert_eq!(
            store.unified_count_scoped(None, false).unwrap(),
            2,
            "le classique montre TOUJOURS tout"
        );
        // La pile : la mini-carte du fil, la plus récente en tête.
        assert!(store.toggle_mis_de_cote(inbox, 1, 1_100).unwrap());
        let pile = store.pile_mis_de_cote().unwrap();
        assert_eq!(
            pile.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
            vec![2, 1],
            "la pile, du plus récent au plus ancien"
        );
        assert!(
            store
                .routage_unified_scoped("kiosque", None, false, 0, 10)
                .unwrap()
                .is_empty(),
            "mis de côté, la lettre quitte AUSSI sa vue de routage"
        );

        // « Terminé » : le fil revient D'OÙ IL VIENT.
        assert!(!store.toggle_mis_de_cote(inbox, 2, 1_200).unwrap());
        assert_eq!(
            store.reception_organisee_count_scoped(None, false).unwrap(),
            1,
            "l'ordinaire revient en Réception"
        );
        assert!(!store.toggle_mis_de_cote(inbox, 1, 1_300).unwrap());
        assert_eq!(
            store
                .routage_unified_scoped("kiosque", None, false, 0, 10)
                .unwrap()
                .len(),
            1,
            "la lettre revient au Kiosque"
        );
        assert!(store.pile_mis_de_cote().unwrap().is_empty());

        // La pastille de la nav suit la pile (constat de capture E5) :
        // un non-lu mis de côté ne compte plus en mode organisé.
        assert!(store.toggle_mis_de_cote(inbox, 2, 1_400).unwrap());
        let compte = test_account(&store);
        let dossiers = store.canonical_folders(compte).unwrap();
        let (organise, _) = store.nav_unread_counts(compte, &dossiers, true).unwrap();
        assert_eq!(organise, 0, "le non-lu mis de côté quitte la pastille");
        let (classique, _) = store.nav_unread_counts(compte, &dossiers, false).unwrap();
        assert_eq!(classique, 2, "le classique ne bouge pas");
    }

    /// La mise de côté suit le FIL (patron pins) : posée sur un
    /// message, elle tient quand une réponse déplace la tête ; une
    /// épingle mise de côté quitte la section préposée de la Réception
    /// organisée (le classique la garde).
    #[test]
    fn la_mise_de_cote_suit_le_fil_et_retire_l_epingle_preposee() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_mode_organise(true, 1_000).unwrap();
        store
            .upsert_envelopes(inbox, &[envelope(1, "sujet", 100, true)])
            .unwrap();
        assert!(store.toggle_pin(inbox, 1, 500).unwrap());
        assert!(store.toggle_mis_de_cote(inbox, 1, 600).unwrap());
        let mut reponse = envelope(2, "Re: sujet", 700, true);
        reponse.in_reply_to = Some("<m1@example.com>".to_string());
        store.upsert_envelopes(inbox, &[reponse]).unwrap();

        assert!(
            store.etat_mis_de_cote(inbox, 2).unwrap(),
            "l'état se lit par le fil, tête nouvelle comprise"
        );
        assert!(
            store
                .pinned_unified_scoped(None, false, true)
                .unwrap()
                .is_empty(),
            "l'épingle d'un fil mis de côté ne se prépose plus en mode organisé"
        );
        assert_eq!(
            store
                .pinned_unified_scoped(None, false, false)
                .unwrap()
                .len(),
            1,
            "le classique garde son épingle"
        );
        // « Terminé » depuis la tête NOUVELLE libère le fil entier.
        assert!(!store.toggle_mis_de_cote(inbox, 2, 800).unwrap());
        assert!(!store.etat_mis_de_cote(inbox, 1).unwrap());
    }

    /// A43/A89 : la mise de côté meurt avec son courrier — une boîte
    /// réinitialisée (UIDVALIDITY) et un retrait local la purgent, un
    /// UID recyclé n'hérite de rien.
    #[test]
    fn la_mise_de_cote_meurt_avec_son_courrier() {
        let (mut store, inbox) = store_with_mailbox();
        store
            .upsert_envelopes(
                inbox,
                &[envelope(1, "a", 100, true), envelope(2, "b", 200, true)],
            )
            .unwrap();
        assert!(store.toggle_mis_de_cote(inbox, 1, 300).unwrap());
        store.remove_local(inbox, 1).unwrap();
        assert!(store.pile_mis_de_cote().unwrap().is_empty());

        assert!(store.toggle_mis_de_cote(inbox, 2, 400).unwrap());
        store.reset_mailbox(inbox, 2).unwrap();
        assert!(
            store.pile_mis_de_cote().unwrap().is_empty(),
            "l'UIDVALIDITY neuve ne laisse aucune mise de côté fantôme"
        );
    }

    /// La garde de plan de la Réception organisée (leçon S2-bis,
    /// spikes/routage-plan) : la page suit l'index PARTIEL miroir
    /// (`idx_threads_date_organise`) — offset stable par construction,
    /// jamais une sonde par rangée sautée, jamais un scan d'envelopes.
    #[test]
    fn la_reception_organisee_suit_l_index_partiel_jamais_un_scan() {
        let store = Store::open_in_memory().unwrap();
        let plan: Vec<String> = store
            .0
            .prepare(&format!(
                "EXPLAIN QUERY PLAN {}",
                unified_page_sql(false, false, true)
            ))
            .unwrap()
            .query_map(params![10, 0], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            plan.iter().any(|l| l.contains("idx_threads_date_organise")),
            "la page ne suit pas l'index partiel : {plan:?}"
        );
        assert!(
            !plan
                .iter()
                .any(|l| l.starts_with("SCAN") && l.contains("envelopes")),
            "plan avec scan d'envelopes : {plan:?}"
        );
        // E4 : l'index PORTE le tri à sections DANS le squelette
        // paginé — un tri matérialisé AVANT le LIMIT serait le tri de
        // toute la boîte (548 ms mesurées au spike S1 sans l'index
        // d'expression). Le re-tri EXTERNE des ≤200 lignes retenues
        // (après « SCAN t ») est borné et légitime — l'expression de
        // section ne se dérive pas de la jointure.
        let jointure = plan
            .iter()
            .position(|l| l == "SCAN t")
            .expect("le plan a perdu sa co-routine paginée");
        assert!(
            !plan[..jointure].iter().any(|l| l.contains("TEMP B-TREE")),
            "tri matérialisé DANS le squelette paginé : {plan:?}"
        );
        // Revue E4 : les DEUX autres chemins organisés portent la même
        // garde — la vue « Boîtes » (index préfixé par compte) et
        // l'onglet Non lus. Sans elle, un changement de clé d'index
        // rendrait le tri matérialisé de S1 (548 ms/page) en silence.
        for (nom, sql, params_n) in [
            (
                "par compte",
                unified_page_sql(true, false, true),
                params![10, 0, 1].to_vec(),
            ),
            (
                "non-lus",
                unified_page_sql(false, true, true),
                params![10, 0].to_vec(),
            ),
        ] {
            let plan: Vec<String> = store
                .0
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .unwrap()
                .query_map(rusqlite::params_from_iter(params_n), |row| {
                    row.get::<_, String>(3)
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert!(
                plan.iter().any(|l| l.contains("idx_threads_date_organise")),
                "chemin organisé « {nom} » sans index partiel : {plan:?}"
            );
            let jointure = plan
                .iter()
                .position(|l| l == "SCAN t")
                .expect("co-routine paginée absente");
            assert!(
                !plan[..jointure].iter().any(|l| l.contains("TEMP B-TREE")),
                "chemin organisé « {nom} » : tri matérialisé dans le squelette : {plan:?}"
            );
        }
    }

    /// L'historique du Portier lit la liste du plus récent décidé au
    /// plus ancien — l'œil y cherche la dernière décision.
    #[test]
    fn routages_se_listent_du_plus_recent() {
        let store = Store::open_in_memory().unwrap();
        store
            .router_expediteur("ancien@ex.fr", "registre", None, 100)
            .unwrap();
        store
            .router_expediteur("recent@ex.fr", "ecarte", Some("archive"), 200)
            .unwrap();
        let liste = store.routages().unwrap();
        assert_eq!(
            liste.iter().map(|r| r.address.as_str()).collect::<Vec<_>>(),
            vec!["recent@ex.fr", "ancien@ex.fr"]
        );
        assert_eq!(liste[0].regle.as_deref(), Some("archive"));
    }
}
