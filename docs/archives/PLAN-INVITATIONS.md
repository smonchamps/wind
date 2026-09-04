> **Historical record — French, frozen** (closed on 2026-08-23; PLAN-ENGLISH-SWITCH
> D1, debt D-58). Not translated; the living documentation is in `docs/`.

# PLAN-INVITATIONS — traiter une invitation de réunion reçue

> **CHANTIER SOLDÉ le 2026-08-23 — terrain complet.** GO CE (STOP 1,
> D1-D7) le 2026-08-22 ; livré en un commit `1c159bc` (A76 + ADR
> 0024), CI verte run 32605745661. Terrain en QUATRE passes les
> 2026-08-22/23, chaque constat corrigé dans la session : 1re passe
> (lien croisé d'annulation, invitation transférée répondable, gestes
> et visage dans la liste, somme des pièces du fil), 2e passe
> (optimisme instantané, icônes + tons, Supprimer par message), 3e
> passe (puce instantanée même sans sélection — bump `version` du
> fenêtrage), 4e passe : OK. Reports : DEBT D-29, D-30, D-31.
> **Livré en 0.7.0** (décision D7), publiée le 2026-08-23 et vérifiée
> **18/18 PASS** le jour même.

> Chantier ouvert le 2026-08-22 (`/job`), sur le retour
> pré-bêta : « les utilisateurs ne peuvent pas traiter les invitations
> à des réunions reçues ». Énoncé : afficher une invitation en **carte
> lisible** (titre, date/heure, organisateur, lieu, statut) et pouvoir
> **accepter / répondre provisoire / refuser**, la réponse iTIP
> (`METHOD:REPLY`) partant par le chemin d'envoi existant. Le concept
> paper ([PLAN.md](PLAN.md) §1) excluait « le calendrier » de la v1 ;
> le CE a tranché le 2026-08-22 : le fait terrain rouvre le point, mais
> le périmètre reste **une fonctionnalité email** — pas un calendrier.

---

## Constat — faits vérifiés sur pièces (2026-08-22)

### 1. Ce qu'une invitation devient aujourd'hui dans Wind

- Une invitation Gmail/Outlook arrive en `multipart/alternative`
  (texte, HTML, `text/calendar; method=REQUEST`). `mail-parser` classe
  la partie calendrier en **pièce jointe** (`MimeType::TextOther`,
  `is_inline = false`) : Wind affiche le corps HTML et une **puce
  inerte nommée `piece-jointe.calendar`** (le repli de nom,
  `convert.rs:558-561`) — c'est le symptôme exact du retour terrain.
- **Le paramètre `method=REQUEST` est perdu au parsing** :
  `part_mime` (`convert.rs:454-466`) ne garde que `type/sous-type`.
  La table `attachments` porte `name, mime, size` — le mime stocké
  `text/calendar` suffit à **détecter** une invitation a posteriori.
- **Le MIME brut n'est jamais stocké** (`bodies.html` = HTML extrait
  seul, commentaire store.rs:98-100) : retrouver le VCALENDAR d'un
  message déjà synchronisé exige un **re-fetch IMAP complet** — chemin
  déjà rodé et assumé par `fetch_attachment` (lib.rs:670-693, ~192 ms
  mesurés).
- **Cas limite réel** : un message dont la racine EST `text/calendar`
  (sans partie HTML) fait échouer `extract_html` → rien n'est stocké,
  l'UI répond « message introuvable sur le serveur », et le message
  reste éternellement candidat au rattrapage (`scanned` jamais posé).
- Aucune crate iCalendar au dépôt (vérifié sur `Cargo.lock` : ni
  `ical`, ni `calendar`, ni `rrule`, ni `chrono-tz`). La seule
  occurrence calendrier du dépôt est `mime_for_name` :
  `"ics" => "text/calendar"` (`commands.rs:3902`), côté envoi.

### 2. Le chemin d'envoi — réutilisable, avec deux extensions

- La chaîne journal → vidange → SMTP (ADR 0003, règles d'or) est
  entière : `compose()` → `enqueue_outbox_full` → `flush_outbox` →
  `SmtpMailer::send` (lettre 0.11). **Aucun envoi hors composeur
  n'existe** — `compose()` n'a qu'un seul appelant de production
  (`queue_send`, commands.rs:2857). Une réponse d'invitation sera le
  premier : même journal, même vidange, mêmes règles d'or.
- **Rien dans `outbox` ne peut porter un corps iCalendar** : colonnes
  texte/HTML/pièces seulement. Le `multipart/alternative` est figé à
  deux parties (`corps_alternatif`, mail-smtp/lib.rs:254,
  `alternative_plain_html` de lettre). Il faut **une colonne
  `outbox.ics_reply`** et un assemblage `MultiPart` explicite avec
  `ContentType::parse("text/calendar; method=REPLY; charset=utf-8")` —
  le motif existe déjà (`file_part`, lib.rs:264 ; en-têtes X-Priority,
  lib.rs:151-184).
- **L'écho « Envoyés » est gratuit** : `echo_envoi` (echo.rs:174) lit
  tout depuis la ligne `outbox` au passage à `sent` — rien d'autre à
  alimenter.
- L'organisateur ne se déduit d'aucune structure existante
  (`Envelope` : rien de calendaire) — il vient du VCALENDAR parsé.

### 3. L'UI de lecture — les patrons existent tous

- Le point d'insertion est `Fil.svelte` `.contenu` (l. 233) : pièces
  jointes (242-275) → garde d'images (276-284) → iframe (285-287).
  **Le modèle de bloc inséré entre entête et corps existe** :
  `.garde-images` (fond `--panel`, bord `--border`, rayon 6 px).
- Boutons d'action : patron `.actions-message` (Fil.svelte:298-308,
  30 px, `--surface`/`--border`, principal en `--accent`).
- Voile A70/A75 : transposable puce → carte (Onboarding.svelte).
- Statut porteur de sens : patron A74 (`role="img"` + `aria-label`),
  bascules dites par `aria-pressed`.
- Journal du Système : dernier numéro **A75** — ce chantier prendra
  **A76**. Glyphes : 76 au sous-ensemble (`?v=76`), procédure d'ajout
  rodée (README des icônes, 6 obligations, preuve `apercu.html`).
- Coutures e2e : motif établi (`lib/`, `!== undefined`, dernier
  maillon) ; le décor e2e est hors ligne par construction — un envoi
  de réponse y reste `queued`, observable sans couture réseau.

### 4. Le point dur : parser l'iCalendar (fuseaux, TZID Windows)

RFC 5545/5546 : dépliage de lignes, échappements, paramètres,
`DTSTART;TZID=…` en heure locale + `VTIMEZONE` embarqué. **Google émet
des TZID IANA (`Europe/Paris`), Outlook/Exchange des TZID Windows
(`Romance Standard Time`)** — la résolution en instant UTC est le vrai
risque. Départagé en set-based (§ Options).

---

## Options — set-based, verdicts chiffrés

Deux spikes jetables (worktrees d'agents, 2026-08-22 : `spike-ics-calcard`,
`spike-ics-maison`), corpus commun de 6 fixtures (Google avec
VTIMEZONE IANA, Outlook avec TZID Windows « Romance Standard Time »,
UTC nu, journée entière, CANCEL, récurrence) + épreuve de génération
d'un `METHOD:REPLY` (pliage 75 octets, CRLF, re-parse). Mesures sur
arm64, release.

| Critère | A — `calcard` 0.3.11 (Stalwart) | B — parseur maison + `chrono-tz` |
|---|---|---|
| Justesse sur corpus | **71/71 champs PASS** | **81/81 vérifications PASS** |
| Génération REPLY | 5/5 PASS (writer natif, plie à 75) | PASS (pliage UTF-8-sûr écrit main) |
| TZID Windows | table complète **embarquée** | table maison 24 entrées (~140 CLDR = ~+120 lignes) |
| VTIMEZONE embarqué | non interprété — TZID inconnu = flottant SANS erreur (à détecter par `resolve()`) | non interprété — repli flottant + drapeau `tz_non_resolue` implémenté |
| Poids binaire | **+1,73 Mio** | **+1,36 Mo** (le poste commun est la base chrono-tz ; filtrée Europe : +0,40 Mo mais TZID hors filtre perdus — écarté) |
| Dépendances nettes | 23 transitives (dont `mail-builder` neuve ; mail-parser déjà là) | **4** (chrono-tz, phf ×2, siphasher) |
| Coût de possession | **~120-150 lignes** de glue | 361 lignes au spike → **~600-700 industrialisées** + maintenance des cas limites (un bug réel commis et corrigé pendant le spike : pile BEGIN/END) |
| Temps de parsing | 2-8 µs | 2,6-6,7 µs |

**Verdict** : justesse et vitesse à égalité parfaite sur le corpus. Le
poids est comparable (+1,7 vs +1,4 Mo — la base de fuseaux domine les
deux ; budget installeur < 15 Mo : tenu large dans les deux cas). Ce
qui les sépare : **le coût de possession** (150 lignes de glue contre
600-700 lignes possédées à vie sur un format à pièges — le spike B en
a payé un pendant sa propre écriture) et la table Windows complète
déjà embarquée côté calcard. La règle §2.3 (« l'alternative doit
battre l'hypothèse nettement ») joue : **B ne bat pas A** —
**recommandation : A, `calcard` en `default-features = false`**, avec
la garde explicite sur `resolve()` = None (heure flottante affichée
telle quelle + mention « heure locale de l'organisateur », jamais une
conversion mensongère). — décision D1.

---

## Périmètre

### Ce qu'on livre

1. **La carte d'invitation** dans la lecture, entre l'entête et le
   corps : titre, date/heure en **heure locale du poste**, lieu,
   organisateur, statut ; états : à répondre / répondue (« Vous avez
   accepté/refusé/répondu provisoire ») / annulée (`CANCEL`) /
   réponse d'un tiers (`REPLY` reçu : « X a accepté »).
2. **Les trois gestes** Accepter / Provisoire / Refuser → email iTIP
   `METHOD:REPLY` à l'organisateur par la boîte d'envoi (journal
   d'abord, écho Envoyés, hors ligne = part au prochain lancement,
   sémantique D1 de PLAN-RETOURS-6 déjà dite au produit).
3. **Adoption de l'existant** (invariant §6.7) : les invitations déjà
   synchronisées (base de 256 k messages) affichent leur carte à
   l'ouverture — re-fetch à la demande + écriture en cache, aucune
   migration de masse.
4. **Le cas C corrigé** : un message dont la racine est `text/calendar`
   ne tombe plus en « introuvable ».

### Refus de périmètre explicites (§2.6)

- **Pas de vue calendrier, pas de synchro CalDAV/Graph, pas de
  création d'évènement** — c'est le « non » du concept paper, maintenu.
  Microsoft exigerait Graph (ADR 0006 le refuse), Google des scopes
  OAuth nouveaux sur le chemin critique CASA. À réévaluer sur frictions
  observées EN bêta, par spikes dédiés.
- **Pas d'expansion des récurrences** (RRULE) : la carte dit « se
  répète » et montre la première occurrence. L'expansion est le gouffre
  du domaine ; rien ne la justifie pour répondre à une invitation.
- **Pas d'état croisé entre messages** : chaque message montre SA
  carte (son `SEQUENCE`). Pas de réconciliation « dernière version de
  la réunion » inter-messages — comportement Gmail de base, à observer
  en bêta avant d'en faire plus.
- **Pas de contre-proposition** (`COUNTER`), pas de délégation, pas de
  transfert d'invitation traité spécialement.
- **Pas de rappel/notification** d'évènement — il n'y a pas de
  calendrier.

---

## Architecture de la tranche

```
réception : mail-parser → convert.rs (partie text/calendar → ICS brut)
            → mail-ical::parse (pur, décision) → table invitations
            (cache écrit à save_body ; à la demande + write-back pour
            l'existant, motif fetch_attachment)
lecture   : invitation_view (locale d'abord, repli re-fetch hors du
            chemin mesuré) → carte dans Fil.svelte
réponse   : repondre_invitation (commande neuve, hors_pompe)
            → mail-ical::reply (pur) → compose() + outbox.ics_reply
            → flush → mail-smtp (multipart explicite method=REPLY)
            → écho Envoyés existant ; statut écrit dans invitations
```

- **`mail-ical`** : crate nouvelle du workspace, pure (zéro I/O),
  parseur + générateur REPLY — la décision testable séparée de l'I/O
  (motif §4).
- **Table `invitations`** (mail-core) : clé `(mailbox_id, uid)`,
  colonnes method, event_uid, sequence, summary, location,
  organisateur, début/fin epoch UTC (NULL si TZID irrésolu → heure
  flottante affichée telle quelle), all_day, recurring, partstat,
  reponse, reponse_epoch. Migration de schéma rembobinable (ADR 0012).
- **Sujet de la réponse** : « Accepté : {titre} » dans la langue de
  l'UI (D5) ; corps texte d'une ligne ; destinataire = organisateur
  seul ; le compte émetteur = le compte qui a reçu l'invitation.

## Revue à regard neuf (2026-08-22) — 8 angles, 11 trouvailles retenues

40+ candidats, dédoublonnés et vérifiés sur pièces. **Corrigées dans la
session** : (1) **désalignement d'index des pièces sur base héritée**
(confirmé par 4 angles — cliquer une pièce servait le MAUVAIS fichier
en silence) → réparation one-shot `pieces-calendrier` (motif
`reparations`, comme `corps-fffd`) : corps et pièces des messages à
partie calendrier sont relus, indices ET carte naissent du re-scan —
ce qui a permis de SUPPRIMER tout le dispositif d'adoption (commande
`invitation_view`, marqueur négatif `methode='aucune'`, sniff UI par
nom de fichier, ~192 ms de re-fetch) : la carte voyage désormais AVEC
le corps (`BodyView.invitation`) ; (2) `reset_mailbox`/`remove_local`
purgent `invitations` (et `attachments`, trou préexistant corrigé au
passage) — une carte périmée sur UID recyclé aurait envoyé un REPLY
pour la mauvaise réunion ; (3) `peut_repondre` exige d'être INVITÉ
(partstat lu) — un `.ics` transféré n'offre plus Accepter ; (4) le
répondant d'un REPLY n'est jamais l'organisateur écho d'Exchange ;
(5) la garde D1 s'évalue PAR EXTRÉMITÉ (un couple début-résolu /
fin-flottante ne se compacte plus en plage mensongère) ; (6) email
iTIP + réponse en UNE transaction (`enqueue_reponse_invitation`) —
ligne disparue = rien ne part ; (7) garde d'octets avant le parse
calendrier (le 3e parse MIME coûtait ~60 s de CPU sur un rattrapage
de 200 k) ; (8) prédicat calendrier UNIFIÉ (le trou `application/ics`
sans nom) ; (9) lectures SQL par NOM de colonne ; helpers d'adresse et
grammaire de dates DÉDUPLIQUÉS (`Store::account_email`,
`quand.dateAbsolue`), CSS des boutons fusionné ; (10) le décor e2e
traverse le VRAI parseur (ICS semé en UTC, horaire local asserté) ;
(11) **ADR 0024** écrit, STANDARD §4/§5/§10 amendés. **Assumées** :
DEBT **D-29** (cas C : corps vide définitif — recherche/transfert),
**D-30** (invitation héritée sans ligne de pièce calendrier : pas de
carte avant relecture fortuite), **D-31** (`drafts` sans `ics_reply`,
chemin inatteignable).

## Étapes — toutes livrées le 2026-08-22

- **E1 — `mail-ical` ✓** (D1) : parseur + générateur REPLY, TDD (RED
  14 échecs montré → GREEN), corpus des spikes versé en tests — **16
  tests** (dont l'écho Exchange du répondant, ajouté en revue).
- **E2 — extraction à la réception ✓** : `FetchedBody.ics`, table
  `invitations` écrite dans la transaction du corps
  (`save_body_full`), cas C corrigé (racine calendrier → affichable),
  partie calendrier inline JAMAIS listée en pièce (D3 à la source).
- **E3 — l'existant ✓** (remanié en revue) : le dispositif
  d'adoption à la demande initialement écrit est REMPLACÉ par la
  réparation one-shot `pieces-calendrier` (motif `reparations`) —
  corps et pièces des messages à partie calendrier relus par le
  rattrapage, indices ET carte du même re-scan ; test sur base
  fichier.
- **E4 — la carte ✓** : quatre états, AUCUN glyphe neuf (tuile de
  date typographique, statuts en toutes lettres), catalogue fr/en,
  Système **A76** ; la carte voyage avec le corps
  (`BodyView.invitation`).
- **E5 — la réponse ✓** : `repondre_invitation` →
  `enqueue_reponse_invitation` (email iTIP + réponse en UNE
  transaction), `outbox.ics_reply`, partie
  `text/calendar; method=REPLY` en alternative (structure MIME
  assertée), écho Envoyés gratuit.
- **E6 — revue à regard neuf ✓** : 8 angles, 11 trouvailles retenues,
  10 corrigées + 1 assumée (§ Revue). Gate complète : § ci-dessous.

Chiffres : tests Rust workspace **545** (mail-core 358 → 370,
mail-imap 65 → 70, mail-smtp 24 → 26, mail-ical **16** neufs) ; e2e
117 → **120** (`refonte-invitations.spec.js`, le décor traverse le
vrai parseur).

## Terrain — verdict du 2026-08-22, constats corrigés le 2026-08-23

Neuf points joués sur les vrais comptes : **sept OK du premier coup**
(réparation + adoption, invitation Google, acceptation vue chez
l'organisateur, TZID Windows d'Outlook juste, changement d'avis,
organisateur/REPLY reçu, budgets tenus). Deux constats (R6, R8), deux
évolutions demandées (R10, R11) et un bug débusqué (R12) — corrigés
dans la session :

- **R8 (verdict CE, renverse la garde de la revue)** : une invitation
  TRANSFÉRÉE est une invitation — qui la transfère en prend la
  responsabilité. `peut_repondre` n'exige plus le PARTSTAT.
- **R6** : l'annulation arrivait dans une conversation neuve, sans
  lien lisible. Réglé par le **lien croisé** : colonne
  `invitations.annule`, posée par l'écrivain unique
  (`ecrire_invitation`) dans les deux ordres d'arrivée — la carte
  d'ORIGINE dit « Invitation annulée » et n'offre plus de réponse.
  (Le regroupement de fils reste aux en-têtes RFC 5322 — ADR 0008
  n'est pas rouvert.)
- **R10** : répondre DEPUIS la liste — le rang de puces porte les
  trois gestes (puces-boutons 24 px, gabarit h2 du fenêtrage
  inchangé : `aPuces` est le prédicat unique), même chemin
  transactionnel que la carte.
- **R11** : une invitation répondue prête son VISAGE à la ligne
  (sujet, expéditeur, aperçu de l'invitation + puce de réponse) — le
  seul cas où la liste ne montre pas le dernier message du fil ;
  l'ordre de tri ne bouge pas.
- **R12 (bug générique débusqué)** : la puce « n fichiers » comptait
  la seule TÊTE du fil — elle somme désormais le fil entier.

Mécanique commune R10-R12 : `Store::enrichir_lignes`, une passe bornée
à la PAGE servie (deux requêtes indexées par page + un regard par
ligne isolée) — la requête chaude de la liste ne paie rien (leçon
PLAN-DEFILEMENT-PROFOND). e2e : la spec invitations passe de 3 à 4
tests (gestes depuis la liste, puce après rechargement).

**Seconde passe (verdict du 2026-08-23, six points — 1/2/4/5/6 OK,
quatre retours de finition corrigés dans la session)** :

- **R3'a — l'optimisme** : la puce de réponse remplace les gestes À
  L'INSTANT du clic (liste ET carte) ; le journal suit ; un échec rend
  l'état d'avant et le dit (toast).
- **R3'c — deux rangs** : les gestes d'invitation occupent un rang à
  eux, les autres puces vivent dessous et remontent avec la puce de
  réponse. Le fenêtrage compte des RANGS (0/1/2) — coût marginal
  constant (`extraPuce`), toujours deux gabarits mesurés, la
  correction d'A44 généralisée.
- **R3'b/R7/R9 — icônes et couleurs** : `check_circle` (accepté —
  sens « confirmé » élargi, A3 tenu), **2 glyphes neufs** `cancel`
  (refusé — jamais `close`, qui garde « fermer ») et `question_mark`
  (provisoire — jamais `hourglass_empty`, qui garde « programmé ») ;
  sous-ensemble 76 → **78**, `?v=78`, **preuve 79/79 rejouée** ; la
  couleur dit le sens PAR L'ICÔNE (accent / neutre / alerte), le texte
  double toujours (A8), paires de contraste déjà gatées ; « Annulée »
  typographique en alerte.
**Troisième passe (2026-08-23, quatre points — 2/3/4 OK)** : la puce
« optimiste » n'apparaissait à l'instant que si la ligne était DÉJÀ
sélectionnée — la mutation écrivait dans les pages NON réactives du
fenêtrage, et seule une invalidation venue d'ailleurs (la sélection,
une sonde) redessinait. Racine corrigée : le geste bump `version` — le
canal d'invalidation maison de la fenêtre — à l'écriture optimiste ET
au rembobinage d'erreur.
**Quatrième passe (2026-08-23, un point — 1 OK)** : puce instantanée
sans sélection préalable constatée au terrain. Gate rejouée
intégralement verte après le correctif : 547/0 Rust, 121/121 e2e,
contrastes 3 052, cohérence 476, garde-thread 77, clippy 0.
Terrain soldé.

- **R8' — « Supprimer » par message** : quitte la barre du fil pour la
  barre de CHAQUE message (le pendant destructif de la barre de
  réponse de RETOURS-3) — on supprime CE message, le fil reste ouvert
  s'il lui en reste (`retirerMessage`), l'écran 03 ne retourne à la
  boîte que si le fil se ferme, l'écho dit l'attente comme avant.
  Toast dédié « Message supprimé. ».

## § Décisions CE — tranchées le 2026-08-22 (STOP 1, GO)

Réponses du Chef Ingénieur, mot pour mot :

- **D1 → « calcard »** — crate Stalwart 0.3.11, `default-features =
  false` ; garde explicite sur TZID inconnu (heure flottante dite,
  jamais convertie à faux).
- **D2 → « REQUEST+CANCEL+REPLY »** — invitation à répondre, réunion
  annulée, et « X a accepté/refusé » quand on est l'organisateur.
- **D3 → « Masquer l'inline »** — la partie calendrier sans nom
  disparaît des puces quand sa carte est rendue ; un fichier `.ics`
  nommé, lui, reste enregistrable.
- **D4 → « Trois boutons neutres »** — aucun accent dans la carte,
  A14 reste intact, la carte ne hiérarchise pas la réponse.
  (La recommandation « Accepter en accent » est REJETÉE.)
- **D5 → « Langue de l'UI »** — « Accepté : … » / « Provisoire : … » /
  « Refusé : … » via le catalogue fr/en.
- **D6 → « Autorisé »** — on peut changer de réponse (nouvel email
  iTIP) ; l'état affiché suit la dernière réponse envoyée ou en file.
- **D7 → « 0.7.0 seule »** — MINEUR, livrée dès terrain validé et CI
  verte ; premier auto-update x64 constatable au passage.

## § Décisions CE (énoncés d'origine)

- **D1 — le parseur iCalendar** : crate `calcard` (A) ou parseur
  maison + `chrono-tz` (B) ? *Verdicts chiffrés des spikes au tableau
  § Options ; recommandation portée à la présentation.*
- **D2 — périmètre des méthodes affichées** : REQUEST seul, ou
  REQUEST + CANCEL + REPLY reçu ? *Recommandation : les trois — le
  parseur est le même, le coût marginal est deux états de carte.*
- **D3 — la puce `.ics`** : masquer la puce de pièce jointe de la
  partie calendrier quand la carte est affichée (elle fait doublon et
  s'appelle « piece-jointe.calendar »), ou la conserver ?
  *Recommandation : masquer la partie inline sans nom ; conserver un
  vrai fichier `.ics` joint nommé.*
- **D4 — le bouton Accepter** : en accent (geste attendu, convention
  du domaine) — dérogation à « Répondre reste le seul bouton accent »
  (A14, qui vaut pour la barre d'actions du message) — ou trois
  boutons neutres ? *Recommandation : Accepter en accent, amendé au
  Système.*
- **D5 — la langue du sujet de réponse** : « Accepté : … » dans la
  langue de l'UI (convention Gmail/Outlook localisés), ou anglais
  fixe ? *Recommandation : langue de l'UI.*
- **D6 — changer de réponse après envoi** : autorisé (la carte reste
  actionnable, nouvelle REPLY — comportement Gmail), ou figé ?
  *Recommandation : autorisé.*
- **D7 — la release** : capacité nouvelle visible → **MINEUR, 0.7.0**
  (§2.9). Livrer seule ou grouper ? *Recommandation : seule — c'est le
  déverrou de la bêta.*
