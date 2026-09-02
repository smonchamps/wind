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
  **Gate complète VERTE** (3,8 min, 187/187, 0 flaky), commit `595ac6e`
  (E4-E6).

- **E7 — livrée le 2026-09-02.** `SEUIL_ENVOI = 5` (D5) : au cinquième
  échec transitoire consécutif, `flush_outbox` REFUSE le message (motif
  « 5 tentatives : … », l'utilisateur tranchera — l'état `rejected` et
  sa ligne dans la fente existaient) et CONTINUE avec le suivant ; avant,
  `attempts` se comptait sans jamais se lire. `outbox_pending_count`
  (un COUNT) remplace la relecture de toute la file, octets des pièces
  compris, que `flush_outbox` faisait par compte à chaque cycle pour un
  `.is_empty()` ; `outbox_metadonnees()` (pièces sans octets, `NULL` en
  colonne) sert `outbox_status` toutes les 10 s — le chemin de lecture
  reste unique (`charger_pieces(avec_octets)`). RED de compilation puis
  GREEN : `cinq_echecs_transitoires_refusent_le_message_et_liberent_la_
  file` (« a » refusé au 5e cycle, « b » a eu son tour : `attempts` 1),
  `le_statut_ne_charge_aucun_octet_de_piece`. mail-core 444 → 446.

- **E8 — livrée le 2026-09-02.** CSP + `object-src 'none'; base-uri
  'self'; form-action 'none'` (`withGlobalTauri: false` refusé, C8 :
  `__TAURI_INTERNALS__` reste injecté quoi qu'il arrive — la CSP est la
  frontière). `chemin_de_sortie(dest)` pure : absolu, sans `..`, nom de
  fichier, dossier existant — `save_attachment` l'exige ; `attach_files`
  n'accepte qu'un chemin absolu vers un fichier régulier. RED de
  compilation puis GREEN : `un_chemin_relatif_ou_a_remontee_est_refuse`
  (desktop 31 → 32). `mail-render` : quatre filets NOMMÉS — `<svg
  onload>`, `srcset` distant sous `BlockRemote`, `<meta http-equiv=
  refresh>`, `<base href>` — **verts d'emblée** (l'allowlist ammonia
  tenait déjà) : ce n'est pas un RED, ce sont les noms de la seconde
  frontière (mail-render 22 → 26) ; l'échappement CSS qui passe le
  filtre naïf reste documenté tel quel, CSP en garde-fou.

- **E9 — livrée le 2026-09-02.** CI : les quatre actions épinglées par
  SHA (`gh api repos/…/commits/<tag>`), `dependabot.yml` hebdomadaire
  `github-actions`. Playwright : rapporteur JSON + `e2e/flaky.mjs`, et
  `gate.ps1` imprime « flaky : N » (avec les noms) au verdict — le
  chiffre que D4 attendait existe désormais ; `globalSetup` compile les
  exemples hors du timeout de spec ; `globalTeardown` restaure
  `AppsUseLightTheme` depuis un témoin que l'épreuve « suivi OS » écrit
  AVANT de basculer (D7 — le `finally` existait, un kill du runner le
  sautait). `expect.poll` sur les 14 assertions nues après hover/drag
  (retours-7 ×3, volets ×9, sélection ×3 — dont le flaky de la gate
  E1-E3). `demarrage.spec` : plancher de présence (≥ 5 sondes sur 8,
  sinon rouge — le `continue` rendait le filet vide sur renommage).
  `launch.mjs` : `CLES_LOCALES` + `purgerLocales(page, clés)` — cinq
  specs perdent leur copie. `gate.ps1` : dix étapes, `node --check`
  sur `e2e/*.mjs` et `scripts/*.mjs` (2,3 s), paramètre
  `-DocsSeulement` ; **le hook `pre-push` délègue à `gate.ps1`** (D-32
  soldée : neuf commandes recopiées, deux divergences). `verifier-
  release.ps1` : `minisign -Vm` contre la clé publique du manifeste si
  l'outil est au PATH, sinon « NON PROUVÉ » dit (jamais PASS). Gate
  documentaire jouée : 8 s, six étapes. **Gate complète VERTE** (3,6
  min, dix étapes, `flaky : 0`), commit `2b9c03e` (E7-E9).

- **E10 — livrée le 2026-09-02** (journal **A107**). (1) **Corps
  réessayable** : un échec du cœur retire la marque `''` et pose
  `fil.erreurs[k]` ; le cadre porte la ligne d'incident (grammaire de la
  garde d'images) « Le message n’a pas pu être chargé. » + Réessayer ;
  couture e2e `window.__e2ePanne` (le prochain appel d'une commande
  échoue une fois) — test `un_corps_que_le_cœur_ne_sert_pas_se_dit_et_
  se_rejoue` (retours-7). (2) **Kiosque** : fusion par clé à la
  resservie (`lu` figé au premier service, pages 2..n conservées) et
  fenêtrage à ±12 rangs de la première carte visible (±5 depuis D9,
  terrain du 2026-09-02 ; rang DOM calculé
  sections et groupes confondus ; une carte qui sort laisse un bloc de
  la hauteur mesurée de son corps — pas de saut). (3) `Liste.svelte` :
  drapeau `vivant` baissé par le nettoyage d'un `$effect`, le `.finally`
  ne pompe plus après démontage. (4) `rattraperApercus` gardée en
  réentrance. (5) **Sonde unique `etat_ui`** (nav + synchro + envois,
  une connexion) toutes les 5 s, les trois lectures extraites en
  `lire_nav`/`lire_synchro`/`lire_envois` partagées avec les commandes
  d'origine — IPC au repos **5 → 2 par 10 s** (`etat_ui` ×2 ; les
  brouillons gardent leur sonde de 10 s : `list_drafts` reste une liste
  entière, vague 3). (6) Rafales coalescées à 50 ms : `rechargerVues`
  et le chemin de `chargerNav` qui va chercher (la sonde fournit
  l'instantané, immédiat). (7) **D8** : `forward_context` sert le bloc
  en `BlockRemote` avec un marqueur `data-wind-transfert="compte/uid/
  boîte"` (allowlisté à la frontière : un brouillon de transfert repris
  le garde) ; `queue_send` relit la source, la rend `AllowRemote` et
  `substituer_transfert` remplace tout ce qui suit le marqueur — le
  mot tapé avant reste, une retouche DANS le bloc est perdue (limite
  dite) ; source d'un autre compte : corps tel quel. RED de compilation
  puis GREEN : `un_transfert_n_embarque_aucune_image_distante_a_la_
  composition_mais_les_rend_a_l_envoi`, `le_marqueur_de_transfert_
  survit_a_la_frontiere` (RED franc). Limites dites : pas de test node
  pour `fil.svelte.js` (runes, hors node) — le filet est e2e ; la mesure
  « RAM après 5 pages Kiosque » n'est pas jouable sur le décor e2e (le
  Kiosque y a quelques cartes) — à voir au terrain ; l'e2e du retry a
  été écrit avec l'UI, sans RED joué à part. Specs jouées : retours-7,
  démarrage, mode-organisé, réception-14 : 46/46. **Gate complète
  VERTE** (3,7 min ; e2e 187/187 + 1 flaky : `refonte-retours-6.spec.js:
  42`, la signature aux Réglages — hors des fichiers touchés ; le message
  du commit dit « flaky : 0 » à tort), commit `ed42fce` (E10, A107).

- **E11 — livrée le 2026-09-02** (journal **A108**, D-47 amendée : les
  menus soldés, les jumeaux du cœur restent ; D-4 amendée). **STOP
  visuel** joué sur la Liste (captures du décor e2e : menu au clavier,
  ligne « Réessayer ») — **GO CE « OK, porter les sept autres »**.
  `Menu.svelte` : carte flottante (surface, bordure, rayon des
  contrôles, `--shadow`, z-index 30), items 32 px au survol/focus en
  `--hover`, filets et titres en une copie ; ancrage aux coordonnées
  (borné) ou sous le déclencheur (`absolu`, le fil) ; **clavier** :
  focus sur le premier item, ↑/↓ bouclent, Début/Fin, Entrée joue, Échap
  et Tab ferment, clic dehors ferme, le focus REVIENT au déclencheur.
  Les huit surfaces portées (Liste, Kiosque, Portier, Nettoyage,
  Registre, TriSection, Réglages, Fil) ; trois `<svelte:window>` et
  trois `$effect` de fermeture disparaissent, 24 règles CSS de copies
  retirées (trois ombres dont `var(--ombre)` inexistant, trois
  z-index). Réglages : focus posé à l'ouverture (`panneau.querySelector`,
  patron `Retour.svelte`). `brancherLiens` rejoue chaque `keydown` de
  l'iframe sur la fenêtre parente (même touche, mêmes modificateurs) —
  les raccourcis ne sont plus inertes après un clic dans un corps. e2e
  `menu-clavier.spec.js` (4 tests, mode organisé) : **joué RED sur le
  menu d'avant** (le focus restait sur le déclencheur), GREEN après ;
  trois pièges d'écriture payés : le « clic dehors » visait un bandeau
  qui n'existe pas en mode organisé (pendu 3 min) ; le clic d'OUVERTURE
  atteignait le nouvel écouteur « dehors » pendant sa propre propagation
  (le menu se fermait à l'instant — Kiosque/Fil sans `stopPropagation`),
  réglé par un filtre sur le déclencheur (un `setTimeout(0)` faisait
  rater la frappe suivante) ; et le cadre du corps est SANS script (S1),
  Playwright n'y évalue rien — le test « e depuis le corps » focalise
  l'iframe depuis le parent et frappe la vraie touche, dans
  `refonte-retours-7` (classique, lancement neuf : la bascule
  organisé → classique dans la même session le faisait échouer, non
  élucidé — sondé : le rejeu atteint la fenêtre une seule fois et le
  geste part sur un lancement neuf). Build ui-v2 sans avertissement,
  cohérence du Système 68 jetons ; specs des huit surfaces : 91/94 au
  premier passage (les trois ci-dessus), puis vertes.

## Revue à regard neuf (2026-09-02, `/code-review high` sur `b96878b..HEAD` + arbre)

Huit angles (Sonnet : diff ligne à ligne, comportements retirés,
traçage inter-fichiers, réutilisation, simplification, efficacité,
altitude, conventions) / ~30 candidats / vérifiés sur pièces et par
test — **un réfuté par la preuve, quatorze corrigés, le reste
consigné** :

1. **Réfuté en le prouvant** — « la porte rapide d'E1 ouvre les
   connexions SANS `PRAGMA foreign_keys` (il vit dans `SCHEMA`), les
   cascades de `delete_account` ne jouent plus » : le test écrit pour
   le prouver est resté VERT sans la ligne — rusqlite `bundled` compile
   SQLite avec `SQLITE_DEFAULT_FOREIGN_KEYS=1`. La ligne reste avant la
   porte (une ceinture qui ne dépend pas d'un drapeau de compilation) et
   le test la garde (`la_porte_rapide_garde_les_cles_etrangeres`).
2. **`agir_groupe` disait « N faits » en sautant les boîtes inconnues**
   (CONFIRMED, deux angles) — désormais un REFUS franc du lot (D6, tout
   ou rien), test étendu (cible sur « Disparue » ⇒ Err, 0 écriture).
3. **`substituer_transfert` tronquait tout après le marqueur**
   (CONFIRMED) — une conclusion tapée APRÈS le bloc transféré partait
   à la poubelle ; appariement des `<div>` (imbrications du courrier
   cité comprises), test : avant ET après conservés.
4. **Un transfert dont la source a disparu bloquait l'envoi**
   (CONFIRMED) — repli : le message part au pixel neutre, une ligne de
   `wind.log` le dit.
5. `NOT IN` gardé contre un `NULL` (`address IS NOT NULL`, `email IS
   NOT NULL` — un `NULL` vidait tout le Nettoyage) ; le marqueur se
   cherche par l'ATTRIBUT puis la balise (un `style` posé avant par
   l'éditeur ne le fait plus rater, test) ; `largeur={220}` numérique.
6. Efficacité : `messages_du_fil` (trois colonnes) remplace
   `thread_messages` dans le lot ; `sync_state` mémorisé par (compte,
   boîte) ; `a_reindexer` compare par référence (cinq clones par
   enveloppe relue) ; `outbox_avec(octets)` unique.
7. Altitude/conventions : la source du transfert est un type du cœur
   (`SourceTransfert { account_id, uid, mailbox }`, `cle()` /
   `source_du_transfert` parsent et encodent, testés — le shell ne fait
   que relire et substituer) ; `SEUIL_ENVOI` = `Store::SEUIL_QUARANTAINE`
   (deux constantes pour un seuil) ; le nom de l'index des expéditeurs
   en UNE constante (`INDEX_EXPEDITEURS`, quatre `INDEXED BY`) ; le
   bornage à la fenêtre vit dans `Menu.svelte` (sa vraie taille), les
   sept parents passent l'ancre nue (sept `Math.min` à constantes
   divergentes retirés).

Consignés sans correction (D-52) : la sonde `RFC822.SIZE` coûte un
aller-retour par lot de 50 corps pour une borne rarement atteinte
(l'alternative — stocker la taille à la relève — est un chantier) ;
`etat_ui` à 5 s double la cadence de la nav et des envois (assumé :
c'est la relève par le veilleur qui impose 5 s) ; `mesurerFenetre`
parcourt les cartes à chaque frame de défilement (200 nœuds, ~1 ms) ;
`__e2ePanne` est une cinquième couture sans `import.meta.env` (vague 3
avec les quatre autres) ; le registre de la porte rapide est clé par
CHEMIN, pas par identité de fichier — sûr sous la mono-instance, non
gardé par le code (un outil qui remplacerait le fichier dans le même
processus ouvrirait une base sans schéma).

Après revue : mail-core 447 → 448, clippy propre, specs des menus 38/38,
les trois specs jadis flaky 79/79 avec la coalescence en front montant.

### Andon de la gate finale (2026-09-02)

La gate finale après revue est sortie ROUGE à l'étape e2e :
`refonte-ecran02:1228` (transfert hors ligne, PJ-D4), échec identique
aux deux tentatives, reproduit seul (52 passés, 1 échec). Constat par
une spec jetable : au clic « Envoyer », ni toast ni erreur, la
composition reste — `queue_send` ne répond JAMAIS. Cause : le
`fin_du_bloc` posé en revue (comptage des `<div>` imbriqués) avançait
octet par octet avec `bas[i..]` sur une `str` ; le premier « é » du
corps (« Message transféré ») tombait hors frontière de caractère et
PANIQUAIT ; nue dans la tâche async de la commande, la panique laissait
l'invoke sans réponse. Le test unitaire de la revue n'avait que de
l'ASCII, et la spec du transfert n'avait pas été rejouée après cette
correction — seules les specs « flaky » l'avaient été.

Deux corrections, TDD (RED montré : « start byte index 69 is not a
char boundary; it is inside 'é' ») : `fin_du_bloc` travaille sur les
octets (les balises cherchées sont ASCII, l'index rendu reste une
frontière) ; rendu ET substitution passent sous `hors_pompe`, où
`spawn_blocking` rapporte une panique comme une erreur dite — plus
jamais un gel muet. mail-core 448 → 449. Enseignement au STANDARD §9.

## Gate & terrain

- Boucle intérieure : `cargo test -p <crate> <nom>` par étape ; specs
  e2e impactées en fichier entier (E6 : `selection-multiple`, E9 :
  les cinq specs touchées).
- Gates complètes jouées : E1-E3, E4-E6, E7-E9, E10, E11 — cinq
  vertes (3,6 à 5,4 min ; flaky 0 à 2, nommés) ; finale après revue
  ROUGE (andon, ci-dessus) ; finale après andon VERTE en 2,9 min,
  193 e2e, flaky 0 ; après la passe 1 de terrain : ROUGE au format
  (deux tests non formatés, `cargo fmt`), puis VERTE en 2,7 min,
  194 e2e + 1 banc ignoré, flaky 0 ; après D9 (fenêtre 5) : VERTE en
  2,7 min, 193 e2e, flaky 1 (`selection-multiple:174`, connu D4) ;
  après la passe 2 : ROUGE à la cohérence du Système (icône « mail »
  hors catalogue dans la sonde — « work », un repère), puis VERTE en
  3 min, 194 e2e, flaky 1 (`selection-multiple:174` — deux gates de
  suite : à instruire si une troisième).

## STOP 2 — checklist de terrain (CE)

Préparer le poste (état de la base, build release AVEC trace) :

```powershell
powershell -ExecutionPolicy Bypass -File scripts\terrain.ps1
```

```powershell
powershell -ExecutionPolicy Bypass -File scripts\lancer-wind.ps1
```

1. **E1 — ouverture, sur la VRAIE base** (durées seules, aucun contenu
   lu). Attendu : le second `Store::open` sous 5 ms (200 k : 0,9 ms) ;
   le premier porte la reconstruction de l'index des expéditeurs UNE
   fois (≈ 0,7-1 s sur 250 k), puis retombe.

```powershell
cargo run -p mail-core --example diagnostic_ouverture --release -- "$env:APPDATA\dev.elements.wind\wind.db"
```

2. **E4 — Nettoyage de printemps** : ouvrir la section, choisir
   « tout » / « dossiers et archives », poser un verdict sur le plus
   gros groupe. Attendu : la liste des groupes en moins d'une seconde
   perçue, et dans `wind.log` une ligne « nettoyage : N groupes en
   X ms » — **X < 200 ms** sur la base réelle (200 k / 5 000 : 67 ms).

```powershell
Get-Content "$env:APPDATA\dev.elements.wind\wind.log" -Tail 30
```

3. **E6 — geste de masse** : cocher 20 conversations (dont un fil de
   plusieurs messages), Archiver. Attendu : UN toast « 20 conversations
   archivées », la barre ne gèle pas, les 20 partent d'un coup (les fils
   entiers) ; annuler = rien (tout ou rien).
4. **E7 — envoi empoisonné** : hors ligne, envoyer un message, remettre
   en ligne avec un mot de passe SMTP faux (compte générique) : après
   cinq cycles (≈ 25 min) la fente dit « refusé » avec le motif, et un
   second message en file derrière est tenté. (Si pas de compte
   générique : lire la ligne de la fente sur le premier échec, c'est le
   même chemin.)
5. **E10 — Kiosque** : mode organisé, dix pages de Kiosque défilées.
   Attendu : pas de saut au retour vers le haut ; RAM privée (Gestionnaire
   des tâches, processus WebView2) — noter la valeur ; une relève
   pendant la lecture d'une carte ne la déplace pas de section.
6. **E10 — transfert** : transférer une infolettre à images distantes.
   Attendu : AUCUNE image chargée dans le composeur (pixels neutres),
   le destinataire reçoit les images ; un mot tapé avant ET après le
   bloc arrive.
7. **E10 — corps non servi** : difficile à provoquer au terrain ; le
   filet e2e couvre. Si un cadre reste vide, la ligne « Le message n’a
   pas pu être chargé. » + Réessayer doit y être.
8. **E11 — menus** : sur chaque surface (⋯ d'une rangée, carte du
   Kiosque, Portier, Nettoyage, Registre, tri, Réglages > Portier,
   « Déplacer vers… » du fil) : ouvrir, ↓ ↓ Entrée ; puis ouvrir,
   Échap — le focus revient sur le bouton ; un clic ailleurs ferme ;
   le menu ne sort jamais de la fenêtre (le tester près du bord bas).
   **Verdict d'apparence sur les huit** (un seul dessin). Réglages :
   le focus est dans le panneau à l'ouverture (Tab avance dedans).
9. **E11 — raccourcis depuis le corps** : cliquer DANS le corps d'un
   message, frapper `e` : la conversation s'archive (avant : rien).
10. **E5 — Reply-To** : répondre à un message de liste/notification
    portant `Reply-To` : le À vise l'adresse de `Reply-To`.
11. **Gel** : sonde 60 s pendant un geste de masse de 50 conversations
    et l'ouverture du Nettoyage. Attendu « OK : aucun gel > 150 ms ».
    **Jamais pendant une gate.**

```powershell
python e2e\sonde-gel.py C:\mesure\clarity.db 60
```

12. **wind.log** : aucune adresse, aucun sujet ; la ligne « sans
    CONDSTORE » ne doit PAS apparaître (Gmail, Microsoft).

Budgets à re-mesurer (STANDARD §3) : gel de la pompe 0 > 150 ms
(point 11) ; démarrage inchangé (`terrain.ps1` lit `demarrage`) ; RAM
privée < 200 Mo après le point 5.

## STOP 2 — verdict terrain du 2026-09-02 (passe 1) : 9 OK, 3 KO, 3 observations

Verdict du CE, point par point : 1 OK (second `Store::open` 0,76 ms,
premier 5,2 ms sur 249 k enveloppes) ; 2 OK (« nettoyage : 159 groupes
en 86 ms », puis 17 ms) ; 3, 4, 7, 8, 9, 11, 12 OK (sonde : « OK : aucun
gel > 150 ms sur 60 s ») ; **5 KO**, **6 KO**, **10 KO**.

**(A) `table envelopes has no column named reply_to`** — dans
`wind.log`, à CHAQUE passe du veilleur (« passe de connexion en échec »,
« passe légère en échec »), et « Répondre à tous impossible » au point
10. Cause : la colonne `reply_to` (E5) vivait dans le `CREATE TABLE`
seul, jamais dans la liste `add_missing_columns` d'`envelopes` ;
`special_use` et `relevee_epoch`, elles, y étaient. Les décors e2e,
semés à neuf, ne pouvaient pas le voir — c'est la leçon §9 « une
fonctionnalité neuve doit ADOPTER les données anciennes », récidivée.
TDD : `une_base_d_avant_la_vague_2_recoit_la_colonne_reply_to` (base
fichier, colonne retirée, réouverture) — RED sur l'erreur du terrain mot
pour mot, GREEN par la migration.

**(B) « Toujours afficher les images » sans effet au Kiosque** (point 5,
après dix pages défilées). Cause : `accorderImages` appelait le cœur
puis `charger(0)`, qui ne re-sert que la page 0 — la fusion par clé
(E10) gardait telle quelle une carte au-delà. Le décor organisé ne
pouvait pas le montrer : router un expéditeur au Kiosque pose déjà la
règle d'images (RETOURS-14), aucune carte n'y avait de garde. Décor
neuf : `seed_arrivee` accepte `corps=images` (un corps à image distante
par arrivée), `injecterArrivee({ corps: 'images' })`. Spec
`kiosque-images.spec.js` : 25 lettres routées, règle révoquée, la garde
d'une carte de page 2 — RED (garde toujours là), GREEN : la carte se
re-sert elle-même par `message_body` (le même document que le volet),
la règle d'expéditeur re-sert toutes les cartes encore gardées.

**(C) mot tapé APRÈS le bloc transféré perdu à l'envoi** (point 6 ;
avant OK, envoi OK). Cause : dans un contenteditable, le curseur posé en
fin de corps tombe DANS le dernier bloc — le bloc marqué, que
`substituer_transfert` remplace. Le bloc se termine désormais par une
ligne vide éditable (`<div><br></div>`) : RED unitaire
(`un_transfert_laisse_une_ligne_editable_apres_le_bloc`), GREEN ; filet
e2e dans `refonte-ecran02` (Ctrl+Fin, un mot tapé, hors du bloc marqué).

Observations : (i) capture du point 1 — le libellé « DÉJÀ CONSULTÉ »
chevauche une rangée (Doctolib) et un vide d'une rangée la suit ; la
Liste n'a pas changé sur ce point dans la vague (diff : Menu et drapeau
de vie seulement) — **à reproduire avec le CE** (mode, épingles, geste
qui précède) avant toute correction ; (ii) « passe geste compte 2 :
inventaire 505,9 s, total 544,7 s » — une passe de neuf minutes, sous
les échecs (A) ; hypothèse : verrous et reprises en cascade, **à
re-mesurer après (A)** ; (iii) **RAM privée 249 Mo sur 6 processus
WebView2 après dix pages de Kiosque** — budget STANDARD §3 < 200 Mo
(repos : 95,5 Mo sur 7) : **budget dépassé, ligne arrêtée sur ce
point** ; la fenêtre E10 tient 2×12+1 = 25 iframes vivantes ; mesure à
deux largeurs sur un décor à corps de 100 Ko, ci-dessous, décision CE
D9.

### Mesure RAM du Kiosque (2026-09-02) — décision CE D9

Outil : `e2e/tests/banc-ram-kiosque.spec.js` (sous `WIND_BANC_RAM=1`),
`mesure-ram.ps1 -AppPid -Profil` — corrigé le jour même : il filtrait
`dev.elements.wind` AVANT le profil et ne comptait, sur le profil e2e,
que deux processus (6 Mo, un mensonge — STANDARD §3, « un outil de
mesure se vérifie »). Décor : 200 lettres à corps synthétique de 100 Ko
(`launchAppV2({ comptes: [{ …, ko: 100 }] })`), 16 expéditeurs routés,
build DEBUG, 7 processus, working set privé sommé, pauses de 8 s.

| Fenêtre (`FENETRE`) | iframes vivantes | repos | page 1 | 160 cartes | retour Réception | + 25 s |
|---|---|---|---|---|---|---|
| 12 (livré) | 13 | 119 | 254 (+136) | 335 (+217) | 286 (+167) | — |
| 5 | 6 | 113 | 222 (+108) | 282 (+168) | 236 (+123) | — |
| 1 | 2 | 113 | 182 (+70) | 209 (+96) | 207 (+94) | 206 (+94) |

Lecture : (a) la largeur de fenêtre pèse — −54 Mo à 160 cartes entre 12
et 5, −126 entre 12 et 1 ; (b) une seule page coûte déjà +70 à +136 Mo :
une iframe `srcdoc` de 100 Ko vaut des dizaines de Mo au rendu ; (c) au
retour en Réception, 94 à 167 Mo RESTENT, stables à +25 s — ce n'est pas
le ramasse-miettes qui tarde : quelque chose retient les documents
démontés (piste : les documents des iframes retirées, ou une référence
depuis `corpsAuto`/`brancherLiens`) — **à instruire**, c'est la vraie
racine, la fenêtre n'est qu'une borne.

Sur le poste du CE (release, vraies lettres) : 249 Mo sur 6 processus
après dix pages, budget STANDARD §3 < 200 Mo (repos 95,5 Mo).

**D9 (CE)** : (1) réduire la fenêtre à 5 dès cette passe (−54 Mo sur
le décor, 11 iframes vivantes couvrent plus d'un écran) — recommandé ;
(2) la laisser à 12 ; (3) 1 (trois iframes — la carte lue et ses deux
voisines ; à vérifier au défilement rapide). Dans tous les cas, la
rétention après retour ouvre un chantier propre (ce n'est pas un réglage
de fenêtre), et le budget « RAM < 200 Mo » se relit : au repos, ou après
dix pages de lettres ? Le STANDARD dit « working set privé » sans
préciser le geste.

**D9 tranchée le 2026-09-02 (CE) : « 5 (Recommandé) »** — `FENETRE = 5`
(11 iframes vivantes au plus), livré dans la passe 2 de terrain ; la
racine reste D-53, le budget à préciser au `/solde`.

## STOP 2 — passe 2 du 2026-09-02 : 4 OK, 1 KO, 1 remarque

Après `7d474ad` + `fe5ffec` (fenêtre 5) : journal sain, Répondre à
tous OK, Kiosque « Toujours afficher » OK, transfert (avant + après)
OK. **RAM après dix pages de Kiosque : 251,5 Mo sur 6 processus** —
GPU 132,3 Mo, rendu « Wind » 69,6, gestionnaire 36,3, réseau 8,1,
stockage 3,2, crashpad 1,8. La fenêtre 5 ne change rien au total
(249 → 251) : **le processus GPU porte plus de la moitié** — ce sont
les surfaces composées des iframes (et de leurs images distantes
accordées), pas le DOM. D9 tenue (5 ne coûte rien), mais le levier est
ailleurs : D-53 amendée (GPU), budget à préciser au solde.

**KO persistant : « DÉJÀ CONSULTÉ » chevauche la rangée Doctolib**,
un vide d'une rangée sous elle, en Réception organisée (capture 2 :
l'écho du transfert de test vient ensuite, section « Déjà consulté »).
La bande de section est positionnée en absolu d'après le MODÈLE de
hauteurs (`decalage` : h1 nue, h2 porteuse, entêtes) tandis que les
rangées s'empilent en flux à leur hauteur RÉELLE — dès qu'une rangée
au-dessus est plus haute que le modèle, l'entête remonte. **Reproduit
sur le décor Clarity** (géométrie lue dans le DOM) : bande à 458 px,
son vide à 481 — 23 px trop haut ; rangées réelles 94 / 121 px, modèle
h1 = 88 / h2 = 115 : **les sondes ne rendaient ni le bloc de boîte (vue
mêlée) ni le ⋯ du mode organisé** (24 px centrés dans une ligne de
14 px) — 6 px de moins par rangée, une rangée entière au bout de vingt
(le CE). TDD : `sections-liste.spec.js` (la bande calée sur son vide
au pixel, la première rangée lue juste sous elle) — RED 22,8 px, GREEN
en donnant aux sondes ce qui donne sa hauteur à la ligne réelle, sous
les mêmes conditions.

**Remarque d'apparence (CE)** : la barre collante du fil (Archiver /
Signaler comme spam / Épingler) doit respecter le Système : une
élévation ou des traits autour — on doit sentir qu'elle flotte
au-dessus du message. Aujourd'hui : bande à fond `--bg`, sans bord ni
ombre. Proposition (STOP visuel) : l'objet flottant du produit (A108,
`Menu.svelte`) — surface, bordure `--border`, rayon des contrôles,
`--shadow`, décollée de 8 px du haut du scrollport.

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
