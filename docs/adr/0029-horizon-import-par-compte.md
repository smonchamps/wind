# ADR 0029 — L'horizon d'import des corps, par compte

Date : 2026-08-30 · Statut : accepté
· Amende [ADR 0010](0010-synchronisation-integrale.md) (les corps
  gagnent un horizon choisi ; les enveloppes restent intégrales)

## Contexte

Le CE demande, à l'ajout d'un compte, le choix de la longueur
d'historique importée en local pour la recherche (PLAN-HORIZON-
NETTOYAGE, volet A). L'ADR 0010 avait tranché « tout, sans horizon » —
mais sa plomberie a gardé la borne : chaque pompe de rattrapage prend
un `since_epoch`, la production passait `NO_HORIZON`, et le commentaire
de `backfill.rs` prévoyait qu'« un futur réglage utilisateur la
retrouverait telle quelle ». C'est ce réglage.

## Décision (CE, 2026-08-30 — D1-D4 du PLAN)

**Les enveloppes restent intégrales ; seuls les CORPS sont bornés à
l'horizon choisi** (D1, option A2). Le poids d'un message est son corps
(~49 des ~50 ko/message mesurés à l'ADR 0010 §4) ; l'enveloppe (~1,2 ko)
reste bon marché et porte la liste, le regroupement et la recherche par
objet/expéditeur.

- **Vocabulaire fermé** : `1m, 2m, 3m, 6m, 1a, 2a, tout`
  (`HORIZONS_IMPORT`, `backfill.rs`). Traduction en epoch par
  `horizon_epoch()` — jours pleins, dérivés à la LECTURE : la borne
  suit l'horloge, jamais une date figée à l'ajout.
- **Pref par compte** `horizon_import.{id}` (patron signature/repère,
  `PREFS_PAR_COMPTE` — purgée au retrait). Absente ou corrompue :
  « tout » — le défaut SÛR, jamais une amputation silencieuse (D4 :
  les comptes d'avant le réglage importent tout, rien ne change pour
  eux).
- **Défaut du sélecteur à l'ajout : « 1 an »** (D2).
- **Réglable après coup** (D3, Réglages > Comptes) : étendre rend des
  corps éligibles — la pompe les rattrape à sa prochaine passe (elle
  est reprenable, l'état c'est la base) ; **réduire n'efface rien**.
- **Portée de la borne** : la pompe des corps et ses compteurs
  (`bodies_pending_count` / `bodies_total_count` — numérateur ET
  dénominateur parlent du même corpus, sinon la barre n'atteint jamais
  100 %) et les corps d'arrivée (uniforme, sans effet en pratique).
  **Hors borne** : les en-têtes de fil et les destinataires (de
  l'enveloppe, ~3 ko — la raison de l'ADR 0010 §Contexte demeure) et
  le chargement au clic — un message hors horizon reste lisible à
  l'ouverture, son corps vient du serveur à la demande (chemin
  existant).

## Conséquences

- La recherche plein-texte des corps porte sur l'horizon choisi ; la
  recherche d'objets et d'expéditeurs porte sur tout (l'index FTS suit
  la base, aucune logique de recherche nouvelle).
- Ce que l'ADR 0010 promettait en « tout cherchable » devient un CHOIX
  utilisateur dont « tout » reste une valeur du vocabulaire — et le
  défaut factuel des comptes existants.
- La garde d'espace disque (0010 §4) surestime désormais pour un
  compte borné (elle compte ~50 ko/message pour des messages dont seul
  ~1,2 ko sera stocké hors horizon). Assumé : l'estimation était
  « délibérément haute » par principe.

## Alternatives écartées

| Option | Pourquoi non |
|---|---|
| A1 — tout borner (enveloppes comprises, `UID SEARCH SINCE`) | Les messages anciens n'existeraient nulle part (ni liste, ni lecture, ni recherche d'objet) ; renverse 0010 frontalement ; bornage neuf à écrire dans `SyncEngine` quand la plomberie des corps existe déjà. |
| Purger les corps quand on réduit l'horizon | On n'efface pas ce qu'on a — un trou fabriqué après coup est pire qu'un disque occupé (même esprit que 0010, « quota avec purge » refusé). |
| Date figée à l'ajout du compte | Une borne qui ne suit pas l'horloge dériverait : « 1 an » deviendrait « 1 an à partir de 2026 ». |
