# Banc S3 — que coûte le préchargement des corps du Kiosque ?

Spike du PLAN-MODE-ORGANISE, décision CE D5 : si le Kiosque précharge
les corps des lettres d'information, il le fait **borné à la page
servie**, sur le patron d'`enrichir_lignes`
(`crates/mail-core/src/nav.rs:421` — lectures indexées par
`thread_id IN (...)`, jamais dans la requête chaude qui pagine).
Question à trancher sur chiffres : ce préchargement tient-il dans un
budget qui ne menace pas le chemin chaud de la liste (~quelques ms) ?

## Ce que la lecture du code a établi (avant de mesurer)

- Les corps vivent dans `bodies(mailbox_id, uid, html TEXT NOT NULL,
  scanned, preview TEXT)`, PK `(mailbox_id, uid)`
  (`crates/mail-core/src/store.rs:103`). Le HTML est stocké **entier,
  en clair, dans la ligne** — pas de table externe, pas de compression.
- `message_body` sert un corps par `Store::body`
  (`store.rs:1749`) : jointure `mailboxes` (compte + nom) puis sonde
  de la PK de `bodies`. Unitaire, une ligne.
- Taille typique d'un corps réel : **59,2 Ko en moyenne** (mesure
  courrier Gmail réel, `spikes/body-backfill/README.md`) — la
  fourchette 30-150 Ko du banc encadre ce chiffre.
- Index disponibles pour un lot borné à la page :
  `idx_envelopes_thread(thread_id, date_epoch DESC)` (`store.rs:3074`)
  et la PK de `bodies`.

## Protocole

```powershell
cd spikes/kiosque-precharge
cargo run --release -- "$env:TEMP\kiosque-banc.db"   # seed au 1er passage
```

- **Machine** : Snapdragon X Elite X1E80100 (12 cœurs, ARM64),
  15,6 Go RAM, SSD NVMe Samsung MZ9L4512 ; Windows 11, 2026-08-29.
  Base posée dans `%TEMP%` local (hors OneDrive).
- **Base synthétique** : schéma copié de `store.rs`/`thread.rs`
  (colonnes et index identiques pour les tables traversées), WAL comme
  en prod. 200 000 enveloppes / 200 000 fils, dont **2 000 lettres
  d'information** à corps HTML réalistes (tables imbriquées, styles
  inline, 30-150 Ko, dispersées dans tout le fichier — 1 sur 100) et
  ~20 % des messages ordinaires avec un corps de 2-8 Ko. Fichier
  final : **479,8 Mo** (seed 6,4 s).
- **Page du Kiosque** : les 20 / 50 lettres les plus récentes,
  identiques à toutes les itérations.
- **20 itérations** par scénario ; médiane, p95, min, max.
- **Froid** : purge du cache fichier Windows (ouverture
  `FILE_FLAG_NO_BUFFERING` puis fermeture, sur `.db` + `-wal` +
  `-shm`) et connexion SQLite neuve à chaque itération. Ce n'est pas
  le froid post-redémarrage du STANDARD §9, mais le pire cas d'une
  session déjà lancée : pages du fichier évincées. **Preuve que la
  purge opère** : écart froid/chaud ×10-20 sur tous les bancs.
- **Chaud** : même connexion, un échauffement, puis 20 itérations.

## Chiffres bruts (2026-08-29)

### 1-2. Lecture des corps d'une page en un lot

| Banc | Mode | méd ms | p95 ms | min | max | Ko/page |
|---|---|---:|---:|---:|---:|---:|
| lot PK `(mailbox_id,uid) IN (VALUES …)`, page 20 | froid | 14,11 | 21,86 | 12,48 | 42,91 | 1 600 |
| — | chaud | 1,09 | 1,36 | 0,93 | 1,36 | 1 600 |
| lot PK, page 50 | froid | 33,72 | 46,25 | 29,56 | 52,37 | 4 275 |
| — | chaud | 6,66 | 8,03 | 4,94 | 8,27 | 4 275 |
| lot fils `e.thread_id IN (…) JOIN bodies` (patron enrichir_lignes), page 20 | froid | 12,24 | 19,60 | 10,88 | 24,04 | 1 600 |
| — | chaud | 1,81 | 2,74 | 1,67 | 2,80 | 1 600 |
| lot fils, page 50 | froid | 29,59 | 39,62 | 24,73 | 45,07 | 4 275 |
| — | chaud | 3,86 | 4,67 | 2,67 | 4,68 | 4 275 |

Poids transféré : **1,6 Mo pour 20 rangées, 4,3 Mo pour 50** —
~80 Ko/corps en moyenne sur cette page (tirage 30-150 Ko).

### 3. Variante « aperçu d'abord, corps à l'approche du viewport »

Corps **unitaire** par le chemin exact de `Store::body` (jointure
`mailboxes` puis PK), 20 corps distincts de 33,8-147 Ko :

| Mode | méd ms | p95 ms | min | max |
|---|---:|---:|---:|---:|
| froid | 1,02 | 1,47 | 0,69 | 1,77 |
| chaud | 0,05 | 0,08 | 0,03 | 0,09 |

L'aperçu, lui, est **déjà dans la page servie** : `SELECT_UNIFIED`
embarque `b.preview` (`store.rs:555`) — la variante aperçu-d'abord ne
coûte aucune lecture supplémentaire au chargement de la page.

### 4. EXPLAIN QUERY PLAN

```
lot PK (VALUES) :
  SEARCH bodies USING INDEX sqlite_autoindex_bodies_1 (mailbox_id=? AND uid=?)
  LIST SUBQUERY 2 / SCAN 20-ROW VALUES CLAUSE

lot fils (patron enrichir_lignes) :
  SEARCH e USING INDEX idx_envelopes_thread (thread_id=?)
  SEARCH b USING INDEX sqlite_autoindex_bodies_1 (mailbox_id=? AND uid=?)

unitaire (Store::body) :
  SEARCH m USING COVERING INDEX sqlite_autoindex_mailboxes_1 (account_id=? AND name=?)
  SEARCH b USING INDEX sqlite_autoindex_bodies_1 (mailbox_id=? AND uid=?)
```

Aucun scan : toutes les lectures sont des sondes d'index, dans les
deux variantes de lot comme en unitaire. Le lot par fils n'est pas
plus cher que le lot par PK (mêmes ordres de grandeur froid ; il paie
une sonde d'`idx_envelopes_thread` de plus par rangée, ~0,7-2 ms
cumulées à chaud).

## Ce qui invaliderait ces chiffres

- **Disque plus lent** : banc sur NVMe. Le froid est dominé par la
  relecture des pages du fichier (~1 600 pages de 4 Ko dispersées pour
  20 corps) ; un SATA ou un disque chiffré/throttlé multiplierait le
  froid, pas le chaud. Le poste x64 du terrain doit confirmer avant de
  graver un budget.
- **Clustering favorable** : la base est écrite en un seul lot, les
  lignes de `bodies` sont physiquement contiguës par date. Une base
  réelle accrète par synchro (ordre proche de la date, mais avec
  fragmentation) ; le froid réel peut être un peu au-dessus.
- **Corps plus gros que 150 Ko** : le courrier marchand réel dépasse
  parfois 500 Ko ; le coût est linéaire au poids (≈ 8 ms/Mo froid,
  ≈ 1,5 ms/Mo chaud sur ce banc).
- **Froid ≠ post-redémarrage** : STANDARD §9. La purge
  `FILE_FLAG_NO_BUFFERING` évince les pages du fichier, pas les caches
  intermédiaires du contrôleur. L'écart ×340 de PLAN-RECHERCHE
  concernait 7 Go relus en entier — ici on relit ≤ 4,3 Mo bornés,
  l'échelle du risque n'est pas la même, mais le chiffre froid reste
  un plancher optimiste.

## Coût d'industrialisation estimé (pas un avis sur l'option)

- **Lot borné à la page** : une méthode `Store` sur le patron
  d'`enrichir_lignes` (une requête préparée, `params_from_iter`),
  une commande Tauri, branchement au rendu du Kiosque. Aucune
  migration de schéma, aucun index à créer — tout existe.
  Ordre de grandeur : petit (½ journée avec tests).
- **Unitaire au viewport** : `message_body` existe tel quel ;
  le coût est côté UI (IntersectionObserver + annulation). Ordre de
  grandeur : petit côté noyau (zéro), moyen côté UI.
- **Hors de ce banc** : le coût de *rendu* WebView de 20 iframes de
  80 Ko (mémoire, layout) n'est pas mesuré ici — c'est un autre banc
  si l'option « tout rendre d'avance » reste en course.
