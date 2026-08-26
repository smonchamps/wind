# PLAN-DEMARRAGE — rendre les soixante premières secondes

> **TERRAIN VALIDÉ le 2026-08-26 — 6/6, aucun constat.** STOP 1 passé le
> même jour, décisions D1-D9 tranchées et consignées mot pour mot (§5).
> E0, E1, E1-bis et E2 livrés ; revue à regard neuf passée (2 trouvailles,
> les 2 corrigées) ; **gate complète VERTE 9/9**, e2e 137 → **138**.
> Reste : commit, push, CI verte, puis `/solde`.

### Verdict du Chef Ingénieur au terrain — 2026-08-26

| | Ce qui a été joué | Verdict |
|---|---|---|
| 1 | Premier lancement (paie la reconstruction d'index) | **« environ 2 secondes avant que la liste apparaisse »** — le banc annonçait 2 209 ms, dont 1 770 de reconstruction. La prédiction tenait. |
| 2 | Second lancement | **« tout instantané »** — banc : 384,6 ms. |
| 3 | Défiler, ouvrir, changer de dossier dans les 20 premières secondes | **« fonctionne comme attendu »** — le gel de service est mort. |
| 5 | Les repères et noms de compte à la première peinture | **« tout semble instantané »** — aucune rangée ne se repeint. Le refus de différer les sondes est validé au terrain. |
| 6 | Un vieux message à pièce jointe | **« oui, c'est ok »** — le retrait du critère `scanned` (D8) ne se voit nulle part. |
| 7 | Second poste x64 | **différé** — rien ne dit comment ces chiffres s'y transposent. |

> Énoncé (2026-08-26) : « Bug : freezes et lenteurs au démarrage, une
> fois la fenêtre ouverte, jusqu'à l'état stable. »

> **Phase 0 est faite** — instruction sur pièces et campagne de mesure
> menées les 25 et 26/08/2026 sur la base réelle du CE (12,84 Go,
> 251 524 enveloppes), sur `main` à `c090bf9` (Wind 0.10.0). Preuves au
> dépôt : `spikes/demarrage/` (banc, 29 journaux de spans, dépouilleur).
> Ce plan ne ré-instruit pas. Il conçoit — et il **corrige un point du
> dossier d'instruction** : le correctif principal, posé tel qu'il y
> était écrit, n'aurait rien changé (§3.1).

---

## 1. Constat — ce qui est mesuré

**Le budget est manqué.** `docs/PLAN.md:32` dit « démarrage à froid,
fenêtre utilisable, < 1 s ». Au run froid sur le décor réel :

| Jalon | run 01 (froid) | p50 chaud |
|---|---|---|
| A — fenêtre tao construite | 24,1 ms | 19,9 |
| B — tranche WebView2 | **750,9 ms** | 278,8 |
| C — fenêtre → première trame | 773,0 ms | 296,0 |
| fenêtre → **requête** de liste | 880,0 ms | 368,4 |
| fenêtre → **liste complète** | **1 157,3 ms** | 417,0 |

**Puis le cœur ne s'arrête plus.** Somme des latences du run froid :
**60 354 ms sur 61 s d'observation**. Les trois postes qui la portent :

| Commande | run 01 | p50 chaud | max | verrou global |
|---|---|---|---|---|
| `backfill_bodies` | 17 300 ms | 12 500 | 35 300 | non (sature disque et réseau) |
| `sync_inbox` | 16 300 ms | 4 100 | 8 180 | non |
| **`backfill_status`** | **8 870 ms** | 1 040 | 1 160 | **OUI** |

**LE défaut.** `backfill_status` part à t + 3 s (`App.svelte:913`), passe
par `hors_pompe` (`commands.rs:4768`) donc **tient le verrou global des
commandes**, et appelle `bodies_pending_count` pour chacune des 64
boîtes (`commands.rs:4803`). Le prédicat
`NOT EXISTS (… AND b.scanned = 1)` (`store.rs:2214`) oblige SQLite à
rappeler la ligne de `bodies` — **56 ko en moyenne** — pour lire un
entier d'un bit : ~251 k lectures aléatoires dans 11,4 Go. En SQL
direct sur la base réelle, trois exécutions reproductibles :
**20 839 · 22 728 ms à froid**, 889 ms à chaud. La même requête privée
de `b.scanned` : **396 ms à froid, 76 ms à chaud**.

Pendant ces secondes-là, la fenêtre **n'est pas gelée** au sens Win32 —
`hors_pompe` passe par `spawn_blocking` et `sync_activity` bat à 1 Hz
tout du long. **C'est un gel de SERVICE** : aucune commande applicative
n'est servie. Ni une page de liste, ni un rafraîchissement de nav, ni
l'ouverture d'un message.

**Le décor explique tout.** `bodies` pèse **11 460 Mo — 89 % de la
base** ; `envelopes` 47 Mo. L'ensemble de travail du premier écran pèse
**moins de 80 Mo** et tient sans effort en mémoire ; les 11,4 Go de
corps ne tiendront **jamais** dans le cache disque d'une machine de
15,6 Gio (5,6 libres). Toute requête qui les touche paie de la lecture
aléatoire à froid, **à chaque lancement**.

**Et la liste est dixième dans la file.** Dix commandes partent dans un
seul tick (`App.svelte:900-966`), huit prennent le verrou ;
`list_category` et `pinned_rows` ne partent qu'au flush Svelte suivant.
Chronologie relevée : rafale à 856 ms, liste à 880 ms. **La seule
commande dont l'utilisateur attend le résultat est la dernière servie.**

**La dette était écrite, avec sa clause.** D-8 (`docs/DETTE.md`,
2026-08-15) portait `pending_total` à 575 ms, décision CE « les
optimiser sans constat serait du travail sans mesure », clause
« **Rouvre si : le terrain désigne le coût.** » La base est passée de
1,3 à 12,8 Go, les 575 ms sont devenus 20 800. **La clause est
remplie.** Le chiffre de 865 ms que D-8 porte pour `nav_snapshot` est
**périmé** (re-mesuré : ~31 ms froid, ~11 ms chaud) et doit être
corrigé dans le même geste.

### Ce qui est RÉFUTÉ — à ne pas rechasser

| Soupçon | Verdict mesuré |
|---|---|
| `orphans()` rejoué à chaque ouverture | Portée **3 432 lignes sur 251 524** (seules INBOX et Envoyés portent `threaded = 1`) : 42 ms froid, **0,7 ms chaud**. Propreté, pas latence. Seuil de surveillance : ~70 000 messages dans INBOX + Envoyés. |
| `nav_snapshot` à 865 ms | Périmé — réécrit depuis, passe par `nav_unread_counts` : **~31 ms froid, ~11 ms chaud**. |
| OneDrive fausse les mesures | Même base aux deux emplacements : **13,32 vs 13,44 ms**. Aucun écart — et la base de production vit dans `%APPDATA%\Roaming`. |
| Chaque commande s'exécute deux fois | Non : **préflight CORS** par endpoint distinct (52 transports pour **29 exécutions** sur 23 endpoints), ~5 ms au total. Prouvé par les spans `tauri::ipc` de `sondes/preflight-cors.log`. |
| La pénalité `preview` vaut 198 ms | 198 ms est le chiffre **en SQL direct à froid** ; dans l'application, `list_category` vaut **38 ms** au run froid, 34,9 ms à chaud — les 200 lignes du haut restent en cache. Le défaut est réel, son ordre de grandeur ne l'est pas. |
| La fenêtre est « gelée » | Gel de **service**, pas de fenêtre — `sync_activity` le prouve. |

---

## 2. Périmètre — et ce qu'on ne fait pas

**Dans le périmètre** : ce qui se passe **entre l'apparition de la
fenêtre et l'état stable**, et seulement ce qui est **mesuré**. Le gel
de service (défauts 01 et 02), la place de la liste dans la file
(défaut 03), les paliers nommés qui remplacent les `setTimeout`, et le
**filet qui rend ces défauts détectables** — sans lui, la gate restera
verte pendant que le terrain se dégrade, exactement comme jusqu'ici.

**Refusés explicitement** (§2.6 du STANDARD) :

| | |
|---|---|
| **La connexion SQLite maintenue** (76 `Store::open`, aucun PRAGMA de performance) | Le banc annonce 845 → 56 ms sur une rafale de 12 commandes — **mais sur un décor mono-boîte où tout est en portée de fils**. Sur la base du CE, la portée d'`orphans()` est de 1,4 % : `Store::open` y vaut **~1,5 ms**, pas 66. Le gain direct est modeste, `Store` n'est pas `Sync`, et c'est un chantier entier. **Dette** — à rouvrir sur constat, comme D-8 l'a été. |
| **L'éviction du cache de corps** (11,4 Go conservés pour toujours, ADR 0010) | Arbitrage **produit**, pas technique : qu'est-ce qui est purgeable et qu'est-ce qui ne l'est jamais ? Il appelle son propre chantier, avec l'ADR 0010 à rouvrir. Les correctifs de ce plan le rendent d'ailleurs **plus sûr** — une liste découplée des corps rend l'éviction inoffensive pour l'écran. |
| **La restauration de la vue** (dossier, tri, sélection, défilement) | C'est une **fonctionnalité**, pas un défaut de démarrage. Personne au marché ne la livre entièrement ; elle se décide, elle ne se corrige pas. |
| **La colonne fantôme de `echos`** (`store.rs`, un `\n` dans un commentaire SQL d'une chaîne Rust) | Défaut réel — **toute base neuve** reçoit une colonne parasite —, mais ce n'est **pas** un défaut de performance, et le retirer d'une base existante demande une réécriture de table. Sujet propre. **Dette, à ouvrir en D-36.** |
| **`orphans()` et sa garde** | 0,7 ms à chaud, plafond du gain ~1,5 ms par commande. Propreté. |
| **Le découpage du bundle** (232 ko, zéro import dynamique) | Servi en **0,31 ms** : le poids ne coûte rien au transport, seulement au parse — **non mesuré**. Le dossier le dit lui-même : à ne pas traiter avant de l'avoir mesuré. |
| **La coupure environnement / contrôleur dans les 279 ms de WebView2** | Exigerait un `wry` vendoré sur une branche jetable. À n'ouvrir que si B pèse une part inacceptable **du palier retenu en D1**. |

**Refusés par le Chef Ingénieur au STOP 1 du 2026-08-26** (§5) —
instruits, chiffrés, et écartés de CE chantier :

| | |
|---|---|
| **La fenêtre différée** (`visible: false` + `show()` au palier) — D2 | Échange perceptible (rien pendant ~800 ms, puis tout) doublé d'un piège mesuré : une page cachée fait **retarder le premier rendu** par Chromium. Elle mérite son STOP visuel et sa mesure propre, **après** que le gel soit mort — deux changements simultanés ne se départagent pas. **Dette.** |
| **L'aperçu stocké derrière le corps** (défaut 06) — D3 | **38 ms dans l'application** au run froid, pas les 198 ms du SQL direct : des dizaines de millisecondes contre des milliers pour le défaut 01. Le correctif juste (une table `apercus`) porte un invariant de Système **et** une migration qui relit 11,4 Go de corps. **Dette** — prioritaire le jour où l'éviction sera décidée. |
| **`sync_progress`** (152 ms à froid, toutes les 5 s, à vie) — D6 | Son correctif est un compteur tenu à l'écriture : même famille et même risque de dérive que l'option C du §3.1, pour **~1,8 s** sur les 60 premières secondes contre ~26 s pour le défaut 01. **Dette.** |

---

## 3. Les points durs

### 3.1 Point dur 1 — le compte des corps manquants, et le piège de l'index

Le dossier d'instruction propose :
`CREATE INDEX idx_bodies_scanned ON bodies(mailbox_id, uid, scanned)`,
« rend la sonde couvrante ». **C'est faux, et c'est mesuré.**

Le plan d'exécution, sur le schéma de production réduit à ses tables
utiles (SQLite 3.50.4, décor peuplé, avec et sans `ANALYZE`) :

| variante | l'étape de la sous-requête `NOT EXISTS` |
|---|---|
| aujourd'hui | `SEARCH b USING INDEX sqlite_autoindex_bodies_1` |
| **+ index `(mailbox_id, uid, scanned)`, sans directive** | `SEARCH b USING INDEX sqlite_autoindex_bodies_1` — **inchangé** |
| idem, après `ANALYZE` | **inchangé** |
| + index **partiel** `WHERE scanned = 1`, sans directive | **inchangé** |
| + `INDEXED BY idx_bodies_scanne` | `SEARCH b USING **COVERING** INDEX idx_bodies_scanne` |
| la sonde **sans** `b.scanned` (la variante mesurée à 396 ms) | `SEARCH b USING **COVERING** INDEX sqlite_autoindex_bodies_1` |

**Pourquoi.** L'index automatique de la clé primaire répond déjà
`(mailbox_id = ? AND uid = ?)`. SQLite n'a aucune raison d'en changer,
et le rappel de ligne qu'il paie ensuite pour lire `scanned` est
invisible à son modèle de coût. L'index aurait été livré, la migration
aurait tourné, l'index aurait grossi la base — **et le gel serait
resté**. Seul le terrain l'aurait dit, une version plus tard.

La dernière ligne du tableau est la **preuve du mécanisme** : la
variante mesurée à 396 ms est rapide précisément parce que son plan dit
`COVERING` sur le même index. Le mot `COVERING` **est** l'assertion.

C'est mot pour mot l'enseignement déjà payé par le dépôt (STANDARD §9,
« Une promesse d'index ne vaut que pour la requête qu'on avait en
tête »), et le remède y est écrit : **un test de PLAN D'EXÉCUTION**.

#### La Phase 2 a retourné le point dur — trois mesures

Prises le 2026-08-26, **en lecture seule sur la base réelle** (11,96 Go,
5,63 Go de RAM libre), agrégats seuls, aucun contenu de message lu :

| | Mesure | Résultat |
|---|---|---|
| **A** | `COUNT(*) FROM bodies` — couvrant sur l'auto-index de la PK | 251 466 lignes, **0,18 s** |
| **B** | le même **en lisant `scanned`** — le terme dominant de toute construction d'index | **18,1 s** |
| **C** | lignes portant **`scanned = 0`** | **0** — et **0 aussi sur le second poste** |

**B confirme** que construire un index sur `scanned` coûterait des
dizaines de secondes : l'écran de migration aurait bien été nécessaire.
**C le rend inutile.** Et une lecture du code achève de le retourner :
**rien en production n'écrit jamais `scanned = 0`** — l'unique écriture
est `INSERT OR REPLACE … VALUES (?1, ?2, ?3, 1, ?4)`, un `1` en dur
(`store.rs:1761`). Le seul `UPDATE bodies SET scanned = 0` du dépôt vit
**dans un test**, qui simulait l'héritage.

Le critère `AND b.scanned = 1` est donc la trace d'une **passe
d'héritage soldée** — les corps rapatriés avant que les pièces jointes
n'existent. Il coûtait le gel de 8 870 ms **pour protéger zéro ligne**,
sur une flotte de deux machines qui est la flotte entière (la bêta n'est
pas engagée).

#### Les options, et leur verdict

| | Option | Plan obtenu | Ce qu'elle coûte | Verdict |
|---|---|---|---|---|
| **G** | **Retirer le critère `scanned`** des deux requêtes | `COVERING` sur l'auto-index de la PK — **vérifié sur la base réelle** | rien. Aucune migration, aucun index, aucun écran. La colonne devient vestigiale | **RETENUE — décision CE D8** |
| **A** | Index `(mailbox_id, uid, scanned)` **+ `INDEXED BY`** | `COVERING` | **18-30 s de migration une fois** + l'écran ADR 0012 + l'annulation par `InterruptHandle` + le mappage `SQLITE_INTERRUPT` + le `busy_timeout` manquant de `pending_adoption` + deux tests à amender + un test qui `DROP` l'index. Et `INDEXED BY` **échoue au `prepare`** si l'index manque — sur le seul chemin qui l'emprunte, dont le `catch` est un `console.error` nu (`App.svelte:519`) : un échec dur ajouté à un chemin muet | écartée — tout cet appareil pour zéro ligne |
| **A′** | Index **UNIQUE** `(mailbox_id, uid, scanned)`, sans directive | `COVERING` — **mesuré** : SQLite le choisit seul, là où l'index ordinaire est ignoré même après `ANALYZE` | mêmes 18-30 s de migration, mais sans le mode d'échec dur de `INDEXED BY`. La contrainte UNIQUE n'ajoute aucun invariant (la PK l'impose déjà) et `INSERT OR REPLACE` reste sain, bascule `scanned` comprise | écartée avec A — meilleure forme, même prix |
| **C** | Compteur par boîte, tenu à l'écriture | O(1) | tous les chemins d'écriture, risque de dérive, **et le même balayage initial de 18 s** | écartée — voir §3.2 |
| **D** | Le drapeau du corps porté par l'**enveloppe** | `SCAN e` seul | une colonne, tous les chemins d'écriture, **le même balayage initial** | écartée |

**Ce que G coûte, et il faut le dire** : une base portant des corps
d'avant les pièces jointes cesserait de les relire — leurs pièces
jointes resteraient invisibles. Aucune base de la flotte n'est dans ce
cas (mesuré, deux fois), et le code ne peut plus en produire. C'est une
sémantique qu'on **abandonne**, pas une optimisation qu'on gagne : le
plan le dit ici pour que personne ne la « restaure » par mégarde.

**Ce qui reste vrai de l'analyse d'origine, et qu'il faut garder :** le
correctif du dossier d'instruction — `CREATE INDEX … ON bodies(mailbox_id,
uid, scanned)` posé nu — **n'aurait rien changé**. Mesuré au plan
d'exécution, avec et sans `ANALYZE`, sur deux moteurs : SQLite garde
`sqlite_autoindex_bodies_1`. Il aurait fallu `INDEXED BY`, ou l'index
UNIQUE. C'est l'enseignement du STANDARD §9 (« une promesse d'index ne
vaut que pour la requête qu'on avait en tête ») payé une fois de plus —
et la raison d'être de la garde de plan.

#### Ce qui a été livré (E1, 2026-08-26)

- Le prédicat est **extrait** en `CORPS_ABSENT`, lu par les deux
  requêtes **et** par la garde de plan : le test interroge l'écriture de
  production, jamais une copie (patron d'`unified_page_sql` ; la copie
  était la vacuité payée à PLAN-ESPACEMENT).
- La garde `les_sondes_de_corps_manquants_ne_rappellent_jamais_la_ligne_grasse`
  a été **vue ROUGE** avant le correctif (`SEARCH b USING INDEX
  sqlite_autoindex_bodies_1`) puis verte après. Non-vacance prouvée par
  le RED lui-même, pas par raisonnement.
- Le test de l'héritage est **réécrit**, pas supprimé : il porte
  désormais la décision D8, ses trois faits, et l'invariant d'écriture
  (`scanned` vaut toujours 1) qui la rend sûre.
- Le commentaire de la colonne au `SCHEMA` disait « il faut le relire » —
  **il mentait désormais** : remis d'équerre.

**Mesure avant/après sur la base réelle**, les deux formes dos à dos, la
nouvelle **en premier** pour qu'elle paie le cache le plus froid :

| | Temps | Plan |
|---|---|---|
| **après** (forme retenue) | **0,297 s** | `SEARCH b USING COVERING INDEX` |
| **avant** (forme livrée en 0.10.0) | 0,736 s | `SEARCH b USING INDEX` |

**Ces deux chiffres sont à CHAUD, et le chaud n'a jamais été le
problème.** Ils prouvent deux choses et pas une de plus : le plan est
couvrant **sur la vraie base**, et la forme retenue est plus rapide même
quand tout est en cache. Le gain qui compte — **20 839 ms → 396 ms**,
×53 — est celui que la campagne du 26/08 a mesuré **à froid sur ces deux
mêmes formes** ; il reste une prédiction tant que le terrain ne l'a pas
rejoué. Sans outil d'éviction sur ce poste, la seule mesure honnêtement
froide est **le premier lancement après démarrage machine** : c'est le
point 3 de la liste de terrain, et c'est lui qui tranche.

#### Ce qu'on attend de la mesure froide — écrit AVANT de la prendre

Le CE a choisi la séquence complète (2026-08-26) : copie de la base,
**redémarrage machine**, puis premier lancement tracé. Ce sera la
**première mesure honnêtement froide** du démarrage jamais prise au
projet — le dossier d'instruction liste explicitement « le premier
lancement après démarrage machine n'a pas été mesuré » parmi ses
lacunes. Elle la comble.

Deux conséquences à poser d'avance, sinon elles se liront de travers :

1. **L'« après » sera PLUS froid que l'« avant ».** `reel-01` a été pris
   sur une copie fraîche **sans redémarrage** : ses pages étaient encore
   tièdes (c'est le piège ×340 du STANDARD §9, que le dossier reconnaît).
   L'écart mesuré jouera donc **contre** le correctif, pas pour lui.
2. **Le palier 3 ne doit PAS bouger, et ce n'est pas un échec.** E1 tue
   un gel qui commence à t + 3 s, c'est-à-dire **après** que la liste
   soit peinte. Le palier 3 reste l'affaire d'E2 (servir la liste en tête
   de file). Ce qu'E1 doit faire tomber, et lui seul :

| Ce qu'on lit | `reel-01` (avant) | Attendu après E1 |
|---|---|---|
| `backfill_status`, verrou global tenu | **8 870 ms** | quelques centaines de ms |
| somme des latences sur la fenêtre | **60 354 ms** | fortement en baisse |
| palier 3 « liste peinte » | 1 157,3 ms | **inchangé** — c'est E2 qui l'attaque |
| paliers A, B, C | 24,1 / 750,9 / 773,0 ms | inchangés |

Si `backfill_status` ne tombe pas, le correctif est faux malgré le plan
d'exécution, et la ligne s'arrête.

#### Où passent les 2 740 ms restants — instrumenté, pas supposé

Le CE a demandé (2026-08-26) d'instrumenter `Store::open` avant de
décider où frapper. Trois jalons posés dans `backfill_status` derrière
la feature `mesure` — l'ouverture, la sonde, le corpus — **jamais dans
le binaire livré** (clippy `-D warnings` vert dans les deux
configurations).

| jalon | run 01 (chaud) | run 02 (chaud) |
|---|---|---|
| `Store::open` | **31,1 ms** | **3,70 ms** |
| **`pending_total`** | **670 ms** | **527 ms** |
| `corpus_total` | 13,9 ms | 12,4 ms |
| *total de la commande* | *725 ms* | *547 ms* |

**La comptabilité se boucle à ~10 ms près** : aucun terme caché.

Deux hypothèses tombent, et il faut le dire :

1. **`Store::open` n'est pas le sujet** — 31 ms puis 3,7. Le refus
   « connexion SQLite maintenue » du §2 était juste, et il est maintenant
   adossé à une mesure **dans l'application**, plus seulement à une
   extrapolation depuis un banc mono-boîte.
2. **Une réparation non gardée de `migrate()` n'y est pour rien** :
   hypothèse instruite puis écartée sur pièces — les quatre marqueurs
   (`apercus-entites`, `corps-fffd`, `objets-escapes`,
   `pieces-calendrier`) sont posés, `user_version` = 2, l'index FTS porte
   `recipients`. Rien de lourd ne tourne à l'ouverture sur cette base.

**Ce qui reste est la STRUCTURE de la sonde**, pas son prédicat : 64
boîtes, chacune parcourue en entier, **251 k lignes d'index arpentées
pour répondre « 59 »**. C'est le défaut 02, et c'est l'option C du
§3.1 — le compteur tenu à l'écriture — que ce plan avait mise en attente
de ce chiffre.

⚠️ **Conséquence pour E2, découverte par cette mesure.** Le plan prévoyait
d'accrocher `rattraperCorps` au palier « liste peinte » au lieu de son
`setTimeout(3000)`. **Tel quel, ce serait un recul** : la commande tient
le verrou 2,7 s à froid ; la déclencher à ~0,8 s au lieu de 3 s
avancerait le gel de service au lieu de l'éloigner. Le palier à viser
pour ce rattrapage-là est **« état stable »**, pas « liste peinte » —
ou il faut d'abord rendre la sonde gratuite.

#### E1-bis — l'index des enveloppes, et une hypothèse de plus retournée

Le CE a approuvé (2026-08-26) la direction « une requête au lieu de 64 »,
à mesurer avant d'écrire. **La mesure l'a refusée, et a désigné autre
chose.** Chemin de production reproduit hors de l'application, machine au
repos (`spikes/demarrage`, sous-commandes `pending` et `pending1`) :

| | avant | après |
|---|---|---|
| `pending_total` entier | **521,9 ms** | **107,9 ms** |
| dont `accounts()` | 0,016 ms | 0,012 ms |
| dont `mailbox_names()` | 0,038 ms | 0,029 ms |
| dont la sonde **par boîte** (p50) | 0,021 ms | 0,014 ms |
| dont la sonde, **pire boîte** | **400,5 ms** | **46,3 ms** |
| le même nombre **en UNE requête** | 83,3 ms | 87,2 ms |

**Les 64 allers-retours ne coûtaient rien** : 63 boîtes à 21 µs, et
**une** à 400 ms. Le regroupement en une requête ne rapporte plus que
21 ms une fois l'index posé — **il n'est donc pas écrit**. C'est très
exactement à cela que sert de mesurer avant.

**La cause, et c'est la même classe que le défaut 01.**
`idx_envelopes_date` portait `(mailbox_id, date_epoch DESC)` — **pas
`uid`**. Le prédicat de date traîne la requête sur cet index, qui ne peut
pas fournir l'`uid` du sondage : SQLite allait donc chercher **la ligne
d'enveloppe** pour chacun des 87 117 messages du plus gros dossier. Le
plan le disait d'un mot manquant — `USING INDEX`, pas `USING COVERING
INDEX`.

**Le correctif : `uid` en troisième colonne de l'index.** Une ligne au
`SCHEMA`, plus une migration qui lit la DÉFINITION de l'index et le
reconstruit s'il lui manque la colonne (`CREATE INDEX IF NOT EXISTS` est
un no-op muet sur une base existante — le défaut aurait survécu). Patron
de la sonde `recipients` de l'index de recherche.

**Reconstruction mesurée : 0,332 s** — elle ne lit que `envelopes`
(47 Mo), jamais les corps. **Aucun écran de migration**, contrairement
aux 18 s qu'aurait coûtées l'index sur `bodies` écarté par D8. C'est
toute la différence entre une migration muette acceptable et le gel du
2026-08-17. *(Chiffre pris à chaud : le coût à froid reste à confirmer à
la passe terrain.)*

**La garde de plan gardait la moitié du défaut.** Elle n'assertait que
l'étape `bodies` ; le même défaut vivait côté `envelopes` et serait passé.
Elle asserte désormais `COVERING` **des deux côtés**, et la non-vacance a
été prouvée en cassant : schéma remis à deux colonnes **et** migration
désarmée — le test rougit sur l'étape `e` ; restaurés, il repasse.

**Et le refus du §2 tient, désormais mesuré dans l'application** :
`Store::open` vaut 31,1 ms puis 3,70 ms. La « connexion SQLite
maintenue » n'aurait rien rapporté ici.

### 3.2 Point dur 2 — le balayage repayé par lot : à ARBITRER SUR LE CHIFFRE

`run_backfill_all` refait le compte de **toutes les boîtes de chaque
compte** avant de décider s'il se connecte (`commands.rs:5149`), et
`backfill_bodies` refait un compte par boîte à la fin de chaque lot
(`backfill.rs:133`). L'UI reboucle tant qu'il reste du travail
(`App.svelte:499-517`), à raison de `BACKFILL_BUDGET = 200` messages par
appel.

**La sévérité dépend du retard.** Sur la base du CE, la sonde répond
« 59 » : c'est **un seul lot**, donc une poignée de balayages
supplémentaires — d'où les 17 300 ms. Sur une boîte fraîchement
connectée, c'est un balayage par lot de 200, indéfiniment.

**Prédiction, à vérifier et non à croire** : une fois A posé, chaque
balayage tombe de ~21 s à ~0,4 s. Le temps de verrou du démarrage
passerait de ~26 s à **~1,2 s** — et le défaut 02 cesserait d'être un
défaut sur ce décor.

**C'est le STOP mesuré de ce chantier.** On ne corrige pas 02 avant
d'avoir le chiffre de 01. Si le chiffre le justifie encore, deux voies,
départagées à ce moment-là :

| | | |
|---|---|---|
| **(i)** | Le compte tenu **en mémoire le temps d'une passe** — le verrou `verrou_rattrapage` sérialise déjà la passe ; le compte se décrémente de `fetched` au lieu de se refaire | petit, local, sans persistance donc sans dérive possible |
| **(ii)** | Le compteur **persisté**, tenu à l'écriture (option C du §3.1) | supprime le balayage pour de bon, au prix de tous les chemins d'écriture et d'un risque de dérive |

### 3.3 Point dur 3 — le filet est aveugle par construction

**Fait, vérifié dans le code** : `e2e/mesure-v2.mjs:56` appelle
`seed_inbox` avec `"${db}" ${nombre} ${email}` — donc `corps = 500` et
`ko_par_corps = 0`. À `ko_par_corps = 0`, le corps semé est
`<p>Corps du message n°N : contenu de démonstration.</p>` — **une
soixantaine d'octets**. Avec `page_size = 4096` (le défaut : le dépôt ne
pose aucun `PRAGMA page_size`), une ligne de cette taille **ne déborde
jamais** — et le débordement EST le mécanisme des défauts 01, 02 et 06.

**La table des corps du banc pèse quelques centaines de kilo-octets
contre 11,4 Go au terrain. Aucun des trois défauts ne peut s'y
manifester. C'est pourquoi la gate est restée verte pendant que le
terrain se dégradait.** L'outil sait déjà faire mieux : sa propre
documentation dit « l'ADR 0007 a mesuré ~34 Ko par corps stocké ; c'est
la valeur à passer ici ». Le paramètre existe, personne ne l'a passé.

Trois décors distincts, pour trois usages distincts — les confondre est
le piège :

| Décor | Ce qu'il prouve | Où il vit | Coût |
|---|---|---|---|
| **Le plan d'exécution** — base en mémoire, aucune masse | L'**invariant** : la sonde est couvrante, la page ne touche pas `bodies`. Machine-indépendant, comme le dit le dépôt : « on interroge le plan plutôt qu'un chronomètre » | `cargo test`, **dans la gate** | nul |
| **Le mécanisme** — `ko_par_corps ≥ 8`, quelques milliers de corps | Le débordement existe vraiment ; le décor **produit la condition** que le code prétend traiter (STANDARD §9) | banc, hors gate | secondes |
| **La magnitude** — `corps = nombre`, `ko_par_corps = 34` (~8,5 Go à 251 k) | Le **chiffre** à froid, celui du STOP mesuré | banc, **manuel** | minutes de semis, 8,5 Go de disque |

**Le filet de la gate est donc fait de tests de plan**, pas de
chronomètres — et il est **non vacant par construction** : retirer la
directive `INDEXED BY` fait disparaître le mot `COVERING` du plan, donc
rougir le test. La preuve se fait en cassant, comme au chantier
précédent.

**Ce qui reste à trancher (D4)** : le banc `mesure-v2` doit-il porter la
masse — et donc payer le semis — ou reste-t-il léger, la masse vivant
dans un décor nommé, joué à la main aux STOP mesurés et avant release ?

### 3.4 Point dur 4 — la fenêtre montrée vide, et le piège de la page cachée

**Mesuré** : `visible` vaut `true` par défaut, `tauri.conf.json` ne le
contredit pas, le HWND est affiché **avant** la création du WebView2, et
les deux classes de fenêtre — la parente `tao` et l'enfant `wry` — ont
un **pinceau de fond NUL** : rien ne peint. Au run froid, l'utilisateur
regarde un rectangle qui ne dit rien pendant **773 ms** avant la
première trame, **1 157 ms** avant la liste.

Le correctif proposé — `"visible": false`, un `backgroundColor` pris du
thème, `show()` au palier « coquille peinte » — est celui que Microsoft
documente (« Don't use WebView2 for initial UI »).

**Mais il porte un piège que le dossier signale sans le chiffrer** :
Chromium **retarde le premier rendu d'une page cachée**. Montrer la
fenêtre plus tard peut donc rendre le premier pixel plus tardif — le
contraire du but. Le garde-fou existe
(`performance.getEntriesByType('visibility-state')`, run rejeté si la
fenêtre était cachée), mais l'arbitrage se prend **sur la mesure**, pas
sur le principe.

C'est aussi un **échange perceptible** : aujourd'hui quelque chose
apparaît vite et ne dit rien ; demain rien n'apparaît pendant ~800 ms
puis tout est là. Ce n'est pas une évidence technique, c'est un choix de
produit. **D2.**

Le **flash de thème** voyage avec : `restaurerTheme()` appelle
`refleter()` de façon synchrone alors que le seul canal vivant du thème
OS dans WebView2 est `fenetre.theme()`, une IPC asynchrone que `mount()`
n'attend jamais (le dépôt sait déjà qu'`prefers-color-scheme` ne suit
pas l'OS dans `wry` — A42). Sur un poste en mode nuit, **chaque
démarrage produit un flash blanc puis une repeinture complète**.

---

### 3.5 Point dur 5 — E2 tel qu'il était écrit était en partie NUISIBLE

Contre-expertise du 2026-08-26 (trois angles : ce que l'écran montre, le
gel de service, la vacuité du filet) : **24 failles, les trois angles
rendent « ne tient pas »**. Trois décident.

**1. Différer les sondes FABRIQUE un défaut qui n'existe pas.**
`nav_snapshot` est émis en tête et prend le **même verrou global** que
`list_category` : sa réponse précède donc les lignes, et le premier paint
porte **déjà** le bloc de boîte (« sur … », A80). Le différer ferait
rendre chaque rangée sans son bloc puis la repeindre — le bloc s'insère
en `flex:0 3 auto; max-width:33%` et c'est l'**expéditeur** qui se
retronque, sur toutes les lignes visibles d'un coup. Sur tout poste à
deux comptes ou plus, celui du CE compris. Le raisonnement « c'était
déjà comme ça » est faux ici : le gain et le défaut sont le même levier.

**2. Le plan se contredisait sur le palier de `rattraperCorps`** — le
tableau des étapes disait « liste peinte », le §3.1 « état stable » — et
l'assertion prévue (« ne part jamais avant que la liste soit peinte »)
aurait été **satisfaite par le recul** qu'elle devait interdire. Pire :
`rattraperCorps` a **quatre appelants** ; le chemin de synchro le
déclenche vers t + 17 s, qu'aucun palier ne commande.

**3. Le filet prévu était vacant de trois façons.** Le journal posé selon
le patron du dépôt (`page.evaluate` après le premier rendu) est **vide**
des commandes du démarrage — le test aurait été vert sur le code fautif.
« Aucune sonde périodique ne la précède » est vrai par construction (les
intervalles ne tirent qu'à 5 s au plus tôt). Et l'ordre d'**émission**
que le journal prouve n'est pas l'ordre d'**acquisition du verrou**, seul
subi par l'utilisateur : `hors_pompe` prend un `std::sync::Mutex` depuis
un `spawn_blocking`, et un mutex n'est pas équitable.

#### Ce qui a été livré — et ce qui ne l'a PAS été

**Décision CE (2026-08-26) : E2 réduit au `tick` seul.** `prete = true`
ne peint pas tout de suite — Svelte planifie le flush par microtâche —,
donc les dix appels qui suivent partaient avant que `<Liste>` ne soit
monté. Un `await tick()` rend la main au flush : **la liste demande sa
page la première, seule, donc sans concurrent au verrou.** Un mot-clé.

Mesuré par le filet, et le chiffre corrige le §1 : `list_category` était
émise en **douzième** position (le plan disait dixième), `nav_snapshot`
en deuxième.

**Refusés, chacun sur une faille mesurée ou lue :**

| | |
|---|---|
| **Différer les six sondes** | Fabriquerait le repeint horizontal de chaque rangée (faille 1). `nav_snapshot` coûte 31 ms à froid : ce n'est pas là qu'est le défaut. |
| **Déplacer `rattraperCorps` sur un palier** | Il a quatre appelants ; un palier n'en commande qu'un. Et « état stable » n'a **aucun instrument** en production — l'accrocher à un signal qui n'existe pas. |
| **Déplacer `rattraperApercus`** | `liste?.recharger()` y est appelé **inconditionnellement**, même quand il n'y a rien à rattraper : le déplacer avance une recharge complète de la liste. Le vrai geste serait de rendre cette recharge conditionnelle — sujet propre. |
| **Le palier `data-startup`** | Il exige `totalPrecis`, donc la réponse de `category_total`, elle-même sous verrou. S'y accrocher mettrait les sondes derrière **deux** prises de verrou de plus, et si `category_total` échoue elles ne partiraient **jamais**. |

**Ce que le filet prouve, et il le dit dans son en-tête** : l'ordre
d'**émission**, sur une **recharge** à cache chaud. Pas l'ordre de
service, pas le démarrage à froid. La preuve opposable du gain reste le
palier mesuré au banc.

### 3.6 La passe froide — le verdict mesuré

Prise le 2026-08-26, **premier lancement après redémarrage machine**, sur
une copie de la base du terrain (11,96 Go), index volontairement remis à
deux colonnes pour que la migration se rejoue et se chiffre. Trois runs.

| | ligne de base (26/08) | run 01 (paie la migration) | run 02 (index construit) |
|---|---|---|---|
| A — fenêtre tao | 24,1 ms | 35,6 | 20,7 |
| B — tranche WebView2 | 750,9 ms | 304,1 | 265,6 |
| → requête de liste | 880,0 ms | 426,0 | **349,6** |
| **palier 3 — liste complète** | **1 157,3 ms** | 2 209,4 | **384,6 ms** |
| **`backfill_status`, verrou tenu** | **8 867,8 ms** | 289,4 | **124,9 ms** |

**`backfill_status` : 8 867,8 → 124,9 ms, ×71.** Aucune variable de
confusion : c'est le gel que le CE avait constaté, et il est mort.

**Le palier 3 demande une attribution honnête.** B vaut 750,9 ms à la
ligne de base contre 265,6 aujourd'hui — mais **B n'est pas notre fait** :
la campagne du 26/08 mesurait sous charge d'E/S (copie fraîche, sans
redémarrage), aujourd'hui le disque était au repos. En retirant B des
deux côtés, la part que le chantier commande passe de **406,4 ms à
119,0 ms — ×3,4**. Le budget de 1 000 ms est tenu avec marge.

**E2 confirmé au terrain** : `list_category` est émise à 93,3 ms, **la
première**, devant `pinned_rows` et les sept sondes. Elle était douzième.

**Ce que le chantier NE peut PAS revendiquer.** La latence cumulée passe
de 60 354 à 26 805 ms, mais le run 02 de la ligne de base valait
19 880 ms : la somme est dominée par `backfill_bodies` et `sync_inbox`,
du réseau que le chantier ne touche pas, et le retard de corps de la
copie a dérivé au fil des runs. **Ces deux chiffres ne sont pas
comparables** — ils ne sont pas portés au crédit du chantier.

**Le coût neuf, mesuré et assumé (décision CE D9).** Au **premier
lancement après mise à jour**, la reconstruction de l'index tombe sur le
premier `Store::open` plein — c'est-à-dire, depuis E2, sur
`list_category` — et lui coûte **1 770 ms**, sans écran. Le palier 3 y
vaut 2 209,4 ms : **le budget est dépassé, une fois.** Les cinq commandes
suivantes font la queue derrière (repères, noms, télémétrie à ~1,8 s).
Verdict CE : **assumé, et inscrit au STANDARD §3** — un écran qui
s'affiche et disparaît en 1,8 s est plus pénible que l'attente, et le
dépôt assume déjà 3,66 s d'adoption et ~4 min de reconstruction FTS.

*Preuve que la migration a bien tourné : l'index de la copie porte
désormais `uid`, et son SQL a le formatage exact de la migration Rust —
pas celui du `sqlite3` manuel qui l'avait posé la première fois.*

## 4. Les quatre paliers, et le budget

Le budget du dépôt dit « fenêtre utilisable < 1 s » sans dire **quand**.
Les trois instants se mesurent maintenant ; il faut choisir lequel est
opposable.

| Palier | Ce que l'utilisateur a | run 01 (froid) | p50 chaud |
|---|---|---|---|
| **1 · fenêtre visible** | un rectangle | 0 (par définition) | 0 |
| **2 · coquille peinte** | l'entête, le cadre, le thème juste | **773 ms** | 296 |
| **3 · liste peinte** | ses messages | **1 157 ms** | 417 |
| **4 · état stable** | plus rien en vol | ~60 s | ~20 s |

Trois faits à poser avant de choisir :

1. **La tranche WebView2 (B) est incompressible à notre main** :
   278,8 ms au repos, 750,9 ms sous charge d'E/S. Sur un budget de
   1 000 ms, c'est **28 % déjà consommés** avant qu'une ligne de notre
   code ne s'exécute.
2. **Le front n'est pas en cause** : il demande sa première page 75 ms
   après l'arrivée du document.
3. **Prédiction du chantier, à vérifier** : servir la liste en tête de
   file (défaut 03) économise l'attente derrière neuf sondes — la
   requête part à 880 ms aujourd'hui, elle pourrait partir vers 380 ms.
   Le palier 3 tomberait vers **~850 ms**. Sous la seconde, **de
   justesse**, et seulement une fois 01 corrigé (sinon la file se
   reforme derrière le gel).

**Palier retenu par le Chef Ingénieur (D1, 2026-08-26) : le palier 3,
« liste peinte ».** C'est le seul qui décrive ce que l'énoncé du produit
promet — une fenêtre *utilisable* est une fenêtre qui montre ses
messages, pas un cadre vide.

**Conséquence, et elle est franche : le budget est MANQUÉ aujourd'hui**
— 1 157,3 ms au run froid contre 1 000. Un budget dépassé arrête la
ligne (STANDARD §3, andon) : **ce chantier ne se solde pas tant que le
palier 3 n'est pas rendu**, et la mesure finale se prend au banc, à
froid, en médiane sur N lancements — jamais sur un seul.

Deux corollaires à tenir :

1. **Le palier 3 ne tombera pas sous la seconde tant que le gel vit.**
   Servir la liste en tête de file (défaut 03) n'économise l'attente
   des neuf sondes que si la file ne se reforme pas derrière un
   `backfill_status` de 9 secondes. E1 avant E2, dans cet ordre.
2. **Si, une fois 01 et 03 livrés, le palier 3 reste au-dessus de
   1 000 ms**, la question suivante n'est plus la nôtre : la tranche
   WebView2 pèse 278,8 ms au repos et 750,9 ms sous charge d'E/S,
   c'est-à-dire **28 % à 75 % du budget avant qu'une ligne de notre
   code ne s'exécute**. C'est exactement la clause de réouverture du
   refus écrit au §2 (« la coupure environnement / contrôleur »), et
   c'est alors un andon à porter au CE, pas un correctif à improviser.

Les paliers 1, 2 et 4 restent **mesurés et publiés** à chaque banc —
ils ne sont simplement pas opposables.

Les quatre paliers doivent en outre être **horodatés à chaque
lancement** pour qu'un budget soit opposable du tout — c'est la raison
d'être de l'instrument (D5).

---

## 5. Décisions CE

| | | |
|---|---|---|
| **D1** | **Sur quel palier le budget « < 1 s » est-il opposable ?** (§4) | Recommandation : **palier 3, « liste peinte »**. C'est le seul qui décrive ce que l'énoncé du produit promet — « fenêtre utilisable ». Le palier 2 serait plus confortable (773 ms, déjà tenu) mais mesurerait une coquille vide ; le palier 1 ne mesurerait rien. Conséquence assumée : le budget est **manqué aujourd'hui** (1 157 ms) et le chantier doit le rendre, pas le redéfinir. |
| **D2** | **La fenêtre montrée vide et le flash de thème (défauts 04 et 05) entrent-ils dans ce chantier ?** (§3.4) | Recommandation : **le flash de thème OUI, la fenêtre différée NON — pas encore**. Le flash est un défaut pur, cheap, sans arbitrage : le thème doit être posé avant `mount()`. La fenêtre différée est un **échange perceptible** doublé d'un piège mesuré (page cachée = premier rendu retardé) : elle mérite un STOP visuel et une mesure à elle, après que le gel soit mort. La séparer protège aussi le STOP mesuré de E1 — deux changements simultanés ne se départagent pas. |
| **D3** | **L'aperçu stocké derrière le corps (défaut 06) : ici, ou dette ?** | Recommandation : **dette**. Le chiffre honnête est **38 ms dans l'application**, pas les 198 ms du SQL direct — des dizaines de millisecondes contre des milliers pour le défaut 01. Le correctif juste (une table `apercus`, pas une colonne déplacée) porte un invariant de Système et une migration qui **relit tous les corps** : c'est un chantier, pas un appoint. Il redeviendra prioritaire le jour où l'éviction sera décidée. |
| **D4** | **Quelle masse le filet porte-t-il, et où ?** (§3.3) | Recommandation : **la gate porte les tests de plan (coût nul) ; la masse vit dans un décor nommé du banc, joué à la main**. `mesure-v2` gagne deux variables (`MESURE_CORPS`, `MESURE_KO`) et sa valeur par défaut ne change pas — un semis de 8,5 Go à chaque gate coûterait des minutes pour prouver ce qu'un plan d'exécution prouve gratuitement. Le refus symétrique s'écrit : **la gate ne prouve pas la vitesse, elle prouve l'invariant** ; la vitesse se prouve au banc et au terrain. |
| **D5** | **L'instrument reste-t-il au dépôt ?** (feature `mesure`, spike, 9,1 Mo de journaux) | Recommandation : **oui pour l'instrument, oui pour le banc, à alléger pour les journaux**. La feature `mesure` et `armer_les_spans` sont **la seule façon d'asserter un palier de démarrage**, et le binaire livré ne change pas (la release ne passe pas la feature). Les 9,1 Mo de journaux, eux, sont à 95 % des relancements à cache chaud dont le README dit lui-même qu'ils ne représentent pas le terrain : garder `reel-01.log` (le seul froid), `sondes/preflight-cors.log` (qui tranche le doublement), le dépouilleur et les READMEs ; **~350 ko au lieu de 9,1**. Défaut relevé en passant, à corriger si l'instrument reste : la docstring de `depouiller.py:22` annonce un seuil de 50 ms quand le code en pose 1,0 (`SEUIL_PREFLIGHT_MS`) — un outil de mesure se vérifie comme le reste (STANDARD §9). |
| **D6** | **Le ménage mesuré : ici, ou dette ?** — `sync_progress` (152 ms à froid, **toutes les 5 s, à vie**), `db_path()` qui fait un `create_dir_all` par commande sous le verrou déjà pris (`commands.rs:4792`), la colonne fantôme de `echos` | Recommandation : **`db_path` ici** (trois lignes, gratuit, sur le chemin de **chaque** commande) ; **`sync_progress` et la colonne fantôme en dette**. `sync_progress` est un `SUM` de `COUNT` corrélés : son correctif est un compteur tenu à l'écriture — la même famille que l'option C, le même risque de dérive, et il pèse ~1,8 s sur les 60 premières secondes contre ~26 s pour le défaut 01. La colonne fantôme n'est pas un défaut de performance et sa réparation demande une réécriture de table. |

### Verdicts du Chef Ingénieur — 2026-08-26 (STOP 1)

| | réponse | |
|---|---|---|
| **D1** | **« Liste peinte »** | Le budget « < 1 s » porte sur le palier 3. Il est donc **MANQUÉ aujourd'hui** (1 157,3 ms au run froid) : andon au sens du STANDARD §3 — le chantier ne se solde pas tant qu'il n'est pas rendu, mesuré à froid en médiane sur N lancements. Les paliers 1, 2 et 4 restent mesurés, non opposables. *(Décision reposée le 2026-08-26 : le premier clic avait été donné sans que les options soient visibles.)* |
| **D2** | **« Le thème oui, la fenêtre non »** | Le flash de thème est corrigé ici (E3). La fenêtre différée sort du périmètre — le refus est écrit au §2, avec le piège de la page cachée. |
| **D3** | **« Dette »** | L'aperçu derrière le corps ne se traite pas ici. Refus écrit au §2. |
| **D4** | **« Gate = plans, masse au banc »** | La gate porte les tests de plan d'exécution (coût nul) ; `mesure-v2` gagne `MESURE_CORPS` / `MESURE_KO` sans changer son défaut. **La gate prouve l'invariant, pas la vitesse** — la vitesse se prouve au banc et au terrain. |
| **D5** | **« Tout garder »** | L'instrument ET les 9,1 Mo de journaux entrent au dépôt tels quels : tout chiffre du dossier reste redérivable, médianes chaudes comprises, sans relancer une campagne. La docstring fautive de `depouiller.py:22` (seuil annoncé à 50 ms quand le code en pose 1,0) est corrigée dans le même commit. |
| **D8** | **« Le retirer »** | Le critère `AND b.scanned = 1` quitte les deux requêtes. Conséquence assumée, écrite au §3.1 : une base portant des corps d'avant les pièces jointes cesserait de les relire — aucune n'existe, et le code ne peut plus en produire. Tout l'appareil de migration (écran, annulation, `InterruptHandle`, index) devient **hors périmètre**. |
| **D9** | **« L'assumer, et l'inscrire au budget »** | La reconstruction de l'index (1 770 ms au premier lancement après mise à jour, sans écran) est assumée. Inscrite au STANDARD §3 comme ligne de budget mesurée. L'écran de migration est refusé pour cette passe : six failles à refermer pour 1,8 s, et un écran qui clignote est pire que l'attente. |
| **D7** | **« Copie de votre base »** | Le STOP mesuré se prend sur une copie de la base du terrain (12,84 Go), jamais sur la base elle-même, supprimée après mesure — le protocole du 26/08. L'« avant » existe donc déjà ; le protocole ci-dessous dit à quelles conditions l'« après » lui est comparable. |
| **D6** | **« `db_path` ici, les deux autres en dette »** | Le `create_dir_all` par commande meurt ici (trois lignes, sur le chemin de **chaque** commande, sous le verrou déjà pris). `sync_progress` et la colonne fantôme de `echos` partent en dette. |

**D7, posée le 2026-08-26 après STOP 1** — *sur quel décor le STOP
mesuré se prend-il ?* D4 avait tranché que la masse vit au banc ; restait
à dire **laquelle**. Recommandation : **une copie de la base du CE** —
l'« avant » du 26/08 vaut alors directement comme référence, et la
distribution réelle des corps (56 ko de moyenne, 28 Mo au max) donne des
chaînes de débordement qu'un semis uniforme ne reproduit pas.
**Verdict CE : « Copie de votre base ».**

**D8, posée le 2026-08-26 au STOP mesuré** — *le critère
`AND b.scanned = 1` : gardé derrière une migration, ou retiré ?* Trois
mesures l'ont ouverte (§3.1) : la construction d'un index coûterait
18-30 s une fois, et le critère protège **zéro ligne** sur les deux
machines de la flotte, que la production ne peut plus alimenter.
Recommandation : **le retirer**. **Verdict CE : « Le retirer ».**

**Consigné, sans arbitrage** — la dette **D-8 est rouverte** : sa clause
« Rouvre si : le terrain désigne le coût » est remplie, et son chiffre
de 865 ms pour `nav_snapshot` est périmé (à corriger dans le même
geste). Les dettes neuves iront de **D-36**.

---

## 6. Étapes

Rien ne commence avant le GO du CE (STOP 1). Aucun code de production
n'a été écrit.

| | | gate |
|---|---|---|
| **E0** | **L'instrument au dépôt (D5 : tout garder)** : la feature `mesure`, `main.rs`, la ligne `.gitignore`, le banc et les 29 journaux, commités tels quels — plus la **docstring de `depouiller.py:22`** corrigée (elle annonce 50 ms, le code pose `SEUIL_PREFLIGHT_MS = 1.0` ; un outil de mesure se vérifie comme le reste). Rien d'autre ne commence sur un arbre sale. | fmt + clippy + tests Rust |
| **E1** ✅ | **Le compte couvrant — le cœur du chantier.** Livré le 2026-08-26. **RED vu** : la garde de plan a échoué sur `SEARCH b USING INDEX sqlite_autoindex_bodies_1`. **GREEN** : le prédicat extrait en `CORPS_ABSENT` (lu par les deux requêtes ET par la garde), le critère `AND b.scanned = 1` retiré (D8), le test d'héritage réécrit pour porter la décision, le commentaire du `SCHEMA` remis d'équerre. **Aucune migration, aucun index, aucun écran** — l'appareil est mort avec D8. | ✅ fmt 0, clippy 0, **tous les tests Rust verts** |
| **⛔** | **STOP mesuré CE.** Trois chiffres, pris au banc sur décor à masse réelle, **à froid**, avant et après : (a) le coût du **balayage de construction** de l'index, (b) `backfill_status` mesuré par `time.idle`, (c) le palier retenu en D1. Le chantier ne se déroule pas plus loin sans l'arbitrage du CE **sur ces chiffres**. | — |
| **E2** ✅ | **La liste d'abord (défaut 03) — RÉDUIT.** Livré le 2026-08-26 : un `await tick()` après `prete = true`. **Rien n'est différé** (§3.5). RED vu : `list_category` émise en **12e** position, `nav_snapshot` en 2e ; GREEN après. | e2e `demarrage.spec.js` |
| **E3** | **Le flash de thème (D2)** : `fenetre.theme()` attendu avant `mount()`, ou le thème posé côté Rust avant la navigation — le seul canal vivant du thème OS dans WebView2 (A42 : `prefers-color-scheme` ne suit pas l'OS dans `wry`). Plus **le ménage de D6** : `db_path()` cesse de faire un `create_dir_all` par commande (`commands.rs:4792`). | e2e du fichier touché |
| **E4** | *(conditionnel — sur le chiffre de E1)* Le **balayage repayé par lot** (§3.2), voie (i) ou (ii) selon ce que la mesure aura montré. | tests `mail-core` |
| **E5** | **Les paliers instrumentés et la gate de démarrage** : les quatre paliers horodatés, `mesure-v2` doté de `MESURE_CORPS` / `MESURE_KO` (D4), médiane sur N lancements — jamais un seul —, `balayerZombies` appelé, et le **palier 3 « liste peinte » opposable** (D1) — les paliers 1, 2 et 4 mesurés et publiés sans l'être. **La gate du chantier est là** : sous 1 000 ms à froid, sinon andon. | banc |
| **E6** | **Documentation** : Système **A84** (l'invariant du premier écran), **ADR 0027** si le CE valide l'invariant comme structurant, `DETTE.md` (D-8 rouverte et son chiffre périmé corrigé, D-36 et suivantes pour les refus du §2), `STANDARD.md` §3 (le budget du balayage de migration, dans la famille de l'adoption et du FTS) et §9 (l'enseignement de §3.1), `ETAT.md`. | **gate complète, une fois** |

### Le protocole du STOP mesuré — et ses trois pièges

Décor : **une copie** de la base du terrain (D7, 12,84 Go), jamais la
base elle-même ; supprimée après mesure, comme au 26/08.

**Piège 1 — le run 01 de l'« après » n'est PAS comparable au run 01 de
l'« avant ».** La migration s'exécute au **premier `Store::open`** : sur
la copie, le premier lancement porte l'écran, le balayage et la
construction de l'index. Comparer ce run-là aux 8 870 ms du 26/08
conclurait que le correctif a **aggravé** le démarrage. La séquence est
donc en trois temps, et le banc la sert déjà (`banc-reel.ps1 -N`) :

| | Run | Ce qu'on en tire |
|---|---|---|
| **A** | le premier sur la copie | **mesure (a)** : le coût du balayage de construction — le chiffre qui n'a pas d'« avant », et le seul que l'utilisateur paie une fois |
| **B** | le suivant, index en place | **mesures (b) et (c)** : `backfill_status` par `time.idle`, et le palier 3 — **à comparer à `reel/reel-01.log`** |
| **C…N** | les relancements | les p50 à cache chaud — **à comparer aux 18 autres journaux** de `reel/` |

**Piège 2 — la copie fraîche ment, et l'« avant » en est déjà teinté.**
`STANDARD` §9 : une mesure d'I/O disque ne vaut qu'à froid — 0,7 s sur
copie fraîche contre ~4 min au terrain, écart **×340**. La campagne du
26/08 n'a pas redémarré la machine entre la copie et la mesure : les
8 870 ms sont donc un **plancher**, pas un plafond. La règle qui en
découle est la comparabilité avant la fidélité absolue : **l'« après »
reproduit le protocole de l'« avant » à l'identique** (copie, pas de
redémarrage, run A/B/C…N). Un run supplémentaire **après redémarrage
machine** se prend en plus, pour le chiffre honnête — il n'a pas
d'« avant », il borne le monde réel, et il comble la lacune que le
dossier d'instruction déclare lui-même.

**Piège 3 — l'instrument déplace la borne qu'il mesure.** Chaque
`initialization_script` est un aller-retour bloquant **dans** la tranche
mesurée ; les journaux du 26/08 ont été pris **sans CDP**, dont le
surcoût n'est chiffré nulle part. L'« après » se prend donc avec le
**même binaire instrumenté, sans CDP, `balayerZombies` appelé** — un
`msedgewebview2.exe` zombie rend le run tiède, et fausse dans le bon
sens.

---

## 7. Le filet — ce qui naît, et sa preuve de non-vacance

| Test | Ce qu'il asserte | Comment on le prouve non vacant |
|---|---|---|
| **plan · la sonde des corps manquants est couvrante** | L'étape `bodies` de `bodies_pending_count` porte le mot `COVERING` | retirer `INDEXED BY` ⇒ le mot disparaît ⇒ **rouge** |
| **plan · le rattrapage est couvrant** | idem pour `bodies_to_backfill` | idem |
| **migration · l'index absent réclame l'écran** | `pending_adoption` rend `Some(n)` sur une base sans l'index | poser l'index à la main ⇒ rend `None` |
| **migration · l'annulation rembobine** | Le patron ADR 0012 : `user_version` inchangé, aucune adoption partielle | — (patron existant, à rejouer) |
| **e2e · la liste n'attend pas les sondes** | Dans `__e2eJournal`, `list_category` est parmi les **deux premières** commandes émises après `prete`, et **aucune sonde périodique** ne la précède | remettre l'ordre d'origine ⇒ **rouge** |
| **e2e · le palier remplace le délai** | `rattraperCorps` ne part **jamais** avant que la liste soit peinte | remettre `setTimeout(3000)` ⇒ **rouge** |
| **e2e · pas de flash de thème** *(D2)* | Sous `colorScheme: dark`, `documentElement.dataset.theme` porte la nuit **au premier rendu**, et aucune bascule ensuite | retirer l'attente ⇒ **rouge** |

Trois pièges qui invalideraient une mesure, à armer avec le banc : un
`msedgewebview2.exe` zombie rend le run tiède (`balayerZombies` existe —
l'appeler) ; mesurer **sous CDP** a un surcoût que personne n'a chiffré
(les mesures d'aujourd'hui sont **sans** CDP) ; et l'instrumentation
elle-même déplace la borne — chaque `initialization_script` est un
aller-retour bloquant **dans** la tranche mesurée, donc identique entre
tous les runs comparés.

---

## 8. Terrain — la liste de contrôle du Chef Ingénieur

À dérouler **après E1** (STOP mesuré) puis à nouveau en fin de chantier,
sur les vrais comptes, **base réelle**.

1. Le **premier lancement après la mise à jour** : l'écran de migration
   apparaît-il, dit-il quelque chose de vrai, et l'annulation
   rend-elle la main ? *C'est le seul lancement où le balayage se paie.*
2. Le **deuxième lancement** : plus d'écran, et la fenêtre → liste sous
   le budget de D1.
3. **Pendant les vingt premières secondes** : la liste défile, un
   message s'ouvre, la nav répond — *le gel de service est mort ou il ne
   l'est pas ; c'est le point qui décide du chantier.*
4. Le compteur « **N restants** » du rattrapage dit-il toujours la
   vérité ? *La correction ne doit pas avoir menti sur le nombre.*
5. Un **compte fraîchement connecté** (gros retard de corps) : le
   rattrapage avance-t-il sans re-geler à chaque lot ?
6. **Aucune régression de contenu** : les messages sans corps sont
   toujours rattrapés, les pièces jointes toujours détectées — le
   drapeau `scanned` garde son sens.
7. **Second poste x64** : rien ne dit comment ces chiffres s'y
   transposent. Une passe minimale y est due.

### Les commandes, prêtes à copier

L'état du poste — base, version installée, identifiants, traces :

```bash
powershell -ExecutionPolicy Bypass -File scripts\terrain.ps1
```

Le lancement release **avec trace** (l'exe nu ne trace rien — piège
STANDARD §9) :

```bash
powershell -ExecutionPolicy Bypass -File scripts\lancer-wind.ps1
```

Le binaire instrumenté du banc :

```bash
cargo build -p wind-desktop --release --features mesure
```

Le banc sur décor réel (exige une **copie** de la base — la base de
travail n'est jamais ouverte par le banc) :

```bash
powershell -ExecutionPolicy Bypass -File spikes\demarrage\banc-reel.ps1
```

Le dépouillement — les jalons, puis la latence par commande :

```bash
python spikes\demarrage\journaux\depouiller.py jalons reel
```

```bash
python spikes\demarrage\journaux\depouiller.py latences reel
```

La gate complète, en un appel :

```bash
powershell -ExecutionPolicy Bypass -File scripts\gate.ps1
```

---

## 9. Ce que le chantier ne ferme pas

- **Le premier lancement après démarrage machine n'a jamais été
  mesuré.** Tous les chiffres ont été pris binaires WebView2 chauds (six
  processus Edge tournaient) et profil chaud. C'est la mesure qui manque
  pour affirmer quoi que ce soit sur le « démarrage à froid » du budget.
- **`[CreateProcess → première ligne de `main()`]`** — loader Windows,
  DLL, CRT — reste hors d'atteinte sous `unsafe_code = "forbid"`.
  Contournement possible depuis le banc, par `(Get-Process).StartTime`.
- **Aucune mesure sous contention** : un seul processus, aucune synchro
  concurrente tenant le verrou d'écriture. Or c'est exactement la
  situation du démarrage.
- **Un seul poste, ARM64.**
- **Aucun correctif n'a encore été essayé.** Les gains cités sont des
  variantes de requête mesurées — une borne solide, pas une preuve.
