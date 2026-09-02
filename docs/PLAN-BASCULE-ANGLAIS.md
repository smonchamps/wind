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

`commands.rs` (110 commandes, ~45 renommées), `veilleur.rs` →
`watcher.rs`, `demenagement.rs` → `relocation.rs`, `instance.rs`,
`trace.rs`, `telemetry.rs`, `main.rs` (`generate_handler!`) **et** les
appels `appel('…')` de l'UI **au même commit** — E1b vert, e2e vertes.
Les noms de fichiers sur disque (`wind.log`, `maj.log`,
`telemetry.json`, `discovery.db`) ne changent pas (D3).

### E5 — L'UI (G)

- 25 composants renommés (`Reglages` → `Settings`, `Nettoyage` →
  `Cleanup`, `Kiosque` → `Feed`, `Portier` → `Screener`, `Registre` →
  `PaperTrail`, `PileMisDeCote` → `SetAsidePile`, `GuichetCompte` →
  `AccountDesk`, `FenteAvis` → `NoticeSlot`, `Fil` → `Thread`,
  `BarreFil` → `ThreadBar`, `Liste` → `List`, `Lecture` → `Reading`,
  `Retour` → `Back`, `Marque` → `Brand`, `DrapeauUE` → `EUFlag`,
  `Icone` → `Icon`, `TriSection` → `SectionSort`, `ModaleMigration` →
  `MigrationModal`, `Conversation`, `Menu`, `Nav`, `Toast`, `Onboarding`,
  `Composition` → `Compose`, `App`) et les 24 modules `lib/`. Sur NTFS
  un renommage **par la casse seule** exige deux `git mv` — inventorier
  avant.
- Identifiants (455 définitions), commentaires (~34 000 mots).
- **Clés des catalogues** renommées dans `catalogue.fr.js`,
  `catalogue.en.js` et les 569 `t()` — `refonte-langue.spec.js` (clés
  identiques) est l'oracle. L'anglais devient la **référence et le
  repli** (`texte.svelte.js`, ADR 0016 amendé) ; la langue par défaut
  suit D4 ; `Lang::from_pref` (`notify.rs`) suit.
- Les 15 jetons CSS : renommés dans `systeme.css` + `theme.js` +
  `systeme.dc.html` au même commit (DC-D2, gate `coherence-systeme`) —
  ou laissés tels quels si le CE juge `--marque`/`--rep-bleu` tolérables
  (à trancher à E0, ligne du glossaire).
- Aucun libellé fr visible ne change : la table `FR` est le prototype,
  mot pour mot (PLAN-LANGUES) — seules ses **clés** bougent.

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
