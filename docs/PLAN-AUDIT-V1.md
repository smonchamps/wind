# PLAN-AUDIT-V1 — vague 1 de l'audit du 2026-09-01 : les S1 du cœur et du shell

> **CHANTIER SOLDÉ le 2026-09-02 — terrain complet** (GO CE au STOP 1
> le 2026-09-01 au soir, D1-D4 ; verdict terrain « ok » le 2026-09-02
> au matin, **0 constat KO au STOP 2** ; `wind.log` relevé par le CE :
> relève des deux comptes et vidange tracées, sans sujet ni adresse).
> Commits `152a382` (E1), `15c312e` (E2+E3), `62ae2f7` (E4), `17b3009`
> (E5), `67ad6e0` (E6-E9), `75f9408` (revue, dix corrections) ; CI verte
> run 33568895402 ; journal **A106** ; **ADR 0030**. Release : 0.16.0
> (décision B, entrée CHANGELOG écrite à la clôture).
>
> **Chiffres kaizen** (session unique 145b0508, qui couvre AUSSI l'audit
> complet et la vague 0) : 11,8 h de mur, 26 prompts CE, 642 tours,
> ≈ 42,7 M d'équiv. input fil principal + 34,3 M sur 24 agents (six
> auditeurs, huit angles de revue, dix vérificateurs) ; **6 gates
> complètes jouées pour la vague 1** (E1, E2+E3, E4, E5, E6-E9, finale)
> + 6 pre-push ; mur bloqué sur commandes > 30 s : 235 min sur la
> semaine (e2e 148 min — la gate par étape se paie).
>
> Ouvert le 2026-09-01 (soir), à la suite de
> [AUDIT-2026-09-01.md](AUDIT-2026-09-01.md) §5 vague 1, vague 0
> livrée le même jour (`12114d6`, `0f1ed96`, `fb8c671`, `3097d22`).
> Neuf lots, tous des **S1 ou S2 de perte de données, de gel ou de
> silence** ; aucune surface UI neuve. Livraison prévue en **0.16.0**
> avec la vague 0 (décision CE B du 2026-09-01 : pas de 0.15.1,
> clause de réouverture : un gel au clic de lien ou un pixel HTTP
> signalé par un testeur ⇒ 0.15.1 le jour même).
>
> Principe directeur (CE) : **la chose la plus simple et la plus sûre
> qui fonctionne.**

## Constat (instruction sur pièces, 2026-09-01 — chaque fait re-vérifié à la main)

### C1 — Aucune garde mono-instance
`main.rs:96-97` nomme le risque (« deux pompes concurrentes mettraient
en quarantaine les envois l'une de l'autre ») ; les verrous
`outbox_flush`/`drafts_push`/`bodies_backfill` sont par processus ;
aucun plugin single-instance (`Cargo.toml`), aucune mention en
DETTE/ETAT/ADR. `fs4` 0.13 est déjà en dépendance (`commands.rs:1368`,
garde d'espace) et expose `try_lock_exclusive`.
**Décision CE du 2026-09-01 : verrou fichier, pas de plugin.**

### C2 — Une boîte vidée redevient « synchro initiale »
`sync.rs:99` `if state.last_uid == 0 { initial_sync }` ; `sync.rs:106`
`last_uid = max_uid` ; `store.rs:3639` `COALESCE(MAX(uid), 0)`. INBOX
archivée à zéro ⇒ relève suivante en `SyncMode::Initial` ⇒
`notify.rs:50` ne bulle rien, `initial_sync` repaie `list_uids` +
fetch complet (sans `remove_absent`). Le test `sync.rs:732` fige le
mécanisme sans voir la conséquence. La décision initial/incrémental/
reset est inline avec `select`, `record_remote_total`,
`replay_actions` (`sync.rs:57-115`) — pas de `plan_sync` pur.

### C3 — Une action refusée à jamais bloque le journal d'une boîte, en silence
`sync.rs:229` `Err(_) => break` ; `pending_actions(id, mailbox_id,
uid, kind)` sans `attempts` ni `last_error` (`store.rs:141-146`) ;
`error.rs` `Server(String)` sans Transient/Permanent (la distinction
existe pour SMTP, `transport.rs:19-29`) ; `SyncReport` sans champ
d'erreur ; `faut_relever` (`sync.rs:395`) force la relève à chaque
cycle tant que la file n'est pas vide. `store.rs:2061-2067` : une
ligne `pending_actions` au `kind` inconnu fait échouer TOUT
`pending_actions(mailbox_id)`.

### C4 — Purges non atomiques, trois listes divergentes
`reset_mailbox` `store.rs:1455-1522` (9 DELETE/UPDATE autocommit puis
`rebuild_account`, appelé nu par `sync.rs:69`) ; `remove_local`
`store.rs:1968-2005` (7 DELETE + `thread::refresh` autocommit hors
`echo.rs` ; appelants `store.rs:1835-1837` par UID = N×8 fsync,
`store.rs:3532-3536`) ; `remove_absent` `store.rs:1942-1956` ne purge
que 3 tables sur 8 (orphelins : `attachments`, `invitations`,
`images_messages`, `mis_de_cote`, `kiosque_lus`). `set_thread_scope`
`1268-1276` et `set_recipients` `2523-2546` idem.

### C5 — 17 commandes `async` hors `hors_pompe`, garde aveugle
`garde-thread-principal.mjs:100-106` saute toute commande `async`.
Liste (`commands.rs`) : `connect_accounts` 174/204/247, `add_*_account`
344/520, `reconnect_account` 376, `remove_account` 560/581,
`sync_inbox` 951/1039, `sync_inbox_light` 1077/1168, `message_body`
2047/2132, `save_attachment` 2407/2419 (`std::fs::write`),
`reply_context` 3670, `reply_all_context` 3763, `forward_context`
3833, `flush_outbox` 3956, `sync_apres_geste` 4058,
`fetch_source_attachment` 5170-5245, `sync_drafts` 5299,
`backfill_bodies` 5967, `telecharger_et_lancer` 6269. Aucune ne tient
`AppState.commandes`. Plus : `sync_apres_geste` `4086`/`4109` laisse
`en_vol` levé à vie sur un `?` ; empoisonnement condamnant à vie à
`5590`, `4365`, `5316`, `5984` contre `into_inner` à `5633`, `839`,
`4077`.

### C6 — Veille IDLE : deux lectures non bornées par tour
`mail-imap/src/lib.rs:478-493` ; vérifié dans
`imap-3.0.0-alpha.15/src/extensions/idle.rs:225-247` : `init()` avant
la pose du timeout, `set_read_timeout(None)` en sortie, `Drop →
terminate()` lit la réponse au `DONE` sans borne. La crate n'expose
pas la socket (`client.rs:164` `pub(crate)`), mais `ImapServer`
enrobe lui-même le `TcpStream` avant TLS (`lib.rs:88-114`) : un
`try_clone()` gardé dans la struct partage les options socket.

### C7 — SMTP : 535 en plein flush ⇒ `Permanent`, `References` tronqué, pas de discriminant
`mail-smtp/src/lib.rs:8-11` (doc) vs `lettre` sans `pool` (chaque
`send` rouvre + ré-authentifie) et `lib.rs:120` tout 5xx ⇒ Permanent
⇒ `record_rejection`. `lib.rs:206-208` `.references(parent)` seul
(RFC 5322 §3.6.4 veut la chaîne). `lib.rs:95-107` tout ⇒ Transient et
le shell fait `Err(_) => refresh` (`commands.rs:5466`).

### C8 — OAuth : refresh token renouvelé jeté, attente sans limite, `Debug` avec secrets
`mail-auth/src/lib.rs:183-185` `store_refresh: None` (Azure AD renvoie
un jeton neuf, l'ancien expire à 90 j) ; `flow.rs:193-218` `incoming()`
bloquant, `read_line` sans timeout, `Err` final inatteignable ;
`flow.rs:111-113` « ouvrez manuellement » quand le listener est déjà
tombé ; `lib.rs:39,49,63` `#[derive(Debug)]` sur `access_token`,
`password`.

### C9 — Traces : sujet des envois contre §6.8, `eprintln!` perdus en release
`commands.rs:4429-4432` (`message.subject` dans la trace de vidange,
sous un commentaire qui invoque §6.8 — STANDARD:364 « ni sujet, ni
expéditeur ») ; `eprintln!` à `1514`, `4285`, `4414`,
`veilleur.rs:104/113/150/173` : sous `windows_subsystem = "windows"`
rien ne survit ; `trace_maj` (`6392-6405`, `maj.log` à côté de la
base) est le patron qui marche.

## Périmètre — refus explicites (§2.6)

- **Pas d'UI neuve** au-delà d'une ligne dans la fente d'avis
  (D2) : « Réessayer / Abandonner » une action refusée, la page des
  refus, l'ordonnanceur de synchro côté shell, `agir_groupe`, le
  `Menu.svelte` unique → vagues 2-3.
- **Pas de connexion SQLite partagée** (2.1 de l'audit) ni de
  `Store::open_leger` : mesure due sur vraie base d'abord (§9).
- **Pas de pile TLS unique, pas de `withGlobalTauri: false`** :
  décisions CE ouvertes, vagues 2-3.
- **Pas de focus de la fenêtre existante** à la seconde instance
  (sans plugin, ce serait `FindWindowW` + `SetForegroundWindow`, deux
  appels Win32 de plus pour un cas rare) — sauf D1 contraire.
- **CONDSTORE absent** (drapeaux jamais resynchronisés) : décision CE
  ouverte, hors vague.
- **`References: None` stocké vide** (`backfill.rs:285`) : à confirmer
  sur `mail-imap` d'abord (vague 2).

## Étapes (ordre : la mono-instance d'abord — décision CE — puis le cœur, puis les adaptateurs, puis les traces)

Chaque étape : RED montré, GREEN, boucle intérieure sur ses seuls
tests ; **une gate complète après E5** (la plus large) et **une avant
le commit final**. Un seul commit par étape, ou un par paire d'étapes
courtes (E7+E8, E9 seul).

### E1 — Mono-instance par verrou fichier (`fs4`)
- `apps/desktop/src/instance.rs` : `fn verrouiller(dossier: &Path) ->
  Result<GardeInstance, DejaOuvert>` — ouvre `wind.lock` à côté de
  `wind.db` (même dossier que `db_path`, donc `WIND_DB_PATH` isole les
  e2e et la sonde), `try_lock_exclusive` ; la garde vit dans
  `AppState` (libérée à la sortie, et par l'OS sur un crash — jamais
  de fichier « collant »). Décision pure et testable : le verrou seul,
  aucun Tauri.
- `main.rs` : après `demenager()`, avant toute base. Seconde instance :
  `MessageBoxW` « Wind est déjà ouvert. » puis `exit(0)` (D1) — le
  patron du déménagement (`main.rs:193-196`) gagne au passage sa
  `MessageBoxW` (S3 de l'audit : son `eprintln!` est invisible en
  release).
- **RED** : test Rust `deux_verrous_sur_le_meme_dossier_le_second_est_refuse`
  (deux `File` sur le même chemin : le second `try_lock_exclusive`
  rend `false` — vrai sur Windows par handle) ; `un_verrou_relache_se_reprend`.
  Preuve terrain : double lancement ⇒ une fenêtre + le message.

### E2 — `plan_sync` pur, boîte vidée ≠ synchro initiale
- `sync.rs` : `pub(crate) fn plan_sync(etat: Option<&SyncState>,
  instantane: &Snapshot) -> SyncPlan { Reset, Initial, Incremental
  { modseq } }` — pur, décidé sur **`SyncState.initialisee`** (bool,
  colonne `sync_state.initialisee INTEGER NOT NULL DEFAULT 0` dans
  `SCHEMA` + `add_missing_columns` ; posée à 1 après la première
  synchro réussie ; bases existantes : `UPDATE … SET initialisee = 1
  WHERE last_uid > 0` à la migration — les lignes à 0 restent « pas
  encore initialisées », comportement d'avant). `last_uid == 0` ne
  décide plus rien.
- **RED** : `une_boite_videe_reste_en_incremental_et_bulle`
  (add(1) → sync → expunge(1) → sync → add(2) → sync ⇒ `mode ==
  Incremental`, `arrivals_to_notify` non vide) ; tests unitaires de
  `plan_sync` (état absent, UIDVALIDITY changée, initialisée sans
  modseq, initialisée avec modseq).

### E3 — Actions refusées : quarantaine, jamais un blocage silencieux
- `error.rs` : `Server { kind: ServerErrorKind::{Transient, Permanent},
  detail: String }` ; `mail-imap` classe (BAD/NO nommés « TRYCREATE »,
  dossier inexistant, `[CANNOT]` ⇒ Permanent ; réseau/timeout/`[INUSE]`
  /`[OVERQUOTA]` ⇒ Transient — la garde D-17 vit là).
- `pending_actions` : colonnes `attempts INTEGER NOT NULL DEFAULT 0`,
  `refusee INTEGER NOT NULL DEFAULT 0`, `last_error TEXT` ; index
  `(mailbox_id, uid)` (le scan quadratique de `nettoyage_verdict`
  tombe avec).
- `replay_actions` : Permanent ⇒ `refusee = 1` (sortie de file, le
  reste CONTINUE) ; Transient ⇒ `attempts += 1`, `break` (comme
  aujourd'hui) ; `attempts >= 5` ⇒ `refusee = 1` (D2 pour le seuil).
  `SyncReport.refusees: usize`. `pending_actions()` ignore une ligne
  au `kind` illisible en la marquant `refusee` avec `last_error`,
  jamais `Err(Corrupt)` sur toute la file. `faut_relever` ne compte
  que les non refusées.
- Surface : une ligne dans la fente d'avis « N actions refusées par
  le serveur » (compteur `actions_refusees` dans `nav_snapshot`,
  texte catalogues fr/en, sans bouton) — D2.
- **RED** : `une_action_refusee_ne_bloque_pas_les_suivantes`
  (FakeServer refuse `MoveTo` en Permanent, `MarkSeen` suivant
  rejoué), `cinq_echecs_transitoires_mettent_en_quarantaine`,
  `une_ligne_illisible_ne_fait_pas_echouer_la_file`,
  `faut_relever_ignore_les_refusees`.

### E4 — Purges atomiques, une seule liste
- `store.rs` : `fn purger_message(tx: &Connection, mailbox_id, uid)`
  — LA liste des 8 tables ; `remove_local`, `reset_mailbox`,
  `remove_absent` l'appellent ; `remove_local(&Connection)` exige la
  transaction de l'appelant (`upsert_envelopes` regroupe ses retraits
  dans SA transaction ; `nettoyage_verdict` idem ; `sync.rs:69`
  enveloppe `reset_mailbox`). `set_thread_scope`, `set_recipients` sous
  `unchecked_transaction`.
- **RED** : `un_message_disparu_du_serveur_ne_laisse_aucun_orphelin`
  (8 tables comptées après `remove_absent`),
  `reset_mailbox_est_atomique` (échec injecté dans `rebuild_account`
  ⇒ `unified_count == page.len()`, aucun `threads.inbox_size > 0`
  sans enveloppe).

### E5 — Garde du thread principal étendue, 17 commandes migrées, RAII, `into_inner`
- `garde-thread-principal.mjs` : pour une commande `async`, le corps
  hors `hors_pompe(`/`spawn_blocking(` ne contient aucun `Store::`,
  `db_path(`, `lock_accounts(`, `auth_for(`, `std::fs`, `keyring`,
  `sanitize_with(` — **RED d'abord** sur l'état actuel (17 rouges
  nommés), puis GREEN commande par commande. Exemptions nommées et
  justifiées comme les 7 `PURES`.
- Patron : `hors_pompe` (lecture) → `spawn_blocking` nu (réseau) →
  `hors_pompe` (écriture) — `fetch_source_attachment` en modèle.
  `save_attachment` : `std::fs::write` sous `hors_pompe`.
- `sync_apres_geste` : `struct Vol<'a>` avec `Drop` sur
  `passes_geste`. `lock_accounts`, `run_flush_all`, `run_draft_sync_all`,
  `run_backfill_all` : `into_inner()` (un panic ne condamne plus, ADR
  0019 tenu) — le panic est déjà consigné par la télémétrie.
- **RED** : la garde elle-même (prouvée rouge sur `main` d'avant E5) ;
  test Rust `en_vol_retombe_meme_si_la_passe_echoue` (le `?` de
  `reposer_sessions` simulé). Mesure : `sonde-gel.py` 60 s avec
  ouverture d'un corps de 28 Mo (D-1) pendant une relève — 0 gel.
- **Gate complète ici.**

### E6 — Veille IDLE bornée
- `mail-imap/src/lib.rs` : `ImapServer { socket: TcpStream (clone) }`
  pris dans `connect_client` avant l'enrobage TLS ; `veiller` pose
  `set_read_timeout(Some(IO_TIMEOUT))` à l'entrée ET après
  `wait_while` (couvre `init()` du tour suivant et le `DONE` du
  `Drop`). Doc-comment `466-476` rendu vrai.
- **RED** : test `un_serveur_qui_acquitte_et_se_tait_rend_la_main`
  (listener local : `* OK`, login, `+ idling`, puis silence ⇒
  `veiller(relance = 200 ms)` rend `Echeance`/`Err` sous 5 s ; sans le
  correctif le test pend — borné par un timeout de test de 10 s).

### E7 — SMTP : 53x transitoire, `References` complet, réseau ≠ auth
- `mail-smtp/src/lib.rs` : `match err.status()` 530/534/535/538 ⇒
  `Transient` ; `pub fn is_connection_error(&str) -> bool` jumeau de
  `mail_imap`, préfixe « connexion hôte:port : » ; le shell ne fait
  `refresh` que sur une erreur d'auth (`commands.rs:5466`).
- `OutboxMessage.references: Option<String>` ; `compose` le remplit
  (« `References` du parent + `Message-ID` du parent », RFC 5322
  §3.6.4) depuis la base (`thread_headers`) ; l'adaptateur recopie.
- **RED** : `un_535_en_plein_flush_est_transitoire`,
  `references_porte_la_chaine_entiere` (3e message d'un fil),
  `une_panne_reseau_smtp_n_est_pas_un_refus_d_auth`.

### E8 — OAuth : refresh token renouvelé, attente bornée, `Debug` masqué
- `mail-auth/src/lib.rs:183` : si `tokens.refresh_token()` est
  `Some` et différent ⇒ `set_password` (3 lignes).
- `flow.rs` : `listener.set_nonblocking(true)` + boucle `accept` avec
  échéance **5 min** (D3) et `set_read_timeout(2 s)` ; le repli
  « ouvrez manuellement » garde le listener vivant jusqu'à l'échéance.
- `impl Debug` manuels : `access_token: "<masqué>"`, `password:
  "<masqué>"`.
- **RED** : `un_refresh_token_renouvele_est_stocke` (double
  `Authenticator` avec coffre de test), `l_attente_de_redirection_expire`
  (échéance 200 ms en test), `debug_ne_montre_aucun_secret`.

### E9 — Traces sans PII, un seul fichier
- `apps/desktop/src/trace.rs` : `pub fn trace(app_dir: Option<&Path>,
  ligne: &str)` — le patron de `trace_maj` généralisé : `eprintln!` +
  append daté dans `wind.log` à côté de la base, **tronqué à 1 Mo**
  (D4) ; `trace_maj` devient un appel. Les `eprintln!` de
  `commands.rs` (relève, passe geste, vidange) et `veilleur.rs` passent
  par lui. La vidange trace `id`, tentatives, erreur — **jamais le
  sujet** ; test `la_trace_de_vidange_ne_porte_ni_sujet_ni_expediteur`
  (fonction pure `ligne_vidange(&OutboxMessage, &Issue) -> String`).
- **RED** : le test ci-dessus + `la_trace_est_bornee_a_un_mega`.

## Livraison

- **E1 — livrée le 2026-09-01 (soir).** `apps/desktop/src/instance.rs`
  (`verrouiller`, `dossier_de_la_base`, garde RAII), pris dans `main`
  AVANT `Builder::build` — la fenêtre naît avant `setup`
  (tauri-2.11.5 `app.rs:2524` puis `2531`), une vérification dans
  `setup` aurait fait clignoter une fenêtre. Message par `rfd`
  (déjà compilé via tauri-plugin-dialog, `default-features = false`,
  zéro feature de plus) ; l'échec du déménagement gagne la même boîte
  (son `eprintln!` était invisible en release). TDD : RED de
  compilation sur trois tests, GREEN 3/3
  (`deux_verrous_sur_le_meme_dossier_le_second_est_refuse`,
  `un_verrou_relache_se_reprend`, `le_dossier_absent_est_cree`).
  Preuve du geste (release, `WIND_DB_PATH` = base Clarity) : A =
  `Tauri Window` vivante ; B = un seul `#32770` « Wind », stderr
  « Wind est déjà ouvert. », **sortie 0** à la fermeture du dialogue ;
  A toujours vivante. `demenagement::IDENTIFIANT` passe `pub(crate)`
  (une seule copie de l'identifiant côté Rust).

- **E2 — livrée le 2026-09-01 (soir).** `sync.rs` : `SyncPlan::{Reset,
  Initial, Incremental{modseq}}` et `plan_sync(etat, instantané)` pur ;
  `sync()` ne fait que l'exécuter. `mailboxes.initialisee` (SCHEMA +
  `add_missing_columns`, UNE fois : `UPDATE … SET initialisee = 1 WHERE
  last_uid > 0` à la pose de la colonne — les lignes à 0 gardent le
  comportement d'avant), posée à 1 par `update_state`. TDD : RED de
  compilation (2 tests), GREEN 425/425 mail-core ; **le scénario prouvé
  en le cassant** — décision sabotée en `last_uid == 0` ⇒ `left:
  Initial, right: Incremental`, restaurée ⇒ vert. Clippy workspace
  propre. Le filet D-36 (colonnes saines) a vu passer la colonne neuve.

- **E3 — livrée le 2026-09-01 (soir).** Le plus simple et sûr :
  `Error::Server(String)` reste (transitoire par défaut — on retente),
  une variante **`Error::Refus`** naît pour le refus explicite ;
  `mail-imap::server_err` y range `imap::Error::{No, Bad}` (dossier
  disparu, `[CANNOT]`, `[TRYCREATE]`), tout le reste (I/O, TLS,
  connexion perdue, réponse inattendue) reste `Server`.
  `pending_actions` : colonnes `attempts`, `refusee`, `last_error` +
  index `(mailbox_id, uid)` (le scan quadratique de `nettoyage_verdict`
  tombe avec) ; `replay_actions` rend (rejouées, refusées) — `Refus` ⇒
  quarantaine immédiate et le rejeu CONTINUE ; transitoire ⇒
  `attempts + 1`, `break`, quarantaine au 5e (`SEUIL_QUARANTAINE`, D2) ;
  une ligne au `kind` illisible est mise en quarantaine avec son motif,
  jamais fatale. `has_pending_actions`, `sync_progress`, l'anti-doublon
  de la règle du Non, les trois requêtes d'`echo.rs` (balayage,
  dossiers/comptes avec travail) sont aveugles aux refusées.
  `SyncReport.refusees`. Surface (D2) : `OutboxStatus.actions_refusees`,
  `avisRefus` dans la fente (alerte, glyphe `error`, sans bouton,
  priorité juste après l'échec d'envoi), catalogues fr/en avec pluriel
  `|` — Système **A106**. TDD : RED de compilation (3 tests), GREEN
  428/428 mail-core, 70/70 mail-imap, clippy propre, build ui-v2 0
  avertissement, contraste 440 paires, cohérence 68 jetons. Limite
  dite : la ligne de la fente n'a pas de scénario e2e (aucun décor ne
  sait faire refuser une action au serveur) — couverte par le cœur et
  la parité des catalogues ; à voir au terrain (STOP 2).

- **E4 — livrée le 2026-09-01 (soir).** Le constat s'est affiné à la
  lecture : `geste_avec_echo` ET `nettoyage_verdict` enveloppaient déjà
  `remove_local` dans leur transaction ; seuls `upsert_envelopes`
  (retraits de la règle du Non, après commit, un autocommit par
  message) et `reset_mailbox` étaient nus. Livré : `purger_message(conn,
  mailbox_id, uid)` — LA liste des sept tables par message (fil relevé
  avant, rafraîchi après) — appelée par `remove_local`, `remove_absent`
  (qui en oubliait cinq : `attachments`, `invitations`,
  `images_messages`, `mis_de_cote`, `kiosque_lus`) et les retraits
  d'`upsert_envelopes` (désormais en UNE transaction) ; `remove_local`
  ouvre sa transaction seulement s'il n'en a pas (`is_autocommit`),
  sinon vit dans celle de l'appelant ; `reset_mailbox`,
  `set_thread_scope`, `set_recipients` sous `unchecked_transaction`.
  TDD, **RED prouvé par un déclencheur SQLite** (`RAISE(ABORT)` sur la
  suppression des enveloppes = panne au milieu de la purge) : avant,
  6 tables sur 7 déjà effacées quand la panne survient ; après, les 7
  intactes et l'UIDVALIDITY inchangée. `un_message_disparu_du_serveur_
  ne_laisse_aucun_orphelin` : 5 tables orphelines avant, 0 après.
  Au passage, une première version laissait `set_recipients` sans
  `commit` — deux tests d'annuaire l'ont attrapé (rollback au drop).
  Tests mail-core 428 → 431, clippy propre.

- **E5 — livrée le 2026-09-02 (nuit).** **La garde d'abord, RED** :
  `garde-thread-principal.mjs` retire du corps d'une commande `async`
  les appels `hors_pompe(…)`/`spawn_blocking(…)` (parenthèses
  équilibrées) et refuse dans la glu restante `Store::`, `std::fs`,
  `File::`, `keyring`, `auth_for(`, `connected_jobs(`,
  `account_email(`, `mail_render::sanitize`, `connect_imap(`,
  `trace_maj(` — **17 rouges nommés sur `main` d'avant**, le compte de
  l'audit. Puis GREEN : `db_path` devient une lecture pure (`OnceLock`,
  dossier créé au premier appel — il quitte les marqueurs de glu, reste
  interdit aux exemptées) ; `connected_jobs(app)` et `auth_for(app,
  id)` sur l'`AppHandle`, appelés SOUS `hors_pompe` ; `raw_body`
  (cache + session sous `hors_pompe`, réseau nu), `citation_reply` et
  `forward_context` (assainissement sous le verrou : du CPU, un corps
  de 28 Mo), `message_body` (trois lectures + assainissement sous le
  verrou), `save_attachment` (écriture disque sous le verrou),
  `fetch_source_attachment` (lecture → réseau → écriture, plus de
  connexion SQLite tenue à travers l'attente réseau — le TOCTOU nommé
  par l'ADR 0019), `connect_accounts`/`reconnect_account`/
  `add_generic_account`/`remove_account` (base et sessions sous le
  verrou, OAuth/IMAP/coffre en `spawn_blocking` nu), les cinq boucles
  (`sync_inbox`, `sync_inbox_light`, `flush_outbox`, `sync_drafts`,
  `backfill_bodies` : `connected_jobs` sous le verrou, la boucle réseau
  reste nue — l'audit 2.3 « verrou global ≠ boucles réseau » reste à
  confirmer sur traces, hors vague), `solder_releve` partagé par les
  deux cycles. **`VolGarde`** (RAII) : `en_vol` retombe à la
  libération, `?` compris ; test `le_vol_retombe_quand_la_garde_est_
  relachee_meme_par_une_sortie_precoce` (RED sans enseignement : c'est
  le `Drop`). **`into_inner` uniforme** : `lock_accounts` et les trois
  verrous de boucle (vidange, brouillons, rattrapage) ne condamnent
  plus jusqu'au redémarrage (ADR 0019 tenu ; le panic est consigné par
  la télémétrie). Cinq commandes perdent un paramètre `state` devenu
  inutile ; `envelope_of` meurt (remplacé par `enveloppe_et_compte`).
  Garde : **110 commandes vérifiées, 0 défaut** ; clippy propre ;
  tests desktop 27 → 28. Mesure due au STOP 2 : `sonde-gel.py` 60 s
  avec ouverture d'un corps lourd pendant une relève.

- **E6 — livrée le 2026-09-02 (nuit), après deux fausses pistes que
  les tests ont tuées.** (1) Le plan disait « socket clonée +
  `set_read_timeout` » : le test de la propriété a **PENDU** — sur
  Windows, `SO_RCVTIMEO` est propre au handle, un clone
  (`WSADuplicateSocket`) n'en hérite pas. (2) Chien de garde qui fait
  `shutdown()` par le clone : la lecture bloquée n'est revenue qu'après
  120 s, pas 200 ms — le `shutdown` d'un handle dupliqué n'interrompt
  pas non plus un `recv` en cours. Le remède qui tient, et le plus
  simple : **`FluxBorne`**, notre propre type de flux que la crate
  emballe (`ImapConnection` est un trait public à impl blanket) —
  `set_read_timeout(None)` y vaut le PLANCHER (`IO_TIMEOUT`, 120 s) :
  la crate ne peut plus désarmer la borne en sortie de veille, le `+`
  de l'IDLE suivant et la réponse au `DONE` sont bornés sur le MÊME
  handle qui lit. Tests : `un_flux_borne_refuse_de_perdre_sa_borne`
  (None ⇒ plancher, explicite ⇒ tel quel) et
  `une_lecture_sur_un_serveur_muet_expire_au_plancher` (le geste de la
  crate, puis un serveur muet : la lecture expire à 200 ms). Tests
  mail-imap 70 → 72, clippy propre. Piège gravé : jamais de timeout ni
  de shutdown par un clone de socket sur Windows.

- **E7 — livrée le 2026-09-02 (nuit).** `mail-smtp` : `classer_echec`
  pure (530/534/535/538 ⇒ transitoire même en 5xx — c'est la session
  qu'il faut refaire, pas le message ; les autres 5xx restent
  définitifs) ; `test_transport` préfixe « connexion : » (aucune réponse
  du serveur) ou « authentification : » ; `is_connection_error` jumeau
  de mail-imap, et le shell (`connect_smtp`) ne refait la session OAuth
  que sur un refus d'authentification. `References` : `Draft.references`
  / `OutboxMessage.references` / colonne `outbox.refs` (**pas
  `references` : mot réservé SQLite — toutes les bases refusaient de
  s'ouvrir, 400 tests rouges d'un coup**), `Store::references_de` (refs
  du parent + son Message-ID, RFC 5322 §3.6.4) posée à la composition
  (`queue_send`, réponse iTIP) ; l'adaptateur recopie, parent seul en
  repli. TDD : RED de compilation (4 tests), GREEN — mail-core 431 → 432,
  mail-smtp 26 → 29, clippy propre.

- **E8 — livrée le 2026-09-02 (nuit).** `authenticate_silent` stocke le
  refresh token RENOUVELÉ s'il diffère (Azure AD en renvoie un neuf à
  chaque échange et fait expirer l'ancien à 90 j — déconnexion
  silencieuse différée ; à confirmer au terrain sur un compte
  Microsoft, le coffre ne se simule pas en test) ; `wait_for_redirect`
  en `accept` non bloquant avec échéance **5 min** (D3) et lecture
  bornée à 2 s par connexion (une connexion muette — sonde,
  pré-ouverture du navigateur — n'immobilise plus l'attente) ; `Debug`
  manuels sur `Authenticated` et `GenericCredentials` (`<masqué>`).
  Tests : `l_attente_de_redirection_expire`,
  `une_connexion_muette_n_immobilise_pas_l_attente`,
  `debug_ne_montre_aucun_secret` — mail-auth 21 → 24. Limite dite : le
  repli « ouvrez manuellement » (`BrowserFallback`) rend toujours la
  main sans attendre — cas rare (aucun navigateur), inchangé.
- **E9 — livrée le 2026-09-02 (nuit).** `apps/desktop/src/trace.rs` :
  `trace(ligne)` = stderr + append daté dans `wind.log` à côté de la
  base, **tronqué au méga** (D4) ; dossier posé dans `main` avant tout
  geste. Les `eprintln!` de la relève, de la passe d'après-geste, de
  la vidange, de l'horizon illisible et des quatre du veilleur passent
  par lui ; la ligne des envois en attente perd le SUJET (§6.8).
  `maj.log` garde son fichier (la mesure de MAJ que le CE attend).
  Tests `la_trace_est_bornee_a_un_mega`, `chaque_ligne_est_datee` —
  desktop 28 → 30.

## Gate & terrain

- Boucle intérieure : `cargo test -p <crate> <nom>` par étape ;
  `garde-thread-principal.mjs` seul pour E5.
- Gates complètes jouées : E1, E2+E3, E4, E5, E6-E9 (toutes vertes,
  e2e 187/187), puis la finale avant le dernier commit ;
  `/code-review high` sur l'ensemble de la vague avant ce commit.

## Revue à regard neuf (2026-09-02, `/code-review high` sur 3097d22..67ad6e0)

Huit angles (Sonnet) / ~27 candidats / 16 dédoublonnés / 10 vérifiés
un à un (8 CONFIRMED, 2 PLAUSIBLE, 3 REFUTED) — **10 corrigés** :

1. **Course déménagement / verrou (CONFIRMED)** — le verrou mono-
   instance ne PEUT pas précéder `demenager()` (il créerait le dossier
   cible et ferait sauter la migration) ; deux lancements concurrents
   sur un poste Discovery faisaient échouer le second `rename` avec
   « Échec du déménagement ». `rename_tolerant` : un `rename` raté
   alors que la cible existe et que la source a disparu est un succès.
   Test `un_rename_perdu_contre_l_autre_instance_est_un_succes`.
2. **`veilleur::reconcilier` sur verrou empoisonné (CONFIRMED)** —
   retour silencieux, plus aucun veilleur démarré ni arrêté ; repris
   (`into_inner`) comme `lock_accounts`.
3. **Adresse dans `wind.log` (CONFIRMED, §6.8)** —
   `AuthError::Vault("aucun jeton pour {email}")` tracé par quatre
   sites ; corrigé à la source (« aucun jeton au coffre pour ce compte »).
4. **Refusée éternelle (CONFIRMED)** — rien ne retirait une action en
   quarantaine tant que son message existait : la ligne de la fente ne
   pouvait que croître. Un geste neuf sur le même message REMPLACE ses
   refusées (`enqueue_action`, `geste_avec_echo`) — sans toucher à D2.
   Test `un_nouveau_geste_remplace_l_ancienne_refusee`.
5. **`thread::refresh` par message (CONFIRMED)** — `purger_message`
   rend le fil sans rafraîchir ; `remove_absent` et les retraits du Non
   dédoublonnent par fil (~500× sur un fil de 500 disparus).
6. **`SyncReport.refusees` jamais tracé (CONFIRMED)** — une ligne dans
   `relever_inbox`, point de sortie unique des quatre chemins.
7. **Quatrième copie de la liste des tables (CONFIRMED)** —
   `TABLES_PAR_MESSAGE` partagée par `purger_message` et `reset_mailbox`.
8. **`message_body` en deux prises du verrou (CONFIRMED)** — une seule
   (`vue_du_corps`), le réseau seul reste nu ; `reply_*`/`forward`
   (3 prises, chemins rares) laissés — vague 2.
9. **Premier `db_path` hors `setup` (PLAUSIBLE)** — appelé une fois
   dans `.setup`, thread principal, avant la fenêtre.
10. **`noter_echec_action` UPDATE + SELECT (PLAUSIBLE)** — `RETURNING`
    (SQLite embarqué 3.50.2).

Réfutés : `fetch_body` écrit hors verrou (état antérieur, consigné à
l'audit 2.3), refresh OAuth sur `Refus` (comportement identique à
avant), index sur `refusee` (table petite par construction).
Candidats de propreté non retenus (vague 3) : `into_inner` ×7 → une
aide ; `session_de(app, id)` ×4 ; `trace` / `trace_maj` deux writers ;
`is_connection_error` dupliqué IMAP/SMTP ; `dossier_de_la_base` vs
`db_path` (deux sources du chemin, sans test qui les lie) ;
`sync_inbox`/`sync_inbox_light` toujours jumeaux ; `remove_local` à
deux chemins (`is_autocommit`) ; `compose()` sans `references` ;
`SEUIL_QUARANTAINE` dans le Store.

Tests après revue : mail-core 432 → 433, desktop 30 → 31, mail-auth 24,
clippy propre, garde 110/0.

## STOP 2 — checklist de terrain (CE)

Préparer le poste (build release + trace) :

```powershell
powershell -ExecutionPolicy Bypass -File scripts\terrain.ps1
```

```powershell
powershell -ExecutionPolicy Bypass -File scripts\lancer-wind.ps1
```

1. **E1 — mono-instance.** Wind ouvert, lancer une seconde fois
   `target\release\wind-desktop.exe` (double-clic) : une boîte
   « Wind est déjà ouvert. », aucune seconde fenêtre, la première
   intacte. Fermer la boîte : le processus a disparu.
2. **E2 — boîte vidée.** Sur un compte de test, archiver TOUT ce que
   la Réception montre, relever (bouton), puis s'envoyer un message
   depuis un autre compte : **la bulle apparaît**. Avant : silence.
3. **E3 — action refusée.** Créer un dossier « Temp » côté webmail,
   relever, déplacer un message vers « Temp » depuis Wind HORS LIGNE
   (Wi-Fi coupé), supprimer « Temp » côté webmail depuis le téléphone,
   revenir en ligne, relever : la fente d'avis dit **« 1 action
   refusée par le serveur… »**, le message est resté en Réception, et
   un marquage lu/non-lu fait juste après **passe** (contrôle au
   webmail). Avant : rien de dit, et plus aucun geste de cette boîte
   ne passait.
4. **E5 — gel.** Sonde 60 s, pendant laquelle : ouvrir le plus gros
   message connu (une infolettre lourde) PENDANT une relève (bouton),
   puis cliquer un lien. Attendu : « OK : aucun gel > 150 ms ».
   **Jamais pendant une gate.**

```powershell
python e2e\sonde-gel.py C:\mesure\clarity.db 60
```

5. **E6 — veille bornée.** Wind ouvert avec IDLE actif (compte
   Gmail), mettre le poste en veille 5 min, le réveiller : dans les
   2 min, un message envoyé depuis le téléphone arrive avec sa bulle.
   `wind.log` montre « veilleur compte N : reconnexion dans … s ».
6. **E7 — fil chez le destinataire.** Répondre au 3e message d'un fil
   reçu depuis Gmail, et vérifier dans Gmail que la réponse reste DANS
   la conversation (avant : elle pouvait la casser).
7. **E8 — attente OAuth.** Réglages > Comptes > ajouter un compte
   Google, FERMER l'onglet de consentement sans répondre : au bout de
   5 min, Wind rend « consentement non reçu en 5 min — relancez… » et
   le guichet est de nouveau utilisable (avant : commande gelée pour
   toujours).
8. **E9 — trace.** Après une relève et une vidange :

```powershell
Get-Content "$env:APPDATA\dev.elements.wind\wind.log" -Tail 20
```

   Attendu : lignes datées « relève compte … », « vidange : … » —
   **aucun sujet, aucune adresse** ; taille du fichier < 1 Mo.

Budgets à re-mesurer (STANDARD §3) : gel de la pompe 0 > 150 ms
(point 4) ; démarrage inchangé (`scripts\terrain.ps1` lit
`demarrage`).

## § Décisions CE — tranchées au STOP 1, le 2026-09-01 (soir)

**GO du CE le 2026-09-01.** Réponses mot pour mot :
- **D1** : « Message puis sortie ».
- **D2** : « Ligne dans la fente d'avis » (seuil de quarantaine
  transitoire : 5 cycles, tel que proposé).
- **D3** : « 5 minutes ».
- **D4** : « 1 Mo, tronqué ».

Énoncé des décisions telles que posées :

- **D1 — Seconde instance** : (a) message « Wind est déjà ouvert » puis
  sortie (recommandé : le plus simple, deux appels Win32 de moins) ;
  (b) message + tentative de mise au premier plan de la fenêtre
  existante.
- **D2 — Actions refusées** : (a) quarantaine dans le cœur + une ligne
  dans la fente d'avis « N actions refusées par le serveur », sans
  bouton (recommandé : l'intention n'est plus perdue en silence, l'UI
  de décision attend la vague 2) ; (b) cœur seul, trace et compteur
  dans `SyncReport`, rien à l'écran ; (c) fente + « Abandonner » par
  ligne dès maintenant. Seuil de quarantaine transitoire : **5**
  cycles (proposé).
- **D3 — Échéance de l'attente OAuth** : **5 min** (proposé ; au-delà,
  la commande rend « consentement non reçu » et le guichet repropose).
- **D4 — `wind.log`** : borné à **1 Mo**, tronqué (le fichier repart
  de zéro) — proposé ; alternative : rotation `wind.log.1`.
