# État — l'instantané de relève de Wind

> **Ce document est réécrit à chaque chantier — c'est sa fonction.**
> Version livrée, prochain chantier, chiffres du terrain, arbitrages
> ouverts, reports : tout le volatile vit ici. La méthode, les
> invariants et les pièges vivent dans [STANDARD.md](STANDARD.md) —
> eux ne se réécrivent pas, ils s'amendent.
>
> Extrait de PASSATION.md (§1 + §8) le 2026-08-19 — PLAN-DOCUMENTATION.

---

## Où on en est, et quoi faire en premier

🔨 **CHANTIER EN COURS : [PLAN-MODE-ORGANISE](PLAN-MODE-ORGANISE.md)**
(ouvert le 2026-08-29 — le second mode de tri inspiré de HEY, D1-D9
tranchées au STOP 1, dont **D3 REJET de la présomption annuaire :
tout le monde au Portier, arrivées seules**). **E1 (le socle) est
LIVRÉE et validée terrain le jour même** : va-et-vient « Organisé »
(prefs SQLite, époque de première activation gravée), nav organisée
Kiosque/Registre, routage local par expéditeur (**ADR 0028**, table
`routage_expediteurs`, vocabulaire fermé), « Déplacer vers… » à la
barre du fil (l'adresse résolue au cœur — jamais soi), 5 glyphes (85
au jeu), A96. Spikes S1-S3 **mesurés** (verdicts §5bis du plan :
sections et groupes AU SERVICE avec index partiels/agrégat, exclusion
par message gratuite, préchargement Kiosque dans le budget). Revue 8/8
corrigées (deux « perte de courrier » prouvées RED). e2e 153 → **157**.

**E2 (le Portier) est LIVRÉE le 2026-08-30** (commit `fe33b51`, CI
verte run 33302233626, STOP visuel CE GO et **terrain CE zéro
constat le jour même**, mesure S4 engagée une semaine) : rétention
des inconnus
(D3 « arrivées seules » — `portier_attente` matérialisée à l'arrivée,
drapeau `threads.organise_hors` entretenu par `thread::refresh` +
index partiel miroir, verdict S2-bis : toute forme calculée à la
requête s'effondre à l'offset profond, le drapeau vaut MIEUX que le
témoin 4,2 vs 6,5 ms ; colonne générée `envelopes.sender_norm` —
piège gravé : un index d'EXPRESSION SQLite ne sert jamais une
jointure), exclusion partagée flot + totaux + épingles + pastilles de
nav, page du guichet à la forme du prototype (STOP visuel CE GO),
Oui/Non + minis ⋯, historique, réintégration aux mêmes règles que
l'arrivée, glyphe `more_horiz` (86), Système A97. Revue à regard neuf
**10/10 corrigées** — dont la règle d'or prouvée RED (un fil mêlé à
un écarté RESTE — `ecarte` n'a pas de vue), le message SANS Date qui
contournait le guichet, et le rattrapage E1→E2 à la migration. Tests
mail-core 387 → **401**, e2e 157 → **161** (rétention prouvée en la
cassant). Dette neuve **D-46** (l'anatomie de rangée du Portier est
une copie main de celle de la Liste).

**E3 (les règles du Non) est LIVRÉE le 2026-08-30** (commit
`59a8378`, CI verte run 33304306395, **terrain CE zéro constat le
jour même** — exécution serveur vérifiée au webmail ; Système A98) : à
l'arrivée d'un message d'un écarté avec règle, l'action est
journalisée **dans la transaction du lot** (revue E3 — l'exécution
après commit laissait une fenêtre de crash où la règle se perdait
pour toujours) via `pending_actions`, rejouée en tête de chaque
synchro, retrait local sans écho ; `corbeille` → la corbeille
serveur, jamais définitive (D4) ; garde anti-doublon de la file (la
re-livraison après un rejeu en échec aurait coincé toutes les
actions derrière une seconde action identique) ; « ses prochains
messages » seulement — un backfill ne touche jamais l'historique ;
les règles S'ÉTEIGNENT avec le mode (D2). Limites dites au PLAN
(Date falsifiée, spam sans dossier résolu, inventaire CONDSTORE
pendant un rejeu en attente). Tests mail-core 401 → **406**, e2e
161 → **162** (règle prouvée en la cassant, témoin de synchro au
filet).

**E4 et E5 sont LIVRÉES le 2026-08-30** (D7 AMENDÉE par le CE : la
première MINEUR porte E1-E5, E6 suivra). **E4 — la Réception
organisée** : sections « Nouveau pour vous · n » / « Déjà consulté »
(UN flot ordonné non-lus d'abord, index partiels d'expression global
+ par compte, gardes de plan sur les trois chemins, banc 200 k
0,03/1,6 ms), colonne centrée 760 px sans volet, clic → écran 03, ⋯
de gestes par rangée. **E5 — Mis de côté** : table `mis_de_cote`
(patron pins, purges), exclusion partagée par UNE écriture
(`exclusion_organisee()`), pile + éventail + tableau, bascule barre
du fil (état SEMÉ, patron épingle) et ⋯. STOP visuels CE GO (un
constat E4 : l'air des titres de section, bande 34 → 52 px, corrigé).
Revue à regard neuf **10 trouvailles / 9 corrigées** (triage clavier
qui marquait lue une conversation jamais montrée, pastille comptant
les mis de côté, couture d'une autre source, testid vacant…), un
refus motivé (« Terminé » sur un expéditeur écarté suit le verdict).
Système **A99** ; dette **D-47** (menus ×3, jumeaux pins/pile).
Tests mail-core **410**. ⚠️ **Périmètre à trancher CE : le « Kiosque
en cartes déjà ouvertes » du prototype n'est assigné à AUCUNE étape**
(S3 l'a mesuré, personne ne l'a bâti — le Kiosque livré est une
liste). **Reste : terrain E4+E5, puis LA RELEASE MINEUR E1-E5** —
l'entrée CHANGELOG s'écrira à ce moment-là (§2.9, `gh release list`
d'abord), avec Innamoramento qui attend aussi cette release. Puis E6.

Le même jour, deux retours CE réglés : **« Mona » renommée
« Innamoramento »** (A95, ids migrés, commit `0a25105`, CI verte) et
**le bloquant bêta levé** (`feedback-wind@fcts.io` reçoit — délai de
propagation DNS ; la première vague D9 est ouverte, action CE au
PLAN-BETA).

Dernier soldé :
**[PLAN-MONA](PLAN-MONA.md)** (2026-08-29, ouvert et clos le même
jour, commit `409c8ae`, CI verte run 33270609284, terrain CE « Terrain
OK sur les deux thèmes, GO » — zéro constat). **Deux thèmes neufs,
« Mona » et « Mona · nuit »** — **renommés « Innamoramento » /
« Innamoramento · nuit » le 2026-08-29 (A95, retour CE, avant toute
release ; ids migrés, migration prouvée au filet)** — (couleurs CE :
accent `#AD204C` —
6,80:1 sur blanc, accent ET marque du clair tel quel — et la teinte
de tuile `#A0868F` déclinée par polarité `#EFDFE4`/`#2C2126` : la
valeur brute est impossible aux seuils, 2,04:1 sous ink2, 1,88:1 sous
le pire repère, 3,33:1 au mieux en nuit). **V7 est AMENDÉE (A94, ADR
0027)** : la table des thèmes est courte et VIVANTE — ajouts/retraits
possibles, jamais le retour des 28 Wada. Mécanique : la table claire
des repères servie par `[data-theme$="-nuit"]` (zéro hex recopié),
son parseur centralisé dans `jetons.mjs` (`lireReperes` — les deux
gates portaient chacune leur copie de la regex), la garde de
migration de `theme.js` dérive de `THEMES` (liste en dur retirée —
prouvée RED : sans elle un choix `mona-nuit` persisté était réécrit à
chaque démarrage), `NOMBRE_ATTENDU` 2 → 4. Contraste **440 paires
(220 → 440), 0 échec** ; cohérence 4 thèmes / 68 jetons ; e2e
**153/153** (deux specs étendues, comptes 2 → 4, migration prouvée en
la cassant). Revue à regard neuf 6 angles / 10 retenues /
**9 corrigées** (dont le chiffre gravé faux 166 → 220 et la fuite de
thème du test de migration) ; dette neuve **D-45** (vignettes du
Système = seule copie d'hex hors gate). **Innamoramento part avec la
prochaine release** — l'entrée CHANGELOG s'écrira à ce moment-là,
sous ce nom (§2.9). **Le
bloquant bêta est LEVÉ le 2026-08-29** : `feedback-wind@fcts.io`
reçoit — les mails du 28 sont arrivés, c'était un délai de
propagation DNS, pas une panne d'alias. **La première vague bêta
(5-10 proches, D9) est ouverte** — action CE au PLAN-BETA.

---

Le chantier soldé précédent :
**[PLAN-RETOURS-12](PLAN-RETOURS-12.md)** (2026-08-28 → 29, commits
`60225b0`/`331832d`, CI verte run 33216010954, terrain CE **4/4 le
2026-08-29, zéro constat**). Cinq retours : (1) **un compte ajouté
Wind ouvert se dit connecté** — `compteAjoute()` rappelle
`connecter()` (le tableau `connectes` n'était rempli qu'au démarrage),
la nav se recharge AVANT le réseau ; couture e2e `__e2eAjout` ;
(2) **la taille du package est PLATE, fait mesuré sur 12 releases**
(arm64 5,04 → 5,66 Mo, une seule marche +0,44 Mo à la 0.6.0 bi-arch ;
x64 ±1 %) — le bandeau de MAJ plus long vient du chemin honnête de la
0.10.2, pas des octets ; chemin instrumenté (durées manifeste /
téléchargement / écriture / spawn), **traces visibles SEULEMENT via
`lancer-wind.ps1`** (app fenêtrée sans stderr) ; (3) **marque
d'entête 28 px** (A93 — au passage, la fiche V11 portait « Wind
15 px », faux : 18 px réels, corrigée) ; (4) **les versions du
workspace Cargo suivent la version produit** (0.1.0 gelé depuis
l'origine → 0.12.0 ; `faire-release.ps1` bumpe désormais les deux,
validations avant toute écriture) ; (5) **l'entête du message en deux
lignes** (A92) : « Nom <adresse> sur Boîte » (règle D7 conservée) puis
« À : Nom <adresse>, … » et « Cc : … » si présents — les noms des
destinataires viennent de l'**annuaire des correspondants**
(commande `noms_adresses`, lookup PK borné aux À/Cc du fil, ~0,2 ms ;
cache `cacheNoms` survivant à la bascule de cadre). Revue à regard
neuf 8 angles / 10 retenues / **8 corrigées** ; dettes neuves
**D-43** (écho sans Cc) et **D-44** (`connectes` sans cycle de
rafraîchissement — le symptôme miroir de R1). e2e 150 → **153**.
**Piège d'outillage payé et gravé** : le gabarit de seed e2e périme à
MINUIT même « frais » au TTL (deux specs rouges au pre-push de 00 h ;
`launch.mjs` exige désormais le même jour calendaire). **LIVRÉ en
0.13.0, PUBLIÉE le 2026-08-29** (commit `9599b31`, CI verte run
33217432151, tag nu, Latest, **vérifiée 18/18** par
`verifier-release.ps1` — exe arm64 200 / 5 667 616 o, x64
200 / 6 405 481 o, signatures distinctes — et **auto-update 0.12.0 →
0.13.0 prouvé aux DEUX postes le jour même**, GO CE : « Autoupdate OK
sur les deux postes »). Première release où `faire-release.ps1` a
bumpé AUSSI le workspace Cargo (0.13.0 partout — E4 prouvé en
condition réelle). Un flaky consigné à la gate pre-push de release
(refonte-volets:86, scrim de composition qui intercepte le clic —
retry vert, 152 passed). NB : les traces `maj :` n'ont pas été
captées à cette MAJ (postes lancés normalement) — la mesure du
bandeau attend une MAJ acceptée depuis un Wind lancé par
`lancer-wind.ps1`. **Le prochain sujet reste la première vague bêta**
(PLAN-BETA — bloquant CE : faire recevoir `feedback-wind@fcts.io` ;
puis inviter 5-10 proches, D9).

**Audit de la suite e2e (2026-08-29, commit `84651bb`, CI verte run
33217676308)** : les 20 specs (153 tests) confrontés statiquement au
source `apps/desktop/ui-v2/src` — **0 obsolète, 0 vacant, 0 doublon** ;
chaque sélecteur, testid et texte de catalogue retrouvé. Seul défaut :
le titre et l'entête de `refonte-retours-8.spec.js` disaient « quatre
étapes » quand le corps prouve les 5 (bêta 4/5, A91) — deux chaînes
corrigées (GO CE), zéro comportement touché. ~10 tests FRAGILES
recensés (assertions sur `__e2eJournal`, `outbox_status`,
`getComputedStyle`, focus, calage 2 px au pixel) — compromis assumés
et documentés dans les specs, laissés en l'état ; à surveiller au
prochain refactor CSS/IPC. Refus de périmètre §2.6 : rien de sain n'a
été réécrit.

---

Le chantier soldé précédent :
**[PLAN-RETOURS-11](PLAN-RETOURS-11.md)** (2026-08-27 → 28, commits
`a562fdd`/`a9f93e0`, CI verte runs 33127472066/33127940550, **LIVRÉ
en 0.12.0 PUBLIÉE le 2026-08-28, vérifiée 18/18 et auto-update prouvé
aux DEUX postes le jour même** — GO CE : « Release ok, auto update ok
sur les deux postes »). Trois retours : (1) **la garde d'images
a une mémoire** — « Afficher les images » persiste par MESSAGE (clé
d'enveloppe, patron `pins` ; renverse l'invariant A43, décision D1) et
« Toujours afficher les images de cet expéditeur » pose une règle
globale au poste (adresse exacte normalisée, autorité au CŒUR dans
`message_body`), révocable aux Réglages > Affichage (D4) — A89 ;
(2) **« Made in EU »** + drapeau UE (SVG figé hors thèmes, hors
registre) dans À propos — A90 ; (3) **la bêta est lancée** :
[PLAN-BETA.md](PLAN-BETA.md) (actions datées), [BETA.md](BETA.md)
(guide testeur), et — constats terrain du 28, corrigés le jour même —
le **bouton Feedback** à l'entête (glyphe neuf, envoi par `queue_send`
vers feedback-wind@fcts.io, `flush_outbox` immédiat) et l'**étape
d'accueil 4/5 « Wind est en bêta »** (A91, textes CE mot pour mot).
⚠️ L'adresse des retours **ne reçoit pas encore** (alias fcts.io,
prouvé hors Wind — action CE bloquante avant toute invitation, au
PLAN-BETA). Revue à regard neuf 8 angles / 10 retenues / 9 corrigées
(dont la purge `reset_mailbox` de la mémoire d'images — un UID recyclé
aurait hérité d'un consentement, TDD) ; dette neuve **D-42**
(révocation par message absente). e2e 148 → **150**, 4 gates
complètes (2,2-2,6 min). Plus rien n'est dû sur cette version. **Le
prochain sujet : la première vague bêta** (PLAN-BETA — bloquant CE :
faire recevoir `feedback-wind@fcts.io`, prouvé en panne le 28 depuis
tout client ; puis inviter 5-10 proches, D9).

---

Le chantier soldé précédent :
**[PLAN-RETOURS-10](PLAN-RETOURS-10.md)** (2026-08-27, ouvert et clos
le même jour, commit `a72f341`, CI verte run 33111561147) — quatre
retours CE : **sélection multiple** (Ctrl-clic qui coche ET déplace le
focus de lecture, Shift-clic depuis l'ancre ou la sélection, case au
survol dans une gouttière de 8 px avec contenu écarté à 34 px, barre
de la liste transformée — lu/non-lu/archiver/indésirable/supprimer —,
raccourcis e/Suppr sur le lot, et **le fil part ENTIER** — D6, tranché
devant l'exemple Vantis), **icône Windows** remise à la marque Elements
(elle portait la « W-pastille » d'avant l'adoption du 2026-08-24 ;
`faire-icone.ps1` réécrit), **marque d'entête 24 px** (D2), **calage
optique des glyphes de la nav** (planche de trois variantes, verdict
C — baseline + 2 px, D7). Terrain validé en DEUX passes le jour même
(8 constats de première passe, tous corrigés dans la session) ; revue
à regard neuf 8 angles, 10 trouvailles, 9 corrigées ; e2e 137 →
**148** ; glyphe `check` neuf (79) ; A86-A88. Dette neuve : **D-41**
(coche clavier). **La 0.11.0 est PUBLIÉE le 2026-08-27** (commit
`d0f9c8c`, CI verte run 33113349707), **vérifiée §2.10 le jour même :
18/18 PASS** (Latest au tag nu, 5 assets, `latest.json` sans BOM
1 590 o aux DEUX clés de plateforme, signatures == `.sig` et
distinctes, exe arm64 200 / 5 630 211 o, exe x64 200 / 6 354 494 o) et
**prouvée au terrain : auto-update 0.10.2 → 0.11.0 confirmé sur les
DEUX postes** (GO CE : « release ok, auto update ok sur les 2
postes »). Plus rien n'est dû sur cette version. NB : aucun refus SAC
n'a été signalé à cette MAJ — la preuve du filet d'échec de MAJ **en
condition de refus** (PLAN-SIGNATURE) reste due, à la prochaine
occasion où SAC refusera réellement.

La **0.10.2 est PUBLIÉE le 2026-08-27**, vérifiée §2.10 (tout passe,
2 canaux), **auto-update prouvé au terrain sur les DEUX postes** (GO
CE : « release ok auto update ok sur les 2 postes »). Ensuite : la
**bêta fermée** — avec, en travers de sa route, la dette **D-39**
(signature Authenticode gelée : sur tout poste Smart App Control,
l'installation d'un exe non signé est une loterie par binaire ET par
jour — prouvé les 26-27/08) et **D-40** (issue amont
tauri-plugin-updater, GO CE en attente).

---

✅ **[PLAN-SIGNATURE](PLAN-SIGNATURE.md)** (2026-08-26 → 27, SOLDÉ) :
un échec d'installation de mise à jour **se voit** désormais (bandeau
qui se réarme, Réglages sans cul-de-sac, timeout 10 min, version
annoncée = posée, témoin en répertoire neuf, garde e2e, crate épinglé
`=2.10.1`) au lieu de fermer l'application en silence. La signature
Authenticode attend (D2) : validation individuelle Trusted Signing
fermée hors USA/Canada. Preuve du filet en condition de refus : à la
première MAJ depuis la 0.10.2 sur poste SAC.

Le constat d'origine : « Installer » **fermait Wind sans rien
installer** — Smart App Control (`On`) refusait l'exe non signé et le
plugin updater sortait par `exit(0)` sans lire le retour de
`ShellExecuteW` (spike `spikes/maj-x64/`). Détail complet au plan
(constat, E1-E5, décisions D1-D5) et au journal **A85**.

---

✅ **[PLAN-DEMARRAGE](PLAN-DEMARRAGE.md)** (2026-08-26, SOLDÉ le 27) :
livré, terrain 6/6, commits `b94d63b`/`385ee64`, CI verte, **0.10.1
publiée** (18/18) ; la preuve auto-update x64 (D5) est tombée le 27
(0.10.0 → 0.10.1 appliqué, après un premier refus SAC).

Le constat du CE — « freezes et lenteurs au démarrage, une fois la
fenêtre ouverte » — était **un gel de SERVICE, pas de fenêtre** :
`backfill_status` partait à t + 3 s, tenait le **verrou global des
commandes 8 870 ms**, et pendant ce temps aucune commande applicative
n'était servie. Mesuré au terrain, premier lancement **après
redémarrage machine** (la première mesure honnêtement froide du
projet) :

| | avant | après |
|---|---|---|
| `backfill_status`, verrou tenu | **8 867,8 ms** | **124,9 ms** (×71) |
| fenêtre → liste complète | **1 157,3 ms** | **384,6 ms** |
| la part hors tranche WebView2 | 406,4 ms | **119,0 ms** (×3,4) |

**Trois correctifs, tous d'une ligne ou presque, tous mesurés avant
d'être écrits :** le critère `AND b.scanned = 1` quitte les requêtes du
rattrapage (il forçait 251 k rappels de ligne grasse dans 11,4 Go pour
protéger **zéro** ligne — mesuré sur les deux postes) ; `idx_envelopes_date`
gagne `uid` (sans lui SQLite allait chercher la ligne d'enveloppe pour
lire l'uid du sondage — `pending_total` 521,9 → 107,9 ms) ; et un
`await tick()` fait partir la première page de la liste **avant** les
sondes, où elle était douzième.

**Quatre hypothèses ont été retournées par la mesure ou la
contre-expertise, et aucune n'a été écrite avant d'être éprouvée** — le
correctif du dossier d'instruction (un index nu que SQLite n'aurait
jamais choisi : il fallait `INDEXED BY` ou UNIQUE), la contention, les
64 allers-retours, et le E2 du plan (différer les sondes aurait
**fabriqué** un repeint de chaque rangée du premier écran). Économie :
un index inutile à 18 s de migration, un regroupement de requêtes sans
effet, et un défaut visible chez le CE.

**Décisions CE : D1-D9** (§5 du plan), dont D1 « palier liste peinte »,
D8 « retirer le critère `scanned` » et D9 « assumer les 1,77 s de
reconstruction d'index au premier lancement après mise à jour, sans
écran » — inscrit au STANDARD §3.

**Dette rouverte puis FERMÉE : D-8.** Dettes neuves : **D-36**
(colonne fantôme de `echos`), **D-37** (`sync_progress`), **D-38** (le
rattrapage des aperçus recharge la liste pour rien).

**Deux défauts d'outillage payés et corrigés en route** :
`depouiller.py` mourait hors PowerShell 7 sur sa propre flèche, et le
banc écrivait `$n` dans sa boucle bornée par `$N` — **en PowerShell
c'est la même variable** ; un `-N 3` partait pour ~550 tours. C'est
aussi ce qui explique les « 19 lancements » de la campagne du 26/08.

**Reste :** le terrain (STOP 2), puis commit, push et CI, puis
`/solde`.

**Dernier chantier soldé : [PLAN-ESPACEMENT](PLAN-ESPACEMENT.md)**
(2026-08-25, terrain CE **7/7 zéro constat**, gate verte 2 min, e2e
129 → **137**) — **trois crans d'air entre les messages** (A83) :
« Faible » (l'existant au pixel près, padding 13 px, rangée 88),
« Moyen » (19, 100), « Élevé » (25, 112), aux Réglages > Affichage
(sélecteur natif, patron d'A26). Le cran se pose en **jeton
`--rangee-pad`** sur le cadre de la liste (patron de `--l-nav`) : toutes
les rangées le prennent d'un coup, sondes comprises. **L'air vit dans le
padding et nulle part ailleurs** — une marge ou un `row-gap` donneraient
12,375 px par rangée invisibles à `offsetHeight`, donc au fenêtrage.
**Les sondes de hauteur deviennent permanentes**, dans une **cage
positionnée** : `sondees`/`sonder()` sont morts, `bind:offsetHeight` les
remplace. Mesuré au banc (`spikes/espacement/`, msedge = WebView2, 4
variantes × 5 hauteurs) : sans le `position:relative` de la cage, les
sondes ajoutent jusqu'à **85 px de défilement fantôme** ; avec, **zéro**
à toutes les hauteurs. Défaut préexistant corrigé au passage (décision
D3) : `visibles` lisait `clientHeight`, qui n'est pas un signal —
agrandir la fenêtre laissait une bande vide. Revue à regard neuf : 7
angles, **29 trouvailles retenues, toutes corrigées** — dont l'ordre des
effets (le ré-ancrage lisait une position déjà réécrite par l'effet des
épinglées : **44 rangées de dérive mesurées**) et le piège `in` sur la
chaîne de prototypes. **Enseignement payé** : trois des cinq tests du
premier filet ne pouvaient pas échouer ; le filet réécrit (8 tests) lit
ce que l'utilisateur VOIT et a été **prouvé non-vacant** en cassant
volontairement le code. **Livré en 0.10.0** (publiée le 2026-08-25).

**Le chantier soldé précédent : [PLAN-REPERE-LIGNE](PLAN-REPERE-LIGNE.md)**
(2026-08-25, terrain CE **15/15**, gate verte, e2e 124 → **129**) — **la
boîte se dit en toutes lettres, sur la ligne de l'expéditeur**
(A80-A82). Le badge de repère sous l'avatar est remplacé par un **bloc
de texte** dans la ligne d'entête — `sur` à l'encre atténuée, le glyphe
du repère **en tracé nu** à la teinte du compte, le libellé (nom
personnalisé A78, sinon l'adresse) — motif du CE : *la phrase se lit,
elle évite d'avoir à se souvenir en permanence d'une couleur ou d'un
logo*. Trois règles de troncature mesurées (l'heure ne cède jamais, le
bloc cède trois fois plus vite que l'expéditeur, plafond au **tiers** —
plateau 33-36 %, mesuré sur 22 dessins et 5 planches jetables). **La
tuile aux initiales quitte la LISTE** (A81) — elle survit au fil et au
dossier Brouillons, où elle travaille. **La pastille de repère quitte la
nav** pour un tracé de 16 px (A82) : mesuré sur la fenêtre entière, le
disque est désormais **la seule forme ronde de l'écran 02**, ce que V4
visait sans l'atteindre ; la pastille survit aux Réglages, comme
pastille de *choix*. Les 24 hex du nuancier servant deux fois
(background et color), ils passent en **jetons `--rep-*`** — la gate de
contraste a été amendée avec, et la gate de cohérence contrôle
désormais les DEUX tables plus les jetons des deux polarités (prouvée
rouge sur les trois pannes qu'elle vise). **Aucune paire de contraste
neuve**, aucun glyphe neuf, les deux gabarits d'A44 mesurés inchangés.
Revue à regard neuf : 8 angles, 40 candidats, **14 défauts distincts —
tous corrigés** (dont un bloc qui pouvait se peindre par-dessus l'heure,
et une règle D7 qui donnait un refrain aux postes à un seul compte).
Constat terrain unique (point 12 : le volet parlait quand la liste se
taisait) corrigé le jour même. **Livré en 0.10.0** (publiée le 2026-08-25).

**Le chantier soldé précédent : [PLAN-ELEMENTS](PLAN-ELEMENTS.md)**
(2026-08-24, `fb32238` → `0de3689`, terrain CE **8/8 zéro KO** le jour
même, CI verte run 32752449754) — le Système « Elements » est devenu
LE Système de référence
(`docs/design/systeme.dc.html`, ADR 0026, journal A79, l'ancien archivé
en `docs/archives/systeme.v1.dc.html`) et l'UI le livre entière : cinq
étapes commises gate-vertes le 2026-08-24 (E1 socle 2 thèmes et
`--panel` mort, E2 les 78 glyphes en SVG et la fonte Material morte,
E3 zéro rayon / tuile d'initiales / disque de non-lu, E4 la marque
Elements et la mort du trait hitofude, E5 le registre 340), quatre
STOP visuels CE validés le jour même, revue à regard neuf passée
(6 angles, 10 trouvailles — 7 corrigées), 124/124 e2e, la réserve
Fluent de V14 levée à la fenêtre réelle. **Livré en 0.9.0, PUBLIÉE le
2026-08-24** (tag `0.9.0` sur `f135791`, Latest) — release **vérifiée
18/18** et **toutes preuves terrain faites** le 2026-08-25, détail plus
bas. Dette neuve : **D-35** (palier 16 des icônes — les maîtres
réduits sont livrés, décision D4). Banc du 2026-08-24 (256 k) : page
p50 85,8 / p95 180,9 ms (P1 : p95 307,6), thème 0,3 ms, RAM 8,1 Mo.

⚠️ **Piège payé le 2026-08-25 — ce document a menti une journée
entière.** Il annonçait « Reste : la release 0.9.0 » alors qu'elle
était publiée depuis la veille au soir, et le CHANGELOG portait encore
« [0.9.0] - à venir » (comme « [0.8.0] - à venir », publiée l'avant-
veille). Conséquence : deux chantiers ont été écrits sous une entrée de
version DÉJÀ LIVRÉE, et il a fallu les déplacer en 0.10.0. **La règle
qui en sort : dater l'entrée du CHANGELOG au moment de la publication,
dans le même geste** — une entrée « à venir » sur une version publiée
est un mensonge qui se propage. Le contrôle qui ne coûte rien :
`gh release list` avant d'écrire une note de version.

**La 0.8.0 est publiée** (tag `0.8.0` sur `a3d04fb`, 2026-08-23) —
elle porte PLAN-RETOURS-9 (OAuth compilé, « Retirer le compte » dit,
noms de comptes). Sa preuve terrain différée est **FAITE le
2026-08-25** : un compte connecté sur le second poste depuis une
release publiée, **sans aucun `setx`** — l'**ADR 0025 est CLOS**. Elle
aura glissé de deux versions (attendue à la 0.8.0, venue après la
0.9.0) ; la décision, elle, est restée inchangée pendant ce temps.

**Sujet ÉCARTÉ le 2026-08-25 : les glyphes de repère en remplissage
plein.** Demandé par le CE le matin, instruit sur pièces, mis en
planche l'après-midi (`spikes/glyphes-pleins/`, les douze repères au
trait contre plein+trait aux trois tailles et deux polarités) —
verdict CE devant la planche : **« Le trait suffit. »** Refus de
périmètre §2.6, **aucun code de production touché**. Les faits mesurés
sont gardés au README du spike pour ne pas les remesurer : 9 glyphes
sur 12 se remplissent sans redessin, le plein seul en amaigrit trois,
et surtout il **rapproche les silhouettes** (recouvrement 0,24 → 0,47)
alors que le travail d'un repère est de distinguer douze comptes. À ne
pas re-proposer sans raison neuve.

**Ensuite : la bêta fermée 20-50 utilisateurs** ([PLAN.md](PLAN.md)
§4, dernière étape avant le gate 5) — **ENGAGÉE le 2026-08-28**
(PLAN-RETOURS-11 R3) : plan d'actions à [PLAN-BETA.md](PLAN-BETA.md),
guide testeur à [BETA.md](BETA.md), bouton Feedback dans l'app (A91).
Première vague D9 : 5-10 proches, dès que l'adresse des retours
reçoit (action CE bloquante).

**perf-lecture est éteint** (décision CE D1 du 2026-08-21) : le
symptôme (corps à la demande bridé à ~7 s au lancement, terrain du
2026-08-19) est mort au terrain depuis la 0.2.1 — les comptages ont
quitté le chemin d'affichage (A64). À rouvrir seulement si le terrain
le redit ; le WIP d'alors avait été retiré (décision CE du
2026-08-20), sa matière reste au § revue de
[PLAN-COMPOSITION-HTML](PLAN-COMPOSITION-HTML.md).

**Dernière version livrée : 0.13.0** (publiée **2026-08-29**, tag nu
sur `9599b31`, marquée Latest, **vérifiée 18/18** et **auto-update
0.12.0 → 0.13.0 prouvé aux DEUX postes le jour même**). Elle porte
PLAN-RETOURS-12 : l'entête du message en deux lignes avec les noms
des destinataires (annuaire), le compte ajouté dit connecté, le logo
28 px, les versions workspace alignées, le chemin de MAJ instrumenté.

**La version précédente, 0.12.0** (publiée **2026-08-28**, tag nu
sur `a9f93e0`, marquée Latest, **vérifiée 18/18** par
`scripts/verifier-release.ps1` et **auto-update 0.11.0 → 0.12.0
prouvé aux DEUX postes le jour même**). Elle porte PLAN-RETOURS-11 :
la mémoire de la garde d'images (par message et par expéditeur,
révocable), le bouton Feedback et l'étape d'accueil bêta, « Made in
EU » dans À propos.

**La version précédente, 0.11.0** (publiée **2026-08-27**, tag nu
sur `d0f9c8c`, marquée Latest, vérifiée 18/18 et prouvée aux deux
postes le jour même — détail plus haut). Elle porte PLAN-RETOURS-10 :
la sélection multiple (fil entier, D6), l'icône Windows Elements, la
marque d'entête 24 px, le calage nav C.

**Une version antérieure, 0.10.0** (publiée **2026-08-25 à 21:02**,
tag nu sur `f94a008`, marquée Latest). Elle porte les deux chantiers du
2026-08-25 : **PLAN-REPERE-LIGNE** (la boîte en toutes lettres sur la
ligne, A80-A82) et **PLAN-ESPACEMENT** (les trois crans d'air, A83).
**Release vérifiée par `scripts/verifier-release.ps1 0.10.0` le jour
même : 18/18 PASS** — Latest au tag nu, 5 assets nommés, `latest.json`
sans BOM 1 590 o aux DEUX clés de plateforme, URL au tag nu, signatures
== `.sig` et distinctes, exe arm64 200 / 5 632 535 o, exe x64
200 / 6 350 877 o. **Preuve terrain FAITE le jour même : auto-update
0.9.0 → 0.10.0 confirmé sur les DEUX postes** — la chaîne signée
bi-arch (ADR 0013/0023) est prouvée vivante dans les deux sens pour la
**deuxième version consécutive**. Plus rien n'est dû sur cette version.

**La version précédente, 0.9.0** (publiée 2026-08-24 à 16:59, tag
nu sur `f135791`, marquée Latest). Elle porte PLAN-ELEMENTS : la
direction « Elements » entière. **Release vérifiée par
`scripts/verifier-release.ps1 0.9.0` le 2026-08-25 : 18/18 PASS** —
Latest au tag nu, 5 assets nommés, `latest.json` sans BOM 1 581 o aux
DEUX clés de plateforme, URL au tag nu, signatures == `.sig` et
distinctes (garde anti-croisement), exe arm64 200 / 5 629 324 o, exe
x64 200 / 6 351 726 o. **TOUTES ses preuves terrain sont faites**
(2026-08-25) : **auto-update confirmé sur les DEUX canaux** — la chaîne
signée bi-arch (ADR 0013/0023) est prouvée vivante dans les deux sens,
comme à la 0.7.0 — et la **preuve OAuth du second poste SANS `setx`**,
qui **clôt l'ADR 0025**. Plus rien n'est dû sur cette version.

**La version précédente, 0.8.0** (publiée 2026-08-23, tag nu sur
`a3d04fb`, release **vérifiée** par `scripts/verifier-release.ps1
0.8.0` le 2026-08-24 : **tout passe** — Latest au tag nu, 5 assets
nommés, manifeste aux deux clés de plateforme, signatures == `.sig`
et distinctes, exe x64 200 / 6 397 182 octets). La 0.8.0 porte
PLAN-RETOURS-9 (identifiants OAuth compilés — ADR 0025, « Retirer le
compte » dit, noms de comptes).

**La version précédente, 0.7.0** (publiée 2026-08-23, tag nu sur
`68384d2`, release **vérifiée** par `scripts/verifier-release.ps1
0.7.0` le jour même, **18/18 PASS** : Latest au tag nu, 5 assets
nommés, manifeste sans BOM 1 278 o aux deux clés de plateforme,
signatures == `.sig` et distinctes, exe arm64 200 / 5 668 094 octets,
exe x64 200 / 6 390 669 octets ; **preuves terrain PAR CANAL le
2026-08-23 : auto-update 0.6.0 → 0.7.0 confirmé sur ce poste (arm64)
ET — PREMIER de l'histoire du canal — auto-update x64 confirmé sur le
second poste** : la chaîne signée bi-arch (ADR 0013/0023) est prouvée
vivante dans les DEUX sens). La 0.7.0 porte les invitations de réunion et
le « Supprimer » par message (PLAN-INVITATIONS, MINEUR — décision
D7). **Publication en DEUX temps, enseignement payé** : un premier
run de `faire-release.ps1` (nuit du 2026-08-22 au 23) a commis et
poussé le bump puis est mort avant le tag ; le run du matin a rebâti
et signé, mais échouait sur « rien à commettre » — publication
terminée à la main à l'identique du script (tag `0.7.0` ancré sur
`68384d2`, l'arbre exact des binaires), et le script est désormais
**reprenable** (le commit vide se saute, la publication continue).

**La version précédente, 0.6.0 — la PREMIÈRE release bi-arch**
(publiée 2026-08-22, `4a72a53`, CI verte run 32584117219 ; release
**vérifiée** par `scripts/verifier-release.ps1 0.6.0` le jour même,
**18/18 PASS** : Latest au tag nu, **5 assets nommés** (2 exe, 2
`.sig`, `latest.json`), manifeste sans BOM 1 581 o aux **deux clés de
plateforme**, signatures == `.sig` et **distinctes** (garde
anti-croisement), exe arm64 résout 200 / 5 504 084 octets, exe x64
200 / 6 215 897 octets ; **preuves terrain PAR CANAL le 2026-08-22 :
auto-update 0.5.0 → 0.6.0 confirmé sur ce poste (arm64) ET install
0.6.0 x64 confirmée sur le second poste** (décision D5) — la chaîne
signée ADR 0013 reste prouvée vivante, le canal x64 est NÉ prouvé à
l'install ; son premier auto-update ne sera constatable qu'à la
release suivante). La 0.6.0 porte les trois retours de PLAN-RETOURS-8
(MINEUR, D8), ci-dessous.

**La version précédente, 0.5.0** (publiée 2026-08-21, release
**vérifiée** STANDARD.md §2.10 le 2026-08-22 : Latest, 3 assets au tag
nu, `latest.json` sans BOM 876 o, URL au tag nu, signature == `.sig`,
exe résout 200 / 5 066 813 octets ; **auto-update 0.4.0 → 0.5.0
confirmé au terrain**). La 0.5.0 porte les quatre retours de
PLAN-RETOURS-7 (MINEUR, D6).

**Le chantier soldé précédent : [PLAN-RETOURS-9](PLAN-RETOURS-9.md)**
(2026-08-23, `19e39cf`, A77-A78 + **ADR 0025**, terrain CE **6/6** le
jour même — zéro KO —, CI verte run 32647649916, **à livrer en 0.8.0**
MINEUR, décision D5). Trois sujets : (1) **identifiants OAuth compilés
dans la release** (D1, ADR 0025) — `option_env!("WIND_RELEASE_*")`
posés par le seul `faire-release.ps1` pour la seule durée des deux
builds (`finally` — la revue a tué la release qui se serait bloquée
elle-même au pre-push), la variable d'exécution prime (dev/e2e),
test « un build dev n'embarque rien », message d'échec réécrit pour
les deux lecteurs ; preuve différée alors, **faite le 2026-08-25**
sur le second poste SANS setx — ADR 0025 clos. (2) **« Retirer le compte »** en icône + texte (D2 —
« Supprimer » refusé : rien n'est supprimé du serveur), aria WCAG
2.5.3. (3) **Nom personnalisé par compte** (D3/D4) : pref
`nom_compte.{id}` purgée au retrait via LA constante
`PREFS_PAR_COMPTE` (la liste en dur cross-crate est morte), porte =
libellé de la rangée (aucun glyphe neuf, A3), 60 caractères max
refusés jamais tronqués, surfaces : nav, badge de liste, Réglages
(Comptes ET Signature), composeur « Nom — adresse » ; le nom ne
touche JAMAIS le `From:`. Revue 8 angles : 10 trouvailles, toutes
corrigées avant le terrain. Reports : DETTE **D-34**. e2e : 121 →
**124** ; gate verte 2 min 13 s.

**Le chantier soldé précédent : [PLAN-KAIZEN-CLAUDE](PLAN-KAIZEN-CLAUDE.md)
vague 2** (2026-08-23, `ceb59c4` + `a3ed285`, terrain CE 3/3 le jour
même, CI verte run 32642956082) — les contre-mesures techniques du
kaizen, ordre D3, **qualité constante** (121/121 e2e, 0 KO terrain).
Chiffres : **gate complète 4 min 34 s → 1 min 43 s** (e2e 256 → 86 s ;
rebuild mémoïsé par empreinte + gabarits de seed copiés par spec, TTL
30 min — les seeders figent l'horloge, rouge payé et corrigé) ; **une
spec e2e 74 s → 13-30 s** ; gate en UN appel `scripts/gate.ps1` (9
étapes, fail-fast) ; `retries: 1` (flaky consigné, deux échecs =
andon) ; chemin rapide docs-only du pre-push ; `scripts/terrain.ps1` +
`scripts/lancer-wind.ps1` (l'état du poste et le lancement tracé, PS
5.1, plus de one-liners au STOP 2) ; **nextest rejeté sur le chiffre**
(le poste entier = 9,3 s). Reports : DETTE **D-32** (gate en deux
encodages), **D-33** (dist périmé tenu en JS seul, pas dans build.rs).
Reste du kaizen : vague 3 hors fenêtre, bilan PDCA le 2026-09-06 (D4).

**Le chantier soldé précédent : [PLAN-INVITATIONS](PLAN-INVITATIONS.md)**
(2026-08-23, `1c159bc`, A76 + **ADR 0024**, **terrain complet en
QUATRE passes les 2026-08-22/23** — chaque constat corrigé dans la
session —, CI verte run 32605745661, **livré en 0.7.0** — publiée le
2026-08-23, vérifiée 18/18, décision
D7). Une invitation de réunion reçue se TRAITE dans Wind — périmètre
tenu : une fonctionnalité email, PAS un calendrier (refusés : vue
calendrier, CalDAV/Graph, création d'évènements, expansion RRULE
au-delà de « Se répète », COUNTER/délégation). (1) **Crate pure
`mail-ical`** sur calcard 0.3.11 (`default-features = false`, spikes
chiffrés — ADR 0024) : REQUEST/CANCEL/REPLY, TZID IANA ET Windows
(« Romance Standard Time »), garde D1 **par extrémité** — TZID
inconnu ⇒ heure flottante DITE, jamais une conversion mensongère ;
répondant d'un REPLY = premier ATTENDEE hors organisateur (écho
Exchange). (2) **Carte au fil** : elle voyage avec le corps
(`BodyView.invitation` — zéro aller-retour à l'ouverture), titre,
horaire local, organisateur, lieu, statut, trois gestes NEUTRES avec
icônes (D4 — « Accepter en accent » REJETÉE) ; **annulation croisée
dans les DEUX ordres** (CANCEL avant ou après son REQUEST) ; une
invitation transférée EST répondable (R8 terrain : être invité n'est
pas exigé) ; la partie calendrier inline disparaît des puces quand la
carte est rendue (D3), un `.ics` nommé reste enregistrable. (3)
**Réponse iTIP transactionnelle** : `enqueue_reponse_invitation` —
email et réponse dans UNE transaction, rien ne part si la ligne a
disparu ; MIME `text/calendar; method=REPLY` en alternative ;
changement d'avis autorisé (D6) ; sujet dans la langue de l'UI (D5).
(4) **La liste répond** (R10-R12 terrain) : `enrichir_lignes`, une
passe bornée à la PAGE (jamais la requête chaude — leçon
DEFILEMENT-PROFOND) ; gestes au rang de puces (fenêtrage généralisé à
N rangs au coût marginal constant), face-swap de la ligne quand la
réponse est posée (R11 : l'invitation d'origine reprend le devant —
seule exception au « dernier message du fil »), somme des pièces du
fil (R12), **optimisme instantané via `version`** (les pages du
fenêtrage sont NON réactives ; 3e/4e passes — écriture ET rollback).
(5) **Adoption de l'existant** : réparation one-shot
`pieces-calendrier` (motif `corps-fffd`) — les invitations déjà en
base gagnent leur carte, les index de pièces désalignés d'une base
héritée se réparent au passage ; `reset_mailbox`/`remove_local`
purgent `invitations`. (6) **« Supprimer » par message** (2e passe) :
la barre du fil garde archiver/spam/épingler, l'écran 03 ne retourne
à la boîte que si le fil se ferme. Glyphes 76 → **78** (`cancel`,
`question_mark`, `?v=78`, preuve **79/79**). Revue à regard neuf 8
angles : 11 trouvailles, toutes traitées avant le terrain. Reports :
DETTE **D-29** (cas C : corps vide si l'invitation est la seule
partie), **D-30** (invitation héritée sans ligne de pièce), **D-31**
(`drafts` sans `ics_reply`). e2e : 117 → **121** ; tests Rust
workspace : **547** (crate `mail-ical` neuve, 16 tests corpus sur
fixtures Google/Outlook réelles).

**Le chantier soldé précédent : [PLAN-RETOURS-8](PLAN-RETOURS-8.md)**
(2026-08-22, `cbf795a`, A74-A75 + **ADR 0023**, **terrain complet en
CINQ passes le même jour** — 16 constats R2, chacun corrigé dans la
session —, CI verte run 32576771340, **livré en 0.6.0** — la première
release bi-arch, vérifiée 18/18 et prouvée au terrain sur les DEUX
canaux, ci-dessus). (1) **Repère de compte** (icône + teinte) :
jeu DÉDIÉ de 12 glyphes (sous-ensemble 64 → 76, `?v=76`, preuve
77/77 ; A3 tenu par réservation) + nuancier mesuré **12 familles × 2
déclinaisons** (fait mesuré : aucune teinte unique ne tient 3:1 sur
les fonds clairs ET `-nuit` — bascule `[data-theme$="-nuit"]`, gate
contraste 2 716 → **3 052 paires**, fonds `tuile` compris, hex et
encres LUS du CSS expédié) ; choisi dans Réglages > Comptes (l'icône
de la rangée est la porte, un repère n'existe qu'ENTIER — écriture
transactionnelle `set_text_prefs`), remplace `person` dans la nav,
badge sous l'avatar en **boîte unifiée (D3) et en recherche**
(toujours multi-comptes), dit aux lecteurs d'écran ; les prefs
suffixées **meurent avec le compte** (`delete_account` — l'id SQLite
se réutilise, signature comprise) ; gate de cohérence : le jeu dédié,
UNE liste sur quatre porteurs. (2) **Parcours de premier démarrage**
en quatre étapes (comptes / disposition / thème / récapitulatif),
forme arrêtée au terrain en cinq passes : titre → « Étape n/4 » →
texte, **Continuer jamais grisé** (absent sans compte — D4, masqué
sous le guichet générique), captures RÉELLES de l'app à l'étape 2
(`e2e/capture-accueil.mjs`, rejouable), vignettes de thème dans la
disposition CHOISIE, récapitulatif en cartes-portes côte à côte
(texte au-dessus des miniatures, voile « Revenir à cette étape » aux
règles d'A70) ; marques `wind-accueil-fait`/`-commence` (localStorage,
V-D4) : une installation existante est réputée accueillie, un
parcours abandonné REPREND, zéro compte → guichet seul ; couture
`__e2eAccueil` dans `lib/accueil.js` (jamais dans la décision
produit). (3) **Release bi-arch** (ADR 0023) : le canal x64 REVIENT
(retiré en 0.1.3) — cross-build local prouvé (1 min 45 s, override
`lld-link` étendu au triple x64), `faire-release.ps1` à deux builds
`--target` **tout-ou-rien** (D7), `latest.json` à DEUX clés
construites par plateforme + garde anti-croisement des signatures
(la panne silencieuse encodée, jamais laissée à la vigilance),
5 assets dérivés de `$cibles`, BOM UTF-8 restauré (piège PS 5.1) ;
**`verifier-release.ps1` neuf** (§2.10 scripté ×2 plateformes,
contrôles au TAG de la version, échec en verdict — prouvé sur la
0.5.0) ; STANDARD §2.9 (MAJEUR évalué **par canal**) et §2.10 (cinq
assets nommés) amendés. Revue à regard neuf 8 angles : 10 trouvailles
confirmées, toutes corrigées avant le terrain. Reports : D-10
rouverte-refermée sans solde (ordre A41 vérifié sur pièces,
l'assertion `prefs.lang` reste à écrire). e2e : 108 → **117** ; tests
Rust : mail-core 357 → **358**, wind-desktop 18 → **20**.

**Le chantier soldé précédent : [PLAN-RETOURS-7](PLAN-RETOURS-7.md)**
(2026-08-22, `2cb9460`, A70-A73, **terrain complet en deux passes le
2026-08-21** — le constat visuel corrigé dans la session —, CI verte,
**livré en 0.5.0**). (1) **Survol descriptif des pièces jointes** :
un voile couvre la puce au survol et au focus clavier — glyphe
`download` + « Enregistrer » (D1 : le vocabulaire du produit, pas
« télécharger ») — même géométrie, la rangée ne reflue pas ; jamais
sur la puce inerte d'un écho. (2) **Pièces jointes en tête du
message**, entre l'entête et le corps (la garde d'images reste collée
au corps) ; premier e2e à verrouiller l'ordre DOM. (3) **Écran 03 à
plat** (renverse « l'écran 03 garde sa carte pleine » d'A46) : plus
de carte englobante, scène en un seul flot, colonne centrée 960 px
(D2) — la forme à plat est désormais LA forme unique du composant Fil
dans ses deux cadres. (4) **Épingler une conversation** (Réception
seule D4, barre du fil D3, en tête SEULEMENT D5) : table `pins`
locale à clé d'enveloppe (survit à la reconstruction des fils ;
JAMAIS `flagged`, écrasé par la synchro), section épinglée préposée à
la page 0 — le flot paginé ET les totaux l'excluent (exclusion
partagée, garde de plan étendue : la sous-requête part de `pins` par
`CROSS JOIN` directif, sinon SQLite sans ANALYZE scannait `envelopes`
entière sur le chemin le plus chaud, ~24 ms mesurés à 200 k) ; l'état
du bouton est SEMÉ de la ligne servie (zéro aller-retour à
l'ouverture) ; la ligne épinglée porte le dessin de la tuile de nav
(`--tuile`/`--tuileInk`, constat terrain corrigé le jour même) ; le
vide de la liste exige la réponse des DEUX sources (flot + épingles).
3 glyphes neufs (61 → 64, `?v=64`, preuve 65/65). Reports : DETTE
**D-28** (épingle orpheline si le message-clé quitte sa boîte — cas
limite assumé). e2e : 103 → **108** ; tests Rust 355 → **357**.

**Le chantier soldé précédent : [PLAN-RETOURS-6](PLAN-RETOURS-6.md)**
(2026-08-21, `13d4bed`, A66-A69, **terrain complet en trois passes le
même jour** — chaque constat corrigé dans la session —, CI verte,
**livré en 0.4.0**). (1) **Signature par compte** (Réglages >
Signature) : éditeur riche réduit (G/I/S, allowlist ammonia par LA
frontière), portée par compte (« aussi réponses/transferts », défaut
nouveaux seuls), « Appliquer à tous » copie signature ET portée
visiblement ; au composeur, insertion au gabarit (nouveau : sous deux
lignes vides ; réponse/transfert : entre amorce et citation) et la
signature **suit le compte émetteur** (gabarit de corps recomposable —
citation regarnie à l'identique) ; garde anti-churn `corpsAuto`
(fermer sans frappe ne sème RIEN). Stockage `prefs`, aucune migration.
(2) **Envoi différé** : `outbox.send_at_epoch`, filtre échu-seulement
DANS `outbox_to_send` (porte unique), `enqueue_outbox_full`
transactionnel ; annulation = brouillon ENTIER recréé (pièces avec
octets ; entrées programmées non échues seulement — course avec une
vidange concurrente verrouillée, revue) ; barre d'état « N
programmé(s) · départ {quand} », fente d'avis « Annuler l'envoi »,
départ par minuterie courte armée par la sonde 10 s (~1 s). Sémantique
locale DITE (D1) : part si Wind est ouvert, sinon au prochain
lancement. (3) **« Important »** : bouton-icône de la barre de mise en
forme (aria-pressed, infobulle « Marquer le message comme
important »), colonnes `drafts.important`/`outbox.important` (le
bascule seul avance l'horodatage), en-têtes SMTP `X-Priority: 1` +
`Importance: high` (aucun sur l'ordinaire). (4) **Entête du
composeur** sur `--panel` (le pied de page de Wind). 3 glyphes neufs
(58 → 61, `?v=61`, preuve 62/62). Report : affichage des importants
REÇUS (§ reports). e2e : 99 → **103**.

**Le chantier soldé précédent : [PLAN-RETOURS-5](PLAN-RETOURS-5.md)**
(2026-08-21, `6f94922`, A65, terrain complet — cinq points, le point 2
instruit puis rejoué —, CI verte, **livré en 0.3.0**). (1) **L'entrée
temporaire d'Envoyés dit vrai** : l'écho local d'un envoi
(PLAN-REACTIVITE E3, mécanisme intact) affichait « À : envoyes » (le
slug de destination servi comme destinataire par la tranche des échos
de nav) et un titre « Fichiers joints » vide (métadonnées jamais
rapatriées). Désormais : colonne `echos.to_addrs` remplie à la
naissance (envoi : `outbox.recipients` ; geste : `envelopes.to_addrs`),
pièces en nom + poids depuis le journal d'envoi (`echo_attachments`),
puces INERTES pendant la fenêtre (D2) ; l'écho de geste ne montre plus
de section vide (revue). (2) **Autocomplétion des adresses** (D3-D5) :
annuaire des correspondants — table dédiée, JAMAIS un parcours
d'enveloppes par frappe (leçon A64) — appris du courrier vu
(expéditeurs hors indésirables/corbeille avec leur nom, destinataires
de nos envois), rattrapé UNE fois sur l'existant à l'ouverture
(142 ms/200 k, marque `prefs`) ; suggestion « Nom + adresse », insertion
en **adresse nue** (D3, chemin d'envoi et garde anti-injection
intacts) ; 22 ms au pire cas (budget < 50 ms). (3) ETAT remis
d'équerre (perf-lecture éteint D1, report « envoi de pièces jointes »
retiré — livré depuis PLAN-PIECES-JOINTES). Reports : **D-27** (la
boîte d'envoi ne retente qu'en fin de cycle ou au geste — envois
jamais perdus, règles d'or tenues). Pièges gravés : STANDARD §9
(`2> fichier` sur l'exe fenêtré lancé nu ne capte rien — tracer via un
lanceur qui attend). e2e : 97 → **99**.

**La version précédente, 0.4.0** (publiée 2026-08-21, release
**vérifiée** §2.10 : Latest, 3 assets au tag nu, `latest.json` sans
BOM 876 o, signature == `.sig`, exe 200 / 5 055 194 octets ;
**auto-update 0.3.0 → 0.4.0 confirmé au terrain le 2026-08-21**). La
0.4.0 porte les quatre retours de PLAN-RETOURS-6 : signatures par
compte, envoi différé, marquage « important », entête du composeur.

**La version précédente, 0.3.0** (publiée 2026-08-21, release
**vérifiée** §2.10 : Latest, 3 assets au tag nu, `latest.json` sans
BOM, signature == `.sig`, exe 200 / 5 038 998 octets ; **auto-update
0.2.1 → 0.3.0 confirmé au terrain le 2026-08-21**). La 0.3.0 porte
l'**autocomplétion des adresses** À/Cc/Cci et l'**écho d'envoi qui
dit vrai** (PLAN-RETOURS-5).

**La version précédente, 0.2.1** (publiée 2026-08-20, release
**vérifiée a posteriori** STANDARD.md §2.10 : Latest, 3 assets au tag
nu, `latest.json` sans BOM, URL au tag nu, signature == `.sig`, exe
résout 200 / 5 014 053 octets ; **auto-update 0.2.0 → 0.2.1 confirmé
au terrain le 2026-08-20** — la chaîne signée ADR 0013 reste prouvée
vivante). La 0.2.1 porte le **défilement profond réparé**
(PLAN-DEFILEMENT-PROFOND, CORRECTIF §2.9) : la liste ne se fige plus
au drag de la barre, l'écran vide ne ment plus, démarrage et premiers
affichages immédiats. Enseignement gravé au passage (STANDARD §2.9 ⚠️,
oubli commis trois fois) : **les notes utilisateur au CHANGELOG
s'écrivent AVANT `faire-release.ps1`** — le script refuse sans elles.

**La version précédente, 0.2.0** (publiée 2026-08-20, **auto-update
0.1.11 → 0.2.0 confirmé au terrain le 2026-08-20** — la chaîne signée
ADR 0013 reste prouvée vivante ; release **vérifiée a posteriori**
STANDARD.md §2.10 : Latest, 3 assets au tag nu, `latest.json` sans BOM,
URL au tag nu, signature == `.sig`, exe résout 200 / 5 008 012 octets).
La 0.2.0 porte le **composeur enrichi HTML** (PLAN-COMPOSITION-HTML,
première capacité nouvelle du 0.x → MINEUR §2.9) et la **reconnexion
d'un compte au jeton mort** depuis Réglages > Comptes.

**La 0.1.11** (publiée 2026-08-19, `6977778`,
auto-update **0.1.10 → 0.1.11 confirmé au terrain le 2026-08-19** — mise à
jour depuis l'app, la chaîne signée ADR 0013 reste prouvée vivante ;
release **vérifiée a posteriori** : Latest, 3 assets, `latest.json` sans
BOM, URL au tag nu, signature == `.sig`). La 0.1.11 porte les **trois
retours** de PLAN-RETOURS-4 (R1-R3, un **CORRECTIF** — aucune capacité
nouvelle, STANDARD.md §2.9) : téléchargement d'une pièce par dialogue
« Enregistrer sous » ; nom + poids d'une pièce dans une seule puce ; corps
des messages toujours sur dalle claire (thèmes sombres redevenus lisibles).

**La 0.1.10** (2026-08-18, `a25c566`, auto-update 0.1.9 → 0.1.10 confirmé)
portait les quatre retours de PLAN-RETOURS-3 (% de rattrapage ; spam /
non-spam ; supprimer un brouillon ; réponse par message). **La publication
est pilotée de bout en bout par `scripts/faire-release.ps1`** : bump de
tauri.conf.json + build signé (clé `C:\Keys\wind.key`, mot de passe à la
main) + manifeste + — après confirmation `OUI` — commit de release, push
(gate rejouée), tag nu + Release GitHub marquée Latest, notes tirées du
CHANGELOG. Fait prouvé : `TAURI_SIGNING_PRIVATE_KEY` accepte le **chemin**
du fichier de clé (pas seulement son contenu) ; la publication n'est donc
**plus manuelle** (l'ADR 0013 la décrivait ainsi ; le script la fait,
derrière une confirmation).

**La 0.1.9** (2026-08-17) portait les quatre retours de PLAN-RETOURS-2
(Cc/Cci ; cadence de synchro Gmail 5→30 min ; trait de chargement ; retrait
« Rendre indépendante »). **La 0.1.8** (2026-08-16) portait les quatre
correctifs courrier de PLAN-RETOURS-MAIL.

**La 0.1.7** (2026-08-16) reste la ligne de la refonte entière — le
Système v2 « Wada » et son élargissement (28 thèmes, PLAN-WADA /
PLAN-WADA-ELARGI), l'UI v3 et les retours CE (A44-A47, PLAN-UI-V3 /
PLAN-RETOURS-V3), les trois modes d'affichage (PLAN-VOLETS), l'interface
v1 retirée (PLAN-RETRAIT-V1), sur une fenêtre qui ne gèle plus
(PLAN-GELS, ADR 0019) ; auto-update 0.1.6 → 0.1.7 confirmé au terrain.
Tous les plans de cette ligne sont soldés.

**Chantier soldé précédent : PLAN-DEFILEMENT-PROFOND** (2026-08-20,
`70e44e3`, A64, terrain complet — trois passes le même jour —, CI
verte, run 32382945877). Le bug terrain du drag tenu dans Archives
(blocs « .. », puis « Aucun message ici. » dans TOUS les dossiers
pendant des minutes) est mort à la racine : la liste demandait **une
page par position traversée** (~161 appels pour 2 s de barre, mesurés)
dans la file sérialisée de `hors_pompe`, et le changement de source
affichait un vide non prouvé. Désormais : **un seul vol de page à la
fois** (dernière fenêtre gagne, la page 0 d'une source neuve passe
devant la jauge), **écran vide honnête** (squelette tant que la source
n'a pas répondu), et — terrain des 2e/3e passes — **les comptages ont
quitté le chemin d'affichage** : la page ne porte plus de total (une
page courte dit la fin exacte d'elle-même ; `category_total` séparé,
demandé la pompe au repos), et `nav_snapshot` ne paie plus que les
deux non-lus que la nav AFFICHE (il recalculait toutes les 10 s huit
compteurs par compte, dont le total d'intégrale à ~240 ms la sonde —
le calcul le plus cher de l'application, jeté). Chiffres : attente
bout-en-bout p50 2 408 → **17 ms** (banc `mesure-defilement.mjs`,
versé au dépôt, décision D2) ; premier affichage d'Archives 253 →
**14 ms** de cœur (SQL, décor intégrale 200 k). Reports : **D-26**
(page profonde O(offset) assumée, décision D1 — ~129 ms à l'offset
80 000, un seul vol, écran qui dit le chargement). e2e : 94 → **97**.
Publié en **0.2.1** (CORRECTIF, §2.9).

**Chantier soldé précédent : PLAN-COMPOSITION-HTML** (2026-08-20,
`537a1e4`, A62-A63 + ADR 0022, terrain complet, CI verte — **livré
en 0.2.0**). Le composeur passe au **corps riche de bout en bout** :
colonne `body_html` à côté du texte (drafts + outbox, migration
rembobinable, NULL sur l'existant), envoi `multipart/alternative`
(texte dérivé du même HTML par LA frontière unique `frontiere_corps`),
reflet Brouillons et tirage pareil (un brouillon riche re-rapatrié
garde sa mise en forme), écho Envoyés en HTML, citation `blockquote`,
éditeur `contenteditable` + `execCommand` legacy (sortie = allowlist
ammonia exacte), barre R4 stricte (D1-D3 : sans Lien/Citation, familles
génériques + 4 crans, nuancier 12 teintes), icônes 46 → 58. **Images
distantes par geste (D5 terrain)** : réponse au pixel neutre (une
citation `AllowRemote` chargeait les pixels espions au clic Répondre —
revue), transfert conserve les images. **Deux constats terrain corrigés
le jour même (A63)** : reconnexion d'un compte au jeton mort
(`invalid_grant`) depuis Réglages > Comptes (`reconnect_account`, garde
d'identité, e2e dédié) ; l'avis de déconnexion mène aux Réglages.
Revue à regard neuf : 10 trouvailles confirmées, corrigées (dont trois
pièges du contenteditable gravés au STANDARD §9). Reports : DETTE D-25.
e2e : 92 → **94**.

**Chantier soldé précédent : PLAN-DOCUMENTATION** (2026-08-19, `78a2a91`
→ `8cf8ac3`, CI verte, terrain E4 : reprise à froid et test du stub
propres). La documentation est restructurée en trois gestes kaizen :
**la méthode vit dans [STANDARD.md](STANDARD.md)** (numérotation
§2-§10 figée, s'amende par kaizen, ne se réécrit pas), **l'état dans
ce document** (réécrit à chaque chantier), les 24 plans soldés et
5 revues de phase dans [archives/](archives/), le normatif orphelin
rapatrié au dépôt (vérification de release → STANDARD §2.10, piège du
cache chaud → §9) ; les mémoires Claude ne portent plus que des faits
machine et des pointeurs. Stub PASSATION.md temporaire (D-24 : une
reprise propre sur les deux requises comptée).

**Chantier soldé précédent : PLAN-RETOURS-4** (2026-08-18, `52aec3e`, A59-A61,
terrain complet, CI verte — **R1-R3 livrés en 0.1.11** (`6977778`, auto-update
confirmé le 2026-08-19) ; **R4 reporté en chantier dédié**, décision CE D1).
Trois retours, tous **corrections / ajustements de l'existant** (aucune
capacité nouvelle → **CORRECTIF**, STANDARD.md §2.9). (1) **Téléchargement d'une pièce
par dialogue** : le clic enregistrait en SILENCE dans Téléchargements (« il ne
se passe rien ») ; il ouvre désormais « Enregistrer sous » natif — dossier ET
nom au choix, défaut Téléchargements + nom assaini ; nouvelle commande
`chemin_enregistrement_suggere` (le nom vient de l'UI ; `safe_file_name` reste
l'autorité de désinfection), `save_attachment(dest)` écrit au chemin choisi,
capability `dialog:allow-save`, couture e2e `__e2eDestination` (A59). (2) **Nom
+ poids d'une pièce dans la MÊME puce** : la lecture s'aligne sur le composeur,
exception assumée à « 1 puce = 1 information » ; glyphe `storage` retiré de
l'usage, conservé réservé au sous-ensemble (précédent A53, A60). (3) **Corps
toujours sur dalle claire** : mesuré au terrain que seul le texte à COULEURS
d'expéditeur (infolettres pensées pour fond blanc) était noir sur sombre — le
texte sans couleur propre était déjà lisible ; le corps bake désormais
`mail_render::Palette::default` (fond blanc) quel que soit le thème, comme les
clients mûrs, **renversant la dalle sombre d'A42** ; le front ne transmet plus
de palette (`paletteLecture` retirée, params `palette` de
`message_body`/`echo_body` retirés — A61). Report : DETTE D-23. **Piège gravé
(A61)** : ne JAMAIS re-transmettre une palette de thème au corps d'un message —
le corps est volontairement clair partout (le texte d'expéditeur est composé
pour fond blanc) ; garde e2e « le corps reste sur dalle claire même sous un
thème sombre ».

**Chantier soldé précédent : PLAN-RETOURS-3** (2026-08-18, `8819090`, A55-A58,
terrain complet, CI verte — **livré en 0.1.10**, auto-update confirmé au
terrain le 2026-08-18). Quatre retours terrain. (1) **Pourcentage de
rattrapage** : la barre d'état passe à « N restants · P % » ; `P` = corps
présents / corpus en portée, fonction pure `backfill_percent` (sœur de
`sync_percent`, plafonnée à 99 tant qu'un corps manque), dénominateur
`bodies_total_count` ; le % vit dans le TEXTE (A55). (2) **Spam / non-spam** :
`report_spam`/`mark_not_spam` réutilisent `MoveTo` — le dossier indésirable
est résolu par compte (`canonical_folders`), c'est le fournisseur qui apprend ;
geste par fil, indisponible si pas de dossier Junk (A56). (3) **Supprimer un
brouillon** depuis la composition — geste destructif avec confirmation inline,
distinct d'« Annuler » qui conserve (A57). (4) **Réponse par message** :
Répondre/Répondre-tous/Transférer passent en bas de chaque message ; la barre
du fil ne garde que le tri + spam. **Constat terrain corrigé le jour même** :
les 3 gestes sur nos PROPRES messages aussi — répondre y vise les
destinataires d'origine (À pour Répondre, À+Cc pour Répondre-tous ; fonction
pure `reply_to`), jamais soi-même (A58). Reports : **D-21** (double COUNT du
rattrapage, famille D-8, budget tenu au terrain), **D-22** (report_spam
déjà-spam via recherche). Piège confirmé : la trace terrain (`… 2> fichier`)
échoue si le chemin n'existe pas — le « Bureau » est redirigé sous OneDrive
(`C:\Users\<u>\Desktop` absent) ; écrire à la racine du dépôt.

**Chantier soldé précédent : PLAN-RETOURS-2** (2026-08-17, `dfa6224`, A52-A54
+ ADR 0021, terrain complet, **livré en 0.1.9**). (1) **Synchro Gmail « trop
longue »** : mesurée au terrain (trace `run_sync`, ~135 s en release quand
22 vues Gmail ont bougé — ~5 s par dossier changé, bridage probable). La
sobriété (ADR 0017) tient ; c'était la **cadence** qui coûtait. Le
veilleur IDLE (ADR 0018) tenant INBOX en temps réel, le **cycle complet
passe de 5 à 30 min** (+ passe légère INBOX à 5 min en filet) — S-D4
tranché, **ADR 0021**. All Mail RESTE synchronisé (Archives intacte, ADR
0010 préservé) ; l'exclusion des vues virtuelles est reportée (STANDARD.md §2.6). (2)
**Trait de chargement** : le mode « au pourcentage » (figé chez Chromium,
A40) meurt ; le trait fait sa boucle complète dès qu'une action tourne, le
% reste dans le TEXTE (A52). (3) **« Rendre indépendante » retirée** —
placeholder inerte, multi-fenêtre reporté en chantier dédié (A53). (4)
**Cc/Cci fonctionnels** — tranche compose→Draft→outbox→SMTP→UI ; **Cci
dans l'enveloppe SMTP SEULE** (`send_raw`, jamais un en-tête Bcc servi),
« Répondre à tous » remet les Cc d'origine en Cc (`reply_all_split`) ;
brouillons locaux portent cc/bcc (A54). Piège payé : l'app **release** est
sous-système *windows* → `eprintln` **muet** en console (mesurer en
débogage ou `2> fichier`).

**Chantier soldé précédent : PLAN-RETOURS-MAIL** (2026-08-16, `19ea16a`,
A48, terrain complet, livré en 0.1.8). Quatre retours du CE sur le
courrier réel : objets/noms débarrassés des escapes `quoted-string`
d'IMAP que `imap-proto` laisse (correctif + migration de l'existant),
dossier « Envoyés » qui dit enfin le vrai destinataire, « Répondre à
tous » instantané, et le `<head><title>` de certaines infolettres qui ne
fuit plus en tête de corps. **Fait d'état à retenir : l'enveloppe stockée
porte désormais les destinataires** (`envelopes.to_addrs`/`cc_addrs`,
tirés de la même ENVELOPE que l'expéditeur) — le « l'enveloppe ne porte
que l'expéditeur » d'avant est renversé ; `reply_all_context` les lit
d'abord (hors ligne), la relève serveur n'est qu'un repli. Reports :
DETTE D-15/D-16.

### L'état du terrain — chiffres du 2026-07-26, boîte réelle

La synchronisation intégrale (ADR 0010) a tout ramené : **256 312
messages** (7 539 avant), 4 comptes, tous dossiers — spam et corbeille
compris, décision explicite du Chef Ingénieur.

**La passe d'en-têtes a convergé à zéro** : `diagnostic_fils` affiche
`jamais lus : 0` dans la portée du regroupement. Ce chiffre est final —
plus rien n'est en train de bouger côté fils. Résultat du regroupement :

| | avant ADR 0009 | avant ADR 0010 | **final** |
|---|---|---|---|
| fils de 2 à 5 | 15 (tous confondus) | 242 | **577** |
| fils de 6 à 20 | — | 6 | **35** |
| fils de plus de 20 | — | 0 | **1** |

**La portée tient à l'échelle** : 248 771 messages hors portée n'ont créé
aucun fil et n'ont fait remonter aucune conversation — c'est l'invariant
STANDARD.md §6.9, tenu par test.

**Ce qui bouge encore : le rattrapage des corps.** ~250 000 messages
attendent leur corps, à 200 par lot, au fil de l'usage — une longue
traîne de plusieurs jours ou semaines, reprenable, visible dans le
bandeau ocre de l'application. **La base grandira vers ~13 Go**
(256 312 × ~50 ko) ; le budget « < 1 Go » est levé (ADR 0010 §2) et la
garde d'espace disque veille avant chaque engagement.

**Premier réflexe d'une nouvelle session :** demander à l'utilisateur où
en est le bandeau de rattrapage et ce que pèse
`%APPDATA%\dev.elements.wind\wind.db` (avec ses compagnons `-wal` et
`-shm`). Rappel du STANDARD.md §7.1 : tu ne peux pas lire sa base toi-même.
(Avant PLAN-WIND E3 : `dev.discovery.app\discovery.db` — le déménagement
est automatique au premier lancement Wind.)

### Les budgets non tenus, avec leur remède

| Poste | Mesure (2026-07-26) | Levier |
|---|---|---|
| Page profonde d'une catégorie (hors réception) | ~129 ms à l'offset 80 000 (cœur seul, décor 200 k, 2026-08-20) | **assumé** (décision CE D1, DETTE D-26) : parcours d'index O(offset) ; un seul vol à la fois, écran qui dit le chargement, comptage hors chemin (A64) — à rouvrir si le terrain dépasse ~1 s la page |
| Adoption d'une base héritée | 3,66 s à 200 000 messages, une seule fois | **réglé en forme par l'ADR 0012** : visible, annulable, rembobinable — la durée est assumée, la passe est unique |
| Recherche | ~~113–210 ms~~ → **~66 ms ✅** (2026-08-17) | **réglé** : `prefix='2 3'` + destinataires indexés + **soupape tri-date armée** au-delà de 10 k corr. (`WIDE_QUERY_THRESHOLD`) ; mesuré sur la VRAIE base (251 k / 7 Go), pire cas préfixe 3 car. (36 k corr.) à ~66 ms (PLAN-RECHERCHE, A50) |

Le budget recherche est **tenu au terrain**. Enseignement du terrain : le
mur n'est pas le plafond de rendu (l'hydratation ne coûte que ~0,2 ms/ligne)
mais le **plancher BM25** — classer 36 k correspondances d'un préfixe 3 car.
prend ~80 ms, quel que soit le plafond, et ce plancher monte avec le corpus.
La soupape tri-date de l'ADR 0004 le résout : au-delà de 10 k correspondances,
`search_capped` classe par date (meilleur ordre pour une requête aussi large,
de toute façon), ~66 ms. Reste indexé désormais : destinataires (`to:`/`à:`),
le trou de pertinence le plus courant.

### Arbitrages — tranchés et ouverts

**Tranchés** (ne pas rouvrir sans mesure) :
- ~~Synchroniser l'archive ?~~ → **Tout est synchronisé** (ADR 0010),
  spam et corbeille compris, sans quota. La question est soldée.
- ~~Périmètre de la Phase 5 ?~~ → La migration visible et interruptible
  d'abord — **faite** (ADR 0012). Suivent, dans l'ordre : installeur,
  télémétrie, bêta.
- ~~Identifiants OAuth de l'app distribuée ?~~ → **CLOS le 2026-08-25**
  (ADR 0025, décision D1 de PLAN-RETOURS-9). Les client ids sont
  compilés dans la release par le seul `faire-release.ps1`
  (`option_env!("WIND_RELEASE_*")`, tout-ou-rien) ; la variable
  d'exécution garde la priorité en dev et en e2e, et le test
  `dev_builds_embed_no_credentials` crie sur un build empoisonné. **La
  preuve terrain qui fermait l'arbitrage est faite** : un compte
  connecté sur le second poste depuis une release publiée, sans aucun
  `setx`. Elle aura glissé de deux versions — attendue à la 0.8.0,
  venue après la 0.9.0.

**Ouverts** (au Chef Ingénieur) :
- **Recherche sans limite pratique** (2026-08-17) — le plafond lui-même est
  soldé (`SEARCH_LIMIT = 100`, barre « N sur M » avec le vrai total ;
  A50/PLAN-RECHERCHE). Reste ouvert le seul vrai « tout voir » : liste de
  résultats **virtualisée + pagination par curseur** (le mur : hydratation
  `SELECT_UNIFIED` par ligne + liste non fenêtrée). Chantier à part.
- **Tri par date de la recherche** — **armé** au terrain (2026-08-17) : le
  plancher BM25 d'un préfixe 3 car. très large (36 k corr.) dépasse le budget
  quel que soit le plafond. `search_capped` bascule sur la date au-delà de
  `WIDE_QUERY_THRESHOLD` (10 k corr.) ; en deçà, BM25. Seuil calé sur cette
  machine — à re-mesurer si le budget se tend en bêta.
- **Doublons multi-boîtes dans la recherche** — observé au terrain : le
  même message vit copié dans plusieurs boîtes (« 19 messages partagent
  un Message-ID »), et la recherche renverra chaque copie. Dédoublonner à
  l'affichage ? À observer en usage réel avant de décider (D2, gardé ouvert).

### Ensuite — la Phase 5

Durcissement et bêta ([PLAN.md](PLAN.md) §4). Ordre arbitré : migration
visible et interruptible **✓ faite (ADR 0012)** → installeur + mise à
jour signée **✓ faite (ADR 0013)** → télémétrie de crash opt-in **✓
faite (ADR 0014)** → **bêta fermée 20-50 utilisateurs (prochain)**.
Gate 5 : deux semaines sans défaut critique.


---

## Ce qui reste

### Le chantier fait : migration visible et interruptible (ADR 0012)

Terminé et **validé au terrain** le 2026-07-26, sur copies. L'adoption
est une unité transactionnelle unique (du DROP conditionnel des tables
de fils jusqu'à `user_version`) : annuler rembobine tout, la passe se
rejoue entière au prochain lancement. Écran modal au démarrage — chaque
commande ouvre sa propre connexion, sans porte la première venue
paierait la passe en silence. Preuves : test de rembobinage sur une
vraie base de fichier, banc (3,66 s, pas de régression), annulation
exercée en pleine passe à l'échelle du gate 3.

### Le chantier fait : installeur NSIS + mise à jour signée (ADR 0013)

Terminé et **validé au terrain** le 2026-07-26 : la boucle 0.1.1 → 0.1.2
s'applique sur l'app installée, base intacte. **Re-validé le 2026-08-16 à
l'échelle de la refonte** : l'auto-update 0.1.6 → 0.1.7 (la refonte
entière) s'applique sur l'app installée, chaîne signée prouvée vivante.
NSIS (**pas MSIX** — il
virtualiserait `%APPDATA%` et orphelinerait la base) ; updater Tauri
signé minisign, piloté depuis Rust (capabilities au minimum) ; signature
de code Windows reportée à la bêta. Publication d'une version :
`scripts/faire-release.ps1 <version>` fait TOUTE la release — depuis
PLAN-RETOURS-8/ADR 0023 en **bi-arch** (arm64 natif + x64 cross,
tout-ou-rien, 5 assets, `latest.json` à deux clés, tag = version
nue) ; vérification scriptée par `scripts/verifier-release.ps1`.

### Le chantier fait : télémétrie de crash locale et opt-in (ADR 0014)

Terminé et **validé au terrain** le 2026-07-26. Fichier local seul
(aucun réseau, aucun tiers), panics backend seuls, opt-in off par
défaut ; le **message du panic est supprimé** (seul vecteur de donnée
personnelle), prouvé à deux niveaux (mémoire et fichier écrit). Le hook
ne touche jamais la base (consentement en fichier + `AtomicBool`).
Trouvaille terrain corrigée : un crash sur le thread principal produit un
**double panic** à la frontière FFI de WebView2 — compteur `SEQ` (noms
uniques) + filtre du secondaire `cannot unwind`.

### Le chantier fait : plus aucune commande sur le thread principal (ADR 0019)

Terminé et **validé au terrain** le 2026-08-15 (PLAN-GELS, `e32280b`,
A39/A40). Le freeze du démarrage (25,2 s de gels cumulés sur 40 s,
mesurés) est mort à la racine : toute commande bloquante passe par
`hors_pompe()` — spawn_blocking + verrou global, la sérialisation
d'avant conservée — tenu par la gate `garde-thread-principal.mjs` et le
budget « aucun gel de pompe > 150 ms » (`sonde-gel.py`). Au passage, le
terrain a livré et fait corriger le jour même : l'avancement figé à
99 % par les départs en attente de rejeu (le dénominateur s'ajuste), et
la boucle du trait hitofude morte-née (animation CSS dans un `<mask>`
non rendu → SMIL). Dette ouverte : D-8 (sondes chères, hors pompe).

### Le chantier suivant : bêta fermée 20-50 utilisateurs

Dernière étape avant le gate 5 ([PLAN.md](PLAN.md) §4). Kaizen
hebdomadaire sur les frictions **observées**. **Engagée le
2026-08-28** (PLAN-RETOURS-11 R3) : actions à
[PLAN-BETA.md](PLAN-BETA.md), guide à [BETA.md](BETA.md), canal de
retours dans l'app (bouton Feedback, A91) — bloquant restant : faire
recevoir `feedback-wind@fcts.io` (alias fcts.io, côté CE).

### La longue traîne en cours

Le rattrapage intégral des corps (~250 000 messages restants) avance à
200 par lot au fil de l'usage. Rien à coder ; surveiller le disque et le
bandeau. La recherche gagne en profondeur à mesure.

### Reports assumés

- **Requêtes chères des sondes périodiques** (PLAN-GELS D4) : hors de
  la pompe elles ne gèlent plus rien, mais leur coût CPU reste réel —
  registre **D-8** de [DETTE.md](DETTE.md), chiffres et pistes dedans.
- **Doublons multi-boîtes dans la recherche** (nouveau, ADR 0010) : le
  même message copié dans plusieurs boîtes remonte plusieurs fois dans
  les résultats. À observer en bêta avant de décider d'un dédoublonnage.
- **Tri par date de la recherche** — **armé** (A50, PLAN-RECHERCHE) : au-delà
  de 10 k correspondances (`WIDE_QUERY_THRESHOLD`), le classement bascule sur
  la date, le plancher BM25 dépassant sinon le budget. Plus un report.
- **Défilement profond de la LISTE** — la PANNE est morte
  (PLAN-DEFILEMENT-PROFOND, A64, 2026-08-20) : file bornée à UN vol,
  écran vide honnête, comptages hors du chemin d'affichage
  (`category_total` séparé, nav allégée à ses deux non-lus). Reste
  ASSUMÉ (décision CE D1, DETTE **D-26**) : le parcours d'index
  O(offset) d'une page profonde hors réception (~129 ms à l'offset
  80 000 sur 200 k, cœur seul) — une seule vole à la fois, l'écran dit
  le chargement ; à rouvrir si le terrain dépasse ~1 s la page.
- **Affichage des messages importants REÇUS** (drapeau, tri, filtre) —
  refus de périmètre explicite de PLAN-RETOURS-6 : le composeur pose
  les en-têtes de priorité à l'ENVOI ; lire ceux des messages entrants
  est un chantier à part. Fait d'instruction utile : Gmail web
  n'affiche AUCUN indicateur pour `X-Priority`/`Importance` (marqueur
  algorithmique maison) — Outlook/Thunderbird montrent le « ! » ;
  l'en-tête se vérifie par « Afficher l'original ».
- **Épingle orpheline si le message-clé quitte sa boîte** (DETTE
  **D-28**, PLAN-RETOURS-7) : l'épingle est portée par la seule
  enveloppe du geste — un tiers qui supprime exactement ce message la
  fait sauter en silence. Cas limite assumé ; jamais d'affichage faux
  (la jointure écarte les orphelines). À rouvrir si le terrain ou la
  bêta rapporte des épingles qui « sautent ».
- **Filtre « a une pièce jointe »**. (L'**envoi de pièces jointes**
  est LIVRÉ — PLAN-PIECES-JOINTES soldé, `38cd812`/`27ed056` ; le
  **`to:` dans la recherche** est LIVRÉ — A49.)
- **CONDSTORE réel, IDLE/push** — reports de Phase 1 inchangés.
- **Dossier CASA Google** — chemin critique du lancement public, côté
  produit-owner.

### Dette connue, non corrigée

`apps/desktop/ui/style.css` : la règle d'élément `header { display: flex }`
s'applique aussi à `#detail-header`. Tout enfant pleine largeur ajouté là
devient un item flex écrasé à 0 px. (Le bandeau d'avancement de
l'ADR 0010 et l'écran de migration de l'ADR 0012 ont été placés **hors**
de tout `<header>` pour cette raison.)

Cousin de cette dette, désormais **tenu par une règle** : toute règle
d'ID qui pose un `display` écrase le `[hidden]` du navigateur et exige
son garde-fou `#id[hidden] { display: none }`. Huit occurrences à ce
jour ; la dernière (`#detail`) laissait l'iframe sandboxée capter le
premier clic et tuer les raccourcis clavier (STANDARD.md §9). Un E2E tient le cas.

### La Phase 5

Installeur MSIX/NSIS + mise à jour signée, télémétrie de crash opt-in,
bêta fermée 20-50 utilisateurs, kaizen hebdomadaire sur les frictions
**observées**. Gate 5 : deux semaines sans défaut critique.

