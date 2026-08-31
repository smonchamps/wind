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
  CASA reste le chemin critique du PUBLIC, hors bêta — détaillé au §4.

## 3. Les actions

Cochées au fil de l'eau ; les actions CE sont marquées **[CE]**.

- [x] Guide du testeur versé au dépôt (BETA.md) — 2026-08-27.
- [x] **[CE]** L'adresse des retours (D7) : **feedback-wind@fcts.io**
  — tranchée au terrain du 2026-08-28. Le canal principal est
  désormais DANS l'app : le bouton **Feedback** de l'entête (A91)
  envoie par email depuis le compte du testeur ; l'adresse reste au
  guide comme repli (Wind bloqué à l'installation).
- [x] **[CE]** L'adresse `feedback-wind@fcts.io` **reçoit** —
  constat CE du 2026-08-29 : les mails du 28 sont bien arrivés, le
  défaut était un délai de propagation de la configuration DNS, pas
  une panne d'alias. Le bloquant du 2026-08-28 est levé ; la voie des
  invitations est ouverte.
- [ ] **[CE]** Première vague (D9) : 5-10 proches — les inviter par
  email personnel avec le lien du guide
  (https://github.com/smonchamps/wind/blob/main/docs/BETA.md).
  Viser au moins UN poste Smart App Control `On` et UN compte
  Gmail : les deux risques du §2 doivent être éprouvés tôt.
  **Vague ouverte le 2026-08-31** : registre nominatif au **§3 bis**,
  mot d'invitation modèle au **§3 ter**, guide relu et corrigé le
  jour même (parcours à cinq étapes, § « Organisé »). Reste le
  geste CE : envoyer, puis remplir le registre.
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

## 3 bis. Vague 1 (D9) — le registre des cinq

> Ouvert le 2026-08-31. Cinq proches, pas plus : la vague se dépouille
> à la main. Deux postures sont OBLIGATOIRES dans le lot (§2) — au
> moins **un poste Smart App Control `On`** et **au moins un compte
> Gmail**. Sans elles, la vague n'éprouve aucun des deux risques.

Release en vigueur au lancement de la vague : **0.15.0** (publiée le
2026-08-30, x64 + arm64, `latest.json` en place).

**Le registre est ANONYME, et c'est une règle, pas une pudeur**
(décision CE du 2026-08-31) : le dépôt est PUBLIC. Les noms et les
adresses des cinq restent chez le CE, hors du dépôt ; ici ne vivent
que le pseudonyme de rang, la posture du poste et les faits. Aucun
retour recopié au dépôt ne porte de nom, d'adresse ni de contenu de
message — le fait, la version, la date.

| # | Poste (arch. / SAC) | Compte visé | Invité le | Installé le | Retours |
|---|---------------------|-------------|-----------|-------------|---------|
| T1 | — | — | — | — | — |
| T2 | — | — | — | — | — |
| T3 | — | — | — | — | — |
| T4 | — | — | — | — | — |
| T5 | — | — | — | — | — |

Règles de tenue du registre :

- **La correspondance reste chez le CE.** Le rapprochement T1-T5 ↔
  personnes ne s'écrit nulle part dans le dépôt.
- **Invité le** : la date d'envoi du mot d'invitation (§3 ter).
- **Installé le** : la date où le testeur DIT que Wind s'est lancé —
  pas une supposition. Un silence de plus de trois jours se relance
  une fois, puis se consigne comme tel.
- **Refus d'installation** : ligne au registre de dette **D-39**
  (poste, version, date) — c'est la mesure qui rouvrira le chantier
  signature, elle ne vaut que si elle est écrite.
- **Retours** : chaque retour instruit entre au dépôt par `/terrain`
  (constat du jour) ou `/chantier` (friction de fond) ; la case porte
  le numéro du PLAN qui l'a traité.

## 3 ter. Le mot d'invitation (modèle)

> Envoyé personnellement, un destinataire par mail — jamais en copie
> cachée collective : un testeur invité en lot répond en lot,
> c'est-à-dire jamais.

**Objet** : Wind — tu veux bien l'essayer ?

```
Bonjour <prénom>,

Je te sollicite comme cobaye. Wind est le client email Windows sur
lequel je travaille : rapide, sobre, local — tes messages restent sur
ta machine, aucune télémétrie. Il est en bêta fermée, vous êtes cinq.

Le guide d'installation (5 minutes, tout est dedans) :
https://github.com/smonchamps/wind/blob/main/docs/BETA.md

Deux avertissements t'attendent, et ils sont normaux — le guide les
explique : Windows peut afficher « Windows a protégé votre
ordinateur » (l'installeur n'est pas encore signé), et Google un
écran « application non validée » si tu connectes un Gmail (l'audit
est en cours). Sur certains PC récents, l'installation peut être
refusée SANS recours : si ça t'arrive, dis-le-moi — c'est un retour
précieux, pas une fausse manœuvre, et surtout ne désactive rien.

Il y a dans l'entête une bascule « Organisé » : c'est la nouveauté de
cette version — elle ouvre trois destinations (Réception, Kiosque,
Registre) et un Portier où TU décides, expéditeur par expéditeur.
Lis le §3 du guide avant de t'en servir : les trois destinations ne
déplacent rien chez ton fournisseur, mais le « Non » du Portier, lui,
agit sur ta vraie boîte (par défaut : la corbeille, sur les messages
qui arrivent ensuite). C'est là-dessus que ton avis m'intéresse le
plus.

Ce que j'attends de toi : que tu t'en serves pour de vrai, quelques
jours, et que tu me dises ce qui cloche. Le bouton « Feedback » en
haut à droite envoie directement. Le retour le plus utile tient en
trois lignes : ce que tu faisais, ce que tu attendais, ce qui s'est
passé. Un texte pas clair ou un geste qui te manque comptent autant
qu'un bug.

Merci,
<signature>
```

## 4. Le dossier de vérification Google (chemin du lancement PUBLIC)

> Ajouté le 2026-08-28 sur décision CE. **Rien ici ne bloque la bêta** :
> les 5-10 proches passent par l'app publiée non vérifiée (§2). Ce
> dossier est le chemin du lancement PUBLIC — il entre au PLAN-BETA
> parce que ses délais sont longs et que son premier jalon (un
> domaine) n'est pas posé.

Le formulaire est refusé d'entrée sans **domaine** : le dépôt GitHub ne
peut pas tenir lieu de page d'accueil (Google exclut explicitement les
liens de plateforme). C'est la brique manquante, et tout le reste en
dépend.

- [ ] **[CE]** **Le domaine**, et sa propriété prouvée dans la Google
  Search Console (vérification de marque : 2-3 jours ouvrés annoncés).
  Piste : `fcts.io`, déjà retenu pour l'adresse des retours (§3).
- [ ] **[CE]** **Page d'accueil publique** sur ce domaine : accessible
  sans connexion, manifestement liée à Wind.
- [ ] **Politique de confidentialité** sur le MÊME domaine : comment
  Wind accède, utilise, stocke et partage les données Google, et
  conformité **Limited Use** (pas de publicité, pas de revente, pas
  d'entraînement de modèles, pas de lecture humaine). Wind est en
  position forte — rien ne quitte le poste, aucune télémétrie réseau
  (ADR 0014) — mais la position doit être ÉCRITE, pas déduite.
- [ ] **Écran de consentement exact** : nom, logo, email de support,
  liens — tous cohérents avec le domaine.
- [ ] **Justification scope par scope**, au moindre privilège. Le point
  dur attendu : `https://mail.google.com/` est le scope le plus large,
  et le réviseur demandera pourquoi pas `gmail.readonly` +
  `gmail.send`. La réponse est vérifiable — **XOAUTH2 sur IMAP/SMTP
  n'accepte que celui-là** — mais elle doit figurer au dossier.
- [ ] **Vidéo de démonstration** : YouTube non répertoriée, en anglais,
  montrant le parcours OAuth complet, le nom de l'app à l'écran de
  consentement, le **`client_id` lisible dans la barre d'adresse**, et
  chaque scope restreint à l'œuvre.

**Question ouverte, en amont de tout coût.** L'évaluation de sécurité
CASA (Tier 2, labo agréé App Defense Alliance, ~540 $ relevés pour le
scan DAST, **à refaire tous les 12 mois**) est conditionnée par Google
à un critère précis : elle est due si l'app « accède ou a la capacité
d'accéder aux données utilisateur Google **depuis ou via un serveur** ».
**Wind n'a aucun serveur** — jetons au coffre de l'OS, IMAP/SMTP en
direct poste ↔ Google, aucun backend qui voie passer un message. Si
l'exemption couvre cette architecture, il ne reste que la liste
ci-dessus. La documentation publique ne tranche pas (la page CASA ne
détaille pas ses exceptions ; des sources tierces affirment l'inverse
sans distinguer les clients purs). Le formulaire de vérification pose
la question du serveur, le labo aussi. **À faire trancher AVANT de
payer ou de planifier quoi que ce soit** : c'est ce qui rouvrirait
l'affirmation « long, coûteux » de [PLAN.md](PLAN.md) §2.3 — posée en
Phase 0, jamais vérifiée depuis.

## 5. Ce que la bêta ne fait pas (refus §2.6)

- Pas de télémétrie réseau ni de crash reporting distant (ADR 0014
  tient : local et opt-in). Les retours passent par l'email D7.
- Pas de canal GitHub Issues imposé aux testeurs (D7) — le dépôt
  public reste ouvert à qui préfère, sans en faire une exigence.
- Pas de build « bêta » séparé : les testeurs installent LA release
  courante et vivent l'auto-update réel — c'est lui qu'on éprouve.
