# ADR 0031 — Vague 2 de l'audit : envoi borné, lot atomique, transfert sans image distante, un seul menu

Date : 2026-09-02 · Statut : accepté
· Amende [ADR 0003](0003-boite-envoi-smtp.md) (un envoi transitoire n'est
  plus retenté sans fin), lève l'exception §6.4 du terrain du 2026-08-20
  (les images du transfert), et clôt la moitié « front » de D-47 (le
  menu du produit).

## Contexte

La vague 2 de l'audit du 2026-09-01 (`docs/AUDIT-2026-09-01.md` §5,
`docs/PLAN-AUDIT-V2.md`) traite les S2 mesurables. La plupart sont des
remèdes techniques sans décision produit — l'ouverture de la base, les
index du Nettoyage, les lots IMAP, la synchro reprenable. Cinq points
appartenaient au Chef Ingénieur : ce que devient un envoi qui échoue
toujours, ce que vaut un geste de masse qui échoue au milieu, ce que
« Transférer » charge, ce qu'on fait d'un serveur sans CONDSTORE, et si
un flaky doit rendre la gate rouge.

## Décisions (CE, 2026-09-02 — D1-D8 de PLAN-AUDIT-V2)

**Un envoi empoisonné est REFUSÉ au cinquième échec transitoire** (D5,
`SEUIL_ENVOI = 5`, comme la quarantaine des actions) : il sort de la
file avec son motif, l'utilisateur tranche (renvoyer, supprimer), et
le message suivant a son tour. Avant, `attempts` se comptait sans
jamais se lire — un message empoisonné retenait la file du compte à
vie.

**Un geste de masse est TOUT OU RIEN** (D6) : `Store::agir_groupe`
développe les fils côté cœur et enchaîne le lot dans UNE transaction ;
une panne au milieu ne laisse rien à moitié fait, et l'UI le dit. Un
seul appel remplace N × k commandes unitaires en série (250 + 50 IPC
pour cinquante conversations).

**« Transférer » ne charge AUCUNE image distante** (D8) : le composeur
reçoit le bloc transféré au pixel neutre, marqué de sa source
(`data-wind-transfert="compte/uid/boîte"`, allowlisté à la frontière) ;
à l'envoi, le bloc est remplacé par le rendu de la source AVEC ses
images — le destinataire reçoit le même message, le pixel de suivi ne
part plus au clic « Transférer ». Limite dite : une retouche DANS le
bloc transféré est perdue (on transmet, on ne commente pas ligne à
ligne) ; ce qui est tapé AVANT reste.

**Un serveur sans CONDSTORE est une dette dite, pas un chantier** (D3) :
ses drapeaux ne se resynchronisent pas ; une ligne de `wind.log` le
nomme à la relève, pour savoir si le cas existe en bêta (Gmail,
Microsoft 365 et Dovecot l'annoncent tous).

**Un flaky se COMPTE, il ne rend pas la gate rouge** (D4, confirme
PLAN-KAIZEN E3) : le rapporteur JSON de Playwright et `e2e/flaky.mjs`
impriment « flaky : N » au verdict — le chiffre que la décision
`failOnFlakyTests` attendait n'existait pas.

**Un seul menu** (D1, STOP visuel du 2026-09-02 sur la Liste) :
`Menu.svelte` porte le dessin et la mécanique (clavier compris) des
huit surfaces ; le front est entré dans la même vague que le cœur.

## Conséquences

- Mesures gravées au PLAN : second `Store::open` 36 → 0,9 ms (200 k),
  indexation d'un corps de 28 Mo 401 → 338 ms et 210 → 133 Mo,
  `nettoyage_groupes` 380 → 67 ms (200 k / 5 000 expéditeurs), analyse
  MIME 18,2 → 11,1 ms les 50 corps, sondes au repos 5 → 2 par 10 s.
- Deux remèdes de l'audit REFUSÉS sur mesure ou sur preuve : le COUNT
  par frappe (1,5 ms sur 57 : le coût est la page triée) et
  `withGlobalTauri: false` (`__TAURI_INTERNALS__` reste injecté ; la
  CSP est la frontière, complétée).
- Pièges gravés : le SQLite embarqué (3.50) peut préférer l'index de
  date là où celui d'un autre outil choisit bien — `INDEXED BY` et un
  test de PLAN d'exécution ; un cadre sans script (S1) n'est pas
  évaluable par Playwright — on focalise l'iframe depuis le parent et
  on frappe la vraie touche.
