# Spike S1 — fenêtrage de la Réception organisée : au service ou à l'affichage

Départager, par des chiffres, où vivent les deux mécanismes du mode
organisé (PLAN-MODE-ORGANISE) :

- les **sections** « Nouveau pour vous » (non-lus) / « Déjà consulté » ;
- le **repli** « un expéditeur groupé = UNE rangée ».

Deux familles d'options : **au service de page** (SQL, dans
`unified_page_sql`) ou **à l'affichage** (post-traitement JS des lignes
servies par `Liste.svelte`, PAGE=200).

```powershell
cargo run --release -- target/fenetrage.db target/rows.json
node affichage.mjs target/rows.json
```

## Protocole

- **Machine** : poste de dev x64 (Windows 11 Home), build `--release`,
  SQLite 3.50.2 (rusqlite bundled). Base sur disque, **cache chaud**
  (3 échauffements, 20 itérations, médiane/p95 ; `prepare` inclus dans
  la mesure, comme en production). Le chemin passe par OneDrive — sans
  effet mesuré à cache chaud, mais un froid n'a PAS été mesuré.
- **Base synthétique** (LCG déterministe) : 200 000 enveloppes, 1 fil
  par message (200 000 conversations — le pire cas en nombre de
  rangées), 2 000 expéditeurs, **24 002 non-lus (12 %,** taux du
  terrain : 331/2 929**)**. 5 « bavards » marqués groupés à ~600
  messages chacun ; le 5e en **rafale : 600 messages sur 12 h** il y a
  30 jours — le cas défavorable du repli à l'affichage.
- **Requête témoin** : `unified_page_sql(false, false)` reproduite à
  l'identique (SELECT_UNIFIED, exclusion PINNED_THREADS avec CROSS
  JOIN, sous-requête paginée sur `threads`, jointures sur les 200
  lignes retenues). Schéma et index de prod reproduits ; pas
  d'`ANALYZE` (la prod ne l'exécute jamais). Un seul vol de page par
  mesure ; les lignes sont consommées et décodées.

## Chiffres (2026-08-29)

### Témoin V0 — la page de prod reproduite

| Mesure | méd | p95 |
|---|---|---|
| offset 0 | 1,69 ms | 2,47 ms |
| offset 100 000 | 7,47 ms | 8,58 ms |

Plan : `SCAN threads USING INDEX idx_threads_date_globale`, jointures
sur 200 lignes. Cohérent avec les mesures de prod.

### Sections AU SERVICE — deux requêtes bornées (A1)

Le squelette existe déjà (`non_lues` filtre `unseen > 0`) ; la section
« Déjà consulté » est le même squelette avec `unseen = 0`.

| Mesure (méd / p95) | index de prod seuls | + 2 index partiels dédiés¹ |
|---|---|---|
| non-lus, offset 0 | 5,95 / 7,06 ms | **1,69 / 2,17 ms** |
| non-lus, offset 23 752 (fond de section) | **538,67 / 565,64 ms** | **2,88 / 3,89 ms** |
| lus, offset 0 | 1,74 / 2,13 ms | 1,64 / 2,35 ms |
| lus, offset 100 000 | **309,92 / 335,78 ms** | **7,52 / 8,14 ms** |
| COUNT non-lus (la couture entre sections) | — | 0,37 / 0,38 ms |

¹ `CREATE INDEX … ON threads(last_epoch DESC, last_uid DESC, account_id)
WHERE inbox_size > 0 AND unseen > 0` (resp. `= 0`). Sans eux, le filtre
s'évalue ligne à ligne sur `idx_threads_date_globale` : l'offset paie
chaque ligne sautée ET rejetée — 539 ms au fond des non-lus.

Variante A2 (UNE requête, `ORDER BY (unseen > 0) DESC, last_epoch DESC,
…`) : **548 ms à CHAQUE page** sans index (`USE TEMP B-TREE FOR ORDER
BY` — le tri de toute la boîte, l'interdit) ; avec UN index
d'expression partiel `((unseen > 0) DESC, last_epoch DESC, last_uid
DESC, account_id) WHERE inbox_size > 0` : **1,85 ms** (offset 0),
**6,77 ms** (offset 100 000) — même profil que V0, un seul flot, un
seul offset, la couture donnée par le COUNT à 0,37 ms.

Ce que la variante impose au fenêtrage : A1 = deux offsets et une
bascule à `COUNT(non-lus)` ; A2 = **rien ne change** (un flot, un
offset, l'ordre porte les sections).

### Sections À L'AFFICHAGE (fait, sans variante à défendre)

Les non-lus vivent à toute profondeur du flot servi :

- le **200e** fil non lu est au rang **1 693** → **9 vols** de 200
  lignes pour remplir la PREMIÈRE page de la section « Nouveau pour
  vous » ;
- le dernier non-lu est au rang 199 998 → la section complète exige
  **1 000 vols** (toute la boîte).

Servi/affiché : 1 693 lignes pour 200 rangées (8,5×). Infaisable sans
violer « un seul vol de page ».

### Repli de groupe AU SERVICE — naïf (B), puis industrialisé (B'')

**B — sans dénormalisation** (le flot non groupé exige une jointure
`envelopes` par fil pour connaître l'expéditeur, puis `UNION ALL` avec
l'agrégat `GROUP BY sender_address`, tri externe) :

| Mesure | méd | p95 |
|---|---|---|
| offset 0 | **1 570,09 ms** | 1 799,28 ms |
| offset 5 000 | 1 507,06 ms | 1 586,55 ms |
| offset 100 000 | 1 510,48 ms | 1 541,34 ms |
| COUNT total organisé (l'offset stable l'exige) | **1 508,12 ms** | 1 559,99 ms |

Plan : `USE TEMP B-TREE FOR ORDER BY` sur le compound — SQLite
matérialise et trie les ~197 000 lignes du UNION à chaque page, dès
l'offset 0. **Infaisable en l'état** : c'est le tri de toute la boîte
par une autre porte. La question dure de l'offset (500 messages = 1
rangée) ne se pose même pas ici — le mur est avant.

**B'' — industrialisé** : drapeau `threads.groupe` précalculé + index
partiel `WHERE inbox_size > 0 AND groupe = 0` sur la clé de tri ; les
rangées de groupe sortent du flot paginé et se servent À PART (même
motif que les épingles R4), depuis un agrégat matérialisé
`groupes_agg(address, n, last_epoch, last_uid)` :

| Mesure | méd | p95 |
|---|---|---|
| flot hors groupes, offset 0 | 1,62 ms | 1,93 ms |
| flot hors groupes, offset 100 000 | 6,59 ms | 7,61 ms |
| rangées de groupe (5, matérialisées) | 0,00 ms | 0,00 ms |
| (borne haute : agrégat recalculé à la volée sur 3 000 msgs) | 10,72 ms | 11,26 ms |

**L'offset redevient stable par construction** : les messages groupés
ne comptent plus DANS le flot ; 500 messages qui font 1 rangée ne
décalent rien, la rangée s'insère à l'affichage à sa date (comme les
épingles s'insèrent en tête). Le total = COUNT du flot filtré +
nombre de groupes.

### Repli de groupe À L'AFFICHAGE (C) — simulation Node

Page servie telle quelle (V0), repli en JS par vol de 200 (état de
session : expéditeurs groupés déjà vus). `node affichage.mjs` :

- post-traitement : **méd 0,004 ms, p95 0,031 ms** par vol —
  négligeable, ce n'est pas là que ça se joue ;
- rangées affichables par vol : **moyenne 197,0/200**, mais **min
  32/200** ;
- **la rafale (600 messages / 12 h) : 5 vols traversés (#39..#43,
  affichables 109, 35, 32, 39, 176) pour UNE rangée affichée** —
  600 lignes servies, rendement 0,17 %. L'utilisateur qui défile dans
  cette zone déclenche ~5 vols pour remplir son écran : le « un seul
  vol de page » saute ;
- le compte `n` de la rangée de groupe ne peut être que « servi
  jusqu'ici » (35 au premier vol, pas 600) — le vrai total exige une
  requête à part de toute façon.

## Ce que chaque variante impose au fenêtrage (synthèse factuelle)

| Variante | ms/page (méd, chaud) | servi/affiché | offset | totaux |
|---|---|---|---|---|
| Sections à l'affichage | 1,69–7,47 (V0) | **8,5× pour la 1re page non-lus** | multi-vols en cascade | gratuits mais faux (partiels) |
| Sections au service (A1, 2 requêtes + 2 index) | 1,69–7,52 | 1× | 2 offsets + couture COUNT (0,37 ms) | COUNT non-lus 0,37 ms |
| Sections au service (A2, 1 requête + 1 index d'expression) | 1,85–6,77 | 1× | **inchangé (1 offset)** | idem |
| Repli au service naïf (B) | **~1 510 à TOUTE page** | 1× | stable mais COUNT à 1,5 s | 1,5 s |
| Repli au service industrialisé (B'') | 1,62–6,59 + 0 (groupes à part) | 1× | **stable par construction (groupes hors flot)** | COUNT flot + 5 |
| Repli à l'affichage (C) | V0 + 0,004 | moy. 197/200, **min 32/200 ; rafale : 5 vols → 1 rangée** | dérive (l'affiché ≠ le servi) | n de groupe faux sans requête à part |

## Coût d'industrialisation estimé (non mesuré, sur pièces)

- **A1/A2** : 1 à 2 `CREATE INDEX` partiels sur `threads` (colonnes
  existantes, pas de migration de données ; A2 demande un index
  d'EXPRESSION — première du genre dans le schéma), le paramètre
  d'ordre dans `unified_page_sql`, la garde de plan étendue. Petit.
- **B''** : colonne `threads.groupe` (ou `sender_address`) maintenue
  dans la transaction d'écriture — même discipline que les agrégats
  `size`/`unseen` ; table `groupes_agg` maintenue pareil ; index
  partiel ; service « à part » calqué sur `pinned_unified_scoped`.
  Moyen — et la définition « l'expéditeur DU fil » reste à trancher
  pour un fil multi-expéditeurs (le spike est 1 message = 1 fil).

## Limites — ce qui invaliderait ces chiffres

- **Cache chaud uniquement** ; un premier affichage à froid n'est pas
  couvert (leçon STANDARD §9 : le cache chaud ment sur les froids).
- 1 message = 1 fil (200 k conversations, majorant) ; pas de fils
  multi-expéditeurs, `pins` vide, 1 seul compte.
- Pas d'`ANALYZE` (comme la prod) — un jour d'`ANALYZE` changerait les
  plans, notamment B.
- Répartition : 12 % non-lus uniformes ; des non-lus concentrés en
  tête (boîte « bien tenue ») réduiraient l'écart A1-sans-index, sans
  changer le classement.
- SQLite 3.50.2 bundled ; base sous OneDrive (lecture chaude seulement).

Spike jetable — aucun fichier de production touché, workspace `[ ]`
propre (même isolement que `spikes/search-engine`).
