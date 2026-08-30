# Spike S2 — plan SQLite du routage d'expéditeurs (PLAN-MODE-ORGANISE)

**Question** : l'exclusion « expéditeur retenu au Portier » et le filtre
de destination tiennent-ils dans les requêtes chaudes sans régression
mesurable, et SQLite sonde-t-il `routage_expediteurs` par sa clé
primaire ou scanne-t-il `envelopes` ?

Jetable. Aucun code de production touché. Modèle : ADR 0004.

## Protocole

- **Machine** : poste x64 (Windows 11 Home 26200), dépôt sous OneDrive,
  mesure du 2026-08-29. Base **chaude** (5 échauffements, WAL) — le
  cache froid n'est pas mesuré ici.
- **Moteur** : SQLite **3.51.2** via `node:sqlite` (Node v24.14.0). La
  prod parle à SQLite via rusqlite (bundled) ; même moteur, versions
  proches — les plans sont comparables, les temps absolus indicatifs.
- **Base** : `banc.db` — 200 000 enveloppes semées par
  `cargo run -p mail-core --example seed_inbox --release -- spikes/routage-plan/banc.db 200000 seed@exemple.fr 0 0 INBOX`
  (schéma et index EXACTS de la prod, fils construits par `thread.rs`),
  puis `bench.mjs` réécrit `sender_address` en **2 020 adresses
  distinctes** (distribution zipf-ienne : 20 grosses adresses portent la
  moitié du courrier) et crée
  `routage_expediteurs(adresse TEXT PRIMARY KEY, destination, regle, decide_epoch)`
  avec **50 adresses routées** (25 kiosque / 25 registres) couvrant
  **52 000 messages** (26 % — délibérément dur : les newsletters routées
  sont les grosses adresses).
- **Requêtes** : la page unifiée est la copie conforme de
  `unified_page_sql(false, false)` (store.rs:2674, SELECT_UNIFIED l.555,
  PINNED_THREADS l.575, UNIFIED_JOIN_TAIL l.582). 20 itérations
  chronométrées, médiane et p95. Rejouer : `node bench.mjs banc.db`
  (chemins relatifs au dépôt : `spikes/routage-plan/...`).

## Chiffres (médiane / p95, ms)

### Page unifiée (LIMIT 50)

| Cas | Variante | Médiane | p95 | Plan (l'essentiel) |
|---|---|---:|---:|---|
| U0 | **Témoin existant**, offset 0 | **0,228** | 0,837 | SCAN `idx_threads_date_globale`, sondes PK |
| U0d | Témoin, offset 100 000 | 6,675 | 8,852 | idem (coût = le saut d'index) |
| U1 | + `NOT EXISTS (r WHERE r.adresse = e.sender_address)` | **0,209** | 0,220 | `SEARCH r USING COVERING INDEX sqlite_autoindex_routage_expediteurs_1 (adresse=?)` |
| U2 | + `LEFT JOIN r … WHERE r.adresse IS NULL` | **0,209** | 0,267 | idem, sonde PK en LEFT-JOIN |
| U3 | + `NOT EXISTS` **avec époque** (`e.date_epoch > r.decide_epoch`) | **0,178** | 0,182 | idem, sonde PK |
| U4 | Exclusion **par fils** façon pins : `t.id NOT IN (SELECT re.thread_id FROM r CROSS JOIN envelopes re ON re.sender_address = r.adresse)`, sans index | 270,229 | 289,396 | `AUTOMATIC PARTIAL COVERING INDEX` reconstruit à CHAQUE requête |
| U4i | idem, avec `INDEX envelopes(sender_address)` | 101,465 | 105,436 | sonde l'index puis rappelle 52 000 lignes pour lire `thread_id` |
| U4c | idem, avec index **couvrant** `(sender_address, thread_id)` | 13,512 | 18,916 | COVERING, mais matérialise 52 000 thread_id par page |

### Kiosque et totaux

| Cas | Variante | Médiane | p95 | Plan |
|---|---|---:|---:|---|
| K1 | Page Kiosque : tranche `category_page` + `sender_address IN (SELECT adresse FROM r WHERE destination='kiosque')` | **0,087** | 0,095 | `SEARCH idx_envelopes_date` + bloom filter sur la liste des 25 adresses ; l'index sender n'y change rien (K1i : 0,087) |
| T0 | **Témoin** `category_totals` existant (COUNT + SUM sur 200 k) | 49,092 | 50,975 | `SEARCH idx_envelopes_date (mailbox_id=?)` |
| T1 | + exclusion `NOT EXISTS` | 67,112 | 73,057 | + sonde PK de `r` par ligne (**+18 ms**) |
| T2 | Totaux du Kiosque (COUNT filtré destination) | 60,009 | 69,175 | idem T0 + bloom filter (index sender sans effet : T2i 60,138) |

Plans bruts complets : sortie de `bench.mjs` (rejouable à l'identique,
tout est déterministe).

## Verdict factuel par variante

1. **Exclusion par message (U1/U2/U3) : coût nul mesuré.** SQLite sonde
   `routage_expediteurs` par sa PK (`SEARCH r USING COVERING INDEX
   … (adresse=?)`) sur les 50 lignes retenues seulement — jamais un scan
   d'`envelopes`. **Aucun `CROSS JOIN` directif nécessaire** : la table
   n'entre qu'en sous-requête corrélée/LEFT JOIN sur des lignes déjà
   paginées, le piège des pins (choix de table extérieure) ne se pose
   pas. `NOT EXISTS`, `LEFT JOIN` et la variante avec époque sont
   indistinguables (0,18–0,21 ms vs témoin 0,23 ms).
   **Réserve sémantique** : posée APRÈS le `LIMIT`, l'exclusion rend des
   pages courtes (37 lignes sur 50 ici) — l'industrialisation doit soit
   filtrer en amont (voir U4), soit accepter/compenser les trous, soit
   retirer les messages routés du fil au moment du routage.
2. **Exclusion par fils façon pins (U4) : disqualifiée en l'état.**
   270 ms sans index (index automatique reconstruit par requête),
   101 ms avec index simple, et encore **13,5 ms avec l'index couvrant**
   `(sender_address, thread_id)` — 59× le témoin — parce que la
   sous-requête matérialise les fils des 52 000 messages routés à chaque
   page. Le patron pins ne se transpose pas : `pins` compte quelques
   lignes, le routage en couvre des dizaines de milliers.
3. **Filtre de destination (K1) : 0,087 ms.** Le bloom filter sur les
   25 adresses du Kiosque suffit ; aucun index supplémentaire requis
   pour la page.
4. **`category_totals` (T1) : +18 ms sur un témoin déjà à 49 ms** — la
   sonde PK se paie sur les 200 000 lignes du COUNT. Pas une régression
   de la page chaude, mais le témoin lui-même n'est pas gratuit ; si le
   surcoût gêne, la voie est un compteur entretenu, pas une autre forme
   de la requête.

## Conditions qui invalideraient ces chiffres

- **Part routée bien plus grande** (tout le courrier en newsletters) :
  U1–U3 et K1 ne bougent pas (coût par ligne rendue), U4 empire, T1/T2
  croissent avec la boîte, pas avec la part routée.
- **Destination RARE dans une grosse boîte** : K1 marche l'index de
  dates jusqu'à trouver 50 correspondances — si le Kiosque ne reçoit
  presque rien, la page peut parcourir toute la boîte (~T2, ~60 ms).
  Non mesuré finement ici.
- **Cache froid** (OneDrive, 11 Go au terrain) : tout est mesuré chaud ;
  la leçon STANDARD §9 s'applique avant toute généralisation au terrain.
- Moteur 3.51.2 (node) vs rusqlite bundled : plans identiques attendus,
  à re-prouver par la garde de plan en prod.

## Coût d'industrialisation estimé

- Table `routage_expediteurs` + migration : petite (patron
  `images_expediteurs`, store.rs:319).
- Prédicat d'exclusion : une constante partagée façon `CORPS_ABSENT`
  (alias `e` exigé), injectée dans `unified_page_sql`,
  `category_page`, `category_totals` + **garde de plan** (patron
  `la_boite_unifiee_ne_materialise_pas_son_tri`, store.rs:6135) qui
  prouve la sonde PK et l'absence de scan d'`envelopes`.
- Aucun index nouveau requis pour U1–U3/K1. Si l'exclusion par fils
  était retenue malgré U4, il faudrait l'index couvrant ET une autre
  stratégie (matérialisation entretenue), pas la sous-requête.
- Le point dur n'est pas le SQL : c'est la sémantique de page courte
  (verdict 1) — décision de conception, pas de plan.

---

## S2-bis (E2, 2026-08-30) — la RÉTENTION du Portier dans le chemin chaud

La question : « expéditeur en attente » = SANS ligne de routage ET
premier message POSTÉRIEUR à l'époque (D3 arrivées seules). Où se paie
ce prédicat ? Scripts : `bench-portier.mjs` (V1/V2), `diag-v3b.mjs`
(V3), `diag-v4.mjs` (V4, verdict), `diag-pv.mjs` (page du Portier).
Décor : 200 k, 2 000 adresses zipf, 300 nouveaux post-époque (dont 20
décidés), 50 anciens routés. Sémantique bancée : un fil quitte la
Réception s'il porte un message d'un expéditeur routé AILLEURS (miroir
de `fil_route_sql`) ; il n'est RETENU au Portier que si TOUS ses
messages viennent d'inconnus en attente (un fil mêlé RESTE — règle
d'or, jamais perdre de courrier).

| Variante | page off. 0 | page off. 100 k | count | verdict |
|---|---|---|---|---|
| U0 témoin (existant) | 0,236 ms | 6,49 ms | 10,6 ms | — |
| V1 sondes corrélées à la requête | 0,331 ms | **299 ms** | 310 ms | ÉCARTÉ |
| V2 idem, attente matérialisée | 0,311 ms | **145 ms** | 158 ms | ÉCARTÉ |
| V3 listes NOT IN (patron pins) | — | 75-112 ms/req | idem | ÉCARTÉ |
| **V4 drapeau `threads` + index partiel** | **0,212 ms** | **4,24 ms** | **4,15 ms** | **RETENU** |

- **V1/V2 s'effondrent en profondeur** : la sonde par fil se paie sur
  CHAQUE rangée sautée par l'OFFSET — la réserve d'industrialisation
  de S2 (« pages courtes ») devenue réelle sur le squelette paginé.
- **Piège SQLite payé** : l'index d'EXPRESSION
  `(lower(trim(sender_address)), date_epoch)` n'est employé que
  contre un littéral/paramètre — dans une JOINTURE (`= r.address`),
  SCAN complet (2,3 s la liste V3). Contre-mesure prouvée : colonne
  générée `sender_norm` VIRTUAL (ALTER 14 ms) + index réel (188 ms à
  200 k) — la jointure redevient SEARCH.
- **V3 (listes matérialisées par requête)** : 52 583 messages routés
  au décor (les newsletters dominent) → 112 ms à CHAQUE requête. La
  liste n'est pas « minuscule par construction » comme les pins.
- **V4 = le patron déjà RETENU à S1 pour les groupes** : drapeau
  `threads.organise_hors` maintenu en transaction (comme
  `size`/`unseen`), index partiel MIROIR de `idx_threads_date_globale`
  (`WHERE inbox_size > 0 AND organise_hors = 0`, 40 ms). Offset stable
  par construction, et MOINS de rangées que le témoin (4,2 < 6,5 ms).
  Entretien mesuré : recompute d'UN fil 26 µs ; décision sur le plus
  gros expéditeur du décor (10 000 fils) 63 ms — geste unique ;
  rattrapage complet des drapeaux 370 ms (activation/migration).
- **`portier_attente(address PK, premiere_epoch)` matérialisée** :
  décision d'arrivée (3 sondes : routage PK, attente PK, « connu avant
  l'époque » par index) **7,4 µs/message** ; rattrapage complet 21 ms.
- **Page du Portier** (attente → dernier message + compte par
  expéditeur, via `sender_norm`) : **0,32 ms** ; pastille de nav
  (messages en attente) : **0,26 ms**.

Coût d'industrialisation V4 : colonne générée + index `sender_norm`
(~190 ms de migration à 200 k, une fois), table `portier_attente`,
colonne `threads.organise_hors` + index partiel, entretien aux trois
points d'écriture (arrivée, décision de routage, réintégration) +
rattrapage à l'activation. Garde de plan à écrire : la page organisée
emploie l'index partiel, jamais un scan.
