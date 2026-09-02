# ADR 0030 — Une seule instance, et des actions refusées mises en quarantaine

Date : 2026-09-02 · Statut : accepté
· Amende [ADR 0003](0003-boite-envoi-smtp.md) (la distinction
  transitoire / permanent de la boîte d'envoi s'étend au journal
  d'actions) et [ADR 0019](0019-commandes-hors-du-thread-principal.md)
  (la garde du thread principal vérifie aussi les commandes `async`).

## Contexte

L'audit complet du 2026-09-01 (`docs/AUDIT-2026-09-01.md`) a relevé
deux silences structurels : aucune garde mono-instance alors que
`main.rs` nommait le risque (deux pompes concurrentes mettant en
quarantaine les envois l'une de l'autre), et un journal d'actions où le
premier refus DÉFINITIF du serveur (`NO`/`BAD` — dossier disparu)
bloquait toute la file d'une boîte, à vie, sans un mot, parce que le
port réseau ne distinguait pas un refus d'une coupure.

## Décisions (CE, 2026-09-01 — D1 et D2 de PLAN-AUDIT-V1)

**Mono-instance par verrou fichier**, pas de plugin : `wind.lock` à
côté de `wind.db`, pris en exclusif par le premier processus (`fs4`,
déjà en dépendance), relâché par l'OS à sa mort — jamais de verrou
collant. La seconde instance dit « Wind est déjà ouvert. » et sort
(D1 : message puis sortie, pas de mise au premier plan). Le verrou se
prend AVANT toute base et toute fenêtre — mais APRÈS le déménagement
Discovery → Wind, qu'il ferait sauter en créant le dossier cible ; le
déménagement est donc tolérant à la course (`rename_tolerant`).

**Le journal d'actions distingue le refus de la panne** :
`Error::Refus` (NO/BAD) à côté d'`Error::Server` (transitoire par
défaut, on retente). Un refus met l'action en QUARANTAINE sur-le-champ
et le rejeu continue ; cinq échecs transitoires y mènent aussi. Une
refusée n'est pas éternelle : un geste neuf de l'utilisateur sur le
même message la remplace. La fente d'avis compte les refusées (D2 :
sans bouton — l'UI de décision attend la vague 2).

## Conséquences

- Deux postures dites au terrain : double lancement ⇒ un message, une
  fenêtre ; dossier supprimé côté serveur ⇒ une ligne dans la fente, les
  gestes suivants passent.
- La garde `garde-thread-principal.mjs` refuse désormais la base, le
  coffre et les fichiers dans la glu d'une commande `async` (hors
  `hors_pompe`/`spawn_blocking`) — 17 commandes migrées.
- Pièges gravés : sur Windows, ni un timeout ni un `shutdown` posés sur
  un CLONE de socket n'agissent sur le handle d'origine — la veille IDLE
  est bornée par un flux dont `set_read_timeout(None)` vaut un plancher
  (`FluxBorne`) ; `REFERENCES` est un mot réservé SQLite.
