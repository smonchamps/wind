---
name: gate
description: Rejouer la gate complète de Wind (fmt, build ui-v2, contrastes, cohérence du Système, garde du thread principal, clippy, tests Rust, e2e) et rapporter les faits bruts. Obligatoire avant tout commit — jamais les tests seuls.
---

# /gate — la gate complète, du plus rapide au plus lent

L'ordre est celui du hook `.githooks/pre-push` (échouer tôt), plus la
vérification de cohérence du Système jouée en CI. Exécuter **tout**,
rapporter les faits bruts — chiffres, sorties d'échec — sans adoucir.

**La gate complète s'exécute en UN appel** (fail-fast, verdict chiffré
par étape — jamais les 9 commandes en tours d'outil séparés) :

```
powershell -ExecutionPolicy Bypass -File scripts/gate.ps1
```

Les 9 étapes du script, pour la re-gate partielle (rejouées alors une à
une, seules les étapes concernées) :

```
cargo fmt --all -- --check
(cd apps/desktop/ui-v2 && npm run build)        # zéro avertissement exigé
node e2e/contraste.mjs                          # paires WCAG (A8)
node e2e/coherence-systeme.mjs                  # Système ↔ systeme.css, valeur pour valeur
node e2e/garde-thread-principal.mjs             # aucune commande bloquante sur la pompe (PLAN-GELS)
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets            # --all-targets n'est PAS décoratif (examples/)
cargo test --workspace --doc
(cd e2e && npm test)                            # la vraie fenêtre, CDP WebView2
```

## Règles

- **Un rouge = andon.** On arrête, on corrige. Un warning clippy ou un
  avertissement du build ui-v2 est un rouge.
- **Re-gate partielle après correction** : rejouer l'étape rouge et ce
  que la correction peut impacter — si du Rust a bougé, l'amont aussi
  (fmt, clippy, tests Rust) ; si l'UI a bougé, build ui-v2, contrastes,
  cohérence, e2e. La **gate complète finale avant commit reste due**,
  inchangée — la re-gate partielle ne vaut que pour la boucle de
  correction.
- **Après un sed/remplacement mécanique, toujours rejouer `fmt`** —
  la CI rouge du 2026-08-14 vient de là.
- **E2E rouge en local ≠ régression.** La suite flake sur cette machine
  (profil WebView2, OneDrive, charge). Playwright rejoue une fois de
  lui-même (`retries: 1`) : un test qui sort **« flaky » se consigne au
  verdict de gate**, tel quel — la gate reste verte mais le fait est
  dit. Un rouge franc (deux échecs de suite) : rejouer le **spec en
  fichier entier, en isolation, UNE fois** — jamais la suite complète
  pour trancher un flake ; si le doute persiste, `gh run list` — **la
  CI est la référence**. Le flake connu : le brouillon fantôme
  (documenté au commit 0956c85).
- **La toolchain du gate doit être celle de la CI**
  (`rust-toolchain.toml` + ref épinglée dans `ci.yml`) — STANDARD §7.4.
- Le verdict final est la liste : chaque étape, verte ou rouge, avec
  ses chiffres (nombre de tests, avertissements, paires, valeurs).
- **Jamais d'attente CI au premier plan** : `git push` (le pre-push
  rejoue la gate) et `gh run watch` se lancent en arrière-plan ; la
  session annonce le verdict quand il tombe.
