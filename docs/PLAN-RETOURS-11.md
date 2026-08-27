# PLAN-RETOURS-11 — trois retours CE (mémoire d'images, Made in EU, bêta)

> Ouvert le 2026-08-27, GO CE au STOP 1 le jour même (D1-D9, §5).
> **Terrain VALIDÉ le 2026-08-28 en DEUX passes** : première passe —
> R1 « OK », R2 « OK », et DEUX constats bêta posés en route (T1
> bouton Feedback, T2 étape d'accueil bêta — §7), **corrigés dans la
> session** ; seconde passe : « Ok sur les gestes ». La non-réception
> à l'adresse des retours est tranchée HORS Wind (des emails d'un
> autre client n'arrivent pas non plus — alias fcts.io, action CE).
> Revue à regard neuf 8 angles / 22 candidats / 10 retenus / 9
> corrigés avant terrain (dont la purge `reset_mailbox` de la mémoire
> d'images — bug de vie privée, TDD) ; dette D-42. e2e 148 → **150** ;
> glyphe `feedback` neuf (80) ; A89-A91. Trois STOP visuels R2, un
> STOP visuel R1, deux STOP visuels bêta (textes CE mot pour mot).
> Reste : commit, push, CI — puis release **0.12.0** (D6).

## 1. Constat — instruction sur pièces (2026-08-27)

Trois retours du Chef Ingénieur, instruits sur le code, le Système et
l'état du projet.

### R1 — La garde d'images redemande à chaque lecture

Demande : mémoriser le clic « Afficher les images » par message, et
offrir « Toujours afficher les images de cet expéditeur » pour que ses
messages s'affichent sans bandeau.

Faits établis :

- Le bandeau vit dans `Fil.svelte:430-438`
  (`data-testid="garde-images"`, textes `lecture.imagesBloquees` /
  `lecture.afficherImages` au catalogue). Le geste
  (`fil.svelte.js:211-216`) pose `fil.imagesVoulues[k] = true` — un
  état Svelte **volatil**, clé `account_id/mailbox/uid` — puis recharge
  le corps avec `showImages: true`.
- Côté Rust, `message_body` / `echo_body` (`commands.rs:1940-1989`,
  `2547-2567`) traduisent `show_images` en
  `mail_render::ImagePolicy::AllowRemote` / `BlockRemote` ; la CSP de
  l'iframe suit (`mail-render/src/lib.rs:128-129`). Rien n'est écrit
  nulle part : `fermerFil()` / `ouvrirFil()` remettent tout à zéro.
- **Le comportement actuel est un invariant VOULU, pas un oubli** :
  verdict A43 (séance CE 2026-08-16, Système l. 4381) — « l'opt-in
  d'images ne survit pas à la sélection » — et un e2e le verrouille
  explicitement (`refonte-ecran02.spec.js:667-679` : « la garde est DE
  RETOUR »). Ce retour **renverse une décision gelée** ; le plan le dit
  et le Système sera amendé en conséquence (A-n).
- Modèles de persistance en place : la table `pins` (clé d'enveloppe
  `(mailbox_id, uid)`, `store.rs:293-305`) — le patron exact d'un choix
  local par message qui survit à la reconstruction des fils ; la table
  `prefs` clé-valeur. **Aucun stockage par expéditeur n'existe**
  (l'annuaire `correspondants` est un appris, pas une liste de
  confiance).
- L'adresse de l'expéditeur existe en base
  (`envelopes.sender_address`, `envelope.rs:20`) mais n'est **pas**
  transmise à `message_body` ni exposée nue à l'UI (`MessageRow.sender`
  est la chaîne d'affichage).

Architecture retenue à instruire : **l'autorité en Rust**. Deux tables
neuves — `images_messages` (clé d'enveloppe, patron `pins`, `ON DELETE
CASCADE`) et `images_expediteurs` (adresse normalisée en minuscules) —
consultées PAR `message_body` lui-même, qui lit `sender_address` de
l'enveloppe et rend `AllowRemote` si l'une des deux répond. L'UI ne
décide rien : le bandeau s'affiche sur le seul
`remote_images_blocked > 0` retourné. Deux commandes d'écriture
(`allow_images_message`, `allow_images_sender`), appelées par les deux
boutons du bandeau. Coût : deux lectures indexées de plus par corps —
négligeable devant le rendu.

Capacité nouvelle → **MINEUR** (§2.9).

### R2 — « Made in EU » dans À propos

Demande : une mention « Made in EU » avec un petit drapeau de l'Union
européenne dans la fenêtre À propos.

Faits établis :

- « À propos » n'est pas une fenêtre : c'est un groupe des Réglages
  (`Reglages.svelte:812-850`) — marque en tuile 40 px + « Wind »,
  ligne Version (lue du binaire), bloc Mises à jour, ligne Icônes.
  Rangées `.ligne-apropos` (clé 110 px `--muted`, valeur `--ink`).
- Les couleurs figées hors thèmes existent déjà à cet endroit : la
  marque en tuile (`#141414`/`#F2EDE3`/`#1F8A8A`, régime W-D3). Le
  drapeau UE suivra le même régime — couleurs officielles figées
  (champ `#003399`, étoiles `#FFCC00`), un petit SVG inline dédié,
  **hors registre des glyphes** (le registre est monochrome
  `currentColor`, grille 24, gardé par la gate de cohérence — un
  drapeau bicolore n'y a pas sa place ; précédent : `MARQUE` vit déjà
  hors inventaire).
- e2e existant : `refonte-ecran02.spec.js:854-864` (parcours clavier,
  version, « Apache 2.0 », flux MAJ) — à étendre d'une assertion.
- Le Système fige la maquette d'À propos (l. 3771-3795) — amendement
  A-n dans le même commit (DC-D2).

Ajustement de l'existant, aucun impact de version au-delà de R1.

### R3 — Lancer les actions du plan pour la bêta fermée

Demande : engager la bêta fermée ([PLAN.md](PLAN.md) §4, Phase 5 —
« Bêta fermée 20-50 utilisateurs ; le CE dépouille chaque retour »).
ETAT : « rien n'est engagé ».

Faits établis, dans l'ordre de ce qui la conditionne :

- **La chaîne de livraison est prête et prouvée** : dépôt GitHub
  public, releases bi-arch signées minisign, auto-update prouvé aux
  deux postes sur trois versions consécutives (0.9.0 → 0.11.0),
  vérification scriptée 18/18.
- **D-39 (signature Authenticode GELÉE)** : tout testeur dont le poste
  a Smart App Control `On` joue l'installation à la loterie (verdict
  cloud par binaire, prouvé les 26-27/08). C'est LE risque n° 1 de la
  bêta — il ne se lève pas (Trusted Signing fermé hors USA/Canada),
  il se **dit** aux testeurs (guide d'installation) et se **mesure**
  (chaque refus SAC est un retour attendu, et la première MAJ refusée
  prouvera le filet de PLAN-SIGNATURE, preuve encore due).
- **Le plafond Google** : tant que la vérification CASA n'est pas
  faite (dossier côté produit-owner, chemin critique du PUBLIC, pas de
  la bêta), l'app OAuth est limitée — en mode « test », chaque adresse
  Gmail doit être inscrite à la main dans la console (plafond 100) ;
  en « production non vérifiée », écran d'avertissement dissuasif.
  L'état réel de la console n'est lisible que par le CE — à constater
  avant d'inviter quiconque (D8).
- **La télémétrie** est locale et opt-in (ADR 0014) : aucun retour
  automatique — tout passera par le canal de retours (D7).
- Ce qui n'existe pas encore : un guide d'installation testeur (avec
  les contournements SmartScreen/SAC dits honnêtement), un canal de
  retours outillé, une liste de testeurs, la cadence de dépouillement.

Le livrable code de R3 est **documentaire et outillage** (guide,
gabarit de retour) ; le reste est une checklist d'actions CE datées.
La cadence du PLAN (kaizen hebdomadaire sur les frictions observées)
s'incarne dans les chantiers PLAN-RETOURS-n existants — rien de neuf à
inventer là.

## 2. Périmètre

- R1 : mémoire persistante du choix « Afficher les images » par
  message (clé d'enveloppe) + règle par expéditeur (bouton au bandeau,
  rendu automatique sans bandeau), autorité en Rust, révocation selon
  D4.
- R2 : une ligne « Made in EU » avec drapeau UE inline dans
  Réglages > À propos, catalogue FR/EN, Système amendé.
- R3 : PLAN-BETA.md (les actions datées, qui fait quoi), guide
  d'installation testeur versé au dépôt, gabarit de retour selon D7.
- Version cible : **0.12.0, MINEUR** (D6) — R1 est une capacité
  nouvelle ; R2 voyage avec ; R3 ne touche pas le binaire (sauf D5 si
  un lien de retour entre dans l'app — non proposé ici).

## 3. Refus de périmètre (§2.6)

- **Pas de réglage global « toujours afficher toutes les images »** :
  le blocage par défaut est l'invariant de vie privée du produit ; on
  n'offre que des exceptions EXPLICITES, par message ou par
  expéditeur.
- **Pas de règle par domaine** (`*@exemple.com`) : l'adresse exacte
  seulement — un domaine entier est une surface d'abus (newsletters
  qui tournent leurs sous-adresses).
- **Pas de synchronisation du choix entre postes** : mémoire locale en
  base, comme les épingles.
- **Les échos locaux restent hors mémoire** (`echo_body`) : leur clé
  est éphémère par nature, la fenêtre de vie courte.
- **Pas de nouvelle fenêtre À propos** (R2) — la rangée existante des
  Réglages suffit.
- **Le drapeau n'entre pas au registre des glyphes** (R2) — SVG dédié
  hors inventaire, comme la marque.
- **Pas de télémétrie réseau ni de crash reporting distant** (R3) —
  ADR 0014 tient ; les retours passent par le canal D7.
- **Le dossier CASA / lancement public** reste hors périmètre (R3) —
  chemin critique du PUBLIC, côté produit-owner, déjà tracé au PLAN.

## 4. Étapes

Ordre : du plus petit au plus gros, STOP visuel précoce groupé.

- **E1 — R2, Made in EU** : ligne + drapeau SVG inline (couleurs
  figées), catalogue FR/EN, e2e étendu (RED d'abord), maquette du
  Système amendée (même commit). → **STOP visuel CE** (capture de la
  section À propos), groupé avec le premier rendu d'E2.
- **E2 — R1, la mémoire par message** : TDD — tables + commandes Rust
  (tests store), `message_body` consulte et rend `AllowRemote` ;
  le clic « Afficher les images » écrit puis recharge ; l'e2e qui
  verrouille la non-persistance est RÉÉCRIT en son contraire (rouvrir
  le message : le bandeau ne revient PAS), prouvé non-vacant.
- **E3 — R1, la règle par expéditeur** : second bouton au bandeau
  (« Toujours afficher les images de cet expéditeur »), écriture
  normalisée, rendu automatique sans bandeau dès l'ouverture ; e2e :
  un AUTRE message du même expéditeur s'ouvre images affichées, un
  expéditeur tiers garde son bandeau. → **STOP visuel CE** sur le
  bandeau à deux boutons (avec E1).
- **E4 — R1, la révocation selon D4** : la liste des expéditeurs
  autorisés aux Réglages avec retrait (si D4 la retient), e2e.
- **E5 — R3, la bêta** : PLAN-BETA.md (actions datées : constat
  console Google D8, liste de testeurs D9, envoi du guide, cadence de
  dépouillement), guide d'installation testeur (SmartScreen/SAC dits,
  gestes de contournement, comment donner un retour selon D7), gabarit
  de retour. Relecture CE.
- **E6 — Qualité et clôture** : revue à regard neuf sur le diff,
  gate complète, **STOP 2 terrain** (checklist + commandes
  PowerShell), documentation (Système A-n, ETAT, CHANGELOG), commit,
  push, CI en arrière-plan.

## 5. Décisions CE

Posées une à une et tranchées par le CE le **2026-08-27** :

- **D1 — Renverser l'invariant A43 (R1)** : « l'opt-in d'images ne
  survit pas à la sélection » était un verdict CE du 2026-08-16.
  → **« Oui, renverser »** — le clic « Afficher les images » est
  mémorisé, rouvrir le même message ne redemande plus. L'invariant
  qui RESTE : blocage par défaut, exceptions explicites seulement.
  Le Système est amendé (A-n).
- **D2 — La portée de la mémoire par message (R1)** :
  → **« Le message seul »** — clé d'enveloppe, patron `pins` ; chaque
  message garde son propre choix, cohérent avec l'opt-in par message
  d'A43.
- **D3 — La portée de la règle par expéditeur (R1)** :
  → **« Globale au poste »** — une adresse = une personne de
  confiance, quel que soit le compte qui reçoit ; survit au retrait
  d'un compte.
- **D4 — La révocation (R1)** :
  → **« Liste aux Réglages »** — un « toujours » sans porte de sortie
  est un piège ; petite liste (adresse + retirer), livrée dans ce
  chantier (E4).
- **D5 — Le libellé de R2** :
  → **« “Made in EU” tel quel »** — un label, pas une phrase ;
  identique en FR et EN, à côté du drapeau.
- **D6 — La version cible** :
  → **« 0.12.0 MINEUR »** — publication en fin de chantier après
  terrain.
- **D7 — Le canal de retours bêta (R3)** :
  → **« Email dédié »** — le testeur écrit un mail, zéro friction,
  pas de compte GitHub requis ; le CE dépouille chaque retour.
  L'adresse exacte est une action d'E5 (à fournir par le CE).
- **D8 — L'état de la console Google (R3)** :
  → **« Production non vérifiée »** — pas d'inscription préalable des
  testeurs ; le guide assume et explique l'écran « application non
  vérifiée » de Google au premier login.
- **D9 — La première vague (R3)** :
  → **« Petite vague proche (5-10) »** — entourage direct d'abord, la
  friction d'installation se mesure à petite échelle avant d'élargir
  vers 20-50.

## 6. Terrain (2026-08-28) — verdicts et constats, mot pour mot

Première passe : « 1 OK 2 OK » ; deux constats sur le point 3 (bêta) :

- **T1 — bouton Feedback** : « Mettre un bouton "feedback" en haut à
  droite qui ouvre un formulaire avec un champ permettant d'écrire
  son feedback et qui envoie l'email à feedback-wind@fcts.io (qui est
  l'adresse pour recevoir les retours). » → livré : bouton à l'entête
  (glyphe `feedback` neuf), surimpression champ + Annuler/Envoyer
  (Envoyer ABSENT tant que vide), envoi par `queue_send` depuis le
  premier compte, version au sujet, `flush_outbox` immédiat (constat
  de seconde passe : sans lui le retour attendait une synchro).
  Textes de la fenêtre et de l'étape arrêtés par le CE mot pour mot.
- **T2 — étape d'accueil bêta** : « Ajouter une étape juste avant le
  récap qui explique que Wind est en bêta et présente le bouton
  feedback avec ce que ça fait. » → livré : parcours 4 → 5 étapes
  (A75 amendé, A91), étape 4 « Wind est en bêta » avec l'échantillon
  inerte du bouton.

Seconde passe : « Ok sur les gestes. » La non-réception à
`feedback-wind@fcts.io` est prouvée HORS Wind (échec identique depuis
un autre client) — alias fcts.io à régler côté CE, consigné au
PLAN-BETA.

## 7. Reste (dette)

- **D-42 — la mémoire d'images PAR MESSAGE n'a pas de porte de
  sortie** (revue à regard neuf du 2026-08-28, angle altitude) : le
  choix « Afficher les images » d'un message s'écrit en base mais ne
  se liste ni ne se révoque nulle part — la liste D4 des Réglages ne
  couvre que les règles d'expéditeur. Un clic malencontreux sur un
  message suspect recharge son pixel à chaque réouverture. Périmètre
  assumé de ce chantier (D4 n'a tranché que les expéditeurs) ; à
  rouvrir si le terrain ou la bêta le demande.
- Chemins de session sans filet dédié (revue, angle B) : la garde d'un
  ÉCHO local revient à la resélection (hors mémoire par nature), et le
  repli « images de session malgré une écriture en échec » — aucun
  test ne les tient ; consigné, coût d'un filet jugé supérieur au
  risque.
