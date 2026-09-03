# PLAN-BASCULE-ANGLAIS — tout le dépôt du français vers l'anglais

> **CHANTIER OUVERT le 2026-09-02** (GO CE après 0.16.0, 0.17.0 et le solde
> de PLAN-AUDIT-V2 ; quatorze décisions tranchées, §6).
>
> Rédigé le 2026-09-02 sur commande du CE (« basculer l'ensemble du
> code et de la documentation du français vers l'anglais »). **Aucun
> code n'est écrit avant le GO du STOP 1** (décisions D1-D14 ci-dessous).
> Ce plan est lui-même en français : il se traduit à son étape (E7),
> comme les autres documents vivants. La règle STANDARD §2.8 (« tout
> est en français ») s'amende au **premier commit** du chantier ; à
> partir de là, les commits sont en anglais.
>
> Principe directeur : **un chantier de renommage n'est pas un chantier
> de comportement.** Chaque commit laisse un produit strictement
> identique pour l'utilisateur (hors D4), une gate verte et un état
> bisectable. Le compilateur Rust, le build Vite, les e2e et trois
> filets neufs (E1) sont les oracles — jamais l'œil seul.

## 1. Constat — où vit le français (mesuré le 2026-09-02, `0a3fb7d`)

Toutes les mesures excluent `target/`, `node_modules/`, `dist/`,
`spikes/` et `.claude/worktrees/`. « Français-ish » = identifiant dont
au moins un segment est un mot français d'une liste de ~300 (heuristique,
sous-estime plutôt qu'elle ne surestime).

### 1.1 Le code

| Couche | Fichiers | Lignes | Mots de commentaires | Définitions françaises | Identifiants français | Notes |
|---|---|---|---|---|---|---|
| Rust, 6 crates | 54 `.rs` | ~36 800 | ~60 500 | — | — | `store.rs` seul : 10 504 lignes |
| Rust, shell `apps/desktop/src` | 7 `.rs` | ~7 200 | ~18 300 | — | — | `commands.rs` 7 146 lignes |
| Rust, total | 61 | 43 988 | ~78 800 | **613 / 1 851 (33 %)** | 873 / 3 757 | 664 `#[test]` aux noms français (`une_base_neuve_n_a_aucune_colonne_fantome`) ; 20 messages `#[error]` français ; ~62 000 mots dans des littéraux (SQL, traces, erreurs, jeu d'essai) |
| UI Svelte + JS (`ui-v2/src`) | 25 composants + 24 modules `lib/` | 11 693 + ~2 800 | ~33 800 | **455 / 844 (54 %)** | 587 / 1 691 | tous les noms de composants sont français (`Reglages`, `Nettoyage`, `Kiosque`, `PileMisDeCote`, `GuichetCompte`, `FenteAvis`…) |
| e2e + scripts | 29 specs, ~20 outils `.mjs`, 9 `.ps1`, 1 `.py` | ~9 300 | ~22 300 | 140 / 420 | 313 / 1 677 | **2** ancrages seulement sur un libellé français (`getByRole … name: 'Annuler l'envoi'`) : les parcours s'ancrent sur la structure, pas sur les mots |
| Outillage méta | `CLAUDE.md`, 4 skills (275 l.), agent `spike`, `gate.ps1`, hook `pre-push`, `ci.yml`, `launch.json` | ~700 | — | — | — | noms de scripts français (`make-release.ps1`, `verify-release.ps1`, `run-wind.ps1`, `install-workstation.ps1`, `build-wind.mjs`, `measure-sessions.mjs`) |

Surfaces de **contrat** (un nom des deux côtés d'une frontière) :

- **IPC Tauri** : 110 commandes `#[tauri::command]`, ~45 françaises
  (`agir_groupe`, `nettoyage_*` ×6, `kiosque_*` ×3, `portier_*` ×5,
  `registre_*` ×2, `repere_*` ×2, `pile_mis_de_cote`, `router_expediteur*`,
  `retirer_routage`, `completer_adresses`, `chemin_enregistrement_suggere`,
  `sync_apres_geste`, `reseau_etat`, `etat_ui`, `nom_set`/`noms_get`…).
  Appelées depuis l'UI par `appel('nom')` (`lib/transport.js`) : **le
  compilateur ne voit pas cette frontière** — un nom raté = un rejet à
  l'exécution, découvert par un e2e ou au terrain.
- **Schéma SQLite** (`store.rs:26-460`) : 26 tables dont **9 françaises**
  (`correspondants`, `echos`, `mis_de_cote`, `kiosque_lus`,
  `images_expediteurs`, `routage_expediteurs`, `portier_attente`,
  `nettoyage_session`, `reparations`) et ~30 colonnes françaises
  (`annule`, `borne_epoch`, `debut_epoch`, `journee_entiere`, `lieu`,
  `methode`, `organisateur_*`, `perimetre`, `plage`, `refusee`, `regle`,
  `relevee_epoch`, `repondant_*`, `reponse`, `titre`, `traites`,
  `initialisee`…). Six clés `prefs` (`lang`, `mode_organise`,
  `mode_organise_epoch`, `horizon_import`, `nom_compte`, `notif_pref`).
  Fichiers sur disque : `wind.db`, `discovery.db`, `wind.log`, `maj.log`,
  `telemetry.json`. **Tout cela vit sur les postes des testeurs.**
- **Catalogues de langue** (ADR 0016) : `catalogue.fr.js` 645 l.,
  `catalogue.en.js` 616 l., **clés françaises** (`'boite.kiosque'`,
  `'portier.oui'`…), 569 appels `t()`. Le français est la référence et
  le repli ; `refonte-langue.spec.js` affirme « le français du prototype
  est la langue par défaut » ; `Lang::from_pref` (`notify.rs:35`) rend
  `Fr` par défaut. **L'anglais produit existe déjà, validé CE** :
  Kiosque → *Feed*, Portier → *Screener*, Registre → *Paper trail*,
  Mis de côté → *Set aside*, Repère → *Marker*, Nettoyage → *Clean*.
- **Jetons CSS** : 15 propriétés `--*` dans `systeme.css`, mixtes
  (`--ink`, `--bg`, `--border` anglais ; `--marque`, `--r-controle`,
  `--r-tuile`, `--rep-bleu`… français), tenues valeur pour valeur par la
  gate `coherence-systeme.mjs` contre `systeme.dc.html` et `theme.js`
  (DC-D6) : renommer un jeton = trois fichiers au même commit.
- **Traces `wind.log`** : 15 sites `trace::trace(…)`, lignes françaises ;
  aucun script du dépôt ne les relit (grep `wind.log|maj.log` dans
  `scripts/`, `e2e/`, skills : 0) — le CE les lit à l'œil au terrain.

### 1.2 La documentation

| Corpus | Volume | Statut |
|---|---|---|
| `docs/*.md` vivants : ETAT 1 459 l., STANDARD 987, DETTE 964, PLAN.md 254, WORKFLOW 141, BETA 141, AUDIT 843, PASSATION 10 | ~4 800 lignes | normatif ou volatile, relu à chaque session |
| 30 `PLAN-*.md` non archivés | 9 537 lignes | mélange : PLAN-AUDIT-V2 en cours, les autres soldés mais pas encore déplacés |
| 31 ADR | 2 916 lignes | **décisions gelées**, vivantes ; noms de fichiers français (`0008-regroupement-en-conversations.md`) |
| 29 archives (`docs/archives/`) | 6 804 lignes | clos ; PHASE0-3, plans soldés |
| `CHANGELOG.md` | 758 lignes | public ; **lu par `make-release.ps1`** pour les notes de Release (`## [x.y.z]` obligatoire) ; 5 Releases publiées 0.11→0.15 avec notes françaises |
| `README.md`, `e2e/README.md`, `assets/icones/README.md`, `ANNOTATIONS-V3.md` | 36 + 100 + 18 + 65 l. | vivants |
| **`docs/design/systeme.dc.html`** | 570 Ko, **~37 000 mots**, 109 amendements A-n | **seul normatif de l'UI** (A18), outillé par la gate ; le journal A-n est cité 3 978 fois (docs + code) |
| `docs/architecture/index.html` | ~4 300 mots | vivant |
| Skills, agent, `CLAUDE.md`, mémoire persistante (17 fichiers hors dépôt) | ~600 l. | instruction permanente de la session |
| **Total markdown** | 104 fichiers, 25 346 lignes, **~200 000 mots** | |

Renvois qui cassent à un renommage de fichier : **71 liens markdown**
vers `adr/`, `PLAN-*.md`, `archives/` ; 592 mentions « ADR nnnn »
(numéro, pas chemin — survivent) ; 43 noms de PLAN cités dans le code ;
9 chemins `docs/adr/…` cités dans des commentaires Rust. Aucun
vérificateur de liens n'existe au dépôt.

### 1.3 Ce qui ne bouge pas, par nature

- **L'historique git** : 488 commits français, sans accents. Il vient
  d'être réécrit une fois (2026-09-01, `filter-repo`, pour des PII) et
  le ticket support GitHub est encore dû : **on ne le réécrit pas pour
  une langue** (refus §5).
- **Les Releases publiées** (0.11 → 0.15) et leurs notes.
- **Les bases et fichiers sur les postes** (D3).

### 1.4 Ordre de grandeur

~135 000 mots de commentaires de code + ~200 000 mots de markdown +
~41 000 mots de HTML normatif = **~375 000 mots de prose**, plus
**~1 200 définitions** à renommer, plus ~115 fichiers à renommer
(25 composants, 31 ADR, 30 PLAN, 29 specs/outils, 9 scripts). La
traduction de prose est assistée par modèle et relue (D9) ; le
renommage est mécanique et vérifié par les oracles. **Le premier crate
livré (E3a) donne le débit réel** ; l'estimation totale (§4) se
re-mesure à ce moment-là, pas avant.

## 2. Périmètre

**Dedans** : identifiants, commentaires, chaînes techniques, noms de
fichiers et de modules du code Rust, Svelte, JS, PowerShell, Python ;
contrat IPC ; clés des catalogues ; noms de tests et de specs ; scripts,
gate, hook, CI ; tous les documents vivants (§1.2) ; le Système et la
carte d'architecture ; skills, agent, `CLAUDE.md`, mémoire ;
STANDARD §2.8 (la règle elle-même) ; la convention de commit.

**Dehors, par refus explicite (§5)** : l'historique git ; les
identifiants **persistés** (schéma SQLite, clés `prefs`, fichiers sur
disque) sauf décision D3 contraire ; `spikes/` (jetables, hors
workspace — 1,1 Go, 20 dossiers) ; tout changement de comportement ;
toute langue d'interface au-delà de fr/en.

**À la décision du CE** : les archives (D1), la langue par défaut (D4),
les documents des testeurs (D11), le CHANGELOG passé (D13).

## 3. Options — départagées sur des faits

### 3.1 Ordre d'attaque

| Option | Fait qui tranche | Verdict |
|---|---|---|
| A. Docs d'abord, code ensuite | les docs citent ~1 200 identifiants et 43 PLAN par leur nom : traduire la prose AVANT de renommer produit des noms pendants qu'il faudra re-corriger | rejetée |
| B. Big bang, un commit | gate complète ~4-10 min mais revue impossible, bisect mort, un rouge e2e flaky (mémoire : la suite flake ici) bloque tout | rejetée |
| **C. Par couche, du bas vers le haut, un commit par couche, gate à chaque** | chaque couche a son oracle (crate : `cargo build` ; shell+UI : build Vite + e2e ; docs : liens + filet de langue) ; un commit = un état livrable | **retenue** |

Ordre C : glossaire (E0) → filets (E1) → outillage méta (E2) →
crates feuilles puis `mail-core` (E3) → shell + UI **au même commit
pour l'IPC** (E4) → UI seule (E5) → e2e/scripts (E6) → docs vivantes
(E7) → Système (E8) → archives selon D1 (E9) → mémoire et solde (E10).

### 3.2 Mécanique du renommage

| Option | Fait qui tranche | Verdict |
|---|---|---|
| `sed` global sur le texte | `fil` est un sous-mot de `filtre`, `profil`, `fil_route` ; `nom` de `nombre` : collisions garanties sur 44 000 lignes | rejetée |
| rust-analyzer « rename symbol » | pas de pilotage en ligne de commande dans l'outillage du dépôt ; ne couvre ni Svelte ni les chaînes IPC | rejetée comme outil principal |
| **Dictionnaire `ancien → nouveau` appliqué par un script sur les identifiants entiers (`\b`), hors littéraux et hors commentaires, puis compilation** | le compilateur Rust attrape toute référence ratée ; côté JS il n'y a pas de compilateur → filet `no-undef` à mesurer (E1c) ; `cargo fmt` rejoué après (mémoire : fmt après tout remplacement mécanique) | **retenue** |

Le dictionnaire est **le** livrable de conception (E0) : chaque nom
tranché une fois, appliqué partout. Les collisions connues à instruire
au glossaire : `fil` → *thread* alors que `mail-core::thread` (union-find
des conversations) et `std::thread` existent déjà ; `releve` (la passe
de synchro) ; `geste` (action utilisateur vs `Action` de
`pending_actions`) ; `boite` (mailbox vs inbox) ; `corps` (body) ;
`pièce` (attachment).

### 3.3 La preuve « tout est en anglais »

| Option | Fait qui tranche | Verdict |
|---|---|---|
| Compter les accents | après la bascule, un « é » oublié se voit, mais « le fichier est ouvert » n'a pas d'accent ; 0 accent ≠ 0 français | insuffisant seul |
| **Filet de mots-outils français** (`le la les des une est pas pour dans avec sur qui que ne cette sont été était jamais toujours…`) par fichier, avec **cliquet** : une base de référence commitée, la gate refuse toute hausse, chaque étape abaisse la base, à la fin la base est 0 et le filet devient absolu | mesure ce qu'un lecteur voit ; exclut par liste les corpus légitimement français (`catalogue.fr.js`, `BETA.fr.md` si D11, archives si D1-gel) | **retenue** (E1a) |

Le filet se prouve **en le cassant** (mémoire : trois tests sur cinq
étaient décoratifs à PLAN-ESPACEMENT) : un commentaire français glissé
dans un fichier « propre » doit rougir la gate avant que le filet ne
soit déclaré livré.

## 4. Étapes

Tailles : P < ½ jour, M = 1 jour, G = 2-3 jours. Chaque étape = un ou
plusieurs commits, gate complète avant chacun (les étapes docs
empruntent le chemin rapide documentaire du hook, étapes 1-6).

### E0 — Glossaire et dictionnaire (M)

> **Livré le 2026-09-02, STOP 1 bis joué le jour même : « Validé tel quel » (CE)** — [GLOSSARY.md](GLOSSARY.md)
> (en anglais — le premier document du dépôt à l'être, à dessein) et
> `scripts/rename/` : `tokens.csv` (1 588 segments et locutions),
> `dictionary.csv` (1 210 identifiants dérivés), `keys.csv` (480 clés de
> catalogue sur 496), `dom.csv` (542 test ids, classes CSS et coutures
> e2e sur 652), `test-names.txt` (227 phrases de test, à la main à E3),
> `collisions.txt` (360 homonymes, presque tous hors de toute portée
> commune). Inventaire réel : 1 580 définitions françaises (Rust 777,
> UI 595, e2e/scripts 208) — l'estimation de §1.4 (~1 200) était basse
> de 30 %. Rulings de collision au §4 du glossaire (fil/thread,
> geste/action, ligne row/line, compte account/count…).

- `docs/GLOSSARY.md` : le vocabulaire **produit** (repris de
  `catalogue.en.js`, déjà tranché CE à PLAN-LANGUES et
  PLAN-MODE-ORGANISE : Feed, Screener, Paper trail, Set aside, Marker,
  Clean…) et le vocabulaire **technique** (relève → *sweep* ou *poll*,
  veilleur → *watcher*, déménagement → *relocation*, geste → *action*,
  fente d'avis → *notice slot*, guichet → *account desk*, rangée → *row*,
  volet → *pane*, repère → *marker*, horizon → *horizon*…). Une entrée
  par mot, tranchée une fois (D6).
- `scripts/rename-dictionary.csv` : identifiant → identifiant, dérivé
  du glossaire, **complet avant le premier renommage** (les ~1 200
  définitions de §1.1, extraites par le script de mesure de ce plan).
- Inventaire des cas à décision unitaire : collisions §3.2, les 15
  jetons CSS, les clés des catalogues.

### E1 — Trois filets, prouvés en les cassant (M)

> **Livré le 2026-09-02.** `e2e/language-gate.mjs` (cliquet : 275 fichiers
> suivis, 260 avec du français, **142 113 marqueurs** à la base de
> référence `e2e/language-baseline.json` ; `spikes/`, `docs/archives/`,
> `catalogue.fr.js`, `BETA.fr.md`, `scripts/rename/` exemptés ; une
> ligne portant `lang:fr` est exemptée), `e2e/ipc-contract.mjs`
> (110 commandes définies, 111 enregistrées, 104 appelées par nom ;
> **`queue_send` n'était vu par personne** — un commentaire entre les
> attributs), `e2e/docs-links.mjs` (77 fichiers, 207 liens relatifs, 0
> mort ; 3 morts trouvés dans `spikes/`, hors périmètre). **Chacun prouvé
> en le cassant** : un mot français ajouté à `dependabot.yml` (11 → 16,
> rouge), un `appel('nope_cmd')` (rouge), un lien mort dans GLOSSARY.md
> (rouge), puis vert une fois rétabli. **E1c set-based, mesuré** : eslint
> `no-undef` (flat config, plugin Svelte, runes en globals) attrape la
> casse en 3-5 s dans `.svelte` ET `.js`, 0 erreur préexistante ;
> `svelte-check --threshold error` l'attrape aussi (8,5 s) mais sur
> **1 059 erreurs préexistantes** (`checkJs`) — rejeté, désinstallé.
> Gate : 13 étapes (7-9 neuves, jouées aussi sur le chemin documentaire),
> `npm run lint` dans l'étape 2 ; CI : quatre pas neufs au job `ui-v2`.
> Les ajouts de ce commit sont écrits en anglais : le cliquet interdit
> déjà toute hausse.

- **E1a `e2e/language-gate.mjs`** : le cliquet de §3.3, ajouté à
  `gate.ps1` (étape textuelle, secondes) et à `ci.yml` job `ui-v2`.
- **E1b `e2e/ipc-contract.mjs`** : chaque `appel('x')` de `ui-v2/src`
  doit exister dans `generate_handler![…]` de `main.rs`, et
  réciproquement toute commande enregistrée a un appelant ou une raison
  écrite. Ce filet n'existe pas aujourd'hui ; il survit au chantier.
- **E1c** filet JS `no-undef` : set-based à deux options mesurées sur
  UN renommage volontairement cassé — (i) `eslint` avec la seule règle
  `no-undef` + plugin Svelte, (ii) `svelte-check` seul. On garde ce qui
  attrape la casse en moins de 10 s ; si aucun n'attrape, les e2e
  restent l'oracle et on le dit.
- **E1d `e2e/docs-links.mjs`** : tout lien markdown relatif résout
  (71 liens à risque, 592 mentions d'ADR). Chemin rapide documentaire.

### E2 — Outillage méta (P)

> **Delivered on 2026-09-02** (this note, and every new paragraph from
> here on, is written in English — D2; the ratchet forbids any rise).
> Renamed with `git mv` and rewritten in English, ASCII in the `.ps1`
> (the PowerShell 5.1 trap, no BOM needed any more): `make-release.ps1`,
> `verify-release.ps1`, `run-wind.ps1`, `install-workstation.ps1`,
> `build-wind.mjs`, `measure-sessions.mjs` (identifiers too, self-contained),
> `field.ps1`, `make-icon.ps1`; skills `job`, `field`, `close`, `gate`;
> the `spike` agent, `CLAUDE.md`, `WORKFLOW.md`, `gate.ps1` (13 steps,
> `-DocsOnly`), the pre-push hook, `ci.yml`, `dependabot.yml`,
> `launch.json`, the `.gitignore` comment; **STANDARD §2.8 amended**
> ("Everything is in English", commits in English, the "no accents" rule
> void). Every reference to the old names replaced in 38 tracked files
> (living docs, Rust and TOML comments, memory) — the mapping tables of
> GLOSSARY.md, the archives, `spikes/` and the design HTML left alone.

`CLAUDE.md`, les 4 skills, l'agent `spike`, `WORKFLOW.md`, `gate.ps1`,
`.githooks/pre-push`, `ci.yml`, `launch.json`, `dependabot.yml`, les 9
scripts renommés (`make-release.ps1`, `verify-release.ps1`,
`run-wind.ps1`, `install-workstation.ps1`, `build-wind.mjs`,
`measure-sessions.mjs`, `field.ps1`, `make-icon.ps1`) — et **STANDARD
§2.8 amendé au même commit** : *everything is in English ; commits
`type: description` in English*. La convention « sans accents » devient
sans objet et s'efface. Mémoire : les pointeurs vers les scripts
(`gate-complete-avant-commit`, `numerotation-versions-semver`,
`verifier-release-wind`) suivent à E10.

### E3 — Les crates, du bord vers le cœur (G, en plusieurs commits)

> **E3a delivered on 2026-09-02** — `mail-ical`, `mail-render`, `mail-smtp`,
> `mail-auth` (3 476 lines) rewritten in English: identifiers by the
> glossary, comments and doc comments translated, literals in English
> (D5: `#[error]` messages, HTTP replies of the OAuth loopback, the
> `is_connection_error` prefix now `connection`), 96 test names turned
> into English sentences. The public API of `mail-ical` changed
> (`Method`, `When`, `Person`, `ReplyRequest`, `IcalError`, `parse`,
> `itip_reply`, fields `title`/`location`/`organizer`/`start`/`end`/
> `attendee`…): its two dependents (`mail-core/invitation.rs`, the
> shell's `repondre_invitation`) were updated in the same commit — the
> stable strings of the database (`"accepte"`, `"sans_reponse"`…) stay
> (D3). Oracles: `cargo build`, `clippy -D warnings`, 24 + 16 + 27 + 29
> tests green. Rate measured: four crates in one session hour — the
> remaining Rust (`mail-imap` 2 800 lines, `mail-core` ~24 000, shell
> ~7 200) is ten times that volume.
>
> **E3b delivered on 2026-09-02** — `mail-imap` (3 398 lines): `lib.rs`,
> `convert.rs`, `mutf7.rs`, `tests_e3.rs`, `faux_serveur.rs` →
> `fake_server.rs`, `examples/sync_gmail.rs`. Public API renamed:
> `Veille` → `Watch` (`Courrier`/`Echeance` → `Mail`/`Timeout`),
> `veiller` → `watch`; the shell's `veilleur.rs` updated in the same
> commit. Internals: `FluxBorne` → `BoundedStream`, `SocketSous` →
> `InnerSocket`, `Speciaux` → `SpecialFolders`, `analyser` → `parse`,
> `lots_bornes` → `bounded_batches`, `LOT_*` → `*_BATCH*`. The
> `is_connection_error` prefix is now `connection ` (D5). The unnamed
> attachment fallback name becomes `attachment.<ext>` (was
> `piece-jointe.<ext>`, a user-visible string generated by the core — D5).
> The French server folder names of the fallback lists stay, marked
> `lang:fr` (they are data). Oracles: build, clippy, 79 + 32 tests green.
>
> **E3c-1 delivered on 2026-09-02** — `store.rs` (10 504 lines, 140 tests) in
> English. Method: the file split in seven chunks at function boundaries,
> translated in parallel by seven Sonnet agents against a fixed rename
> table (101 cross-chunk identifiers), reassembled, then the public
> renames applied to the dependents by whole-identifier replacement
> (`sync`, `nav`, `thread`, `echo`, `outbox`, `backfill`, `compose`, the
> shell). Public API: `SessionNettoyage`/`GroupeNettoyage`/`InvitationRang`
> → `CleanupSession`/`CleanupGroup`/`InvitationRank`, `PERIMETRES_/PLAGES_
> NETTOYAGE` → `CLEANUP_SCOPES/RANGES`, the `nettoyage_*`, `portier_*`,
> `routage*`, `mis_de_cote`, `kiosque`, `mode_organise` methods per the
> glossary; `ecrire_invitation` → `write_invitation` (a database write,
> not a compose — the derived row was wrong); `REGISTRE` (the static of
> initialized databases) → `REGISTRY`, not `PAPER_TRAIL`. Two traps found
> by the nets and fixed the same hour: the whole-identifier pass reached
> the shell — 14 Tauri command names (caught by the IPC contract, E1b)
> and the serialized `dernier_epoch`/`dernier_objet` payload fields the
> UI sorts on (caught by the e2e `retours-14`, reproduced in isolation) —
> both reverted, they are E4's. Oracles: build, clippy, 451 mail-core
> tests, full gate green in 201 s (flaky 1). Baseline 134 436 → 124 290.
>
> **E3c-2 delivered on 2026-09-02** — `sync.rs`, `thread.rs`, `search.rs`,
> `backfill.rs` (4 822 lines, 120 tests) in English, four agents in
> parallel on copies, one file each. Public API: `RepereLocal` → `LocalMarker`
> (fields `uidnext_seen`, `local_messages`, `pending_actions`, `modseq_seen`),
> `faut_relever` → `must_poll`, `SyncReport.refusees/sans_condstore` →
> `refused/without_condstore`; the shell updated in the same commit.
> Internals: `Sortie` → `Output`, `PALIER_RAPPORT` → `REPORT_STEP`, `JOUR` →
> `DAY`. The French fixtures that ARE the test (accent folding, French
> entity names, a prose `References` header, the accented "to:" filter alias) stay,
> marked `lang:fr`. Oracles: build, clippy, 451 mail-core tests, full gate
> green. Baseline 124 290 → 119 883.
>
> **E3c-3 delivered on 2026-09-02** — the rest of `mail-core` (20 source
> files, 13 examples, ~11 600 lines, 191 tests), nine agents in parallel on
> copies. `correspondants.rs` → `contacts.rs`; examples renamed per §5.1
> (`bench_*`, `diag_*`, `seed_arrival`) with the two ADR links and the e2e
> launcher path following. Public API: `Correspondant` → `Contact`,
> `GesteGroupe`/`CibleGeste` → `GroupGesture`/`GestureTarget`,
> `GroupeRegistre` → `PaperTrailGroup`, `InvitationStockee` →
> `StoredInvitation`, `extraire_invitation` → `extract_invitation`,
> `SourceTransfert` → `ForwardSource`, `Error::Refus` → `Refusal`, the
> `InvitationRow`/`CanonicalFolders`/`NavCounts` fields per the glossary; the
> shell reads updated in the same commit, its own serialized payload fields
> and its Tauri command names untouched (E4). `#[error]` messages in English
> (D5). Kept French by decision, marked `lang:fr`: the notification texts,
> the quoting and forward labels composed into message bodies, the size
> units `o`/`Ko`/`Mo` (the e2e specs assert them — to follow the UI language
> at E5, an open point for the CE), the French fixtures that are the test.
> The three seed examples (`seed_clarity`, `seed_inbox`, `seed_arrival`) keep
> their French fixture DATA: they are the e2e decor and the specs assert those
> subjects (`Vantis` 47 times) — a first full translation of them turned
> the e2e wave red (86 passed, 100 did not run) and was reverted; only
> their API calls changed. Their prose goes English at E6 with the specs.
> `mail-core` now carries 124 French markers, all deliberate. Oracles: build,
> clippy, 451 mail-core tests, full gate green. Baseline 119 883 → ~110 100.

Ordre par dépendances : `mail-ical` (345 l.) → `mail-render` →
`mail-smtp` (978 l.) → `mail-auth` → `mail-imap` (2 800 l., dont
`faux_serveur.rs`, `tests_e3.rs`) → **`mail-core`** (~24 000 l., dont
`store.rs` 10 504). Par crate, trois passes dans le même commit :

1. identifiants par le dictionnaire, `cargo build` puis `cargo clippy
   -D warnings` comme oracle, `cargo fmt` ;
2. commentaires et doc-comments traduits (assistés, relus — D9) ; les
   renvois « ADR nnnn », « A-n », « D-n », « PLAN-XXX », « §2.9 »
   restent **tels quels** (numérotation figée, STANDARD en-tête) ;
3. littéraux : messages `#[error]` (D5), traces, textes de diagnostics
   et bancs (`examples/` renommés : `bench_indexing.rs`,
   `diag_opening.rs`, `seed_inbox.rs`…) ; **le SQL ne bouge pas** (D3).

Noms des 664 tests traduits (ce sont des phrases : « a new database has
no phantom column »). `mail-core` seul : 2 à 3 commits (store ; sync +
thread + search ; le reste).

### E4 — Shell + IPC, un seul commit (M)

> **E4 delivered on 2026-09-03** — `apps/desktop/src` (8 552 lines): `commands.rs`
> split in four chunks at struct boundaries, six Sonnet agents in parallel on
> copies, a fixed rename table with a KEEP list. The 36 command names of §5.3
> renamed in the shell, the `generate_handler!` list, the UI `appel()` calls,
> the specs' `invoke()` calls and the two e2e tools that name commands
> (`demarrage.spec.js`, `garde-thread-principal.mjs`) in the same commit;
> `hors_pompe` → `off_pump` with the guard's literal. Files: `veilleur.rs` →
> `watcher.rs`, `demenagement.rs` → `relocation.rs`. Shared state and helpers
> in English (`Recul` → `Backoff`, `VolPasse`/`VolGarde` → `PassFlight`/
> `FlightGuard`, `doit_relever` → `must_poll`, `AppState` fields
> `sync_backoffs`/`poll_locks`/`watchers`/`gesture_passes`/`commands`).
> **Deferred to E5, on purpose (the IPC contract with the UI):** the command
> PARAMETER names (the JSON keys the UI sends: `adresse`, `regle`, `boite`…)
> and the FIELDS of the serialized payloads (`titre`, `dernier_epoch`, `fils`,
> `pieces`…) — they change together with the Svelte reads and the catalogue
> keys, never from the shell side (E3c lesson); the struct NAMES did change
> (`CarteKiosque` → `FeedCard`, `PortierRow` → `ScreenerRow`…). The two
> native dialogs (second instance, failed relocation) stay French, `lang:fr`
> (E5 decides the language of shell-composed text with `human_size`). The
> unnamed attachment fallback of `safe_file_name` is `attachment` (D5, as
> mail-imap at E3b). The shell keeps 33 French markers, all deliberate.
> Oracles: build, clippy, 32 shell tests, IPC contract and main-thread guard
> green, full gate green. Baseline 110 125 → 102 688. Two nets followed the
> rename in the same commit: the System coherence net reads `MARKER_ICONS`/
> `MARKER_HUES` from `commands.rs`, the main-thread guard matches `off_pump(`.
> One shell error string a spec asserts (“connexion IMAP impossible”, the
> onboarding contract test) stays French, `lang:fr`, until E5/E6. **Field (STOP 2) on 2026-09-03: E3c and E4 validated by the CE, no finding** — release launched with trace, both accounts polled, cleanup groups 99 ms cold / 16 ms warm, every screen of the checklist OK.

`commands.rs` (110 commandes, ~45 renommées), `veilleur.rs` →
`watcher.rs`, `demenagement.rs` → `relocation.rs`, `instance.rs`,
`trace.rs`, `telemetry.rs`, `main.rs` (`generate_handler!`) **et** les
appels `appel('…')` de l'UI **au même commit** — E1b vert, e2e vertes.
Les noms de fichiers sur disque (`wind.log`, `maj.log`,
`telemetry.json`, `discovery.db`) ne changent pas (D3).

### E5 — L'UI (G)

> **Investigated on 2026-09-03 (Phase 0, on the evidence).** The UI is
> 14 915 lines: 25 components, 24 `lib/` modules, `main.js`,
> `systeme.css` (300 lines). No case-only rename on NTFS (every renamed
> file changes letters, the six unchanged keep their name). 465 keys in
> `catalogue.fr.js`, 480 rows in `keys.csv`; every key used by the code
> exists in the catalogue; 24 `t()` calls build their key dynamically
> from a VALUE that crosses the IPC (`boite.${dest}`,
> `statut.phase.${phase}`, `inv.puce_${reponse}`, `horizon.${h}`,
> `volets.${n}`, `theme.${id}.nom`). Six test ids are dynamic the same
> way (`kiosque-vers-${dest}`, `tri-${choix.id}`…). CSS tokens:
> `--r-controle` 144 uses in 21 files, `--marque` 61, `--tuile` 54,
> `--r-tuile` 44, `--rep-*` 16 (all in `systeme.css`, read by name by
> `jetons.mjs`, `coherence-systeme.mjs`, `contraste.mjs`). DOM contract:
> 305 test ids, 230 classes, 7 seams; the specs hold 62 class locators
> and hundreds of `[data-testid="…"]` literals. E4 leftovers in
> `commands.rs`: **17 French command parameters** (`cibles`, `non_lus`,
> `perimetre`, `plage`, `regle`, `limite`, `valeur`, `icone`, `teinte`,
> `nom`, `en_ligne`, `actif`, `corps`, `reponse`, `sujet`, `oui`, `non`)
> and **~45 French serialized fields** across 20 payload structs
> (`titre` read 23 times by the UI, `icone` 36, `teinte` 17, `statut`
> 11…); the specs read four of them (`nom`, `boite`, `compte`, `icone`).
>
> **The hard point the handover did not name: the VALUE vocabularies.**
> Five enum-like vocabularies cross the IPC as French strings, are
> persisted in the database (D3), and feed catalogue keys, CSS
> selectors and test ids downstream:
> - the **category ids** `reception`, `envoyes`, `brouillons`,
>   `indesirables`, `archives`, `corbeille`, `kiosque`, `registre`,
>   `portier` (the `list_category` parameter, `NavAccount`, the routing
>   `destination` column) — `keys.csv` already maps `boite.reception` →
>   `mailbox.inbox`, so the dynamic key `boite.${dest}` only resolves if `dest` is
>   `inbox` on the wire;
> - the **12 marker hues** `rouge`… `brun` (`prefs.repere_teinte.N`,
>   `MARKER_HUES`, `data-teinte="bleu"`, `--rep-bleu`, `repere.teinte.bleu`)
>   — D12 already decided `--mk-blue`;
> - the **cleanup scopes** `reception|dossiers|dossiersArchives|archives`
>   (`CLEANUP_SCOPES`, the cleanup session row);
> - the **invitation replies** `accepte|refuse|provisoire` (+
>   `sans_reponse`; `attendee_status` column, `Participation` enum);
> - the **sync phases** `inventaire|fils|brouillons` (transient, not
>   persisted).
> Decision D16 below (**STOP 1 for E5 played on 2026-09-03: D15 “E5d now”, D16 “(a) Boundary maps”, D17 “Keep, debt D-56” — GO**). Because the hue VALUES name the `--rep-<hue>` tokens, the CSS selectors and the System's table, the whole `--rep-*` → `--mk-*` family moves at E5a (A110), E5c keeps the four other tokens. Set-based is not needed: the options differ in
> principle (where the French stops), not on a figure.
>
> **Delivery in four commits, each under the full gate**, the language
> baseline lowered after each (`--update`); one field pass at the end:
>
> - **E5a — the IPC keys and vocabularies** (M): the 17 parameters and
>   ~45 fields renamed in `commands.rs` together with every Svelte/JS
>   read and every `appel('…', {…})` argument object, and the specs'
>   `invoke()` objects (`regle: null`, `destination: 'kiosque'`). The
>   vocabularies per D16. The IPC net does not see keys — the whole e2e
>   wave is the oracle, played before the commit.
> **E5a delivered on 2026-09-03.** `wire.rs` (four two-way tables, category / hue / scope / reply, 4 tests; the sync phases and the six bulk-gesture words are transient and renamed in place); 18 command parameters and ~45 payload fields of `commands.rs` renamed with every Svelte/JS read, every `appel()` argument object and the specs' `invoke()` objects; the value-derived catalogue keys in both catalogues and the `t()` calls (`boite.inbox`, `statut.phase.inventory`, `inv.puce_accepted`, `repere.teinte.blue`, `nettoyage.perimetre.folders`, `horizon.all`) and the value-keyed object literals (`icones.js`, `invitation.js`, `portier.js`, three components); `--rep-*` → `--mk-*` for the twelve hues in `systeme.css`, the System (A110, four glyph captions renamed) and the three nets that parse them; the coherence net reads `WIRE_HUES` from `wire.rs`. Trap caught by the gate (step 2, zero warnings): the whole-identifier pass had renamed CSS selectors inside the Svelte `<style>` blocks and the `class:` directives — restored from HEAD, the DOM contract is E5d's. Traps caught by the fresh-eyes review (ten findings, all fixed): the same pass had rewritten template literals (`status.phase.`, `mailbox.${dest}`, `repere.icon.`, the localStorage key `wind-accueil-fait`), attribute names (`data-teinte`, `data-nom`), and prose comments word by word; two string-literal field reads (`'compte' in quoi`, `de('reception_non_lues')`) had kept the old key; `reply_invitation` wrote the wire word into the database. The e2e wave then caught what no textual net sees: the specs' inline attribute selectors (`data-categorie="reception"`, `data-couleur="bleu"`, `data-groupe="portier"`), the test ids built from a value (`barre-lu`, `gestes-registre`), the object literals keyed by an old value (`GESTES_GROUPE.archiver`, `choisirRepere(…, 'teinte')`), four view test ids equal to a category word swept by the value pass (`kiosque`, `nettoyage`, `portier`, `registre` — their dom.csv rows are done early), and one vocabulary conflated in the shell: the routing RULE `archive` mapped through the category table became `archives` and the core refused the verdict — a fifth table, `RULES`, with its own test. Lesson for E5b: a whole-identifier pass must skip template literals, attribute names, comments and string literals alike; every string-literal key read, every object literal keyed by a value and every selector literal in the specs must be listed by hand; the whole e2e wave before the gate, never after. Oracles: build, clippy, 36 shell tests, the four textual nets, full gate green in 157 s (second pass; the first e2e wave was red on the traps above). Dictionary amended, never patched: `tokens.csv` (+12 rows), `keys.csv` (value segments), GLOSSARY §2 (the hue table and the boundary rule). Commit `d384724`, CI green 33742728494. **Field (STOP 2) on 2026-09-03: validated by the Chief Engineer, no finding** — release launched with trace, both accounts polled (52 + 11 folders), every screen of the checklist OK; the `%APPDATA%\dev.elements.wind` path is the database's, not `%LOCALAPPDATA%`.
>
> - **E5b — the UI itself** (G): 43 files renamed per §5.1 (`git mv`,
>   imports rewritten), identifiers (568 dictionary rows), comments
>   (~34 000 words) — one Sonnet agent per component on a scratchpad
>   copy against the fixed table, then a string-literal-aware
>   whole-identifier pass on the dependents; the **catalogue keys**
>   renamed in both catalogues and the 569 `t()` calls per `keys.csv`
>   (`refonte-langue.spec.js` and the dynamic keys are the oracle);
>   **D4 applied**: `catalog.en.js` becomes the reference and the
>   fallback of `text.svelte.js`, the first-launch default is `en` when
>   the system language is neither, `Lang::from_pref` defaults to `En`,
>   ADR 0016 amended in place, the language spec's default-language test
>   inverted. Oracles: Vite build, eslint `no-undef`, `catalogues.test`,
>   `coherence-systeme`, the e2e.
> **E5b delivered on 2026-09-03.** The UI in English: 38 files renamed (§5.1, `git mv`, every import path rewritten, the four e2e nets that read UI files by path following), ~850 identifiers (the 568 rows of the E0 dictionary plus 282 definitions the E0 inventory had missed — props, params, destructured locals, event-handler props such as `onchoisir` → `onchoose` — found by seven read-only Sonnet agents, one per component group, merged into `dictionary.csv` under one-French-word-one-English-word), the 518 catalogue keys (`keys.csv` completed with 21 rows: the value-derived `inv.*`, the four marker icons and `theme.*-nuit.name` it lacked), the `{placeholders}` of the catalogue VALUES (34 per catalogue, renamed after their param keys — invisible to the user, but a param key renamed on one side only rendered an empty name, an empty organizer and a dateless status; the first e2e wave caught nine of them), and the comments (~1 000 blocks across 51 files, eight Sonnet agents in parallel, one per file group, under a mechanical oracle: every file stripped of its comments must be byte-identical to the snapshot taken before the round — 51 files compared, 0 code difference; a second mechanical pass then mapped the identifiers the agents had kept in backticks through the dictionary). **D4 applied**: `lib/language.js` (one pure decision, `detectLanguage`, RED then GREEN in `e2e/language.test.mjs`), `text.svelte.js` falls back to `EN`, `Lang::from_pref` defaults to `En` (RED then GREEN in `notify.rs`), the language spec's first test retitled (the suite pins `--lang=fr`, the first launch DETECTS French), ADR 0016 amended in place, System A111. Two persisted UI preferences migrate at read, never reset: `wind-largeurs` (`liste` → `list`) and `wind-espacement` (`faible|moyen|eleve` → `low|medium|high`). Bridges listed by hand from the applier's `--report` (E5a lesson): the pane ids, the thread frame values (`volet|plein` → `pane|full`), the drawing of the thread bar, the yes/no menu type, the shortcut ids (`suppr|echap` → `delete|escape`); kept on purpose: the settings group ids, the theme ids, the glyph ids `tri_*` (the System's figcaptions, E5c), the `data-*` attribute names, the CSS classes and the `class:` directive names (E5d), the localStorage keys (D-55). The `__mesure` seam and the `__e2eJournal` record (`{command, start, arrival}`) are English on both sides — five specs and two benches updated. The applier itself is committed (`scripts/rename/apply-ui.mjs`, GLOSSARY §6): a tokenizer, not a regex, with a `--report` mode. Traps met and fixed the same hour: the derivation had produced four JavaScript keywords (`delete`, `default`, `new`, `do`) and the builtin `queueMicrotask`; `etat` → `state` collides with the Svelte `$state` rune in a component (renamed `snapshot()`); a `{#snippet}` took the name of a Map (`pending`); a glyph id (`groupe`) is an object key the dictionary must not touch; a template prefix rewritten in the wrong order (`nettoyage.perimetre.` → `cleanup.perimetre.`, longest prefix first now); the lib inventory generalized a closure name (`poser` → `setThemeAttribute`) into `transport.js`; a first e2e wave played while the passes were still running is worth nothing — the wave that counts is the one on the final tree. Oracles: Vite build (zero warnings), eslint `no-undef`, the System, IPC and contrast nets, `catalogues.test` + `language.test` (4 tests), the comment oracle, then the full e2e wave: wave 1 (played while the passes were still running — worthless) 122 passed / 9 failed; wave 2 on the final tree 192 passed, 2 failed, both spec-side (`poserCran('moyen')` left unmapped, the handle test id built from the pane id `poignee-liste` → `poignee-list`), replayed green as whole files (17/17); the full gate below. UI markers: 96 of 14 804 left, all deliberate (the abbreviation of Chief Engineer, French UI examples quoted in comments, the DOM contract’s class names, the French language name in the English catalogue). Fresh-eyes review (eight angles, Sonnet): ten findings kept, eight fixed the same hour — the spacing legacy map had lost its French key to the applier itself (a tester’s “Moyen” would have reset; the map is now quoted), two French keys derived to one English key (`header.search` twice: the aria-label had become the placeholder sentence — `header.searchHint`), the P1 bench still called `__mesure.ouvrir`, the System net’s icon regexes had matched nothing since E5a (`<Icon name=`, `icon:`), the invitation chip tone classes had not followed the English reply values since E5a (`.ton-accepted`…), D4 was never exercised in a real launch (`refonte-langue.spec` now relaunches on an empty database with the WebView pinned to `de-DE`: the header speaks English — `launchAppV2({ lang })`), no net guarded the catalogue `{placeholders}` against the param keys (`e2e/placeholders.test.mjs`, 3 tests, in the test script), the IPC net accepted the old `appel(` form (it reads `call(` only now). Two findings deferred, on purpose: the list row is named `line` in the UI (the E0 dictionary row) while the glossary rules `row` — a global pass collides with the existing `row` identifiers, it is done with the `ligne-*` test ids at E5d; the list handle’s test id follows the pane id (`poignee-list`) one step ahead of the DOM contract, `dom.csv` starts from it. Baseline 102 638 → 87 926. Full gate green in 167 s (198 e2e passed, flaky 0). Commit `59c6ee1`, CI green 33759685493. **Field (STOP 2) on 2026-09-03: validated by the Chief Engineer, no finding** — release launched with trace, both accounts polled (52 + 11 folders), one send flushed, the ten steps of the checklist OK, the trace without panic nor missing-key warning.
> - **E5c — the CSS tokens** (P, D12): `systeme.css` → `system.css`,
>   the tokens of §5.5 (the `--rep-*` family becoming `--mk-*` for all
>   12 hues under D16 (a)) in the CSS, every `var(--…)` of the
>   components, `theme.js`, `systeme.dc.html` (A110, DC-D2), and the
>   three nets that parse them by name — one commit.
> **E5c delivered on 2026-09-03.** One mechanical pass (a regex bounded on the token name, `--tuileInk` before `--tuile`): `--r-controle` → `--r-control` (144 uses), `--marque` → `--brand` (61), `--tuile` → `--tile` (54), `--tuileInk` → `--tileInk` (37), `--r-tuile` → `--r-tile` (44) in the 25 components, `system.css` (renamed from `systeme.css`, `git mv`, the import in `main.js`), the System (the `:root` blocks, the contract table’s `data-jeton` cells, the prose) and the three nets — the contrast net carried the names in its pair table, the coherence net in its form-token list. The four sort glyphs of A104 follow the sort they draw (`sort_newest`, `sort_oldest`, `sort_az`, `sort_za`: `icons.js` keys, `SectionSort` icon names, the System’s figcaptions). System A112. Fifteen comments and the gate skill named the stylesheet; they say `system.css`. Values, contrast table and the zero-radius rule untouched: nothing visible changes. Oracles: build zero warnings, eslint, the System net (68 token values, the doc says the delivered), the contrast net (440 pairs); the first full gate was red on one spec that asserted the glyph ids by prefix (`svg[data-nom^="tri_"]`, the value-built selector of the E5a lesson) — updated, replayed as a whole file, full gate green in 145 s (198 e2e, flaky 0). Commit and CI recorded with the field verdict.
> - **E5d — the DOM contract** (M, D15): 305 test ids, 230 classes,
>   7 seams renamed in the Svelte (markup, `<style>`, `class:` directives)
>   and in the specs' selector literals in the same commit, `dom.csv`
>   as the single table; `.repere-nu` in the two nets follows.
>
> Not in E5: the spec FILE names, identifiers and comments (E6); the
> French `catalogue.fr.js` values (D3); the `localStorage` keys (D-54);
> the Material icon names (already English); the `inv.*` keys built from
> a reply value follow D16.

### E6 — e2e et scripts (M)

29 specs renommées (`refonte-ecran02.spec.js` → `redesign-screen02.spec.js`…),
outils `.mjs`, `sonde-gel.py` → `freeze-probe.py`, `mesure-ram.ps1`,
`bascule-sombre.ps1` ; identifiants, commentaires ; les 2 ancrages sur
libellés français restent (ils testent le fr, qui reste livré) ; le
`README.md` d'e2e. `playwright.config.js`, `launch.mjs`, `flaky.mjs`.

### E7 — Documents vivants (G)

Dans cet ordre, chacun son commit, E1d vert : `README.md` ;
`STANDARD.md` (structure §0-§10 **intacte**, numérotation figée) ;
`WORKFLOW.md` ; `ETAT.md` (réécrit de toute façon au solde — traduire
la version d'alors) ; `DETTE.md` (D-1…D-53, numéros intacts) ;
`PLAN.md` ; `BETA.md` (D11) ; `AUDIT-2026-09-01.md` ; `PASSATION.md` ;
`CHANGELOG.md` (D13 — l'en-tête et `## [0.17.0]` au minimum ;
`make-release.ps1` continue de lire `## [x.y.z]`) ; **31 ADR** traduits
et renommés (`0008-conversation-grouping.md`), liens corrigés ; les 30
`PLAN-*.md` non archivés : les soldés partent en `archives/` d'abord
(c'est leur place, PLAN-DOCUMENTATION), puis D1 s'applique ; le présent
plan et PLAN-AUDIT-V2 (en cours) traduits. `ANNOTATIONS-V3.md`,
`assets/icones/README.md`.

### E8 — Le Système et la carte d'architecture (G)

`systeme.dc.html` (~37 000 mots) : traduire la prose **en place**, le
journal A1-A109 gardant ses numéros et ses dates, la table des jetons
gardant ses valeurs (gate `coherence-systeme`) ; revue visuelle CE par
`launch.json` `maquettes-design`. Nouvel amendement **A110** au journal :
« le Système est rédigé en anglais depuis le … ». `architecture/index.html`
(~4 300 mots). D8 tranche s'il faut une version V-n neuve ou la même
en place.

### E9 — Archives (selon D1)

Gel : un bandeau anglais en tête de chaque fichier (« Historical
record, French, closed on … ») et exclusion du filet E1a ; ou
traduction complète (6 804 lignes + les PLAN soldés déplacés à E7).

### E10 — Mémoire, solde (P)

17 fichiers de mémoire et `MEMORY.md` traduits, pointeurs vers les
scripts renommés corrigés ; `/close` : ETAT, DETTE (ce qui reste
français par décision — D3 — entre au registre comme dette **assumée**,
avec ce qui la rouvrirait), chiffres kaizen (T1, W3, KO du STOP 2).

### Estimation, à re-mesurer après E3a

E0 M + E1 M + E2 P + E3 G×2 + E4 M + E5 G + E6 M + E7 G + E8 G + E9
(0 ou G) + E10 P ≈ **12 à 16 jours de chantier**, sur des commits
indépendants : le chantier **s'interrompt sans dommage** à chaque
frontière (un retour bêta, une release). Le débit réel du premier crate
corrige ce chiffre au STOP intermédiaire proposé à D10.

## 5. Refus explicites (§2.6)

- **Pas de réécriture de l'historique git.** 488 commits restent
  français ; une seconde réécriture en deux jours, pour une langue,
  n'apporte rien à l'utilisateur et rouvre le ticket support.
- **Pas de migration de schéma dans ce chantier** (sauf D3 contraire) :
  renommer 9 tables et ~30 colonnes sur les bases des testeurs est une
  migration rembobinable à écrire, tester, jouer au terrain (ADR 0012) —
  un chantier à part entière, sans gain visible. Le SQL reste français
  derrière des fonctions Rust anglaises ; la dette s'écrit.
- **Pas de changement de comportement** embarqué : un commit de
  renommage ne corrige pas un bug qu'il croise — il l'écrit à DETTE.
- **Pas de bibliothèque i18n**, pas de troisième langue (ADR 0016 tient).
- **Pas de release dédiée** : la bascule part avec la prochaine
  MINEURE ; aucune n'est déclenchée pour elle (sauf D4 ⇒ MINEURE de toute
  façon, §2.9).
- **`spikes/` intouchés** : jetables, hors workspace ; seul
  `spikes/ui-socle-v2/RAPPORT.md` est cité (STANDARD §10) — le lien
  reste, le rapport reste français.
- **Pas de renommage des fichiers sur disque** (`wind.db`, `wind.log`,
  `maj.log`, `telemetry.json`) : ils sont documentés aux testeurs
  (BETA.md) et lus au terrain.

## 6. Décisions CE — à trancher une à une au STOP 1

> **STOP 1 joué le 2026-09-02 en deux temps** : quatre décisions le
> matin (D1, D3, D4, D10), les dix autres le jour même après la
> publication de 0.16.0 et 0.17.0 et le solde de PLAN-AUDIT-V2 —
> **GO CE le 2026-09-02** (« Tu peux lancer les travaux
> d'implémentation »). Toutes consignées mot pour mot ci-dessous.
> Prochain arrêt : **STOP 1 bis**, validation du glossaire (D14).

| # | Question | Recommandation | Décision CE (mot pour mot, datée) |
|---|---|---|---|
| D1 | Archives (`docs/archives/`, 29 fichiers, 6 804 l.) et PLAN soldés : **geler** avec bandeau anglais, ou **traduire** ? | Geler : clos, jamais relus par la méthode (§0 : on lit STANDARD, ETAT, PLAN, ADR) ; ~40 % du volume doc pour zéro valeur vivante |**2026-09-02 : « Geler avec bandeau »** — dette D-55 à l'ouverture |
| D2 | Commits : anglais dès le premier commit du chantier (E2 amende §2.8) ; le corps porte toujours chiffres et raisonnement | Oui ; convention « sans accents » abolie au même amendement |**2026-09-02 : « Oui, en bloc »** (recommandation prise telle quelle) |
| D3 | Identifiants persistés (schéma SQLite, clés `prefs`, fichiers disque) : **garder** le français derrière des API anglaises, ou migrer ? | Garder ; dette D-54 « SQL français » avec clause de réouverture (« si une migration de schéma s'ouvre pour une autre raison, y adosser les renommages ») |**2026-09-02 : « Garder, dette D-54 »** |
| D4 | Langue par défaut de l'UI : aujourd'hui « système si couvert, sinon fr ». Passer à « système si couvert, sinon **en** », l'anglais devenant référence/repli des catalogues ? | Oui : c'est la conséquence logique ; effet visible uniquement sur un système ni fr ni en ⇒ MINEURE (§2.9) ; fr reste livré mot pour mot |**2026-09-02 : « Oui, en par défaut »** — la prochaine release est MINEURE |
| D5 | Messages techniques (`#[error]`, traces `wind.log`, diagnostics) : anglais seul, l'enveloppe UI restant traduite (L-3 de PLAN-LANGUES) ? | Oui ; un utilisateur fr verra un détail technique anglais après « Transfert impossible : … » — comme aujourd'hui les erreurs serveur |**2026-09-02 : « Oui, en bloc »** |
| D6 | Vocabulaire produit dans le code : les mots de `catalogue.en.js` (Feed, Screener, Paper trail, Set aside, Marker, Clean) plutôt que le littéral (Kiosk, Doorman, Register) ? | Les mots du catalogue : déjà tranchés CE, un seul vocabulaire des deux côtés de l'écran |**2026-09-02 : « Oui, en bloc »** |
| D7 | Noms de fichiers ADR / PLAN / specs : renommer en anglais (liens corrigés par E1d) ou garder les noms français ? | Renommer ; les numéros (`0008-`, `A-n`, `D-n`) sont l'identité, pas le slug |**2026-09-02 : « Oui, en bloc »** |
| D8 | Système : traduction en place (A110) ou nouvelle version V-n ? | En place : une version neuve implique un contrat de jetons neuf, ce n'est pas le cas |**2026-09-02 : « Oui, en bloc »** |
| D9 | Relecture : les normatifs (STANDARD, WORKFLOW, ADR, Système, skills) relus **intégralement** par le CE ; commentaires de code et plans relus par échantillon (10 %) et par les oracles ? | Oui ; la dérive de sens d'un normatif est le seul risque que les oracles ne voient pas |**2026-09-02 : « Normatifs en entier »** |
| D10 | Séquencement : après la publication 0.16.0 et le solde de PLAN-AUDIT-V2, avant la vague 3 de l'audit ; sur `main`, un commit par couche ; **STOP intermédiaire** après E3a (premier crate) pour re-mesurer le débit ? | Oui ; un chantier qui touche chaque fichier ne cohabite avec aucun autre |**2026-09-02 : « Après 0.16.0 et le solde d'AUDIT-V2 »** — rien ne commence avant, E0-E1 compris |
| D11 | Documents des testeurs (`BETA.md`, mot d'invitation, guide) : conserver une copie française `docs/BETA.fr.md` tant que la vague 1 (T1-T5) court ? | Oui si les cinq testeurs lisent le français (fait connu du CE seul, mémoire « identités hors dépôt ») |**2026-09-02 : « Oui, BETA.fr.md conservé »** |
| D12 | Jetons CSS français (`--marque`, `--r-controle`, `--rep-*`) : renommer (trois fichiers, gate DC-D6) ou tolérer ? | Renommer à E5, en un commit dédié — le Système est la référence de l'UI, il ne peut pas rester mixte |**2026-09-02 : « Oui, en bloc »** |
| D13 | `CHANGELOG.md` : traduire tout (758 l., public, lu par la release) ou l'en-tête + entrées à venir ? | Tout : un journal public bilingue est illisible ; les Releases déjà publiées gardent leurs notes |**2026-09-02 : « Oui, en bloc »** |
| D14 | Le glossaire E0 : relu et **validé CE avant E2** (c'est la conception du chantier) ? | Oui : STOP 1 bis, une heure, sur le tableau des mots |**2026-09-02 : « Oui, je valide le glossaire »** — STOP 1 bis joué le 2026-09-02 : « Validé tel quel » |
| D15 | The DOM contract (305 test ids, 230 classes, 7 `__e2e*` seams): renamed at **E5d** with the specs' selector literals in the same commit, or kept French until E6 (spec files renamed then)? | E5d now: the Svelte files would otherwise stay half French (a test id is a marker for the ratchet), and the selector literals are exact strings replaced mechanically from `dom.csv` — the same move as the 36 command names at E4 |**2026-09-03: “E5d now”** |
| D16 | The five VALUE vocabularies that cross the IPC and are persisted (category ids, 12 marker hues, cleanup scopes, invitation replies, sync phases): **(a)** translate at the shell boundary — the database keeps the French value (D3), the wire, the catalogue keys, the CSS selectors and the test ids carry the English one, five small two-way maps in the shell with round-trip tests; **(b)** keep the French value on the wire and downstream (`data-hue="bleu"`, `mailbox.reception`, `--mk-bleu`), amending `keys.csv` and §5.5 accordingly; (c) migrate the values in the database — refused by D3 | (a): D12 already decided `--mk-blue`, and `keys.csv` already decided `mailbox.inbox`; (b) leaves French in the English UI's DOM for good. Cost of (a): ~120 lines of Rust, 5 tests; risk: a value missed in a map, caught by the coherence net and the e2e (`repere-ligne`, `mode-organise`, `nettoyage`, `refonte-invitations`) |**2026-09-03: “(a) Boundary maps”** |
| D17 | Shell-composed text now that the UI language is known to the shell: the size units `o`/`Ko`/`Mo` of `human_size` (shown in attachments, drafts, the outbox), the two native dialogs, the one asserted error string: **keep French with `lang:fr`** and write debt D-56 (a later small job: send bytes, format in the UI), or fold it into E5a? | Keep and write the debt: it is a behavior change (formatting moves to the UI), §5 refuses embedded behavior changes; an English user sees "Ko" for one more release |**2026-09-03: “Keep, debt D-56”** |

## 7. Checklist terrain (STOP 2) — ce que le CE joue

Sur ses vrais comptes, sur la base **existante** (aucune migration
attendue — c'est le premier contrôle) :

| # | Geste | Attendu |
|---|---|---|
| T1 | Lancer Wind après la bascule sur `wind.db` réel | pas de modale de migration ; `migration_check` inchangé ; ouverture < 1 s |
| T2 | Réglages > Affichage : fr, puis en, puis fr | chaque écran majeur dans la langue ; aucune clé brute `xxx.yyy` visible (clés renommées, catalogues alignés) |
| T3 | Portier, Kiosque, Registre, Nettoyage, Mis de côté, Repères : un geste chacun | comportement identique à 0.17.0 ; `wind.log` porte une ligne par geste, en anglais, sans PII (§6.8) |
| T4 | Notification d'arrivée en fr et en en | texte de la langue courante ; défaut selon D4 |
| T5 | Un envoi, une réponse à invitation, une pièce jointe enregistrée | identiques ; erreur provoquée (SMTP coupé) : enveloppe fr + détail anglais (D5) |
| T6 | `scripts\make-release.ps1` en **dry-run** (branche ≠ main pour qu'il refuse) | refuse en anglais ; lit `## [x.y.z]` du CHANGELOG traduit |
| T7 | `git push` d'un commit docs seul | hook : chemin rapide documentaire, E1a et E1d joués |
| T8 | Casser volontairement un `appel('…')`, un lien md, un commentaire français | E1b, E1d, E1a rouges — puis rétablir |
| T9 | RAM et démarrage (`e2e/measure-ram.ps1`, banc de démarrage) | dans les budgets §3 ; un renommage ne bouge pas un chiffre |

Un constat KO ⇒ correction le jour même, re-gate, re-terrain (§2.5).

## 8. Risques nommés

- **Dérive de sens dans un normatif** (STANDARD, ADR, Système) — D9.
- **Collision d'identifiants** au renommage mécanique (`fil`/`thread`,
  `nom`/`nombre`) — dictionnaire sur identifiants entiers, compilateur.
- **JS sans compilateur** : un renommage raté ne se voit qu'à
  l'exécution — E1c mesuré, e2e complètes, E1b pour l'IPC.
- **E2E flaky en local** (mémoire) : un rouge local se contre-vérifie
  par `gh run list` avant de suspecter le renommage.
- **Liens cassés** dans 104 fichiers markdown — E1d.
- **NTFS et la casse** : `git mv` en deux temps pour tout renommage par
  la casse seule ; OneDrive peut retenir un handle sur un fichier
  renommé — jouer les renommages en masse dans un lot, vérifier
  `git status` propre.
- **Un chantier long qui traverse la bêta** : chaque commit est
  livrable ; un retour testeur passe devant, en `/field`, sur `main`.
- **Le glossaire figé trop tôt** : un mot mal choisi coûte un second
  passage sur toutes les couches — D14, STOP 1 bis.

## 9. Dette prévue

- **D-54** (si D3 = garder) : schéma SQLite, clés `prefs`, fichiers
  disque en français derrière des API anglaises ; rouvre si une
  migration de schéma s'ouvre pour une autre raison.
- **D-55** (si D1 = geler) : archives françaises, bandeau en tête ;
  rouvre si un lecteur anglophone en a besoin (bêta ouverte, contributeur).
- Les Releases 0.11-0.15 gardent des notes françaises — pas une dette,
  un fait historique.
