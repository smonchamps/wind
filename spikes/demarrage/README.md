# Spike PLAN-DEMARRAGE — le coût du chemin d'ouverture

**Jetable.** Ce banc ne livre rien : il instruit le constat terrain
« freeze et lenteurs au démarrage, après l'ouverture de la fenêtre ».
Il ne touche ni `crates/`, ni `apps/`, ni `docs/`, et il ne compile pas
dans le `target/` du workspace (clé `[workspace]` vide dans
`Cargo.toml`, sur le modèle de `spikes/idle`).

## Le décor

- Machine : Snapdragon X Elite X1E80100 (12 cœurs, 3,40 GHz), 15,6 Gio,
  Windows 11 ARM64.
- Bases fabriquées par la sous-commande `seed`, **recopie littérale** de
  `crates/mail-core/examples/seed_inbox.rs` (mêmes constantes, même
  distribution, mêmes 500 corps). Recopiée plutôt qu'appelée via
  `cargo run -p mail-core --example` pour ne pas prendre le verrou cargo
  du workspace pendant qu'un autre agent y mesure.
- Quatre tailles : 2 000 / 20 000 / 50 000 / 250 000 messages, une seule
  boîte INBOX, un seul compte. La base réelle de l'utilisateur
  (`%APPDATA%`) n'a **jamais** été ouverte.
- Toujours `--release`. Chauffe systématique avant chronométrage
  (20 tours pour `open`, 10 pour `ventilation`, 3 pour `rafale`).
  p50 **et** p95, jamais une moyenne seule (STANDARD §9).

## Reproduction

```powershell
cd spikes/demarrage
cargo build --release
./target/release/spike-demarrage.exe seed        ./petit.db    2000
./target/release/spike-demarrage.exe seed        ./moyen.db   20000
./target/release/spike-demarrage.exe seed        ./gros.db    50000
./target/release/spike-demarrage.exe seed        ./terrain.db 250000
./target/release/spike-demarrage.exe open        ./terrain.db 100
./target/release/spike-demarrage.exe ventilation ./terrain.db 100
./target/release/spike-demarrage.exe fils        ./terrain.db 100
./target/release/spike-demarrage.exe rafale      ./terrain.db  20
./target/release/spike-demarrage.exe rafale1     ./terrain.db  30
./target/release/spike-demarrage.exe requetes    ./terrain.db  50
./target/release/spike-demarrage.exe colonnes    ./petit.db
```

Les `.db` ne sont pas laissés sur le disque (150 Mio pour `terrain.db`,
dans un dossier OneDrive) : les regénérer prend 12 s.

## 1. Le coût d'un `Store::open` sur une base À JOUR

`crates/mail-core/src/store.rs:582`. Aucune migration à faire, aucune
adoption à faire — la base est au schéma courant, `PRAGMA user_version`
est à jour, la requête d'adoption rend **zéro ligne**.

| messages | p50 | p95 |
|---|---|---|
| 2 000 | **1,09 ms** | 1,23 ms |
| 20 000 | **5,29 ms** | 6,79 ms |
| 50 000 | **12,15 ms** | 12,59 ms |
| 250 000 | **65,96 ms** | 75,08 ms |

C'est **linéaire en nombre de messages** : ~0,26 µs par message, plus
~0,9 ms de plancher fixe. Sur une base à jour, où il n'y a rien à faire.

Rappel de l'enjeu : `apps/desktop/src/commands.rs` contient **76**
`Store::open`, et toutes les commandes passent par `hors_pompe`
(`commands.rs:4768`), qui les sérialise derrière un mutex global.

## 2. La ventilation (base à jour, 250 000 messages)

Séquence de `Store::init_with` (`store.rs:735`), refaite pas à pas avec
`rusqlite`. Le `SCHEMA` n'est pas retranscrit à la main : il est
**extrait du source** par `include_str!` pour ne pas mesurer autre chose
que la production.

Sur 50 000 messages (n=200) :

| étape | p50 | p95 |
|---|---|---|
| 1. `Connection::open` | 0,087 ms | 0,154 ms |
| 2. `busy_timeout(30 s)` | 0,000 ms | 0,000 ms |
| 3. `PRAGMA journal_mode = wal` | 0,465 ms | 0,681 ms |
| 4. `execute_batch(SCHEMA)` | 0,112 ms | 0,181 ms |
| 5. `migrate()` (sondes seules) | 0,142 ms | 0,204 ms |
| 6. **bloc `BEGIN..fils..COMMIT`** | **11,325 ms** | 13,904 ms |
| 7. `rattraper_correspondants()` | 0,013 ms | 0,042 ms |
| somme | 12,169 ms | 15,171 ms |

La somme des sept étapes (12,17 ms) recouvre le `Store::open` complet
mesuré séparément (12,15 ms) : la recopie est fidèle, rien n'a été
oublié.

**L'étape 6 domine, et elle est la seule à croître avec la taille.**
Découpée (`fils`), sur 250 000 messages (n=100) :

| sous-étape | p50 | p95 |
|---|---|---|
| 6a. `BEGIN` | 0,001 ms | 0,001 ms |
| 6b. `PRAGMA user_version` | 0,016 ms | 0,020 ms |
| 6c. `execute_batch(thread::SCHEMA)` | 0,033 ms | 0,057 ms |
| 6d. **requête d'adoption (`thread::orphans`)** | **64,31 ms** | 71,70 ms |
| 6e. `COMMIT` | 0,013 ms | 0,016 ms |

**Le coupant est une seule requête**, `thread::orphans`
(`crates/mail-core/src/thread.rs:576`), qui rend **zéro ligne** et coûte
64 ms. Son plan, mesuré par `EXPLAIN QUERY PLAN` :

```
SCAN m
SEARCH e USING INDEX idx_envelopes_date (mailbox_id=?)
USE TEMP B-TREE FOR ORDER BY
```

Le `CROSS JOIN` fait bien piloter le parcours par les boîtes en portée
(l'optimisation consignée en commentaire, thread.rs:566-580), mais il
parcourt ensuite **toutes les enveloppes de ces boîtes** pour écarter
ligne à ligne celles dont `thread_id` n'est pas NULL. Le prix suit le
nombre de messages en portée, jamais le nombre d'orphelins — et il est
payé **à chaque `Store::open`, donc à chaque commande.**

## 3. L'effet de la taille : oui, entièrement

`Store::open` : 1,09 → 5,29 → 12,15 → 65,96 ms pour 2 000 → 20 000 →
50 000 → 250 000. Les étapes 1 à 5 et 7 sont plates (~0,9 ms au total,
quelle que soit la taille) ; c'est l'étape 6d qui porte toute la pente.

**Contrôle OneDrive** — le dépôt vit dans OneDrive, ce qui pouvait
fausser la mesure. Même base de 50 000 messages, mesurée aux deux
emplacements : OneDrive p50 = 13,32 ms, disque local hors OneDrive
p50 = 13,44 ms. **Aucun effet mesurable** ; l'emplacement n'est pas un
facteur.

## 4. La rafale sérialisée

Douze « commandes » qui ouvrent chacune leur `Store` puis font leur
requête, en série — le squelette de `hors_pompe` + `Store::open`, sans
Tauri. C'est le temps pendant lequel l'UI attend.

| base | rafale (12 cmd) p50 | p95 | plafond : 12 requêtes sur **une** connexion |
|---|---|---|---|
| 2 000 | 16,03 ms | 16,43 ms | 0,58 ms |
| 50 000 | 157,93 ms | 171,88 ms | 9,61 ms |
| 250 000 | **844,77 ms** | 931,43 ms | 55,68 ms |

Sur 250 000 messages, **96 % de la rafale est de la ré-ouverture**
(845 ms contre 56 ms de travail utile). Douze commandes est une
hypothèse basse posée par ce banc : le nombre réel de commandes émises
au démarrage n'a pas été mesuré ici.

## 5. Le coût des requêtes elles-mêmes (connexion déjà ouverte)

| requête | 2 000 | 50 000 | 250 000 |
|---|---|---|---|
| `nav_unread_counts` | 0,224 ms | 7,71 ms | **42,02 ms** |
| `unified_recent_scoped(0, 50)` | 0,218 ms | 0,50 ms | 0,40 ms |
| `unified_count_scoped` (total exact) | 0,088 ms | 2,84 ms | 12,98 ms |

Deuxième point dur, indépendant du premier : `nav_unread_counts`
(`crates/mail-core/src/nav.rs:301`) coûte 42 ms sur 250 000 messages
même connexion déjà ouverte. La page de 50 lignes, elle, est
**constante** — la pagination tient sa promesse.

## Trouvaille annexe (hors perf, non corrigée)

Le littéral `SCHEMA` de `store.rs` contient, ligne 257, un `\n` **dans
un commentaire SQL** :

```
    -- enveloppes (adresses jointes par '\n') — la liste d'Envoyes dit
```

En Rust, `\n` devient un vrai saut de ligne : le commentaire `--` se
termine là, et SQLite avale la suite comme une **colonne fantôme** de la
table `echos`. Vérifié par `PRAGMA table_info(echos)` sur une base créée
par le vrai `Store::open` :

```
colonne ") — la liste d" type "Envoyes dit\n    -- « A : X » … to_addrs         TEXT"
```

La colonne `to_addrs` déclarée dans `SCHEMA` est donc absorbée par ce
type ; la vraie `to_addrs` de `echos` n'existe que parce que
`add_missing_columns` la rajoute plus tard (`store.rs`, migration
« Destinataires de l'echo »). Sans conséquence de performance, mais
c'est une déclaration de schéma qui ne dit pas ce qu'elle croit dire.

## Ce que ces mesures NE disent PAS

- Combien de commandes le démarrage émet réellement (12 est une
  hypothèse de banc, pas un relevé).
- Le coût du côté WebView2/Svelte, du réseau, de la synchro IMAP, de la
  mise à jour automatique — rien ici ne touche au réseau.
- Le comportement sur une base **héritée** (migration ou adoption à
  faire) : tout est mesuré sur des bases **à jour**, cas nominal.
- Le décor est mono-compte, mono-boîte, corps minuscules. Une vraie
  boîte multi-comptes avec corps de ~34 Ko a un fichier bien plus gros —
  le coût de 6d suit le nombre de messages en portée, pas les octets,
  mais cela n'a pas été vérifié sur un tel décor.
- Rien n'a été mesuré sous contention : un seul processus, pas de synchro
  concurrente tenant le verrou d'écriture.

## Note

`.gitignore` ignore `/target` à la racine et liste les `target/` des
autres spikes un par un : `spikes/demarrage/target/` n'y est pas. Si ce
banc est conservé, il lui faut sa ligne. Rien n'a été commité.
