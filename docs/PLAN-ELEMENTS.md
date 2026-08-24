# PLAN-ELEMENTS — le Système v2 « Elements » devient la référence, et l'UI le livre

> Énoncé (2026-08-24) : `docs/design/systeme.v2.dc.html` doit devenir le
> nouveau système design de référence de Wind et être implémenté.

## 1. Constat — ce qui est prouvé

**Le Système v2 existe, il est mesuré, et ses arbitrages sont déjà
tranchés.** `docs/design/systeme.v2.dc.html` (480 Ko) est **généré** par
`spikes/direction-elements/v2/` (`faire.mjs`), sur la matière de deux
spikes (`direction-elements`, `marque-hitofude`). Il porte :

- **14 décisions CE au journal (V1–V14)**, toutes datées 2026-08-24,
  dont : la marque Elements et le jeton `--marque` (V1), la mort du
  trait hitofude au profit du disque/anneau — confrontée à 6
  alternatives et confirmée (V2), la mort de `--panel` (V3), le rond
  rendu au disque — tuile d'initiales carrée, compteur de nav en nombre
  (V4), le repère de compte qui GARDE son glyphe (V5, mesuré :
  appliquer la doctrine régresserait WCAG 1.4.1), le registre
  d'affichage graisse 340 (V6), **2 thèmes au lieu de 28** (V7,
  renverse A42), **les 78 glyphes dessinés en SVG en ligne, la fonte
  meurt** (V8), la dette du palier 16 (V9), et **zéro rayon** (V14,
  validé au terrain le 2026-08-24 sur le rendu réel — réserve nommée :
  l'idiome Fluent se re-constatera à la première fenêtre livrée).
- **Trois gardes à la génération** : relevé d'icônes ↔ catalogue dans
  les deux sens (A18 rendu assertion), 76 paires + 24 repères de
  contraste calculés (0 échec), journal A1–A78 repris verbatim.
- **Le coût d'adoption relevé fichier par fichier** (section Thèmes) :
  sept fichiers, trois contrôles de gate.

**L'état du code (reconnaissance du 2026-08-24, sur pièces) :**

| Poste | Fait mesuré |
|---|---|
| Icônes | fonte Material Symbols vendorisée (`?v=78`), classe `.ms` en ligature, **95 emplois dans 10 composants** ; tailles 10–18 px, défaut 16 px ; preuve `apercu.html` 79/79 |
| Thèmes | 28 fiches dans `lib/theme.js`, table des jetons écrite à la main dans `systeme.css` (17 rôles × 28 blocs), repli `nature`, persistance `localStorage['wind-theme']` |
| `--panel` | **16 emplois dans 7 composants** — exactement le relevé du v2 |
| Rayons | **~85 occurrences** de `border-radius` (2–11 px, 50 %, 99/999 px) dans 13 fichiers |
| Hitofude | `Hitofude.svelte` (SMIL dans `<mask>`), 4 emplacements : entête, barre d'état, tiroir, accueil. La modale de migration a sa PROPRE jauge linéaire |
| Typo | stack système seule (`Segoe UI`), **aucune graisse 340 déclarée** — le repli 400 de V6 est le cas réel d'aujourd'hui |
| Rust | **rien à toucher** : les noms de glyphes des repères ne changent pas, l'allowlist `commands.rs` reste vraie |
| Gates | `jetons.mjs` (NOMBRE_ATTENDU = 28, repli `nature`), `coherence-systeme.mjs` (7 contrôles), `contraste.mjs` (25 paires × 28 thèmes = 700 mesures), `refonte-ecran02.spec.js` (nomme `nature`/`nature-nuit`) |

## 2. Périmètre

**Fait** : la bascule documentaire (le v2 devient LE Système, au chemin
que la gate lit), puis l'implémentation intégrale de V1–V14 dans ui-v2 —
jetons et 2 thèmes, retrait de `--panel`, icônes SVG en ligne, zéro
rayon, tuile d'initiales, marque Elements, disque/anneau, registre 340 —
avec les gates réécrites au fil (le relevé « sept fichiers, trois
contrôles » du v2 est le contrat).

**Refus explicites (§2.6)** :

- **Le palier 16 n'est pas dessiné dans ce chantier** (selon D4 §5) :
  les maîtres réduits sont livrés, la dette V9 est consignée, le STOP
  visuel et le terrain jugent la netteté réelle.
- **La dalle du corps de courriel ne bouge pas** : `mail-render` bake
  `#222222`/`#ffffff` (A61 — jamais re-transmettre une palette au corps).
- **L'icône d'application garde son rayon de plateforme** (15/64) —
  l'exception déclarée et permanente de V14.
- **Les 4 glyphes réservés restent réservés** (`open_in_new`, `link`,
  `format_quote`, `storage`).
- **Aucun comportement ne change** : écrans, gestes, textes, dispositions
  et raccourcis sont hors périmètre — ce chantier change la peau, pas la
  chair. Tout constat fonctionnel découvert en route part en chantier ou
  en dette, pas dans ce diff.
- **Pas de retour partiel des 28 thèmes** ni de « thème personnalisé » :
  V7 est tranchée. Le suivi clair/sombre de l'OS (`wind-theme-auto`)
  survit — il n'a plus que deux cibles.

## 3. Options et verdicts — le set-based est déjà payé

La conception set-based a eu lieu dans les spikes, sur mesures :

- **Direction** : `spikes/direction-elements/` — banc de contraste 74
  mesures 0 échec, palette du document corrigée du minimum à teinte
  constante (`--mut` 4,19 → 5,21:1, filet au seuil du filet expédié),
  centrage du disque mesuré à 0,00 px. Verdict : la direction tient sur
  un client courrier ; **ne pas adopter par morceaux**.
- **Signature** : 7 propositions animées à taille réelle
  (`signatures.mjs`) — l'anneau gardé, les six autres écartées avec
  leur raison (V2).
- **Coins** : arbitrage V10 (3 rayons) dépassé le jour même par V14
  (zéro rayon), mesuré (78 glyphes, 654 commandes, aucun coin arrondi)
  et validé au terrain. Rembobinage en une ligne si la première fenêtre
  livrée dit autre chose.

Aucune option nouvelle à départager ici : le chantier est une
**exécution** de décisions déjà instruites, plus cinq arbitrages
résiduels (§5).

## 4. Étapes

Chaque étape se termine gate-verte (`/gate`) et commit (DC-D2 : le
Système est déjà amendé — c'est lui qui mène). TDD : chaque étape ouvre
sur le RED de ses gates/e2e adaptés avant le code.

### E1 — La bascule du socle (doc + jetons + thèmes + gates)

Un seul commit cohérent, parce que la gate compare doc ↔ CSS ↔ fiches ↔
catalogues dans les deux sens :

- `systeme.v2.dc.html` → **`docs/design/systeme.dc.html`** (le chemin
  que la gate lit) ; l'ancien archivé selon D1.
- `systeme.css` : table des jetons réécrite — 17 jetons × 2 thèmes,
  valeurs du contrat (`elements` = `:root` sans attribut, repli),
  `--panel` retiré ; les **16 emplois** retombent sur `--bg` (dont les
  deux « états » relevés par V3 : tuile de date éteinte, garde d'images).
- `lib/theme.js` : 2 fiches, pastilles sans `panel` ; **migration du
  choix mémorisé** : tout `*-nuit` → `elements-nuit`, le reste →
  `elements` (précédent : la migration « La nuit » → `nature-nuit`).
- Catalogues fr/en : 2 clés `theme.<id>.nom` ; accueil étape 3 : 2
  cartes ; récapitulatif idem.
- Gates : `jetons.mjs` (NOMBRE_ATTENDU 28 → 2, repli `elements`),
  `contraste.mjs` (la table du v2 : 37 paires + 24 repères, × 2
  thèmes), `coherence-systeme.mjs` contrôles 2 et 3 retirés (le
  contrat de fonte meurt), 4 et 5 réduits à 2 fiches,
  `refonte-ecran02.spec.js` renommé sur les thèmes Elements.

**⛔ STOP visuel précoce** : l'app sous les jetons Elements (encore avec
la fonte d'icônes et les rayons d'avant — dit clairement au CE).

### E2 — Les icônes dessinées, la fonte meurt (V8)

- Le catalogue (`jeu.mjs`, 78 glyphes + marque) versé dans
  `ui-v2/src/lib/` ; composant `Icone.svelte` (SVG en ligne, viewBox 24,
  `currentColor`, miroir, tailles) ; l'état « dossier ouvert »
  (aujourd'hui FILL 1/600) redéfini dans la grammaire selon le relevé
  du Système.
- Les **95 emplois** de `.ms` remplacés ; la fonte, `?v=78` et la copie
  `public/icones/` retirés ; `assets/icones/README.md` remplacé par le
  renvoi au relevé du Système (le relevé EST l'inventaire) ; mention
  « À propos » selon D2 ; fusions selon D3.
- **Gate A18 neuve** : le relevé de la section Icônes du Système ↔ le
  catalogue du code, dans les deux sens (reprend l'assertion du
  générateur) ; l'ancienne preuve 79/79 retirée avec la fonte.

**⛔ STOP visuel icônes** : la netteté à 16 px et 10–12 px au rendu réel —
c'est ici que la dette V9 (D4) se juge sur pièce.

### E3 — Les formes : zéro rayon, la tuile, le disque (V4, V14)

- ~85 `border-radius` → **0**, sauf : disque/pastille de repère (50 %),
  poignée d'interrupteur (50 %), piste d'interrupteur (999) ; la jauge
  de migration perd sa pilule.
- Avatar d'initiales → **tuile carrée** 28 px, sol `--tuile`, encre
  `--tuileInk`, **filet 1 px** (mesuré : sans lui la tuile n'existe pas).
- Compteur de non-lus de la nav : la pilule meurt, **nombre nu** en
  chiffres tabulaires à l'accent ; le non-lu d'une rangée gagne son
  **disque 9 px** `--marque`, centré par construction (flex — leçon du
  spike : 0,00 px mesuré).

### E4 — La marque et la signature (V1, V2, V11)

- `Hitofude.svelte` meurt (entête, barre d'état, tiroir, accueil) ; la
  marque Elements le remplace — **deux régimes** (V11) : tuile figée
  hors thèmes (accueil, migration, À propos), glyphe à l'encre courante
  (entête, tiroir).
- **Disque/anneau** : disque plein 9 px au repos, anneau évidé tournant
  pendant un cycle — barre d'état et modale de migration. A52 tenu : le
  pourcentage reste dans le TEXTE.

### E5 — Le registre d'affichage (V6)

- Titre de conversation (24 px) et hero d'accueil (40 px) en graisse
  340, interlettrage −.03em, via `Segoe UI Variable Display` en tête de
  pile ; **repli 400 explicite** (V6 : aucun dessin ne dépend de la
  graisse). Aucune fonte embarquée.

### E6 — Qualité et documentation

- Captures réelles de l'accueil régénérées (`e2e/capture-accueil.mjs`).
- **Revue à regard neuf** : `/code-review high` sur le diff complet ;
  corriger le confirmé.
- **Gate complète** ; journal du Système : entrée d'adoption (série
  selon D1) ; **ADR** (structurante : le Système Elements remplace
  Wada — modèle 0004, réversibilité V14 en une ligne) ; CHANGELOG ;
  ETAT ; DETTE (V9 palier 16, et ce que la revue reporte).

Puis **⛔ STOP 2 terrain** (checklist + commandes PowerShell :
`scripts/terrain.ps1`, `scripts/lancer-wind.ps1`), corrections le jour
même, re-gate, et Phase 5 (commits sans accents, push + CI en arrière-
plan, `/solde` à la CI verte). Release selon D5.

## 5. § Décisions CE

- **D1 — La mécanique de bascule documentaire.** Le contenu v2 remplace
  `docs/design/systeme.dc.html` au même chemin. (a) **Le HTML devient
  LA source, éditée à la main** ; le générateur reste figé en spike
  (trace), l'ancien Système part en archive ; le journal continue en
  série A (A79 = l'adoption), la série V est close. (b) Le générateur
  est promu hors de `spikes/` et le Système reste généré — une
  toolchain de plus à tenir.
  — *Réponse CE (2026-08-24) : « HTML = source à la main » — option (a).*
- **D2 — L'attribution des glyphes** (V8 : « à trancher avant toute
  adoption »). La ligne « À propos » dit aujourd'hui « Material Symbols
  Rounded (Google), licence Apache 2.0 ; police embarquée ». Demain :
  (a) « Icônes : jeu original de Wind, dessiné d'après Material Symbols
  (Google, Apache 2.0) » — dit vrai et garde la courtoisie de l'origine ;
  (b) retirer toute mention ; (c) autre formulation du CE.
  — *Réponse CE (2026-08-24) : « Dessiné d'après Material » — option
  (a), le fichier LICENSE des icônes adapté en conséquence.*
- **D3 — Les trois fusions de glyphes** (`archive`/`inventory_2`,
  `download`/`system_update_alt`, `check_circle`/`cancel`/`error`/
  `info`) : réduits à la grammaire ils retombent sur le même dessin, et
  A3 (« une icône, un sens ») ne tolère pas deux sens sur un dessin.
  (a) **Différencier** — un détail minimal hors grammaire par famille,
  consigné comme écart ; (b) fusionner : réattribuer un seul glyphe par
  famille et amender le relevé ; (c) livrer identiques (A3 rompu, dit).
  — *Réponse CE (2026-08-24) : « Différencier » — option (a), les
  redessins soumis au retour CE comme les trois tours précédents.*
  *Verdict au STOP E2 (2026-08-24) : « Suffit — consigner tel quel » —
  les marques distinctives actuelles (poignée corps/couvercle ; bandeau
  supérieur ; marque intérieure du cercle) suffisent à 16 px, aucun
  redessin ; le relevé du Système consigne les familles.*
- **D4 — La dette V9 (palier 16).** (a) **Livrer les maîtres réduits**,
  dette consignée, le STOP visuel d'E2 et le terrain jugent — rouvrir
  en chantier dédié si le flou se voit ; (b) dessiner d'abord les 74
  paliers 16 + 12 paliers 10–12 (86 dessins, chantier de dessin avant
  toute livraison).
  — *Réponse CE (2026-08-24) : « Maîtres réduits » — option (a), dette
  V9 consignée à DETTE, verdict de netteté au STOP visuel d'E2 et au
  terrain.*
- **D5 — Le véhicule de release.** La 0.8.0 est publiée (preuve OAuth
  du second poste encore différée). (a) **0.9.0, MINEUR**, à la fin du
  chantier terrain-validé — la preuve OAuth du second poste peut se
  faire sur la 0.9.0 ; (b) attendre la preuve 0.8.0 avant de publier ;
  (c) autre.
  — *Réponse CE (2026-08-24) : « 0.9.0 MINEUR » — option (a), la preuve
  OAuth du second poste peut se faire sur la 0.9.0 et fermer
  l'arbitrage au même geste.*

Les cinq décisions sont tranchées le 2026-08-24 — **GO de la Phase 2.**
