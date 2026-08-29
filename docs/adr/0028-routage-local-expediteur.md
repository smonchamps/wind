# ADR 0028 — Le routage du Mode organisé est LOCAL, par expéditeur

Date : 2026-08-29 · Statut : accepté

## Contexte

Le Mode organisé (PLAN-MODE-ORGANISE, prototype validé en six passes)
trie le courrier en destinations — Réception, Kiosque, Registre,
écarté — sur le modèle HEY. La question structurante : où vit cette
destination ? Côté serveur (déplacer les messages IMAP) ou côté poste
(une donnée de présentation) ?

## Décision (CE, D1 du STOP 1, 2026-08-29)

**Routage local seul, par expéditeur.** La table
`routage_expediteurs(address PK, destination, regle, epoch)` — clé
adresse exacte en minuscules, LA même autorité de normalisation que la
garde d'images (A89), globale au poste comme elle. Jamais un
déplacement IMAP : les autres clients du compte voient le courrier
inchangé, et « Réintégrer » (DELETE de la ligne) défait tout — le
retour arrière est total.

- Un fil appartient à une destination si **n'importe lequel de ses
  messages** vient d'un expéditeur routé là — jamais la seule tête,
  qui est le dernier message toutes boîtes confondues : y répondre la
  déplace en Envoyés et le fil s'éjectait (prouvé RED, revue E1).
- Le geste « Déplacer vers… » résout l'adresse **côté cœur** : le
  dernier message du fil qui ne vient pas du compte — jamais la
  propre adresse de l'utilisateur (revue E1, prouvé RED).
- Le vocabulaire est fermé et validé en Rust avant toute écriture
  (`valider_routage`) ; les CHECK SQLite ne sont que la ceinture.
- Seules les **règles du Non** (E3 : spam / archive / corbeille — D4 :
  jamais une suppression définitive) toucheront le serveur, par la
  file `pending_actions` existante.

## Conséquences

- Les requêtes chaudes filtrent par une sonde `EXISTS`
  (`idx_envelopes_thread` puis PK routage — spike S2 : 0,209 ms à
  200 k, aucun `CROSS JOIN` directif nécessaire), garde de plan
  « jamais un scan d'envelopes » au filet.
- Les vues organisées n'excluent PAS les épinglées (leur section
  préposée n'existe qu'en Réception — les exclure ferait disparaître
  un fil épinglé routé de toutes les vues).
- Limite assumée et dite : la comparaison SQL `lower(trim(...))`
  diverge de la normalisation Rust sur les majuscules non-ASCII et
  blancs Unicode — une adresse réelle est ASCII (domaine punycode).

## Réversibilité

`DROP TABLE routage_expediteurs` + retrait des vues : le classique est
intact par construction (garde e2e « zéro diff » du mode éteint).
