# Spike — Mode organisé (exploration, 2026-08-29)

> Le dossier d'instruction du chantier d'implémentation vit dans
> [`docs/PLAN-MODE-ORGANISE.md`](../../docs/PLAN-MODE-ORGANISE.md) —
> comportements arrêtés, architecture proposée, points durs à spiker,
> décisions CE à trancher au STOP 1.

Planche cliquable **jetable** : Wind 0.13.0 (Système « Elements »
repris verbatim — jetons, glyphes grille 24 / trait 2, coin vif,
deux polarités) + un « Mode organisé » inspiré des six features HEY
fournies par le CE (PDF « Hey Features / Must Have »).

Retours CE du 2026-08-29 (passe 2, sur captures HEY) — appliqués :
- **The Screener = Portier**, **The Feed = Kiosque** (noms arrêtés) ;
- la **Réception organisée n'a PAS de volet de lecture** : fil de
  messages centré, un clic ouvre le message à plat (écran 03,
  retour en tête) ;
- le **Portier est un OUI/NON sur l'expéditeur, rien d'autre** — ni
  tri de destination, ni traitement du message. Un Oui arrive en
  Réception ; le routage fin (Kiosque, Registre) se fait plus tard,
  depuis un message, par « Déplacer vers… ». Le choix de destination
  au guichet et l'option domaine de la passe 1 sont RETIRÉS.

Retours CE (passe 3) — appliqués :
- les boutons du Portier passent **à droite** du message ;
- chaque bouton porte un **mini ⋯ au coin haut-droit** (le patron du
  chevron HEY) : sur **Oui**, il oriente les messages (Réception /
  Kiosque / Registre) ; sur **Non**, il pose la règle (signalés
  indésirables / archivés automatiquement / supprimés
  automatiquement). L'historique dit la règle choisie.

Retours CE (passe 4) — appliqués :
- les rangs du Portier prennent **le format des rangées du volet
  central** (expéditeur/heure, objet, aperçu — mêmes classes),
  l'adresse en clair en plus ;
- les sections de la Réception (« Nouveau pour vous » / « Déjà
  consulté ») prennent **le dessin des sections du Portier**
  (libellé + trait) ;
- un clic sur un message ouvre **l'écran 03 du classique 1-2
  volets** (surimpression plein écran : entête « ← Réception » +
  « Écrire », colonne centrée 960 px, barre du fil au pied ; Échap
  ferme).

| HEY | Planche | Ce qui se clique |
|---|---|---|
| The Screener | **Portier** | premier message d'un inconnu retenu ; Oui → Réception, Non → écarté (jamais prévenu) ; historique avec « Réintégrer » |
| The Imbox | **Réception** centrée, sans volet | « Nouveau pour vous » / « Déjà consulté » — le lu ne gêne plus le non-lu |
| The Feed | **Kiosque** | lettres d'information déjà ouvertes, la plus récente en tête, défilement sans traitement |
| Paper Trail | **Registre** | reçus / confirmations / factures hors du chemin |
| Set Aside | **Mis de côté** | pile en bas à droite de la Réception ; clic = éventail ; « Voir le tableau » ; « Terminé » renvoie le message |
| Bundle Emails | **Grouper** | un expéditeur bavard (GitHub semé) tient en UNE rangée « Groupé · N nouveaux » ; clic = ses nouveaux messages sur une page |

Le **va-et-vient « Organisé »** vit à droite de la barre de recherche
(pilule + disque — les deux seules formes rondes légitimes, V14).
Les gestes par message (⋯ au survol d'une rangée, barre du message) :
Déplacer vers…, Mettre de côté, Grouper, Écarter. Le mode classique
reste l'app d'aujourd'hui (trois volets), intacte.

Retours CE (passe 5) — appliqués : le Registre prend le format des
messages de la Réception (rangées au filet, sans cadre englobant) ;
le glyphe du Portier devient un **majordome** (tête, buste au tracé
de `person`, nœud papillon).

Ouvrir `index.html` dans un navigateur — aucune dépendance, aucune
donnée réelle, AUCUN code de production touché. Cinq glyphes
d'exploration hors catalogue (portier, fil, registre, pile, groupe) :
s'ils survivent, ils entreront au Système par la voie normale
(relevé + gate de cohérence).
