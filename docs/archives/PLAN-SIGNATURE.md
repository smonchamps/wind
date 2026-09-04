> **Historical record — French, frozen** (closed on 2026-08-27; PLAN-ENGLISH-SWITCH
> D1, debt D-58). Not translated; the living documentation is in `docs/`.

# PLAN-SIGNATURE — signer les installateurs, et voir un échec de lancement

> **CHANTIER SOLDÉ le 2026-08-27 — terrain complet.** STOP 1 le
> 2026-08-26 (D1-D5, §5) ; E1 échouée proprement le 27 (D2 : attendre
> + filet seul, E2/E3 gelées → DEBT D-39) ; E4 en TDD ; **revue à
> regard neuf : 10 trouvailles, 10 corrigées** ; **gate complète VERTE
> 9/9** (2,2 min, 138 e2e) ; Système : **A85**. Commits `e00bd3f`
> (E4), `e97ad60` (intermittence SAC), `d0812ac` (release) — **CI
> verte sur les trois**. **0.10.2 publiée et vérifiée §2.10 : tout
> passe** (2 canaux, signatures distinctes). **Terrain (GO CE du
> 2026-08-27) : « release ok auto update ok sur les 2 postes »** —
> chemin nominal prouvé bi-arch, x64 SAC compris (loterie gagnée ce
> jour-là) ; la preuve du filet en condition de refus reste due À LA
> PREMIÈRE MAJ depuis la 0.10.2 sur poste SAC (consigné en E5). Aucun
> constat KO au STOP 2. Kaizen : session 9,2 M équiv. input, 4 gates
> complètes (1 explicite + 3 pre-push). Dette : D-39 (signature
> gelée), D-40 (issue amont, GO CE en attente).

> Énoncé (2026-08-26) : « Feature : signer les installateurs Windows en
> Authenticode (Azure Trusted Signing) pour que Smart App Control ne
> bloque plus l'installation ni l'auto-update. Inclut le filet : un
> échec de lancement de l'installateur doit se voir au lieu de fermer
> l'application en silence. » Décision CE du même jour : « je paierai
> les 10 $ par mois ».

> **Phase 0 est faite** — instruction menée le 2026-08-26, spike
> `spikes/maj-x64/` (commit `ad1e1f7`), relevés joués par le CE sur le
> poste x64. Ce plan ne ré-instruit pas.

---

## 1. Constat — ce qui est mesuré

**Le clic « Installer » de la 0.10.1 ferme Wind sans rien installer sur
le poste x64.** La chaîne est prouvée maillon par maillon (relevés du
2026-08-26, ZEPHYRUSSMO, Windows 11 Home 26200) :

- manifeste, clé `windows-x86_64`, téléchargement entier (6 355 666
  octets), signature minisign, écriture du témoin, droits d'écriture
  sur `C:\Users\smonc\AppData\Local\Wind` : **tout est vert** ;
- le lancement du témoin avec les arguments mêmes de l'updater
  (`/P /R /UPDATE /ARGS`) est **REFUSÉ** : « Une stratégie de contrôle
  d'application a bloqué ce fichier » — **Smart App Control**,
  `SmartAppControlState = On` ;
- contre-épreuve : l'installateur **0.10.0**, non signé pareillement,
  **se lance** sur le même poste. Le verdict SAC se rend **binaire par
  binaire** (réputation cloud du hash) — chaque release non signée est
  une loterie, et l'issue peut changer avec le temps (panne
  intermittente possible) ;
- le silence vient du plugin : `tauri-plugin-updater` 2.10.1,
  `updater.rs:854-865` — le retour de `ShellExecuteW` n'est **jamais
  testé**, puis `std::process::exit(0)`. Vérifié sur les sources du
  crate. L'application se ferme quoi qu'il arrive.

**Deux défauts distincts, donc deux remèdes :**

1. **Sans signature Authenticode, l'installation de Wind dépend d'un
   état local qu'on ne contrôle pas.** SAC est `On` par défaut sur les
   Windows 11 récents : tout poste client peut vivre la même panne. La
   signature minisign (ADR 0013) protège l'intégrité mais Windows ne la
   voit pas. L'ADR 0013 (décision 3) avait **reporté** la signature de
   code « au lancement public » — le terrain vient de rendre le report
   caduc : la bêta fermée est devant nous et l'auto-update est déjà
   cassé sur un poste sur deux.
2. **Un échec de lancement est invisible.** Même signé, un lancement
   peut échouer (stratégie d'entreprise, antivirus tiers) ; fermer
   l'application sans un mot viole §9 (jamais d'erreur avalée).

## 2. Le point dur externe — mesuré le 2026-08-26

**L'onboarding Azure Trusted Signing (rebaptisé « Artifact Signing »)
est restreint depuis avril 2025** : préversion limitée aux
organisations USA/Canada avec 3 ans d'historique vérifiable ; la
validation d'identité **individuelle** (USA/Canada seulement) est
**en pause** pour les nouveaux inscrits, sans échéance annoncée.
Tarif inchangé : Basic 9,99 $/mois, 5 000 signatures (une release en
consomme ~6 : exe applicatif + installateur NSIS, × 2 canaux, marge
comprise). Seul l'essai réel au portail, avec le compte du CE, dira si
la porte est ouverte — c'est **E1, échec franc et rapide avant tout
code**.

**Repli si la porte est fermée** (à trancher en D2 seulement si E1
échoue) :

| Option | Coût | Faits |
|---|---|---|
| A. Certum Open Source Code Signing | ~69 €/an + carte/lecteur ~40 € une fois | réservé aux projets open source (le dépôt `smonchamps/wind` est public) ; certificat individuel OV ; clé sur carte physique — la signature reste locale, compatible builds du poste |
| B. SSL.com eSigner ou OV cloud | ~200–400 $/an | signature cloud, pas de matériel ; quota de signatures selon formule |
| C. Attendre la réouverture Trusted Signing | 0 $ en attendant | l'auto-update x64 reste une loterie pendant l'attente ; le filet (E4) rend au moins l'échec visible |

Toutes les trois donnent un Authenticode chaîné à une CA reconnue —
ce que SAC exige. La préférence CE (10 $/mois) désigne Trusted
Signing ; le repli n'existe que pour l'échec d'E1.

## 3. Refus de périmètre

- **Pas de certificat EV ni de réputation SmartScreen garantie** : la
  réputation s'accumule d'elle-même sur l'identité signante ; on ne
  paie pas un EV pour l'accélérer.
- **Pas de MSIX, pas de Store** : tranché à l'ADR 0013, rien de neuf.
- **Pas de signature en CI** : les builds de release sont locaux
  (`make-release.ps1`, D6 de PLAN-RETOURS-8) et le restent — les
  secrets ne montent pas sur GitHub.
- **Pas de correctif amont du plugin dans ce chantier** : une issue
  est ouverte chez `tauri-plugin-updater` (le retour de `ShellExecuteW`
  ignoré), mais notre filet ne l'attend pas.
- **Les builds dev, gate et e2e ne signent rien** : aucun secret ni
  réseau exigé pour développer (même isolation que les identifiants
  OAuth, D1 de PLAN-RETOURS-9).

## 4. Étapes

**E1 — la porte Azure (action CE, aucun code). ÉCHOUÉE le 2026-08-27**
— compte créé, rôles posés, mais la validation individuelle est
fermée hors USA/Canada (adresse du CE en France). Verdict propre,
obtenu avant tout code de signature, comme voulu. → D2. Créer la ressource
Artifact Signing (Basic), soumettre la validation d'identité
individuelle (Canada), créer le profil de certificat « Public Trust ».
Gate : profil actif dans le portail. **Échec franc** : si
l'inscription individuelle est refusée/en pause → STOP, décision D2
(repli) avant toute suite.

**E2 — GELÉE (D2) — la preuve par un témoin.** Outillage du poste mainteneur
(ARM64) : `trusted-signing-cli` (ou `signtool` + dlib), authentification
par app registration dédiée au moindre privilège. Signer un exe témoin,
puis **le lancer sur le poste x64, SAC On** — la seule preuve qui
compte. Gate : `Get-AuthenticodeSignature` = `Valid` ET lancement
accepté par SAC sur ZEPHYRUSSMO.

**E3 — GELÉE (D2) — l'intégration release.** `make-release.ps1` injecte
`bundle.windows.signCommand` au moment du build (`--config` de
surcharge — `tauri.conf.json` committé reste sans signature : un build
dev ne signe jamais). Tout-ou-rien conservé : secrets absents = release
interrompue avant les builds. `verify-release.ps1` étendu : contrôle
Authenticode des deux exe publiés (18 → 20 contrôles). Gate : une
release à blanc (non publiée) sort deux exe signés `Valid`.

**E4 — le filet : l'échec de lancement se voit.** Remplacer le
`download_and_install` du plugin par `download()` + lancement **à
nous** : écrire le témoin, le lancer par `std::process::Command`
(retour testé, erreur typée), quitter seulement si le lancement a
réussi ; sinon l'erreur remonte au bandeau (la voie `erreur.maj`
existe déjà dans l'UI). TDD sur la décision pure (construction des
arguments NSIS, mapping des erreurs) ; le lancement lui-même est de
l'I/O de plateforme — preuve au terrain, comme l'ADR 0013 l'assume.
Issue ouverte en amont. Gate : tests Rust + e2e existants verts (le
bandeau ne change pas de forme, seul son texte d'échec devient
possible).

**E5 — la preuve vivante** *(réécrite après D2, ré-amendée le
2026-08-27)*. Fait nouveau : l'auto-update 0.10.0 → 0.10.1 **a fini
par passer** sur le poste x64 — le même exe, refusé le 26, accepté le
27 : le verdict SAC change AVEC LE TEMPS (réputation du hash), la
loterie est complète, intermittence prouvée. Release **0.10.2** non
signée, vérification §2.10 (18/18), puis terrain :
- arm64 : auto-update 0.10.1 → 0.10.2 normal ;
- x64 **SAC On, désormais en 0.10.1** (updater ANCIEN — le filet
  n'existe qu'une fois la 0.10.2 posée) : le clic donne SOIT
  l'installation (chemin nominal prouvé sur x64), SOIT le refus SAC du
  hash neuf — et alors l'app se ferme encore en silence : c'est
  l'ancienne version qui parle, pas une régression. **La preuve du
  filet au terrain se fait à la PREMIÈRE mise à jour depuis la
  0.10.2** sur ce poste (prochaine release, ou refus SAC de celle-ci
  suivi d'un passage ultérieur). C'est le STOP 2.

**E6 — documentation.** ADR 0027 (la signature de code : lève le
report de l'ADR 0013 décision 3, consigne le choix Trusted Signing et
les faits SAC) ; STANDARD §2.9/§2.10 amendés (secrets du poste,
20 contrôles) ; ETAT ; mémoire persistante ; DEBT si l'issue amont
reste ouverte.

## 5. Décisions CE — tranchées le 2026-08-26 (STOP 1 passé)

- **D1 — Le compte Azure : « GO, je tente E1 ».** Le CE crée la
  ressource au portail ; si la porte est fermée, retour trancher D2.
- **D2 — Le repli : « Attendre + filet seul »** (tranchée le
  2026-08-27). **E1 a ÉCHOUÉ** — la validation d'identité individuelle
  Trusted Signing est réservée aux résidents USA/Canada ; l'adresse du
  CE est en France. Le compte Azure créé (`rg-fcts`) est à supprimer.
  Décision : on ne signe pas pour l'instant (ni Certum ~69 €/an, ni OV
  cloud) ; la 0.10.2 part avec le seul filet E4 — l'échec devient
  visible et retentable, l'auto-update x64 reste une loterie SAC tant
  que rien n'est signé. **E2 et E3 sont GELÉES**, à réveiller quand la
  porte individuelle rouvre (ou si une autre voie de signature est
  choisie) → consigné en DEBT.
- **D3 — Les secrets de signature : « Fichier sous C:\Keys ».** Le
  patron de la clé minisign — chemins lus par `make-release.ps1`,
  jamais le dépôt, jamais de variables persistantes.
- **D4 — Le filet E4 : « Lancement testé chez nous ».** `download()`
  du plugin puis lancement par nos soins, retour testé, erreur au
  bandeau. Issue amont ouverte en plus.
- **D5 — La version de preuve : « 0.10.2 »**, publiée à l'issue d'E5
  seulement.
