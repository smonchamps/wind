# ADR 0025 — Identifiants OAuth compilés dans le binaire de release

Date : 2026-08-23 · Statut : accepté (PLAN-RETOURS-9, décision CE D1)

## Contexte

Wind lisait `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET`/
`MICROSOFT_CLIENT_ID` **à l'exécution** (`std::env::var`, un seul
point : `Authenticator::from_env`). Constat terrain du 2026-08-23
(second poste x64) : sur un poste sans `setx`, la connexion échoue
avec un message de développeur. Un bêta-testeur ne fera jamais de
`setx` — à régler avant la bêta fermée.

## Options

| Option | Verdict |
|---|---|
| **Compilés au build de release** via `option_env!` sur des noms DÉDIÉS `WIND_RELEASE_*` | **Retenue.** Zéro geste utilisateur ; dépôt public propre (les valeurs n'y entrent jamais) ; un build dev n'embarque rien ; pratique des clients mûrs — les client ids d'apps natives ne sont pas des secrets au sens strict. |
| Fichier de config livré à côté de l'exe | Rejetée : une pièce de plus à signer/distribuer, modifiable, portée par NSIS. |
| Statu quo documenté | Rejetée : c'est le constat lui-même. |

## Décision

- Deux champs **en données** sur `Provider` (`embedded_client_id`,
  `embedded_client_secret`), remplis par
  `option_env!("WIND_RELEASE_{PREFIXE}_...")`. Microsoft n'embarque
  **jamais** de secret (client public, ADR 0006).
- **La variable d'exécution prime** (`resolve_credential` : runtime →
  embarqué → erreur ; une variable vide ne compte pas) — les postes
  dev et l'isolation e2e (purge des variables, `isolation-oauth.json`)
  gardent leur levier.
- Les `WIND_RELEASE_*` ne sont posées QUE par `make-release.ps1`,
  **pour la seule durée des deux builds**, retirées en `finally` —
  la revue à regard neuf a montré que, laissées dans le processus,
  elles faisaient rougir le test `dev_builds_embed_no_credentials`
  dans la gate pre-push du push final : la release se serait bloquée
  elle-même, et le binaire debug de la gate aurait embarqué les
  identifiants. Présence des trois valeurs vérifiée AVANT les builds
  (tout-ou-rien, D7 d'ADR 0023).
- Garde : `dev_builds_embed_no_credentials` (provider.rs) — tout
  build dev/test/CI doit prouver `embedded_* == None`.
- Message d'échec réécrit pour les deux lecteurs : « identifiants
  OAuth absents de ce binaire — installez une version officielle de
  Wind ; en développement, définissez {VAR} ».

## Limites nommées

- La table `$oauth` du script duplique les `option_env!` de
  `provider.rs` (DEBT D-34) : un fournisseur ajouté côté Rust doit
  l'être côté script, sinon sa release part sans identifiant.
- ~~**Preuve terrain différée** : la première release qui suit (0.8.0)
  doit connecter un compte sur un poste SANS `setx` — c'est elle qui
  ferme l'arbitrage.~~ **FAITE le 2026-08-25** : un compte connecté sur
  le second poste depuis une release publiée, sans aucun `setx` —
  l'arbitrage est **clos**. La preuve a glissé de deux versions (elle
  était attendue sur la 0.8.0, elle est venue après la 0.9.0) : la
  décision, elle, tient telle qu'elle a été prise.
