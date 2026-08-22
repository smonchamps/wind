# ADR 0024 — Parseur iCalendar : la crate `calcard`, dans une crate `mail-ical` pure

Date : 2026-08-22 · Statut : accepté (décision CE D1, PLAN-INVITATIONS)

## Contexte

Traiter les invitations de réunion (PLAN-INVITATIONS) exige de lire
l'iCalendar (RFC 5545) des parties `text/calendar` et de générer les
réponses iTIP (`METHOD:REPLY`, RFC 5546). Le point dur est la
**résolution des fuseaux** : Google émet des TZID IANA
(`Europe/Paris`), Outlook/Exchange des TZID **Windows** (« Romance
Standard Time ») — une résolution fausse afficherait une heure de
réunion fausse, le pire mensonge possible pour cette fonctionnalité.

## Décision

**La crate `calcard` 0.3.11** (Stalwart Labs — la maison de
`mail-parser`, déjà au dépôt), en `default-features = false`, dans une
**crate `mail-ical` pure** (zéro I/O, zéro horloge — le DTSTAMP vient
de l'appelant) : parseur + générateur REPLY, l'application ne voit que
`Invitation` et `reponse_itip`.

## Départage (set-based, 2026-08-22)

Deux spikes jetables sur un corpus commun de 6 fixtures (Google/IANA,
Outlook/TZID Windows, UTC nu, journée entière, CANCEL, récurrence) +
épreuve de génération REPLY (pliage 75 octets, CRLF, re-parse) :

| Critère | A — `calcard` | B — parseur maison + `chrono-tz` |
|---|---|---|
| Justesse corpus | 71/71 PASS | 81/81 PASS |
| TZID Windows | table complète embarquée | table maison (~140 entrées CLDR à tenir) |
| Poids binaire (arm64, release) | +1,73 Mio | +1,36 Mo (la base tz domine les deux) |
| Coût de possession | **~150 lignes de glue** | **~600-700 lignes possédées** (un vrai bug payé pendant l'écriture du spike) |
| Temps de parse | 2-8 µs | 2,6-6,7 µs |

Justesse et vitesse à égalité, poids comparable (budget installeur
< 15 Mo tenu large) : par la règle §2.3, l'alternative ne bat pas
l'hypothèse — c'est le coût de possession qui tranche.

## Le piège gravé

`TzResolver::resolve_or_default` retombe sur `Tz::Floating` pour un
TZID hors tables : l'heure serait traitée comme UTC — **un décalage
silencieux**, mesuré au spike (sonde « Zone Perso Wind » : 09:00 rendu
09:00Z au lieu de 08:00Z). `mail-ical` appelle donc **`resolve()`** et
traite le `None` : l'heure devient `Quand::Flottant`, affichée TELLE
QUELLE avec la mention « heure locale de l'organisateur » — jamais une
conversion mensongère (garde D1, tenue par test). Les VTIMEZONE
embarqués ne sont pas interprétés (calcard résout par le NOM du TZID) ;
un producteur au TZID propriétaire tombe dans ce repli honnête.

## Conséquences

- Sixième crate du workspace (`crates/mail-ical`), consommée par
  `mail-core` (motif §4 : décision pure et testable, I/O ailleurs).
- Dépendances : `calcard` + `chrono-tz` (+ `mail-builder`, transitif).
- Le corpus des spikes est versé en tests (`crates/mail-ical/tests/`),
  rejoué à chaque gate.
