# Wind — guide du bêta-testeur

Merci de tester Wind. Ce guide couvre l'installation, les deux
avertissements que vous pouvez rencontrer (ils sont attendus, et
expliqués honnêtement), et la façon de donner un retour.

Wind est un client email Windows : rapide, sobre, local. Vos messages
restent sur votre machine ; l'application ne contacte que vos
fournisseurs de messagerie et la page des mises à jour. Aucune
télémétrie réseau.

## 1. Installer

1. Ouvrez la page des versions :
   <https://github.com/smonchamps/wind/releases/latest>
2. Téléchargez l'installeur qui correspond à votre machine :
   - `Wind_<version>_x64-setup.exe` — PC Intel/AMD (le cas le plus
     courant) ;
   - `Wind_<version>_arm64-setup.exe` — PC ARM (Surface Pro X,
     Copilot+ à puce Snapdragon…).
   En cas de doute : Paramètres Windows > Système > Informations
   système, ligne « Type du système ».
3. Lancez l'installeur et suivez-le.

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
**dites-le-nous** (voir §4) — c'est un retour précieux, pas une
fausse manœuvre de votre part. Nous ne vous demanderons jamais de
désactiver Smart App Control (sa désactivation est définitive et
c'est une protection réelle).

## 2. Connecter votre boîte

Au premier lancement, Wind vous guide en quatre étapes : compte,
disposition, thème, récapitulatif.

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

## 3. Les mises à jour

Automatiques et signées : Wind vérifie au lancement, installe sur
votre accord, redémarre. Vous pouvez vérifier à la main dans
Réglages > À propos. Si une mise à jour échoue (Smart App Control,
encore lui), Wind vous le dit et vous laisse réessayer — signalez-le.

## 4. Donner un retour

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

## 5. Ce que la bêta n'est pas encore

- Pas de signature commerciale de l'installeur (les avertissements du
  §1 — en attente de l'ouverture de la validation d'émetteur).
- Windows seulement, pas de version web ni mobile.
- Le rattrapage complet d'une très grosse boîte (des centaines de
  milliers de messages) s'étale sur les premières heures d'usage — la
  recherche gagne en profondeur à mesure.
