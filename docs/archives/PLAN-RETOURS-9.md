# PLAN-RETOURS-9 — OAuth transparent, libellé de suppression, noms de comptes

> **CHANTIER SOLDÉ le 2026-08-23 — terrain complet** (6/6, zéro KO au
> STOP 2). GO CE (D1-D5) le 2026-08-23 ; commit `19e39cf` (+ reliquat
> kaizen `fcfaf09`), A77-A78, ADR 0025, CI verte run 32647649916.
> Reste UNE preuve différée, consignée à l'STATE : à la release 0.8.0,
> connexion d'un compte sur le second poste SANS `setx` — elle ferme
> l'arbitrage OAuth par canal.
>
> Chiffres kaizen (session 3418fb1f, unique, 1,2 h) : 11,4 M équiv.
> input (T1), 320 tours / 1 prompt CE, 10 agents (5,5 M — revue 8
> angles + 2 reconnaissances), **2 gates complètes** (1 rouge fmt
> fail-fast 0,4 s + 1 verte 2 min 13 s) + pre-push (W3 ≤ 3 : tenu),
> 0 KO au STOP 2 (garde-fou qualité : tenu).
>
> Ouvert le 2026-08-23. Trois sujets du Chef Ingénieur, dont le
> premier solde l'arbitrage ouvert de [STATE.md](STATE.md) (« un
> bêta-testeur ne fera jamais de `setx` »).

## Constat (instruction sur pièces, 2026-08-23)

### Sujet 1 — identifiants OAuth de l'app distribuée

- Les identifiants entrent dans le processus en UN seul endroit :
  `Authenticator::from_env` (`crates/mail-auth/src/lib.rs:108-127`),
  par `std::env::var` **à l'exécution**. Aucun `env!`/`option_env!`
  nulle part au dépôt (vérifié par balayage exhaustif).
- Le message d'échec — « {VAR} manquante — lancez l'application depuis
  un terminal où la variable est définie » — est produit là
  (`lib.rs:112-113`), remonte en `String` brute par
  `commands.rs:323/382/212` et s'affiche tel quel au guichet
  (`GuichetCompte.svelte:63,94`, clé `erreur.connexion`). C'est le
  message vu au terrain du 2026-08-23 sur le second poste.
- Politique de secret **en données** (`mail-auth::provider`, ADR
  0006) : Google `ClientSecret::Required` (les apps installées Google
  reçoivent un secret, non confidentiel par nature), Microsoft
  `ClientSecret::Forbidden` (client public, PKCE seul). Trois
  variables vivantes : `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`,
  `MICROSOFT_CLIENT_ID`.
- Le build de release est **local** (`scripts/make-release.ps1`,
  deux `cargo tauri build --target`, tout-ou-rien D7 d'ADR 0023) —
  point d'injection naturel. La CI ne buildera rien.
- **Contrainte forte relevée** : le harnais e2e PURGE ces variables
  (`e2e/isolation-oauth.json`, lu par `isolation.mjs` et
  `sonde-gel.py`) et le test
  `refonte-onboarding.spec.js:86-97` s'appuie sur leur ABSENCE pour
  obtenir l'échec rapide « Connexion impossible ». Toute solution qui
  embarquerait des identifiants dans un build **de dev** casserait
  cette isolation.

### Sujet 2 — libellé de la suppression de compte

- La rangée d'un compte (`Reglages.svelte:439-442`) porte le bouton
  de suppression en **icône seule** (glyphe `delete`,
  `aria-label` = `reglages.retirerCompte` : « Retirer {email} »),
  `data-testid="compte-retirer"`.
- Le panneau est mixte : « Ajouter un compte » est icône + texte
  (`person_add`), « Reconnecter » est texte seul, le repère est icône
  seule. La confirmation sous la rangée dit déjà « Retirer » /
  « Annuler » (`action.retirer`).
- **Vocabulaire du produit** : partout « retirer » (le geste enlève le
  compte de Wind ; le serveur garde tout — la confirmation
  `reglages.retirerConfirme` le dit). L'énoncé CE dit « supprimer le
  compte » — conflit de vocabulaire à trancher (D2).

### Sujet 3 — noms personnalisés des comptes

- La table `accounts` (id, email, provider, sent_mailbox, + config
  IMAP générique) **n'a aucun champ nom/label** ; le struct `Account`
  ne porte que id/email/provider.
- Partout dans l'UI, **l'adresse brute est le libellé** : tuiles de
  nav (`Nav.svelte:47`), rangées Réglages (`Reglages.svelte:424`),
  section Signature (`:615`), sélecteur d'expéditeur du composeur
  (`Composition.svelte:1136-1143`), récapitulatif d'onboarding,
  badge repère (title/aria dans `Liste.svelte:772-776`).
- Patron existant pour une donnée par compte : prefs suffixées
  (`repere_icone.{id}`, `signature.{id}`), écrites par
  `set_text_prefs` (transactionnel), et PURGÉES dans `delete_account`
  (`store.rs:1240-1248`) — l'id SQLite se réutilise, un nom hérité
  serait un mensonge d'identité (revue PLAN-RETOURS-8).

## Périmètre — refus explicites

- **Le nom personnalisé ne touche JAMAIS le `From:` des messages
  sortants** : c'est un libellé local d'affichage, pas un display
  name SMTP. (Un chantier « identité d'envoi » serait à part.)
- Pas de keyring/DPAPI pour les client ids : ce ne sont **pas des
  secrets** au sens strict (pratique des clients mûrs — Thunderbird
  les livre dans son binaire) ; le dépôt public reste propre, les
  valeurs n'y entrent pas.
- Pas d'écran de consentement OAuth modifié, pas de rotation
  d'identifiants, pas de config utilisateur pour les ids.
- Pas de maquette d'étude pour les sujets 2-3 (retouches de rangée) :
  le **STOP visuel précoce** de Phase 2 en tient lieu.
- Pas de renommage des dossiers, pas d'avatar de compte.

## Options — sujet 1 (set-based, verdict sur pièces)

| Option | Verdict |
|---|---|
| **A. Identifiants compilés au build de release** via `option_env!` sur des noms DÉDIÉS `WIND_RELEASE_*`, posés par `make-release.ps1` seul ; à l'exécution la variable d'environnement garde la priorité (dev/tests) | **Recommandée.** Zéro geste utilisateur ; le dépôt reste propre (valeurs jamais commises) ; un build dev n'embarque RIEN (les `WIND_RELEASE_*` n'existent que dans le run du script) → l'isolation e2e et le test onboarding survivent tels quels ; cargo suit les env de `option_env!` (rebuild correct). |
| B. Fichier de config à côté de l'exe | Rejetée : un fichier de plus à signer/distribuer, modifiable par l'utilisateur, et l'installeur NSIS devrait le porter — complexité sans gain. |
| C. Statu quo + documentation `setx` | Rejetée : c'est le constat terrain lui-même — un testeur ne le fera pas. |

Résolution proposée (fonction pure, testable) : *runtime env* →
*valeur embarquée* → erreur. En release publique les deux premières
existent ; sur un poste dev, la variable `setx` continue de servir ;
sans rien, le message d'échec est réécrit pour les DEUX lecteurs :
« identifiants OAuth absents de ce binaire — installez une version
officielle de Wind ; en développement, définissez {VAR} » (le premier
remède dit ne parle plus de terminal — livré, revue du 2026-08-23).

## Étapes

- **E1 — sujet 1** : fonction pure de résolution (RED d'abord) dans
  `mail-auth` ; constantes `option_env!("WIND_RELEASE_GOOGLE_CLIENT_ID")`
  etc. ; `make-release.ps1` mappe les valeurs du poste mainteneur
  vers `WIND_RELEASE_*` avant les deux builds, **tout-ou-rien** (pas
  de release sans les trois valeurs) ; `install-workstation.ps1` amendé
  (le `setx` reste un geste de poste dev, dit comme tel). Gate :
  tests mail-auth + e2e onboarding INCHANGÉ vert (preuve que le build
  dev n'embarque rien).
- **E2 — sujet 2** : texte visible à côté du glyphe `delete`
  (`Reglages.svelte:439-442`), clé catalogue fr/en, e2e
  `refonte-retrait-compte` ajusté. DC-D2 : journal **A77**.
- **E3 — sujet 3** : pref `nom_compte.{id}` (RED : `delete_account`
  doit la purger — ajout à la liste `store.rs:1241` ; RED :
  roundtrip) ; commande de lecture/écriture ; édition dans la rangée
  Réglages (patron carte sous la rangée, comme repère/retrait) ;
  affichage aux surfaces arbitrées en D4 ; e2e neuf. STOP visuel
  précoce dès le premier rendu. DC-D2 : journal **A78**.

Puis Phase 3 (revue à regard neuf + `/gate`), STOP 2 terrain,
Phase 4 (docs, STATE, mémoire), Phase 5 (commit, push + CI en fond).

## Livraison (2026-08-23)

- **E1 livré** : `resolve_credential` (runtime prime, embarqué en
  repli, variable vide = absente) + champs `embedded_client_id`/
  `embedded_client_secret` en données sur `Provider`
  (`option_env!("WIND_RELEASE_*")` — Microsoft n'embarque JAMAIS de
  secret, client public) ; test `dev_builds_embed_no_credentials`
  (un build dev/test n'embarque rien — l'isolation e2e garde son
  levier) ; `make-release.ps1` : présence des 3 valeurs vérifiée
  AVANT les builds (tout-ou-rien), posées pour la SEULE durée des
  deux builds, retirées en `finally` ; `install-workstation.ps1` : le
  `setx` requalifié geste de poste dev. TDD : RED montré (6 erreurs)
  → GREEN 21/21 mail-auth.
- **E2 livré** : bouton icône + texte « Retirer le compte »
  (`reglages.retirer`), aria-label « Retirer le compte {email} »
  (WCAG 2.5.3 : le texte visible vit dans le nom accessible). A77.
- **E3 livré** : pref `nom_compte.{id}` (purge au retrait via LA
  constante `PREFS_PAR_COMPTE` de store.rs — la liste en dur
  cross-crate de la purge est morte), `nom_normalise` (60 max,
  refus jamais troncature — pas de `maxlength` UI), commandes
  `noms_get`/`nom_set`, porte = libellé de la rangée (pas de glyphe
  neuf : le jeu n'a pas de crayon, A3 interdit le réemploi),
  surfaces D4 (nav, badge liste, Réglages Comptes ET Signature,
  composeur « Nom — adresse » par `libelleDe` unique). A78.
  TDD : RED montré (purge KO + 11 erreurs + 2 specs e2e rouges)
  → GREEN (22 tests e2e sur 4 specs, Rust vert).

## Revue à regard neuf (2026-08-23, 8 angles)

10 trouvailles confirmées, **toutes corrigées avant le terrain** —
la plus grave : les `WIND_RELEASE_*` posées par `make-release.ps1`
survivaient aux builds et faisaient rougir le pre-push du push final
(cargo suit les `option_env!`) — **la release se serait bloquée
elle-même** ; portée désormais bornée aux deux builds, `finally`.
Les autres : carte fermée par un save tardif d'un autre compte,
Entrée non gardée (`nomOccupe`), ellipsis perdu dans `.identite`,
`maxlength` tronquant en silence (contrat D3), aria-label sans le
texte visible, collision de sélecteur `.nom` (→ `.nom-compte`),
surfaces D4 manquantes (Signature livrée, onboarding consigné sans
objet), message d'erreur promis livré, attente manquante au test 3.
Kaizen au passage : `fermerCartes()` (l'invariant « jamais deux
cartes » en un point), `libelleDe` (le format en un point),
`PREFS_PAR_COMPTE` (la purge en un point).

Consigné sans correction (assumé) : `chargerNoms`/`noms_get`
clonent le patron repères (2e occurrence — factoriser à la 3e) ;
un `Store::open` de plus au démarrage (~qq ms, file sérialisée) —
fusion possible dans un `identites_get` si une 3e table naît ; la
table `$oauth` du script duplique les `option_env!` de provider.rs
(commentaire croisé posé des deux côtés) ; erreurs Rust en français
dans l'UI en (cohérent avec tout le produit).

## § Décisions CE

- **D1 — Voie pour les identifiants de release** : tranchée le
  2026-08-23 — **« Compilés au build »** (option A : `option_env!`
  sur noms dédiés `WIND_RELEASE_*`, posés uniquement par
  `make-release.ps1`, tout-ou-rien ; à l'exécution la variable
  d'environnement garde la priorité).
- **D2 — Vocabulaire du bouton** : tranchée le 2026-08-23 —
  **« Retirer le compte »** (cohérent avec l'existant, honnête —
  rien n'est supprimé du serveur).
- **D3 — Stockage du nom personnalisé** : tranchée le 2026-08-23 —
  **pref `nom_compte.{id}`** (patron repère, zéro migration, purge
  dans `delete_account`).
- **D4 — Surfaces d'affichage du nom** : tranchée le 2026-08-23 —
  **proposition du plan** : le nom REMPLACE l'adresse en nav,
  infobulles/badges et onboarding ; en Réglages le nom s'affiche
  AVEC l'adresse (rangée Comptes ET section Signature — la surface
  où éditer le mauvais compte coûte) ; au composeur « Nom — adresse »
  (l'adresse reste la donnée fonctionnelle d'envoi).
  **Précision consignée à la revue (2026-08-23)** : la surface
  « onboarding » de D4 est **sans objet par construction** — le
  parcours de premier démarrage couvre la fenêtre avant que Réglages
  ne soit accessible, aucun nom ne peut exister à ce stade ; le
  récapitulatif garde l'adresse (repli naturel `nom ?? adresse` si le
  cas naissait un jour). Ce n'est pas un retrait de périmètre : la
  surface est vide.
- **D5 — Version** : tranchée le 2026-08-23 — **MINEUR → 0.8.0**
  (les noms personnalisés sont une capacité nouvelle, §2.9).
