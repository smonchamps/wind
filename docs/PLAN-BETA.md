# PLAN-BETA — la bêta fermée (Phase 5, PLAN §4)

> Ouvert le 2026-08-27 (PLAN-RETOURS-11 R3, décisions D7-D9). But :
> 20-50 utilisateurs réels, chaque retour dépouillé par le CE (genchi
> genbutsu), kaizen hebdomadaire sur les frictions **observées**.
> Gate 5 : deux semaines sans défaut critique → lancement.

## 1. Ce qui est prêt (constaté le 2026-08-27)

- **Chaîne de livraison prouvée** : dépôt public, releases bi-arch
  signées minisign, auto-update prouvé aux deux postes sur trois
  versions consécutives (0.9.0 → 0.11.0), vérification scriptée
  18/18 (`scripts/verifier-release.ps1`).
- **Parcours de premier démarrage** (PLAN-RETOURS-8) : un testeur
  neuf est guidé en quatre étapes.
- **OAuth compilé dans la release** (ADR 0025, prouvé sans `setx`).
- **Échec de mise à jour VISIBLE** (PLAN-SIGNATURE) : plus de
  fermeture silencieuse.
- **Guide du testeur** : [BETA.md](BETA.md) — installation,
  SmartScreen, Smart App Control, écran Google « non validée »,
  comment donner un retour (la forme en trois lignes).

## 2. Les deux risques assumés, et leur traitement

- **D-39 — installeur non signé Authenticode** : sur poste Smart App
  Control `On`, l'installation est une loterie par binaire (prouvé
  les 26-27/08). Traitement : le guide le DIT, chaque refus est un
  retour attendu et compté ; la première MAJ refusée sur poste SAC
  prouvera le filet de PLAN-SIGNATURE (preuve encore due). Le levier
  de fond (signature) reste gelé — validation fermée hors USA/Canada.
- **App Google en production NON VÉRIFIÉE** (constat CE, D8 du
  2026-08-27) : pas d'inscription préalable des testeurs, mais un
  écran dissuasif au premier login Gmail. Traitement : le guide
  l'explique et donne le chemin (« Paramètres avancés »). Le dossier
  CASA reste le chemin critique du PUBLIC, hors bêta (PLAN §7).

## 3. Les actions

Cochées au fil de l'eau ; les actions CE sont marquées **[CE]**.

- [x] Guide du testeur versé au dépôt (BETA.md) — 2026-08-27.
- [x] **[CE]** L'adresse des retours (D7) : **feedback-wind@fcts.io**
  — tranchée au terrain du 2026-08-28. Le canal principal est
  désormais DANS l'app : le bouton **Feedback** de l'entête (A91)
  envoie par email depuis le compte du testeur ; l'adresse reste au
  guide comme repli (Wind bloqué à l'installation).
- [ ] **[CE — BLOQUANT avant toute invitation]** L'adresse
  `feedback-wind@fcts.io` **ne reçoit pas** (constat du 2026-08-28 :
  échec identique depuis un autre client — alias/redirection fcts.io
  à régler, hors Wind). La prouver par un aller simple depuis un
  client tiers, PUIS par le bouton Feedback de Wind.
- [ ] **[CE]** Première vague (D9) : 5-10 proches — les inviter par
  email personnel avec le lien du guide
  (https://github.com/smonchamps/wind/blob/main/docs/BETA.md).
  Viser au moins UN poste Smart App Control `On` et UN compte
  Gmail : les deux risques du §2 doivent être éprouvés tôt.
- [ ] **[CE]** Dépouiller chaque retour ; les frictions confirmées
  entrent au dépôt par `/chantier` ou `/terrain` (le kaizen
  hebdomadaire du PLAN §4 — la mécanique existe déjà, rien de neuf).
- [ ] Élargir vers 20-50 quand la première vague tourne (installation
  éprouvée, retours qui arrivent, pas de défaut critique ouvert).
- [ ] Compter les refus SAC (D-39) : si un testeur est bloqué à
  l'installation, consigner poste/version/date au registre de dette
  D-39 — c'est la mesure qui rouvrira le chantier signature.
- [ ] Gate 5 : deux semaines sans défaut critique → préparer le
  lancement (avec, sur son chemin : CASA, signature).

## 4. Ce que la bêta ne fait pas (refus §2.6)

- Pas de télémétrie réseau ni de crash reporting distant (ADR 0014
  tient : local et opt-in). Les retours passent par l'email D7.
- Pas de canal GitHub Issues imposé aux testeurs (D7) — le dépôt
  public reste ouvert à qui préfère, sans en faire une exigence.
- Pas de build « bêta » séparé : les testeurs installent LA release
  courante et vivent l'auto-update réel — c'est lui qu'on éprouve.
