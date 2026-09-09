# Journal des modifications

La traduction française du [CHANGELOG](CHANGELOG.md), tenue depuis la
0.20.0 — **pour les personnes qui utilisent Wind** : la section de
chaque version s'affiche dans l'application, dans la fenêtre « Quoi
de neuf », au premier lancement après une mise à jour. Chaque version
publiée ajoute son entrée ici ET dans l'original anglais (une entrée
absente ici retombe sur l'anglais — jamais de fenêtre vide). Écrit en
mots simples, sur ce que vous pouvez faire ou remarquerez, sans
vocabulaire interne au projet (guide : `docs/STANDARD.md` §2.9).

## [0.22.0] - unreleased

La fenêtre retient sa taille, les indésirables se vident d'un geste,
les claviers Mac se sentent chez eux, et une série de demandes des
testeurs de la bêta.

### Ajouté

- **Wind retient votre fenêtre.** Elle s'ouvre en plein écran la
  première fois ; ensuite elle rouvre à la taille et à l'endroit où
  vous l'avez laissée.
- **Tout replier dans le Kiosque d'un coup.** Un bouton « Tout
  replier » dans la section Non lus replie chaque carte sur son objet
  pour une vue d'ensemble — et les redéplie. Replier ne marque jamais
  rien comme lu.
- **Tout sélectionner, vider les indésirables.** Ctrl+A (⌘A sur Mac)
  coche toutes les conversations affichées ; les indésirables gagnent
  un bouton « Vider » qui les vide d'un geste, après confirmation.
- **Les claviers Mac se sentent chez eux.** Sur macOS la touche ⌫
  supprime la conversation sélectionnée, et la liste des raccourcis
  montre les touches Mac (⌫, ⌘Espace).

### Modifié

- **Les invitations reflètent votre réponse d'où qu'elle vienne.** Si
  vous acceptez ou refusez une réunion dans une autre application de
  courrier, l'invitation dans Wind affiche cette réponse une fois la
  synchronisation faite.
- **Un message plus clair quand une connexion manque l'accès au
  courrier.** Si une autorisation Google ou Microsoft est validée sans
  cocher la permission de la boîte, Wind le dit clairement et indique
  quelle case cocher, dans votre langue.
- **La barre « aider à améliorer Wind » disparaît.** Elle proposait de
  conserver des rapports de plantage locaux ; elle ajoutait du bruit
  pour peu de chose, elle est retirée.

### Corrigé

- **Une réponse envoyée hors ligne ne laisse plus de brouillon
  fantôme.** Répondre et envoyer aussitôt hors ligne laissait une
  copie dans les Brouillons avec un message déroutant ; la réponse
  part maintenant proprement.

## [0.21.0] - 2026-09-08

Voyez ce qui a changé après chaque mise à jour, joignez des captures
d'écran à vos retours, et la première vague de corrections issues des
testeurs de la bêta.

### Ajouté

- **« Quoi de neuf » après une mise à jour.** Le premier lancement
  d'une nouvelle version ouvre une fenêtre avec les changements de
  cette version — cette page, dans l'application, dans votre langue.
  Continuer la ferme ; elle ne s'affiche jamais deux fois.
- **Des images sur le formulaire de retour.** Joignez jusqu'à trois
  captures d'écran (jpg ou png) à votre retour ; elles voyagent avec
  le message et ne peuvent plus se perdre une fois Envoyer pressé.
- **Copier l'adresse de l'expéditeur.** Dans le panneau de lecture,
  l'adresse de l'expéditeur ouvre un petit menu avec « Copier
  l'adresse ».
- **Ctrl+F cherche là où vous êtes.** Le raccourci atterrit sur le
  filtre propre de l'écran courant quand il en a un — le Nettoyage
  gagne exactement ce filtre (restreindre les groupes par nom ou
  adresse d'expéditeur) — et sur la recherche principale sinon.

### Modifié

- Les résultats de recherche se lisent du plus récent au plus ancien,
  page après page.
- Une carte du Kiosque ne se marque lue qu'après un vrai temps de
  lecture — ouvrir le Kiosque ne « lit » plus les cartes courtes au
  premier regard.

### Corrigé

- Les menus « ⋯ » se ferment d'un second clic sur leur propre bouton,
  sur tous les écrans ; un menu près du bas de la fenêtre s'ouvre
  désormais au-dessus de son bouton au lieu de glisser par-dessus.
- Dans Réglages › Portier, une adresse longue ne peut plus pousser le
  verdict hors de sa ligne.

## [0.20.0] - 2026-09-07

Sauvegardez et restaurez vos données de courrier, sélectionnez au
clavier, et une longue liste de défauts discrets trouvés par une
revue complète de Wind, désormais corrigés.

### Ajouté

- **Sauvegarde et restauration, dans Réglages › Vos données.**
  Enregistrez une copie complète de vos données locales où vous voulez
  (les mots de passe n'y sont jamais). La restauration est prudente :
  elle s'applique au prochain démarrage, garde les données remplacées
  à côté, et les messages non envoyés trouvés dans la sauvegarde sont
  retenus jusqu'à votre décision : restaurer une sauvegarde ne peut
  jamais envoyer de vieux courrier tout seul.
- **« Oublier aussi ce que Wind a appris. »** À la suppression d'un
  compte, un second choix efface tout ce que Wind avait appris de
  lui : contacts, décisions d'expéditeurs et permissions d'images qui
  n'appartenaient qu'à ce compte. Ce qui est partagé avec un autre
  compte reste.
- **Des états « indisponible » honnêtes.** Un message dit désormais si
  son contenu n'est simplement pas encore téléchargé (il réessaiera)
  ou n'existe plus sur le serveur. Une recherche dit aussi quand une
  partie de votre courrier ancien n'est pas incluse, au lieu de
  montrer discrètement moins de résultats.
- **Sélection au clavier.** Ctrl+Espace coche ou décoche le message en
  focus, Maj+Espace étend la sélection, comme à la souris.
- **Les invitations d'agenda se comportent comme du vrai courrier.**
  Une invitation arrivée sans texte se retrouve désormais par son
  titre et son lieu dans la recherche, et son transfert envoie le
  vrai fichier d'invitation au lieu d'un message vide.
- **Changez d'avis sur les images.** La permission « afficher les
  images distantes », donnée message par message, peut désormais se
  reprendre, message par message.

### Modifié

- **Wind parle votre langue partout.** Tailles de fichiers, dialogues
  système, la ligne « a écrit : » des réponses, les en-têtes de
  messages transférés et les écrans d'accueil suivent la langue
  choisie. Plus de fragments français dans une interface anglaise.
- Le bouton « réparer » la connexion n'apparaît que quand la connexion
  en a vraiment besoin, et l'état de connexion reste à jour au lieu de
  figer sur son dernier mot.
- La barre de progression de la synchronisation est plus légère sur
  les grandes boîtes.

### Corrigé

- Les réponses, transferts et contenus collés gardent leur mise en
  forme pendant que le compositeur continue de bloquer les images
  distantes. Le texte supprimé d'une citation reste supprimé à
  l'enregistrement ou à l'envoi.
- Si l'enregistrement d'un brouillon échoue, le compositeur reste
  ouvert et le dit, au lieu de se fermer en perdant votre texte. La
  fermeture attend d'abord que les modifications et pièces jointes en
  cours soient en sûreté.
- Les brouillons commencés dans une autre application de courrier
  gardent leurs destinataires, pièces jointes, priorité et contexte de
  réponse quand Wind les importe.
- L'envoi est honnête dans l'incertitude : quand la réponse du serveur
  est ambiguë, Wind ne réessaie jamais en silence au risque d'un
  doublon, un envoi interrompu par un plantage est récupéré au
  prochain démarrage, et un message en cours de déplacement n'est
  retiré de son origine qu'une fois la copie confirmée.
- Répondre à tous répond désormais à l'adresse demandée par
  l'expéditeur (Reply-To) ; un message aux seuls destinataires cachés
  (Cci) part sans les exposer ; vos messages envoyés montrent leur Cc
  tout de suite.
- Une première synchronisation interrompue en route se termine
  désormais correctement à la reprise : les messages supprimés entre
  temps ne reviennent plus.
- Les mises à jour d'invitations arrivent dans le bon ordre : une
  demande de réunion plus récente n'est plus annulée par une
  annulation plus ancienne, et répondre à une occurrence d'une série
  répond à cette occurrence.
- Moins de boucles surprises « reconnectez-vous » avec les comptes
  Gmail et Outlook, et un chemin de réparation clair quand l'accès
  expire vraiment.
- Un dossier que le serveur refuse d'ouvrir le dit désormais au lieu
  de disparaître en silence, et les adresses d'images non sécurisées
  sont promues en adresses sécurisées quand vous autorisez les images.
- La lecture reste instantanée pendant une grosse synchronisation :
  plus de panneau de lecture vide pendant plusieurs secondes.
- Le sélecteur d'expéditeur est centré verticalement à côté de son
  étiquette De.
