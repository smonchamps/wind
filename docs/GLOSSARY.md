# Glossary — the English vocabulary of Wind

> Written on 2026-09-02 for [PLAN-BASCULE-ANGLAIS](PLAN-BASCULE-ANGLAIS.md)
> step E0, validated by the Chief Engineer at STOP 1 bis (decision D14).
> This is the design deliverable of the switch: **every French word of
> the code and the documentation is translated once, here, and applied
> everywhere.** The mechanical rename dictionary
> (`scripts/rename/dictionary.csv`, 1 210 identifiers; `keys.csv`,
> 480 catalogue keys; `dom.csv`, 542 test ids, CSS classes and e2e
> seams) is *derived* from the token tables below — a word changed here
> changes the dictionary, never the other way round.
>
> The first document of the repository written in English, on purpose.

## 1. Rules

1. **Product words come from the English catalogue** (`catalogue.en.js`,
   decided by the CE at PLAN-LANGUES and PLAN-MODE-ORGANISE): the code
   uses the same word the user reads (D6).
2. **One French word → one English word**, whatever the layer. Where
   the same French word carries two meanings, §4 rules on each.
3. **Naming style is unchanged**: Rust `snake_case` / `PascalCase` /
   `UPPER_SNAKE`, JS `camelCase`, DOM ids and CSS classes
   `kebab-case`. The dictionary rebuilds the style automatically.
4. **French compounds are head-first, English head-last**: `cleCarte`
   (« clé de carte ») becomes `cardKey`, `champCorps` `bodyField`,
   `menuGestes` `gestureMenu`. Every two-noun compound is ruled by hand
   in the token table (§5), never by word order.
5. **Test names are sentences**, translated by hand at E3 (227 Rust
   test functions such as `une_base_neuve_n_a_aucune_colonne_fantome`
   → `a_fresh_database_has_no_phantom_column`). They are referenced
   nowhere, so they are outside the dictionary.
6. **What stays French — by decision, not by oversight** (D3, D11):
   - the SQLite schema (26 tables, ~30 French columns), the six `prefs`
     keys, the files on disk (`wind.db`, `wind.log`, `maj.log`,
     `telemetry.json`, `discovery.db`) — debt D-54;
   - the browser `localStorage` keys (`wind-theme`, `wind-volets`,
     `wind-largeurs`, `wind-espacement`, `wind-accueil-*`): persisted on
     every tester's machine, renaming them would silently reset their
     layout — same debt;
   - the strings of `catalogue.fr.js`: the French UI is delivered word
     for word (only its **keys** are renamed);
   - `docs/archives/` and the closed plans, frozen with a banner (D1);
   - `BETA.fr.md`, kept for the current tester wave (D11);
   - the git history and the published release notes.
7. **Numbers are identities**: ADR `0008`, amendments `A-n`, debts
   `D-n`, decisions `D1`… and the STANDARD section numbers `§2.9` never
   change. Only the slug after the number is translated.
8. French typography goes: « » become “ ”, the em dash rule of the
   catalogues (RETOURS-14 R3) already forbids `—` in UI text; in prose
   it is allowed but not required.

## 2. Product vocabulary (from the English catalogue)

| French | English (UI) | Code token | Note |
|---|---|---|---|
| Réception | Inbox | `inbox` | |
| Réception organisée | Inbox (organized mode) | `organized_inbox` | |
| Kiosque | Feed | `feed` | `Kiosque.svelte` → `Feed.svelte` |
| Registre | Paper trail | `paper_trail` | `Registre.svelte` → `PaperTrail.svelte` |
| Portier | Screener | `screener` | `Portier.svelte` → `Screener.svelte` |
| écarté (par le Portier) | screened out | `screened_out` | |
| réintégrer | reinstate | `reinstate` | |
| Nettoyage (de printemps) | Spring cleaning / clean | `cleanup` | `Nettoyage.svelte` → `Cleanup.svelte` |
| Mis de côté, la pile | Set aside, the pile | `set_aside`, `pile` | `PileMisDeCote.svelte` → `SetAsidePile.svelte` |
| Repère (de compte) | Marker | `marker` | icon + hue of an account |
| teinte (du repère) | color | `hue` | `--rep-bleu` → `--mk-blue` |
| Fil | Thread | `thread` | screen 03; `Fil.svelte` → `Thread.svelte` — see §4 |
| Conversation | Conversation | `conversation` | unchanged |
| Fente d'avis | Notice slot | `notice`, `NoticeSlot.svelte` | the banner area under the header |
| Guichet (de compte) | Account desk | `desk`, `AccountDesk.svelte` | the add-account form |
| Accueil (premier lancement) | Onboarding | `onboarding` | `accueil.js` → `onboarding.js`; keys `accueil.*` → `onboarding.*` |
| Retour (feedback) | Feedback | `feedback`, `Feedback.svelte` | `Retour.svelte` is the feedback form; `retour` as *back* → `back` |
| Marque | Brand | `brand`, `Brand.svelte` | `--marque` → `--brand` |
| Drapeau UE | EU flag | `EUFlag.svelte` | |
| Icône | Icon | `icon`, `Icon.svelte` | |
| Lecture (volet) | Reading pane | `reading`, `Reading.svelte` | |
| Liste | List | `list`, `List.svelte` | |
| Composition | Compose | `compose`, `Compose.svelte` | keys `compo.*` → `compose.*` |
| Réglages | Settings | `settings`, `Settings.svelte` | |
| Volets | Layout / panes | `panes` | `volets.svelte.js` → `panes.svelte.js` |
| Espacement | Spacing | `spacing` | |
| Mode organisé | Organized mode | `organized_mode` | `organise.svelte.js` → `organized.svelte.js` |
| Tri (de section) | Sort | `sort`, `SectionSort.svelte` | |
| Modale de migration | Migration modal | `MigrationModal.svelte` | |
| Toast, Menu, Nav, Onboarding, Conversation, Theme | unchanged | | |
| Brouillon | Draft | `draft` | |
| Corbeille | Trash | `trash` | |
| Indésirables | Junk | `junk` | |
| Archives | Archive | `archive` | |
| Envoyés | Sent | `sent` | |
| Épingler / épinglé | Pin / pinned | `pin` | |
| Puce (compteur) | Chip | `chip` | `compose-format-puces` (bulleted list) → `compose-format-bullets` |
| Pastille (non-lu) | Badge | `badge` | |
| Horizon (d'import) | History to import | `horizon` | unchanged, it is the domain word |
| Signature, Notifications, Raccourcis, À propos | Signature, Notifications, Shortcuts, About | `signature`, `notifications`, `shortcuts`, `about` | settings groups |
| Bulles (d'arrivée) | Arrival notifications | `bubbles` | the in-app word stays |
| MAJ, mise à jour | Update | `update` | `verifierMaj` → `checkUpdate` |
| Invitation, organisateur, répondant | Invitation, organizer, attendee | `invitation`, `organizer`, `attendee` | |
| Pièce (jointe) | Attachment | `attachment` | |
| Expéditeur / destinataire | Sender / recipient | `sender`, `recipient` | |
| Objet, sujet | Subject | `subject` | see §4 |

## 3. Technical vocabulary (the core and the tooling)

| French | English | Where it lives | Why this word |
|---|---|---|---|
| relève, relever | poll | `sync.rs`, `commands.rs` (`faut_relever` → `must_poll`, `relever_inbox` → `poll_inbox`) | a relève is one pass over a folder asking the server what changed |
| relève (instantané de) | handover | `ETAT.md` → `STATE.md`, « l'instantané de relève » → *the handover snapshot* | a different meaning of the same word |
| veilleur, veille | watcher, watch | `veilleur.rs` → `watcher.rs`; `Veille` → `Watch`; `veille_session` → `watch_session` | IMAP IDLE watcher (ADR 0018) |
| déménagement | relocation | `demenagement.rs` → `relocation.rs`; `demenager_dossier` → `relocate_folder` | moving `discovery.db` to `wind.db` |
| geste | gesture | `GesteGroupe` → `GroupGesture`, `agir_groupe` → `act_on_group`, `sync_apres_geste` → `sync_after_gesture`, `menuGestes` → `gestureMenu` | a user act on a row; *action* is taken by `pending_actions` (`Action`) — see §4 |
| écho (d'envoi) | echo | unchanged | the local copy of a sent message |
| correspondants | contacts | `correspondants.rs` → `contacts.rs`; `Correspondant` → `Contact`; table `correspondants` stays | address book for autocomplete |
| rattrapage | backfill | `backfill.rs` already; `rattraper_apercus` → `backfill_previews` | |
| aperçu | preview | | |
| boîte | mailbox | `boite.js` → `mailbox.js`; keys `boite.*` → `mailbox.*` | *inbox* is only Réception |
| périmètre | scope | `boites_du_perimetre` → `mailboxes_in_scope`; `PERIMETRES_NETTOYAGE` → `CLEANUP_SCOPES` | |
| plage | range | `PLAGES_NETTOYAGE` → `CLEANUP_RANGES` | the period to clean |
| routage, router | routing, route | `router_expediteur` → `route_sender`; `REGLES_ROUTAGE` → `ROUTING_RULES` | ADR 0028 |
| règle | rule | | |
| décor | fixture | `TransportDecor` → `FixtureTransport` | test scenery |
| banc | bench | `banc_recherche.rs` → `bench_search.rs` | |
| diagnostic | diag | `diagnostic_ouverture.rs` → `diag_opening.rs` | |
| faux serveur | fake server | `faux_serveur.rs` → `fake_server.rs`; `FauxImap` → `FakeImap` | |
| borne, borné | bound, limit | `FluxBorne` → `BoundedStream`; `BORNE_OCTETS` → `BYTE_LIMIT` | |
| garde | guard | `GardeInstance` → `InstanceGuard`; `VolGarde` → `FlightGuard` | |
| vol, en vol | flight, in flight | `VOL_MAX` → `MAX_IN_FLIGHT`; `reponsesEnVol` → `repliesInFlight` | a call to the core that has not returned |
| pompe, hors pompe | pump, off pump | `hors_pompe` → `off_pump`; `HORS_POMPE` → `OFF_PUMP` | the Windows message pump (ADR 0019) |
| passe, passe légère | pass, light pass | `passe_legere_compte` → `light_pass_account` | |
| cycle, fin de cycle | cycle, cycle end | `FinDeCycle` → `CycleEnd` | ADR 0021 |
| sonde | probe | `sonde-gel.py` → `freeze-probe.py`; `sonderEtat` → `probeState` | |
| filet | net | `filet` (e2e) → `net`; the gate scripts are *nets* | a test that catches a regression |
| jeton | token | CSS tokens and async tokens alike | |
| retenue, couture | hold, seam | `__e2eRetenue` → `__e2eHold`; `lancerCouture` → `startSeam` | e2e seams in the transport |
| gabarit | template | `gabaritCorps` → `bodyTemplate` | |
| volet, cadre, carte, barre, rangée, ligne | pane, frame, card, bar, row, line | UI `ligne` (a list row) → `row`; Rust `ligne` (a trace line) → `line` — see §4 | |
| tête (de message), tuile, voile, tiroir, poignée | head, tile, veil, drawer, handle | | |
| encre, fond, sombre, clair, nuit | ink, bg, dark, light, night | | the token names already half English |
| lot, en masse | batch, bulk | `LOT` → `BATCH` | |
| chrono, coût, seuil, plafond, plancher | stopwatch, cost, threshold, cap, floor | | |
| quarantaine, refusée | quarantine, refused | ADR 0030 | |
| réparation, héritée, fantôme, adoption | repair, legacy, phantom, adoption | `reparations` table stays | migration words (ADR 0012) |
| trace, journal | trace, log | `trace.rs` unchanged; `journal` (e2e) → `log` | |
| clé, verrou | key, lock | `NOM_VERROU` → `LOCK_NAME` | |
| identifiant (d'app) | app id | `IDENTIFIANT` → `APP_ID` | `dev.elements.wind` |
| poste | workstation | `installer-poste.ps1` → `install-workstation.ps1` | |

### Method vocabulary (STANDARD, WORKFLOW, skills)

| French | English | Note |
|---|---|---|
| Chef Ingénieur, CE, shusa | Chief Engineer, CE, shusa | |
| chantier | job | `/chantier` → `/job`; a *job* runs from statement to green CI |
| terrain, au terrain | field, in the field | `/terrain` → `/field`; genchi genbutsu |
| gate | gate | unchanged; `/gate` |
| solde, soldé | close-out, closed | `/solde` → `/close`; « CHANTIER SOLDÉ » → *JOB CLOSED* |
| STOP 1 / STOP 2 | STOP 1 / STOP 2 | |
| andon, kaizen, jidoka, genchi genbutsu, set-based | unchanged | Toyota words stay Toyota words |
| constat | finding | |
| instruction (sur pièces) | investigation (on the evidence) | |
| refus de périmètre | scope refusal | |
| dette | debt | `DETTE.md` → `DEBT.md` |
| décision CE | CE decision | |
| revue à regard neuf | fresh-eyes review | |
| piège | trap | the §7.1 list |
| enseignement | lesson | STANDARD §9 |
| relève (instantané de) | handover snapshot | `ETAT.md` → `STATE.md` |
| passation | handover | `PASSATION.md` → `HANDOVER.md` |
| jeu d'essai | test set | `seed_inbox` |
| mesure, chiffre | measurement, figure | |
| le Système | the System | `systeme.dc.html` → `system.dc.html`, `systeme.css` → `system.css` |
| amendement A-n | amendment A-n | |
| journal des versions | changelog | |
| release, publier, vérifier | release, publish, verify | `faire-release.ps1` → `make-release.ps1`, `verifier-release.ps1` → `verify-release.ps1` |

## 4. Collision rulings

| Word | Conflict | Ruling |
|---|---|---|
| fil → thread | `mail-core::thread` (conversation grouping) and `std::thread` already coexist | `fil` is the UI thread screen and the conversation: **thread** everywhere; `messages_du_fil` → `messages_of_thread` because `thread_messages` exists; `fil_route` → `thread_route` |
| geste → gesture vs Action | `pending_actions` / `Action` is the queued server action | **gesture** = what the user does; **action** = what the queue replays |
| ligne | list row (UI, e2e) vs trace line (Rust) | UI/e2e `ligne` → **row** where it names a list row (`ligne-case` → `row-checkbox`), **line** in Rust and for text lines |
| objet / sujet → subject | both used in the UI and the specs | one thing, one word: **subject**. Measured: never both in one scope (`objet` is the Compose field, `sujet` a local in `Fil`/`Liste` and the specs); if a later scope holds both, the applier reports it and the second becomes `subjectText` |
| compte | account vs count | `compte_echos` → **count_echos**; everywhere else **account** |
| relève / relever | noun vs verb | both **poll** (`let releve = …` is the result of a poll; `_releve` lock guards → `_poll`); `faut_relever` (mail-core) and `doit_relever` (shell) both → `must_poll`, two crates |
| ouvert / ouvrir | adjective vs verb | `ouvert` → **isOpen**, `ouvrir` → **open** |
| clé / touche → key | CSS/storage key vs keyboard key | both **key** — `touche` is one closure local in `Menu.svelte`, no clash measured |
| attente / enAttente → pending | | `attente` (a waiting text/timer local) → **pending** too; two files, no clash |
| début / démarrer / lancer → start | | `debut` → **start** (noun), `demarrer` → **start** (verb, `Nettoyage` only), `lancer` → **launch** |
| envoyés / envoyé → sent | the Sent folder in tests vs a sent body local | both **sent**, never in one scope |
| écho d'envoi | `echo_envoi` (fn, `echo.rs`) and `envoi_echo` (local, `seed_clarity`) | both **send_echo**, two crates |
| retour | back vs feedback | `Retour.svelte` → **Feedback**; `accueil.retour`, `retour-boite` → **back** |
| marque / marquer | brand vs mark | `marque` → **brand**, `marquer` → **mark** |
| pile | the set-aside pile only | **pile** unchanged (no stack in the code) |
| chips / puces | Liste chips vs compose bullets | **chip**, except `compose-format-puces` → **bullets** |
| sous | under vs sub | `SocketSous` → **InnerSocket**; `sousTitre` → **subtitle**; `sousAgents` → **subagents** |
| léger / clair → light | | `passe_legere` → **light_pass**; `clair` (theme) → **light**; never in one scope |

## 5. Renames

### 5.1 Files

| Layer | From → to |
|---|---|
| Rust modules | `correspondants.rs` → `contacts.rs`, `faux_serveur.rs` → `fake_server.rs`, `demenagement.rs` → `relocation.rs`, `veilleur.rs` → `watcher.rs` (`echo.rs`, `invitation.rs`, `trace.rs` unchanged) |
| Rust examples | `banc_indexation` → `bench_indexing`, `banc_migration_fils` → `bench_thread_migration`, `banc_nettoyage` → `bench_cleanup`, `banc_page_liste` → `bench_list_page`, `banc_recherche` → `bench_search`, `diagnostic_boites` → `diag_mailboxes`, `diagnostic_brouillons` → `diag_drafts`, `diagnostic_fils` → `diag_threads`, `diagnostic_ouverture` → `diag_opening`, `seed_arrivee` → `seed_arrival` |
| Svelte components | `BarreFil` → `ThreadBar`, `Composition` → `Compose`, `DrapeauUE` → `EUFlag`, `FenteAvis` → `NoticeSlot`, `Fil` → `Thread`, `GuichetCompte` → `AccountDesk`, `Icone` → `Icon`, `Kiosque` → `Feed`, `Lecture` → `Reading`, `Liste` → `List`, `Marque` → `Brand`, `ModaleMigration` → `MigrationModal`, `Nettoyage` → `Cleanup`, `PileMisDeCote` → `SetAsidePile`, `Portier` → `Screener`, `Registre` → `PaperTrail`, `Reglages` → `Settings`, `Retour` → `Feedback`, `TriSection` → `SectionSort` (`App`, `Conversation`, `Menu`, `Nav`, `Onboarding`, `Toast` unchanged) |
| UI `lib/` | `accueil` → `onboarding`, `boite` → `mailbox`, `clavier` → `keyboard`, `corps` → `body`, `icones` → `icons`, `liens` → `links`, `portier` → `screener`, `quand` → `when`, `reperes` → `markers`, `tri` → `sort`, `vocabulaires` → `vocabularies`, `espacement.svelte` → `spacing.svelte`, `fil.svelte` → `thread.svelte`, `largeurs.svelte` → `widths.svelte`, `organise.svelte` → `organized.svelte`, `texte.svelte` → `text.svelte`, `volets.svelte` → `panes.svelte`, `catalogue.fr/en` → `catalog.fr/en`, `systeme.css` → `system.css` |
| e2e tools | `args-navigateur` → `browser-args`, `bascule-sombre.ps1` → `dark-toggle.ps1`, `capture-accueil` → `capture-onboarding`, `catalogues.test` → `catalogs.test`, `coherence-systeme` → `system-coherence`, `contraste` → `contrast`, `garde-thread-principal` → `main-thread-guard`, `geste-defilement` → `scroll-gesture`, `jetons` → `tokens`, `mesure-defilement` → `measure-scroll`, `mesure-ram.ps1` → `measure-ram.ps1`, `mesure-scrollbar` → `measure-scrollbar`, `mesure-v2` → `measure-v2`, `sonde-gel.py` → `freeze-probe.py` |
| e2e specs | `banc-ram-kiosque` → `bench-ram-feed`, `demarrage` → `startup`, `espacement` → `spacing`, `kiosque-images` → `feed-images`, `menu-clavier` → `keyboard-menu`, `mode-organise` → `organized-mode`, `nettoyage` → `cleanup`, `refonte-*` → `redesign-*` (`defilement` → `scroll`, `ecran02` → `screen02`, `langue` → `language`, `parcours-portes` → `gated-journeys`, `reconnexion` → `reconnect`, `retours-n` → `feedback-n`, `retrait-compte` → `account-removal`, `volets` → `panes`), `repere-ligne` → `row-marker`, `retours-12-entete` → `feedback-12-header`, `retours-14-reception` → `feedback-14-inbox`, `retours-9-nom-compte` → `feedback-9-account-name`, `sections-liste` → `list-sections`, `selection-multiple` → `multi-select` |
| Scripts | `faire-release` → `make-release`, `verifier-release` → `verify-release`, `lancer-wind` → `run-wind`, `installer-poste` → `install-workstation`, `construire-wind.mjs` → `build-wind.mjs`, `mesurer-sessions.mjs` → `measure-sessions.mjs`, `terrain.ps1` → `field.ps1`, `faire-icone` → `make-icon` (`gate.ps1` unchanged) |
| Skills | `chantier` → `job`, `terrain` → `field`, `solde` → `close` (`gate` unchanged); agent `spike` unchanged |
| Docs | `ETAT` → `STATE`, `DETTE` → `DEBT`, `PASSATION` → `HANDOVER`, `PLAN-BASCULE-ANGLAIS` → `PLAN-ENGLISH-SWITCH`; `design/systeme.dc.html` → `design/system.dc.html`; `ANNOTATIONS-V3` unchanged |

### 5.2 ADR slugs (numbers unchanged)

`0001-workspace-structure`, `0002-tauri-desktop-shell`, `0003-smtp-outbox`,
`0004-fts5-search-engine`, `0005-e2e-gate-outside-hosted-ci`,
`0006-microsoft-imap-oauth2`, `0007-body-backfill`,
`0008-conversation-threading`, `0009-thread-scope-per-account`,
`0010-full-synchronization`, `0011-wal-journal`,
`0012-visible-interruptible-migration`, `0013-nsis-installer-signed-update`,
`0014-local-crash-telemetry`, `0015-ui-v2-svelte-foundation`,
`0016-language-homegrown-catalog`, `0017-poll-guarded-by-status`,
`0018-idle-watcher`, `0019-commands-off-the-main-thread`,
`0020-thread-one-object-two-frames`, `0021-full-cycle-cadence`,
`0022-rich-multipart-body`, `0023-x64-channel-return`,
`0024-icalendar-parser-calcard`, `0025-embedded-oauth-credentials-release`,
`0026-elements-system`, `0027-living-theme-table`, `0028-local-sender-routing`,
`0029-import-horizon-per-account`, `0030-single-instance-and-action-quarantine`,
`0031-audit-wave-2-bounded-send-atomic-batch-imageless-forward`.

### 5.3 IPC commands (shell and UI in the same commit, E4)

`agir_groupe` → `act_on_group`, `chemin_enregistrement_suggere` →
`suggested_save_path`, `completer_adresses` → `complete_addresses`,
`etat_ui` → `ui_state`, `kiosque_cartes` → `feed_cards`,
`kiosque_marquer_lu` → `feed_mark_read`, `kiosque_non_ouverts` →
`feed_unopened`, `mode_organise_get/set` → `organized_mode_get/set`,
`nettoyage_demarrer` → `cleanup_start`, `nettoyage_etat` → `cleanup_state`,
`nettoyage_groupes` → `cleanup_groups`, `nettoyage_messages` →
`cleanup_messages`, `nettoyage_terminer` → `cleanup_finish`,
`nettoyage_verdict` → `cleanup_verdict`, `nom_set` → `name_set`,
`noms_adresses` → `address_names`, `noms_get` → `names_get`,
`pile_mis_de_cote` → `set_aside_pile`, `portier_adresses` →
`screener_addresses`, `portier_attente` → `screener_waiting`,
`portier_defauts_get/set` → `screener_defaults_get/set`, `portier_total` →
`screener_total`, `registre_groupe_page` → `paper_trail_group_page`,
`registre_groupes` → `paper_trail_groups`, `repere_set` → `marker_set`,
`reperes_get` → `markers_get`, `repondre_invitation` → `reply_invitation`,
`reseau_etat` → `network_state`, `retirer_routage` → `remove_routing`,
`routages` → `routings`, `router_expediteur` → `route_sender`,
`router_expediteur_de` → `route_sender_from`, `sync_apres_geste` →
`sync_after_gesture`, `toggle_mis_de_cote` → `toggle_set_aside`. The other
~72 commands are already English (`lang_get`, `horizon_import_set`,
`images_senders`… unchanged).

### 5.4 Catalogue key namespaces (both catalogues + 569 `t()` calls, E5)

`boite.` → `mailbox.`, `entete.` → `header.`, `portier.` → `screener.`,
`kiosque.` → `feed.`, `registre.` → `paper_trail.`, `nettoyage.` →
`cleanup.`, `pile.` → `pile.`, `repere.` → `marker.`, `reglages.` →
`settings.`, `compo.` → `compose.`, `lecture.` → `reading.`, `liste.` →
`list.`, `statut.` → `status.`, `erreur.` → `error.`, `avis.` → `notice.`,
`accueil.` → `onboarding.`, `guichet.` → `desk.`, `raccourci.` →
`shortcut.`, `puce.` → `chip.`, `onglet.` → `tab.`, `depuis.` → `since.`,
`groupe.` → `group.`, `retour.` → `feedback.`, `langue.` → `language.`,
`brouillons.` → `drafts.`, `volets.` → `panes.`, `espacement.` →
`spacing.`, `quand.` → `when.`, `tri.` → `sort.` (`action.`, `toast.`,
`inv.`, `conv.`, `nav.`, `horizon.`, `migration.`, `theme.` unchanged).
The second segment follows the token table (`reglages.repereCompte` →
`settings.markerAccount`). 478 of 496 keys change.

### 5.5 CSS tokens (three files in one commit, DC-D6 — decision D12)

`--marque` → `--brand`, `--r-controle` → `--r-control`, `--r-tuile` →
`--r-tile`, `--tuile` → `--tile`, `--rep-bleu/-magenta/-olive/-rouge` →
`--mk-blue/-magenta/-olive/-red` (`--ink`, `--bg`, `--border`, `--alert`,
`--panel`, `--shadow`, `--r-surface` unchanged).

### 5.6 DOM contract and e2e seams (Svelte and specs in one commit, E5-E6)

Test ids and CSS classes follow the token table in kebab-case
(`ligne-case` → `row-checkbox`, `nettoyage-demarrer` → `cleanup-start`,
`kiosque-vers-…` → `feed-to-…`, `.repere-nu` → `.bare-marker`,
`.tete-message` → `.message-head`). Seams: `__e2eRetenue` → `__e2eHold`,
`__e2eJournal` → `__e2eLog`, `__e2eAjout` → `__e2eAdd`, `__e2eLiens` →
`__e2eLinks`, `__e2eAccueil` → `__e2eOnboarding`, `__e2eLiberer` →
`__e2eRelease`, `__e2ePanne` → `__e2eFailure`, `__e2ePieces` →
`__e2eAttachments`, `__e2eDestination` unchanged.

## 6. How the dictionary is built and applied

- `scripts/rename/tokens.csv` — the token tables above, one French
  segment or phrase per line (phrases win over single segments, longest
  first).
- `scripts/rename/dictionary.csv` — `layer, old, new, occurrences, files`
  for every French-named definition found by the inventory
  (1 210 rows: Rust 777 definitions minus 227 test sentences, UI 595,
  e2e and scripts 208). `keys.csv` and `dom.csv` carry the catalogue
  keys and the DOM contract.
- `scripts/rename/test-names.txt` — the 227 test sentences left to the
  hand, `collisions.txt` — every new name that already exists somewhere
  in the code (360 lines, almost all harmless: a local `compte` becoming
  `account` next to another function's `account`); the applier reports
  a clash only when both names share one scope.
- `scripts/rename/extract-defs.py` and `build-dictionary.py` — the
  inventory and the derivation (Python, throw-away tooling of E0; E1
  ports the applier to Node so the repository keeps a single Python
  tool, `freeze-probe.py`).
- The applier (`scripts/rename/apply.mjs`, written at E1) replaces
  **whole identifiers only** (`\b…\b`), never inside string literals or
  comments, one layer at a time; the compiler, the Vite build, the e2e
  suite and the three nets of E1 decide. `cargo fmt` runs after every
  pass.
- Rows the CE strikes at STOP 1 bis are edited in `tokens.csv`, the
  dictionary is regenerated — never patched by hand.
