# PLAN-AUDIT-V2 — vague 2 de l'audit du 2026-09-01 : perf et robustesse mesurables (S2)

> Ouvert le 2026-09-02, à la suite de
> [AUDIT-2026-09-01.md](AUDIT-2026-09-01.md) §5 vague 2, la vague 1
> ([PLAN-AUDIT-V1](PLAN-AUDIT-V1.md)) soldée le même jour. Dix lots
> S2 à l'audit ; **chaque constat re-vérifié sur le code d'aujourd'hui
> (`b96878b`)** par cinq reconnaissances Sonnet, lot par lot — plusieurs
> constats sont déjà soldés par la vague 1, deux sont des non-défauts,
> un remède de l'audit est réfuté (voir « Ce que la vague 1 a déjà
> réglé » et « Refus »).
>
> Principe directeur (CE) : **la chose la plus simple et la plus sûre
> qui fonctionne.** Chaque lot porte SA mesure avant/après ; une
> mesure qui ne bouge pas = le remède ne rentre pas (§2.3).

## Ce que la vague 1 a déjà réglé (ne pas redoubler)

- `db_path` en `OnceLock` (E5) — l'audit 2.1 le demandait.
- Index `pending_actions(mailbox_id, uid)` (E3) — l'audit 2.2.
- `References` complet à l'envoi (E7) ; `thread::refresh` dédoublonné
  par fil dans `remove_absent` et les retraits du Non (revue) — mais
  **pas** dans `nettoyage_verdict` (voir C4).
- `save_attachment` : l'écriture disque est sous `hors_pompe` (E5) ;
  reste la question du chemin (C8).
- Le test « bascule Windows » **restaure déjà** le réglage dans un
  `finally` (`refonte-ecran02.spec.js:1057-1060`) — l'audit disait le
  contraire ; seul un kill du runner le contourne.
- `launch.mjs` ne porte plus d'hygiène `localStorage` : les cinq
  specs la font chacune (C9).

## Constat (instruction sur pièces, 2026-09-02)

### C1 — Ouverture : chaque `Store::open` rejoue schéma + ~20 `table_xinfo` + migrations
`store.rs:760-786, 913-952` : `busy_timeout`, `PRAGMA journal_mode`,
`execute_batch(SCHEMA)`, `migrate()` (≈ 20 `add_missing_columns`,
chacun un `PRAGMA table_xinfo`), puis le contrôle d'adoption des fils
(`thread.rs:551` a SA porte `user_version`, mais `orphans()` est
rejoué avant). **103 sites** `Store::open` dans le shell (102
`commands.rs` + 1 `veilleur.rs`). Aucune porte rapide. Deux
ouvertures de trop par relève : `arrival_notification_problem`
(`commands.rs:1632`) rouvre alors que l'appelant tient `&mut Store`
(`:803`) ; `passe_legere_compte` ouvre deux fois (`:891`, `:902`).
`SyncSummary.total` (`:65`) coûte un `unified_count()` par cycle
(`solder_releve`, `:4025-4027`) pour un champ que l'UI ne lit jamais
(`App.svelte:237-240` ne lit que `accounts/errors/fetched/deleted`).
Instrument existant : `examples/diagnostic_ouverture.rs` (trois
chronos : SQLite brut, `Store::open` complet, SECOND `open` = le coût
payé à chaque commande). Chiffre de l'audit : 2 740 ms à froid au
jalon.

### C2 — Indexation : ré-indexation systématique, quatre copies, COUNT par frappe
`store.rs:1809-1826` : dans `upsert_envelopes`, CHAQUE enveloppe du
lot relit le HTML du corps et `index_message` (= `deindex` + INSERT),
nouvelle ou non — 500 deltas CONDSTORE = 500 corps relus et
re-tokenisés sous le verrou d'écriture. `search.rs:610-638`
`indexable_text` : `to_ascii_lowercase` (copie 1), `out` (2),
`decode_entities` (3), `split_whitespace().collect().join()` (4-5) —
cinq allocations pleine taille par corps. `search.rs:295`
`search_capped` appelle `search_total` **inconditionnellement** ; sa
doc `:412-416` dit « seulement si plafonné ». Nuance vérifiée : la
**soupape tri-date** (`WIDE_QUERY_THRESHOLD`, A50) décide sur ce
total AVANT la page — le COUNT n'est donc pas gratuit à retirer,
il se BORNE (voir E2). `preview_catchup` (`:2360-2398`) charge
jusqu'à 500 corps HTML complets par appel (`App.svelte:505`).

### C3 — IMAP : en-têtes entiers, trois parses par corps, lots sans borne d'octets
`mail-imap lib.rs:640-659` : `BODY.PEEK[HEADER]` entier pour trois
champs ; le commentaire dit que la crate n'expose pas `HEADER.FIELDS`
— **faux aujourd'hui** : `imap-proto 0.16.7 body.rs:20-31` range
`HEADER.FIELDS (…)` dans `MessageSection::Header`, donc
`fetch.header()` rend les champs demandés — **une chaîne à changer**.
`convert.rs:382, 555/567, 600` : `extract_html`,
`extract_attachments`, `extract_ics` ouvrent chacun leur
`MessageParser` sur les mêmes octets (`lib.rs:978-992`) ; la garde
d'octets de `extract_ics` (« le troisième du chemin ») dit le coût.
`fetch_bodies_html` (`:708-726`) : un `UID FETCH` groupé, **aucun
`RFC822.SIZE`** — 50 messages entiers en RAM sans borne.
`changes_since` (`:682-687`) : `UID FETCH 1:* (UID ENVELOPE INTERNALDATE
FLAGS) (CHANGEDSINCE n)` — après un long hors-ligne, toutes les
enveloppes changées en une réponse. **UIDPLUS jamais vérifié** :
`expunge_uid` (`:600-608`) envoie `UID EXPUNGE` sans garde (3
`supports_*` existent : move, condstore, list_status). **Cinq
`LIST "" "*"`** par session (`:349, 375, 402, 439, 781`, caches
séparés) et trois `CAPABILITY` (`:567, 580, 594`). `decode_header`
(`convert.rs:638-651`) : un `MessageParser` par sujet. `Reply-To`
et `Sender` jamais lus (`convert.rs:206-223`, `Envelope` sans
`reply_to`, `envelope.rs:14-41`) : « Répondre » part vers `From`.

### C4 — Nettoyage/Portier : listes non bornées, `refresh` par message, périmètre recalculé
`store.rs:3466-3472` `nettoyage_groupes`, `:3243-3257`
`portier_attente`, `:3615` `nettoyage_messages`, `:2858`
`pile_mis_de_cote`, `:3218` `routages` : sans LIMIT. Mesure due
« vraie base 200 k » toujours absente — aucun banc ne couvre ces
requêtes (`banc_page_liste`, `banc_recherche`, `banc_migration_fils`
prennent une base en argument ; aucune base 200 k n'existe sur le
poste : `C:\mesure\clarity.db` 340 ko, les bases du gate 3 ont
disparu du Temp). `nettoyage_verdict` (`:3589-3595`) appelle
`remove_local` par message dans une transaction déjà ouverte → branche
`else` de `remove_local` (`:1997-2010`) = `thread::refresh` +
`deindex` **par message** (le `BTreeSet` de `remove_absent`
`:1974-1984` n'est pas repris). `boites_du_perimetre` (`:3314-3349`) :
`prepare` par compte, rejoué à 4 sites d'une même session
(`:3435, 3458, 3510, 3611`).

### C5 — Synchro : initiale non reprenable, `[Gmail]` en dur, adresse prise pour un Message-ID
`sync.rs:158-181` : `initial_sync` redemande `list_uids` entier et
re-fetch TOUS les lots, sans retirer les UID déjà en base ;
`update_state` (qui pose `initialisee = 1`) n'arrive qu'après
(`:147-148`) : une coupure au lot k rejoue tout. `nav.rs:166-174`
`canonical_folders` `.unwrap_or(None)` : base verrouillée = « pas
d'Envoyés ». `nav.rs:114-124` `feuille_canonique` : `[Gmail]` en dur ;
`Folder` (`remote.rs:104-113`) n'a ni délimiteur ni SPECIAL-USE alors
que `mail-imap` calcule `SpecialUse` à la volée. `thread.rs:111-113`
`is_message_id` = « contient `@` et pas d'espace » : `<alice@x.fr>`
dans `In-Reply-To` passe. `echo.rs:352-362` : `balayer_echos` retire
tout écho `origin_action_id IS NULL` — l'écho d'ENVOI (`echo_envoi`,
`:179`) n'a pas d'action d'origine ; à confirmer par le test
(`echo_envoi` puis `balayer_echos` sans relève d'Envoyés ⇒ survit ?).
**Non-défaut** : `References: None` (`backfill.rs:285`) — `mail-imap`
rend TOUJOURS `Some` (`convert.rs:306`, test
`l_absence_de_references_se_distingue_de_l_absence_de_lecture`) ; la
branche `None` est morte par construction.

### C6 — Gestes de masse : N × k IPC séquentiels
`Liste.svelte:899-909` → `App.svelte:1588-1648` `groupe()` : par
conversation cochée, un `thread_messages` (si `thread_size > 1`) puis
**un `appel(geste)` par message**, tout `await` en série ; 50
conversations × 4 messages = 250 + 50 IPC, chacun sous le verrou
global, chacun sa transaction. Aucune commande plurielle dans
`commands.rs` ; `geste_avec_echo` (`echo.rs:79-85`) est unitaire.
Le transport n'a pas de file (`transport.js:53-67`) : la série vient
de la boucle UI. Chaîne après UN geste (`archiver`, `App.svelte:
1452-1476`) : `rechargerVues()` (Liste + Kiosque + Registre) +
`nav_snapshot` + `sync_apres_geste` (+ resservie si bilan > 0).

### C7 — Boîte d'envoi : poison sans borne, octets des pièces chargés pour compter
`outbox.rs:409-418, 549-553` : `attempts += 1` puis `break` — jamais
relu, aucun seuil (le seuil existe pour `pending_actions`,
`store.rs:2094-2096`). `outbox()`, `outbox_to_send()`,
`outbox_in_state()` (`:331-362`) chargent les **octets** de chaque
pièce (`load_outbox_attachments`, `:367-388`) ; `flush_outbox`
(`commands.rs:4510-4517`) les charge pour `.is_empty()` par compte à
chaque cycle, `outbox_status` (`:4585, 4613`) pour `.len()` toutes
les 10 s.

### C8 — Défense en profondeur : chemins libres, CSP incomplète, tests d'assainissement absents
`commands.rs:2442-2469` `save_attachment(dest: String)` écrit où l'UI
dit (le dialogue vit côté webview, `transport.js:95`, capability
`dialog:allow-save`) ; `attach_files` (`:5187-5269`) lit tout chemin
reçu. CSP (`tauri.conf.json:20-22`) : `default-src 'self'; connect-src
ipc: http://ipc.localhost; img-src 'self' data: https: http:;
style-src 'self' 'unsafe-inline'` — sans `object-src`, `base-uri`,
`frame-ancestors`. `mail-render/sanitize.rs` : 12 tests, **aucun**
nommé sur `<svg onload>`, `srcset`, `<meta http-equiv=refresh>`,
`<base>` (`javascript:` et `data:` en `href` sont couverts). **Remède
de l'audit réfuté** : `withGlobalTauri: false` ne retire que
`window.__TAURI__` ; Tauri 2 injecte `window.__TAURI_INTERNALS__.invoke`
dans toute fenêtre (`tauri-2.11.5/scripts/core.js`, injecté par
`manager/webview.rs:396`) — un script hostile dans le document
principal appellerait `invoke` quand même. La vraie frontière est la
CSP (`default-src 'self'`, pas d'`unsafe-inline` script : aucun
`<script>` ni gestionnaire inline ne s'exécute) + le sandbox S1 des
iframes ; c'est elle qu'on complète.

### C9 — Outillage : actions sur tags mobiles, flakes non comptés, copies
`ci.yml` : 4 actions × 3 tags mobiles (`checkout@v5`,
`rust-toolchain@1.97.1`, `rust-cache@v2`, `setup-node@v5`), pas de
`dependabot.yml`. `playwright.config.js` : `retries: 1`, pas de
`failOnFlakyTests`, pas de `globalSetup` — `cargo build --examples`
se joue dans le `beforeAll` de la première spec qui sème, sous le
timeout de 180 s (`launch.mjs:50-54`). **Aucun registre de flakes** :
`.last-run.json` ne porte que `status` ; le chiffre que la décision
CE n°5 de l'audit demandait n'existe pas (PLAN-KAIZEN W1 : 0 flaky
sur 121 ; dernier run local : passé). Assertions nues après hover/drag
: `refonte-retours-7.spec.js:60, 66`, `refonte-volets.spec.js:
222-246` (10), `selection-multiple.spec.js:131, 139-140`. Hygiène
`localStorage` en 5 copies de spec. `demarrage.spec.js:95` `continue`
sur sonde absente : 8 renommages = test vert vide. `pre-push` et
`gate.ps1` : mêmes 8 commandes recopiées, divergence `[n/8]`/`[n/9]`
(D-32). `verifier-release.ps1` : aucune crypto (`minisign` absent du
PATH du poste). Aucun `node --check`. 11 `waitForTimeout` = 8,4 s.

## Périmètre — refus explicites (§2.6)

- **Front (lot 2.5 : corps réessayable, Kiosque fusion + fenêtrage,
  `vivant`, garde `rattraperApercus`, sondes coalescées, rafales) et
  `Menu.svelte` unique, focus des Réglages, raccourcis depuis
  l'iframe** → chantier séparé **PLAN-AUDIT-V2-FRONT** avec STOP
  visuel (D1) : huit menus validés au STOP visuel de leurs chantiers,
  un composant partagé les retouche tous.
- **Connexion SQLite partagée dans `AppState`** (audit §4 patron 4) →
  vague 3, sauf si la mesure d'E1 ne suffit pas (STOP mesuré).
- **`withGlobalTauri: false`** → réfuté (C8).
- **`References: None`** → non-défaut (C5).
- **Bornes UI (pagination) sur `nettoyage_groupes`/`portier_attente`**
  → seulement si la mesure d'E4 dépasse le budget page (100 ms).
- **Pile TLS unique, ordonnanceur côté shell, images au
  « Transférer », type du client OAuth Google** → décisions CE de
  l'audit hors vague (vague 3 / front / hors chantier).
- **`HEADER.FIELDS` pour `decode_header`** : non — c'est un parse
  local par sujet ; remplacé par un décodeur d'en-tête direct
  (`mail_parser::decoders`) seulement si la mesure E3 le montre.

## Mesures — instruments et décors

- **Décor 200 k** : `seed_inbox C:\mesure\banc200k.db 200000 …`
  (une fois, plusieurs minutes, HORS dépôt), corps sur les 500 plus
  récents ; c'est le banc de E1, E2, E4, E6.
- **Base réelle** (251 k, 13,3 Go) : au STOP 2 seulement, par le CE,
  via `diagnostic_ouverture` (durées seules) et `wind.log` (E4 y
  écrit ses durées).
- **Corps lourd** : `banc_indexation` (exemple neuf, 40 lignes) —
  corps HTML synthétique de 28 Mo, chrono + pic mémoire par
  allocateur compteur (E2).
- **IPC** : `__e2eJournal` (selection-multiple.spec) pour E6.
- **Réseau** : `FakeServer` + compteur d'octets/commandes pour E3
  (nombre de `LIST`, `CAPABILITY`, taille des FETCH).

## Étapes

Chaque étape : mesure AVANT, RED montré, GREEN, mesure APRÈS, boucle
intérieure sur ses seuls tests. **Gates complètes : après E3, après
E6, avant le commit final.** Commits : E1, E2, E3, E4+E5, E6+E7,
E8+E9, revue.

### E1 — Ouverture (lot 2.1) — STOP mesuré précoce
- Mesure AVANT : `diagnostic_ouverture` sur `banc200k.db` (3 chronos)
  ; le CE sur la base réelle.
- `Store::init_with` : **porte rapide par processus** — un registre
  `OnceLock<Mutex<HashSet<PathBuf>>>` des chemins dont l'initialisation
  complète a RÉUSSI (schéma + migrations + adoption des fils soldée) ;
  toute ouverture suivante du même chemin ne fait que `busy_timeout` +
  `journal_mode`. Zéro changement de schéma, zéro colonne ; la
  mono-instance (E1 vague 1) garantit qu'aucun autre processus ne
  migre la base entre-temps ; les bases mémoire (tests) ne sont
  jamais inscrites. Le `pending_adoption` (ADR 0012) reste une sonde
  sans `Store::open`, inchangé.
- `SyncSummary.total` supprimé (et son `unified_count` par cycle) ;
  `arrival_notification_problem(store: &Store, …)` ;
  `passe_legere_compte` en une ouverture.
- **RED** : `une_seconde_ouverture_du_meme_chemin_ne_rejoue_pas_les_migrations`
  (compteur de `table_xinfo` via `sqlite3_trace`, ou plus simple :
  un `add_missing_columns` espion) ; `une_base_memoire_n_est_jamais_
  inscrite`. Mesure APRÈS : second `open` < 5 ms sur 200 k. **STOP
  mesuré** : si le second `open` reste > 20 ms sur la base réelle, la
  connexion partagée remonte de la vague 3 (décision CE sur le
  chiffre).

### E2 — Indexation (lot 2.2)
- `upsert_envelopes` : n'indexer que si nouvelle enveloppe, ou si
  sujet/expéditeur/destinataires ont changé (comparaison sur la ligne
  relue — elle l'est déjà pour l'état lu/non-lu) ; le corps est indexé
  par `save_body`, pas par la relève.
- `indexable_text` en UNE passe : balayage sur les octets
  (minuscule comparée à la volée, pas de `shadow`), entités décodées
  en écrivant dans `out`, blancs repliés en écrivant — une allocation.
- `search_capped` : `search_total` **borné** à
  `WIDE_QUERY_THRESHOLD + 1` (`COUNT(*) FROM (… LIMIT ?)`) pour
  décider la soupape ; le total exact seulement si `rows == limit`
  (« 100 sur N »).
- `preview_catchup` : sous-lots de 100 corps en interne (l'API et
  l'UI gardent `limit: 500`) — RAM bornée à ~5 Mo au lieu de ~28.
- **RED** : `une_enveloppe_resynchronisee_sans_changement_n_est_pas_
  reindexee` (compteur d'`index_message`), `indexable_text_ne_copie_
  qu_une_fois` (allocateur compteur), `le_total_n_est_exact_que_si_
  la_page_est_pleine`. Mesures : `banc_indexation` 28 Mo (pic RAM,
  ms), `banc_recherche` préfixe 3 lettres (< 50 ms sur 200 k).

### E3 — IMAP (lot 2.3) — gate complète après
- `fetch_thread_headers` : `BODY.PEEK[HEADER.FIELDS (MESSAGE-ID
  IN-REPLY-TO REFERENCES)]` ; commentaire corrigé.
- `convert::analyser(raw) -> Analyse { html, attachments, ics }` : UN
  `MessageParser` ; `extract_*` deviennent des vues (les 57 tests de
  `convert.rs` restent).
- `fetch_bodies_html` : `UID FETCH (UID RFC822.SIZE)` d'abord, puis
  sous-lots ≤ 32 Mo (un message > 32 Mo voyage seul).
- `changes_since` : `(UID FLAGS) (CHANGEDSINCE n)` d'abord (léger),
  puis `ENVELOPE INTERNALDATE` par lots de 200 pour les UID inconnus
  localement — les UID connus ne reçoivent que leurs drapeaux.
- `supports_uidplus()` sur le patron des trois autres ; sans UIDPLUS :
  `COPY` + `+FLAGS \Deleted` + `EXPUNGE` (RFC 3501) — jamais
  `UID EXPUNGE` à l'aveugle.
- Un `LIST "" "*"` par session (`liste_dossiers()` mémoïsée, les
  quatre `*_folder` la lisent) ; un `CAPABILITY` (`capabilites()`
  mémoïsée, les `supports_*` la lisent).
- **RED** : sur un `FakeImap` de test (listener local, patron E6 de
  la vague 1) : `les_en_tetes_de_fil_ne_demandent_que_trois_champs`,
  `un_lot_de_corps_est_borne_a_32_mo`, `un_serveur_sans_uidplus_
  n_envoie_jamais_uid_expunge`, `une_session_ne_liste_qu_une_fois`,
  `une_session_n_interroge_capability_qu_une_fois` ; unitaire :
  `analyser_parse_une_fois` (compteur). Mesure : octets/commande sur
  200 en-têtes (avant ≈ 200 × en-tête entier, après ≈ 200 × 3
  lignes) ; CPU du rattrapage de 50 corps (avant/après).

### E4 — Nettoyage/Portier (lot 2.4)
- Mesure AVANT sur `banc200k.db` : `nettoyage_groupes`,
  `portier_attente`, `nettoyage_messages`, `pile_mis_de_cote`,
  `routages` (exemple `banc_nettoyage`, durées seules). Le shell
  trace « nettoyage : N groupes en X ms » dans `wind.log` (le CE lit
  la vraie base au STOP 2 — la mesure due depuis HORIZON-NETTOYAGE).
- `nettoyage_verdict` : retraits dédoublonnés par fil (patron
  `remove_absent`, `purger_message` sans refresh puis UN `refresh`
  par fil touché) ; `deindex` inchangé (par message, c'est sa
  granularité).
- `boites_du_perimetre` : calculé UNE fois par appel de commande et
  passé aux quatre lecteurs (paramètre, pas de cache d'état).
- Bornes : **seulement si la mesure dépasse 100 ms** sur 200 k —
  alors LIMIT + « Voir plus » (le Registre en a le patron), sinon
  refus dit.
- **RED** : `un_verdict_sur_un_groupe_de_n_messages_rafraichit_
  chaque_fil_une_fois` (compteur), `le_perimetre_n_est_resolu_qu_une_
  fois_par_verdict`.

### E5 — Synchro et domaine (lot 2.8)
- `initial_sync` : `uids` moins `store.uids_connus(mailbox_id)` avant
  `chunks` ; `update_state` au fil des lots (`last_uid` seulement,
  `initialisee` à la fin) — une coupure au lot k reprend au lot k.
- `canonical_folders` : `.optional()?`.
- `Folder { special_use: Option<SpecialUse> }` porté depuis les
  attributs LIST (RFC 6154) ; colonne `mailboxes.special_use` ;
  `feuille_canonique` lit d'abord la colonne, `[Gmail]` reste le
  repli nommé.
- `thread::canonical_ids` rejette un jeton égal à une adresse de
  l'enveloppe (expéditeur, destinataires) — ADR 0008 dans l'esprit.
- `Envelope.reply_to` + colonne + `reply_context`/`reply_all_context`
  visent `Reply-To` s'il existe (`Sender` reste ignoré : sans usage
  produit).
- Écho d'envoi : test d'abord ; si l'écho est balayé, `echos.
  origin_outbox_id` et le balayage exige « Envoyés relevé après
  l'envoi ».
- **RED** : `une_initiale_coupee_au_lot_2_reprend_au_lot_2` (FakeServer
  échouant au 2e `fetch_envelopes`, compteur de lots au 2e appel),
  `une_base_verrouillee_est_une_erreur_pas_une_absence_d_envoyes`,
  `google_mail_uk_a_ses_archives` (LIST `\All` sous `[Google Mail]`),
  `une_adresse_entre_chevrons_n_est_pas_un_message_id`,
  `repondre_vise_reply_to`, `l_echo_d_envoi_survit_au_balayage_sans_
  releve_des_envoyes`.

### E6 — Gestes de masse (lot 2.6) — gate complète après
- `Store::gestes_groupe(cibles: &[Cible], action, destination) ->
  Bilan { faits, echecs }` en UNE transaction (les fils multi-messages
  développés côté cœur — `thread_messages` n'est plus demandé par
  l'UI) ; commande `agir_groupe` sous `hors_pompe` ; règle de
  réussite selon D6.
- `App.svelte::groupe()` : un `appel('agir_groupe', …)` ; les
  resservies restent (une seule chaîne au lieu de N).
- **RED** : `cinquante_conversations_archivees_en_une_transaction`
  (mail-core, échec injecté au 30e ⇒ 0 ou 50 selon D6) ; e2e
  `selection-multiple.spec.js` : `__e2eJournal` compte **1** IPC de
  geste pour 3 conversations cochées (avant : ≥ 3).

### E7 — Boîte d'envoi (lot 2.7)
- `flush_outbox` : `Transient` avec `attempts + 1 >= SEUIL_ENVOI`
  (D5) ⇒ `record_rejection("N tentatives : <motif>")` — l'état
  `rejected` existe, la fente le montre déjà ; sinon `break` comme
  aujourd'hui.
- `outbox_pending_count(account_id)` pour `.is_empty()` ;
  `outbox_status` lit `COUNT(*)` des pièces, pas leurs octets ; les
  octets ne sont chargés que par `outbox_to_send` au `flush`.
- **RED** : `cinq_echecs_transitoires_refusent_le_message_et_liberent_
  la_file`, `le_statut_ne_charge_aucun_octet_de_piece` (compteur SQL
  ou colonne espionne). Mesure : RSS avec 3 pièces de 25 Mo en file
  pendant 60 s de sondes (avant : +75 Mo × 6 / min).

### E8 — Défense en profondeur (lot 2.9, part technique)
- CSP : `+ object-src 'none'; base-uri 'self'; frame-ancestors
  'none'; form-action 'none'`.
- `save_attachment` : `dest` absolu, sans `..`, parent existant,
  extension conservée du nom assaini ; `attach_files` : chemins
  absolus, fichiers réguliers. Le dialogue reste côté webview (sa
  capability est déjà la plus étroite).
- `mail-render` : tests RED nommés `<svg onload>`, `srcset` distant
  sous `BlockRemote`, `<meta http-equiv=refresh>`, `<base href>`,
  `style="background:url()"` avec échappement CSS (déjà documenté
  comme passant : on le laisse dit, CSP en garde-fou).
- **RED** : les tests ci-dessus + `un_chemin_relatif_est_refuse`.

### E9 — Outillage (lot 2.10)
- Actions par SHA + `dependabot.yml` (`github-actions`, hebdo).
- `playwright.config.js` : rapporteur `json` → `test-results/
  rapport.json` ; `gate.ps1` imprime « flaky : N » au verdict (D4) ;
  `globalSetup` = `cargo build -p mail-core --examples` (hors du
  timeout de spec).
- `expect.poll` aux 13 assertions nues après hover/drag ;
  hygiène `localStorage` en UNE liste dans `launch.mjs` (les 5 specs
  perdent leur copie) ; `demarrage.spec.js` : plancher de présence
  (≥ 5 sondes sur 8, sinon rouge) ; `globalTeardown` restaure
  `AppsUseLightTheme` depuis une valeur écrite AVANT la bascule (D7).
- `verifier-release.ps1` : `minisign -Vm` si `minisign` est au PATH,
  sinon ligne « NON PROUVÉ : minisign absent » (jamais PASS).
- `gate.ps1` : `node --check` sur `e2e/*.mjs` et `scripts/*.mjs`
  (0,2 s) ; `pre-push` = raccourci docs-seuls puis `gate.ps1`
  (D-32 soldée).
- Preuve : gate reproduite 3 fois de suite, « flaky : 0 » ×3.

## Livraison

- **E1 — livrée le 2026-09-02.** Décor `C:\mesure\banc200k.db` semé
  (`seed_inbox`, 200 000 enveloppes, 500 corps de 34 ko, 18,8 s).
  Mesure AVANT (`diagnostic_ouverture`) : ouverture brute 1,7 ms,
  premier `Store::open` 38 ms, **second `Store::open` 36 ms** — le coût
  payé par CHAQUE commande. Livré : registre de processus des chemins
  dont l'initialisation complète a RÉUSSI (`registre_initialisees`,
  inscrit APRÈS le commit de l'adoption et le rattrapage des
  correspondants ; base mémoire jamais inscrite), porte rapide dans
  `init_with` = `busy_timeout` + `journal_mode` seuls. **APRÈS : second
  `open` 0,9 ms (×40).** `SyncSummary.total` et son `unified_count()`
  par cycle supprimés ; `arrival_notification_problem` lit la connexion
  de l'appelant ; `passe_legere_compte` ouvre une fois. TDD : RED
  `une_seconde_ouverture_du_meme_chemin_ne_rejoue_pas_le_schema` (index
  retiré derrière le dos du Store, recréé = schéma rejoué), GREEN ; sept
  tests qui REMBOBINENT une base à la main entre deux ouvertures (décor
  d'une base d'avant, interdit en production par la mono-instance)
  appellent `Store::oublier_les_initialisations()` — tout le registre,
  jamais un chemin (un autre test paie au pire une initialisation de
  plus). mail-core 433 → 434, clippy propre, garde 110/0. STOP mesuré :
  le chiffre rend la connexion partagée (vague 3) sans objet pour
  l'ouverture ; à confirmer sur la base réelle au STOP 2.

- **E2 — livrée le 2026-09-02.** Instrument neuf `banc_indexation`
  (corps HTML synthétique de N Mo, `save_body` chronométré ; pic mémoire
  lu de l'extérieur par `PeakWorkingSet64`, le workspace interdisant
  `unsafe`). AVANT, corps de 28 Mo : **401 ms, pic 210 Mo** (socle
  8 Mo). Livré : `indexable_text` en UNE passe et UNE allocation (balises
  reconnues sans ombre minuscule, entités décodées et blancs repliés en
  écrivant) — **APRÈS : 338 ms, pic 133 Mo** (−77 Mo ; le reste est
  SQLite : liaison du corps, tokenisation FTS5). `upsert_envelopes` ne
  ré-indexe une enveloppe relue que si sujet, expéditeur, adresse,
  destinataires ou copies ont changé (la même lecture qui décidait
  `nouveau`) — RED `une_enveloppe_resynchronisee_sans_changement_garde_
  son_docid` (témoin : un second message derrière, sinon SQLite réutilise
  le dernier rowid et masque la ré-indexation — première version du test
  verte par accident), GREEN. `preview_catchup` par sous-lots de 100
  (même contrat, RAM ÷ 5 ; pas de RED possible, même comportement).
  **Remède retiré par la mesure** : le COUNT par frappe vaut **1,5 ms sur
  57** pour « fac » sur 200 k (borné au seuil : 0,5 ms — 1 ms gagnée) ;
  le coût est la page triée par date, pas le comptage. La borne ne rentre
  pas (§2.3) ; `banc_recherche` gagne la section « comptage seul » et
  le commentaire contradictoire de `search_total` est corrigé.
  mail-core 434 → 435, clippy propre.

- **E3 — livrée le 2026-09-02.** Instrument neuf : **un faux serveur
  IMAP scripté** (`faux_serveur.rs`, `#[cfg(test)]`, en clair sur
  127.0.0.1 — un `Script` : capacités, lignes `LIST`, répondeur de
  `UID FETCH` ; il enregistre chaque commande reçue) et un constructeur
  `ImapServer::pour_test` — les deux constructeurs de production
  partagent désormais `nouveau()` (les 9 champs recopiés de l'audit
  §3.2 disparaissent). **Six tests joués RED contre l'adaptateur
  d'AVANT** (stash de `lib.rs`/`convert.rs`, constructeur de test
  greffé) puis GREEN : `les_en_tetes_de_fil_ne_demandent_que_trois_
  champs` (`BODY.PEEK[HEADER.FIELDS (MESSAGE-ID IN-REPLY-TO
  REFERENCES)]` — l'ancien commentaire disait la crate incapable :
  `imap-proto` range `HEADER.FIELDS` dans `MessageSection::Header`,
  une chaîne suffisait), `un_lot_de_corps_est_borne_a_32_mo`
  (`RFC822.SIZE` d'abord, `lots_bornes` pur : 20 Mo + 20 Mo + 1 ko ⇒
  `1` puis `2:3`), `un_serveur_sans_uidplus_n_envoie_jamais_uid_expunge`
  (`supports_uidplus`, repli `EXPUNGE` RFC 3501),
  `une_session_ne_liste_qu_une_fois_pour_les_dossiers_speciaux`
  (`Speciaux` : corbeille, brouillons, envoyés, stratégie d'archive par
  UNE `LIST` — cinq avant), `une_session_n_interroge_capability_qu_une_
  fois` (`capacites` mémorisée, quatre `supports_*` la lisent — trois
  `CAPABILITY` avant), `les_changements_sont_demandes_en_drapeaux_puis_
  en_enveloppes_par_lots` (`(UID FLAGS) (CHANGEDSINCE n)` puis
  `ENVELOPE` par lots de 500 : 501 changements ⇒ `1:500` + `501`).
  `convert::analyser(raw)` : UNE analyse MIME par corps (html, pièces,
  ics depuis le même `Message` ; `extract_*` deviennent des vues de
  test) — banc `banc_analyse_de_50_corps` (`#[ignore]`, 50 corps
  multipart de 51 ko) : **18,2 ms → 11,1 ms** (−39 %). Refus dit :
  `decode_header` inchangé (un parse local par sujet, non mesuré comme
  coût). mail-imap 72 → 78, clippy propre. `Reply-To` va à E5.
  **Gate complète VERTE** (5,4 min ; e2e 186/187 + 1 flaky :
  `refonte-retours-7.spec.js:46`, le survol d'une pièce jointe — l'une
  des assertions nues après hover que l'audit nomme, corrigée à E9).
  Commit `e0ce62d` (E1-E3 : un commit par gate, pas par étape — la
  gate se paie 5 min).

- **E4 — livrée le 2026-09-02.** Instruments : `seed_inbox` gagne un
  7e argument (nombre d'expéditeurs distincts — le décor d'origine n'en
  a que huit, le GROUP BY du Nettoyage y est gratuit) ;
  `C:\mesure\banc5000.db` (200 k, 5 000 expéditeurs, 48 s) ;
  `banc_nettoyage` (durées seules, MUTE la base : mode organisé, session,
  attente du Portier peuplée à la main, verdict sur le plus gros groupe).
  **AVANT** : `nettoyage_groupes` 380-430 ms, `nettoyage_demarrer`
  320-350 ms, `nettoyage_messages` 105-116 ms, `portier_attente` 50 ms,
  verdict de 40 messages **35 à 580 ms d'une passe à l'autre**.
  Diagnostic (python sur copie, `EXPLAIN QUERY PLAN`) : l'agrégat
  passait par l'index de DATE puis un B-tree temporaire ; les deux
  `NOT EXISTS` corrélés coûtaient un tiers ; un index COUVRANT
  (expéditeur, date, boîte) rend l'agrégat sans lire une ligne de table
  (661 → 111 ms en python, ×6 ; comptage 644 → 68). La variance du
  verdict est la fusion de segments FTS5 à la suppression (23 ms les
  40 `DELETE` sur l'index, mais l'automerge tombe quand il tombe) — pas
  un défaut de Wind, borné sous la seconde, dit ici. Livré, **deux RED
  francs puis GREEN** : `idx_envelopes_sender` étendu à
  `(sender_norm, date_epoch, mailbox_id)` avec reconstruction sur base
  héritée (`reconstruire_index_si_ancien`, helper qui factorise aussi
  l'index de date — le bloc de 40 lignes existait en une copie, il en
  aurait eu deux) ; `nettoyage_critere` en `NOT IN` non corrélé ;
  `nettoyage_groupes_sql` en deux phases (agrégat couvert, puis objet et
  nom par le même index, `GROUP BY` externe contre l'égalité de date) ;
  `nettoyage_compter_groupes` et `nettoyage_messages_sql` par le même
  index (**`INDEXED BY` obligatoire : le SQLite embarqué 3.50 préférait
  l'index de date là où celui de python choisissait bien** — 116 ms
  contre 0,2 ; leçon STANDARD §9, un test de PLAN d'exécution garde les
  trois requêtes, prouvé en le cassant) ; retraits du verdict
  dédoublonnés par fil (patron `remove_absent`) ; `prepare_cached` hors
  de la boucle de `boites_du_perimetre` ; le shell trace « nettoyage :
  N groupes en X ms » dans `wind.log` (la mesure due depuis
  HORIZON-NETTOYAGE, lisible sur la vraie base au STOP 2). **APRÈS** :
  `nettoyage_groupes` **67-78 ms**, `nettoyage_demarrer` **46-74 ms**,
  `nettoyage_messages` **1 ms**, verdict 28-150 ms ; première ouverture
  d'une base héritée : +0,72 s une fois (reconstruction de l'index sur
  200 k — précédent D9 : 1,77 s sans écran). Bornes UI (pagination) :
  refusées, la mesure tient le budget. Au passage, l'oubli du registre
  d'E1 devient PAR CHEMIN (`oublier_initialisation(&path)`, clé de
  SQLite) : vider tout le registre faisait rejouer le schéma sous les
  pieds du test d'E1 qui tourne en parallèle. mail-core 435 → 437.

- **E5 — livrée le 2026-09-02.** Six RED (trois d'exécution, trois de
  compilation) puis GREEN. (1) **Initiale reprenable** : `initial_sync`
  retire les UID déjà en base avant le découpage (`Store::uids_connus`,
  partagé avec `remove_absent`) ; `FakeServer.panne_au_lot_envelopes`
  coupe au n-ième lot — `une_initiale_coupee_au_lot_2_reprend_au_lot_2`
  : avant `[[6,5],[4,3],[2,1]]` rejoués, après `[[4,3],[2,1]]`.
  (2) `canonical_folders` : `.optional()?` — une base illisible REMONTE
  (`une_base_illisible_est_une_erreur_pas_une_absence_d_envoyes`, table
  `accounts` renommée). (3) **SPECIAL-USE porté par `Folder`** :
  `SpecialUse { All, Archive, Drafts, Junk, Sent, Trash }` dans le cœur,
  `folders.special_use` (colonne, migration), `name_to_folder` le lit
  des attributs LIST, `canonical_folders` préfère le rôle annoncé et
  garde le nom en repli (`google_mail_uk_a_ses_archives` : « [Google
  Mail]/All Mail », Spam, Bin résolus). (4) **Une adresse entre chevrons
  n'est pas un Message-ID** : `linking_ids`/`attach` reçoivent les
  adresses de l'enveloppe (expéditeur, À, Cc) et les rejettent ; à la
  synchro comme au rattrapage des en-têtes (le contexte de
  `set_thread_headers` relit les adresses) ; l'adoption d'une base
  héritée passe `&[]` (limite dite). (5) **`Reply-To`** :
  `Envelope.reply_to` (colonne `envelopes.reply_to`, lu de l'ENVELOPE
  par l'adaptateur), `Store::reply_to_de` à la demande (jamais dans les
  lignes de liste — `SELECT_UNIFIED` a des index positionnels), la
  décision pure `reply_to(is_own, sender, to, reply_to)` le préfère à
  `From` sauf sur son propre message ; « Répondre » ET « Répondre à
  tous » le suivent. (6) **Écho d'envoi** : le test l'a PROUVÉ balayé
  dès la première passe (« copie attendue en envoyes jamais vue ») ;
  remède : `mailboxes.relevee_epoch` posé par `update_state`, et un écho
  sans action d'origine n'est balayé que si les Envoyés ont été relevés
  APRÈS lui — un compte sans dossier d'envois annoncé le garde (seule
  trace du message parti). Refus dit : `References: None` (non-défaut,
  C5). Piège d'outillage payé cher : un script de remplacement par
  sous-chaîne (`reply_to: None,` est DANS `in_reply_to: None,`) et un
  motif `Nom {` qui attrape aussi le `-> Nom {` d'une signature — deux
  passes de réparation guidées par le compilateur (E0063). mail-core
  437 → 443, mail-imap 78 → 79, shell compilé, clippy propre.

- **E6 — livrée le 2026-09-02.** `Store::agir_groupe(cibles, geste)` :
  les fils développés côté cœur (`thread_messages`, dédoublonnés), le
  dossier indésirable de chaque compte résolu AVANT la transaction
  (refus franc sans écriture), puis UNE transaction pour tout le lot —
  `geste_avec_echo` scindé en enveloppe + `geste_sous(tx, …)` pour y
  enchaîner N messages. RED de compilation puis GREEN :
  `cinquante_conversations_archivees_en_une_transaction` (déclencheur
  `RAISE(ABORT)` au 30e retrait ⇒ 50 enveloppes intactes, 0 action, 0
  écho ; sans panne ⇒ 50/50/50). Shell : commande `agir_groupe(cibles,
  action)` sous `hors_pompe` (garde 111/0), `CibleArg` en camelCase.
  UI : `groupe()` fait UN appel ; `messagesDe` et les six commandes
  unitaires du lot disparaissent ; tout ou rien = un refus laisse le lot
  intact, le toast le dit (`erreur.groupePartiel`, `spamImpossible`
  gardé). e2e `selection-multiple` : le journal `__e2eJournal` compte
  **1 `agir_groupe`, 0 `archive_message`, 0 `thread_messages`** — joué
  RED avant le changement d'UI, GREEN après (9/9, 25 s). Mesure : 50
  conversations × 4 messages, IPC 300 → 1 par construction.

## Gate & terrain

- Boucle intérieure : `cargo test -p <crate> <nom>` par étape ; specs
  e2e impactées en fichier entier (E6 : `selection-multiple`, E9 :
  les cinq specs touchées).
- Gates complètes : après E3, après E6, finale ; `/code-review high`
  sur l'ensemble avant le dernier commit.
- STOP 2 : checklist remise au CE avec les commandes (`terrain.ps1`,
  `lancer-wind.ps1`, `diagnostic_ouverture` sur la base réelle,
  `wind.log` du Nettoyage, sonde 60 s).

## § Décisions CE — tranchées au STOP 1, le 2026-09-02

**GO du CE le 2026-09-02** (les huit décisions tranchées).
Réponses mot pour mot :
- **D1** : « Tout en un chantier » — le front ENTRE (E10, E11 ci-dessous,
  avec STOP visuel précoce).
- **D2** : « 0.16.0 avant, V2 = 0.17.0 (Recommandé) ».
- **D3** : « Dette explicite + trace (Recommandé) ».
- **D4** : « Garder retries:1 + compter (Recommandé) ».
- **D5** : « 5 (Recommandé) ».
- **D6** : « Tout ou rien (Recommandé) ».
- **D7** : « En gate + globalTeardown (Recommandé) ».
- **D8** : « BlockRemote + substitution à l'envoi (Recommandé) ».

### Conséquence de D1 — deux étapes front s'ajoutent

#### E10 — Front, réactivité (lot 2.5 + D8) — STOP visuel après le premier rendu
- `lib/fil.svelte.js` : un échec IPC remet `fil.corps[k]` à
  `undefined` (`delete`) et le cadre montre « Réessayer » (une ligne
  de la fente du fil, glyphe `error`) ; `chargerMessage` se rejoue au
  clic.
- `Kiosque.svelte` : `recharger()` **fusionne par clé** (`cle(m)`),
  `lu` figé au premier service ; fenêtrage ±N cartes autour de la
  vue (le `{#if estRepliee}` existe : hors fenêtre = repliée sans
  iframe ni `ResizeObserver`).
- `Liste.svelte` : drapeau `vivant` posé par le cleanup d'un
  `$effect` ; `.finally` n'appelle `pomper()` que vivant.
- `App.svelte` : garde de progrès sur `rattraperApercus` (patron
  `corpsEnCours`) ; sondes `nav_snapshot`/`sync_progress`/
  `outbox_status`/`list_drafts` coalescées en UNE commande
  `etat_ui` (10 s ; 5 → 1 IPC par 10 s au repos) ; resservies après
  un geste coalescées (50 ms, une chaîne au lieu de trois).
- **D8** : `forward_context` assainit en `BlockRemote` ; à l'envoi,
  `queue_send` substitue les vraies URL depuis le corps en base
  (`bodies.html`) pour le message transféré — le destinataire reçoit
  le même message ; `Composition.svelte:456` n'affiche plus d'image
  distante.
- **RED** : node (`fil.svelte.js`) : `un_echec_ipc_laisse_le_corps_
  rechargeable` ; e2e : `__e2eJournal` après un « e » (nombre d'IPC),
  « 5 pages de Kiosque = ≤ 2N+1 iframes vivantes », « une relève
  pendant la lecture ne déplace pas la carte lue » ; Rust :
  `un_transfert_n_embarque_aucune_image_distante_a_la_composition_
  mais_les_rend_a_l_envoi`.

#### E11 — Front, un seul menu (clôt D-47, A8 tenu) — STOP visuel
- `Menu.svelte` unique : ancrage, `role=menu`, flèches ↑/↓, Home/End,
  Entrée, Échap, Tab ferme, clic dehors, focus posé sur le premier
  item à l'ouverture et RENDU au déclencheur à la fermeture ; les
  huit menus (`TriSection`, `Registre`, `Reglages`, `Liste`,
  `Kiosque`, `Portier`, `Nettoyage`, `Fil`) le consomment ; classes
  communes en `systeme.css` (ombre, z-index, `min-width` : une
  copie, jeton `--ombre` déclaré).
- Réglages : focus posé à l'ouverture (patron `Retour.svelte:34`).
- Raccourcis depuis l'iframe du corps : `brancherLiens` re-dispatche
  `keydown` (touches nues : `e`, `Suppr`, `/`, `Échap`, `j`/`k`) vers
  le document parent.
- **STOP visuel** au premier menu porté (Liste), avant les sept
  autres. Système : journal A-n, anatomie du menu unique.
- **RED** : e2e « un menu se parcourt au clavier et rend le focus »
  (Liste, Kiosque, Portier), « Réglages ouvre sur son premier
  contrôle », « `e` depuis le corps archive ».

Ordre révisé et gates : E1 → E2 → E3 (gate) → E4 → E5 → E6 (gate) →
E7 → E8 → E9 → E10 (STOP visuel) → E11 (STOP visuel) → gate finale.
Commits : E1, E2, E3, E4+E5, E6+E7, E8+E9, E10, E11, revue.

### Énoncé des décisions telles que posées

- **D1 — Périmètre** : (a) cette vague = cœur, adaptateurs, gestes de
  masse, envoi, défense technique, outillage (E1-E9) ; le **front**
  (lot 2.5, `Menu.svelte` unique, focus Réglages, raccourcis iframe)
  en chantier séparé avec STOP visuel (recommandé) ; (b) tout en un
  seul chantier.
- **D2 — Véhicule de release** : (a) **0.16.0 part AVANT la vague 2**
  (vagues 0+1, CHANGELOG écrite ; les testeurs sont sur 0.15.0 avec
  les défauts S1) et la vague 2 fait **0.17.0** (recommandé) ;
  (b) 0.16.0 attend la vague 2.
- **D3 — CONDSTORE absent** (drapeaux jamais resynchronisés) : (a)
  dette explicite + une ligne `wind.log` « compte N sans CONDSTORE »
  à la connexion, pour savoir si le cas existe en bêta (recommandé :
  Gmail, Microsoft 365 et Dovecot l'annoncent) ; (b) fenêtre
  `FETCH FLAGS` des 500 derniers UID à chaque cycle.
- **D4 — Flakes** : (a) garder `retries: 1` sans `failOnFlakyTests`
  (PLAN-KAIZEN E3) et **compter** : rapporteur JSON + « flaky : N » au
  verdict de gate — le chiffre que l'audit demandait n'existe pas
  aujourd'hui (recommandé) ; (b) `failOnFlakyTests: true` dès
  maintenant (un flake = rouge).
- **D5 — Seuil d'abandon d'un envoi** : nombre d'échecs transitoires
  consécutifs avant « refusé » : **5** (proposé, comme
  `SEUIL_QUARANTAINE`) — ou 10.
- **D6 — Geste de masse** : (a) **tout ou rien** — une transaction,
  un échec annule le lot et le dit (recommandé : l'utilisateur voit
  soit tout fait, soit rien ; le cas est rare) ; (b) meilleur effort,
  bilan « N faits, M échecs ».
- **D7 — Épreuve « bascule Windows »** : (a) garder en gate avec
  restauration en `globalTeardown` (recommandé : le filet A42 reste
  joué) ; (b) hors gate, jouée au terrain seulement.
