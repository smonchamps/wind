# Journal des versions

Toutes les modifications notables de Wind sont consignées ici.

Le format s'inspire de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/),
et le projet suit le [versionnage sémantique](https://semver.org/lang/fr/) :
la règle d'incrémentation `MAJEUR.MINEUR.CORRECTIF` propre à Wind (Wind
n'exposant aucune API publique) est fixée dans
[`docs/PASSATION.md`](docs/PASSATION.md) §2.9.

Les paquets signés et leurs notes vivent dans les
[Releases GitHub](https://github.com/smonchamps/wind/releases) ; la mise à
jour est automatique et signée (minisign, ADR 0013).

## [0.17.0] - 2026-09-02

Wind va plus vite là où il comptait ses pas, et dit ce qu'il ne peut
pas faire.

### Ajouté

- **Un menu unique, au clavier.** Les huit menus du produit (gestes
  d'une rangée, cartes du Kiosque, Portier, Nettoyage, Registre, tri,
  Réglages, « Déplacer vers… ») partagent le même dessin et se
  parcourent aux flèches : Entrée joue, Échap ferme et rend le focus.
  Les Réglages ouvrent sur leur premier contrôle.
- **Un message qui n'a pas pu se charger le dit** et se rejoue d'un
  clic (« Réessayer ») — avant, un cadre vide jusqu'à la fermeture.
- **« Répondre » suit `Reply-To`** : les listes et notifications qui
  demandent une autre adresse de réponse sont enfin entendues.
- Un envoi qui échoue cinq fois de suite sur une panne transitoire est
  refusé avec son motif et libère la file ; les suivants partent.

### Modifié

- **Les gestes de masse partent en un seul appel**, tout ou rien : une
  panne au milieu ne laisse rien à moitié fait.
- **« Transférer » ne charge plus d'image distante** dans le composeur ;
  le destinataire reçoit le message entier, images comprises.
- Le Nettoyage de printemps et le Portier répondent en une fraction de
  seconde sur une grande boîte (index couvrant : la liste des groupes
  380 → 67 ms sur 200 000 messages et 5 000 expéditeurs).
- Chaque commande n'ouvre plus la base « à neuf » : le coût d'ouverture
  tombe de 36 ms à moins d'une milliseconde ; l'indexation d'un corps
  lourd pèse un tiers de mémoire en moins.
- Le Kiosque garde ses cartes entre deux relèves (la carte lue ne saute
  plus de section) et ne tient vivantes que celles proches de l'écran.
- **Les barres du fil.** En trois volets, la barre de tri (Archiver,
  Signaler comme spam, Épingler) est collée sous l'entête du fil et
  reste en tête au défilement ; en plein écran, ses gestes vivent dans
  la barre d'entête, alignés sur la colonne des messages ; la barre
  Répondre / Répondre à tous / Transférer / Supprimer flotte en bas de
  chaque message tant qu'il défile.
- La synchro initiale reprend là où elle s'est arrêtée ; les dossiers
  spéciaux se reconnaissent au rôle annoncé par le serveur (« [Google
  Mail] » compris) ; une adresse entre chevrons n'est plus prise pour
  un identifiant de fil ; l'écho d'un envoi ne disparaît plus avant la
  relève des Envoyés.
- Moins d'aller-retours réseau et de sondes au repos : trois champs
  d'en-tête au lieu du bloc entier, lots de corps bornés à 32 Mo, une
  seule `LIST` et une seule `CAPABILITY` par session, une sonde d'état
  au lieu de trois.

### Corrigé

- Une boîte illisible ne passe plus pour « sans dossier Envoyés ».
- Un serveur sans UIDPLUS n'accumule plus de doublons au déplacement.
- Une erreur de lecture d'un corps n'est plus définitive.
- En Réception organisée, la bande « Déjà consulté » ne chevauche plus
  une rangée après vingt rangées (les gabarits de hauteur ignoraient le
  bloc de boîte et le ⋯ du mode organisé).

## [0.16.0] - 2026-09-02

Wind se protège mieux de lui-même : une seule instance, des gestes
qui ne se perdent plus en silence, des attentes qui finissent.

### Ajouté

- **Une seule fenêtre Wind à la fois.** Un second lancement dit
  « Wind est déjà ouvert. » et se retire — plus de doubles
  notifications ni d'envois qui se disputent.
- **Les actions que le serveur refuse se voient.** Quand un
  déplacement ou un marquage est refusé par le serveur (dossier
  disparu, par exemple), Wind le dit dans la ligne d'avis au lieu de
  bloquer en silence tous les gestes suivants de cette boîte ; le
  message reste où il était, et un nouveau geste dessus remplace
  l'ancien.
- **Un journal léger à côté de la base** (`wind.log`, un méga au plus,
  jamais de sujet ni d'adresse) pour comprendre après coup une relève
  ou un envoi.

### Corrigé

- **Une boîte vidée puis regarnie notifie de nouveau** — un vidage
  complet faisait passer la boîte pour « jamais synchronisée », donc
  muette.
- **La veille temps réel ne peut plus geler sans fin** sur un serveur
  ou un réseau qui acquitte sans répondre : elle expire et se
  reconnecte.
- **Un jeton expiré en pleine série d'envois ne fait plus « refuser »
  un message sain** ; les réponses portent la chaîne de références
  complète, pour rester dans la conversation chez le destinataire.
- **L'ajout d'un compte n'attend plus indéfiniment** un consentement
  qui ne vient pas : cinq minutes, puis on peut recommencer.
- **Les images accordées ne se chargent qu'en HTTPS**, sans révéler
  l'origine de la demande ; un clic sur un lien ne bloque plus la
  fenêtre pendant l'ouverture du navigateur.
- Des purges locales rendues atomiques et complètes (plus de reliquats
  de pièces ou d'invitations après une suppression côté serveur).

## [0.15.0] - 2026-08-30

La profondeur d'historique se choisit, et le grand ménage arrive.

### Ajouté

- **La profondeur d'historique se choisit à l'ajout d'un compte.**
  D'un mois à tout l'historique (un an par défaut). La liste, la
  recherche et les entêtes restent complètes sur tout l'historique :
  seuls les corps des messages plus anciens restent au serveur, et
  s'ouvrent à la demande, d'un clic. Le choix se règle ensuite dans
  Réglages > Comptes ; les comptes déjà en place gardent tout.
- **Le Nettoyage de printemps** (mode organisé). Une page pour faire
  le ménage par expéditeur : une plage (de trois mois à tout) et un
  périmètre choisis, chaque groupe se tranche d'un geste — le verdict
  s'applique au courrier déjà là ET aux prochains messages, dans le
  vocabulaire du Portier. Jamais de suppression définitive : la
  corbeille reste rattrapable. La session se reprend là où on l'a
  laissée, une jauge suit l'avancement.
- **Le clic du Portier suit des défauts réglables.** Oui envoie en
  Réception, Non à la corbeille — modifiables dans
  Réglages > Portier, dans les deux modes.
- **Le Kiosque se souvient de ce qui est lu.** Les nouveautés se
  présentent dépliées, le déjà-lu se replie en piles par expéditeur ;
  une coche marque tout lu. Son glyphe est redessiné.

### Modifié

- **Douze retours sur le Mode organisé** : la boîte principale se
  nomme « Réception » en mode organisé, et on y revient après le
  Portier ; l'entête du Portier est revue (texte d'accueil,
  historique) ; les entêtes du Portier et du Kiosque partagent le
  même format ; « Sombre — Automatique » passe en tête des Thèmes ;
  le rail des Réglages est aligné ; le Registre dit « Déplacés
  automatiquement dans la corbeille ».

## [0.14.0] - 2026-08-30

Le Mode organisé : Wind trie votre courrier — vous gardez la main.

### Ajouté

- **Le Mode organisé, sur invitation.** Un réglage, réversible à tout
  moment : Wind range alors le courrier en trois lieux — la
  **Réception** pour les personnes, le **Kiosque** pour les lettres
  d'information, le **Registre** pour les notifications et reçus.
  « Déplacer vers » corrige un rangement d'un geste, et Wind s'en
  souvient pour la suite. Rien n'est jamais perdu : tout reste dans
  vos dossiers, seule la présentation change.
- **Le Portier.** Les messages d'expéditeurs inconnus attendent à la
  porte au lieu d'encombrer la Réception. Une page simple — Oui ou
  Non — décide pour chacun ; l'historique garde trace de chaque
  verdict et permet de se raviser.
- **Le Non agit pour de bon.** Dire Non au Portier peut aussi marquer
  comme indésirable, archiver ou mettre à la corbeille les prochains
  messages de l'expéditeur — jamais de suppression définitive, et
  tout s'éteint si le mode est désactivé.
- **La Réception à sections.** « Nouveau pour vous » puis « Déjà
  consulté » : ce qui attend votre regard se détache de ce qui est lu.
- **Mis de côté.** Une conversation à garder sous la main rejoint une
  pile discrète en tête de Réception, d'un geste, et en repart de même.
- **Le Kiosque en cartes déjà ouvertes.** Les lettres d'information se
  lisent sur place, corps entier, en défilant — sans un clic. Chaque
  carte garde la garde d'images et ses gestes : replier, mettre de
  côté, déplacer, écarter.
- **Le thème Innamoramento.** Le thème Mona devient Innamoramento,
  en clair et en nuit — accent grenat, contrastes vérifiés.

## [0.13.0] - 2026-08-29

L'entête des messages dit tout, et les petits accrocs s'effacent.

### Ajouté

- **L'entête d'un message dit qui, à qui, en toutes lettres.** En tête
  de chaque message ouvert : l'expéditeur avec son adresse
  (« Camille Rousseau <c.rousseau@…> »), puis « À : » avec chaque
  destinataire — et « Cc : » quand il y en a. Wind retrouve les noms
  des destinataires grâce aux correspondants qu'il connaît déjà.

### Corrigé

- **Un compte ajouté pendant que Wind est ouvert se dit connecté.**
  Réglages > Comptes le marquait « Déconnecté » jusqu'au prochain
  démarrage, alors qu'il venait d'être connecté.
- **Le logo Wind en haut à gauche est plus présent** (28 px au lieu
  de 24).

## [0.12.0] - 2026-08-28

Wind se souvient de vos choix d'images, et la bêta s'outille.

### Ajouté

- **Wind retient votre choix d'afficher les images.** Cliquer
  « Afficher les images » sur un message vaut désormais pour de bon :
  rouvrir ce message ne redemande plus. Et un nouveau bouton,
  « Toujours afficher les images de cet expéditeur », affiche
  d'office les images de tous ses messages, sans bandeau. La liste de
  ces expéditeurs se consulte et se retire dans
  Réglages > Affichage. Les images distantes des autres messages
  restent bloquées par défaut, comme avant.
- **Un bouton Feedback en haut à droite.** Écrivez ce qui cloche ou
  ce qui manque : votre message part par email depuis votre compte,
  avec la version de Wind — nous lisons tous les retours. Le parcours
  de premier démarrage gagne une étape qui le présente.
- **« Made in EU »** avec le drapeau de l'Union européenne, dans
  Réglages > À propos.

## [0.11.0] - 2026-08-27

La sélection multiple arrive dans la liste, et la marque Wind s'affirme.

### Ajouté

- **Sélectionner plusieurs conversations d'un coup.** Ctrl-clic ajoute
  une conversation à la sélection, Maj-clic étend la plage depuis la
  conversation choisie, et une case à cocher apparaît au survol de
  chaque rangée. Dès qu'une sélection existe, la barre de la liste se
  transforme : marquer lu ou non lu, archiver, signaler indésirable
  (ou « Ce n'est pas un spam » dans le dossier Indésirables),
  supprimer, annuler — un seul message de confirmation pour tout le
  lot, et les raccourcis `e` / `Suppr` s'appliquent au lot coché.
  Un geste de masse emporte chaque conversation **entière** — tous les
  messages de ses fils, pas seulement le dernier.

### Modifié

- **L'icône de l'application est désormais la marque actuelle de
  Wind** (l'enveloppe au rabat) — dans la barre des tâches, sur
  l'exécutable et à l'installation.
- **La marque en haut à gauche de la fenêtre est plus présente**
  (24 px au lieu de 20).
- **Les icônes du volet de gauche s'alignent mieux avec leurs
  libellés** — un calage optique choisi sur planche.

## [0.10.2] - 2026-08-27

Un échec de mise à jour se voit, au lieu de fermer l'application.

### Corrigé

- **Cliquer « Installer » ne peut plus fermer Wind sans rien faire.**
  Quand Windows refusait de lancer l'installateur téléchargé (c'est le
  cas sur les PC où le *contrôle intelligent des applications* est
  actif), l'application se fermait sans un mot et rien ne s'installait.
  Désormais l'échec s'affiche dans le bandeau, avec sa raison, et le
  bouton « Installer » reste là pour réessayer. Sur ces PC,
  l'installation reste bloquée par Windows tant que Wind n'est pas
  signé d'un certificat d'éditeur — c'est le chantier suivant ; en
  attendant, au moins, Wind vous le dit.
- **Une mise à jour qui n'avance plus finit par le dire.** Le
  téléchargement n'avait aucune limite de temps : une connexion qui
  calait laissait le bandeau sur « Installation… » pour toujours. Au
  bout de dix minutes, l'échec s'affiche et se retente.
- **Wind installe exactement la version que le bandeau annonce** —
  jamais une autre publiée entre-temps.

## [0.10.1] - 2026-08-26

Le démarrage cesse de se figer.

### Corrigé

- **Wind ne se fige plus quelques secondes après son ouverture.** Sur
  une grande boîte, l'application s'arrêtait de répondre environ trois
  secondes après le lancement, et pendant près de **neuf secondes** :
  impossible de faire défiler la liste, d'ouvrir un message ou de
  changer de dossier. La fenêtre, elle, bougeait toujours — ce qui
  rendait la chose d'autant plus déroutante. C'est fini. Mesuré sur une
  boîte de 251 000 messages : **8,9 secondes d'attente sont devenues un
  dixième de seconde**.
- **La liste s'affiche trois fois plus tôt.** Elle réclamait sa première
  page en douzième position, derrière toutes les vérifications du
  démarrage ; elle passe devant. De l'ouverture de la fenêtre aux
  messages à l'écran : **1,2 seconde avant, 0,4 aujourd'hui**.

**Une seule fois, à cette mise à jour** : le premier lancement prendra
environ deux secondes de plus que d'habitude. Wind réorganise un index
de sa base — c'est ce qui rend tout ce qui précède possible. Les
lancements suivants sont immédiats.

## [0.10.0] - 2026-08-25

La liste dit d'où vient chaque message, et vous choisissez son air.

### Ajouté

- **Chaque message dit sur quelle boîte il est arrivé** : quand vos
  comptes se mélangent — « Toutes les boîtes », ou une recherche —, la
  ligne l'écrit en toutes lettres derrière le nom de l'expéditeur,
  « Camille Roux sur Travail », avec le repère du compte à sa couleur.
  Plus besoin de se souvenir d'une couleur ou d'un logo, et un compte
  sans repère dit sa boîte comme les autres. La mention se retrouve à
  l'identique sur les messages ouverts ; là où le compte est déjà
  connu — la vue d'une seule boîte, ou un seul compte configuré —,
  elle ne dit rien et disparaît.
- **Trois niveaux d'espacement pour la liste** (Réglages > Affichage) :
  « Faible » — ce que vous aviez jusqu'ici, au pixel près —, « Moyen »
  et « Élevé ». Plus d'air entre les messages si vous préférez respirer,
  autant qu'avant si vous préférez en voir beaucoup. Le changement
  s'applique à l'instant, et la liste reste là où vous l'aviez laissée :
  le message que vous regardiez ne bouge pas de l'écran.

### Modifié

- **La vignette aux initiales quitte la liste** : le nom de
  l'expéditeur, écrit juste au-dessus, disait déjà ce qu'elle disait —
  la place revient au message. Elle reste là où elle travaille : sur
  les messages d'une conversation, et au dossier Brouillons.
- **Le repère d'un compte se dessine au trait** dans la navigation, à
  la place de la pastille pleine : la même marque exactement que celle
  de la ligne. La pastille reste dans les Réglages, où elle sert à
  choisir.

## [0.9.0] - 2026-08-24

Wind change de peau : la direction « Elements ».

### Modifié

- **Un nouveau visage, dessiné d'une seule main** : coins vifs
  partout, un jeu d'icônes original dessiné pour Wind (plus aucune
  police d'icônes embarquée), une nouvelle marque, et le disque teal
  qui dit d'un seul geste ce qui est non lu et ce qui travaille.
- **Deux thèmes au lieu de vingt-huit** : « Elements » (clair) et
  « Elements · nuit », composés et mesurés ensemble. Votre ancien
  choix migre tout seul — un thème sombre reste sombre. Le suivi
  clair/sombre de Windows fonctionne comme avant.
- **La liste dit le non-lu au disque** : un point teal devant
  l'expéditeur, en plus du gras — et le compteur de la navigation
  devient un nombre discret.

### Retiré

- Les vingt-six thèmes Wada et le trait calligraphique de la marque.

## [0.8.0] - 2026-08-23

Nommez vos comptes, et connectez-vous sans aucune configuration.

### Ajouté

- **Donnez un nom à vos comptes** : dans Réglages > Comptes, cliquez
  l'adresse d'un compte pour lui donner un nom (« Boulot »,
  « Perso »…). Le nom s'affiche dans la navigation, dans les
  infobulles de la boîte unifiée et dans le sélecteur d'expéditeur du
  composeur — l'adresse reste visible dans les réglages, et vos
  messages partent toujours avec votre adresse, jamais le nom.

### Amélioré

- **La connexion des comptes ne demande plus aucune configuration** :
  les versions installées de Wind portent désormais tout ce qu'il
  faut pour se connecter à Google et Microsoft — plus rien à définir
  sur le poste.
- **Le bouton de retrait d'un compte dit son nom** : « Retirer le
  compte », en toutes lettres à côté de l'icône. Retirer un compte
  ne supprime rien sur le serveur, comme avant.

## [0.7.0] - 2026-08-23

Répondez aux invitations de réunion sans quitter votre boîte mail.

### Ajouté

- **Les invitations de réunion se traitent dans Wind** : une invitation
  reçue (Google Agenda, Outlook, etc.) s'affiche en carte lisible dans
  la conversation : titre, date et heure dans votre fuseau,
  organisateur, lieu, récurrence. Trois boutons : Accepter, Provisoire,
  Refuser ; votre réponse part par email à l'organisateur, comme le
  font les autres clients mail. Vous pouvez changer d'avis : la
  dernière réponse envoyée fait foi, et la ligne de la liste porte une
  puce « Acceptée », « Provisoire » ou « Refusée ».
- **Répondre depuis la liste** : les trois boutons apparaissent
  directement sur la ligne de la conversation dans le volet central,
  sans ouvrir le message.
- **Une réunion annulée se voit** : l'avis d'annulation marque
  l'invitation d'origine « Annulée », même s'il arrive dans une autre
  conversation ; les boutons de réponse se retirent.
- **Les invitations déjà reçues gagnent leur carte** : au premier
  lancement de cette version, Wind repasse une fois sur le courrier
  existant pour reconnaître les invitations arrivées avant.

### Modifié

- **Le bouton « Supprimer » vit désormais dans chaque message** de la
  conversation, à côté de Répondre et Transférer : on supprime CE
  message ; la conversation reste ouverte s'il en reste.

## [0.6.0] - 2026-08-22

Un repère par boîte mail, un premier démarrage guidé, et Wind existe
désormais en deux versions Windows, arm64 et x64.

### Ajouté

- **Un repère pour chaque boîte** : dans Réglages > Comptes, donnez à
  chaque adresse une icône et une couleur. Le repère s'affiche dans le
  panneau de navigation à la place de l'icône générique, et, en mode
  « Toutes les boîtes » comme dans les résultats de recherche, en
  badge sous l'initiale de chaque message : on voit d'un coup d'œil
  sur quel compte un message est arrivé ou parti. Douze icônes, douze
  couleurs, toutes lisibles sur les 28 thèmes, clairs comme sombres.
- **Un premier démarrage guidé, en quatre étapes** : ajoutez vos
  adresses, choisissez votre disposition de fenêtre sur de vrais
  aperçus, choisissez votre thème, vérifiez vos choix. Chaque étape
  s'applique immédiatement, et chaque choix reste modifiable ensuite
  dans les Réglages. Un parcours interrompu reprend au prochain
  lancement ; les installations existantes ne le voient jamais.
- **Wind pour Windows x64** : chaque version est désormais publiée en
  deux éditions : arm64 (natif Snapdragon) et x64 (PC Intel/AMD),
  avec la même mise à jour automatique signée.

## [0.5.0] - 2026-08-21

Épinglez vos conversations, et une lecture plus claire des pièces
jointes comme des conversations ouvertes.

### Ajouté

- **Épingler une conversation** : dans la Boîte de réception, un bouton
  « Épingler » dans la barre de la conversation la garde **toujours en
  haut de la liste**, marquée « Épinglé » et à la teinte de la boîte
  sélectionnée. Elle quitte sa place dans le fil des dates (jamais en
  double). « Désépingler » l'y ramène. Vos épingles survivent au
  redémarrage ; elles restent sur votre machine.

### Modifié

- **Les pièces jointes s'affichent en haut du message**, juste sous son
  entête : plus besoin de dérouler tout le mail pour les trouver.
- **Survoler une pièce jointe dit l'action** : la puce se couvre d'un
  voile « Enregistrer » avec sa flèche de téléchargement : vous savez
  ce qui se passera avant de cliquer.
- **La conversation ouverte (« Ouvrir ») est désormais à plat**, comme
  le volet de lecture : chaque message dans sa carte, la page défile
  d'un seul tenant, dans une colonne de lecture confortable : plus de
  cadre englobant.

## [0.4.0] - 2026-08-21

Votre signature, l'envoi à l'heure choisie, et le marquage important.

### Ajouté

- **Signature par compte** (Réglages > Signature) : rédigez une
  signature (mise en forme comprise) pour chaque compte ; elle
  s'ajoute d'elle-même au bas de vos nouveaux messages, et, si vous
  l'activez, à vos réponses et transferts (entre votre texte et le
  message cité). « Appliquer à tous les comptes » copie la signature
  et ce choix partout d'un geste. Dans le composeur, changer de compte
  émetteur recharge la signature correspondante tant que vous n'avez
  rien écrit : votre texte n'est jamais réécrit.
- **Envoyer plus tard** : à côté d'« Envoyer », choisissez une date et
  une heure : le message part à ce moment-là **si Wind est ouvert**
  (sinon, au prochain lancement ; Wind vous le dit en le programmant).
  La barre d'état affiche le départ prévu, et un bandeau permet
  d'**annuler** à tout moment : le message revient alors dans vos
  brouillons, pièces jointes comprises : rien n'est perdu.
- **Marquer un message comme important** : un bouton « ! » dans la
  barre de mise en forme du composeur. Le message part avec les
  en-têtes de priorité standard (Outlook et Thunderbird affichent le
  « ! » chez le destinataire ; Gmail web les ignore à l'affichage :
  c'est son comportement, l'en-tête est bien présent). Le marquage
  suit vos brouillons.

### Modifié

- **L'entête de la fenêtre « Nouveau message »** porte désormais la
  même couleur que le pied de page de Wind : la carte s'encadre
  haut et bas dans la même teinte.

## [0.3.0] - 2026-08-21

Les adresses se complètent toutes seules, et l'envoi en cours dit vrai.

### Ajouté

- **Autocomplétion des adresses** dans les champs À, Cc et Cci : dès
  deux lettres tapées, Wind suggère les adresses qu'il connaît, les
  gens qui vous écrivent (avec leur nom) et ceux à qui vous avez
  écrit, classées des plus récentes et fréquentes aux autres. Flèches
  puis Entrée, ou un clic, insèrent l'adresse ; Échap ferme le menu.
  Les expéditeurs de vos Indésirables et de la Corbeille ne sont
  jamais proposés.

### Corrigé

- **L'envoi en attente de synchronisation s'affiche correctement**
  dans « Envoyés » : juste après un envoi, l'entrée temporaire (qui
  patiente le temps que votre messagerie range sa copie) disait
  « À : envoyes » et montrait une section « Fichiers joints » vide.
  Elle dit désormais le vrai destinataire et le nom et le poids de
  chaque pièce envoyée (non téléchargeables pendant cette courte
  attente : la vraie copie prend le relais quelques instants plus
  tard).

## [0.2.1] - 2026-08-20

La liste ne se fige plus, et tout s'affiche plus vite.

### Corrigé

- **Défiler vite à la barre ne bloque plus l'application** : tirer
  rapidement la barre de défilement d'un grand dossier (Archives, par
  exemple) laissait la liste en blocs « … », puis affichait à tort
  « Aucun message ici. » dans tous les dossiers pendant plusieurs
  minutes. La liste ne demande plus que ce qu'elle montre : à l'arrêt
  du geste, les messages apparaissent aussitôt, et changer de dossier
  répond immédiatement.
- **L'écran vide ne ment plus** : « Aucun message ici. » ne s'affiche
  qu'une fois la boîte réellement consultée ; pendant le chargement,
  des lignes d'attente le disent honnêtement.

### Modifié

- **Le démarrage et le premier affichage d'un dossier sont
  immédiats** : les comptages internes (dont le plus coûteux, celui des
  Archives Gmail) ne retardent plus l'affichage des messages : le
  nombre d'éléments et la barre de défilement s'ajustent juste après,
  d'eux-mêmes.

## [0.2.0] - 2026-08-20

La mise en forme arrive dans le composeur.

### Ajouté

- **Une vraie barre de mise en forme** dans la fenêtre de composition :
  police (sans serif, serif, monospace), taille (quatre crans), gras,
  italique, souligné, barré, couleur du texte (nuancier de douze
  teintes), alignement gauche/centre/droite, listes à puces et
  numérotée, retrait, et « Effacer la mise en forme ». Les raccourcis
  Ctrl+B/I/U fonctionnent aussi.
- Vos messages partent désormais **en HTML avec un repli texte
  automatique** : les destinataires voient votre mise en forme, et les
  clients texte reçoivent toujours une version lisible.
- **La citation d'une réponse** apparaît dans un bloc au filet gauche,
  comme dans les clients mûrs ; votre réponse s'écrit au-dessus.
- **Les brouillons conservent la mise en forme**, y compris après un
  aller-retour avec le dossier Brouillons de votre messagerie.
- **Reconnecter un compte** : quand la connexion d'un compte expire ou
  est révoquée, Réglages > Comptes le signale (« Déconnecté ») et un
  bouton « Reconnecter » relance l'autorisation dans le navigateur,
  sans rien perdre ni re-synchroniser. L'avis d'accueil mène
  directement à cette page.

### Modifié

- **Les images distantes, selon le geste** : la citation d'une réponse
  les remplace par un pixel neutre (aucun traceur du message cité ne se
  charge à votre insu) ; un transfert, lui, transmet le message entier,
  images comprises.

## [0.1.11] - 2026-08-19

Trois retours du terrain.

### Modifié

- **Enregistrer une pièce jointe** : cliquer une pièce ouvre désormais une
  fenêtre « Enregistrer sous » où vous choisissez le dossier et le nom du
  fichier, au lieu d'un enregistrement silencieux dans Téléchargements.
- Le **nom et le poids d'une pièce jointe** sont réunis dans une seule
  puce, plus lisible d'un coup d'œil.

### Corrigé

- Sur les **thèmes sombres**, le corps des messages s'affiche désormais sur
  fond clair : le texte des courriels (souvent composé pour un fond blanc,
  comme les infolettres) redevient lisible, au lieu d'apparaître parfois en
  noir sur fond sombre.

## [0.1.10] - 2026-08-18

Quatre retours du terrain.

### Ajouté

- **Signaler un courrier comme indésirable**, et l'inverse : un bouton
  « Signaler comme spam » déplace la conversation vers le dossier
  indésirable de votre messagerie : c'est elle qui apprend. Depuis le
  dossier Indésirables, « Ce n'est pas un spam » la ramène en Réception.
- **Supprimer un brouillon** directement depuis la fenêtre de composition,
  d'un seul geste et après confirmation (distinct de « Annuler », qui,
  lui, conserve le brouillon).
- **Répondre message par message** : les boutons Répondre, Répondre à tous
  et Transférer sont désormais au bas de chaque message d'une conversation,
  pour répondre précisément à celui que vous lisez, vos propres messages
  compris, auquel cas la réponse repart vers les destinataires d'origine.

### Modifié

- Le **rattrapage des messages** affiche un pourcentage de progression dans
  la barre d'état, à côté du nombre de messages restants.

## [0.1.9] - 2026-08-17

Quatre retours du terrain.

### Ajouté

- **Cc et Cci** dans le composeur : ajoutez des destinataires en copie et
  en copie cachée. La copie cachée reste cachée (elle ne paraît jamais
  dans le message reçu par les autres) ; « Répondre à tous » replace les
  Cc d'origine en Cc.

### Modifié

- La **synchronisation Gmail est bien plus légère** : le balayage complet
  des dossiers, qui pouvait durer et se répétait toutes les 5 minutes,
  passe à toutes les 30 minutes. L'arrivée du nouveau courrier, elle,
  reste **instantanée** : rien ne change à ce que vous recevez, seulement
  au poids de fond.
- L'**animation de chargement** (le trait) est simplifiée : une animation
  complète et fluide dès qu'une action est en cours, au lieu d'un trait
  qui pouvait rester figé.

### Retiré

- Le bouton « Rendre indépendante » du composeur, qui ne faisait rien, est
  retiré (la fenêtre de composition détachée reviendra plus tard).

## [0.1.8] - 2026-08-16

Quatre correctifs sur le courrier réel, remontés au terrain.

### Corrigé

- Les objets et les noms d'expéditeur ne montrent plus les antislashs
  parasites des chaînes IMAP entre guillemets (ex. `Test \"Envoyés\"`) ;
  les messages déjà synchronisés sont réparés au premier lancement.
- Le dossier « Envoyés » affiche enfin le vrai destinataire (« À : … »),
  dans la liste comme à la lecture, au lieu de répéter votre propre
  adresse ; l'information est rattrapée sur les envois déjà synchronisés.
- « Répondre à tous » pré-remplit « À » instantanément, à partir des
  destinataires stockés, sans plus attendre une relève du serveur à
  chaque clic.
- L'objet ne s'affiche plus en double en tête du corps de certaines
  infolettres (le titre de leur en-tête HTML ne fuit plus dans le
  message).

## [0.1.7] - 2026-08-16

La refonte entière au poste : le Système v2 « Wada » et son élargissement,
l'UI v3 et ses retours CE, sur une fenêtre qui ne gèle plus.

### Ajouté

- Trois modes d'affichage au choix — trois volets (défaut inchangé), deux
  volets, ou un volet avec tiroir de navigation (PLAN-VOLETS).
- Système visuel v2 « Wada » : palette remappée à teinte d'usage
  constante, le trait hitofude comme signature et seul indicateur de
  progression, nav et liste aux dessins des pistes, 119 jetons
  (PLAN-WADA).
- 28 thèmes et sombre automatique par déclinaison `-nuit`
  (PLAN-WADA-ELARGI).
- UI v3 : bandeau de liste, avatars, le volet de lecture devient le fil ;
  volets réglables à la souris, barres natives (PLAN-UI-V3,
  PLAN-RETOURS-V3).

### Modifié

- Volet de lecture au dessin exact de la maquette Classique ; bascule
  Déplier/Replier dérivée de l'état, hauteur du corps au contenu, entête
  de composition allégé, libellés « Tout » (retours CE A44-A47).

### Retiré

- L'interface v1 : la refonte est terminée (PLAN-RETRAIT-V1).

### Corrigé

- La fenêtre ne gèle plus : aucune commande bloquante sur le thread
  principal, jamais de CPU dans la fenêtre du verrou d'écriture,
  `busy_timeout` porté à 30 s (PLAN-GELS, ADR 0019).
- Un lien du corps ouvre le navigateur système et le corps ne bouge
  jamais ; l'iframe reste inerte (A37, invariant S1).
- La langue se lit sans adopter la base ; la modale de migration reste la
  première surface à payer l'adoption (ADR 0012).
- Deux suites e2e simultanées ne se marchent plus dessus : port CDP libre
  par suite, balayage borné au worktree (PLAN-ISOLATION-E2E).

## [0.1.6] - 2026-08-14

### Corrigé

- Réactivité de l'affichage (PLAN-REACTIVITE), validée au terrain : plus
  de lignes d'attente pendant une synchronisation ; suppression,
  archivage et envoi visibles dans leur dossier en moins d'une seconde,
  hors ligne compris (écho local) ; l'aperçu arrive avec la ligne, en un
  seul affichage.

## [0.1.5] - 2026-08-14

### Corrigé

- Icônes des avis rares (dont le bandeau de mise à jour) : police portée à
  43 glyphes.
- La copie Envoyés se relève sitôt l'envoi accepté (`sync_sent`).

## [0.1.4] - 2026-08-14

### Ajouté

- Pièces jointes : envoi et transfert réel.

### Corrigé

- Affichage des pièces jointes à la première ouverture (constat terrain du
  2026-08-14).

### Sécurité

- Première mise à jour signée sous la nouvelle clé (rotation de la clé de
  signature du 2026-08-14).

## [0.1.3] - 2026-08-14

### Modifié

- Discovery devient **Wind** (PLAN-WIND) — la base se déménage
  automatiquement au premier lancement.
- Canal arm64 natif.

### Sécurité

- Rotation de la clé de signature : installation manuelle requise depuis
  discovery 0.1.2 ; la chaîne d'auto-update reprend ensuite.

## [0.1.2] - 2026-07-26

### Corrigé

- `latest.json` corrigé : BOM retiré et URL au tag nu — l'auto-update
  aboutit (ADR 0013).

## [0.1.1] - 2026-07-26

### Ajouté

- Première version publiée (discovery) : installeur NSIS et mise à jour
  signée minisign, pilotée depuis Rust (ADR 0013).

[0.1.11]: https://github.com/smonchamps/wind/releases/tag/0.1.11
[0.1.10]: https://github.com/smonchamps/wind/releases/tag/0.1.10
[0.1.9]: https://github.com/smonchamps/wind/releases/tag/0.1.9
[0.1.8]: https://github.com/smonchamps/wind/releases/tag/0.1.8
[0.1.7]: https://github.com/smonchamps/wind/releases/tag/0.1.7
[0.1.6]: https://github.com/smonchamps/wind/releases/tag/0.1.6
[0.1.5]: https://github.com/smonchamps/wind/releases/tag/0.1.5
[0.1.4]: https://github.com/smonchamps/wind/releases/tag/0.1.4
[0.1.3]: https://github.com/smonchamps/wind/releases/tag/0.1.3
[0.1.2]: https://github.com/smonchamps/wind/releases/tag/0.1.2
[0.1.1]: https://github.com/smonchamps/wind/releases/tag/0.1.1
