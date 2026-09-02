# PLAN-RETOURS-12 — cinq retours CE du 2026-08-28

> **CHANTIER SOLDÉ le 2026-08-29 — terrain complet.** GO CE du
> 2026-08-28 (STOP 1, D1-D6 tranchées) ; E1-E5 implémentées le jour
> même, STOP visuel validé (« GO, continuer ») ; revue à regard neuf
> 8 angles / 10 retenues / 8 corrigées (§4bis), dettes D-43/D-44 ;
> gate complète VERTE (2,8 min, e2e 150 → **153**) ; **terrain CE 4/4
> le 2026-08-29, zéro constat** (« Terrain ok sur les 4 points ») ;
> commits `60225b0` (chantier) + `331832d` (piège d'outillage payé au
> pre-push : le gabarit de seed périme à MINUIT, pas seulement au TTL
> — deux specs rouges à 00 h, verts après purge ; launch.mjs exige
> désormais le même jour calendaire) ; **CI verte run 33216010954**.
> Journal A92 (entête deux lignes) / A93 (marque 28 px). À livrer à la
> prochaine release (entrée CHANGELOG 0.13.0 écrite au solde, §2.9).
>
> Kaizen : 2 gates complètes jouées (1 rouge fmt) + 2 pre-push (1
> rouge minuit) ; 0 constat KO au STOP 2 ; agents : 3 reconnaissance +
> 8 revue (~370 k tokens agents).
>
> Livré : E1 (compteAjoute rappelle connecter(), e2e RED→GREEN,
> couture `__e2eAjout`), E2 (entête deux lignes + `noms_adresses`
> cœur/commande/UI, e2e 2 specs neufs + ecran02 réaligné, A92), E3
> (marque 28 px, A93), E4 (bump workspace dans make-release.ps1,
> Cargo.toml aligné 0.12.0), E5 (instrumentation eprintln du chemin de
> MAJ — RED impossible : pure trace sur un chemin qui exige une release
> réelle, dit ici plutôt que simulé).
> Énoncé : cinq retours — (R1) compte fraîchement ajouté marqué
> « Déconnecté » aux Réglages, (R2) taille du package / bandeau de MAJ
> plus long, (R3) logo d'entête plus grand, (R4) versions internes en
> 0.1.0, (R5) disposition de l'entête du message dans le volet de
> lecture.

---

## 1. Constat — instruction sur pièces (2026-08-28)

### R1 — « Déconnecté » après un ajout de compte, Wind ouvert

**Reproduit sur pièces, racine identifiée.** Il n'existe **aucun
statut de connexion en base** : Réglages dérive l'état d'un pur calcul
d'appartenance — `estDeconnecte = (c) => !connectes.includes(c.email)`
(`apps/desktop/ui-v2/src/Reglages.svelte:272`). Le tableau `connectes`
(`App.svelte:61`) n'est rempli qu'à **un seul endroit** : le retour de
`connect_accounts` dans `connecter()` (`App.svelte:728`), appelée **une
seule fois, au démarrage** (`App.svelte:945-948`).

Or l'ajout d'un compte (`add_account`, `commands.rs:287-341`) insère
bien la session OAuth vivante côté Rust (`commands.rs:339`) — le compte
EST connecté — mais le callback UI `compteAjoute()`
(`App.svelte:1169-1173`) fait `chargerNav()` + `synchroniser()` **sans
jamais rappeler `connecter()`**. Le geste « Reconnecter » d'un compte
mort, lui, le fait (`App.svelte:1643`). Résultat : l'adresse neuve
manque au tableau `connectes` jusqu'au prochain redémarrage → badge
« Déconnecté » mensonger.

`connect_accounts` est silencieuse et isolée par compte
(`authenticate_silent` sur jetons stockés, `commands.rs:151-219`) : la
rappeler après un ajout est le motif déjà employé par « Reconnecter ».

### R2 — taille du package : **le fait mesuré dit non**

Tailles des exe des 12 dernières releases (`gh release view`, octets) :

| Version | arm64 | x64 |
|---|---|---|
| 0.3.0 | 5 038 998 | — |
| 0.4.0 | 5 055 194 | — |
| 0.5.0 | 5 066 813 | — |
| 0.6.0 | 5 504 084 | 6 215 897 |
| 0.7.0 | 5 668 094 | 6 390 669 |
| 0.8.0 | 5 671 223 | 6 397 182 |
| 0.9.0 | 5 629 324 | 6 351 726 |
| 0.10.0 | 5 632 535 | 6 350 877 |
| 0.10.1 | 5 632 016 | 6 355 666 |
| 0.10.2 | 5 636 577 | 6 359 753 |
| 0.11.0 | 5 630 211 | 6 354 494 |
| 0.12.0 | 5 663 062 | 6 397 363 |

**La taille est plate.** Une seule marche : +0,44 Mo à la 0.6.0
(naissance du bi-arch + repères de comptes). Depuis la 0.7.0, la
variation est de ±1 % ; 0.11.0 → 0.12.0 : **+0,5 %** (32 851 o arm64).
La taille du package n'explique PAS un bandeau plus long.

**L'hypothèse de rechange** : depuis la 0.10.2 (PLAN-SIGNATURE E4), le
bandeau « Téléchargement et installation… » couvre honnêtement TOUT le
chemin — téléchargement, écriture du témoin, `spawn` de l'installateur
avec son scan antivirus synchrone et le verdict cloud Smart App Control
par binaire (`commands.rs:5493-5548`) — là où l'ancien plugin sortait
par `exit(0)` sans attendre ni vérifier (l'app se fermait vite, y
compris quand rien ne s'installait). Le temps perçu vit dans le réseau
(CDN GitHub) et dans le verdict SAC, pas dans les octets. **Aucune
mesure de durée n'existe aujourd'hui** sur ce chemin : on ne peut pas
départager réseau / antivirus / SAC sans instrumenter.

### R3 — le logo d'entête

La marque est rendue par `Marque.svelte`, posée à **24 px** dans
l'entête (`App.svelte:1480`, régime glyphe) et dans le tiroir mobile
(`App.svelte:1596`). Les 24 px datent de PLAN-RETOURS-10 (décision D2 :
20 px « se perdait dans l'entête de 52 »). L'entête fait **52 px** :
le plafond raisonnable sans toucher sa hauteur est ~32 px. Le texte
« Wind » adjacent est à **18 px**/600 dans l'app (`App.svelte`,
`.marque` — corrigé en revue : le plan citait les 15 px de la fiche V11
du Système, qui était elle-même en retard sur l'UI) — grossir le glyphe
sans regarder le mot déséquilibre la paire.

### R4 — versions internes en 0.1.0

Une **unique** déclaration : `Cargo.toml:15`
(`[workspace.package] version = "0.1.0"`), héritée par les 6 crates,
l'app `wind-desktop` et le spike membre via `version.workspace = true`.
Jamais touchée depuis la création du dépôt. La version produit vit
seule dans `apps/desktop/tauri.conf.json:4` (0.12.0), bumpée par
`make-release.ps1:113-121`, qui ne committe jamais `Cargo.toml`.
Les deux axes sont totalement découplés — le grief du CE est fondé.
Côté JS, `ui-v2/package.json` et `e2e/package.json` n'ont **aucun**
champ version (rien à corriger : privés, jamais publiés).

### R5 — l'entête du message dans le volet de lecture

Le bloc vit dans `Fil.svelte:296-319` (composant unique, monté par le
volet ET l'écran 03). Aujourd'hui :

- ligne 1 : nom de l'expéditeur + bloc « sur ‹boîte› » (A80, règle D7 :
  seulement si ≥ 2 comptes / vue mélangée) ;
- ligne 2 : « ‹adresse expéditeur› · à ‹destinataire› » (`conv.adrDest`),
  où `destinataire()` (`Fil.svelte:137-145`) ne montre les `to_addrs`
  que sur NOS messages, sinon une heuristique de repli.

La cible CE :

- **Ligne 1** : `Nom de l'expéditeur <adresse> sur Boîte` ;
- **Ligne 2** : `À : Nom 1 <adresse 1>, Nom 2 <adresse 2>, …`.

**Point dur factuel** : `envelopes.to_addrs` ne stocke que des
**adresses nues** (`address_literal`, `mail-imap/src/convert.rs:348-356`
— `mailbox@host`, le nom d'affichage de l'ENVELOPE est jeté). Les noms
des destinataires n'existent nulle part en base… **sauf dans l'annuaire
des correspondants** (`correspondants(address, name, …)`,
`mail-core/src/correspondants.rs` — PLAN-RETOURS-5), qui apprend les
noms du courrier vu. Trois voies possibles, § 3.

---

## 2. Périmètre — et refus explicites (§2.6)

**On fait** : R1 (correctif), R3 (ajustement visuel avec STOP visuel),
R4 (outillage de release), R5 (disposition d'entête). R2 : **verdict
sur mesure** — la taille est hors de cause ; l'action éventuelle
(instrumenter la durée de MAJ) est une décision CE (D1).

**On refuse** :

- R2 : un chantier de réduction de taille du binaire — le fait mesuré
  ne montre aucune augmentation significative ;
- R5 : le rapatriement des noms de destinataires par re-synchro serveur
  de l'existant (des heures de trafic IMAP pour un affichage) ;
- R4 : des versions **par crate** gérées à la main — sept nombres à
  faire vivre pour des crates jamais publiées, la complexité sans le
  bénéfice ;
- tout retour à `downloadAndInstall` du plugin updater (PLAN-SIGNATURE
  a payé pour savoir).

---

## 3. Options (R5 — les noms des destinataires)

| Option | Coût | Verdict proposé |
|---|---|---|
| **A. Annuaire des correspondants** : au chargement du fil, résoudre `to_addrs` → noms via `correspondants` (une requête bornée à la page, locale, table PETITE dédiée à la frappe — leçon A64 respectée) ; repli = adresse nue | ~1 commande ou enrichissement de `thread_view`, requête indexée sur PK | **Recommandé** — les noms existent déjà, zéro trafic réseau, l'existant en profite |
| B. Adresses nues seules | nul | Dégrade l'intention CE (« Nom <email> ») |
| C. Stocker les noms à la synchro (colonne neuve) | migration + les messages DÉJÀ en base restent sans nom | N'aide pas l'existant ; peut venir plus tard en complément si le terrain le demande |

---

## 4. Étapes

- **E1 — R1** : e2e RED (ajout d'un compte via la couture, Réglages dit
  « Déconnecté ») → `compteAjoute()` rappelle `connecter()` (le motif de
  `onreconnecte`, `App.svelte:1643`) → GREEN.
- **E2 — R5** : entête du message réécrite en deux lignes (D4/D5/D6
  tranchées) ; résolution des noms par l'annuaire (option A) ; e2e sur
  la structure visible ; **STOP visuel précoce** dès le premier rendu ;
  amendement `systeme.dc.html` dans le même commit (DC-D2, journal A-n).
- **E3 — R3** : marque d'entête à la taille tranchée (D2) ; **STOP
  visuel** sur capture avant de dérouler ; amendement Système (DC-D2).
- **E4 — R4** : `make-release.ps1` bumpe AUSSI
  `[workspace.package] version` (+ `Cargo.lock`) dans le même commit de
  release ; alignement immédiat une fois sur la prochaine version.
- **E5 — R2** : consignation du constat (ETAT) ; si D1 = instrumenter,
  durées téléchargement / écriture / spawn dans la trace (`lancer-wind`
  §9) pour la prochaine MAJ réelle.

Puis : revue à regard neuf, `/gate`, STOP 2 terrain, documentation,
commit, CI.

## 4bis. Revue à regard neuf (2026-08-28)

8 angles (3 correctness, réutilisation, simplification, efficacité,
altitude, conventions), 10 trouvailles retenues après dédup — **8
corrigées** dans la session : garde `nom !== adresse` unifiée
(`etiquette`, LA forme unique), `chargerNav()` repassé AVANT le réseau
dans `compteAjoute`, couture `__e2eAjout` composée avec `__e2eRetenue`
et retirée par le spec (+ preuve par journal que l'UI relit le cœur),
échec de `noms_adresses` dit en console, cache `cacheNoms` hissé dans
`lib/fil.svelte.js` (zéro RPC redondant à la bascule de cadre),
`Option<Option<>>` absorbé par le SQL, validations de
`make-release.ps1` remontées avant toute écriture, fait « Wind 15 px »
corrigé (18 px réels — plan, fiche V11 et A93). **2 consignées** :
dette D-43 (l'écho n'a pas de colonne Cc — l'entête change à la
réconciliation) et D-44 (`connectes` n'est rafraîchi par aucun cycle —
un jeton révoqué Wind ouvert dit « Connecté » jusqu'au redémarrage, le
symptôme miroir de R1). Limite consignée sur E5 : les traces `maj :`
sont invisibles dans le binaire livré (app fenêtrée sans stderr) — la
mesure exige un lancement par `scripts/run-wind.ps1` (piège §9), dit
à la checklist terrain. Refus maintenus : la boucle par clé primaire de
`noms_adresses` reste (mesurée ~0,2 ms pour 40 adresses — un `IN`
n'achèterait que de la complexité) ; la résolution des noms reste une
commande dédiée (décision D4) ; l'instrumentation MAJ reste en
`eprintln` (le motif du fichier).

## 5. Décisions CE — tranchées le 2026-08-28

- **D1 (R2)** : la taille étant plate (fait mesuré §1), clore R2 sans
  code, OU instrumenter les durées du chemin de MAJ ? —
  **« Instrumenter les durées »** : durées téléchargement / écriture /
  lancement dans la trace, la prochaine MAJ réelle dira où part le
  temps (réseau, antivirus, SAC).
- **D2 (R3)** : quelle taille pour la marque d'entête ? —
  **« 28 px »** (+17 %, l'entête de 52 px respire encore, le mot
  « Wind » — 18 px réels, voir la correction de revue au §1 — reste
  équilibré).
- **D3 (R4)** : versions internes ? — **« Aligner sur la version
  produit »** : `make-release.ps1` bumpe aussi
  `[workspace.package] version` — un seul nombre partout.
- **D4 (R5)** : noms des destinataires ? — **« Annuaire des
  correspondants »** (résolution locale adresse → nom, repli adresse
  nue).
- **D5 (R5)** : « sur Boîte » en ligne 1 ? — **« Garder la règle D7 »**
  (le bloc n'apparaît qu'à ≥ 2 comptes / vue mélangée).
- **D6 (R5)** : les Cc ? — **« Oui, ligne Cc »** sous la ligne « À : »,
  même forme, seulement si des Cc existent.
