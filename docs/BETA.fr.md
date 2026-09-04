# Wind — guide du bêta-testeur

Merci de tester Wind. Ce guide couvre l'installation, les deux
avertissements que vous pouvez rencontrer (ils sont attendus, et
expliqués honnêtement), et la façon de donner un retour.

Wind est un client email pour Windows et macOS : rapide, sobre,
local. Vos messages
restent sur votre machine ; l'application ne contacte que vos
fournisseurs de messagerie et la page des mises à jour. Aucune
télémétrie réseau.

## 1. Installer

1. Ouvrez la page des versions :
   <https://github.com/smonchamps/wind/releases/latest>
2. Téléchargez le fichier qui correspond à votre machine :
   - `Wind_<version>_x64-setup.exe` — PC Intel/AMD (le cas le plus
     courant) ;
   - `Wind_<version>_arm64-setup.exe` — PC ARM (Surface Pro X,
     Copilot+ à puce Snapdragon…) ;
   - `Wind_<version>_x64.dmg` — Mac Intel (les Mac Apple Silicon
     l'exécutent via Rosetta ; une version native viendra avec la
     demande).
   En cas de doute sur PC : Paramètres Windows > Système >
   Informations système, ligne « Type du système ». Sur Mac : menu
   Pomme > À propos de ce Mac.
3. Windows : lancez l'installeur et suivez-le. Mac : ouvrez le dmg et
   glissez Wind dans Applications.

### Si Windows affiche « Windows a protégé votre ordinateur »

C'est SmartScreen : Wind n'est pas encore signé par un certificat
commercial (la validation de l'émetteur est en cours — elle est
fermée hors USA/Canada à ce jour, nous attendons son ouverture).
Cliquez « Informations complémentaires » puis « Exécuter quand même ».

### Si l'installation est refusée sans recours

Sur certains PC récents, **Smart App Control** (Paramètres > Sécurité
Windows > Contrôle des applications et du navigateur) bloque les
programmes non signés **sans proposer de passer outre** — et son
verdict peut varier d'une version de Wind à l'autre. C'est la
limitation connue n° 1 de cette bêta. Si cela vous arrive :
**dites-le-nous** (voir §5) — c'est un retour précieux, pas une
fausse manœuvre de votre part. Nous ne vous demanderons jamais de
désactiver Smart App Control (sa désactivation est définitive et
c'est une protection réelle).

### Sur Mac : « Wind » ne peut pas être ouvert

macOS bloque les applications non notariées par Apple — Wind ne l'est
pas encore (même raison que les avertissements Windows ci-dessus : la
certification attend, la bêta passe d'abord). Une fois, au premier
lancement :

1. Double-cliquez Wind ; macOS refuse. Ouvrez **Réglages Système >
   Confidentialité et sécurité**, descendez jusqu'à *« Wind » a été
   bloqué*, cliquez **Ouvrir quand même**, puis relancez Wind.
2. Sous macOS 14 (Sonoma) ou avant, un raccourci existe : clic droit
   sur Wind dans Applications, « Ouvrir », puis **Ouvrir** dans la
   boîte de dialogue. (macOS 15 a retiré ce raccourci — passez par
   l'étape 1.)

Les mises à jour installées par Wind lui-même ne redemandent pas ce
geste.

## 2. Connecter votre boîte

Au premier lancement, Wind vous guide en cinq étapes : compte,
disposition, thème, un mot sur la bêta (et le bouton « Feedback » de
l'entête), récapitulatif.

### Gmail : l'écran « Google n'a pas validé cette application »

La vérification de Wind par Google est un audit long (plusieurs
mois), en cours. D'ici là, Google affiche un écran d'avertissement au
moment de connecter un compte Gmail. Pour continuer : « Paramètres
avancés » puis « Accéder à Wind (non sécurisé) ». Ce que Wind fait de
cet accès : lire et envoyer VOS emails depuis VOTRE machine, rien
d'autre — aucun serveur tiers ne voit vos identifiants ni vos
messages ; le jeton d'accès reste chiffré sur votre poste.

Outlook/Hotmail et les comptes IMAP classiques se connectent sans cet
écran.

## 3. Le mode « Organisé » — ce qu'on aimerait que vous essayiez

En haut de la fenêtre, à droite de la recherche, une bascule
**« Organisé »**. C'est la nouveauté de cette bêta, et le point sur
lequel votre retour nous intéresse le plus.

Activée, elle ouvre trois destinations au lieu d'une, et un endroit
où vous décidez :

- **Réception** — ce qui vous est écrit personnellement, et rien
  d'autre ;
- **Kiosque** — ce que vous y envoyez : infolettres et courrier
  d'information, en cartes qu'on fait défiler ;
- **Registre** — les envois qui se consultent plutôt qu'ils ne se
  lisent (reçus, alertes, confirmations), groupés par expéditeur ;
- **Portier** — les expéditeurs qui vous écrivent pour la première
  fois. Ils ne sont jamais informés de votre décision.

Ce n'est pas un tri automatique : **c'est vous qui rangez**, un
expéditeur à la fois, une seule fois — Wind applique ensuite votre
décision à tout ce qu'il envoie. Trois choses à savoir avant
d'essayer :

1. **Le « Non » du Portier agit chez votre fournisseur.** Par défaut,
   les messages qui arrivent ENSUITE de cet expéditeur partent **à la
   corbeille de votre boîte** (jamais une suppression définitive ; ce
   qui est déjà arrivé n'est pas touché).
   Vous pouvez choisir une autre règle au moment de décider
   (indésirable, archivage, ou « Écartés sans déplacer », qui ne
   touche à rien), et changer le défaut dans Réglages > Portier. Le
   « Oui » et les trois destinations, eux, ne déplacent rien : ce sont
   des vues de Wind, vos dossiers Gmail ou Outlook restent intacts.
2. **Le Portier ne regarde que les arrivées.** Il ne juge pas votre
   boîte d'hier : seuls les nouveaux expéditeurs, à partir du moment
   où vous activez le mode, passent devant lui.
3. **Tant que vous n'avez pas tranché**, le courrier en attente reste
   visible dans votre fil, marqué « En attente au Portier » — rien ne
   disparaît sans votre décision.

La bascule se rend dans l'autre sens à tout moment, et vos décisions
sont conservées (Réglages > Portier les liste, et permet de les
changer). Dites-nous ce qui vous a manqué, ce qui a été mal rangé, et
si vous avez gardé le mode allumé — ce dernier point est celui qui
nous apprend le plus.

## 4. Les mises à jour

Automatiques et signées : Wind vérifie au lancement, installe sur
votre accord, redémarre. Vous pouvez vérifier à la main dans
Réglages > À propos. Si une mise à jour échoue (Smart App Control,
encore lui), Wind vous le dit et vous laisse réessayer — signalez-le.

## 5. Donner un retour

**Cliquez le bouton « Feedback », en haut à droite de la fenêtre** :
écrivez votre message, il part par email depuis votre compte, avec la
version de Wind. (Si Wind lui-même est bloqué — installation refusée,
par exemple — écrivez directement à <feedback-wind@fcts.io>.)

Tout compte : un bug, une lenteur, un texte pas clair, un geste qui
vous manque, une habitude de votre client actuel que Wind casse.
Chaque retour est lu et instruit.

Le retour le plus utile tient en trois lignes :

1. **Ce que vous faisiez** (le geste, l'écran).
2. **Ce que vous attendiez.**
3. **Ce qui s'est passé** (avec l'heure, si c'est une lenteur).

La version installée se lit dans Réglages > À propos — mentionnez-la.

## 6. Ce que la bêta n'est pas encore

- Pas de signature commerciale de l'installeur (les avertissements du
  §1 — en attente de l'ouverture de la validation d'émetteur).
- Windows et macOS (natif Intel) seulement, pas de version web ni
  mobile.
- Le rattrapage complet d'une très grosse boîte (des centaines de
  milliers de messages) s'étale sur les premières heures d'usage — la
  recherche gagne en profondeur à mesure.
