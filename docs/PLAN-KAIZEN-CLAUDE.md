# PLAN-KAIZEN-CLAUDE — optimiser l'usage de Claude Code sur Wind

> Chantier kaizen ouvert le 2026-08-23, sur l'audit des 46 sessions du
> 11 au 23 août (extraction déterministe des transcripts + analyse
> multi-agents avec vérification adversariale sur pièces du dépôt ;
> 7 recommandations sur 44 rejetées à la vérification). Objet : baisser
> le coût en tokens, le temps de traitement des prompts et le temps
> d'exécution du workflow /chantier→/gate→/solde, **sans toucher au
> niveau de qualité** — la gate complète avant commit, la CI verte, le
> TDD montré et le STOP 2 terrain sont des invariants, pas des
> variables d'ajustement.

---

## Constat — baseline mesurée (2026-08-11 → 2026-08-23, 12 jours)

### Volumes

| Mesure | Valeur |
|---|---|
| Sessions / prompts CE / tours assistant | 46 / 478 / 17 479 (**36,6 tours par prompt**) |
| Coût total (équivalents input : cacheRead ×0,1, cacheCreate ×1,25, output ×5) | **876 M** — cacheRead 68 %, cacheCreate 18 %, output 14 % |
| cacheRead brut | 5,96 Md de tokens ; **top 10 sessions = 62 %** du volume |
| Contexte moyen relu par tour | marathons 410–540 k ; sessions courtes 75–140 k |
| Sessions compactées / closes proprement | 2 compactions sur 46 ; sessions de 90,4 h, 37,3 h, 26,3 h, 25,4 h de mur |
| Chantiers /chantier sur la période | 15 invocations, ~14 chantiers soldés → **~60 M équiv. input par chantier** |

### Temps

| Mesure | Valeur |
|---|---|
| Latence API par appel | ~3 s médiane, **plate** de 106 k à 534 k de contexte (le cache paie) |
| Lancements e2e | 243 ; parmi les 85 > 30 s : médiane 74 s, p90 159 s, max 217 s |
| `git push` (hook pre-push rejoue la gate) | 42 > 30 s, médiane 118 s, max 164 s — au premier plan |
| `gh run watch` / veille CI | 28 > 30 s, médiane 141 s ; ~20 min de mur bloqué par journée dense |
| Gates complètes par chantier | jusqu'à 10+ ; chaque gate = 9 appels d'outil séquentiels (8 tours d'orchestration perdus) |
| Rebuild par lancement e2e | `construireV2` bump le mtime de `main.rs` → recompile + link à chaque lancement, même sans changement |

### Agents et modèles

| Mesure | Valeur |
|---|---|
| Lancements d'agents (Agent tool) | 85 sur la période ; 141 transcripts de sous-agents, 5 301 appels API |
| Coût des agents | **92 M équiv. input, soit ~9,5 % du total** (968 M fil principal + agents) — gisement secondaire, volume déjà sain (doctrine un-seul-fil + spike) |
| Modèle des sous-agents | **100 % haut de gamme** (Opus 5 : 3 098 appels, Fable 5 : 2 100) — y compris les agents d'exploration |
| Modèle du fil principal | Fable 5 : 72 % des messages, Opus 4.8 : 28 %, Sonnet 5 : ~0 % — le mécanique (docs, releases, Notion, veille CI) tourne au tarif maximal |
| Gaspillage agent identifié | timing, pas volume : 2 revues high-effort (~8 agents chacune) payées sur des designs ensuite invalidés par la mesure |

### Pertes récurrentes identifiées (avec preuve, cf. audit)

- Sessions multi-chantiers jamais closes : le contexte d'un chantier
  soldé est refacturé à chaque tour du suivant.
- 9 lancements `/chantier` à vide → un aller-retour perdu chacun.
- ~15 redemandes des commandes PowerShell du STOP 2, après codification.
- 11 re-runs de suite complète pour trancher UN flake (la règle
  spec-en-isolation existait déjà).
- 2 revues high-effort payées sur des designs ensuite invalidés par la
  mesure (chantier recherche) ; 1 chantier UI complet annulé au terrain
  (barre de tri) ; ~1 M tokens jetés sur perf-lecture sans STOP
  intermédiaire mesuré.
- Frictions PowerShell 5.1 en cascade (one-liners régénérés au lieu de
  scripts versionnés).

---

## Objectifs chiffrés — horizon : bilan le 2026-09-06 (2 semaines)

Trois axes, neuf indicateurs. La référence est la fenêtre du 11–23 août ;
la mesure de contrôle est la fenêtre 24 août – 6 septembre, ramenée au
chantier soldé pour neutraliser les variations d'activité.

### Axe T — tokens

| Indicateur | Baseline | Cible | Levier principal |
|---|---|---|---|
| T1. Équiv. input **par chantier soldé** | ~60 M | **≤ 35 M (−40 %)** | T2+T3+T4 combinés |
| T2. Contexte moyen relu par tour (toute session) | 410–540 k (marathons) | **≤ 200 k** | /solde = frontière de session ; /compact aux STOP |
| T3. Sessions closes ou compactées ≤ 24 h de mur ; sessions multi-chantiers | 8+ marathons ; multi-chantiers courant | **100 % ; zéro** | étape finale de /solde |
| T4. Tours assistant par prompt CE | 36,6 | **≤ 25 (−30 %)** | gate scriptée (−8 tours/gate), vagues groupées |
| T5. *(optionnel, validé CE 2026-08-23)* Tokens de sortie par session, à activité comparable | réf. semaine 1 | **essai mesuré** : adopté seulement si baisse sans perte de qualité | output style `Concise` (portée utilisateur) |

### Axe P — temps de traitement des prompts, qualité constante

| Indicateur | Baseline | Cible | Levier principal |
|---|---|---|---|
| P1. Mur bloqué au premier plan sur commandes > 60 s | ~3,5 h / 12 j (push, watch, e2e) | **≤ 15 min / 2 sem.** | arrière-plan systématique (Monitor), Claude annonce le verdict CI |
| P2. Re-runs pour trancher un flake e2e | jusqu'à 11 | **≤ 2** (spec entier en isolation, une fois) | rappel de conformité /gate ; retries:1 |
| P3. Allers-retours évitables (/chantier vide, redemande STOP 2) | 9 + ~15 | **0** | énoncé en argument ; non-conformité signalée |
| Garde-fou qualité (ne doit PAS se dégrader) | — | constats KO au STOP 2 par chantier et CI rouges : **stables ou en baisse** | invariants inchangés |

### Axe W — temps d'exécution du workflow (121 e2e)

| Indicateur | Baseline | Cible | Levier principal |
|---|---|---|---|
| W1. Gate complète (mur, chronométrée) | **4 min 34 s** (mesure W0 du 2026-08-23, cache cargo chaud, 121/121 e2e — l'estimation 9–12 min était pessimiste) | **≤ 6 min** (déjà tenue à W0 — resserrer la cible est un arbitrage CE au bilan) | rebuild mémoïsé, gate.ps1, nextest (si mesuré gagnant) |
| W2. Boucle intérieure : 1 spec e2e | méd. 74 s (dominée par rebuild) | **≤ 45 s** | rebuild mémoïsé + bump conditionnel |
| W3. Gates complètes par chantier | 10+ | **≤ 3** | boucle ciblée codifiée + re-gate partielle |
| W4. Temps de gate cumulé par chantier | ~100 min | **≤ 25 min** | W1×W3 |
| W5. Push documentaire (docs-only) | ~2 min (gate entière) | **≤ 30 s** | chemin rapide pre-push |

### Axe M — modèles et agents (validé CE le 2026-08-23)

| Indicateur | Baseline | Cible | Levier principal |
|---|---|---|---|
| M1. Part du coût sur modèle haut de gamme **hors chantiers** (sessions mécaniques + agents d'exploration) | ~10–15 % du total | **≤ 5 %** | règle « session mécanique = Sonnet 5 » ; agents d'exploration abaissés |
| M2. Revues high-effort par chantier | jusqu'à 3 (dont 2 sur designs jetés) | **1, à la convergence** | déjà porté par la vague 1 (T1) |
| Garde-fou | — | les chantiers (conception, racine, TDD) restent sur Fable 5 — jamais de conception dure sur modèle moindre (précédent perf-lecture, non prouvé mais suspect) | — |

Gain attendu de l'axe M : **−10 à −15 % du coût total**, cumulable avec
l'axe T, sans toucher à la qualité des chantiers. Le *nombre* d'agents
n'est pas un levier : 9,5 % du coût, et les spikes set-based comme les
revues multi-angles sont les meilleurs détecteurs de défauts du
workflow (c'est une revue qui a attrapé la reconstruction d'index FTS5
de ~13 Go).

---

## Contre-mesures — trois vagues

### Vague 0 — mesure de référence (avant tout changement, ½ journée)

1. Verser `scripts/mesurer-sessions.mjs` (adaptation du script d'audit :
   tokens, tours, contexte moyen, commandes > 30 s par catégorie, par
   session) — on ne pilote que ce qu'on mesure.
2. Chronométrer UNE gate complète de référence, cache cargo chaud
   (STANDARD §9 : le cache chaud ment, noter l'état du cache) → fige W1.

   **✓ Fait le 2026-08-23** (cache cargo chaud, aucun changement de code
   en vol) : **total 274,3 s (4 min 34 s)** — fmt 0,7 s, build-ui
   2,8 s, contraste 0,3 s, cohérence 0,3 s, garde-thread 0,3 s, clippy
   2,9 s, tests Rust 9,2 s (--all-targets), doc 1,3 s, **e2e 256,3 s
   (121/121)**. L'e2e est 93 % du mur : le levier dominant est bien la
   vague 2 (rebuild mémoïsé, base gabarit seed) — pas gate.ps1 (les 8
   premières étapes ne pèsent que 18 s, l'orchestration en tours
   d'outil reste le gain de gate.ps1, −8 tours/gate).

### Vague 1 — comportements et skills, zéro code produit (jour 1, un commit `chore:` par amendement)

| # | Contre-mesure | Fichier(s) | Indicateurs servis |
|---|---|---|---|
| 1 | `/solde` : dernière étape « écrire l'entrée CHANGELOG (si release à venir), puis **clore cette session** ; le sujet suivant s'ouvre sur ETAT.md » | `.claude/skills/solde/SKILL.md` | T1 T2 T3 |
| 2 | `/chantier` et `/terrain` : boucle intérieure ciblée — spec(s) impactée(s) **en fichier entier** (jamais `-g`), 2 runs groupés par vague (RED groupé, GREEN groupé) ; gate complète UNE fois avant commit | `chantier/SKILL.md`, `terrain/SKILL.md`, phrase au STANDARD §2.4 | W3 W4 T4 |
| 3 | `/gate` : re-gate partielle après un rouge corrigé (étape rouge + ce que la correction peut impacter, amont compris si Rust) ; gate complète finale avant commit inchangée | `gate/SKILL.md`, `chantier/SKILL.md` | W3 W4 |
| 4 | `/gate` et `/chantier` Phase 5 : push + `gh run watch` **en arrière-plan**, verdict annoncé par la session ; jamais d'attente CI au premier plan | `gate/SKILL.md`, `chantier/SKILL.md` | P1 |
| 5 | `/chantier` : STOP visuel précoce (UI : verdict d'apparence après le premier incrément TDD minimal) ; STOP mesuré précoce (perf : mesure avant/après au premier incrément, arbitrage CE) | `chantier/SKILL.md` | T1 P3 |
| 6 | Discipline CE (sans commit) : énoncé complet en argument de `/chantier` ; pièce à conviction au premier énoncé ; non-conformité signalée plutôt que redemandée ; une seule session écrivante à la fois | — | P3 T4 |
| 7 | Politique de modèles dans WORKFLOW.md : **chantier = Fable 5** (invariant) ; **session mécanique** (docs/ETAT/CHANGELOG, Notion, veille CI, release scriptée, consolidation mémoire) **= Sonnet 5** ; préserve aussi le quota Fable pour les chantiers | `docs/WORKFLOW.md` | M1 |
| 8 | Agents d'exploration/recherche abaissés (Sonnet 5, Haiku pour du pur balayage) ; agents de vérification, de revue et `spike` inchangés (haut de gamme / modèle de session) | `.claude/agents/`, WORKFLOW.md | M1 |

### Vague 2 — petits chantiers techniques (semaine 1, ordre de rentabilité)

| # | Contre-mesure | Gain attendu | Fichier(s) |
|---|---|---|---|
| 1 | Mémoïser `construireV2` par processus de suite + bump de `main.rs` conditionné au hash du dist **et** de tauri.conf.json | 3–8 min/suite ; 25–40 s/spec ; porte W1 et W2 | `e2e/rebuild-v2.mjs`, `e2e/launch.mjs` |
| 2 | `scripts/gate.ps1` fail-fast, 9 étapes dans l'ordre du hook, **sans** les redirections `/dev/null` (le verdict chiffré doit sortir) ; `/gate` l'exécute en un appel | −8 tours/gate ; porte T4 | nouveau script + `gate/SKILL.md` |
| 3 | `retries:1` dans Playwright + tout flaky consigné au verdict de gate (indissociables : un flaky ne rend pas le run rouge) ; andon = rouge franc | 5–15 min/flake ; porte P2 | `e2e/playwright.config.js`, `gate/SKILL.md` |
| 4 | Chemin rapide docs-only du pre-push : sauter les étapes 6–8 (clippy, tests Rust, e2e) si le diff ⊆ `docs/**` + `*.md`, en **excluant `docs/design/**`** (DC-D6) ; garder les étapes en secondes | W5 | `.githooks/pre-push` |
| 5 | `scripts/terrain.ps1` + `scripts/lancer-wind.ps1` compatibles PS 5.1 (CLIENT_ID, chemins OneDrive-sûrs, traces UTF-8 écrites par l'app) | supprime la classe de frictions terminal | nouveaux scripts, référencés au STOP 2 |
| 6 | Base gabarit seed copiée par spec au lieu de ~14 `cargo run --example` par suite | 15–35 s/suite | `e2e/launch.mjs` |
| 7 | `cargo-nextest` sur `--all-targets` : **mesurer avant/après** (gain attendu inter-binaires, ~20 binaires) ; adopter seulement si le chiffre le justifie ; `--doc` inchangé | ~15–25 s/gate si confirmé | `gate.ps1`, pre-push, ci.yml |

#### Déroulé de la vague 2 — **SOLDÉE le 2026-08-23, terrain complet** (ordre D3)

> Commits `ceb59c4` (les 7 contre-mesures) + `a3ed285` (fraîcheur TTL
> des gabarits — rouge payé à la gate du push, corrigé dans la
> session). GO terrain CE le 2026-08-23 (checklist 3/3 : terrain.ps1,
> lancer-wind.ps1 avec trace prouvée, spec 30,3 s). CI verte run
> 32642956082. **Chiffres kaizen du chantier** : 6,8 M équiv. input
> (T1 ; baseline ~60 M/chantier), contexte moyen 181 k/tour (T2 ✓),
> 4 gates complètes jouées (W3 — dont 1 rouge pre-push), 0 KO au
> STOP 2, 0 CI rouge (garde-fou ✓).

| # | Verdict | Mesure |
|---|---|---|
| 1 | **✓ livré** — `empreinteDist` (dist + conf, sha1) + bump conditionné + mémo par processus de suite (`rebuild-v2.mjs`, 4 tests node) | **W2 : 74 s → 13,5–19 s** de mur la spec (`refonte-retours-7`, cache chaud) |
| 2 | **✓ livré** — `scripts/gate.ps1`, 9 étapes fail-fast, verdict chiffré par étape, exceptions PS rendues en rouge nommé ; `/gate` l'appelle en un tour | −8 tours d'orchestration par gate (T4) |
| 3 | **✓ livré** — `retries: 1` + « flaky = consigné au verdict, rouge franc = andon » gravé au skill | porte P2 |
| 4 | **✓ livré** — chemin rapide docs-only du pre-push (⊆ `docs/**`+`*.md`, hors `docs/design/**` ; ref neuve/suppression ⇒ gate entière ; itération par ligne, jamais par mot) | W5 — à chronométrer au premier push docs-only |
| 5 | **✓ livré** — `scripts/terrain.ps1` (état du poste : base, version, OAuth User **et** session, traces) + `scripts/lancer-wind.ps1` (build par `construire-wind.mjs` — la maison unique des pièges du rebuild — puis `cargo run` qui TIENT le handle de trace, §9) ; référencés au STOP 2 de `/chantier` | terrain.ps1 prouvé sur ce poste (0.7.0, base 11,83 Go) |
| 6 | **✓ livré** — gabarits de seed (clé = exe du seeder + recette, **TTL 30 min** — les seeders figent l'horloge à la construction : jours relatifs ET `derniere_synchro` « il y a 2 min » ; une clé à la journée a fait un rouge à la gate du push, corrigé le jour même), copie par spec, construction à côté + rename | compris dans W2 ; ~14 `cargo run --example` → 1 construction / 30 min |
| — | **W1 re-mesuré après E1+E2+E6** : gate complète via `gate.ps1`, 121/121, zéro flaky | **4 min 34 s (W0) → 1 min 43 s** (103 s ; e2e 256 → 86 s), cache cargo chaud aux deux mesures |
| 7 | **✗ rejeté sur le chiffre** — `cargo test --all-targets` mesuré à **9,3 s** cache chaud : le gain espéré (15-25 s) excède le poste entier ; nextest n'est ni installé ni adopté | — |

Revue à regard neuf 8 angles (2026-08-23) : 10 trouvailles, 8 corrigées
avant gate (dist périmé du lanceur terrain, dates figées du gabarit,
word-splitting et suppression de ref du hook, zombies mémoïsés,
sidecars WAL du gabarit, faux ABSENT OAuth, exception PS muette), 2
consignées : **double encodage de la gate** (pre-push sh + gate.ps1 —
deux maisons, à unifier si elles divergent encore) et **piste racine
`build.rs`** (`cargo:rerun-if-changed` sur le dist rendrait le bump
inutile — à instruire hors fenêtre, comportement `tauri_build` à
prouver d'abord).

### Vague 3 — structurel (à planifier, hors fenêtre de mesure)

1. Sortir le dépôt de OneDrive (doctrine existante d'`installer-poste.ps1`)
   — à un moment sans travail non commité en vol ; re-pointer la mémoire
   Claude (clé projet = chemin).
2. Runner self-hosted x64 pour un job e2e CI — l'ADR 0005 planifie cette
   bascule ; déclencheur : jalon bêta fermée. Sort les 121 tests du
   chemin bloquant local.

### Contre-mesure optionnelle T5 — output style `Concise` (à instruire dans une future session)

**✓ Activé le 2026-08-28 au soir** (début de semaine 2), dans
`~/.claude/settings.json` — le vrai fichier de portée utilisateur,
`settings.local.json` n'existant qu'au niveau projet. Prend effet aux
prochaines sessions.

Réglage Claude Code : `"outputStyle": "Concise"` (portée utilisateur ;
exige Claude Code
v2.1.237+ ; la commande `/output-style` n'existe plus, passer par
`/config` ou l'app desktop Settings > Claude Code). Effet : réponses
courtes par défaut, narration réduite ; les verdicts chiffrés des
skills, rapports d'erreur et confirmations restent complets.

Protocole d'essai — l'output ne pèse que 14 % du coût, c'est un
appoint, pas un levier ; il se paie donc en mesure, pas en conviction :

1. Semaine 1 de la fenêtre : baseline sans Concise (déjà en cours).
2. Semaine 2 : activer Concise, même mix d'activité autant que possible.
3. Au bilan du 2026-09-06 : comparer via `scripts/mesurer-sessions.mjs`
   les tokens de sortie par session à activité comparable, ET le
   garde-fou qualité (KO au STOP 2, CI rouges, re-demandes de détail
   par le CE). Adopté si baisse nette sans dégradation ; sinon retiré.

### Pistes instruites et rejetées (ne pas ré-instruire)

sccache (dégrade l'incrémental à chaud) ; fenêtre WebView2 partagée
entre specs (état partagé, STANDARD §7.1/7.5) ; gate déléguée à la CI
hébergée (ADR 0005) ; arbitrage d'un flake e2e par `gh run` (la CI ne
joue aucun e2e).

---

## Mesure et revue (PDCA)

- **À chaque /solde** : noter au PLAN du chantier les 3 chiffres du
  chantier — équiv. input (T1), gates complètes jouées (W3), constats
  KO au STOP 2 (garde-fou qualité).
- **Hebdomadaire (vendredi)** : rejouer `scripts/mesurer-sessions.mjs`
  sur la semaine, remplir le tableau de suivi ci-dessous.
- **Bilan le 2026-09-06** : indicateur par indicateur, atteint / raté /
  cause ; les contre-mesures qui n'ont pas produit leur chiffre sont
  amendées ou retirées (standard work : on garde ce qui marche mesuré).

### Mesure hebdomadaire S1 — 2026-08-28 (fenêtre 24–28/08, 13 sessions)

`node scripts/mesurer-sessions.mjs --depuis 2026-08-24 --jusqua 2026-08-28` :
121 prompts CE, 5 430 tours, 283 M équiv. input fil principal + 22,3 M
agents (7,3 %). Chantiers soldés dans la semaine avec chiffres au PLAN :
RETOURS-9 (11,4 M, 2 gates), RETOURS-10 (2 gates 2,1–2,2 min),
RETOURS-11 (29,1 M, 4 gates), ELEMENTS (29,6 M, 7 gates).

Lecture des écarts :

- **Ce qui tient** : T1 (tous les chantiers ≤ 30 M, −50 % et plus vs
  baseline), W1 (gates 2,1–2,6 min avec une suite passée de 121 à
  148 e2e), M2 (1 revue par chantier), garde-fou qualité (KO terrain
  corrigés le jour même, 0 CI rouge).
- **T3 raté (3 sessions > 24 h)** : dont la session kaizen elle-même
  (81d387ca, 141 h de mur — rouverte à chaque rite au lieu d'un fil
  neuf) et 6e998992 (24,2 h, 55,7 M). La règle « clore au /solde »
  n'est pas encore un réflexe pour les sessions hors chantier.
- **T2 raté (364 k/tour)** : conséquence directe de T3 — les sessions
  qui durent traînent 300–484 k de contexte.
- **T4 brouillé (44,9)** : l'indicateur mélange les sessions agentiques
  (a02fb764 : 519 tours / 0 prompt) avec le pilotage CE ; à re-lire au
  bilan par session pilotée. RETOURS-9 (320 tours / 1 prompt) est au
  contraire le fonctionnement voulu : un énoncé complet, zéro relance.
- **P1 raté (100 min > 30 s au 1er plan)** : dominé par les e2e joués
  au premier plan (54 min, dont b8eb0fe7 : 14 runs / 38 min) — la
  consigne « arrière-plan au-delà de 60 s » (vague 1.4) couvre la CI
  mais pas encore les suites e2e pendant l'implémentation.
- **M1 à moitié appliqué** : agents 40 % abaissés (Sonnet 553 + Haiku 7
  sur 1 411 appels) ✓, mais fil principal encore 100 % haut de gamme
  (Opus 5 + Fable 5) — la règle « session mécanique = Sonnet 5 » de
  WORKFLOW.md n'a pas encore été utilisée une seule fois.
- **T5 (référence « sans Concise »)** : 7,09 M tokens de sortie sur la
  fenêtre, ~1 464 par tour, ~591 k par session. Concise activé ce soir
  pour la semaine 2, conformément au protocole.

### Tableau de suivi

| Indicateur | Baseline | Cible | Sem. 1 | Sem. 2 | Verdict |
|---|---|---|---|---|---|
| T1 équiv. input / chantier | ~60 M | ≤ 35 M | 11–30 M ✓ | | |
| T2 contexte moyen / tour | 410–540 k | ≤ 200 k | 364 k ✗ | | |
| T3 sessions > 24 h non closes | 8+ | 0 | 3 ✗ | | |
| T4 tours / prompt | 36,6 | ≤ 25 | 44,9 ✗ (brouillé, cf. note) | | |
| P1 mur bloqué > 60 s au 1er plan | ~3,5 h | ≤ 15 min | 100 min (> 30 s) ✗ | | |
| P2 re-runs / flake | ≤ 11 | ≤ 2 | aucun flake à trancher — | | |
| P3 allers-retours évitables | 24 | 0 | 0 observé ✓ | | |
| W1 gate complète | 4 min 34 s (W0) | ≤ 6 min | 2,1–2,6 min (148 e2e) ✓ | | |
| W2 1 spec e2e | 74 s | ≤ 45 s | 13,5–19 s (vague 2) ✓ | | |
| W3 gates complètes / chantier | 10+ | ≤ 3 | 2 / 2 / 4 / 7 ~ | | |
| W5 push docs-only | ~2 min | ≤ 30 s | à chronométrer ce soir | | |
| T5 (opt.) tokens de sortie / session (Concise) | réf. sem. 1 | baisse sans perte qualité | 7,09 M ; 1 464/tour (sans) | avec | |
| M1 coût haut de gamme hors chantiers | ~10–15 % | ≤ 5 % | agents 40 % abaissés ; fil 0 % ~ | | |
| M2 revues high / chantier | jusqu'à 3 | 1 | 1 ✓ | | |
| Qualité : KO au STOP 2 / CI rouges | réf. sem. passée | stable ou ↓ | KO corrigés j.-même ; 0 CI rouge ✓ | | |

---

## § Décisions CE

- **D1 — Seuils de session.** Contexte moyen ≤ 200 k (T2) et clôture ≤
  24 h de mur (T3) : valider ou ajuster les deux seuils.
  *Réponse CE (2026-08-23) : « D1 OK » — seuils validés.*
- **D2 — Script de mesure au dépôt.** Verser `scripts/mesurer-sessions.mjs`
  (il lit les transcripts locaux sous `~/.claude/projects/…`, chemin
  propre à la machine — comme `installer-poste.ps1`) : oui / non.
  *Réponse CE (2026-08-23) : « oui » — le script sera versé en vague 0.*
- **D3 — Ordre de la vague 2.** L'ordre proposé (rebuild mémoïsé en
  premier) : valider ou réordonner.
  *Réponse CE (2026-08-23) : « OK pour l'ordre proposé. »*
- **D4 — Fenêtre de bilan.** Bilan PDCA le 2026-09-06 : valider ou
  déplacer.
  *Réponse CE (2026-08-23) : « OK pour le 6 septembre. » (Précision
  actée : ce n'est pas la fin du kaizen — c'est le bilan de la fenêtre
  de mesure ; les contre-mesures qui tiennent leur chiffre restent,
  les autres sont amendées ou retirées.)*
