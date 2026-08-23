# Mode d'emploi — le workflow standardisé de Wind

> Installé le 2026-08-15 (commit `961aab7`, décision CE D1/D2/D3).
> Ce document explique **comment s'en servir** ; la méthode elle-même
> vit dans [STANDARD.md](STANDARD.md) §2 et prime sur tout. Les
> skills sont dans `.claude/skills/`, versionnées au dépôt : les
> amender est un commit comme un autre.

## Vue d'ensemble

Une seule commande porte le flux complet ; trois autres servent les
moments qui reviennent. L'utilisateur est le Chef Ingénieur (*shusa*) :
le workflow s'arrête net aux deux endroits où c'est lui qui tranche.

```
/chantier Bug : …  ou  Feature : …
   │
   ├─ Phase 0  Instruction — reproduire, mesurer, lire (jamais de supposition)
   ├─ Phase 1  Conception — constat, set-based chiffré (agent spike),
   │           prototype si UI, PLAN-XXX.md avec § Décisions CE
   │
   ├─ ⛔ STOP 1  Le CE valide le plan et tranche les décisions, une à une
   │
   ├─ Phase 2  Implémentation TDD, étape par étape (DC-D2 au même commit)
   ├─ Phase 3  /code-review high (une fois), puis /gate — un rouge = andon
   │
   ├─ ⛔ STOP 2  Le CE valide au terrain, sur checklist chiffrée
   │             (un constat → correction le jour même, re-gate, re-terrain)
   │
   ├─ Phase 4  Documentation — journal A-n, PLAN, ADR, ETAT, mémoire
   └─ Phase 5  Commit (sans accents) → push + veille CI en arrière-plan
               → verdict CI annoncé par la session
```

## Quelle commande pour quelle situation

| Situation | Commande | Exemple |
|---|---|---|
| Un défaut à instruire ou une fonctionnalité à livrer | `/chantier` | `/chantier Bug : freeze de 5 s au démarrage en ligne de commande` |
| Un constat fait à l'instant au terrain, périmètre étroit | `/terrain` | `/terrain les traits d'accent réapparaissent après un clic` |
| Vérifier l'état avant un commit, ou après une correction | `/gate` | `/gate` |
| Un chantier terminé, terrain validé, CI verte | `/solde` | `/solde PLAN-SPAM` |

`/terrain` est la voie rapide de la boucle genchi genbutsu — mais si la
racine se révèle profonde ou le périmètre s'élargit, la session bascule
d'elle-même en `/chantier` : la vitesse ne dispense pas de conception.

## Ce que le workflow attend du Chef Ingénieur

Le CE n'a que **quatre gestes** ; tout le reste est porté par la session.

1. **Lancer** : une phrase — `Bug : …` ou `Feature : …`. Pas besoin de
   rappeler la méthode, le TDD, les gates : ils sont dans le standard.
2. **⛔ STOP 1 — arbitrer le plan.** La session présente `PLAN-XXX.md`
   et pose les décisions du § Décisions CE une à une. Répondre, c'est
   tout : les réponses sont consignées au PLAN, mot pour mot, datées.
   Aucun code n'existe avant ce GO.
3. **⛔ STOP 2 — valider au terrain.** La session remet une checklist :
   gestes à jouer sur les vrais comptes, chiffres attendus, budgets à
   re-mesurer. Elle fournit **systématiquement, à ce moment, les
   commandes PowerShell nécessaires à la réalisation du test terrain**
   (lancement de l'app, build, préparation des comptes, mesures) —
   prêtes à copier, une par bloc. Dire ce qui est vu — un constat
   déclenche la correction le jour même, dans la même session.
4. **Fournir les mesures que la session ne peut pas prendre** : rappel
   STANDARD §7.1, elle ne lit pas la base réelle ni le bandeau. Quand
   la Phase 0 a besoin d'un chiffre du terrain, elle le demande et
   attend.

## Les garanties intégrées (plus besoin de les prompter)

Chaque skill embarque les règles payées au fil du projet :

- **TDD strict** — RED montré avant GREEN ; un RED qui n'apprend rien
  est dit, jamais simulé.
- **DC-D2** — tout commit UI amende `docs/design/systeme.dc.html` dans
  le même commit (journal A-n).
- **Gate complète, jamais les tests seuls** — les neuf étapes de
  [/gate](../.claude/skills/gate/SKILL.md), `coherence-systeme`
  comprise, jouées en un appel par `scripts/gate.ps1` ; fmt rejoué
  après tout remplacement mécanique.
- **E2E flaky en local** — un rouge local se contre-vérifie
  (`gh run list`) avant de suspecter une régression : la CI est la
  référence.
- **Commits** — `type: description`, sans accents, corps portant
  chiffres et raisonnement, jamais de Co-Authored-By.
- **CI verte obligatoire** — le chantier n'est clos qu'après
  `gh run watch` vert sur le commit poussé ; push et veille CI se font
  **en arrière-plan**, la session annonce le verdict (jamais d'attente
  au premier plan — kaizen 2026-08-23, ~3,5 h de mur bloqué mesurées
  sur 12 jours).

## Politique de modèles (kaizen 2026-08-23, validée CE — axe M)

La règle tient en deux lignes ; elle préserve aussi le quota Fable
pour ce qui en a besoin.

- **Chantier = Fable 5, invariant.** Conception, remontée à la racine,
  TDD, revue : jamais de conception dure sur un modèle moindre
  (précédent perf-lecture, non prouvé mais suspect).
- **Session mécanique = Sonnet 5.** Docs/ETAT/CHANGELOG, Notion,
  veille CI, release scriptée, consolidation mémoire : le CE ouvre ces
  sessions sur Sonnet 5 (sélecteur de modèle de l'app). Baseline
  mesurée : le mécanique tournait à 100 % au tarif maximal (M1, cible
  ≤ 5 % du coût haut de gamme hors chantiers).

## L'agent `spike` — l'exploration set-based

Quand la conception rencontre un point dur, le départage se fait sur
des chiffres (STANDARD §2.2-2.3) : **un agent `spike` par option**, en
worktree isolé, chacun construisant un prototype jetable dans `spikes/`
et rapportant un protocole et des mesures — jamais un avis. Le poste
principal compare les rapports, le CE tranche. Modèle :
[ADR 0004](adr/0004-moteur-de-recherche-fts5.md).

C'est le **seul** agent custom, à dessein : découper la conception,
l'implémentation ou la documentation en agents séparés perdrait à
chaque transfert le contexte qui fait la qualité des commits. Un seul
fil porte le constat jusqu'à la CI verte.

### Modèle des agents (kaizen 2026-08-23 — axe M)

Baseline mesurée : 100 % des sous-agents tournaient haut de gamme, y
compris le pur balayage. Désormais, au lancement d'un agent (paramètre
`model` de l'outil Agent) :

- **Exploration / reconnaissance** (`Explore`, `Plan`, recherche de
  code) : **Sonnet 5** ; **Haiku** pour du pur balayage (localiser des
  fichiers, inventorier des occurrences).
- **Vérification, revue, `spike`** : inchangés — haut de gamme ou
  modèle de la session ; ce sont les meilleurs détecteurs de défauts
  du workflow (une revue a attrapé la reconstruction d'index FTS5 de
  ~13 Go), on n'y touche pas.

## Amender le workflow

Le standard n'est pas figé — c'est du *standard work* : il s'améliore
par kaizen, sur des faits. Un skill qui frotte à l'usage s'amende par
un commit ordinaire (`chore:`), avec le constat qui motive le
changement dans le corps du message. Ce document s'amende au même
commit que le skill qu'il décrit.
