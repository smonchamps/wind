//! Commandes Tauri : la passerelle entre l'UI et le noyau.
//!
//! Multi-comptes (Phase 3) : l'identité d'un message est `(compte, uid)`
//! — un UID seul ne suffit plus. Chaque opération réseau passe par la
//! connexion de SON compte ; les boucles (synchro, vidange, brouillons)
//! agrègent les comptes connectés. Le travail bloquant (OAuth, IMAP,
//! SMTP) passe par `spawn_blocking` pour ne jamais geler la fenêtre.

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mail_auth::{AccountSession, Authenticated, Authenticator, GenericCredentials};
use mail_core::AccountConfig;
use mail_core::{Action, MailServer, OutboxState, Store, SyncEngine};
use mail_imap::ImapServer;
use mail_smtp::SmtpMailer;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::{AppState, VolPasse};

pub(crate) const MAILBOX: &str = "INBOX";
const LIST_LIMIT_MAX: usize = 500;
const SEARCH_LIMIT: usize = 100;
/// Corps rapatriés par appel, tous comptes confondus. Borner le lot rend
/// l'interruption gratuite : l'UI cesse simplement de rappeler.
const BACKFILL_BUDGET: usize = 200;
/// En-têtes de fil rapatriés par compte et par synchronisation.
///
/// Généreux devant le budget des corps (200) parce que la dépense n'est
/// pas la même : un bloc d'en-têtes pèse ~3 ko contre ~50 ko pour un
/// message entier. Sur la boîte de l'utilisateur (~2 700 messages), deux
/// synchronisations suffisent à regrouper la boîte entière.
const THREAD_HEADER_BUDGET: usize = 2_000;
/// Destinataires rattrapés par compte et par synchronisation (R4/R1),
/// budget PARTAGÉ entre INBOX et Envoyés. Même dépense qu'un en-tête (une
/// ENVELOPE), et la portée est celle, déjà convergée, de la passe de fils :
/// la passe rattrape en quelques cycles, puis se tait.
const RECIPIENTS_BUDGET: usize = 2_000;
/// Arrivees remontees par compte pour les notifications. Au-dela, seul
/// le NOMBRE compte — la bulle resume de toute facon.
const NOTIFY_MAX_ARRIVALS: usize = 50;

#[derive(Serialize)]
pub struct AccountInfo {
    pub id: i64,
    pub email: String,
}

#[derive(Serialize)]
pub struct SyncSummary {
    /// Comptes synchronisés avec succès.
    pub accounts: usize,
    /// Comptes dont la relève ENTIÈRE a échoué (E3, échec partiel dit) :
    /// `errors` ne suffit pas à les compter — il porte aussi les
    /// incidents best-effort des comptes qui ont réussi.
    pub accounts_failed: usize,
    pub fetched: usize,
    pub deleted: usize,
    pub replayed: usize,
    pub elapsed_ms: u64,
    /// Échecs par compte — les autres comptes ne sont pas bloqués.
    pub errors: Vec<String>,
}

#[derive(Serialize)]
pub struct MessageRow {
    pub account_id: i64,
    pub account_email: String,
    /// La boîte qui contient ce message. **Indispensable** : les UID sont
    /// attribués par boîte et repartent de 1, donc l'UID seul ne désigne
    /// plus un message dès qu'un compte en synchronise deux. Toute action
    /// de l'UI la renvoie.
    pub mailbox: String,
    pub uid: u32,
    pub subject: String,
    pub sender: String,
    pub date: String,
    pub seen: bool,
    pub flagged: bool,
    pub has_attachment: bool,
    /// La conversation à laquelle appartient la ligne — c'est par elle
    /// qu'on demande le reste de l'échange.
    pub thread_id: Option<i64>,
    /// Messages de la conversation présents dans la boîte. 1 = isolé.
    pub thread_size: u32,
    /// Non-lus de la conversation : c'est LUI qui décide du gras, et non
    /// l'état du seul message affiché.
    pub thread_unseen: u32,
    /// Secondes Unix du message — la v2 formate l'heure côté client
    /// (« 09:12 », « Hier », « 5 août ») ; `date` reste la chaîne brute
    /// que la v1 affiche telle quelle. 0 = date inconnue.
    pub epoch: i64,
    /// COMBIEN de pièces jointes — la puce du prototype dit « 2
    /// fichiers ». 0 tant que le corps n'a pas été lu.
    pub attachment_count: u32,
    /// L'aperçu sous l'objet (écran 02 v2) ; `None` tant que le corps
    /// n'est pas rapatrié ou rattrapé.
    pub preview: Option<String>,
    /// Adresse brute de l'expéditeur — la ligne « De » de l'écran 03
    /// (`Nom <adresse>`). `sender` reste la chaîne d'affichage.
    pub sender_address: Option<String>,
    /// Destinataires À / Cc bruts (R4). Dans un dossier d'envois — ou pour
    /// nos propres messages d'un fil — l'expéditeur est SOI : c'est le
    /// destinataire qui dit à qui le message est parti. Vides quand
    /// l'ENVELOPE n'en portait pas (anciens envois non rattrapés, reçus
    /// dont l'À n'a pas été stocké).
    pub to_addrs: Vec<String>,
    pub cc_addrs: Vec<String>,
    /// R4 (PLAN-RETOURS-7) : la ligne vient de la section ÉPINGLÉE de la
    /// Réception (`pinned_rows`). Toujours faux dans le flot paginé —
    /// une conversation épinglée en est exclue (D5). Le fil ouvert SÈME
    /// son état d'épingle de ce champ (fil.svelte.js) : c'est lui qui
    /// habille « Épingler »/« Désépingler » sans aller-retour.
    pub pinned: bool,
    /// E5 : le fil est-il MIS DE CÔTÉ — semé par la seule source qui
    /// le sait (la pile) : une ligne d'une vue organisée n'est JAMAIS
    /// mise de côté (le cœur l'exclut), une carte de la pile l'est
    /// toujours. Même règle que `pinned` (revue 2026-08-21 : jamais un
    /// aller-retour par ouverture).
    pub cote: bool,
    /// L'invitation du fil (terrain R10/R11, PLAN-INVITATIONS) : le
    /// rang de puces la dit (réponse donnée, annulation) et porte les
    /// trois gestes — répondre sans ouvrir. `None` = ligne ordinaire.
    pub invitation: Option<InvitationLigne>,
}

/// Le badge d'invitation d'une ligne — la clé pour répondre DEPUIS la
/// liste vise le MESSAGE d'invitation (pas la tête du fil).
#[derive(Serialize)]
pub struct InvitationLigne {
    pub mailbox: String,
    pub uid: u32,
    /// Le titre de la réunion — le sujet de la réponse se construit de
    /// lui, jamais du sujet de la tête (« Re : … »).
    pub titre: String,
    /// `accepte` | `provisoire` | `refuse` — la puce du rang.
    pub reponse: Option<String>,
    pub annulee: bool,
    pub peut_repondre: bool,
}

/// Bilan d'une reconnexion : ce qui est revenu, et POURQUOI le reste ne
/// l'est pas. Un compte muet est pire qu'un compte en erreur — sans cette
/// liste, l'utilisateur voit une pastille manquer sans savoir quoi faire.
#[derive(Serialize)]
pub struct ConnectReport {
    pub accounts: Vec<AccountInfo>,
    pub problems: Vec<String>,
}

/// Connexion silencieuse de TOUS les comptes du registre. Registre vide
/// (base migrée de Phase 2) : l'entrée héritée du coffre peut révéler le
/// compte — elle est alors migrée et le compte en attente revendiqué.
///
/// **Chaque compte est isolé** : la configuration manquante ou le jeton
/// périmé de l'un ne doit jamais empêcher les autres de revenir. Même
/// principe que [`sync_inbox`].
#[tauri::command]
pub async fn connect_accounts(app: AppHandle) -> Result<ConnectReport, String> {
    // Crochet E2E : comptes factices (emails séparés par des virgules),
    // jetons invalides par construction — hors ligne garanti.
    if let Ok(list) = std::env::var("WIND_E2E_ACCOUNT") {
        return hors_pompe(app, move |app| {
            let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
            let state = app.state::<AppState>();
            let mut infos = Vec::new();
            for email in list.split(',').map(str::trim).filter(|e| !e.is_empty()) {
                let id = store
                    .adopt_or_create_account(email, mail_auth::GOOGLE.account_kind)
                    .map_err(|err| err.to_string())?;
                lock_accounts(&state)?.insert(
                    email.to_string(),
                    AccountSession::OAuth(Authenticated {
                        provider: &mail_auth::GOOGLE,
                        email: email.to_string(),
                        access_token: "jeton-e2e-invalide".to_string(),
                    }),
                );
                infos.push(AccountInfo {
                    id,
                    email: email.to_string(),
                });
            }
            // E4 : les comptes du décor gagnent aussi leurs veilleurs — même
            // chemin que le réel, leurs échecs de connexion sont bornés
            // (timeouts P0) et espacés (délai doublé).
            crate::veilleur::reconcilier(&app);
            Ok(ConnectReport {
                accounts: infos,
                problems: Vec::new(),
            })
        })
        .await;
    }

    let path = db_path(&app)?;
    let accounts = hors_pompe(app.clone(), |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store.accounts().map_err(|err| err.to_string())
    })
    .await?;

    let path_for_spawn = path.clone();
    let (connected, mut problems) = tauri::async_runtime::spawn_blocking(move || {
        let mut list = Vec::new();
        let mut problems: Vec<String> = Vec::new();
        for account in accounts {
            // La configuration OAuth manquante d'un fournisseur ne concerne
            // QUE ses comptes : elle ne doit empêcher aucun autre de revenir.
            let outcome = match account.provider.as_str() {
                "imap" => connect_generic(&path_for_spawn, &account),
                kind => match mail_auth::for_account_kind(kind) {
                    Some(provider) => Authenticator::from_env(provider)
                        .and_then(|auth| auth.authenticate_silent(&account.email))
                        .map(|session| Some(AccountSession::OAuth(session)))
                        .map_err(|err| err.to_string()),
                    None => Err(format!("fournisseur inconnu : {kind}")),
                },
            };
            match outcome {
                Ok(Some(session)) => list.push(session),
                Ok(None) => problems.push(format!(
                    "{} : configuration serveur incomplète",
                    account.email
                )),
                Err(reason) => problems.push(format!("{} : {reason}", account.email)),
            }
        }
        // Repli hérité Phase 2 : un compte Gmail sans provider explicite.
        // Propre à Google — la Phase 2 ne connaissait que lui.
        if list.is_empty()
            && let Ok(auth) = Authenticator::google_from_env()
            && let Ok(account) = auth.authenticate_silent_legacy()
        {
            list.push(AccountSession::OAuth(account));
        }
        (list, problems)
    })
    .await
    .map_err(|err| err.to_string())?;

    // E5 : l'écriture des comptes et la pose des sessions sous le verrou
    // des commandes — plus jamais sur le worker async nu.
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let state = app.state::<AppState>();
        let mut infos = Vec::new();
        for session in connected {
            let email = session.email().to_string();
            let provider = match &session {
                AccountSession::OAuth(auth) => auth.provider.account_kind,
                AccountSession::Generic(_) => "imap",
            };
            let id = store
                .adopt_or_create_account(&email, provider)
                .map_err(|err| err.to_string())?;
            infos.push(AccountInfo {
                id,
                email: email.clone(),
            });
            lock_accounts(&state)?.insert(email, session);
        }
        problems.sort();
        // E4 : un veilleur IDLE par compte reconnecté (ADR 0018) — démarrés
        // ICI, après que les sessions sont posées, jamais au boot (rien à
        // veiller sans session).
        crate::veilleur::reconcilier(&app);
        Ok(ConnectReport {
            accounts: infos,
            problems,
        })
    })
    .await
}

/// Reconnecte un compte IMAP générique depuis le coffre et sa
/// configuration. `Ok(None)` : la configuration serveur est incomplète.
fn connect_generic(
    db_path: &Path,
    account: &mail_core::Account,
) -> Result<Option<AccountSession>, String> {
    let password =
        mail_auth::fetch_generic_password(&account.email).map_err(|err| err.to_string())?;
    let config = Store::open(db_path)
        .map_err(|err| err.to_string())?
        .account_config(account.id)
        .map_err(|err| err.to_string())?;
    Ok(build_generic_session(&account.email, &password, &config))
}

/// Ajoute un compte Gmail — parcours navigateur complet, répétable.
/// Google livre l'identité du compte : rien à déclarer.
#[tauri::command]
pub async fn add_account(
    app: AppHandle,
    state: State<'_, AppState>,
    horizon: Option<String>,
) -> Result<AccountInfo, String> {
    add_oauth_account(app, state, &mail_auth::GOOGLE, None, horizon).await
}

/// Ajoute un compte Microsoft 365 / Outlook.com.
///
/// L'adresse est SAISIE : dans le périmètre de scopes mesuré par le spike,
/// Microsoft ne livre pas l'identité du compte ([`mail_auth::Identity`]).
#[tauri::command]
pub async fn add_microsoft_account(
    app: AppHandle,
    state: State<'_, AppState>,
    email: String,
    horizon: Option<String>,
) -> Result<AccountInfo, String> {
    let email = email.trim().to_string();
    // Validation à la frontière : l'adresse déclarée devient la clé du
    // compte ET l'identifiant XOAUTH2. Une saisie vide produirait un
    // compte fantôme que rien ne pourrait plus joindre.
    if !is_plausible_address(&email) {
        return Err("adresse invalide : saisissez l'adresse complète du compte".to_string());
    }
    add_oauth_account(app, state, &mail_auth::MICROSOFT, Some(email), horizon).await
}

/// Le tronc commun des ajouts OAuth2 : consentement navigateur, puis
/// enregistrement du compte sous la clé de SON fournisseur.
async fn add_oauth_account(
    app: AppHandle,
    state: State<'_, AppState>,
    provider: &'static mail_auth::Provider,
    declared_email: Option<String>,
    horizon: Option<String>,
) -> Result<AccountInfo, String> {
    // Validation à la frontière, AVANT le parcours navigateur : refuser
    // un horizon illisible après le consentement laisserait un compte
    // créé sous un geste qui a échoué.
    valider_horizon(horizon.as_deref())?;
    let account = tauri::async_runtime::spawn_blocking(move || {
        Authenticator::from_env(provider)
            .map_err(|err| err.to_string())?
            .authenticate_interactive(declared_email.as_deref())
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())??;

    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let id = store
        .adopt_or_create_account(&account.email, account.provider.account_kind)
        .map_err(|err| err.to_string())?;
    ecrire_horizon_premier_ajout(&store, id, horizon.as_deref())?;
    let info = AccountInfo {
        id,
        email: account.email.clone(),
    };
    lock_accounts(&state)?.insert(account.email.clone(), AccountSession::OAuth(account));
    Ok(info)
}

/// Reconnecte un compte du registre dont le jeton est mort — constat
/// terrain du 2026-08-20 : `invalid_grant` (jeton expiré ou révoqué)
/// laissait l'utilisateur DÉMUNI, aucun geste ne relançait le
/// consentement. Même parcours navigateur que l'ajout, sur la ligne
/// EXISTANTE : rien n'est re-synchronisé, rien n'est perdu.
///
/// Garde d'identité : le consentement doit revenir avec l'adresse du
/// compte visé. Google choisit l'identité au navigateur — un autre
/// choix ne doit pas silencieusement connecter un AUTRE compte sous le
/// geste « reconnecter X » ; Microsoft reçoit l'adresse déclarée, la
/// garde y est structurelle. Un compte IMAP générique n'a pas de jeton :
/// refus franc avec la marche à suivre.
#[tauri::command]
pub async fn reconnect_account(app: AppHandle, account_id: i64) -> Result<AccountInfo, String> {
    let account = hors_pompe(app.clone(), move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .accounts()
            .map_err(|err| err.to_string())?
            .into_iter()
            .find(|account| account.id == account_id)
            .ok_or_else(|| "compte inconnu".to_string())
    })
    .await?;
    if account.provider == "imap" {
        return Err(
            "compte IMAP générique : retirez puis rajoutez le compte pour ressaisir le mot de passe"
                .to_string(),
        );
    }
    let provider = mail_auth::for_account_kind(&account.provider)
        .ok_or_else(|| format!("fournisseur inconnu : {}", account.provider))?;
    // Google livre l'identité ; Microsoft exige l'adresse déclarée.
    let declared =
        (provider.account_kind != mail_auth::GOOGLE.account_kind).then(|| account.email.clone());
    let session = tauri::async_runtime::spawn_blocking(move || {
        Authenticator::from_env(provider)
            .map_err(|err| err.to_string())?
            .authenticate_interactive(declared.as_deref())
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())??;
    if !session
        .email
        .trim()
        .eq_ignore_ascii_case(account.email.trim())
    {
        return Err(format!(
            "le consentement a été donné pour {}, pas pour {} ; rejouez la reconnexion en choisissant le bon compte",
            session.email, account.email
        ));
    }
    hors_pompe(app, move |app| {
        let state = app.state::<AppState>();
        lock_accounts(&state)?.insert(account.email.clone(), AccountSession::OAuth(session));
        // Le compte retrouve son veilleur IDLE sans attendre un relancement.
        crate::veilleur::reconcilier(&app);
        Ok(AccountInfo {
            id: account.id,
            email: account.email,
        })
    })
    .await
}

/// Filtre minimal d'adresse : ce qui suit est vérifié par le fournisseur
/// lui-même au consentement. On ne cherche pas à valider RFC 5322 ici,
/// seulement à refuser ce qui ne peut manifestement pas être une adresse.
fn is_plausible_address(email: &str) -> bool {
    match email.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty() && domain.contains('.') && !domain.starts_with('.')
        }
        None => false,
    }
}

/// Les champs arrivent de l'UI en camelCase. Tauri ne convertit que les
/// ARGUMENTS de commande, pas les champs d'une struct imbriquée : sans ce
/// `rename_all`, `imapHost` ne trouverait pas `imap_host`.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericAccountInput {
    pub email: String,
    pub username: Option<String>,
    pub password: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
}

/// Le miroir côté commande de `Store::set_horizon_import` : refuser une
/// valeur hors vocabulaire AVANT tout travail (connexion, consentement).
fn valider_horizon(horizon: Option<&str>) -> Result<(), String> {
    match horizon {
        Some(h) if !mail_core::HORIZONS_IMPORT.contains(&h) => {
            Err(format!("horizon inconnu : {h:?}"))
        }
        _ => Ok(()),
    }
}

/// L'horizon du guichet ne s'écrit qu'au PREMIER ajout (revue
/// 2026-08-30) : re-jouer l'ajout d'un compte existant (le chemin
/// d'adoption — un geste de réparation) ne doit pas écraser en silence
/// un horizon déjà choisi (ou le « tout » réputé de D4) avec le défaut
/// du sélecteur. Après coup, le réglage vit aux Réglages > Comptes (D3).
fn ecrire_horizon_premier_ajout(
    store: &Store,
    account_id: i64,
    horizon: Option<&str>,
) -> Result<(), String> {
    let Some(h) = horizon else { return Ok(()) };
    let deja = store
        .text_pref(&format!("horizon_import.{account_id}"))
        .map_err(|err| err.to_string())?
        .is_some();
    if !deja {
        store
            .set_horizon_import(account_id, h)
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

/// Ajoute un compte IMAP/SMTP générique : teste la connexion, stocke le
/// mot de passe dans le coffre, puis enregistre le compte en base.
#[tauri::command]
pub async fn add_generic_account(
    app: AppHandle,
    input: GenericAccountInput,
    horizon: Option<String>,
) -> Result<AccountInfo, String> {
    valider_horizon(horizon.as_deref())?;
    let username = input.username.unwrap_or_else(|| input.email.clone());
    let email = input.email.clone();
    let imap_host = input.imap_host.clone();
    let imap_port = input.imap_port;
    let smtp_host = input.smtp_host.clone();
    let smtp_port = input.smtp_port;
    let password = input.password.clone();

    // Test IMAP immédiat : on ne stocke rien tant que la connexion ne
    // fonctionne pas.
    tauri::async_runtime::spawn_blocking({
        let email = email.clone();
        let username = username.clone();
        let imap_host = imap_host.clone();
        let password = password.clone();
        move || {
            let server = mail_imap::ImapServer::connect_password(
                &imap_host, imap_port, &username, &password,
            )
            .map_err(|err| format!("connexion IMAP impossible : {err}"))?;
            server.logout();
            mail_auth::store_generic_password(&email, &password).map_err(|err| err.to_string())
        }
    })
    .await
    .map_err(|err| err.to_string())??;

    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let id = store
            .create_generic_account(
                &email, &username, &imap_host, imap_port, &smtp_host, smtp_port,
            )
            .map_err(|err| err.to_string())?;
        ecrire_horizon_premier_ajout(&store, id, horizon.as_deref())?;

        let session = AccountSession::Generic(GenericCredentials {
            email: email.clone(),
            username: username.clone(),
            password,
            imap_host,
            imap_port,
            smtp_host,
            smtp_port,
        });
        let state = app.state::<AppState>();
        lock_accounts(&state)?.insert(email.clone(), session);
        // E4 : le compte neuf gagne son veilleur IDLE sans attendre.
        crate::veilleur::reconcilier(&app);
        Ok(AccountInfo { id, email })
    })
    .await
}

/// Retire un compte : ses secrets quittent le coffre de l'OS, ses données
/// locales la base, sa session la mémoire. Le serveur n'est JAMAIS
/// touché — le courrier reste chez le fournisseur.
///
/// L'ordre est un choix : le coffre D'ABORD, la base ensuite. Si la base
/// échouait après le coffre, le prochain lancement le DIT (« aucun jeton
/// pour… ») et le retrait se rejoue ; l'inverse — un jeton orphelin qui
/// survit au compte — resterait invisible pour toujours.
#[tauri::command]
pub async fn remove_account(app: AppHandle, account_id: i64) -> Result<(), String> {
    let account = hors_pompe(app.clone(), move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .accounts()
            .map_err(|err| err.to_string())?
            .into_iter()
            .find(|account| account.id == account_id)
            .ok_or_else(|| format!("compte inconnu : {account_id}"))
    })
    .await?;

    // Le coffre est une API bloquante de l'OS : hors du fil de la fenêtre,
    // comme tous ses autres accès.
    {
        let email = account.email.clone();
        let provider = account.provider.clone();
        tauri::async_runtime::spawn_blocking(move || {
            mail_auth::forget_credentials(&provider, &email).map_err(|err| err.to_string())
        })
        .await
        .map_err(|err| err.to_string())??;
    }

    hors_pompe(app, move |app| {
        let mut store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .delete_account(account_id)
            .map_err(|err| err.to_string())?;
        let state = app.state::<AppState>();
        lock_accounts(&state)?.remove(&account.email);
        // E4 : son veilleur IDLE s'éteint au prochain tour.
        crate::veilleur::reconcilier(&app);
        Ok(())
    })
    .await
}

/// Construit une session générique à partir du mot de passe et de la
/// configuration stockée. Retourne `None` si la configuration est incomplète.
fn build_generic_session(
    email: &str,
    password: &str,
    config: &AccountConfig,
) -> Option<AccountSession> {
    Some(AccountSession::Generic(GenericCredentials {
        email: email.to_string(),
        username: config.username.clone().unwrap_or_else(|| email.to_string()),
        password: password.to_string(),
        imap_host: config.imap_host.clone()?,
        imap_port: config.imap_port?,
        smtp_host: config.smtp_host.clone()?,
        smtp_port: config.smtp_port?,
    }))
}

/// Nomme la boîte en cours de relève dans l'activité partagée — le
/// mouvement que le terrain a réclamé (« 2/2 figé 7 minutes »). Chaîne
/// vide entre deux boîtes.
fn poser_boite(cycle: &crate::SyncShared, nom: &str) {
    if let Ok(mut boite) = cycle.boite.lock() {
        boite.clear();
        boite.push_str(nom);
    }
    if let Ok(mut phase) = cycle.phase.lock() {
        phase.clear();
    }
}

/// Nomme l'étape SANS boîte (inventaire des dossiers, fils, brouillons) :
/// `nom` est une clé, l'UI la traduit — le shell ne compose pas de texte
/// d'interface (A15). Exclusif avec la boîte.
fn poser_phase(cycle: &crate::SyncShared, nom: &str) {
    if let Ok(mut phase) = cycle.phase.lock() {
        phase.clear();
        phase.push_str(nom);
    }
    if let Ok(mut boite) = cycle.boite.lock() {
        boite.clear();
    }
}

/// La relève gardée (ADR 0017) : faut-il relever ce dossier ? Toute
/// incertitude — relevé refusé par le serveur, repère illisible —
/// relève : la sobriété n'a pas le droit de coûter un message.
fn doit_relever(
    store: &Store,
    account_id: i64,
    boite: &str,
    statut: Option<&mail_core::FolderStatus>,
    problems: &mut Vec<String>,
) -> bool {
    let Some(statut) = statut else {
        return true;
    };
    let repere = (|| -> Result<Option<mail_core::RepereLocal>, mail_core::Error> {
        let Some(state) = store.sync_state(account_id, boite)? else {
            return Ok(None);
        };
        Ok(Some(mail_core::RepereLocal {
            uid_validity: state.uid_validity,
            uidnext_vu: store.remote_uidnext(state.mailbox_id)?,
            messages_locaux: store.envelope_count(state.mailbox_id)?,
            actions_en_attente: store.has_pending_actions(state.mailbox_id)?,
            // E2b : le modseq du dernier SELECT soldé — c'est lui qui
            // réveille un dossier dont seuls les drapeaux ont glissé.
            modseq_vu: state.highest_modseq,
        }))
    })();
    match repere {
        Ok(repere) => mail_core::faut_relever(statut, repere.as_ref()),
        Err(err) => {
            problems.push(format!("repère de « {boite} » : {err}"));
            true
        }
    }
}

/// Solde le repère d'une relève ABOUTIE : le UIDNEXT du relevé qui l'a
/// précédée. Jamais sur une relève échouée — un repère posé sur un
/// dossier pas rattrapé le ferait sauter à tort au cycle suivant.
fn solder_repere(
    store: &Store,
    account_id: i64,
    boite: &str,
    statut: Option<&mail_core::FolderStatus>,
    problems: &mut Vec<String>,
) {
    let Some(uidnext) = statut.and_then(|statut| statut.uid_next) else {
        return;
    };
    let pose = store.sync_state(account_id, boite).and_then(|state| {
        if let Some(state) = state {
            store.set_remote_uidnext(state.mailbox_id, uidnext)?;
        }
        Ok(())
    });
    if let Err(err) = pose {
        problems.push(format!("repère de « {boite} » : {err}"));
    }
}

/// La relève INBOX d'un compte — le cœur partagé du cycle complet et de
/// la passe légère (E3) : relevé STATUS, relève gardée (E2a), repère
/// soldé, courrier compté et bulles du compte (P1). Rend le rapport ET
/// le relevé payé — le cycle complet le réutilise pour la garde
/// d'espace, il n'est jamais payé deux fois.
fn relever_inbox(
    server: &mut ImapServer,
    store: &mut Store,
    account_id: i64,
    cycle: &crate::SyncShared,
    app: &AppHandle,
    problems: &mut Vec<String>,
) -> Result<(mail_core::SyncReport, Option<mail_core::FolderStatus>), String> {
    poser_boite(cycle, MAILBOX);
    // L'UID le plus haut AVANT la synchro : c'est lui qui separe
    // « nouveau » de « deja connu ». Releve avant, sinon la synchro
    // l'aurait deja deplace.
    let last_uid_before = store
        .sync_state(account_id, MAILBOX)
        .map_err(|err| err.to_string())?
        .map(|state| state.last_uid)
        .unwrap_or(0);
    // INBOX est gardée comme les autres (ADR 0017) : un relevé STATUS,
    // la relève seulement si quelque chose a bougé.
    let statut_inbox = server.folder_status(MAILBOX).ok();
    let report = if doit_relever(store, account_id, MAILBOX, statut_inbox.as_ref(), problems) {
        let report = SyncEngine::default()
            .sync(server, store, account_id, MAILBOX)
            .map_err(|err| err.to_string())?;
        solder_repere(store, account_id, MAILBOX, statut_inbox.as_ref(), problems);
        report
    } else {
        // Rien n'a bougé : rapport incrémental vide — pas d'arrivées,
        // pas de bulles, pas de mensonge.
        mail_core::SyncReport {
            mode: mail_core::SyncMode::Incremental,
            fetched: 0,
            deleted: 0,
            replayed: 0,
            refusees: 0,
        }
    };

    // E4 (PLAN-REACTIVITE, R-D2) : les corps des ARRIVÉES se rapatrient
    // sur la connexion déjà ouverte, AVANT le bump de génération — la
    // ligne naît AVEC son aperçu, au cycle comme à la passe légère comme
    // au veilleur (un seul affichage, jamais une ligne muette qui se
    // remplit plus tard). Borné : un lot qui déborde (rattrapage après
    // coupure) bumpe d'abord — les lignes vite — et les corps échoient à
    // la pompe, que l'UI amorce à la génération. `bodies_to_backfill`
    // sert du plus récent au plus ancien : le budget « nombre
    // d'arrivées » couvre exactement le lot qui vient d'entrer.
    //
    // La borne se mesure sur les ARRIVÉES (UID au-dessus du repère
    // d'avant-relève), JAMAIS sur `report.fetched` — premier terrain E4
    // (2026-08-14) : sur Gmail, chaque arrivée fait glisser HIGHESTMODSEQ
    // et le delta CONDSTORE rend des dizaines d'enveloppes retouchées
    // (l'observation consignée de PLAN-SYNCHRO) ; mesuré sur `fetched`,
    // le lot « débordait » à CHAQUE arrivée et la ligne naissait muette,
    // remplie 3-4 s plus tard par la pompe.
    let arrivees = match store.arrivees_depuis(account_id, MAILBOX, last_uid_before) {
        Ok(n) => n as usize,
        Err(err) => {
            problems.push(format!("compte des arrivées : {err}"));
            0
        }
    };
    let corps = corps_a_l_arrivee(arrivees);
    // L'horizon d'import s'applique ici aussi (uniforme avec la pompe).
    // La borne compare la DATE du message, pas son arrivée : une
    // arrivée à l'en-tête Date ancien (renvoi différé, message déplacé
    // vers INBOX par un autre client) reste hors portée — voulu, c'est
    // la sémantique D1 : son corps se charge au clic.
    let horizon = horizon_corps(store, account_id);
    if corps > 0
        && let Err(err) =
            mail_core::backfill_bodies(server, store, account_id, MAILBOX, horizon, corps)
    {
        problems.push(format!("corps des arrivées : {err}"));
    }

    // P1 (PLAN-SYNCHRO) : le courrier d'INBOX se voit TOUT DE SUITE —
    // le compteur sondé recharge la liste côté UI, et les bulles du
    // compte partent ICI, sans attendre l'inventaire, les dossiers ni
    // les AUTRES comptes (l'agrégat de fin de cycle perdait toujours la
    // course contre le téléphone). Les arrivées ne viennent que d'INBOX :
    // rien n'est annoncé en retard. Best effort, comme les passes
    // voisines : le courrier est là, une annonce qui échoue se consigne.
    if report.fetched > 0 || report.deleted > 0 {
        cycle
            .courrier
            .fetch_add((report.fetched + report.deleted) as u64, Ordering::Relaxed);
        // E4 : la génération MONOTONE — l'UI la sonde via `sync_progress`
        // et recharge la liste quand elle bouge. C'est le chemin par
        // lequel le courrier signalé par un veilleur IDLE se montre au
        // repos, sans canal neuf (R0-S5).
        cycle.generation.fetch_add(1, Ordering::Relaxed);
    }
    match store.new_unread_after(account_id, MAILBOX, last_uid_before, NOTIFY_MAX_ARRIVALS) {
        Ok(arrivals) => {
            let arrivals = mail_core::arrivals_to_notify(report.mode, arrivals);
            if let Some(problem) = arrival_notification_problem(app, store, &arrivals) {
                problems.push(problem);
            }
        }
        Err(err) => problems.push(format!("arrivées à annoncer : {err}")),
    }
    // Revue PLAN-AUDIT-V1 : le cycle qui met une action en quarantaine le
    // DIT — sinon seul le compteur global de la fente le révèle, sans
    // lien avec le cycle fautif. Point de sortie unique des quatre chemins.
    if report.refusees > 0 {
        crate::trace::trace(&format!(
            "relève compte {account_id} : {} action(s) mise(s) en quarantaine",
            report.refusees
        ));
    }
    Ok((report, statut_inbox))
}

/// Combien attendre après `echecs` échecs CONSÉCUTIFS d'un compte —
/// décision pure (complément P0, anti-martèlement). 0 ou 1 échec :
/// rien, la cadence de 5 min est déjà une politesse ; ensuite le délai
/// DOUBLE (10, 20, 40 min), plafonné à 60 — un serveur qui bride a
/// besoin d'air, pas d'un client qui insiste.
fn attente_apres_echecs(echecs: u32) -> Duration {
    if echecs <= 1 {
        return Duration::ZERO;
    }
    let facteur = 1u64 << (echecs - 1).min(4);
    Duration::from_secs((300 * facteur).min(3600))
}

/// Le temps restant du recul de ce compte, s'il court encore. Un
/// verrou illisible vaut « pas de recul » : la protection cède le pas
/// à la relève, jamais l'inverse.
pub(crate) fn recul_en_cours(
    reculs: &Mutex<HashMap<String, crate::Recul>>,
    email: &str,
) -> Option<Duration> {
    let reculs = reculs.lock().ok()?;
    let recul = reculs.get(email)?;
    attente_apres_echecs(recul.echecs)
        .checked_sub(recul.depuis.elapsed())
        .filter(|reste| !reste.is_zero())
}

/// Le verrou de relève de CE compte (E4) : cycle, bouton et veilleur
/// IDLE peuvent vouloir relever le même INBOX au même moment — un
/// compte à la fois. Un verrou de MAP empoisonné se répare en le
/// reprenant : perdre la sérialisation vaut mieux que perdre la relève.
pub(crate) fn verrou_compte(
    verrous: &Mutex<HashMap<String, Arc<Mutex<()>>>>,
    email: &str,
) -> Arc<Mutex<()>> {
    let mut verrous = match verrous.lock() {
        Ok(verrous) => verrous,
        Err(empoisonne) => empoisonne.into_inner(),
    };
    verrous.entry(email.to_string()).or_default().clone()
}

/// La passe légère d'UN compte (ADR 0018) : celle que le veilleur IDLE
/// déclenche — sur `EXISTS`, et à chaque (re)connexion (un mail arrivé
/// pendant une coupure n'émet jamais d'EXISTS, 2ᵉ terrain). Même
/// travail que `sync_inbox_light` pour ce compte : relève gardée (E2a),
/// courrier compté et génération bumpée (l'UI recharge à la sonde),
/// bulles (P1). Best effort : les incidents partent en console —
/// identifiant de compte et décomptes seuls (§6.8).
pub(crate) fn passe_legere_compte(app: &AppHandle, email: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let path = db_path(app)?;
    let session = lock_accounts(&state)?
        .get(email)
        .cloned()
        .ok_or_else(|| "compte non connecté".to_string())?;
    // Un compte à la fois : la relève du cycle ou du bouton peut être
    // en cours sur CE compte — on attend notre tour.
    let verrou = verrou_compte(&state.verrous_releve, email);
    let _releve = verrou
        .lock()
        .map_err(|_| "verrou de relève empoisonné".to_string())?;
    // Le recul se respecte (lecture seule) : si le compte est en échec
    // répété, le cycle reprendra — le veilleur n'insiste pas.
    if recul_en_cours(&state.sync_reculs, email).is_some() {
        return Ok(());
    }
    // UNE connexion pour la passe (PLAN-AUDIT-V2 E1) : elle traverse la
    // connexion IMAP sans rien tenir — en WAL, une connexion ouverte hors
    // transaction ne verrouille personne ; l'identifiant lu avant le
    // réseau est stable, ce n'est pas un état qu'on rejouerait après.
    let mut store = Store::open(&path).map_err(|err| err.to_string())?;
    let account_id = store
        .accounts()
        .map_err(|err| err.to_string())?
        .into_iter()
        .find(|compte| compte.email == email)
        .map(|compte| compte.id)
        .ok_or_else(|| "compte inconnu en base".to_string())?;

    let (mut server, refreshed) = connect_imap(&session)?;
    let mut problems = Vec::new();
    let cycle = state.sync_cycle.clone();
    let resultat = relever_inbox(
        &mut server,
        &mut store,
        account_id,
        &cycle,
        app,
        &mut problems,
    );
    server.logout();
    match resultat {
        Ok(_) => {
            noter_issue(&state.sync_reculs, email, true);
            if let Some(fresh) = refreshed {
                lock_accounts(&state)?.insert(fresh.email().to_string(), fresh);
            }
            // L'horodatage vaut pour cette relève comme pour les autres :
            // l'INBOX vient d'être vérifiée.
            if let Ok(epoch) = SystemTime::now().duration_since(UNIX_EPOCH) {
                let _ = store.set_text_pref(PREF_DERNIERE_SYNCHRO, &epoch.as_secs().to_string());
            }
            for problem in problems {
                crate::trace::trace(&format!("veilleur compte {account_id} : {problem}"));
            }
            Ok(())
        }
        Err(err) => {
            noter_issue(&state.sync_reculs, email, false);
            Err(err)
        }
    }
}

/// Solde l'issue d'une tentative : le succès efface le recul, l'échec
/// l'aggrave et repart de maintenant.
fn noter_issue(reculs: &Mutex<HashMap<String, crate::Recul>>, email: &str, succes: bool) {
    let Ok(mut reculs) = reculs.lock() else {
        return;
    };
    if succes {
        reculs.remove(email);
        return;
    }
    let recul = reculs.entry(email.to_string()).or_insert(crate::Recul {
        echecs: 0,
        depuis: Instant::now(),
    });
    recul.echecs = recul.echecs.saturating_add(1);
    recul.depuis = Instant::now();
}

/// À la sortie du cycle — normale ou par panique — l'activité s'éteint :
/// une barre d'état qui annoncerait un cycle fantôme serait le mensonge
/// exact que E1 corrige.
struct FinDeCycle(Arc<crate::SyncShared>);

impl Drop for FinDeCycle {
    fn drop(&mut self) {
        self.0.en_cours.store(false, Ordering::Relaxed);
    }
}

/// Synchronise TOUS les comptes connectés — l'échec d'un compte ne
/// bloque pas les autres (il est consigné dans le bilan).
#[tauri::command]
pub async fn sync_inbox(app: AppHandle, state: State<'_, AppState>) -> Result<SyncSummary, String> {
    let path = db_path(&app)?;
    let jobs = hors_pompe(app.clone(), |app| connected_jobs(&app)).await?;
    let timer = Instant::now();
    let cycle = state.sync_cycle.clone();
    // Le manche traverse la boucle : les bulles partent PAR COMPTE, dès
    // la relève INBOX soldée (P1) — plus d'agrégat de fin de cycle, qui
    // faisait toujours perdre la course contre le téléphone.
    let app_bulles = app.clone();
    let reculs = state.sync_reculs.clone();
    let verrous = state.verrous_releve.clone();

    let (accounts, accounts_failed, fetched, deleted, replayed, mut errors, refreshed) =
        tauri::async_runtime::spawn_blocking(move || {
            // L'activité pour la barre d'état (PLAN-SYNCHRO E1) : posée
            // AVANT le premier compte, éteinte par le garde quoi qu'il
            // arrive. Un cycle à vide (aucun compte connecté) n'annonce
            // rien.
            let _fin = FinDeCycle(cycle.clone());
            cycle.fait.store(0, Ordering::Relaxed);
            cycle.total.store(jobs.len() as u64, Ordering::Relaxed);
            cycle.courrier.store(0, Ordering::Relaxed);
            cycle.en_cours.store(!jobs.is_empty(), Ordering::Relaxed);
            let mut accounts = 0;
            let mut accounts_failed = 0;
            let mut fetched = 0;
            let mut deleted = 0;
            let mut replayed = 0;
            let mut errors = Vec::new();
            let mut refreshed = Vec::new();
            for (account_id, session) in jobs {
                let email = session.email().to_string();
                // Le recul (complément P0) : un compte en échecs répétés
                // est SAUTÉ tant que son délai court — aucune connexion,
                // aucun refresh OAuth. Sans être TU : il reste compté
                // injoignable, sinon l'alerte de la barre s'éteindrait
                // sur un compte toujours mort.
                if let Some(reste) = recul_en_cours(&reculs, &email) {
                    accounts_failed += 1;
                    errors.push(format!(
                        "{email} : en recul après échecs répétés ; nouvelle tentative dans {} min",
                        reste.as_secs().div_ceil(60).max(1)
                    ));
                    cycle.fait.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                // E4 : un compte à la fois — un veilleur IDLE peut être
                // en pleine passe légère sur CE compte au même moment.
                let verrou = verrou_compte(&verrous, &email);
                let _releve = verrou.lock();
                if let Ok(mut compte) = cycle.compte.lock() {
                    compte.clone_from(&email);
                }
                poser_boite(&cycle, "");
                match run_sync(&session, account_id, &path, &cycle, &app_bulles) {
                    Ok(outcome) => {
                        noter_issue(&reculs, &email, true);
                        accounts += 1;
                        fetched += outcome.report.fetched;
                        deleted += outcome.report.deleted;
                        replayed += outcome.report.replayed;
                        if let Some(fresh) = outcome.refreshed {
                            refreshed.push(fresh);
                        }
                        for problem in outcome.problems {
                            errors.push(format!("{email} : {problem}"));
                        }
                    }
                    Err(err) => {
                        noter_issue(&reculs, &email, false);
                        accounts_failed += 1;
                        errors.push(format!("{email} : {err}"));
                    }
                }
                cycle.fait.fetch_add(1, Ordering::Relaxed);
            }
            (
                accounts,
                accounts_failed,
                fetched,
                deleted,
                replayed,
                errors,
                refreshed,
            )
        })
        .await
        .map_err(|err| err.to_string())?;

    reposer_sessions(&state, refreshed)?;
    solder_releve(&app, accounts, &mut errors).await?;

    Ok(SyncSummary {
        accounts,
        accounts_failed,
        fetched,
        deleted,
        replayed,
        elapsed_ms: timer.elapsed().as_millis() as u64,
        errors,
    })
}

/// La passe légère (PLAN-SYNCHRO E3, S-D2) : STATUS INBOX de chaque
/// compte, relève seulement si ça a bougé (E2a), courrier visible et
/// bulles par compte (P1) — ni inventaire, ni balayage des dossiers, ni
/// fils : la réponse se compte en secondes, tenue par la gate d'E2a.
/// C'est elle que le bouton déclenche, elle que le réveil de veille
/// déclenche, elle que le veilleur IDLE (E4) réveillera.
#[tauri::command]
pub async fn sync_inbox_light(
    app: AppHandle,
    state: State<'_, AppState>,
    force: bool,
) -> Result<SyncSummary, String> {
    let path = db_path(&app)?;
    let jobs = hors_pompe(app.clone(), |app| connected_jobs(&app)).await?;
    let timer = Instant::now();
    let cycle = state.sync_cycle.clone();
    let app_bulles = app.clone();
    let reculs = state.sync_reculs.clone();
    let verrous = state.verrous_releve.clone();

    let (accounts, accounts_failed, fetched, deleted, replayed, mut errors, refreshed) =
        tauri::async_runtime::spawn_blocking(move || {
            // La même activité que le cycle complet : la barre d'état
            // raconte la passe pendant qu'elle tourne, le bouton tourne
            // avec elle (réentrance gardée côté UI).
            let _fin = FinDeCycle(cycle.clone());
            cycle.fait.store(0, Ordering::Relaxed);
            cycle.total.store(jobs.len() as u64, Ordering::Relaxed);
            cycle.courrier.store(0, Ordering::Relaxed);
            cycle.en_cours.store(!jobs.is_empty(), Ordering::Relaxed);
            let mut accounts = 0;
            let mut accounts_failed = 0;
            let mut fetched = 0;
            let mut deleted = 0;
            let mut replayed = 0;
            let mut errors = Vec::new();
            let mut refreshed = Vec::new();
            for (account_id, session) in jobs {
                let email = session.email().to_string();
                // Le recul vaut aussi pour la passe légère (réveil de
                // veille, futur IDLE) — SAUF sur le geste manuel : le
                // clic est un ordre, il force toujours une tentative.
                if !force && let Some(reste) = recul_en_cours(&reculs, &email) {
                    accounts_failed += 1;
                    errors.push(format!(
                        "{email} : en recul après échecs répétés ; nouvelle tentative dans {} min",
                        reste.as_secs().div_ceil(60).max(1)
                    ));
                    cycle.fait.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                // E4 : un compte à la fois — un veilleur IDLE peut être
                // en pleine passe légère sur CE compte au même moment.
                let verrou = verrou_compte(&verrous, &email);
                let _releve = verrou.lock();
                if let Ok(mut compte) = cycle.compte.lock() {
                    compte.clone_from(&email);
                }
                poser_boite(&cycle, "");
                let releve = (|| -> Result<(mail_core::SyncReport, Vec<String>, Option<AccountSession>), String> {
                    let (mut server, fresh) = connect_imap(&session)?;
                    let mut store = Store::open(&path).map_err(|err| err.to_string())?;
                    let mut problems = Vec::new();
                    let (report, _) = relever_inbox(
                        &mut server,
                        &mut store,
                        account_id,
                        &cycle,
                        &app_bulles,
                        &mut problems,
                    )?;
                    server.logout();
                    Ok((report, problems, fresh))
                })();
                match releve {
                    Ok((report, problems, fresh)) => {
                        noter_issue(&reculs, &email, true);
                        accounts += 1;
                        fetched += report.fetched;
                        deleted += report.deleted;
                        replayed += report.replayed;
                        if let Some(fresh) = fresh {
                            refreshed.push(fresh);
                        }
                        for problem in problems {
                            errors.push(format!("{email} : {problem}"));
                        }
                    }
                    Err(err) => {
                        noter_issue(&reculs, &email, false);
                        accounts_failed += 1;
                        errors.push(format!("{email} : {err}"));
                    }
                }
                cycle.fait.fetch_add(1, Ordering::Relaxed);
            }
            (
                accounts, accounts_failed, fetched, deleted, replayed, errors, refreshed,
            )
        })
        .await
        .map_err(|err| err.to_string())?;

    reposer_sessions(&state, refreshed)?;
    // L'horodatage vaut aussi pour la passe légère : chaque INBOX vient
    // d'être vérifiée — c'est la relève du courrier au sens du prototype,
    // et un bouton qui laisserait « il y a 12 minutes » après un clic
    // réussi aurait l'air cassé. Les dossiers, eux, gardent leur cadence.
    solder_releve(&app, accounts, &mut errors).await?;

    Ok(SyncSummary {
        accounts,
        accounts_failed,
        fetched,
        deleted,
        replayed,
        elapsed_ms: timer.elapsed().as_millis() as u64,
        errors,
    })
}

/// « 1,2 Go » ou « 850 Mo » — l'utilisateur doit savoir COMBIEN libérer,
/// pas convertir des octets de tête. Préfixes décimaux (ceux de
/// l'Explorateur serait Gio, mais Go est ce que le grand public lit sur
/// une boîte de disque), virgule française.
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} Go", bytes as f64 / 1e9).replace('.', ",")
    } else {
        // Arrondi au Mo SUPÉRIEUR : annoncer « 0 Mo » à libérer serait
        // absurde, et sous-annoncer ferait échouer la re-tentative.
        format!("{} Mo", bytes.div_ceil(1_000_000).max(1))
    }
}

/// Ce qu'une synchronisation de compte rapporte, au-delà des décomptes.
struct SyncOutcome {
    report: mail_core::SyncReport,
    /// Session dont le jeton vient d'être renouvelé, à remettre en cache.
    refreshed: Option<AccountSession>,
    /// Incidents non bloquants : la synchronisation a réussi, mais un
    /// travail de fond qui l'accompagne a échoué. Rapportés, jamais
    /// avalés — un symptôme sans trace est indiagnosticable.
    problems: Vec<String>,
}

fn run_sync(
    session: &AccountSession,
    account_id: i64,
    db_path: &Path,
    cycle: &crate::SyncShared,
    app: &AppHandle,
) -> Result<SyncOutcome, String> {
    let (mut server, refreshed) = connect_imap(session)?;
    let mut store = Store::open(db_path).map_err(|err| err.to_string())?;
    // Chrono par phase (terrain 2026-08-13 : « INBOX » muet 2 min 15 —
    // l'observation doit devenir une mesure). Durées et décomptes
    // SEULS : ni adresse, ni nom de dossier (règle des diagnostics,
    // PASSATION §6.8) — l'identifiant de compte est un entier interne.
    let chrono = Instant::now();
    let mut problems: Vec<String> = Vec::new();
    let (report, statut_inbox) = relever_inbox(
        &mut server,
        &mut store,
        account_id,
        cycle,
        app,
        &mut problems,
    )?;
    let duree_inbox = chrono.elapsed();

    // L'inventaire : dossier des envois, portée, liste des dossiers,
    // garde d'espace (STATUS sur chaque dossier) — quatre travaux qui
    // vivaient sous l'étiquette « INBOX », à tort.
    poser_phase(cycle, "inventaire");
    let chrono = Instant::now();

    // « Envoyés » : sans lui, un fil ne porte que la moitié reçue de
    // l'échange. Mesuré sur la boîte réelle — 15 conversations de plus
    // d'un message avant, 234 après.
    //
    // Ne devient sûr que parce que l'identité d'un message porte
    // désormais sa BOÎTE (ADR 0009, étape 4b) : sans cela, un UID de ce
    // dossier serait lu dans INBOX, et les UID repartant de 1 dans chaque
    // boîte, la collision serait la norme.
    //
    // Best effort, comme les passes voisines : le courrier ENTRANT est le
    // résultat qui compte, et un serveur sans dossier d'envois doit
    // continuer à fonctionner. L'échec est rapporté, jamais avalé — une
    // boîte qui refuserait de se regrouper serait sinon indiagnosticable.
    let sent = match server.sent_folder_name() {
        Ok(found) => found,
        Err(reason) => {
            problems.push(format!("dossier envoyés : {reason}"));
            None
        }
    };

    // La PORTÉE du regroupement, déclarée avant de verser quoi que ce soit
    // d'autre dans le compte (ADR 0010 §3). Sans elle, les messages des
    // dossiers qu'on s'apprête à synchroniser rejoindraient les fils tout
    // seuls : un spam ferait remonter une conversation en tête de liste.
    //
    // Re-déclarée à CHAQUE synchronisation, et pas seulement à la création
    // du compte : un serveur peut renommer son dossier d'envois.
    //
    // AVANT la boucle, et c'est tout l'objet : le store en garde mémoire
    // sur le compte, donc les boîtes que la boucle va CRÉER naissent déjà
    // du bon côté de la portée. La déclarer après les ferait naître sans
    // fil, et leurs messages attendraient le prochain démarrage.
    if let Err(reason) = store.set_thread_scope(account_id, sent.as_deref()) {
        problems.push(format!("portée des conversations : {reason}"));
    }

    // TOUS les autres dossiers — archive, corbeille, spam, dossiers de
    // l'utilisateur (ADR 0010 §1). INBOX vient d'être faite ; `sync_order`
    // la remet en tête et l'évite en double.
    //
    // LIST-STATUS (RFC 5819) quand le serveur l'annonce : la liste ET le
    // relevé de CHAQUE dossier en UN aller-retour — terrain du
    // 2026-08-13, l'inventaire était le dernier goulot (66 s de ~51
    // STATUS séquentiels sur le compte Gmail). `statuts` en ressort
    // pré-rempli ; la garde d'espace plus bas n'a plus rien à demander.
    // Repli (capacité absente OU échec LIST-STATUS) : un LIST simple,
    // les STATUS partiront un par un — chemin d'avant, intact.
    let mut statuts: HashMap<String, mail_core::FolderStatus> = HashMap::new();
    let avec_statut = match server.folders_with_status() {
        Ok(v) => v,
        Err(reason) => {
            problems.push(format!("inventaire LIST-STATUS : {reason}"));
            None
        }
    };
    let folders = if let Some(avec_statut) = avec_statut {
        let mut folders = Vec::with_capacity(avec_statut.len());
        for (folder, statut) in avec_statut {
            // Le serveur PEUT omettre le relevé d'un dossier (RFC 5819
            // §2) : ce dossier repart alors non gardé, la boucle plus bas
            // le rattrapera par un STATUS ciblé.
            if let Some(statut) = statut {
                statuts.insert(folder.wire.clone(), statut);
            }
            folders.push(folder);
        }
        folders
    } else {
        server.folders().unwrap_or_else(|reason| {
            problems.push(format!("liste des dossiers : {reason}"));
            Vec::new()
        })
    };
    // Rafraîchie UNE fois par cycle — hoistée de `SyncEngine::sync` qui la
    // payait à CHAQUE dossier (~51 LIST par cycle, ADR 0017). Déplacer
    // hors ligne garde sa liste.
    if let Err(reason) = store.replace_folders(account_id, &folders) {
        problems.push(format!("liste des dossiers : {reason}"));
    }
    let order = mail_core::sync_order(&folders, sent.as_deref());

    // La garde d'espace disque (ADR 0010 §4) : estimer AVANT de
    // s'engager, refuser en le chiffrant s'il manque.
    //
    // INBOX est comptée des deux côtés (annonce ET base locale) : la
    // retirer d'un seul ferait sous-estimer le restant.
    //
    // Le relevé de chaque dossier est GARDÉ (ADR 0017) : la garde
    // d'espace et la décision de relève se servent du même relevé — celui
    // de LIST-STATUS s'il a répondu, un STATUS ciblé sinon.
    let mut announced: u64 = 0;
    for boite in &order {
        let statut = if boite == MAILBOX {
            // INBOX a déjà son relevé, payé avant sa relève.
            statut_inbox.ok_or_else(|| "relevé INBOX absent".to_string())
        } else if let Some(statut) = statuts.get(boite).copied() {
            // Déjà relevé par LIST-STATUS : aucun second aller-retour.
            Ok(statut)
        } else {
            server.folder_status(boite).map_err(|err| err.to_string())
        };
        match statut {
            Ok(statut) => {
                announced += u64::from(statut.messages);
                statuts.insert(boite.clone(), statut);
            }
            // Un dossier qui refuse le relevé rend l'estimation basse et
            // sera relevé sans garde. On continue : la garde est une
            // protection, pas un droit de veto — et l'échec est consigné.
            Err(reason) => problems.push(format!("relevé « {boite} » : {reason}")),
        }
    }
    let local = store
        .account_message_count(account_id)
        .map_err(|err| err.to_string())?;
    let pending = announced.saturating_sub(local);
    // L'espace se mesure sur le VOLUME de la base : c'est lui qui
    // encaissera les écritures, pas le disque système.
    let shortfall = match fs4::available_space(db_path.parent().unwrap_or(db_path)) {
        Ok(available) => mail_core::disk_shortfall(pending, available),
        Err(reason) => {
            // Mesure impossible ≠ espace insuffisant. Bloquer le courrier
            // parce qu'un appel système a échoué serait pire que le
            // risque couvert ; l'échec est dit, et SQLite signalera de
            // toute façon un disque plein, écriture par écriture.
            problems.push(format!("espace disque non mesurable : {reason}"));
            None
        }
    };
    let duree_inventaire = chrono.elapsed();
    let n_dossiers = order.len().saturating_sub(1);
    let mut n_sautes = 0usize;
    let chrono = Instant::now();
    if let Some(missing) = shortfall {
        problems.push(format!(
            "espace disque insuffisant : ~{} nécessaires pour {} message(s) \
             restants, il manque {} ; récupération des dossiers suspendue \
             jusqu'à ce que de la place soit libérée",
            format_bytes(pending.saturating_mul(mail_core::SYNC_BYTES_PER_MESSAGE)),
            pending,
            format_bytes(missing),
        ));
    } else {
        for boite in order.into_iter().skip(1) {
            // La relève gardée (ADR 0017) : rien n'a bougé → sauté. Le
            // terrain a payé 26 min de SELECT + SEARCH ALL par cycle
            // pour des dossiers immobiles.
            let statut = statuts.get(&boite);
            if !doit_relever(&store, account_id, &boite, statut, &mut problems) {
                n_sautes += 1;
                continue;
            }
            poser_boite(cycle, &boite);
            match SyncEngine::default().sync(&mut server, &mut store, account_id, &boite) {
                Ok(_) => solder_repere(&store, account_id, &boite, statut, &mut problems),
                Err(reason) => problems.push(format!("dossier « {boite} » : {reason}")),
            }
        }
    }
    let duree_dossiers = chrono.elapsed();
    // La passe d'en-têtes n'est pas une boîte : l'étape est nommée.
    poser_phase(cycle, "fils");
    let chrono = Instant::now();

    // La passe d'en-têtes profite de la connexion déjà ouverte : c'est ce
    // qui la rend gratuite en allers-retours. Son échec ne doit PAS faire
    // échouer la synchronisation — le courrier est arrivé, c'est le seul
    // résultat qui compte — mais il est rapporté, jamais avalé. Sans
    // trace, une boîte qui refuse de se regrouper serait indiagnosticable.
    //
    // Elle passe sur les DEUX boîtes. `References` porte la racine du fil
    // là où `In-Reply-To` ne désigne que le parent immédiat : sans elle,
    // une réponse dont le message d'origine a été archivé hors d'INBOX ne
    // peut pas se raccrocher. L'ADR 0008 (mesure 2) est explicite —
    // `References` est obligatoire, pas un raffinement.
    //
    // Le budget est PARTAGÉ, pas doublé : la seconde boîte ne consomme que
    // ce que la première a laissé. Le coût réseau d'une synchronisation
    // reste donc exactement celui d'avant, et la passe étant reprenable,
    // le reliquat part au tour suivant.
    //
    // SANS horizon depuis l'ADR 0010 : le diagnostic terrain a montré la
    // passe convergée à 1 656 messages lus sur 1 656 éligibles — et 5 883
    // messages hors des 12 mois qui ne seraient JAMAIS lus. La borne
    // venait du budget disque des corps ; un bloc d'en-têtes pèse ~3 ko et
    // ne se range pas sur le disque comme un corps.
    //
    // La passe reste sur INBOX + Envoyés, elle : `References` est le
    // carburant du regroupement, et le regroupement s'arrête à cette
    // portée (ADR 0010 §3). Lire les en-têtes du Spam paierait des
    // allers-retours pour des messages qui ne se rattachent à rien.
    let mut budget = THREAD_HEADER_BUDGET;
    for boite in std::iter::once(MAILBOX).chain(sent.as_deref()) {
        if budget == 0 {
            break;
        }
        match mail_core::backfill_thread_headers(
            &mut server,
            &mut store,
            account_id,
            boite,
            mail_core::NO_HORIZON,
            budget,
        ) {
            Ok(report) => budget = budget.saturating_sub(report.fetched),
            Err(err) => problems.push(format!("conversations incomplètes : {err}")),
        }
    }

    // R4/R1 (PLAN-RETOURS-MAIL) : rattrapage des DESTINATAIRES. La passe
    // d'en-têtes a convergé — les messages déjà synchronisés n'ont aucun
    // À/Cc en base. Deux besoins : dans un dossier d'envois, l'expéditeur
    // est SOI et seul le destinataire dit à qui le message est parti
    // (affichage R4) ; et « Répondre à tous » lit ces mêmes À/Cc pour être
    // instantané, hors ligne (R1 — l'ancienne relève serveur au clic
    // coûtait >10 s). On relit l'ENVELOPE (À/Cc gratuits, avec
    // l'expéditeur) sur la connexion ouverte, borné, reprenable et à budget
    // PARTAGÉ, sur la MÊME portée INBOX + Envoyés que la passe de fils.
    // Best effort : un échec se consigne, il ne fait pas échouer la relève.
    let mut budget_dest = RECIPIENTS_BUDGET;
    for boite in std::iter::once(MAILBOX).chain(sent.as_deref()) {
        if budget_dest == 0 {
            break;
        }
        match mail_core::backfill_recipients(
            &mut server,
            &mut store,
            account_id,
            boite,
            budget_dest,
        ) {
            Ok(report) => budget_dest = budget_dest.saturating_sub(report.fetched),
            Err(err) => problems.push(format!("destinataires manquants : {err}")),
        }
    }

    let duree_fils = chrono.elapsed();
    // Le tirage des brouillons profite lui aussi de la connexion ouverte.
    // Il ne peut PAS vivre dans le cycle de poussée : celui-ci s'arrête
    // tôt quand il n'y a rien à pousser — à raison, sinon chaque frappe
    // ouvrirait une connexion. Un brouillon commencé ailleurs n'arriverait
    // donc jamais.
    poser_phase(cycle, "brouillons");
    let chrono = Instant::now();
    if let Err(reason) = pull_drafts(&mut server, &store, account_id) {
        problems.push(format!("brouillons distants : {reason}"));
    }
    let duree_brouillons = chrono.elapsed();

    // E3 (PLAN-REACTIVITE) : le cycle vient peut-être de faire entrer
    // la vraie ligne d'une destination d'écho — la réconciliation le
    // constate, et la génération resert la liste (l'écho s'efface sous
    // sa vraie ligne, invisible à l'œil).
    match store.reconcilier_echos(account_id) {
        Ok(n) if n > 0 => {
            cycle.generation.fetch_add(1, Ordering::Relaxed);
        }
        Ok(_) => {}
        Err(reason) => problems.push(format!("réconciliation des échos : {reason}")),
    }

    // La trace qui transforme « c'est bloqué » en mesure — lisible dans
    // la console d'un `cargo run`. AVANT logout : un logout qui cale ne
    // doit pas emporter la trace avec lui.
    crate::trace::trace(&format!(
        "relève compte {account_id} : INBOX {:.1}s · inventaire {:.1}s · {n_dossiers} dossiers ({n_sautes} sautés) {:.1}s · fils {:.1}s · brouillons {:.1}s",
        duree_inbox.as_secs_f32(),
        duree_inventaire.as_secs_f32(),
        duree_dossiers.as_secs_f32(),
        duree_fils.as_secs_f32(),
        duree_brouillons.as_secs_f32(),
    ));

    server.logout();

    Ok(SyncOutcome {
        report,
        refreshed,
        problems,
    })
}

/// Rapatrie les brouillons commencés ailleurs, et retire les miroirs
/// devenus périmés.
///
/// La décision appartient au noyau ([`mail_core::plan_draft_pull`], pur et
/// testé) ; ici on ne fait que l'exécuter.
fn pull_drafts(server: &mut ImapServer, store: &Store, account_id: i64) -> Result<(), String> {
    // La garde des repères d'abord, comme dans le cycle de poussée : si
    // l'UIDVALIDITY a changé, les `remote_uid` enregistrés ne désignent
    // plus rien. Comparer la liste distante à des repères périmés ferait
    // passer TOUS les miroirs pour caducs et réimporterait toute la
    // boîte.
    // Pas de dossier Brouillons annoncé : rien à tirer, et rien à
    // signaler. Le serveur n'est pas en panne, il n'a pas la capacité.
    if server
        .drafts_folder_name()
        .map_err(|err| err.to_string())?
        .is_none()
    {
        return Ok(());
    }
    let validity = server.drafts_uidvalidity().map_err(|err| err.to_string())?;
    let reset = store
        .align_drafts_uidvalidity(account_id, validity)
        .map_err(|err| err.to_string())?;
    if reset {
        // Repères abandonnés : rien ne distingue plus nos propres copies
        // de celles d'ailleurs. On laisse le cycle de poussée les
        // rétablir et on tirera au passage suivant. Un doublon reste
        // possible — c'est la règle d'or déjà en vigueur, et elle
        // préfère un doublon à une perte.
        return Ok(());
    }

    let remote = server.draft_uids().map_err(|err| err.to_string())?;
    let local = store.drafts_of(account_id).map_err(|err| err.to_string())?;
    let tombstones = store
        .draft_tombstones(account_id)
        .map_err(|err| err.to_string())?;
    let plan = mail_core::plan_draft_pull(&local, &remote, &tombstones);

    for id in plan.stale {
        store.drop_stale_draft(id).map_err(|err| err.to_string())?;
    }
    for uid in plan.fetch {
        let Some(draft) = server.fetch_draft(uid).map_err(|err| err.to_string())? else {
            // Disparu entre la liste et la lecture : sans conséquence.
            continue;
        };
        // Le corps arrive sous les deux formes MIME possibles ; il passe
        // par LA frontière (`frontiere_corps`) comme tout corps qui entre
        // en base : HTML assaini conservé (un brouillon riche poussé puis
        // re-rapatrié garde sa mise en forme), texte dérivé — le texte
        // MIME ne sert que de repli quand il n'y a pas de HTML.
        let texte = draft.text.unwrap_or_default();
        let (body, body_html) = frontiere_corps(texte, draft.html.as_deref());
        store
            .import_remote_draft(
                account_id,
                uid,
                &draft.to_raw,
                &draft.subject,
                &body,
                body_html.as_deref(),
            )
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

/// Affiche la bulle système d'un lot d'arrivées, s'il y a lieu.
///
/// Un échec — permission refusée, identité applicative non enregistrée —
/// ne doit JAMAIS faire échouer une synchro : le courrier est arrivé,
/// c'est le seul résultat qui compte. Mais il est **rapporté**.
///
/// La première version avalait l'erreur en silence. Sur un poste où
/// Windows refusait le toast, le symptôme était « rien ne se passe » :
/// indiagnosticable. Absorber un échec est une chose, en effacer la
/// trace en est une autre.
/// Clé de la préférence « bulles d'arrivée » (Réglages > Notifications).
const PREF_ARRIVAL_BUBBLES: &str = "arrival_bubbles";
const PREF_LANG: &str = "lang";
/// Epoch (secondes) de la dernière relève réussie — écrit par
/// `sync_inbox`, lu par `sync_progress` pour la barre d'état (E1).
const PREF_DERNIERE_SYNCHRO: &str = "derniere_synchro";

fn arrival_notification_problem(
    app: &AppHandle,
    store: &Store,
    arrivals: &[mail_core::Envelope],
) -> Option<String> {
    use tauri_plugin_notification::NotificationExt;

    // R-D2 (PLAN-REGLAGES) : la préférence vit EN BASE et se lit ICI, à
    // l'émission — le réglage coupe la bulle, jamais la synchro. Base
    // illisible = activées : le défaut protège l'annonce, et la synchro
    // qui vient d'écrire ces arrivées rend ce cas théorique. La même
    // lecture porte la langue des textes (PLAN-LANGUES, E2) :
    // `prefs.lang`, posée par l'UI — absente ou inconnue, français.
    // Sur la connexion de l'appelant (PLAN-AUDIT-V2 E1) : la relève en
    // tient déjà une, en rouvrir une seconde ne protégeait rien.
    let actives = store.bool_pref(PREF_ARRIVAL_BUBBLES, true).unwrap_or(true);
    if !actives {
        return None;
    }
    let lang = mail_core::Lang::from_pref(store.text_pref(PREF_LANG).ok().flatten().as_deref());
    let notification = mail_core::notification_for(arrivals, lang)?;
    app.notification()
        .builder()
        .title(notification.title)
        .body(notification.body)
        .show()
        .err()
        .map(|err| format!("notification non affichée : {err}"))
}

// La page ne porte PLUS de total (terrain 2026-08-20,
// PLAN-DEFILEMENT-PROFOND) : le comptage d'une intégrale Gmail (sonde
// NOT EXISTS par ligne) coûte ~240 ms sur 200 k — plus que la page
// elle-même — et retardait chaque premier rendu. Le total vit dans la
// commande séparée [`category_total`], demandée par le front APRÈS
// l'affichage des lignes ; une page plus courte que sa limite dit
// d'elle-même la fin de la liste.
#[derive(Serialize)]
pub struct MessagePage {
    pub offset: usize,
    pub rows: Vec<MessageRow>,
    pub elapsed_us: u64,
}

/// Les messages d'une conversation, du plus ancien au plus récent.
///
/// Purement locale : ouvrir un fil ne demande jamais le réseau, comme
/// choisir un dossier de destination. C'est la leçon des dossiers, qui
/// avaient été livrés en interrogeant le serveur — inutilisables dès la
/// première coupure.
#[tauri::command]
pub async fn thread_messages(app: AppHandle, thread_id: i64) -> Result<Vec<MessageRow>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        Ok(store
            .thread_messages(thread_id)
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(to_message_row)
            .collect())
    })
    .await
}

/// Mapping partagé entre la boîte unifiée et les résultats de recherche.
fn to_message_row(row: mail_core::UnifiedRow) -> MessageRow {
    MessageRow {
        epoch: row.envelope.date.map(|date| date.timestamp()).unwrap_or(0),
        attachment_count: row.attachment_count,
        preview: row.preview,
        sender_address: row.envelope.sender_address.clone(),
        to_addrs: row.envelope.to_addrs.clone(),
        cc_addrs: row.envelope.cc_addrs.clone(),
        has_attachment: row.has_attachment,
        account_id: row.account_id,
        account_email: row.account_email,
        mailbox: row.mailbox,
        uid: row.envelope.uid,
        subject: row
            .envelope
            .subject
            .unwrap_or_else(|| "(sans sujet)".to_string()),
        sender: row
            .envelope
            .sender
            .unwrap_or_else(|| "(expéditeur inconnu)".to_string()),
        date: row
            .envelope
            .date
            .map(|date| date.format("%Y-%m-%d").to_string())
            .unwrap_or_default(),
        seen: row.envelope.seen,
        flagged: row.envelope.flagged,
        thread_id: row.thread_id,
        thread_size: row.thread_size,
        thread_unseen: row.thread_unseen,
        pinned: false,
        cote: false,
        invitation: row.invitation.map(|rang| InvitationLigne {
            mailbox: rang.mailbox,
            uid: rang.uid,
            titre: rang.titre,
            reponse: rang.reponse,
            annulee: rang.annulee,
            peut_repondre: rang.peut_repondre,
        }),
    }
}

/// Un compte de la nav v2 (écran 02), avec ses compteurs — dossiers
/// canoniques résolus côté cœur (`nav.rs`), l'UI ne voit jamais un nom
/// de boîte réseau.
#[derive(Serialize)]
pub struct NavAccount {
    pub account_id: i64,
    pub email: String,
    // La nav ne dit QUE le non-lu (A29) : la sonde à 10 s ne paie que
    // ces deux compteurs — l'inventaire complet (`nav_counts`, dont le
    // total d'une intégrale à ~240 ms la sonde) ne se recalcule plus au
    // battement (terrain 2026-08-20, PLAN-DEFILEMENT-PROFOND).
    pub reception_non_lues: u64,
    pub indesirables_non_lus: u64,
}

/// L'état complet de la nav en UN appel : comptes et compteurs par
/// catégorie. « Toutes les boîtes » s'agrège côté UI.
#[tauri::command]
pub async fn nav_snapshot(app: AppHandle) -> Result<Vec<NavAccount>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        // E2 : en mode organisé, la pastille de la Réception suit
        // l'exclusion partagée — le non-lu d'un retenu appartient à la
        // pastille du Portier, jamais aux deux.
        let organise = store.mode_organise().map_err(|err| err.to_string())?;
        let mut sortie = Vec::new();
        for compte in store.accounts().map_err(|err| err.to_string())? {
            let dossiers = store
                .canonical_folders(compte.id)
                .map_err(|err| err.to_string())?;
            let (reception_non_lues, indesirables_non_lus) = store
                .nav_unread_counts(compte.id, &dossiers, organise)
                .map_err(|err| err.to_string())?;
            sortie.push(NavAccount {
                account_id: compte.id,
                email: compte.email,
                reception_non_lues,
                indesirables_non_lus,
            });
        }
        Ok(sortie)
    })
    .await
}

/// Une page d'une catégorie de la nav, bornée ou non à un compte.
/// `reception` = la boîte unifiée (conversations) ; les autres = les
/// messages des boîtes canoniques résolues, fusionnés par date.
#[tauri::command]
pub async fn list_category(
    app: AppHandle,
    category: String,
    account_id: Option<i64>,
    non_lus: bool,
    offset: usize,
    limit: usize,
) -> Result<MessagePage, String> {
    hors_pompe(app, move |app| {
        let timer = Instant::now();
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let limit = limit.min(LIST_LIMIT_MAX);
        if category == "reception" {
            // E2 : en Mode organisé, la Réception RETIENT les fils des
            // expéditeurs en attente au Portier et ceux routés ailleurs
            // (drapeau + index partiel — jamais une sonde par rangée).
            // Le mode classique passe par la requête HISTORIQUE, au
            // caractère près : zéro diff (garde e2e).
            let organise = store.mode_organise().map_err(|err| err.to_string())?;
            let mut lignes = if organise {
                store
                    .reception_organisee_scoped(account_id, non_lus, offset, limit)
                    .map_err(|err| err.to_string())?
            } else {
                store
                    .unified_recent_scoped(account_id, non_lus, offset, limit)
                    .map_err(|err| err.to_string())?
            };
            // Terrain R10-R12 : pièces sommées par fil, invitations au
            // rang — une passe bornée à la PAGE, la requête chaude ne
            // paie rien.
            store
                .enrichir_lignes(&mut lignes)
                .map_err(|err| err.to_string())?;
            let rows = lignes.into_iter().map(to_message_row).collect();
            return Ok(MessagePage {
                offset,
                rows,
                elapsed_us: timer.elapsed().as_micros() as u64,
            });
        }
        // PLAN-MODE-ORGANISE E1 : le Kiosque et le Registre sont des
        // vues du flot unifié filtrées par le routage d'expéditeur —
        // jamais des boîtes canoniques.
        if category == "kiosque" || category == "registre" {
            let mut lignes = store
                .routage_unified_scoped(&category, account_id, non_lus, offset, limit)
                .map_err(|err| err.to_string())?;
            store
                .enrichir_lignes(&mut lignes)
                .map_err(|err| err.to_string())?;
            let rows = lignes.into_iter().map(to_message_row).collect();
            return Ok(MessagePage {
                offset,
                rows,
                elapsed_us: timer.elapsed().as_micros() as u64,
            });
        }
        let portee = resoudre_categorie(&store, &category, account_id)?;
        // E3 (PLAN-REACTIVITE) : les échos locaux des destinations de geste
        // entrent dans la page et le total — la Corbeille montre la
        // suppression, Envoyés l'envoi, à la seconde du geste.
        let echos = mail_core::DESTINATIONS_ECHO
            .contains(&category.as_str())
            .then_some((category.as_str(), portee.comptes.as_slice()));
        let mut lignes = store
            .category_page(
                &portee.boites,
                non_lus,
                &portee.exclure,
                echos,
                offset,
                limit,
            )
            .map_err(|err| err.to_string())?;
        store
            .enrichir_lignes(&mut lignes)
            .map_err(|err| err.to_string())?;
        let rows = lignes.into_iter().map(to_message_row).collect();
        Ok(MessagePage {
            offset,
            rows,
            elapsed_us: timer.elapsed().as_micros() as u64,
        })
    })
    .await
}

/// Comptes en portée, boîtes résolues et exclusion d'intégrale d'une
/// catégorie hors réception — la résolution PARTAGÉE de la page
/// (`list_category`) et du comptage (`category_total`).
struct PorteeCategorie {
    comptes: Vec<i64>,
    boites: Vec<i64>,
    exclure: Vec<i64>,
}

fn resoudre_categorie(
    store: &Store,
    category: &str,
    account_id: Option<i64>,
) -> Result<PorteeCategorie, String> {
    let comptes: Vec<i64> = match account_id {
        Some(id) => vec![id],
        None => store
            .accounts()
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|compte| compte.id)
            .collect(),
    };
    let mut boites = Vec::new();
    // Les Archives d'une INTÉGRALE Gmail (« Tous les messages ») privent
    // la catégorie des messages vivant dans une autre canonique — sinon
    // elle montre toute la boîte (défaut terrain, 2026-08-12).
    let mut exclure = Vec::new();
    for compte in &comptes {
        let dossiers = store
            .canonical_folders(*compte)
            .map_err(|err| err.to_string())?;
        if let Some(nom) = dossiers.boite(category)
            && let Some(state) = store
                .sync_state(*compte, &nom)
                .map_err(|err| err.to_string())?
        {
            boites.push(state.mailbox_id);
            if category == "archives" && dossiers.archives_integrale {
                exclure.extend(
                    store
                        .canoniques_hors_archives(*compte, &dossiers)
                        .map_err(|err| err.to_string())?,
                );
            }
        }
    }
    Ok(PorteeCategorie {
        comptes,
        boites,
        exclure,
    })
}

/// Le total d'une catégorie — la commande SÉPARÉE du service des pages
/// (terrain 2026-08-20, PLAN-DEFILEMENT-PROFOND) : le comptage d'une
/// intégrale (sonde NOT EXISTS par ligne, ~240 ms sur 200 k) ne doit
/// jamais retarder un premier rendu — le front l'appelle quand sa
/// pompe de pages est au repos, et la barre de défilement s'ajuste à
/// l'arrivée.
#[tauri::command]
pub async fn category_total(
    app: AppHandle,
    category: String,
    account_id: Option<i64>,
    non_lus: bool,
) -> Result<u64, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        if category == "reception" {
            // E2 : le total suit le flot — exclusion PARTAGÉE avec la
            // page (leçon `pins`), et le classique reste intact.
            let organise = store.mode_organise().map_err(|err| err.to_string())?;
            return if organise {
                store
                    .reception_organisee_count_scoped(account_id, non_lus)
                    .map_err(|err| err.to_string())
            } else {
                store
                    .unified_count_scoped(account_id, non_lus)
                    .map_err(|err| err.to_string())
            };
        }
        if category == "kiosque" || category == "registre" {
            return store
                .routage_count_scoped(&category, account_id, non_lus)
                .map_err(|err| err.to_string());
        }
        let portee = resoudre_categorie(&store, &category, account_id)?;
        let echos = mail_core::DESTINATIONS_ECHO
            .contains(&category.as_str())
            .then_some((category.as_str(), portee.comptes.as_slice()));
        let (tous, jamais_lus) = store
            .category_totals(&portee.boites, &portee.exclure, echos)
            .map_err(|err| err.to_string())?;
        Ok(if non_lus { jamais_lus } else { tous })
    })
    .await
}

/// Rattrape l'aperçu des corps écrits avant la colonne `preview`, par
/// lots bornés — l'UI l'appelle au fil de son sondage jusqu'à zéro,
/// jamais sur le chemin d'ouverture. Rend le nombre restant.
#[tauri::command]
pub async fn preview_catchup(app: AppHandle, limit: usize) -> Result<u64, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store.preview_catchup(limit).map_err(|err| err.to_string())
    })
    .await
}

/// Les résultats d'une recherche : les lignes rendues (plafonnées à
/// `SEARCH_LIMIT`) et le nombre TOTAL de correspondances — pour dire
/// « 100 sur N » quand le rendu est plafonné.
#[derive(Serialize)]
pub struct SearchResults {
    pub rows: Vec<MessageRow>,
    pub total: u64,
}

/// Recherche plein-texte sur tous les comptes, une tranche à la fois.
/// `offset` sert « charger plus » : 0 à la frappe, puis le nombre de lignes
/// déjà affichées. Le déclenchement à partir de 3 caractères et le debounce
/// sont de la responsabilité de l'UI.
#[tauri::command]
pub async fn search_messages(
    app: AppHandle,
    query: String,
    offset: usize,
) -> Result<SearchResults, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        // `search_capped` rend la tranche `[offset, offset+SEARCH_LIMIT)` ET le
        // total exact, et bascule sur le tri par date au-delà du seuil de
        // requête large (le classement BM25 y dépasse le budget et ne veut plus
        // rien dire — ADR 0004). Le tri ne dépendant que du total, les tranches
        // s'enchaînent sans trou ni doublon.
        let (hits, total) = store
            .search_capped(&query, SEARCH_LIMIT, offset)
            .map_err(|err| err.to_string())?;
        let rows = hits.into_iter().map(to_message_row).collect();
        Ok(SearchResults { rows, total })
    })
    .await
}

#[derive(Serialize)]
pub struct BodyView {
    pub document: String,
    pub remote_images_blocked: usize,
    /// Le compte de pièces D'APRÈS-SCAN : la première ouverture d'un
    /// message vient d'écrire ses pièces en base (`load_body`), mais la
    /// ligne de liste qui a mené ici portait le compte d'AVANT — s'y
    /// fier faisait ouvrir les pièces jointes fraîchement reçues sur
    /// une rangée vide (terrain CE, 2026-08-14).
    pub attachment_count: usize,
    /// La carte d'invitation du message, ENTIÈRE (ligne `invitations`
    /// fraîche du même scan) : elle voyage avec le corps — une seconde
    /// commande pour la relire coûtait un aller-retour IPC et une
    /// requête en double par ouverture (revue).
    pub invitation: Option<InvitationVue>,
}

/// Corps d'un message : cache local d'abord (aucun réseau), serveur du
/// compte sinon. Document auto-CSP chargé dans une iframe `sandbox` —
/// les trois couches de défense de la Phase 0.
#[tauri::command]
pub async fn message_body(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    show_images: bool,
) -> Result<BodyView, String> {
    // Chemin courant — corps en cache : UNE prise du verrou, UNE ouverture
    // (revue PLAN-AUDIT-V1 : `raw_body` puis un second `hors_pompe`
    // prenaient le verrou deux fois pour rien).
    let boite = mailbox.clone();
    let en_cache = hors_pompe(app.clone(), move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        match store
            .body(account_id, &boite, uid)
            .map_err(|err| err.to_string())?
        {
            Some(html) => {
                vue_du_corps(&store, account_id, &boite, uid, show_images, &html).map(Some)
            }
            None => Ok(None),
        }
    })
    .await?;
    if let Some(vue) = en_cache {
        return Ok(vue);
    }
    // Corps absent : rapatriement réseau (nu), puis la vue sous le verrou.
    let html = raw_body(&app, account_id, &mailbox, uid).await?;
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        vue_du_corps(&store, account_id, &mailbox, uid, show_images, &html)
    })
    .await
}

/// La vue d'un corps déjà en base : garde d'images, pièces, invitation,
/// assainissement (du CPU : un corps de 28 Mo, D-1) — sous le verrou des
/// commandes (E5), jamais sur un worker async nu.
fn vue_du_corps(
    store: &Store,
    account_id: i64,
    mailbox: &str,
    uid: u32,
    show_images: bool,
    html: &str,
) -> Result<BodyView, String> {
    {
        // R1 (PLAN-RETOURS-11, D1) : la mémoire de la garde d'images se
        // consulte ICI — l'autorité est le cœur, l'UI ne décide rien
        // (elle ne voit qu'un `remote_images_blocked` à zéro, donc pas
        // de bandeau). Trois lectures indexées au pire (point-lookups
        // sur PK), et aucune quand `show_images` tranche déjà.
        let images_accordees = if show_images {
            true
        } else {
            store
                .sync_state(account_id, mailbox)
                .map_err(|err| err.to_string())?
                .map(|s| store.images_allowed(s.mailbox_id, uid))
                .transpose()
                .map_err(|err| err.to_string())?
                .unwrap_or(false)
        };
        let attachment_count = store
            .attachments(account_id, mailbox, uid)
            .map_err(|err| err.to_string())?
            .len();
        let invitation = store
            .invitation(account_id, mailbox, uid)
            .map_err(|err| err.to_string())?
            .map(vue_invitation);

        let policy = if images_accordees {
            mail_render::ImagePolicy::AllowRemote
        } else {
            mail_render::ImagePolicy::BlockRemote
        };
        let sanitized = mail_render::sanitize_with(html, policy);
        // R3 (PLAN-RETOURS-4, D3, 2026-08-18) : le corps s'affiche TOUJOURS
        // sur dalle claire (`Palette::default` = encre sombre / fond blanc),
        // quel que soit le thème. La dalle sombre d'A42 rendait illisible le
        // texte à couleurs d'expéditeur (fréquent : infolettres pensées pour
        // fond blanc — terrain 2026-08-18) ; le courriel se lit tel qu'il a
        // été composé, comme chez les clients mûrs. Le texte SANS couleur
        // propre était déjà lisible ; celui qui en porte l'est désormais aussi.
        Ok(BodyView {
            document: mail_render::email_document(
                &sanitized.html,
                policy,
                &mail_render::Palette::default(),
            ),
            remote_images_blocked: sanitized.remote_images_blocked,
            attachment_count,
            invitation,
        })
    }
}

fn fetch_body(
    session: &AccountSession,
    db_path: &Path,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<String, String> {
    let (mut server, _refreshed) = connect_imap(session)?;
    let mut store = Store::open(db_path).map_err(|err| err.to_string())?;
    let body = mail_core::load_body(&mut server, &mut store, account_id, &mailbox, uid)
        .map_err(|err| err.to_string())?;
    server.logout();
    body.ok_or_else(|| "message introuvable sur le serveur".to_string())
}

/// Corps HTML brut d'un message : cache local d'abord (aucun réseau),
/// serveur du compte sinon — chemin partagé lecture/réponse/transfert.
async fn raw_body(
    app: &AppHandle,
    account_id: i64,
    mailbox: &str,
    uid: u32,
) -> Result<String, String> {
    // E5 : la lecture du cache et la session sous `hors_pompe` (base +
    // verrou des commandes) ; seul le rapatriement réseau part nu.
    let boite = mailbox.to_string();
    let en_cache = hors_pompe(app.clone(), move |app| {
        let cached = Store::open(&db_path(&app)?)
            .and_then(|store| store.body(account_id, &boite, uid))
            .map_err(|err| err.to_string())?;
        match cached {
            Some(html) => Ok(Ok(html)),
            None => Ok(Err(auth_for(&app, account_id)?)),
        }
    })
    .await?;
    match en_cache {
        Ok(html) => Ok(html),
        Err(session) => {
            let path = db_path(app)?;
            // Copie possedee : la fermeture part sur un autre fil.
            let boite = mailbox.to_string();
            tauri::async_runtime::spawn_blocking(move || {
                fetch_body(&session, &path, account_id, boite, uid)
            })
            .await
            .map_err(|err| err.to_string())?
        }
    }
}

/// Une pièce jointe telle que l'UI la présente.
#[derive(Serialize)]
pub struct AttachmentRow {
    pub index: usize,
    pub name: String,
    pub mime: String,
    pub size: String,
}

/// Les pièces jointes connues d'un message — lecture LOCALE, aucun
/// réseau. Vide tant que le corps n'a pas été rapatrié : même condition
/// que la recherche dans le texte, et le rattrapage la lève.
#[tauri::command]
pub async fn message_attachments(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<Vec<AttachmentRow>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let found = store
            .attachments(account_id, &mailbox, uid)
            .map_err(|err| err.to_string())?;
        Ok(found
            .into_iter()
            .map(|attachment| AttachmentRow {
                index: attachment.index,
                size: attachment.human_size(),
                name: attachment.name,
                mime: attachment.mime,
            })
            .collect())
    })
    .await
}

/// La carte d'invitation d'un message, telle que l'UI la présente
/// (PLAN-INVITATIONS). Les horaires sont des epochs UTC quand ils sont
/// résolus ; sinon la forme TEXTE fait foi (`journee_entiere` ou
/// `heure_flottante` — l'UI affiche cette dernière telle quelle, avec la
/// mention « heure locale de l'organisateur », garde D1).
#[derive(Serialize)]
pub struct InvitationVue {
    /// `request` | `cancel` | `reply`.
    pub methode: String,
    pub titre: String,
    pub lieu: Option<String>,
    /// Le nom d'affichage de l'organisateur, sinon son adresse.
    pub organisateur: Option<String>,
    pub debut_epoch: Option<i64>,
    pub fin_epoch: Option<i64>,
    pub debut_texte: Option<String>,
    pub fin_texte: Option<String>,
    pub journee_entiere: bool,
    pub heure_flottante: bool,
    pub recurrent: bool,
    /// Notre dernière réponse partie de Wind (`accepte` | `provisoire` |
    /// `refuse`), sinon le PARTSTAT lu du message.
    pub statut: Option<String>,
    /// Le répondant d'un REPLY reçu (nom, sinon adresse) et son statut.
    pub repondant: Option<String>,
    pub repondant_statut: Option<String>,
    /// La réunion est annulée : vrai sur le CANCEL lui-même ET sur le
    /// REQUEST de la même réunion (lien croisé, terrain R6) — la carte
    /// d'origine dit l'annulation, où que l'utilisateur regarde.
    pub annulee: bool,
    /// Les trois gestes sont possibles : REQUEST avec organisateur, non
    /// annulé. Être dans la liste ATTENDEE n'est PAS exigé (terrain
    /// R8, verdict CE : une invitation transférée EST une invitation —
    /// qui la transfère en prend la responsabilité).
    pub peut_repondre: bool,
}

fn vue_invitation(stockee: mail_core::InvitationStockee) -> InvitationVue {
    let row = stockee.row;
    let annulee = row.methode == "cancel" || row.annule;
    let peut_repondre =
        row.methode == "request" && row.organisateur_adresse.is_some() && !row.annule;
    InvitationVue {
        annulee,
        // La garde D1 par EXTRÉMITÉ : une fin au TZID irrésolu suffit à
        // dire « heure locale de l'organisateur » (revue — un couple
        // début-résolu/fin-flottante affichait une plage mensongère).
        heure_flottante: (row.debut_texte.is_some() || row.fin_texte.is_some())
            && !row.journee_entiere,
        organisateur: row.organisateur_nom.or(row.organisateur_adresse),
        // D6 : l'état affiché suit la DERNIÈRE réponse partie de Wind ;
        // le PARTSTAT du message n'est que l'état de départ.
        statut: stockee.reponse.or(row.partstat),
        repondant: row.repondant_nom.or(row.repondant_adresse),
        repondant_statut: row.repondant_statut,
        methode: row.methode,
        titre: row.titre,
        lieu: row.lieu,
        debut_epoch: row.debut_epoch,
        fin_epoch: row.fin_epoch,
        debut_texte: row.debut_texte,
        fin_texte: row.fin_texte,
        journee_entiere: row.journee_entiere,
        recurrent: row.recurrent,
        peut_repondre,
    }
}

/// Répond à une invitation (PLAN-INVITATIONS, D5-D6) : l'email iTIP
/// `METHOD:REPLY` est JOURNALISÉ dans la boîte d'envoi (règles d'or ADR
/// 0003 — hors ligne, il part au prochain lancement), la réponse est
/// consignée sur la carte, et la vue à jour est rendue. Le sujet et le
/// corps viennent de l'UI : c'est elle qui parle la langue du produit.
#[tauri::command]
pub async fn repondre_invitation(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    reponse: String,
    sujet: String,
    corps: String,
) -> Result<Option<InvitationVue>, String> {
    hors_pompe(app, move |app| {
        let participation = mail_core::participation_de_stable(&reponse)
            .filter(|p| !matches!(p, mail_ical::Participation::SansReponse))
            .ok_or_else(|| format!("réponse inconnue : {reponse}"))?;
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let stockee = store
            .invitation(account_id, &mailbox, uid)
            .map_err(|err| err.to_string())?
            .ok_or_else(|| "aucune invitation sur ce message".to_string())?;
        if stockee.row.methode != "request" || stockee.row.annule {
            // Même règle que `peut_repondre` — R8 : un `.ics` transféré
            // EST une invitation (verdict CE) ; une réunion annulée ne
            // se répond plus.
            return Err("ce message n'est pas une invitation à répondre".to_string());
        }
        let organisateur = stockee
            .row
            .organisateur_adresse
            .clone()
            .ok_or_else(|| "invitation sans organisateur : réponse impossible".to_string())?;
        let from = account_email(&store, account_id)?;
        // La réponse rejoint la conversation de l'invitation (fil).
        let in_reply_to = store
            .envelope(account_id, &mailbox, uid)
            .map_err(|err| err.to_string())?
            .and_then(|envelope| envelope.message_id);
        let mut draft = mail_core::compose(
            &from,
            &organisateur,
            "",
            "",
            &sujet,
            &corps,
            in_reply_to.as_deref(),
        )
        .map_err(|err| err.to_string())?;
        // E7 : la chaîne References entière (RFC 5322 §3.6.4).
        draft.references = store
            .references_de(account_id, &mailbox, uid)
            .map_err(|err| err.to_string())?;
        draft.ics_reply = Some(mail_ical::reponse_itip(&mail_ical::DemandeReponse {
            uid: &stockee.row.event_uid,
            sequence: stockee.row.sequence,
            organisateur_adresse: &organisateur,
            notre_adresse: &from,
            participation,
            dtstamp_epoch: chrono::Utc::now().timestamp(),
        }));
        // Email ET réponse dans UNE transaction (revue) : si la ligne a
        // disparu entre l'affichage et le clic, RIEN ne part — un email
        // en file devant une carte « pas répondu » inviterait au double
        // envoi.
        let journalise = store
            .enqueue_reponse_invitation(
                account_id,
                &draft,
                &mailbox,
                uid,
                &reponse,
                chrono::Utc::now().timestamp(),
            )
            .map_err(|err| err.to_string())?;
        if journalise.is_none() {
            return Err("l'invitation n'existe plus ; rien n'est parti".to_string());
        }
        let maj = store
            .invitation(account_id, &mailbox, uid)
            .map_err(|err| err.to_string())?;
        Ok(maj.map(vue_invitation))
    })
    .await
}

#[derive(Serialize)]
pub struct CorrespondantRow {
    pub address: String,
    pub name: Option<String>,
}

/// Les suggestions d'adresses pour un préfixe tapé dans À/Cc/Cci
/// (PLAN-RETOURS-5, D3/D4) : l'annuaire des correspondants — table
/// petite, appris du courrier vu — classé récence + fréquence. Lecture
/// locale, aucun réseau.
#[tauri::command]
pub async fn completer_adresses(
    app: AppHandle,
    prefixe: String,
    limite: usize,
) -> Result<Vec<CorrespondantRow>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let trouves = store
            .completer_adresses(&prefixe, limite.min(16))
            .map_err(|err| err.to_string())?;
        Ok(trouves
            .into_iter()
            .map(|c| CorrespondantRow {
                address: c.address,
                name: c.name,
            })
            .collect())
    })
    .await
}

/// Le chemin d'enregistrement PROPOSÉ pour une pièce (R1, PLAN-RETOURS-4,
/// D2) : dossier Téléchargements + nom assaini rendu unique. Le nom vient
/// de l'UI (déjà affiché dans la puce) — inutile de rouvrir la base pour
/// le relire ; `safe_file_name` reste l'autorité de désinfection du nom
/// venu du réseau (défense en profondeur, même si le dialogue laisse
/// ensuite l'utilisateur trancher dossier ET nom finals).
#[tauri::command]
pub async fn chemin_enregistrement_suggere(app: AppHandle, name: String) -> Result<String, String> {
    hors_pompe(app, move |app| {
        let directory = app
            .path()
            .download_dir()
            .map_err(|err| format!("dossier Téléchargements introuvable : {err}"))?;
        Ok(unique_path(&directory, &safe_file_name(&name))
            .to_string_lossy()
            .into_owned())
    })
    .await
}

/// Enregistre une pièce jointe au chemin CHOISI par l'utilisateur (R1,
/// PLAN-RETOURS-4, D2) et retourne ce chemin. Le dialogue « Enregistrer
/// sous » est ouvert côté UI (`plugin:dialog|save`) ; ici on ne fait que
/// rapatrier les octets — jamais en cache, retéléchargés à la demande —
/// et les écrire à l'endroit voulu.
#[tauri::command]
pub async fn save_attachment(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    index: usize,
    dest: String,
) -> Result<String, String> {
    let session = hors_pompe(app.clone(), move |app| auth_for(&app, account_id)).await?;
    let bytes = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let (mut server, _refreshed) = connect_imap(&session)?;
        let bytes = server
            .fetch_attachment(&mailbox, uid, index)
            .map_err(|err| err.to_string())?;
        server.logout();
        bytes.ok_or_else(|| "pièce jointe absente du message".to_string())
    })
    .await
    .map_err(|err| err.to_string())??;

    // E5 : l'écriture sur disque (des octets choisis par l'expéditeur,
    // jusqu'à 25 Mo) hors du worker async nu.
    hors_pompe(app, move |_| {
        let dest = chemin_de_sortie(&dest)?;
        std::fs::write(&dest, &bytes).map_err(|err| format!("écriture impossible : {err}"))?;
        Ok(dest.to_string_lossy().into_owned())
    })
    .await
}

/// Le chemin où une pièce reçue peut s'écrire (PLAN-AUDIT-V2 E8, défense
/// en profondeur) : venu du dialogue de la webview, il s'écrit avec des
/// octets choisis par l'expéditeur — absolu, sans remontée `..`, dans un
/// dossier qui existe. Décision pure, testée.
fn chemin_de_sortie(dest: &str) -> Result<std::path::PathBuf, String> {
    let chemin = std::path::Path::new(dest);
    if !chemin.is_absolute() {
        return Err("chemin d'enregistrement relatif refusé".to_string());
    }
    if chemin
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("chemin d'enregistrement avec remontée refusé".to_string());
    }
    if chemin.file_name().is_none() {
        return Err("chemin d'enregistrement sans nom de fichier".to_string());
    }
    match chemin.parent() {
        Some(dossier) if dossier.is_dir() => Ok(chemin.to_path_buf()),
        _ => Err("dossier d'enregistrement introuvable".to_string()),
    }
}

/// Réduit un nom venu du RÉSEAU à un nom de fichier inoffensif.
///
/// Un nom de pièce jointe est une chaîne choisie par l'expéditeur. Tel
/// quel, `../../.ssh/authorized_keys` écrirait hors du dossier voulu :
/// c'est une écriture arbitraire de fichier, déclenchée par un simple
/// clic sur un message reçu. Rien de ce qui suit n'est de la prudence
/// excessive.
fn safe_file_name(raw: &str) -> String {
    // Ne garder que le dernier segment : tout séparateur, toute
    // remontée `..` et tout préfixe de lecteur disparaissent avec lui.
    let base = raw
        .rsplit(['/', '\\', ':'])
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('.');
    let cleaned: String = base
        .chars()
        .map(|c| match c {
            // Interdits par Windows, plus les caractères de contrôle.
            '<' | '>' | '"' | '|' | '?' | '*' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .take(120)
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() || is_reserved_device_name(&cleaned) {
        return "piece-jointe".to_string();
    }
    cleaned
}

/// Noms réservés par Windows : un fichier nommé `CON` ou `LPT1` est
/// refusé par l'OS, quelle que soit l'extension.
fn is_reserved_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.ends_with(|c: char| c.is_ascii_digit() && c != '0'))
}

/// Chemin libre dans `directory` : `facture.pdf`, puis `facture (2).pdf`…
/// Enregistrer deux fois ne doit jamais écraser le premier fichier.
fn unique_path(directory: &Path, name: &str) -> PathBuf {
    let candidate = directory.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => (stem, format!(".{extension}")),
        _ => (name, String::new()),
    };
    for n in 2..1000 {
        let candidate = directory.join(format!("{stem} ({n}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(format!("{stem} ({}){extension}", std::process::id()))
}

/// Archive : disparition locale immédiate + journalisation, le serveur
/// du compte suivra au prochain sync.
#[tauri::command]
pub async fn archive_message(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<(), String> {
    hors_pompe(app, move |app| {
        queue_removal(&app, account_id, mailbox, uid, Action::Archive)
    })
    .await
}

/// Une rangée cochée, telle que la barre de sélection la nomme.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CibleArg {
    pub account_id: i64,
    pub mailbox: String,
    pub uid: u32,
    pub thread_id: Option<i64>,
}

#[derive(Serialize)]
pub struct BilanGroupe {
    pub faits: usize,
    pub total: usize,
}

/// Le geste de MASSE de la barre de sélection (PLAN-AUDIT-V2 E6) : UN
/// appel, UNE transaction, tout ou rien (D6) — l'UI rejouait N × k
/// commandes unitaires en série. `action` : les clés de la barre
/// (archiver, supprimer, spam, nonspam, lu, nonlu).
#[tauri::command]
pub async fn agir_groupe(
    app: AppHandle,
    cibles: Vec<CibleArg>,
    action: String,
) -> Result<BilanGroupe, String> {
    let geste = match action.as_str() {
        "archiver" => mail_core::GesteGroupe::Archive,
        "supprimer" => mail_core::GesteGroupe::Delete,
        "spam" => mail_core::GesteGroupe::Spam,
        "nonspam" => mail_core::GesteGroupe::NotSpam,
        "lu" => mail_core::GesteGroupe::Seen(true),
        "nonlu" => mail_core::GesteGroupe::Seen(false),
        autre => return Err(format!("geste de masse inconnu : {autre}")),
    };
    let total = cibles.len();
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let cibles: Vec<mail_core::CibleGeste> = cibles
            .into_iter()
            .map(|cible| mail_core::CibleGeste {
                account_id: cible.account_id,
                mailbox: cible.mailbox,
                uid: cible.uid,
                thread_id: cible.thread_id,
            })
            .collect();
        let faits = store
            .agir_groupe(&cibles, &geste)
            .map_err(|err| err.to_string())?;
        Ok(BilanGroupe { faits, total })
    })
    .await
}

/// Un dossier proposé à l'utilisateur.
#[derive(Serialize)]
pub struct FolderRow {
    /// Nom RÉSEAU — c'est lui que l'UI renverra pour un déplacement.
    pub wire: String,
    /// Nom lisible, décodé de l'UTF-7 modifié.
    pub display: String,
}

/// Les dossiers d'un compte où un message peut être déplacé.
///
/// Lecture **purement locale** : le cache est rempli par la synchro.
/// Déplacer un message doit marcher hors ligne — l'action est journalisée
/// et rejouée, comme archiver. Interroger le serveur ici rendrait le tri
/// dépendant du réseau, ce que le produit refuse (PLAN.md §1).
///
/// La boîte courante est exclue : « déplacer vers INBOX » depuis INBOX
/// n'a pas de sens, et certains serveurs le refusent.
#[tauri::command]
pub async fn list_folders(app: AppHandle, account_id: i64) -> Result<Vec<FolderRow>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        Ok(store
            .folders(account_id)
            .map_err(|err| err.to_string())?
            .into_iter()
            .filter(|folder| folder.selectable && folder.wire != MAILBOX)
            .map(|folder| FolderRow {
                wire: folder.wire,
                display: folder.display,
            })
            .collect())
    })
    .await
}

/// Déplace un message : disparition locale immédiate + journalisation,
/// le serveur suivra au prochain sync — même boucle qu'archiver.
#[tauri::command]
pub async fn move_message(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    folder: String,
) -> Result<(), String> {
    hors_pompe(app, move |app| {
        // Le nom vient de l'UI, qui le tient de `list_folders` : il est déjà
        // en forme réseau. Le décoder ici ferait échouer le rejeu.
        if folder.trim().is_empty() {
            return Err("dossier de destination manquant".to_string());
        }
        queue_removal(&app, account_id, mailbox, uid, Action::MoveTo(folder))
    })
    .await
}

/// Suppression : disparition locale immédiate + journalisation, mise à
/// la corbeille du serveur du compte au prochain sync.
#[tauri::command]
pub async fn delete_message(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<(), String> {
    hors_pompe(app, move |app| {
        queue_removal(&app, account_id, mailbox, uid, Action::Delete)
    })
    .await
}

/// Signaler un message comme indésirable (R2, PLAN-RETOURS-3) : il part
/// vers le dossier Junk du serveur — c'est LUI qui apprend (Gmail entraîne
/// son filtre sur le déplacement). Même boucle qu'archiver : disparition
/// locale immédiate, action `MoveTo` journalisée et rejouée, le serveur
/// suit. Le dossier indésirable est résolu par compte
/// (`canonical_folders`) ; sans dossier reconnu, le geste échoue
/// franchement plutôt que d'inventer une destination.
#[tauri::command]
pub async fn report_spam(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let dossiers = store
            .canonical_folders(account_id)
            .map_err(|err| err.to_string())?;
        let Some(spam) = dossiers.indesirables else {
            return Err("aucun dossier indesirable reconnu sur ce compte".to_string());
        };
        // Déjà dans les indésirables : rien à faire (la vue n'offre pas le
        // geste, mais la garde évite un déplacement vers soi-même).
        if spam == mailbox {
            return Ok(());
        }
        queue_removal(&app, account_id, mailbox, uid, Action::MoveTo(spam))
    })
    .await
}

/// L'inverse (R2) : un message classé à tort indésirable revient en
/// Réception. Offert depuis la seule vue Indésirables. Même boucle —
/// `MoveTo(INBOX)` journalisé, le serveur réconcilie, le fil se
/// reconstitue à la relève de INBOX (ADR 0009).
#[tauri::command]
pub async fn mark_not_spam(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<(), String> {
    hors_pompe(app, move |app| {
        queue_removal(
            &app,
            account_id,
            mailbox,
            uid,
            Action::MoveTo(MAILBOX.to_string()),
        )
    })
    .await
}

fn queue_removal(
    app: &AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    action: Action,
) -> Result<(), String> {
    let store = Store::open(&db_path(app)?).map_err(|err| err.to_string())?;
    let Some(state) = store
        .sync_state(account_id, &mailbox)
        .map_err(|err| err.to_string())?
    else {
        return Ok(());
    };
    // E3 (PLAN-REACTIVITE, R-D1) : le geste est un DÉPLACEMENT — la
    // matière du message passe à l'écho de destination dans la MÊME
    // transaction que le journal d'action et la disparition de la
    // source. La destination se montre < 1 s, hors ligne compris ; le
    // serveur réconcilie derrière (`sync_apres_geste`). Un déplacement
    // vers un dossier libre n'a pas de liste canonique : pas d'écho.
    let destination = match &action {
        Action::Delete => Some("corbeille"),
        Action::Archive => Some("archives"),
        _ => None,
    };
    store
        .geste_avec_echo(state.mailbox_id, uid, action, destination)
        .map_err(|err| err.to_string())
}

/// Le corps d'un écho local (E3) pour la Lecture : même assainissement
/// que `message_body` (S1 — le HTML d'origine est celui de l'expéditeur,
/// le texte d'envoi est déjà échappé mais repasse par la même porte).
/// Purement local — un écho n'a rien à demander au serveur.
#[tauri::command]
pub async fn echo_body(app: AppHandle, id: i64, show_images: bool) -> Result<BodyView, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let (html, attachment_count) = store
            .echo_vue(id)
            .map_err(|err| err.to_string())?
            .ok_or_else(|| "écho déjà réconcilié".to_string())?;
        let policy = if show_images {
            mail_render::ImagePolicy::AllowRemote
        } else {
            mail_render::ImagePolicy::BlockRemote
        };
        let sanitized = mail_render::sanitize_with(&html, policy);
        // R3 : dalle claire toujours (voir `message_body`) — même porte S1.
        Ok(BodyView {
            document: mail_render::email_document(
                &sanitized.html,
                policy,
                &mail_render::Palette::default(),
            ),
            remote_images_blocked: sanitized.remote_images_blocked,
            attachment_count,
            // Un écho est NOTRE envoi : jamais d'invitation reçue.
            invitation: None,
        })
    })
    .await
}

/// Les pièces d'un écho d'envoi, en métadonnées seules (PLAN-RETOURS-5,
/// D2) : nom, mime, taille depuis le journal d'envoi — les octets sont
/// purgés à `sent`, les puces sont inertes pendant la fenêtre de
/// réconciliation. Un écho de geste rend une liste vide.
#[tauri::command]
pub async fn echo_attachments(app: AppHandle, id: i64) -> Result<Vec<AttachmentRow>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let found = store.echo_attachments(id).map_err(|err| err.to_string())?;
        Ok(found
            .into_iter()
            .enumerate()
            .map(|(index, piece)| AttachmentRow {
                index,
                size: mail_core::human_size(piece.size),
                name: piece.name,
                mime: piece.mime,
            })
            .collect())
    })
    .await
}

/// Marque lu/non-lu : application locale immédiate (optimisme UI) +
/// journalisation — la prochaine synchro du compte rejoue vers le serveur.
#[tauri::command]
pub async fn mark_seen(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    seen: bool,
) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let Some(state) = store
            .sync_state(account_id, &mailbox)
            .map_err(|err| err.to_string())?
        else {
            return Ok(());
        };
        let changed = store
            .set_seen_local(state.mailbox_id, uid, seen)
            .map_err(|err| err.to_string())?;
        if changed {
            let action = if seen {
                Action::MarkSeen
            } else {
                Action::MarkUnseen
            };
            store
                .enqueue_action(state.mailbox_id, uid, action)
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    })
    .await
}

/// Étoile/désétoile : même contrat que lu/non-lu, même file rejouable.
#[tauri::command]
pub async fn mark_flagged(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    flagged: bool,
) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let Some(state) = store
            .sync_state(account_id, &mailbox)
            .map_err(|err| err.to_string())?
        else {
            return Ok(());
        };
        let changed = store
            .set_flagged_local(state.mailbox_id, uid, flagged)
            .map_err(|err| err.to_string())?;
        if changed {
            let action = if flagged {
                Action::MarkFlagged
            } else {
                Action::MarkUnflagged
            };
            store
                .enqueue_action(state.mailbox_id, uid, action)
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    })
    .await
}

/// R4 (PLAN-RETOURS-7) : épingle ou désépingle la conversation du
/// message — donnée LOCALE (IMAP n'a pas ce concept ; `\Flagged` est
/// l'étoile, une autre sémantique). Rend le nouvel état.
#[tauri::command]
pub async fn toggle_pin(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<bool, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let Some(state) = store
            .sync_state(account_id, &mailbox)
            .map_err(|err| err.to_string())?
        else {
            return Ok(false);
        };
        store
            .toggle_pin(state.mailbox_id, uid, epoch_maintenant())
            .map_err(|err| err.to_string())
    })
    .await
}

fn epoch_maintenant() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// La borne d'epoch des pompes de CORPS pour un compte (ADR 0029,
/// PLAN-HORIZON-NETTOYAGE D1) : l'horizon d'import lu de la pref,
/// dérivé à la LECTURE — la borne suit l'horloge. Les enveloppes et les
/// en-têtes de fil restent intégraux, seuls les corps sont bornés.
/// Best effort : une lecture qui échoue ne borne rien — jamais une
/// perte silencieuse sur une erreur.
fn horizon_corps(store: &Store, account_id: i64) -> i64 {
    match store.horizon_import(account_id) {
        Ok(valeur) => mail_core::horizon_epoch(&valeur, epoch_maintenant()),
        Err(err) => {
            // §9 : l'échec se DIT (trace lisible via lancer-wind.ps1),
            // même quand le repli est sûr.
            crate::trace::trace(&format!(
                "horizon_import illisible (compte {account_id}) : {err} ; import intégral par prudence"
            ));
            mail_core::NO_HORIZON
        }
    }
}

/// R1 (PLAN-RETOURS-11, D1-D2) : mémorise « Afficher les images » pour
/// CE message — clé d'enveloppe, la garde ne redemandera plus.
#[tauri::command]
pub async fn allow_images_message(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        // Boîte inconnue = échec DIT, jamais un succès de façade : l'UI
        // afficherait « mémorisé » alors que rien n'est écrit (revue
        // 2026-08-28).
        let Some(state) = store
            .sync_state(account_id, &mailbox)
            .map_err(|err| err.to_string())?
        else {
            return Err(format!("boîte inconnue : {mailbox}"));
        };
        store
            .allow_images_message(state.mailbox_id, uid, epoch_maintenant())
            .map_err(|err| err.to_string())
    })
    .await
}

/// D3 : « Toujours afficher les images de cet expéditeur » — l'adresse
/// est résolue de l'ENVELOPPE côté cœur (l'UI ne parse jamais une
/// adresse), normalisée, globale au poste. Rend l'adresse posée (None :
/// enveloppe sans adresse, rien n'est écrit).
#[tauri::command]
pub async fn allow_images_sender(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<Option<String>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        // Même contrat : l'échec se dit. Le `None` restant (enveloppe
        // sans adresse — rien n'est écrit) est un vrai cas métier que
        // l'UI doit distinguer.
        let Some(state) = store
            .sync_state(account_id, &mailbox)
            .map_err(|err| err.to_string())?
        else {
            return Err(format!("boîte inconnue : {mailbox}"));
        };
        store
            .allow_images_sender_of(state.mailbox_id, uid, epoch_maintenant())
            .map_err(|err| err.to_string())
    })
    .await
}

/// D4 : les règles d'expéditeur, pour la liste des Réglages.
#[tauri::command]
pub async fn images_senders(app: AppHandle) -> Result<Vec<String>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store.images_senders().map_err(|err| err.to_string())
    })
    .await
}

/// D4 : retire une règle d'expéditeur — la porte de sortie du
/// « toujours ».
#[tauri::command]
pub async fn revoke_images_sender(app: AppHandle, address: String) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .revoke_images_sender(&address)
            .map_err(|err| err.to_string())
    })
    .await
}

/// PLAN-MODE-ORGANISE E1 — l'état du mode organisé (D2 amendée :
/// `prefs` SQLite, le cœur lit l'état) et sa borne de rétention
/// (l'époque de première activation, D3 « arrivées seules »).
#[tauri::command]
pub async fn mode_organise_get(app: AppHandle) -> Result<bool, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store.mode_organise().map_err(|err| err.to_string())
    })
    .await
}

/// Bascule le mode organisé. La borne de première activation s'écrit
/// côté cœur, dans le même geste — l'UI ne porte jamais l'époque.
#[tauri::command]
pub async fn mode_organise_set(app: AppHandle, actif: bool) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let mut store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .set_mode_organise(actif, epoch_maintenant())
            .map_err(|err| err.to_string())
    })
    .await
}

/// L'horizon d'import d'un compte (ADR 0029, D3 : réglable après coup).
/// Absent = « tout » — le défaut sûr, côté cœur.
#[tauri::command]
pub async fn horizon_import_get(app: AppHandle, account_id: i64) -> Result<String, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .horizon_import(account_id)
            .map_err(|err| err.to_string())
    })
    .await
}

/// Pose l'horizon d'import (vocabulaire fermé, refusé côté cœur).
/// Étendre rend des corps éligibles — la pompe les rattrape à sa
/// prochaine passe ; réduire n'efface RIEN de ce qui est déjà en local.
#[tauri::command]
pub async fn horizon_import_set(
    app: AppHandle,
    account_id: i64,
    valeur: String,
) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .set_horizon_import(account_id, &valeur)
            .map_err(|err| err.to_string())
    })
    .await
}

/// RETOURS-13 R5/R9 — les actions par défaut des boutons Oui/Non du
/// Portier (livrées : Réception / Corbeille), réglables aux Réglages.
#[derive(serde::Serialize)]
pub struct PortierDefauts {
    pub oui: String,
    pub non: String,
}

#[tauri::command]
pub async fn portier_defauts_get(app: AppHandle) -> Result<PortierDefauts, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let (oui, non) = store.portier_defauts().map_err(|err| err.to_string())?;
        Ok(PortierDefauts { oui, non })
    })
    .await
}

/// Le vocabulaire est fermé et refusé côté cœur — l'UI ne peut pas
/// écrire un défaut troué.
#[tauri::command]
pub async fn portier_defauts_set(app: AppHandle, oui: String, non: String) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let mut store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .set_portier_defauts(&oui, &non)
            .map_err(|err| err.to_string())
    })
    .await
}

/// Une ligne de l'historique du Portier, telle que l'UI la montre.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutagePayload {
    pub address: String,
    pub destination: String,
    pub regle: Option<String>,
    pub epoch: i64,
}

/// Le verdict du Portier sur un expéditeur (Oui nu/orienté, Non
/// nu/avec règle, « Déplacer vers… ») — vocabulaire fermé, refusé côté
/// cœur avant toute écriture.
#[tauri::command]
pub async fn router_expediteur(
    app: AppHandle,
    address: String,
    destination: String,
    regle: Option<String>,
) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .router_expediteur(&address, &destination, regle.as_deref(), epoch_maintenant())
            .map_err(|err| err.to_string())
    })
    .await
}

/// « Déplacer vers… » (E1) : le verdict est posé DEPUIS un message —
/// l'adresse est résolue de l'enveloppe côté cœur, l'UI ne parse
/// jamais une adresse (patron `allow_images_sender`). Rend l'adresse
/// routée ; None = enveloppe sans adresse, rien n'est écrit — un vrai
/// cas métier que l'UI doit dire.
#[tauri::command]
pub async fn router_expediteur_de(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    destination: String,
    regle: Option<String>,
) -> Result<Option<String>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let Some(state) = store
            .sync_state(account_id, &mailbox)
            .map_err(|err| err.to_string())?
        else {
            return Err(format!("boîte inconnue : {mailbox}"));
        };
        store
            .router_expediteur_of(
                state.mailbox_id,
                uid,
                &destination,
                regle.as_deref(),
                epoch_maintenant(),
            )
            .map_err(|err| err.to_string())
    })
    .await
}

/// « Réintégrer » à l'historique du Portier : le verdict disparaît.
#[tauri::command]
pub async fn retirer_routage(app: AppHandle, address: String) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .retirer_routage(&address)
            .map_err(|err| err.to_string())
    })
    .await
}

/// L'historique du Portier — toutes les décisions, la plus récente en
/// tête.
#[tauri::command]
pub async fn routages(app: AppHandle) -> Result<Vec<RoutagePayload>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        Ok(store
            .routages()
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|r| RoutagePayload {
                address: r.address,
                destination: r.destination,
                regle: r.regle,
                epoch: r.epoch,
            })
            .collect())
    })
    .await
}

/// Un rang du guichet du Portier (E2) : l'adresse en attente — LA clé
/// que le verdict prendra — et son dernier message au format des
/// rangées de la liste.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortierRow {
    pub address: String,
    pub row: MessageRow,
}

/// Le guichet du Portier : un rang par expéditeur en attente, le plus
/// récent en tête. Vide tant que le mode n'a jamais été activé.
#[tauri::command]
pub async fn portier_attente(app: AppHandle) -> Result<Vec<PortierRow>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        Ok(store
            .portier_attente()
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|rang| PortierRow {
                address: rang.address,
                row: to_message_row(rang.ligne),
            })
            .collect())
    })
    .await
}

/// La pastille du Portier : combien de MESSAGES attendent au guichet
/// (le dessin du prototype — nav et rechargements légers).
#[tauri::command]
pub async fn portier_total(app: AppHandle) -> Result<u64, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store.portier_total().map_err(|err| err.to_string())
    })
    .await
}

/// RETOURS-14 R4 (revue) — les adresses en attente au guichet, nues :
/// le badge du fil compare des identités, il ne peint pas de rangées.
#[tauri::command]
pub async fn portier_adresses(app: AppHandle) -> Result<Vec<String>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store.portier_adresses().map_err(|err| err.to_string())
    })
    .await
}

/// RETOURS-14 R7 (D8) — la pastille du Kiosque : combien de cartes
/// n'ont JAMAIS été ouvertes (mémoire `kiosque_lus`, la sémantique de
/// la page — jamais l'`unseen` IMAP). Globale, comme `portier_total`.
#[tauri::command]
pub async fn kiosque_non_ouverts(app: AppHandle) -> Result<u64, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .kiosque_non_ouverts(None)
            .map_err(|err| err.to_string())
    })
    .await
}

// ---------------------------------------------------------------------
// Le Nettoyage de printemps (PLAN-HORIZON-NETTOYAGE volet B) — la
// session, les groupes, le verdict de groupe. Vocabulaires fermés,
// refusés côté cœur.
// ---------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNettoyagePayload {
    pub plage: String,
    pub perimetre: String,
    pub total: u64,
    pub traites: u64,
}

impl From<mail_core::SessionNettoyage> for SessionNettoyagePayload {
    fn from(s: mail_core::SessionNettoyage) -> Self {
        SessionNettoyagePayload {
            plage: s.plage,
            perimetre: s.perimetre,
            total: s.total,
            traites: s.traites,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupeNettoyagePayload {
    pub address: String,
    pub qui: Option<String>,
    pub messages: u64,
    pub dernier_epoch: i64,
    pub dernier_objet: Option<String>,
}

impl From<mail_core::GroupeNettoyage> for GroupeNettoyagePayload {
    fn from(g: mail_core::GroupeNettoyage) -> Self {
        GroupeNettoyagePayload {
            address: g.address,
            qui: g.qui,
            messages: g.messages,
            dernier_epoch: g.dernier_epoch,
            dernier_objet: g.dernier_objet,
        }
    }
}

/// RETOURS-14 R6 (D7) — un groupe du Registre : l'expéditeur, ses
/// fils, la récence et l'objet du dernier message.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupeRegistrePayload {
    pub address: String,
    pub qui: Option<String>,
    pub fils: u64,
    pub dernier_epoch: i64,
    pub dernier_objet: Option<String>,
}

impl From<mail_core::GroupeRegistre> for GroupeRegistrePayload {
    fn from(g: mail_core::GroupeRegistre) -> Self {
        GroupeRegistrePayload {
            address: g.address,
            qui: g.qui,
            fils: g.fils,
            dernier_epoch: g.dernier_epoch,
            dernier_objet: g.dernier_objet,
        }
    }
}

/// Les groupes du Registre — un expéditeur × ses fils, récence en
/// tête (D7, patron du Nettoyage).
#[tauri::command]
pub async fn registre_groupes(
    app: AppHandle,
    account_id: Option<i64>,
) -> Result<Vec<GroupeRegistrePayload>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        Ok(store
            .registre_groupes(account_id)
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(Into::into)
            .collect())
    })
    .await
}

/// La page d'un groupe du Registre — les fils de CE seul expéditeur,
/// enrichis comme toute page de liste (invitations).
#[tauri::command]
pub async fn registre_groupe_page(
    app: AppHandle,
    address: String,
    account_id: Option<i64>,
    offset: usize,
    limit: usize,
) -> Result<Vec<MessageRow>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let limit = limit.min(LIST_LIMIT_MAX);
        let mut lignes = store
            .registre_groupe_scoped(&address, account_id, offset, limit)
            .map_err(|err| err.to_string())?;
        store
            .enrichir_lignes(&mut lignes)
            .map_err(|err| err.to_string())?;
        Ok(lignes.into_iter().map(to_message_row).collect())
    })
    .await
}

/// La session en cours — `null` : rien d'entamé (l'écran d'intro).
#[tauri::command]
pub async fn nettoyage_etat(app: AppHandle) -> Result<Option<SessionNettoyagePayload>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        Ok(store
            .nettoyage_etat()
            .map_err(|err| err.to_string())?
            .map(Into::into))
    })
    .await
}

/// Démarre (ou remplace) la session : la borne se fige ici.
#[tauri::command]
pub async fn nettoyage_demarrer(
    app: AppHandle,
    plage: String,
    perimetre: String,
) -> Result<SessionNettoyagePayload, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .nettoyage_demarrer(&plage, &perimetre, epoch_maintenant())
            .map(Into::into)
            .map_err(|err| err.to_string())
    })
    .await
}

/// Les groupes restants (expéditeur × courrier de la plage), le plus
/// récent en tête.
#[tauri::command]
pub async fn nettoyage_groupes(app: AppHandle) -> Result<Vec<GroupeNettoyagePayload>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        // La mesure due depuis HORIZON-NETTOYAGE (« coût sur une vraie
        // base 200 k »), lisible après coup dans `wind.log` — décompte et
        // durée, jamais une adresse (§6.8).
        let depart = std::time::Instant::now();
        let groupes = store.nettoyage_groupes().map_err(|err| err.to_string())?;
        crate::trace::trace(&format!(
            "nettoyage : {} groupes en {} ms",
            groupes.len(),
            depart.elapsed().as_millis()
        ));
        Ok(groupes.into_iter().map(Into::into).collect())
    })
    .await
}

/// Le courrier d'un groupe — VOIR, jamais trier au message.
#[tauri::command]
pub async fn nettoyage_messages(
    app: AppHandle,
    address: String,
) -> Result<Vec<MessageRow>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        Ok(store
            .nettoyage_messages(&address)
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(to_message_row)
            .collect())
    })
    .await
}

/// Le verdict de GROUPE (D5 : le stock de la plage ET l'avenir) — rend
/// l'état à jour, la barre de progression suit dans le même
/// aller-retour.
#[tauri::command]
pub async fn nettoyage_verdict(
    app: AppHandle,
    address: String,
    destination: String,
    regle: Option<String>,
) -> Result<Option<SessionNettoyagePayload>, String> {
    hors_pompe(app, move |app| {
        let mut store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .nettoyage_verdict(&address, &destination, regle.as_deref(), epoch_maintenant())
            .map_err(|err| err.to_string())?;
        Ok(store
            .nettoyage_etat()
            .map_err(|err| err.to_string())?
            .map(Into::into))
    })
    .await
}

/// Clôt la session — les verdicts restent posés (routage).
#[tauri::command]
pub async fn nettoyage_terminer(app: AppHandle) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store.nettoyage_terminer().map_err(|err| err.to_string())
    })
    .await
}

/// E5 — la bascule « Mettre de côté / Reprendre » : l'état vaut pour
/// le FIL (patron de l'épingle), rendu APRÈS le geste.
#[tauri::command]
pub async fn toggle_mis_de_cote(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<bool, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let Some(state) = store
            .sync_state(account_id, &mailbox)
            .map_err(|err| err.to_string())?
        else {
            return Err(format!("boîte inconnue : {mailbox}"));
        };
        store
            .toggle_mis_de_cote(state.mailbox_id, uid, epoch_maintenant())
            .map_err(|err| err.to_string())
    })
    .await
}

/// La pile (E5) : les têtes des fils mis de côté — l'éventail et le
/// tableau s'en servent tels quels.
#[tauri::command]
pub async fn pile_mis_de_cote(app: AppHandle) -> Result<Vec<MessageRow>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let mut lignes = store.pile_mis_de_cote().map_err(|err| err.to_string())?;
        store
            .enrichir_lignes(&mut lignes)
            .map_err(|err| err.to_string())?;
        Ok(lignes
            .into_iter()
            .map(to_message_row)
            .map(|mut row| {
                row.cote = true;
                row
            })
            .collect())
    })
    .await
}

/// Une carte du Kiosque (E5bis) : la rangée ET son corps assaini —
/// « les lettres arrivent déjà ouvertes », le défilement lit sans
/// cliquer. `document` est le MÊME document auto-CSP que l'écran de
/// lecture (mail_render, iframe sandbox S1) ; None = corps pas encore
/// en cache (la carte montre l'aperçu, le rattrapage normal suivra —
/// D5 : le préchargement est borné à la page SERVIE, jamais un réseau
/// par carte).
#[derive(Serialize)]
pub struct CarteKiosque {
    pub row: MessageRow,
    pub document: Option<String>,
    pub remote_images_blocked: usize,
    /// RETOURS-13 R10 : la carte a déjà été lue jusqu'en bas — la
    /// section « Lus précédemment » s'en sert au SERVICE de la page
    /// (jamais en vol : une carte ne saute pas pendant la lecture).
    pub lu: bool,
}

/// La page du Kiosque en CARTES (E5bis, D5/S3) : les rangées de la vue
/// routée + leurs corps lus du CACHE seul (S3 : 12,2 ms froid la page
/// de 20), assainis par LA porte de la lecture — garde d'images
/// consultée par message (autorité au cœur, R1).
#[tauri::command]
pub async fn kiosque_cartes(
    app: AppHandle,
    account_id: Option<i64>,
    offset: usize,
    limit: usize,
) -> Result<Vec<CarteKiosque>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let limit = limit.min(LIST_LIMIT_MAX);
        let mut lignes = store
            .routage_unified_scoped("kiosque", account_id, false, offset, limit)
            .map_err(|err| err.to_string())?;
        store
            .enrichir_lignes(&mut lignes)
            .map_err(|err| err.to_string())?;
        let mut cartes = Vec::with_capacity(lignes.len());
        // La résolution (compte, boîte) → id, UNE fois par boîte de la
        // page — pas vingt sondes identiques par page de vingt cartes
        // (revue E5bis).
        let mut boites: std::collections::HashMap<(i64, String), Option<i64>> =
            std::collections::HashMap::new();
        for ligne in lignes {
            let row = to_message_row(ligne);
            // La résolution de boîte sert le corps ET la marque « lu »
            // (R10) : hoistée hors du match du corps.
            let cle = (row.account_id, row.mailbox.clone());
            let mailbox_id = match boites.get(&cle) {
                Some(id) => *id,
                None => {
                    let id = store
                        .sync_state(row.account_id, &row.mailbox)
                        .map_err(|err| err.to_string())?
                        .map(|s| s.mailbox_id);
                    boites.insert(cle, id);
                    id
                }
            };
            // R10 : le « lu » de la carte — sonde PK, une par carte.
            let lu = mailbox_id
                .map(|id| store.kiosque_lu(id, row.uid))
                .transpose()
                .map_err(|err| err.to_string())?
                .unwrap_or(false);
            // Cache SEUL — un Kiosque hors ligne se lit tel quel.
            let corps = store
                .body(row.account_id, &row.mailbox, row.uid)
                .map_err(|err| err.to_string())?;
            let (document, remote_images_blocked) = match corps {
                Some(html) => {
                    let accordees = mailbox_id
                        .map(|id| store.images_allowed(id, row.uid))
                        .transpose()
                        .map_err(|err| err.to_string())?
                        .unwrap_or(false);
                    let policy = if accordees {
                        mail_render::ImagePolicy::AllowRemote
                    } else {
                        mail_render::ImagePolicy::BlockRemote
                    };
                    let sanitized = mail_render::sanitize_with(&html, policy);
                    (
                        Some(mail_render::email_document(
                            &sanitized.html,
                            policy,
                            &mail_render::Palette::default(),
                        )),
                        sanitized.remote_images_blocked,
                    )
                }
                None => (None, 0),
            };
            cartes.push(CarteKiosque {
                row,
                document,
                remote_images_blocked,
                lu,
            });
        }
        Ok(cartes)
    })
    .await
}

/// RETOURS-13 R10 — une carte du Kiosque défilée jusqu'en bas se
/// marque lue (idempotent ; patron d'adressage de `toggle_mis_de_cote`).
#[tauri::command]
pub async fn kiosque_marquer_lu(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let Some(state) = store
            .sync_state(account_id, &mailbox)
            .map_err(|err| err.to_string())?
        else {
            return Err(format!("boîte inconnue : {mailbox}"));
        };
        store
            .marquer_kiosque_lu(state.mailbox_id, uid, epoch_maintenant())
            .map_err(|err| err.to_string())
    })
    .await
}

/// Les conversations épinglées de la Réception (D4 : Réception seule),
/// servies À PART — le front les prépose à la page 0, le flot paginé
/// les exclut (D5).
#[tauri::command]
pub async fn pinned_rows(
    app: AppHandle,
    account_id: Option<i64>,
    non_lus: bool,
) -> Result<Vec<MessageRow>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        // E2 : la section préposée suit l'exclusion partagée de la
        // Réception organisée — un épinglé routé vit dans sa vue.
        let organise = store.mode_organise().map_err(|err| err.to_string())?;
        let mut lignes = store
            .pinned_unified_scoped(account_id, non_lus, organise)
            .map_err(|err| err.to_string())?;
        store
            .enrichir_lignes(&mut lignes)
            .map_err(|err| err.to_string())?;
        Ok(lignes
            .into_iter()
            .map(to_message_row)
            .map(|mut row| {
                row.pinned = true;
                row
            })
            .collect())
    })
    .await
}

// ---------------------------------------------------------------------
// Composer, répondre, envoyer — la boîte d'envoi (Phases 2-3).
// ---------------------------------------------------------------------

#[derive(Serialize)]
pub struct ComposeContext {
    pub account_id: i64,
    /// La boîte du message auquel on répond. Elle repart avec l'envoi :
    /// sans elle, l'UID ne suffit plus à retrouver le `Message-ID` à citer.
    pub mailbox: String,
    pub uid: u32,
    /// Vide pour un transfert : l'utilisateur choisit le destinataire.
    pub to: String,
    /// Cc pré-rempli — « Répondre à tous » y remet les Cc d'origine (D3) ;
    /// vide pour une réponse simple ou un transfert.
    pub cc: String,
    pub subject: String,
    /// Citation pré-remplie, RICHE (PLAN-COMPOSITION-HTML) : attribution
    /// puis blockquote du corps assaini `BlockRemote` (rien de ce qui
    /// est reposé dans l'éditeur ne charge le réseau — §6.4) ; vide si
    /// le corps est inaccessible (on répond sans citation).
    /// L'utilisateur écrit au-dessus (top-posting) ; le repli text/plain
    /// de l'envoi est dérivé du même HTML par `frontiere_corps`.
    pub body_html: String,
    /// `true` : l'envoi portera In-Reply-To (réponse dans le fil).
    pub reply: bool,
}

#[derive(Serialize)]
pub struct OutboxSummary {
    pub sent: usize,
    pub deferred: usize,
    pub rejected: usize,
    pub quarantined: usize,
    /// Restant en file après la vidange (tous comptes).
    pub queued: usize,
    /// Connexion SMTP impossible (hors ligne, token…) — la file attend.
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct OutboxEntry {
    pub id: i64,
    pub subject: String,
    pub to: String,
    pub state: String,
    pub attempts: u32,
    pub error: Option<String>,
    /// Combien de pièces le journal porte pour cet envoi (PJ-D2) — la
    /// quarantaine et le refus doivent pouvoir dire ce qui repartirait.
    pub pieces: usize,
    /// R2 : l'échéance d'un envoi programmé (secondes epoch) — `None`
    /// pour un envoi ordinaire. L'UI en dérive « programmé pour {h} »
    /// et le geste d'annulation.
    pub send_at_epoch: Option<i64>,
}

#[derive(Serialize)]
pub struct OutboxStatus {
    pub queued: usize,
    pub interrupted: usize,
    pub rejected: usize,
    /// R2 : les envois programmés PAS ENCORE échus — séparés de
    /// `queued`, sans quoi la barre d'état dirait « en attente » d'un
    /// envoi qui attend son heure, pas le réseau (mensonge).
    pub scheduled: usize,
    /// La plus proche échéance parmi les programmés — la sonde du front
    /// déclenche la vidange quand elle passe.
    pub next_scheduled_epoch: Option<i64>,
    /// Tout sauf les envois aboutis, dans l'ordre d'émission.
    pub entries: Vec<OutboxEntry>,
    /// PLAN-AUDIT-V1 E3 (D2) : actions du journal en QUARANTAINE (refus
    /// du serveur, ou cinq échecs) — tous comptes. La fente le dit ;
    /// l'intention n'est plus perdue en silence.
    pub actions_refusees: u64,
}

/// Pré-remplissage d'une réponse : destinataire = adresse brute de
/// l'expéditeur, sujet « Re: » une seule fois, corps cité. La citation
/// est un confort : corps inaccessible = on répond sans elle.
#[tauri::command]
pub async fn reply_context(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<ComposeContext, String> {
    let (envelope, own) = enveloppe_et_compte(&app, account_id, &mailbox, uid).await?;
    let repondre_a = reply_to_de(&app, account_id, &mailbox, uid).await?;
    // Notre propre message ? (l'expéditeur est le compte). Répondre à
    // l'expéditeur nous écrirait à nous-mêmes.
    let is_own = envelope
        .sender_address
        .as_deref()
        .map(|adresse| adresse.trim().eq_ignore_ascii_case(own.trim()))
        .unwrap_or(false);
    // R4 (constat terrain) : sur son propre message, répondre vise les
    // destinataires d'origine (le À) ; sinon, l'expéditeur. Décision pure.
    let mut destinataires = mail_core::reply_to(
        is_own,
        envelope.sender_address.as_deref(),
        &envelope.to_addrs,
        repondre_a.as_deref(),
    );
    // Propre envoi sans destinataires en base (ancien, non rattrapé) :
    // relève serveur UNE fois — même repli que « répondre à tous », jamais
    // un « À » vide sur son propre message.
    if destinataires.is_empty() && is_own {
        let session = hors_pompe(app.clone(), move |app| auth_for(&app, account_id)).await?;
        let boite = mailbox.clone();
        let recipients = tauri::async_runtime::spawn_blocking(move || {
            fetch_recipients_remote(&session, &boite, uid)
        })
        .await
        .map_err(|err| err.to_string())??;
        destinataires = recipients.to;
    }
    if destinataires.is_empty() {
        return Err("destinataire inconnu : resynchronisez la boîte".to_string());
    }
    let to = destinataires.join(", ");
    let body_html = citation_reply(&app, account_id, &mailbox, uid, &envelope).await;
    Ok(ComposeContext {
        account_id,
        mailbox,
        uid,
        to,
        cc: String::new(),
        subject: mail_core::reply_subject(envelope.subject.as_deref()),
        body_html,
        reply: true,
    })
}

/// L'enveloppe d'un message et l'adresse de son compte, en UNE passe
/// sous `hors_pompe` (E5) — la matière commune des trois contextes de
/// composition.
/// Le `Reply-To` du message, lu à la demande (PLAN-AUDIT-V2 E5).
async fn reply_to_de(
    app: &AppHandle,
    account_id: i64,
    mailbox: &str,
    uid: u32,
) -> Result<Option<String>, String> {
    let boite = mailbox.to_string();
    hors_pompe(app.clone(), move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .reply_to_de(account_id, &boite, uid)
            .map_err(|err| err.to_string())
    })
    .await
}

async fn enveloppe_et_compte(
    app: &AppHandle,
    account_id: i64,
    mailbox: &str,
    uid: u32,
) -> Result<(mail_core::Envelope, String), String> {
    let boite = mailbox.to_string();
    hors_pompe(app.clone(), move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let envelope = store
            .envelope(account_id, &boite, uid)
            .map_err(|err| err.to_string())?
            .ok_or_else(|| "message introuvable".to_string())?;
        let own = account_email(&store, account_id)?;
        Ok((envelope, own))
    })
    .await
}

/// La citation riche d'une réponse — un corps inaccessible rend une
/// citation vide (on répond sans elle). Le corps cité est assaini
/// `BlockRemote` : cette chaîne sera REPOSÉE dans l'éditeur
/// (`innerHTML`, document principal) — une image distante y chargerait
/// le pixel espion du message au simple clic « Répondre » (§6.4).
async fn citation_reply(
    app: &AppHandle,
    account_id: i64,
    mailbox: &str,
    uid: u32,
    envelope: &mail_core::Envelope,
) -> String {
    let Ok(html) = raw_body(app, account_id, mailbox, uid).await else {
        return String::new();
    };
    // L'assainissement est du CPU (un corps de 28 Mo, D-1) : sous le
    // verrou des commandes, pas sur un worker async (E5).
    let expediteur = envelope.sender.clone();
    let date = quote_date(envelope);
    hors_pompe(app.clone(), move |_| {
        Ok(mail_core::quote_reply_html(
            expediteur.as_deref(),
            date.as_deref(),
            &mail_render::sanitize(&html).html,
        ))
    })
    .await
    .unwrap_or_default()
}

/// Pré-remplissage d'un « Répondre à tous » : expéditeur + À + Cc du
/// message d'origine, sans doublon ni sa propre adresse.
///
/// Les destinataires À/Cc sont désormais STOCKÉS dans l'enveloppe (R4,
/// depuis la même ENVELOPE que l'expéditeur) : on les lit d'abord —
/// instantané, hors ligne compris (R1, PLAN-RETOURS-MAIL). Le terrain a
/// montré la cause du « À » vide pendant >10 s : l'ancien chemin ouvrait
/// une connexion IMAP authentifiée À CHAQUE clic. On n'y retombe que
/// lorsque le message n'a pas encore ses destinataires en base (envoi
/// ancien non rattrapé) ; là, l'échec reste FRANC — un « à tous » amputé
/// enverrait à moins de monde que promis. La citation reste un confort :
/// corps inaccessible = on répond sans elle.
#[tauri::command]
pub async fn reply_all_context(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<ComposeContext, String> {
    let (envelope, own) = enveloppe_et_compte(&app, account_id, &mailbox, uid).await?;
    let repondre_a = reply_to_de(&app, account_id, &mailbox, uid).await?;
    // Destinataires connus en base : chemin instantané, aucun réseau. Non
    // vide = « lu » (un reçu porte toujours au moins soi en À) ; vide =
    // pas encore rattrapé, on relit le serveur une fois.
    let (to_list, cc_list) = if !envelope.to_addrs.is_empty() || !envelope.cc_addrs.is_empty() {
        (envelope.to_addrs.clone(), envelope.cc_addrs.clone())
    } else {
        let session = hors_pompe(app.clone(), move |app| auth_for(&app, account_id)).await?;
        let boite = mailbox.clone();
        let recipients = tauri::async_runtime::spawn_blocking(move || {
            fetch_recipients_remote(&session, &boite, uid)
        })
        .await
        .map_err(|err| err.to_string())??;
        (recipients.to, recipients.cc)
    };
    // D3 : À et Cc SÉPARÉS — les Cc d'origine restent des Cc (au lieu
    // d'être aplatis dans le À).
    let (mut to, cc) =
        mail_core::reply_all_split(envelope.sender_address.as_deref(), &to_list, &cc_list, &own);
    if to.is_empty() {
        // Message qu'on s'est envoyé à soi seul : l'expéditeur reste le
        // seul destinataire sensé — mieux qu'un champ « À » vide.
        // `Reply-To` prime sur l'expéditeur (PLAN-AUDIT-V2 E5).
        to.extend(repondre_a.or_else(|| envelope.sender_address.clone()));
    }
    if to.is_empty() {
        return Err("adresse de l'expéditeur inconnue : resynchronisez la boîte".to_string());
    }
    let body_html = citation_reply(&app, account_id, &mailbox, uid, &envelope).await;
    Ok(ComposeContext {
        account_id,
        mailbox,
        uid,
        to: to.join(", "),
        cc: cc.join(", "),
        subject: mail_core::reply_subject(envelope.subject.as_deref()),
        body_html,
        reply: true,
    })
}

fn fetch_recipients_remote(
    session: &AccountSession,
    mailbox: &str,
    uid: u32,
) -> Result<mail_core::MessageRecipients, String> {
    let (mut server, _refreshed) = connect_imap(session)?;
    let recipients = server
        .fetch_recipients(mailbox, uid)
        .map_err(|err| err.to_string());
    server.logout();
    recipients?.ok_or_else(|| "message introuvable sur le serveur".to_string())
}

/// Pré-remplissage d'un transfert : sans corps, un transfert ne
/// transmettrait rien — ici l'échec est bloquant. Nouveau fil : pas
/// d'In-Reply-To. Les pièces jointes ne suivent pas encore (Phase 3).
#[tauri::command]
pub async fn forward_context(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<ComposeContext, String> {
    let (envelope, _own) = enveloppe_et_compte(&app, account_id, &mailbox, uid).await?;
    let html = raw_body(&app, account_id, &mailbox, uid).await?;
    // Verdict terrain D5 (2026-08-20) : un transfert TRANSMET — les
    // images distantes sont CONSERVÉES (`AllowRemote`), le destinataire
    // reçoit le message entier. L'exception §6.4 est assumée et
    // consignée : composer le transfert charge ces images dans
    // l'éditeur, comme un « afficher les images » implicite — c'est le
    // geste de transférer qui le dit. La RÉPONSE, elle, reste au pixel
    // neutre (`citation_reply`). L'assainissement sous `hors_pompe` (E5).
    hors_pompe(app, move |_| {
        Ok(ComposeContext {
            account_id,
            mailbox,
            uid,
            to: String::new(),
            cc: String::new(),
            subject: mail_core::forward_subject(envelope.subject.as_deref()),
            body_html: mail_core::quote_forward_html(
                envelope.sender.as_deref(),
                quote_date(&envelope).as_deref(),
                envelope.subject.as_deref(),
                &mail_render::sanitize_with(&html, mail_render::ImagePolicy::AllowRemote).html,
            ),
            reply: false,
        })
    })
    .await
}

/// Date au format de la ligne d'attribution d'une citation.
fn quote_date(envelope: &mail_core::Envelope) -> Option<String> {
    envelope
        .date
        .map(|date| date.format("%Y-%m-%d %H:%M").to_string())
}

/// Journalise l'envoi dans la boîte d'envoi du compte émetteur — AVANT
/// toute tentative réseau (règle « jamais d'envoi perdu »).
#[tauri::command]
// Les arguments d'une commande Tauri sont NOMMÉS à l'appel (objet JS) :
// l'interversion silencieuse que vise le lint ne peut pas s'y produire.
#[allow(clippy::too_many_arguments)]
pub async fn queue_send(
    app: AppHandle,
    account_id: i64,
    to: String,
    cc: String,
    bcc: String,
    subject: String,
    body: String,
    body_html: Option<String>,
    reply_to_mailbox: Option<String>,
    reply_to_uid: Option<u32>,
    draft_id: Option<i64>,
    important: bool,
    // R2 : l'échéance (secondes epoch) d'un envoi différé — None =
    // tout de suite, chemin historique.
    send_at_epoch: Option<i64>,
) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let from = account_email(&store, account_id)?;
        // Corps riche : LA frontière (`frontiere_corps`) — assaini, texte
        // dérivé. Le `body` reçu ne sert qu'au chemin texte.
        let (corps_texte, corps_riche) = frontiere_corps(body, body_html.as_deref());
        // Sans la boîte, on ne résout RIEN — on ne devine pas.
        //
        // Un UID seul ne désigne plus un message depuis que le compte en a
        // deux (ADR 0009) : le n°1 d'INBOX et le n°1 d'« Envoyés » sont deux
        // messages. Deviner produirait un `In-Reply-To` pointant sur un
        // inconnu, donc une réponse greffée sur la conversation de quelqu'un
        // d'autre. L'omettre coupe un fil — « un fil coupé en deux est
        // réparable et honnête ; deux messages étrangers réunis ne le sont
        // pas » (ADR 0008 §2).
        let parent = reply_to_uid.zip(reply_to_mailbox);
        let in_reply_to = parent
            .as_ref()
            .and_then(|(uid, mailbox)| store.envelope(account_id, mailbox, *uid).ok().flatten())
            .and_then(|envelope| envelope.message_id);
        let mut draft = mail_core::compose(
            &from,
            &to,
            &cc,
            &bcc,
            &subject,
            &corps_texte,
            in_reply_to.as_deref(),
        )
        .map_err(|err| err.to_string())?;
        // E7 : la chaîne References entière (RFC 5322 §3.6.4) — le cœur
        // la sait, l'adaptateur la recopie.
        draft.references = parent.as_ref().and_then(|(uid, mailbox)| {
            store
                .references_de(account_id, mailbox, *uid)
                .ok()
                .flatten()
        });
        draft.body_html = corps_riche;
        draft.important = important;
        // Une échéance déjà passée vaut « tout de suite » : la garde vit
        // ici, pas dans l'UI — un datetime resté ouvert pendant que
        // l'heure tournait ne doit pas retenir l'envoi pour rien.
        let echeance = send_at_epoch.filter(|epoch| *epoch > chrono::Utc::now().timestamp());
        // Brouillon-ancre (pièces dans la MÊME transaction, PJ-D2) et
        // échéance (R2) passent par LE chemin unique de la mise en file.
        store
            .enqueue_outbox_full(account_id, &draft, draft_id, echeance)
            .map_err(|err| err.to_string())?;
        Ok(())
    })
    .await
}

/// La fin d'un cycle (complet ou léger) : l'horodatage de la dernière
/// relève réussie (E1) — posé seulement quand AU MOINS un compte a
/// répondu ; un cycle à vide ne rajeunit pas « dernière synchronisation ».
/// L'échec d'écriture est rapporté, jamais avalé ; il ne fait pas
/// échouer la relève, le courrier est là. Sous `hors_pompe` (E5) : base +
/// verrou des commandes. Le `unified_count()` qui vivait ici nourrissait
/// `SyncSummary.total`, que l'UI n'a jamais lu (PLAN-AUDIT-V2 E1).
async fn solder_releve(
    app: &AppHandle,
    accounts: usize,
    errors: &mut Vec<String>,
) -> Result<(), String> {
    if accounts == 0 {
        return Ok(());
    }
    let horodatage = hors_pompe(app.clone(), move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let mut horodatage = None;
        if let Ok(epoch) = SystemTime::now().duration_since(UNIX_EPOCH)
            && let Err(err) =
                store.set_text_pref(PREF_DERNIERE_SYNCHRO, &epoch.as_secs().to_string())
        {
            horodatage = Some(format!("horodatage de la relève : {err}"));
        }
        Ok(horodatage)
    })
    .await?;
    errors.extend(horodatage);
    Ok(())
}

/// Vide les boîtes d'envoi de TOUS les comptes connectés — chacun par
/// SA connexion SMTP. Hors ligne = bilan, pas une erreur. Réentrance
/// interdite (verrou).
#[tauri::command]
pub async fn flush_outbox(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<OutboxSummary, String> {
    let path = db_path(&app)?;
    let jobs = hors_pompe(app.clone(), |app| connected_jobs(&app)).await?;
    let lock = state.outbox_flush.clone();

    let (summary, refreshed) =
        tauri::async_runtime::spawn_blocking(move || run_flush_all(jobs, &path, &lock))
            .await
            .map_err(|err| err.to_string())??;

    reposer_sessions(&state, refreshed)?;
    Ok(summary)
}

/// Quand retenter la relève ciblée d'Envoyés qui n'a rien rapporté —
/// décision pure (PLAN-REACTIVITE E2). Gmail ajoute la copie de façon
/// ASYNCHRONE après l'acceptation SMTP : la première relève peut passer
/// AVANT elle et répondre « rien n'a bougé », honnêtement. Deux
/// retentatives bornées (+5 s puis +15 s), puis silence — le cycle
/// complet rattrapera ; on ne martèle pas un serveur qui n'a rien à
/// donner (la leçon anti-martèlement du complément P0).
fn retenter_apres(tentative: u32) -> Option<Duration> {
    match tentative {
        1 => Some(Duration::from_secs(5)),
        2 => Some(Duration::from_secs(15)),
        _ => None,
    }
}

/// Au-delà, la relève ne rapatrie plus les corps elle-même : les lignes
/// d'abord, la pompe fera les corps. ~192 ms par message amorti par lot
/// (`spikes/body-backfill`) : dix corps coûtent ~2 s au chemin de la
/// bulle — la borne < 30 s de PLAN-SYNCHRO garde ses marges.
const CORPS_A_L_ARRIVEE_MAX: usize = 10;

/// Combien de corps rapatrier DANS la relève INBOX qui vient d'apporter
/// `arrivees` messages NEUFS (UID au-dessus du repère — jamais le
/// `fetched` du rapport, gonflé des drapeaux d'un delta CONDSTORE) —
/// décision pure (PLAN-REACTIVITE E4, R-D2). Un lot courant : tous ses
/// corps, la ligne naît avec son aperçu. Un lot qui déborde (rattrapage
/// après coupure, intégrale) : zéro — le bump part d'abord, les lignes
/// vite, et les corps échoient à la pompe.
fn corps_a_l_arrivee(arrivees: usize) -> usize {
    if arrivees > CORPS_A_L_ARRIVEE_MAX {
        0
    } else {
        arrivees
    }
}

/// Le bilan de la passe d'après-geste — fini le silence : les incidents
/// remontent à l'UI comme ceux du cycle (terrain 0.1.5 : l'instruction
/// était aveugle, tout partait en `eprintln` et le `.catch(() => {})`
/// avalait le reste). `reconcilies` : des échos remplacés par leur
/// vraie ligne ; `balayes` : des échos que le serveur a démentis.
#[derive(Default, Serialize)]
pub struct PasseReport {
    pub fetched: usize,
    pub deleted: usize,
    pub reconcilies: usize,
    pub balayes: usize,
    pub errors: Vec<String>,
}

/// La passe d'après-geste (PLAN-REACTIVITE E3) — la réconciliation de
/// l'écho local : après une suppression, un archivage, un déplacement
/// ou un envoi, le serveur doit suivre SANS attendre le cycle.
///
/// 1. **Les intentions d'abord** : les boîtes qui portent des actions
///    journalisées se relèvent — le rejeu part MAINTENANT (INBOX par le
///    chemin partagé `relever_inbox` : bulles et compteurs, rien ne se
///    raconte deux fois).
/// 2. **L'inventaire** : LIST-STATUS (un aller-retour, E2c) —
///    `faut_relever` désigne les dossiers qui ont bougé : la destination
///    du geste, et elle seule en pratique. La destination n'est JAMAIS
///    devinée (Corbeille RFC 6154, label Gmail : tout se voit au
///    STATUS). INBOX reste au veilleur et au cycle — la relever ici
///    volerait leurs bulles. Repli sans LIST-STATUS : des STATUS ciblés
///    sur les seules destinations canoniques, jamais les ~50 dossiers.
/// 3. **La réconciliation** : l'écho meurt quand la vraie ligne entre
///    (même `message_id` dans la destination) — la ligne ne bouge pas à
///    l'œil.
/// 4. **La retentative** (E2) : des échos attendent encore → +5 s puis
///    +15 s puis silence (copie Gmail asynchrone). Chaque tentative
///    prend et REND le verrou du compte : les pauses ne bloquent rien.
/// 5. **Le balayage** : intention soldée, destination relevée PROPREMENT
///    et toujours pas de copie → l'écho se retire, l'incident se dit —
///    on n'affiche pas ce que le serveur dément. Jamais après une
///    tentative en échec : un serveur qui n'a pas répondu n'a rien
///    démenti.
///
/// `account_id = None` (retour en ligne, R-D3) : tous les comptes qui
/// ont du travail — actions en attente ou échos. Un vol par compte,
/// coalescé : archiver dix messages n'ouvre pas dix passes.
#[tauri::command]
pub async fn sync_apres_geste(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: Option<i64>,
) -> Result<PasseReport, String> {
    let path = db_path(&app)?;
    let cibles: Vec<i64> = match account_id {
        Some(id) => vec![id],
        None => {
            hors_pompe(app.clone(), |app| {
                let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
                store.comptes_avec_travail().map_err(|err| err.to_string())
            })
            .await?
        }
    };
    let mut rapport = PasseReport::default();
    for compte in cibles {
        let session = match hors_pompe(app.clone(), move |app| auth_for(&app, compte)).await {
            Ok(session) => session,
            Err(reason) => {
                rapport.errors.push(reason);
                continue;
            }
        };
        let email = session.email().to_string();
        // Un vol par compte : une passe en cours ABSORBE la demande — le
        // drapeau la fera rejouer une fois, pas dix. Le vol est une GARDE
        // (E5) : un `?` au milieu de la passe le rendait jadis éternel,
        // et toute passe suivante du compte était absorbée jusqu'au
        // redémarrage.
        let Some(vol) = VolGarde::prendre(&state.passes_geste, &email) else {
            continue;
        };
        loop {
            let issue = {
                let path = path.clone();
                let session = session.clone();
                let cycle = state.sync_cycle.clone();
                let verrous = state.verrous_releve.clone();
                let app_bulles = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    passe_apres_geste_compte(&path, session, compte, &cycle, &verrous, &app_bulles)
                })
                .await
                .map_err(|err| err.to_string())
                .and_then(|issue| issue)
            };
            match issue {
                Ok((bilan, refreshed)) => {
                    rapport.fetched += bilan.fetched;
                    rapport.deleted += bilan.deleted;
                    rapport.reconcilies += bilan.reconcilies;
                    rapport.balayes += bilan.balayes;
                    rapport.errors.extend(bilan.errors);
                    reposer_sessions(&state, refreshed)?;
                }
                Err(reason) => rapport.errors.push(format!("{email} : {reason}")),
            }
            // Rejouer UNE fois si un geste est arrivé pendant la passe.
            if !vol.redemande_consommee() {
                break;
            }
        }
        drop(vol);
    }
    Ok(rapport)
}

/// La garde du vol d'une passe d'après-geste (E5) : `en_vol` retombe
/// quand elle est relâchée — par le `drop` explicite comme par un `?`.
struct VolGarde<'a> {
    passes: &'a Mutex<HashMap<String, VolPasse>>,
    email: String,
}

impl<'a> VolGarde<'a> {
    /// `None` : une passe est déjà en vol pour ce compte — la demande
    /// est absorbée (le drapeau la fera rejouer une fois).
    fn prendre(passes: &'a Mutex<HashMap<String, VolPasse>>, email: &str) -> Option<Self> {
        let mut table = match passes.lock() {
            Ok(table) => table,
            Err(empoisonne) => empoisonne.into_inner(),
        };
        let vol = table.entry(email.to_string()).or_default();
        if vol.en_vol {
            vol.redemande = true;
            return None;
        }
        vol.en_vol = true;
        Some(Self {
            passes,
            email: email.to_string(),
        })
    }

    /// Un geste est-il arrivé pendant la passe ? Consomme le drapeau.
    fn redemande_consommee(&self) -> bool {
        let mut table = match self.passes.lock() {
            Ok(table) => table,
            Err(empoisonne) => empoisonne.into_inner(),
        };
        let vol = table.entry(self.email.clone()).or_default();
        std::mem::take(&mut vol.redemande)
    }
}

impl Drop for VolGarde<'_> {
    fn drop(&mut self) {
        let mut table = match self.passes.lock() {
            Ok(table) => table,
            Err(empoisonne) => empoisonne.into_inner(),
        };
        if let Some(vol) = table.get_mut(&self.email) {
            vol.en_vol = false;
        }
    }
}

/// La passe d'UN compte — le corps bloquant de `sync_apres_geste`.
fn passe_apres_geste_compte(
    path: &Path,
    mut session: AccountSession,
    account_id: i64,
    cycle: &crate::SyncShared,
    verrous: &Mutex<HashMap<String, Arc<Mutex<()>>>>,
    app: &AppHandle,
) -> Result<(PasseReport, Vec<AccountSession>), String> {
    let mut rapport = PasseReport::default();
    let mut sessions = Vec::new();
    let mut tentative = 0u32;
    // Posé par CHAQUE tour de boucle avant toute sortie : pas de valeur
    // de départ — le compilateur garantit qu'on ne lit jamais un vide.
    let mut derniere_propre;
    loop {
        tentative += 1;
        let erreurs_avant = rapport.errors.len();
        // Le courrier de CETTE tentative (hors INBOX, qui publie déjà le
        // sien par `relever_inbox`) — c'est lui qui bump la génération.
        let mut courrier_tentative = 0usize;
        let chrono_total = Instant::now();
        {
            let verrou = verrou_compte(verrous, session.email());
            let _releve = verrou.lock();
            let (mut server, fresh) = connect_imap(&session)?;
            if let Some(fresh) = fresh {
                session = fresh.clone();
                sessions.push(fresh);
            }
            let mut store = Store::open(path).map_err(|err| err.to_string())?;
            // 1. Les intentions : le rejeu part MAINTENANT.
            let chrono = Instant::now();
            let sources = store
                .mailboxes_avec_actions(account_id)
                .map_err(|err| err.to_string())?;
            for boite in &sources {
                if boite == MAILBOX {
                    if let Err(reason) = relever_inbox(
                        &mut server,
                        &mut store,
                        account_id,
                        cycle,
                        app,
                        &mut rapport.errors,
                    ) {
                        rapport.errors.push(format!("INBOX : {reason}"));
                    }
                } else {
                    let statut = server.folder_status(boite).ok();
                    if doit_relever(
                        &store,
                        account_id,
                        boite,
                        statut.as_ref(),
                        &mut rapport.errors,
                    ) {
                        match SyncEngine::default().sync(&mut server, &mut store, account_id, boite)
                        {
                            Ok(report) => {
                                rapport.fetched += report.fetched;
                                rapport.deleted += report.deleted;
                                courrier_tentative += report.fetched + report.deleted;
                                solder_repere(
                                    &store,
                                    account_id,
                                    boite,
                                    statut.as_ref(),
                                    &mut rapport.errors,
                                );
                            }
                            Err(reason) => {
                                rapport.errors.push(format!("dossier source : {reason}"))
                            }
                        }
                    }
                }
            }
            let duree_actions = chrono.elapsed();
            let n_sources = sources.len();
            // 2. L'inventaire : seuls les dossiers qui ont BOUGÉ se
            // relèvent — la destination du geste, sans jamais la deviner.
            let chrono = Instant::now();
            let mut releves = 0usize;
            match server.folders_with_status() {
                Ok(Some(avec_statut)) => {
                    for (folder, statut) in avec_statut {
                        if !folder.selectable
                            || folder.wire == MAILBOX
                            || sources.contains(&folder.wire)
                        {
                            continue;
                        }
                        if relever_dossier_passe(
                            &mut server,
                            &mut store,
                            account_id,
                            &folder.wire,
                            statut.as_ref(),
                            &mut rapport,
                            &mut courrier_tentative,
                        ) {
                            releves += 1;
                        }
                    }
                }
                Ok(None) => {
                    // Sans LIST-STATUS : des STATUS ciblés sur les seules
                    // destinations canoniques — jamais les ~50 dossiers.
                    let dossiers = store
                        .canonical_folders(account_id)
                        .map_err(|err| err.to_string())?;
                    for nom in [dossiers.envoyes, dossiers.archives, dossiers.corbeille]
                        .into_iter()
                        .flatten()
                    {
                        if nom == MAILBOX || sources.contains(&nom) {
                            continue;
                        }
                        let statut = server.folder_status(&nom).ok();
                        if relever_dossier_passe(
                            &mut server,
                            &mut store,
                            account_id,
                            &nom,
                            statut.as_ref(),
                            &mut rapport,
                            &mut courrier_tentative,
                        ) {
                            releves += 1;
                        }
                    }
                }
                Err(reason) => rapport
                    .errors
                    .push(format!("inventaire LIST-STATUS : {reason}")),
            }
            let duree_inventaire = chrono.elapsed();
            // 3. La réconciliation : l'écho meurt quand la vraie ligne
            // entre — la liste ne bouge pas à l'œil.
            let reconcilies = store
                .reconcilier_echos(account_id)
                .map_err(|err| err.to_string())?;
            rapport.reconcilies += reconcilies;
            courrier_tentative += reconcilies;
            server.logout();
            // La trace qui instruira D-7 (§6.8 : durées et décomptes
            // seuls) — à lire contre l'horodatage du geste en console.
            crate::trace::trace(&format!(
                "passe geste compte {account_id} : {n_sources} source(s) {:.1}s · inventaire + {releves} relevé(s) {:.1}s · {reconcilies} réconcilié(s) · total {:.1}s",
                duree_actions.as_secs_f32(),
                duree_inventaire.as_secs_f32(),
                chrono_total.elapsed().as_secs_f32(),
            ));
            if courrier_tentative > 0 {
                cycle
                    .courrier
                    .fetch_add(courrier_tentative as u64, Ordering::Relaxed);
                cycle.generation.fetch_add(1, Ordering::Relaxed);
            }
        }
        derniere_propre = rapport.errors.len() == erreurs_avant;
        let en_attente = Store::open(path)
            .map_err(|err| err.to_string())?
            .echos_en_attente(account_id)
            .map_err(|err| err.to_string())?;
        if en_attente == 0 {
            break;
        }
        match retenter_apres(tentative) {
            Some(delai) => std::thread::sleep(delai),
            None => break,
        }
    }
    // 4. Le balayage — après une tentative PROPRE seulement : une relève
    // en échec n'a rien démenti, l'écho vit (hors ligne, recul…).
    if derniere_propre {
        let store = Store::open(path).map_err(|err| err.to_string())?;
        let incidents = store
            .balayer_echos(account_id)
            .map_err(|err| err.to_string())?;
        if !incidents.is_empty() {
            rapport.balayes += incidents.len();
            rapport.errors.extend(incidents);
            // Des lignes viennent de disparaître : la liste se resert.
            cycle.generation.fetch_add(1, Ordering::Relaxed);
        }
    }
    Ok((rapport, sessions))
}

/// Une relève de dossier de la passe (phase inventaire) : gardée par
/// `faut_relever`, soldée, comptée. Rend vrai si le dossier a été relevé.
#[allow(clippy::too_many_arguments)]
fn relever_dossier_passe(
    server: &mut ImapServer,
    store: &mut Store,
    account_id: i64,
    boite: &str,
    statut: Option<&mail_core::FolderStatus>,
    rapport: &mut PasseReport,
    courrier: &mut usize,
) -> bool {
    if !doit_relever(store, account_id, boite, statut, &mut rapport.errors) {
        return false;
    }
    match SyncEngine::default().sync(server, store, account_id, boite) {
        Ok(report) => {
            rapport.fetched += report.fetched;
            rapport.deleted += report.deleted;
            *courrier += report.fetched + report.deleted;
            solder_repere(store, account_id, boite, statut, &mut rapport.errors);
            true
        }
        Err(reason) => {
            rapport
                .errors
                .push(format!("dossier « {boite} » : {reason}"));
            false
        }
    }
}

fn run_flush_all(
    jobs: Vec<(i64, AccountSession)>,
    db_path: &Path,
    lock: &Mutex<()>,
) -> Result<(OutboxSummary, Vec<AccountSession>), String> {
    // E5 : verrou empoisonné = repris (le panic est consigné, ADR 0014).
    let _guard = match lock.lock() {
        Ok(garde) => garde,
        Err(empoisonne) => empoisonne.into_inner(),
    };
    let mut store = Store::open(db_path).map_err(|err| err.to_string())?;

    // Un crash antérieur se constate même hors ligne : quarantaine d'abord.
    let mut summary = OutboxSummary {
        sent: 0,
        deferred: 0,
        rejected: 0,
        quarantined: store.quarantine_inflight().map_err(|err| err.to_string())?,
        queued: 0,
        error: None,
    };
    let mut refreshed_list = Vec::new();

    for (account_id, session) in jobs {
        if store
            .outbox_pending_count(account_id)
            .map_err(|err| err.to_string())?
            == 0
        {
            continue;
        }
        match connect_smtp(&session) {
            // Hors ligne : la file de ce compte survit telle quelle.
            Err(reason) => summary.error = Some(reason),
            Ok((mut mailer, refreshed)) => {
                let report = mail_core::flush_outbox(&mut mailer, &mut store, account_id)
                    .map_err(|err| err.to_string())?;
                summary.sent += report.sent;
                summary.deferred += report.deferred;
                summary.rejected += report.rejected;
                summary.quarantined += report.quarantined;
                if let Some(fresh) = refreshed {
                    refreshed_list.push(fresh);
                }
            }
        }
    }
    let restants = store
        .outbox_in_state(OutboxState::Queued)
        .map_err(|err| err.to_string())?;
    summary.queued = restants.len();
    // La trace terrain de la vidange (§6.8 — lisible en `2> fichier`,
    // l'app release est sous-système windows) : le bilan, puis la
    // dernière erreur de chaque envoi resté en file — c'est elle que la
    // barre d'état ne montre pas (« en attente » n'est pas fautif) et
    // qu'un constat terrain doit pouvoir lire.
    crate::trace::trace(&format!(
        "vidange : {} parti(s), {} differe(s), {} refuse(s), {} quarantaine, {} en file{}",
        summary.sent,
        summary.deferred,
        summary.rejected,
        summary.quarantined,
        summary.queued,
        summary
            .error
            .as_deref()
            .map(|err| format!(" · connexion : {err}"))
            .unwrap_or_default(),
    ));
    for message in &restants {
        if let Some(err) = &message.last_error {
            // §6.8 : l'identifiant, les tentatives, l'erreur — JAMAIS le
            // sujet (E9 ; avant, le sujet partait dans la trace).
            crate::trace::trace(&format!(
                "vidange : envoi {} attend ({} tentative(s)) : {err}",
                message.id, message.attempts
            ));
        }
    }
    Ok((summary, refreshed_list))
}

/// L'état de la boîte d'envoi pour l'UI : tout ce qui n'est pas parti,
/// tous comptes confondus.
#[tauri::command]
pub async fn outbox_status(app: AppHandle) -> Result<OutboxStatus, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let mut status = OutboxStatus {
            queued: 0,
            interrupted: 0,
            rejected: 0,
            scheduled: 0,
            next_scheduled_epoch: None,
            entries: Vec::new(),
            actions_refusees: store.actions_refusees().map_err(|err| err.to_string())?,
        };
        let maintenant = chrono::Utc::now().timestamp();
        for message in store.outbox_metadonnees().map_err(|err| err.to_string())? {
            // R2 : programmé pas encore échu — il n'attend pas le
            // réseau, il attend son heure. Compté à part, et la plus
            // proche échéance remonte (la sonde déclenchera la vidange).
            let programme = message.state == OutboxState::Queued
                && message
                    .send_at_epoch
                    .is_some_and(|epoch| epoch > maintenant);
            match message.state {
                OutboxState::Sent => continue,
                OutboxState::Queued if programme => {
                    status.scheduled += 1;
                    status.next_scheduled_epoch = match status.next_scheduled_epoch {
                        None => message.send_at_epoch,
                        Some(connu) => Some(connu.min(message.send_at_epoch.unwrap_or(connu))),
                    };
                }
                OutboxState::Queued | OutboxState::Sending => status.queued += 1,
                OutboxState::Interrupted => status.interrupted += 1,
                OutboxState::Rejected => status.rejected += 1,
            }
            status.entries.push(OutboxEntry {
                id: message.id,
                subject: message.subject,
                to: message.to.join(", "),
                state: message.state.as_str().to_string(),
                attempts: message.attempts,
                error: message.last_error,
                pieces: message.attachments.len(),
                send_at_epoch: message.send_at_epoch.filter(|epoch| *epoch > maintenant),
            });
        }
        Ok(status)
    })
    .await
}

/// Renvoi d'un envoi en quarantaine ou refusé : LA décision explicite
/// de l'utilisateur qu'exige la règle « jamais d'envoi fantôme ».
#[tauri::command]
pub async fn outbox_requeue(app: AppHandle, id: i64) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store.requeue_outbox(id).map_err(|err| err.to_string())
    })
    .await
}

/// Abandon d'un envoi (décision utilisateur) ; l'historique `sent`
/// est préservé par le noyau.
#[tauri::command]
pub async fn outbox_delete(app: AppHandle, id: i64) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store.delete_outbox(id).map_err(|err| err.to_string())
    })
    .await
}

/// R2, décision CE D2 : annule un envoi programmé — l'entrée quitte le
/// journal et un brouillon COMPLET renaît (destinataires, corps,
/// marquage, pièces). Rend l'id du brouillon recréé, ou `None` si la
/// vidange l'a pris entre-temps : trop tard, le message part — l'UI le
/// dit honnêtement plutôt que de promettre un brouillon fantôme.
#[tauri::command]
pub async fn outbox_cancel_scheduled(app: AppHandle, id: i64) -> Result<Option<i64>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .annuler_envoi_programme(id)
            .map_err(|err| err.to_string())
    })
    .await
}

// ---------------------------------------------------------------------
// Signature par compte (R1, PLAN-RETOURS-6, décisions CE D3/D4).
// ---------------------------------------------------------------------

/// La signature d'un compte et sa portée — ce que Réglages édite et ce
/// que le composeur insère à l'ouverture.
#[derive(Serialize)]
pub struct SignatureRow {
    /// HTML assaini (allowlist ammonia) — `None` : pas de signature.
    pub html: Option<String>,
    /// D4 : la signature s'insère AUSSI dans réponses et transferts.
    /// Défaut : nouveaux messages seuls.
    pub replies: bool,
}

#[tauri::command]
pub async fn signature_get(app: AppHandle, account_id: i64) -> Result<SignatureRow, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let html = store
            .text_pref(&format!("signature.{account_id}"))
            .map_err(|err| err.to_string())?
            .filter(|h| !h.trim().is_empty());
        let replies = store
            .bool_pref(&format!("signature_replies.{account_id}"), false)
            .map_err(|err| err.to_string())?;
        Ok(SignatureRow { html, replies })
    })
    .await
}

/// Enregistre la signature d'un compte. Le HTML passe LA frontière
/// (`frontiere_corps`, allowlist ammonia) — une signature entre en base
/// comme tout corps : assainie, jamais crue. Un HTML au rendu texte
/// vide vaut « signature effacée ».
#[tauri::command]
pub async fn signature_set(
    app: AppHandle,
    account_id: i64,
    html: Option<String>,
    replies: bool,
) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let propre = html
            .as_deref()
            .and_then(|h| frontiere_corps(String::new(), Some(h)).1);
        store
            .set_text_pref(
                &format!("signature.{account_id}"),
                propre.as_deref().unwrap_or(""),
            )
            .map_err(|err| err.to_string())?;
        store
            .set_bool_pref(&format!("signature_replies.{account_id}"), replies)
            .map_err(|err| err.to_string())?;
        Ok(())
    })
    .await
}

// ---------------------------------------------------------------------
// Repère de compte (PLAN-RETOURS-8 R1) : icône + teinte par compte,
// pour différencier les boîtes en boîte unifiée. Préférence locale
// (table `prefs`, patron signature) — le serveur n'a pas ce concept.
// ---------------------------------------------------------------------

/// Le jeu d'icônes DÉDIÉ aux comptes (D2) : des glyphes neufs du
/// sous-ensemble, réservés — jamais réemployés ailleurs (A3 « une
/// icône, un sens »).
const REPERE_ICONES: [&str; 12] = [
    "home",
    "work",
    "school",
    "star",
    "favorite",
    "flight",
    "shopping_bag",
    "account_balance",
    "sports_esports",
    "eco",
    "pets",
    "music_note",
];

/// Le nuancier mesuré (D1) : 12 familles, dont les DEUX déclinaisons
/// (clairs / -nuit) vivent dans `systeme.css` — ici on ne stocke que le
/// nom de famille, jamais un hex.
const REPERE_TEINTES: [&str; 12] = [
    "rouge", "orange", "ocre", "olive", "vert", "sapin", "bleu", "indigo", "violet", "magenta",
    "rose", "brun",
];

/// La décision pure : un repère n'existe que dans l'allowlist croisée
/// (jeu dédié × nuancier). Tout le reste — glyphe du produit, teinte
/// inconnue, chaîne vide — est refusé, à l'entrée comme au retour.
pub(crate) fn repere_valide(icone: &str, teinte: &str) -> bool {
    REPERE_ICONES.contains(&icone) && REPERE_TEINTES.contains(&teinte)
}

/// Relit le repère d'un compte ; une valeur hors allowlist (base
/// corrompue, ancienne version) ne sort jamais vers l'UI.
pub(crate) fn repere_de(
    store: &Store,
    account_id: i64,
) -> Result<Option<(String, String)>, mail_core::Error> {
    let icone = store.text_pref(&format!("repere_icone.{account_id}"))?;
    let teinte = store.text_pref(&format!("repere_teinte.{account_id}"))?;
    Ok(match (icone, teinte) {
        (Some(i), Some(t)) if repere_valide(&i, &t) => Some((i, t)),
        _ => None,
    })
}

/// Pose ou retire (None) le repère — retirer vide les clés (patron
/// signature : la pref vide vaut « jamais posée »). Les DEUX clés
/// partent dans UNE transaction : une paire à moitié écrite serait un
/// repère que personne n'a choisi (revue 2026-08-22).
pub(crate) fn poser_repere(
    store: &mut Store,
    account_id: i64,
    repere: Option<(&str, &str)>,
) -> Result<(), mail_core::Error> {
    let (icone, teinte) = repere.unwrap_or(("", ""));
    let cle_icone = format!("repere_icone.{account_id}");
    let cle_teinte = format!("repere_teinte.{account_id}");
    store.set_text_prefs(&[(cle_icone.as_str(), icone), (cle_teinte.as_str(), teinte)])
}

#[derive(Serialize)]
pub struct RepereRow {
    pub account_id: i64,
    pub icone: String,
    pub teinte: String,
}

/// Tous les repères posés — l'UI les charge UNE fois (nav + liste) et
/// les recharge au changement. Un compte sans repère n'a pas de ligne :
/// son rendu par défaut (`person`, jeton neutre) ne dépend de rien.
#[tauri::command]
pub async fn reperes_get(app: AppHandle) -> Result<Vec<RepereRow>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let mut rows = Vec::new();
        for compte in store.accounts().map_err(|err| err.to_string())? {
            if let Some((icone, teinte)) =
                repere_de(&store, compte.id).map_err(|err| err.to_string())?
            {
                rows.push(RepereRow {
                    account_id: compte.id,
                    icone,
                    teinte,
                });
            }
        }
        Ok(rows)
    })
    .await
}

/// Pose (icône + teinte) ou retire (les deux à None) le repère d'un
/// compte. Une valeur hors allowlist est une erreur franche — l'UI ne
/// propose que le jeu dédié, tout autre appel est un bug.
#[tauri::command]
pub async fn repere_set(
    app: AppHandle,
    account_id: i64,
    icone: Option<String>,
    teinte: Option<String>,
) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let repere = match (icone.as_deref(), teinte.as_deref()) {
            (None, None) => None,
            (Some(i), Some(t)) if repere_valide(i, t) => Some((i, t)),
            (Some(_), Some(_)) => return Err("repère hors du jeu dédié".to_string()),
            _ => return Err("icône et teinte vont ensemble".to_string()),
        };
        let mut store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        poser_repere(&mut store, account_id, repere).map_err(|err| err.to_string())
    })
    .await
}

/// PLAN-RETOURS-9 (D3) : la décision pure du nom personnalisé d'un
/// compte. Espaces rognés ; vide = retiré (None) ; au-delà de 60
/// caractères refusé — jamais tronqué en silence.
pub(crate) fn nom_normalise(brut: &str) -> Result<Option<String>, String> {
    let net = brut.trim();
    if net.is_empty() {
        return Ok(None);
    }
    if net.chars().count() > 60 {
        return Err("nom trop long (60 caractères au plus)".to_string());
    }
    Ok(Some(net.to_string()))
}

/// Relit le nom d'un compte ; une coquille blanche en base (posée hors
/// UI, ancienne version) ne sort jamais vers l'affichage.
pub(crate) fn nom_de(store: &Store, account_id: i64) -> Result<Option<String>, mail_core::Error> {
    Ok(store
        .text_pref(&format!("nom_compte.{account_id}"))?
        .map(|nom| nom.trim().to_string())
        .filter(|nom| !nom.is_empty()))
}

/// Pose ou retire (None) le nom — retirer vide la clé (patron
/// signature/repère : la pref vide vaut « jamais posée »).
pub(crate) fn poser_nom(
    store: &mut Store,
    account_id: i64,
    nom: Option<&str>,
) -> Result<(), mail_core::Error> {
    let cle = format!("nom_compte.{account_id}");
    store.set_text_prefs(&[(cle.as_str(), nom.unwrap_or(""))])
}

#[derive(Serialize)]
pub struct NomRow {
    pub account_id: i64,
    pub nom: String,
}

/// Tous les noms posés — l'UI les charge UNE fois (nav + réglages +
/// composeur) et patche sa table au geste (patron des repères).
#[tauri::command]
pub async fn noms_get(app: AppHandle) -> Result<Vec<NomRow>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let mut rows = Vec::new();
        for compte in store.accounts().map_err(|err| err.to_string())? {
            if let Some(nom) = nom_de(&store, compte.id).map_err(|err| err.to_string())? {
                rows.push(NomRow {
                    account_id: compte.id,
                    nom,
                });
            }
        }
        Ok(rows)
    })
    .await
}

/// Pose ou retire (chaîne vide / None) le nom d'un compte. Retourne le
/// nom NORMALISÉ effectivement écrit — c'est lui que l'UI affiche.
#[tauri::command]
pub async fn nom_set(
    app: AppHandle,
    account_id: i64,
    nom: Option<String>,
) -> Result<Option<String>, String> {
    hors_pompe(app, move |app| {
        let normalise = nom_normalise(nom.as_deref().unwrap_or(""))?;
        let mut store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        poser_nom(&mut store, account_id, normalise.as_deref()).map_err(|err| err.to_string())?;
        Ok(normalise)
    })
    .await
}

// ---------------------------------------------------------------------
// Brouillons locaux + reflet Gmail par compte (Phases 2-3).
// ---------------------------------------------------------------------

#[derive(Serialize)]
pub struct DraftRow {
    pub id: i64,
    pub account_id: i64,
    pub to: String,
    pub cc: String,
    pub bcc: String,
    pub subject: String,
    pub body: String,
    /// Corps riche tel que stocké — `None` pour un brouillon texte : la
    /// reprise le convertit à l'ouverture (échappement + retours),
    /// l'inverse exact de la dérivation texte côté sauvegarde.
    pub body_html: Option<String>,
    pub reply_to_uid: Option<u32>,
    /// La boîte qui donne son sens à `reply_to_uid` (ADR 0009) — la
    /// reprise doit la restituer au composeur, sans quoi la chaîne
    /// réponse → brouillon → reprise perd le fil.
    pub reply_to_mailbox: Option<String>,
    /// Le fil auquel ce brouillon répond, résolu par le cœur — `None`
    /// pour une composition libre ou une cible disparue.
    pub thread_id: Option<i64>,
    /// Marqué « important » (R3) — la reprise restitue l'état du bouton.
    pub important: bool,
    /// L'éditeur le renvoie à la sauvegarde : c'est ce qui lui permet de
    /// détecter qu'un autre a écrit entre-temps.
    pub updated_epoch: i64,
}

/// Ce qu'une sauvegarde a fait — l'éditeur en a besoin pour la suivante.
#[derive(Serialize)]
pub struct DraftSavedRow {
    pub id: i64,
    pub updated_epoch: i64,
    /// Le brouillon avait changé ailleurs : le texte de l'éditeur a été
    /// conservé à part. À dire à l'utilisateur, jamais à taire.
    pub forked: bool,
}

/// Sauvegarde un brouillon — texte brut, jamais validé : c'est un filet.
/// Le contenu tel que l'éditeur l'envoie — regroupé pour la même raison
/// qu'au noyau : quatre chaînes voisines invitent à en intervertir deux.
///
/// `camelCase` : Tauri ne convertit les noms qu'au premier niveau des
/// arguments. Sans cette annotation, l'UI devrait envoyer `reply_to_uid`
/// ici et `replyToUid` ailleurs — une incohérence qui ne se voit qu'à
/// l'exécution.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftContentArg {
    to: String,
    cc: String,
    bcc: String,
    subject: String,
    body: String,
    /// Corps riche de l'éditeur (PLAN-COMPOSITION-HTML) — absent ou vide
    /// = brouillon texte. Assaini côté Rust avant toute écriture.
    body_html: Option<String>,
    reply_to_uid: Option<u32>,
    reply_to_mailbox: Option<String>,
    /// Marqué « important » (R3, PLAN-RETOURS-6). `default` : un
    /// appelant d'avant le champ n'envoie rien — brouillon ordinaire.
    #[serde(default)]
    important: bool,
}

/// LA frontière du corps riche (PLAN-COMPOSITION-HTML) — le point unique
/// par lequel tout corps entre en base (brouillon, journal d'envoi,
/// tirage) : assaini par ammonia, texte du repli DÉRIVÉ du même HTML
/// (une seule autorité, jamais deux vérités).
///
/// `AllowRemote` ICI : la frontière ne re-neutralise pas ce que l'amont
/// a décidé. La politique des images distantes se joue AU CONTEXTE
/// (verdict terrain D5, 2026-08-20) — une RÉPONSE cite en pixel neutre
/// (`citation_reply`, §6.4 : reposée dans l'éditeur, elle ne doit rien
/// charger) et l'assainissement étant idempotent, elle reste neutre en
/// repassant ici ; un TRANSFERT conserve ses images (le destinataire
/// reçoit le message entier), un collage volontaire aussi.
///
/// Un HTML vide, blanc, ou dont le RENDU texte est vide (le `<br>`
/// résiduel d'un contenteditable vidé) vaut « pas de HTML » : chemin
/// texte — sans quoi la partie text/plain d'un envoi partirait vide.
fn frontiere_corps(body: String, body_html: Option<&str>) -> (String, Option<String>) {
    let riche = body_html
        .filter(|html| !html.trim().is_empty())
        .map(|html| mail_render::sanitize_with(html, mail_render::ImagePolicy::AllowRemote).html);
    match riche {
        Some(html) => {
            let texte = mail_render::body_text(&html);
            if texte.trim().is_empty() {
                (body, None)
            } else {
                (texte, Some(html))
            }
        }
        None => (body, None),
    }
}

#[tauri::command]
pub async fn save_draft(
    app: AppHandle,
    account_id: i64,
    id: Option<i64>,
    base_epoch: Option<i64>,
    content: DraftContentArg,
) -> Result<DraftSavedRow, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        // Même frontière que l'envoi (`frontiere_corps`) : HTML assaini,
        // texte dérivé (aperçus et repli).
        let (corps_texte, corps_riche) =
            frontiere_corps(content.body.clone(), content.body_html.as_deref());
        let saved = store
            .save_draft(
                account_id,
                id,
                base_epoch,
                mail_core::DraftContent {
                    to_raw: &content.to,
                    cc_raw: &content.cc,
                    bcc_raw: &content.bcc,
                    body_html: corps_riche.as_deref(),
                    subject: &content.subject,
                    body: &corps_texte,
                    reply_to_uid: content.reply_to_uid,
                    reply_to_mailbox: content.reply_to_mailbox.as_deref(),
                    important: content.important,
                },
            )
            .map_err(|err| err.to_string())?;
        Ok(DraftSavedRow {
            id: saved.id,
            updated_epoch: saved.updated_epoch,
            forked: saved.forked,
        })
    })
    .await
}

#[tauri::command]
pub async fn list_drafts(app: AppHandle) -> Result<Vec<DraftRow>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        Ok(store
            .drafts()
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|draft| DraftRow {
                updated_epoch: draft.updated_epoch,
                id: draft.id,
                account_id: draft.account_id,
                to: draft.to_raw,
                cc: draft.cc_raw,
                bcc: draft.bcc_raw,
                subject: draft.subject,
                body: draft.body,
                body_html: draft.body_html,
                reply_to_uid: draft.reply_to_uid,
                reply_to_mailbox: draft.reply_to_mailbox,
                thread_id: draft.thread_id,
                important: draft.important,
            })
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn delete_draft(app: AppHandle, id: i64) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store.delete_draft(id).map_err(|err| err.to_string())
    })
    .await
}

// ---------------------------------------------------------------------
// Pièces jointes du composeur (PLAN-PIECES-JOINTES E2).
// ---------------------------------------------------------------------

/// Une pièce d'un brouillon, pour les puces du composeur. Métadonnées
/// seules — les octets ne quittent la base qu'à la construction MIME.
#[derive(Serialize)]
pub struct DraftPieceRow {
    pub id: i64,
    pub name: String,
    pub mime: String,
    /// Octets décodés, bruts — le poids total se somme côté UI.
    pub size: u64,
    /// Taille lisible, même forme que la Lecture (« 2.4 Mo »).
    pub human: String,
}

/// Une pièce refusée au plafond (PJ-D3) — la surface dit le nom et la
/// place restante, lisible.
#[derive(Serialize)]
pub struct RefusedPiece {
    pub name: String,
    pub remaining: String,
}

/// Bilan du geste « Joindre ».
#[derive(Serialize)]
pub struct AttachReport {
    /// Le brouillon-ancre (créé au premier fichier si besoin, PJ-D1).
    /// `None` : rien n'est entré ET aucun brouillon n'existait — l'ancre
    /// créée pour rien a été reprise, pas de brouillon vide qui traîne.
    pub draft_id: Option<i64>,
    /// `None` si aucun fichier n'est entré (tout refusé) : le brouillon
    /// n'a pas bougé, l'éditeur garde son repère.
    pub updated_epoch: Option<i64>,
    /// TOUTES les pièces du brouillon après le geste, dans l'ordre.
    pub pieces: Vec<DraftPieceRow>,
    pub refused: Vec<RefusedPiece>,
}

fn piece_row(meta: mail_core::DraftAttachmentMeta) -> DraftPieceRow {
    DraftPieceRow {
        id: meta.id,
        name: meta.name,
        mime: meta.mime,
        size: meta.size,
        human: mail_core::human_size(meta.size),
    }
}

/// Type MIME déduit de l'extension — pour l'en-tête de la pièce, jamais
/// pour une décision : un inconnu part en `application/octet-stream`,
/// honnête et universellement accepté.
fn mime_for_name(name: &str) -> &'static str {
    let extension = name.rsplit('.').next().unwrap_or_default().to_lowercase();
    match extension.as_str() {
        "pdf" => "application/pdf",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "txt" | "md" | "log" => "text/plain",
        "html" | "htm" => "text/html",
        "csv" => "text/csv",
        "zip" => "application/zip",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "eml" => "message/rfc822",
        "ics" => "text/calendar",
        _ => "application/octet-stream",
    }
}

/// Joint des fichiers au brouillon : lit chaque chemin, copie les octets
/// en base au geste (PJ-D1 — le sélecteur a rendu des chemins, ils ne
/// survivent pas à ce appel), refuse au plafond sans punir l'acquis.
///
/// `draft_id: None` : le brouillon-ancre est créé, vide de texte —
/// l'autosave du composeur le remplira avec l'id et l'epoch rendus ici.
#[tauri::command]
pub async fn attach_files(
    app: AppHandle,
    account_id: i64,
    draft_id: Option<i64>,
    paths: Vec<String>,
) -> Result<AttachReport, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let cree = draft_id.is_none();
        let draft_id = match draft_id {
            Some(id) => id,
            None => {
                store
                    .save_draft(
                        account_id,
                        None,
                        None,
                        mail_core::DraftContent {
                            to_raw: "",
                            cc_raw: "",
                            bcc_raw: "",
                            body_html: None,
                            subject: "",
                            body: "",
                            reply_to_uid: None,
                            reply_to_mailbox: None,
                            important: false,
                        },
                    )
                    .map_err(|err| err.to_string())?
                    .id
            }
        };
        let mut updated_epoch = None;
        let mut refused = Vec::new();
        for path in &paths {
            // E8 : un chemin venu de l'UI se lit s'il est absolu et
            // désigne un fichier régulier — jamais un dossier, jamais un
            // chemin relatif au processus.
            let candidat = std::path::Path::new(path);
            if !candidat.is_absolute() || !candidat.is_file() {
                return Err(format!(
                    "pièce refusée : {path:?} n'est pas un fichier absolu"
                ));
            }
            // Échec de lecture = échec franc du geste : les fichiers déjà
            // entrés restent (l'UI relit les puces), celui-ci a un problème
            // que l'utilisateur doit voir, pas un silence.
            let bytes =
                std::fs::read(path).map_err(|err| format!("lecture de {path:?} : {err}"))?;
            let name = std::path::Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            match store.add_draft_attachment(draft_id, &name, mime_for_name(&name), &bytes) {
                Ok(saved) => updated_epoch = Some(saved.updated_epoch),
                Err(mail_core::Error::AttachmentOverBudget {
                    name, remaining, ..
                }) => refused.push(RefusedPiece {
                    name,
                    remaining: mail_core::human_size(remaining),
                }),
                Err(err) => return Err(err.to_string()),
            }
        }
        // L'ancre créée pour rien (tout refusé) est reprise sur-le-champ :
        // pas de brouillon vide fantôme au dossier.
        if cree && updated_epoch.is_none() {
            store
                .delete_draft(draft_id)
                .map_err(|err| err.to_string())?;
            return Ok(AttachReport {
                draft_id: None,
                updated_epoch: None,
                pieces: Vec::new(),
                refused,
            });
        }
        Ok(AttachReport {
            draft_id: Some(draft_id),
            updated_epoch,
            pieces: store
                .draft_attachments_meta(draft_id)
                .map_err(|err| err.to_string())?
                .into_iter()
                .map(piece_row)
                .collect(),
            refused,
        })
    })
    .await
}

/// Bilan du rapatriement d'UNE pièce du message d'origine (transfert,
/// PJ-D4). Deux issues nommées ici, la troisième — l'échec réseau — est
/// l'erreur de la commande : la surface les distingue (refus définitif
/// vs « Réessayer »).
#[derive(Serialize)]
pub struct FetchPieceReport {
    /// `None` : la pièce a été refusée ET aucun brouillon n'existait —
    /// l'ancre créée pour rien a été reprise.
    pub draft_id: Option<i64>,
    pub updated_epoch: Option<i64>,
    /// La pièce versée au brouillon, si le rapatriement a abouti.
    pub piece: Option<DraftPieceRow>,
    /// Le refus au plafond (PJ-D3) — définitif, pas de « Réessayer ».
    pub refused: Option<RefusedPiece>,
}

/// Rapatrie une pièce du message d'origine et la verse au brouillon-ancre
/// (PJ-D4) : les octets viennent du serveur (`fetch_attachment`, le
/// chemin de la Lecture), jamais d'un fichier local. Une par appel — le
/// composeur enchaîne, et chaque puce porte son propre état.
#[tauri::command]
pub async fn fetch_source_attachment(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    index: usize,
    draft_id: Option<i64>,
) -> Result<FetchPieceReport, String> {
    // E5 : lecture (pièce + session) sous `hors_pompe`, réseau nu, puis
    // écriture sous `hors_pompe` — plus jamais une connexion SQLite tenue
    // à travers l'attente réseau, ni un brouillon écrit hors du verrou
    // des commandes (le TOCTOU `save_draft`/`delete_draft` de l'ADR 0019).
    let boite = mailbox.clone();
    let (attachment, session) = hors_pompe(app.clone(), move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let attachment = store
            .attachments(account_id, &boite, uid)
            .map_err(|err| err.to_string())?
            .into_iter()
            .find(|candidate| candidate.index == index)
            .ok_or_else(|| "pièce jointe inconnue".to_string())?;
        Ok((attachment, auth_for(&app, account_id)?))
    })
    .await?;

    let bytes = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let (mut server, _refreshed) = connect_imap(&session)?;
        let bytes = server
            .fetch_attachment(&mailbox, uid, index)
            .map_err(|err| err.to_string())?;
        server.logout();
        bytes.ok_or_else(|| "pièce jointe absente du message".to_string())
    })
    .await
    .map_err(|err| err.to_string())??;

    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let cree = draft_id.is_none();
        let draft_id = match draft_id {
            Some(id) => id,
            None => {
                store
                    .save_draft(
                        account_id,
                        None,
                        None,
                        mail_core::DraftContent {
                            to_raw: "",
                            cc_raw: "",
                            bcc_raw: "",
                            body_html: None,
                            subject: "",
                            body: "",
                            reply_to_uid: None,
                            reply_to_mailbox: None,
                            important: false,
                        },
                    )
                    .map_err(|err| err.to_string())?
                    .id
            }
        };
        match store.add_draft_attachment(draft_id, &attachment.name, &attachment.mime, &bytes) {
            Ok(saved) => Ok(FetchPieceReport {
                draft_id: Some(draft_id),
                updated_epoch: Some(saved.updated_epoch),
                piece: Some(piece_row(saved.attachment)),
                refused: None,
            }),
            Err(mail_core::Error::AttachmentOverBudget {
                name, remaining, ..
            }) => {
                // L'ancre créée pour rien est reprise — même règle
                // qu'`attach_files` : pas de brouillon vide fantôme.
                let draft_id = if cree {
                    store
                        .delete_draft(draft_id)
                        .map_err(|err| err.to_string())?;
                    None
                } else {
                    Some(draft_id)
                };
                Ok(FetchPieceReport {
                    draft_id,
                    updated_epoch: None,
                    piece: None,
                    refused: Some(RefusedPiece {
                        name,
                        remaining: mail_core::human_size(remaining),
                    }),
                })
            }
            Err(err) => Err(err.to_string()),
        }
    })
    .await
}

/// Retire une pièce. Rend le nouvel `updated_epoch` du brouillon, ou
/// `None` si la pièce n'existait plus (double-clic) — rien n'a bougé.
#[tauri::command]
pub async fn detach_file(app: AppHandle, attachment_id: i64) -> Result<Option<i64>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .remove_draft_attachment(attachment_id)
            .map_err(|err| err.to_string())
    })
    .await
}

/// Les pièces d'un brouillon — la reprise redessine ses puces.
#[tauri::command]
pub async fn draft_attachments(
    app: AppHandle,
    draft_id: i64,
) -> Result<Vec<DraftPieceRow>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        Ok(store
            .draft_attachments_meta(draft_id)
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(piece_row)
            .collect())
    })
    .await
}

#[derive(Serialize)]
pub struct DraftSyncSummary {
    pub pushed: usize,
    pub purged: usize,
    /// Brouillons non poussables en l'état — ils restent locaux.
    pub kept_local: usize,
    /// Réseau indisponible — rien de changé, le cycle suivant retentera.
    pub error: Option<String>,
}

/// Reflète les brouillons de TOUS les comptes connectés dans leurs
/// dossiers Brouillons respectifs (poussée seule, v1). Sans travail,
/// aucun réseau. Réentrance interdite (verrou).
#[tauri::command]
pub async fn sync_drafts(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DraftSyncSummary, String> {
    let path = db_path(&app)?;
    let jobs = hors_pompe(app.clone(), |app| connected_jobs(&app)).await?;
    let lock = state.drafts_push.clone();

    let (summary, refreshed) =
        tauri::async_runtime::spawn_blocking(move || run_draft_sync_all(jobs, &path, &lock))
            .await
            .map_err(|err| err.to_string())??;

    reposer_sessions(&state, refreshed)?;
    Ok(summary)
}

fn run_draft_sync_all(
    jobs: Vec<(i64, AccountSession)>,
    db_path: &Path,
    lock: &Mutex<()>,
) -> Result<(DraftSyncSummary, Vec<AccountSession>), String> {
    // E5 : verrou empoisonné = repris (le panic est consigné, ADR 0014).
    let _guard = match lock.lock() {
        Ok(garde) => garde,
        Err(empoisonne) => empoisonne.into_inner(),
    };
    let store = Store::open(db_path).map_err(|err| err.to_string())?;
    let mut summary = DraftSyncSummary {
        pushed: 0,
        purged: 0,
        kept_local: 0,
        error: None,
    };
    let mut refreshed_list = Vec::new();

    for (account_id, session) in jobs {
        let nothing_to_do = store
            .drafts_to_push(account_id)
            .map_err(|err| err.to_string())?
            .is_empty()
            && store
                .draft_tombstones(account_id)
                .map_err(|err| err.to_string())?
                .is_empty();
        if nothing_to_do {
            continue;
        }

        let (mut server, refreshed) = match connect_imap(&session) {
            Ok(pair) => pair,
            Err(reason) => {
                summary.error = Some(reason);
                continue;
            }
        };
        if let Some(fresh) = refreshed {
            refreshed_list.push(fresh);
        }

        // La garde des repères : UIDVALIDITY d'abord, toute purge ensuite.
        match server.drafts_uidvalidity() {
            Ok(validity) => {
                store
                    .align_drafts_uidvalidity(account_id, validity)
                    .map_err(|err| err.to_string())?;
            }
            Err(err) => {
                summary.error = Some(err.to_string());
                server.logout();
                continue;
            }
        }

        if !purge_draft_tombstones(&mut server, &store, account_id, &mut summary)? {
            server.logout();
            continue;
        }

        for draft in store
            .drafts_to_push(account_id)
            .map_err(|err| err.to_string())?
        {
            // Les pièces suivent le texte (PJ-D6) : le reflet distant
            // montre le brouillon entier.
            let pieces = store
                .draft_attachments_full(draft.id)
                .map_err(|err| err.to_string())?;
            let bytes = match mail_smtp::draft_bytes(
                session.email(),
                &draft.to_raw,
                &draft.cc_raw,
                &draft.bcc_raw,
                &draft.subject,
                &draft.body,
                draft.body_html.as_deref(),
                &pieces,
            ) {
                Ok(bytes) => bytes,
                // Pas poussable en l'état : le local reste la référence.
                Err(_) => {
                    summary.kept_local += 1;
                    continue;
                }
            };
            match server.append_draft(&bytes) {
                Ok(remote_uid) => {
                    store
                        .record_draft_pushed(draft.id, remote_uid, draft.updated_epoch)
                        .map_err(|err| err.to_string())?;
                    summary.pushed += 1;
                }
                Err(err) => {
                    summary.error = Some(err.to_string());
                    break;
                }
            }
        }

        // Les remplacements de CE cycle viennent de créer leurs
        // tombstones : purge immédiate — pas de copie double visible.
        if summary.error.is_none() {
            purge_draft_tombstones(&mut server, &store, account_id, &mut summary)?;
        }
        server.logout();
    }
    Ok((summary, refreshed_list))
}

/// Purge les copies distantes en tombstone d'UN compte. Retourne `false`
/// si le réseau a lâché — la dette reste enregistrée pour le cycle suivant.
fn purge_draft_tombstones(
    server: &mut ImapServer,
    store: &Store,
    account_id: i64,
    summary: &mut DraftSyncSummary,
) -> Result<bool, String> {
    for uid in store
        .draft_tombstones(account_id)
        .map_err(|err| err.to_string())?
    {
        match server.delete_draft_remote(uid) {
            Ok(()) => {
                store
                    .clear_draft_tombstone(account_id, uid)
                    .map_err(|err| err.to_string())?;
                summary.purged += 1;
            }
            Err(err) => {
                summary.error = Some(err.to_string());
                return Ok(false);
            }
        }
    }
    Ok(true)
}

// ---------------------------------------------------------------------
// Connexions et état partagé.
// ---------------------------------------------------------------------

/// Ouvre une connexion SMTP adaptée au type de compte. Pour un compte
/// OAuth2, un échec déclenche un refresh silencieux ; pour un compte
/// générique, le mot de passe est fixe (pas de retry possible).
///
/// Les serveurs viennent du fournisseur de la session, jamais d'une
/// constante d'application : c'est ce qui rend un deuxième fournisseur
/// possible sans toucher à cette fonction.
fn connect_smtp(session: &AccountSession) -> Result<(SmtpMailer, Option<AccountSession>), String> {
    match session {
        AccountSession::OAuth(auth) => {
            let smtp = auth.provider.smtp;
            match SmtpMailer::connect_xoauth2(smtp.host, smtp.port, &auth.email, &auth.access_token)
            {
                Ok(mailer) => Ok((mailer, None)),
                // E7 : une panne RÉSEAU n'est pas un refus d'authentification —
                // refaire la session OAuth n'y changerait rien et martelait
                // l'endpoint du fournisseur (le défaut P0 corrigé côté IMAP).
                Err(err) if mail_smtp::is_connection_error(&err) => Err(err.to_string()),
                Err(_) => {
                    let fresh = Authenticator::from_env(auth.provider)
                        .map_err(|err| err.to_string())?
                        .authenticate_silent(&auth.email)
                        .map_err(|err| err.to_string())?;
                    let mailer = SmtpMailer::connect_xoauth2(
                        smtp.host,
                        smtp.port,
                        &fresh.email,
                        &fresh.access_token,
                    )
                    .map_err(|err| err.to_string())?;
                    Ok((mailer, Some(AccountSession::OAuth(fresh))))
                }
            }
        }
        AccountSession::Generic(creds) => {
            let mailer = SmtpMailer::connect_password(
                &creds.smtp_host,
                creds.smtp_port,
                &creds.username,
                &creds.password,
            )
            .map_err(|err| err.to_string())?;
            Ok((mailer, None))
        }
    }
}

/// Ouvre une connexion IMAP adaptée au type de compte. Pour un compte
/// OAuth2, un échec déclenche un refresh silencieux ; pour un compte
/// générique, le mot de passe est fixe.
pub(crate) fn connect_imap(
    session: &AccountSession,
) -> Result<(ImapServer, Option<AccountSession>), String> {
    match session {
        AccountSession::OAuth(auth) => {
            let imap = auth.provider.imap;
            match ImapServer::connect_xoauth2(imap.host, imap.port, &auth.email, &auth.access_token)
            {
                Ok(server) => Ok((server, None)),
                // Une panne de CONNEXION n'est pas un jeton mort : pas de
                // rafraîchissement — marteler l'endpoint OAuth à chaque
                // cycle en panne réseau est le meilleur moyen de
                // transformer un bridage IMAP en gel du compte
                // (complément P0, anti-martèlement).
                Err(err) if mail_imap::is_connection_error(&err) => Err(err.to_string()),
                Err(_) => {
                    let fresh = Authenticator::from_env(auth.provider)
                        .map_err(|err| err.to_string())?
                        .authenticate_silent(&auth.email)
                        .map_err(|err| err.to_string())?;
                    let server = ImapServer::connect_xoauth2(
                        imap.host,
                        imap.port,
                        &fresh.email,
                        &fresh.access_token,
                    )
                    .map_err(|err| err.to_string())?;
                    Ok((server, Some(AccountSession::OAuth(fresh))))
                }
            }
        }
        AccountSession::Generic(creds) => {
            let server = ImapServer::connect_password(
                &creds.imap_host,
                creds.imap_port,
                &creds.username,
                &creds.password,
            )
            .map_err(|err| err.to_string())?;
            Ok((server, None))
        }
    }
}

/// Les comptes du registre qui sont connectés (session en mémoire) —
/// l'unité de travail des boucles synchro/vidange/brouillons.
/// Les comptes connus ET connectés — ouvre la base : à appeler SOUS
/// `hors_pompe` (E5), jamais dans la glu d'une commande async.
fn connected_jobs(app: &AppHandle) -> Result<Vec<(i64, AccountSession)>, String> {
    let store = Store::open(&db_path(app)?).map_err(|err| err.to_string())?;
    let known = store.accounts().map_err(|err| err.to_string())?;
    let state = app.state::<AppState>();
    let connected = lock_accounts(&state)?;
    Ok(known
        .into_iter()
        .filter_map(|account| {
            connected
                .get(&account.email)
                .cloned()
                .map(|session| (account.id, session))
        })
        .collect())
}

/// La session d'un compte — ouvre la base : SOUS `hors_pompe` (E5).
fn auth_for(app: &AppHandle, account_id: i64) -> Result<AccountSession, String> {
    let store = Store::open(&db_path(app)?).map_err(|err| err.to_string())?;
    let email = account_email(&store, account_id)?;
    let state = app.state::<AppState>();
    lock_accounts(&state)?
        .get(&email)
        .cloned()
        .ok_or_else(|| format!("compte non connecté : {email}"))
}

// Délègue à `Store::account_email` (revue PLAN-INVITATIONS) : UNE seule
// réponse à « l'adresse du compte N » — la lecture des invitations et
// l'envoi de leur réponse doivent voir la MÊME vérité (adresse vide =
// compte à moitié provisionné = inconnu, comme `Store::accounts`).
fn account_email(store: &Store, account_id: i64) -> Result<String, String> {
    store
        .account_email(account_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "compte inconnu".to_string())
}

pub(crate) fn lock_accounts<'a>(
    state: &'a State<'_, AppState>,
) -> Result<MutexGuard<'a, HashMap<String, AccountSession>>, String> {
    // E5 : un verrou empoisonné (panic d'une commande) se REPREND — le
    // panic est déjà consigné par la télémétrie (ADR 0014) ; condamner
    // toutes les commandes suivantes jusqu'au redémarrage, comme avant,
    // contredisait l'ADR 0019.
    Ok(match state.accounts.lock() {
        Ok(garde) => garde,
        Err(empoisonne) => empoisonne.into_inner(),
    })
}

/// Repose les sessions rafraîchies par une boucle (jeton OAuth renouvelé)
/// — SANS ressusciter un compte retiré pendant qu'elle tournait : sa
/// ligne en base a disparu, une session orpheline en mémoire ferait
/// échouer chaque cycle suivant jusqu'au redémarrage.
fn reposer_sessions(
    state: &State<'_, AppState>,
    refreshed: Vec<AccountSession>,
) -> Result<(), String> {
    let mut accounts = lock_accounts(state)?;
    for fresh in refreshed {
        if accounts.contains_key(fresh.email()) {
            accounts.insert(fresh.email().to_string(), fresh);
        }
    }
    Ok(())
}

/// Exécute un travail bloquant HORS de la pompe de messages et SOUS le
/// verrou global des commandes (PLAN-GELS).
///
/// Les deux moitiés sont indissociables : `spawn_blocking` libère la
/// pompe (une commande `async` sans lui bloquerait un worker tokio — le
/// gel quitterait la fenêtre pour réapparaître dans la file IPC sur une
/// machine à deux cœurs) ; le verrou restaure la sérialisation que le
/// thread principal offrait gratuitement — sans lui, les paires
/// lecture-décision-écriture des commandes se croiseraient (état local
/// contre file d'actions de `mark_flagged`, TOCTOU des brouillons,
/// `SQLITE_BUSY_SNAPSHOT` que le `busy_timeout` ne couvre pas). Un
/// verrou empoisonné se récupère (même choix que `verrou_compte`) : le
/// travail sous verrou n'a pas d'invariant en mémoire partagée.
pub(crate) async fn hors_pompe<T, F>(app: AppHandle, travail: F) -> Result<T, String>
where
    F: FnOnce(AppHandle) -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    let verrou = app.state::<AppState>().commandes.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _garde = match verrou.lock() {
            Ok(garde) => garde,
            Err(poison) => poison.into_inner(),
        };
        travail(app)
    })
    .await
    .map_err(|err| err.to_string())?
}

pub(crate) fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    // PLAN-AUDIT-V1 E5 : calculé UNE fois (le dossier est créé à ce
    // premier appel), puis une lecture pure — 107 appels par session
    // faisaient chacun leur `create_dir_all`.
    static CHEMIN: OnceLock<PathBuf> = OnceLock::new();
    if let Some(chemin) = CHEMIN.get() {
        return Ok(chemin.clone());
    }
    // Crochet E2E : base isolée fournie par le pilote de test — la vraie
    // base de l'utilisateur ne doit jamais être touchée par un test.
    let chemin = if let Ok(path) = std::env::var("WIND_DB_PATH") {
        PathBuf::from(path)
    } else {
        let dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        dir.join("wind.db")
    };
    Ok(CHEMIN.get_or_init(|| chemin).clone())
}

// ---------------------------------------------------------------------
// Rattrapage des corps (ADR 0007, horizon levé par l'ADR 0010).
// ---------------------------------------------------------------------

/// Combien de messages attendent leur corps, et combien peuvent en
/// porter — tous comptes et TOUTES boîtes confondus (ADR 0010 §1,
/// dénominateur R1 PLAN-RETOURS-3 : `corpus - pending` = corps
/// présents). Purement local : aucune connexion réseau.
/// (en attente, corpus) en UNE passe : l'horizon du compte et sa liste
/// de boîtes se lisent une fois pour les DEUX compteurs (revue
/// 2026-08-30 : deux passes indépendantes payaient deux fois les prefs
/// et les listes — et sur une erreur intermittente, numérateur et
/// dénominateur pouvaient se calculer sous des horizons DIFFÉRENTS).
/// L'horizon borne les deux COMME la pompe : sans lui, la barre d'un
/// compte borné n'atteindrait jamais 100 %.
fn totaux_corps(store: &Store) -> Result<(u64, u64), String> {
    let mut pending = 0;
    let mut corpus = 0;
    for account in store.accounts().map_err(|err| err.to_string())? {
        let horizon = horizon_corps(store, account.id);
        for boite in store
            .mailbox_names(account.id)
            .map_err(|err| err.to_string())?
        {
            pending += store
                .bodies_pending_count(account.id, &boite, horizon)
                .map_err(|err| err.to_string())?;
            corpus += store
                .bodies_total_count(account.id, &boite, horizon)
                .map_err(|err| err.to_string())?;
        }
    }
    Ok((pending, corpus))
}

#[derive(Serialize)]
pub struct SyncProgress {
    /// Messages en base, toutes boîtes déjà visitées confondues.
    pub local: u64,
    /// Messages annoncés par les serveurs pour ces mêmes boîtes.
    pub remote: u64,
    /// `None` tant qu'aucune boîte n'a été sélectionnée : l'interface
    /// n'affiche alors rien, plutôt qu'un « 0 % » qui ferait croire à une
    /// synchronisation en panne.
    pub percent: Option<u8>,
    /// Epoch (secondes) de la dernière relève réussie — `None` tant
    /// qu'aucun cycle n'a abouti : l'interface n'invente pas
    /// d'horodatage (PLAN-SYNCHRO E1).
    pub derniere: Option<i64>,
    /// Génération de courrier, monotone (E4) : l'UI recharge la liste
    /// quand elle bouge — c'est ainsi que le courrier relevé par un
    /// veilleur IDLE se montre au repos, en sondage (R0-S5).
    pub generation: u64,
}

/// Avancement de la synchronisation intégrale (ADR 0010 §5).
///
/// Purement local — aucune connexion réseau : l'interface peut l'appeler
/// en boucle pendant qu'une synchronisation tourne, sans lui coûter un
/// seul aller-retour.
#[tauri::command]
pub async fn sync_progress(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SyncProgress, String> {
    // `State` ne traverse pas le `spawn_blocking` (durée de vie) : on
    // emporte l'Arc du cycle, pas l'état.
    let cycle = state.sync_cycle.clone();
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        let (local, remote) = store.sync_progress().map_err(|err| err.to_string())?;
        // Un horodatage illisible (pref corrompue) vaut « jamais » : la barre
        // d'état retombe sur le texte sans date plutôt que d'afficher n'importe quoi.
        let derniere = store
            .text_pref(PREF_DERNIERE_SYNCHRO)
            .map_err(|err| err.to_string())?
            .and_then(|valeur| valeur.parse::<i64>().ok());
        Ok(SyncProgress {
            local,
            remote,
            percent: mail_core::sync_percent(local, remote),
            derniere,
            generation: cycle.generation.load(Ordering::Relaxed),
        })
    })
    .await
}

/// P0-bis + E4 : l'UI remonte l'état réseau de l'OS (`navigator.onLine`).
/// Hors ligne, les veilleurs IDLE dorment (reconnecter en boucle sans
/// réseau ne sert à rien) ; au retour, les reculs s'effacent — le réseau
/// est neuf, l'échec d'hier était la coupure, pas le serveur — et les
/// veilleurs repartent d'eux-mêmes.
#[tauri::command]
pub fn reseau_etat(state: State<'_, AppState>, en_ligne: bool) -> Result<(), String> {
    state.en_ligne.store(en_ligne, Ordering::Relaxed);
    if en_ligne && let Ok(mut reculs) = state.sync_reculs.lock() {
        reculs.clear();
    }
    Ok(())
}

#[derive(Serialize)]
pub struct SyncActivite {
    /// Comptes déjà soldés dans le cycle en cours.
    pub fait: u64,
    pub total: u64,
    /// Adresse du compte en cours de relève.
    pub compte: String,
    /// Boîte en cours DANS le compte — vide entre deux boîtes.
    pub boite: String,
    /// Étape sans boîte (`inventaire`, `fils`, `brouillons`) — clé de
    /// catalogue, traduite par l'UI. Vide quand une boîte est nommée.
    pub phase: String,
    /// Courrier d'INBOX déjà en base dans CE cycle (arrivées + retraits,
    /// cumulés compte après compte) — P1 : la sonde recharge la liste
    /// dès que ce compteur bouge, sans attendre la fin du cycle.
    pub courrier: u64,
}

/// Le cycle en cours, pour la barre d'état (PLAN-SYNCHRO E1).
///
/// Purement mémoire — aucun réseau, aucune base : l'UI sonde à la
/// seconde PENDANT le cycle sans rien coûter à la boucle (atomiques,
/// patron de `migration_progress`). `None` au repos.
#[tauri::command]
pub fn sync_activity(state: State<'_, AppState>) -> Option<SyncActivite> {
    let cycle = &state.sync_cycle;
    if !cycle.en_cours.load(Ordering::Relaxed) {
        return None;
    }
    let compte = cycle
        .compte
        .lock()
        .map(|nom| nom.clone())
        .unwrap_or_default();
    let boite = cycle
        .boite
        .lock()
        .map(|nom| nom.clone())
        .unwrap_or_default();
    let phase = cycle
        .phase
        .lock()
        .map(|nom| nom.clone())
        .unwrap_or_default();
    Some(SyncActivite {
        fait: cycle.fait.load(Ordering::Relaxed),
        total: cycle.total.load(Ordering::Relaxed),
        compte,
        boite,
        phase,
        courrier: cycle.courrier.load(Ordering::Relaxed),
    })
}

#[derive(Serialize)]
pub struct BackfillStatus {
    pub remaining: u64,
    /// Le pourcentage de corps DÉJÀ présents sur le corpus en portée
    /// (R1, PLAN-RETOURS-3) — `None` sans dénominateur (aucun message).
    pub percent: Option<u8>,
}

/// État du rattrapage, sans rien télécharger — de quoi afficher
/// « N restants · P % » avant même de commencer.
#[tauri::command]
pub async fn backfill_status(app: AppHandle) -> Result<BackfillStatus, String> {
    hors_pompe(app, move |app| {
        // Jalons de mesure (feature `mesure` — jamais dans le binaire
        // livré). Le span d'amont `wry::custom_protocol::handle` donne le
        // TOTAL de la commande et rien de plus : mesuré à froid le
        // 2026-08-26, il valait 2 740 ms après la correction du prédicat,
        // sans dire lequel des trois temps le portait. On a vérifié que
        // ce n'était NI la file d'attente (le verrou était libre) NI
        // `corpus_total` (35 ms) — restait l'ouverture, qu'aucun span ne
        // couvre. D'où ces trois-là.
        let store = {
            #[cfg(feature = "mesure")]
            let _jalon = tracing::debug_span!("mesure::store_open").entered();
            Store::open(&db_path(&app)?).map_err(|err| err.to_string())?
        };
        let (remaining, total) = {
            #[cfg(feature = "mesure")]
            let _jalon = tracing::debug_span!("mesure::totaux_corps").entered();
            totaux_corps(&store)?
        };
        Ok(BackfillStatus {
            remaining,
            // `done = total - remaining` : les corps déjà là. La fonction
            // pure plafonne à 99 tant qu'il reste des corps (R1).
            percent: mail_core::backfill_percent(total.saturating_sub(remaining), total),
        })
    })
    .await
}

// ---------------------------------------------------------------------
// Migration visible et interruptible (Phase 5, ADR 0012).
//
// Chaque commande ouvre sa propre connexion : sans cet écran, c'est la
// PREMIÈRE commande venue qui paierait l'adoption d'une base héritée —
// en silence, dans un gel d'interface. L'UI appelle donc
// `migration_check` AVANT toute commande qui touche la base ; s'il y a
// du travail, elle affiche l'écran, lance `migration_run`, sonde
// `migration_progress`, et `migration_cancel` rembobine tout (§8 de la
// passation : jamais d'adoption partielle persistée).
// ---------------------------------------------------------------------

#[derive(Serialize)]
pub struct MigrationCheck {
    /// Messages à adopter — `null` si l'ouverture sera silencieuse.
    pub pending: Option<u64>,
}

/// Sonde en lecture seule : rien n'est déclenché, rien n'est créé.
#[tauri::command]
pub async fn migration_check(app: AppHandle) -> Result<MigrationCheck, String> {
    hors_pompe(app, move |app| {
        Ok(MigrationCheck {
            pending: Store::pending_adoption(&db_path(&app)?).map_err(|err| err.to_string())?,
        })
    })
    .await
}

#[derive(Serialize)]
pub struct MigrationProgress {
    pub done: u64,
    pub total: u64,
    /// `None` tant que la passe n'a rien annoncé : l'écran n'affiche
    /// alors rien plutôt qu'un « 0 % » qui ferait croire à une panne.
    pub percent: Option<u8>,
}

/// Avancement de la passe en cours. Purement local et sans verrou :
/// la passe écrit des atomiques, le sondage ne la fait jamais attendre.
#[tauri::command]
pub fn migration_progress(state: State<'_, AppState>) -> MigrationProgress {
    let done = state.migration.done.load(Ordering::Relaxed);
    let total = state.migration.total.load(Ordering::Relaxed);
    MigrationProgress {
        done,
        total,
        percent: mail_core::sync_percent(done, total),
    }
}

/// Demande l'annulation : la passe la constate à son prochain palier et
/// rembobine tout — `migration_run` rendra alors `false`.
#[tauri::command]
pub fn migration_cancel(state: State<'_, AppState>) {
    state.migration.cancel.store(true, Ordering::Relaxed);
}

/// Joue la passe d'adoption, visible et interruptible.
///
/// Rend `true` si la base est migrée (ou n'avait rien à faire), `false`
/// si l'utilisateur a annulé — tout est alors défait, `user_version`
/// inchangé, et la passe entière se rejouera au prochain lancement.
#[tauri::command]
pub async fn migration_run(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let path = db_path(&app)?;
    let shared = state.migration.clone();
    shared.cancel.store(false, Ordering::Relaxed);
    shared.done.store(0, Ordering::Relaxed);
    shared.total.store(0, Ordering::Relaxed);

    tauri::async_runtime::spawn_blocking(move || {
        let result = Store::open_with_progress(&path, |progress| {
            shared.done.store(progress.done, Ordering::Relaxed);
            shared.total.store(progress.total, Ordering::Relaxed);
            if shared.cancel.load(Ordering::Relaxed) {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });
        match result {
            // Le Store se referme aussitôt : les commandes suivantes
            // ouvriront le leur, comme d'habitude — mais sans passe à
            // payer, elle est faite.
            Ok(_store) => Ok(true),
            Err(mail_core::Error::Interrupted) => Ok(false),
            Err(err) => Err(err.to_string()),
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[derive(Serialize)]
pub struct BackfillSummary {
    pub fetched: usize,
    pub remaining: u64,
    /// Le pourcentage de corps présents après ce lot (R1) — met à jour la
    /// barre d'état au fil des lots, sans re-sonder depuis l'UI.
    pub percent: Option<u8>,
    pub errors: Vec<String>,
}

/// UN lot de rattrapage, tous comptes connectés confondus.
///
/// Volontairement borné : l'UI rappelle tant qu'il reste du travail, et
/// s'arrête quand l'utilisateur le demande. L'interruption est ainsi
/// gratuite — aucun jeton d'annulation à propager — et une coupure ne
/// coûte jamais plus qu'un lot.
#[tauri::command]
pub async fn backfill_bodies(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BackfillSummary, String> {
    let path = db_path(&app)?;
    let jobs = hors_pompe(app.clone(), |app| connected_jobs(&app)).await?;
    let lock = state.bodies_backfill.clone();

    let (summary, refreshed) =
        tauri::async_runtime::spawn_blocking(move || run_backfill_all(jobs, &path, &lock))
            .await
            .map_err(|err| err.to_string())??;

    reposer_sessions(&state, refreshed)?;
    Ok(summary)
}

fn run_backfill_all(
    jobs: Vec<(i64, AccountSession)>,
    db_path: &Path,
    lock: &Mutex<()>,
) -> Result<(BackfillSummary, Vec<AccountSession>), String> {
    // E5 : verrou empoisonné = repris (le panic est consigné, ADR 0014).
    let _guard = match lock.lock() {
        Ok(garde) => garde,
        Err(empoisonne) => empoisonne.into_inner(),
    };

    let mut store = Store::open(db_path).map_err(|err| err.to_string())?;
    let mut summary = BackfillSummary {
        fetched: 0,
        remaining: 0,
        percent: None,
        errors: Vec::new(),
    };
    let mut refreshed_list = Vec::new();
    // Le budget est PARTAGÉ entre les comptes : un lot reste un lot, même
    // avec trois comptes connectés.
    let mut budget = BACKFILL_BUDGET;

    for (account_id, session) in jobs {
        if budget == 0 {
            break;
        }
        let email = session.email().to_string();
        // TOUTES les boîtes du compte (ADR 0010 §1), dans l'ordre du
        // store : réception d'abord, envois ensuite, le reste après. Le
        // budget est partagé entre elles comme entre les comptes — un
        // dossier d'archive de 80 000 messages ne confisque pas le lot,
        // il consomme ce que les boîtes prioritaires ont laissé.
        let boites = store
            .mailbox_names(account_id)
            .map_err(|err| err.to_string())?;
        // La pompe travaille DANS l'horizon d'import du compte (ADR 0029) :
        // au-delà, les corps restent au serveur et se chargent au clic.
        let horizon = horizon_corps(&store, account_id);
        // Ne pas ouvrir une connexion pour un compte qui n'a rien à faire.
        let mut pending = 0;
        for boite in &boites {
            pending += store
                .bodies_pending_count(account_id, boite, horizon)
                .map_err(|err| err.to_string())?;
        }
        if pending == 0 {
            continue;
        }
        match connect_imap(&session) {
            Err(reason) => summary.errors.push(format!("{email} : {reason}")),
            Ok((mut server, refreshed)) => {
                if let Some(fresh) = refreshed {
                    refreshed_list.push(fresh);
                }
                for boite in &boites {
                    if budget == 0 {
                        break;
                    }
                    match mail_core::backfill_bodies(
                        &mut server,
                        &mut store,
                        account_id,
                        boite,
                        horizon,
                        budget,
                    ) {
                        Ok(report) => {
                            summary.fetched += report.fetched;
                            budget = budget.saturating_sub(report.fetched);
                        }
                        // L'échec d'UNE boîte ne prive pas les suivantes :
                        // même règle que la synchronisation des dossiers.
                        Err(err) => summary.errors.push(format!("{email}, « {boite} » : {err}")),
                    }
                }
                server.logout();
            }
        }
    }

    let (remaining, total) = totaux_corps(&store)?;
    summary.remaining = remaining;
    summary.percent = mail_core::backfill_percent(total.saturating_sub(summary.remaining), total);
    summary.errors.sort();
    Ok((summary, refreshed_list))
}

// ---------------------------------------------------------------------
// Mise a jour automatique signee (ADR 0013).
//
// Pilotee depuis Rust, comme les notifications : la webview n'appelle
// jamais l'API updater, seulement ces deux commandes — les capabilities
// restent `core:default`. La signature minisign est verifiee par le
// plugin AVANT toute installation ; sans elle, `download_and_install`
// echoue plutot que d'appliquer un paquet falsifie.
// ---------------------------------------------------------------------

use tauri_plugin_updater::UpdaterExt;

#[derive(Serialize)]
pub struct UpdateInfo {
    pub version: String,
    /// Notes de version, si la Release en porte.
    pub notes: Option<String>,
    /// Date de publication ISO 8601, telle qu'annoncee par le manifeste.
    pub date: Option<String>,
}

/// Y a-t-il une mise a jour ? `None` = a jour, ou hors ligne.
///
/// Appelee UNE fois au demarrage, en silence : un controle que
/// l'utilisateur doit reclamer n'aurait pas lieu (lecon de l'ADR 0007).
/// Hors ligne, l'endpoint est injoignable — ce n'est pas un defaut, donc
/// l'erreur remonte a l'UI qui reste muette plutot que de harceler ;
/// elle n'est jamais AVALEE (§9), seulement jugee sans gravite par
/// l'appelant.
#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    // Les E2E ne parlent a AUCUN serveur (passation §7.5). Sans cette
    // garde, des qu'une Release existe, l'endpoint `latest.json`
    // repondrait et le bandeau apparaitrait en plein test — un flake.
    // `WIND_DB_PATH` n'est pose que par le harnais : c'est le meme
    // signal d'isolation que la base jetable.
    if std::env::var("WIND_DB_PATH").is_ok() {
        return Ok(None);
    }
    match updater_wind(&app)?
        .check()
        .await
        .map_err(|err| err.to_string())?
    {
        Some(update) => Ok(Some(UpdateInfo {
            version: update.version.clone(),
            notes: update.body.clone(),
            date: update.date.map(|d| d.to_string()),
        })),
        None => Ok(None),
    }
}

/// La version installee, pour la section « A propos » des Reglages.
/// Une lecture du manifeste — aucun reseau, aucune base.
#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Ouvre un lien du corps d'un message dans l'application SYSTEME
/// (navigateur, client mail) — constat terrain 2026-08-15 : sans ce
/// chemin, le clic naviguait l'iframe sandbox vers le site, refuse
/// (X-Frame-Options / CSP), et WebView2 remplacait le corps par sa
/// page « Ce contenu a ete bloque ».
///
/// La GARDE vit ici, pas dans l'UI : seuls http, https et mailto
/// passent — tout autre schema (file, smb, chemins UNC…) est refuse
/// nommement. `open::that_detached` emballe ShellExecuteW sans bloquer
/// le thread de commande — SEULEMENT avec la feature
/// `shellexecute-on-windows` de la crate (Cargo.toml) ; sans elle, c'est
/// un `powershell.exe` lancé de façon synchrone (audit 2026-09-01).
#[tauri::command]
pub fn open_link(url: String) -> Result<(), String> {
    let propre = url.trim();
    let bas = propre.to_ascii_lowercase();
    let permis =
        bas.starts_with("http://") || bas.starts_with("https://") || bas.starts_with("mailto:");
    if !permis {
        return Err(format!("schéma de lien refusé : {propre}"));
    }
    open::that_detached(propre).map_err(|err| err.to_string())
}

/// Bulles d'arrivee : la preference se LIT pour l'afficher…
#[tauri::command]
pub async fn notif_pref_get(app: AppHandle) -> Result<bool, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .bool_pref(PREF_ARRIVAL_BUBBLES, true)
            .map_err(|err| err.to_string())
    })
    .await
}

/// …et se POSE depuis le groupe Notifications des Reglages. Persistee en
/// base (PLAN-REGLAGES, R-D2) : c'est le shell Rust qui emet les bulles,
/// localStorage lui serait invisible.
#[tauri::command]
pub async fn notif_pref_set(app: AppHandle, enabled: bool) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .set_bool_pref(PREF_ARRIVAL_BUBBLES, enabled)
            .map_err(|err| err.to_string())
    })
    .await
}

/// Langue de l'interface (PLAN-LANGUES, A15) : la preference se LIT au
/// demarrage — `None` tant qu'elle n'a jamais ete posee, l'UI detecte
/// alors la langue du systeme et la pose apres la modale de migration.
/// Sonde en LECTURE SEULE, pas `Store::open` : cette commande part
/// AVANT `migration_check` (la langue se restaure avant le premier
/// rendu), et l'ouverture pleine payait l'adoption d'une base heritee
/// en silence — sans modale, contre l'ADR 0012 (terrain 2026-08-15).
/// Et `hors_pompe` quand meme (ADR 0019) : la sonde porte un
/// busy_timeout de 30 s — une base rollback sous ecrivain ferait geler
/// la pompe autrement.
#[tauri::command]
pub async fn lang_get(app: AppHandle) -> Result<Option<String>, String> {
    hors_pompe(app, move |app| {
        Store::text_pref_readonly(&db_path(&app)?, PREF_LANG).map_err(|err| err.to_string())
    })
    .await
}

/// …et se POSE depuis Reglages > Affichage. En base (pas localStorage),
/// meme raison que les bulles : le shell composera les notifications
/// dans cette langue (E2).
#[tauri::command]
pub async fn lang_set(app: AppHandle, lang: String) -> Result<(), String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .set_text_pref(PREF_LANG, &lang)
            .map_err(|err| err.to_string())
    })
    .await
}

/// Les noms connus d'un lot d'adresses (entete du fil, PLAN-RETOURS-12
/// R5) : pure lecture de l'annuaire des correspondants, bornee a la
/// page de messages affichee. Une adresse inconnue est absente du
/// bilan — l'UI replie sur l'adresse nue.
#[tauri::command]
pub async fn noms_adresses(
    app: AppHandle,
    addresses: Vec<String>,
) -> Result<std::collections::HashMap<String, String>, String> {
    hors_pompe(app, move |app| {
        let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
        store
            .noms_adresses(&addresses)
            .map_err(|err| err.to_string())
    })
    .await
}

/// Telecharge, verifie la signature, lance l'installateur, et NE QUITTE
/// QUE SI ce lancement a reussi.
///
/// Le telechargement reste au plugin : la verification minisign est sur
/// ce chemin (updater.rs:712) et y demeure. Le LANCEMENT, lui, est a
/// nous (PLAN-SIGNATURE E4, D4) : le `install()` du plugin appelle
/// `ShellExecuteW` sans lire son retour puis sort par `exit(0)` — un
/// refus de Windows (Smart App Control, constat terrain 2026-08-26)
/// fermait l'application sans un mot et sans rien installer. Ici le
/// refus remonte au bandeau (`erreur.maj`), qui se rearme.
///
/// La base ne bouge pas de `%APPDATA%` (NSIS, pas MSIX — ADR 0013) :
/// une mise a jour ne peut pas orpheliner les messages.
#[tauri::command]
pub async fn update_install(app: AppHandle, version: String) -> Result<(), String> {
    // Meme garde d'isolation que `update_check` (passation §7.5) : un
    // test ne telecharge ni ne lance JAMAIS rien.
    if std::env::var("WIND_DB_PATH").is_ok() {
        return Err("mise à jour indisponible en test".to_string());
    }
    // Une installation a la fois, toutes surfaces confondues (bandeau
    // ET Reglages) : la seconde ecrirait le meme temoin et doublerait
    // le lancement.
    if MAJ_EN_COURS.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err("une installation est déjà en cours".to_string());
    }
    let resultat = telecharger_et_lancer(app, version).await;
    // Sur succes l'application quitte : on ne repasse ici qu'en echec.
    MAJ_EN_COURS.store(false, std::sync::atomic::Ordering::SeqCst);
    resultat
}

/// L'installation ne se double pas (bandeau + Réglages sont deux
/// surfaces pour la même action) — le drapeau se libère sur échec,
/// et n'a plus d'importance sur succès : l'application quitte.
static MAJ_EN_COURS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

async fn telecharger_et_lancer(app: AppHandle, version: String) -> Result<(), String> {
    // Instrumentation (PLAN-RETOURS-12 R2, decision D1) : la taille du
    // paquet est mesuree PLATE sur 12 releases (±1 % depuis la 0.7.0) —
    // si le bandeau « Téléchargement et installation… » dure, le temps
    // part ici : reseau (CDN GitHub), ecriture, ou scan
    // antivirus/verdict SAC au spawn. Chaque etape se trace sur stderr
    // ET dans `maj.log` a cote de la base (`trace_maj`) : la mesure se
    // lit apres coup, quel que soit le lancement.
    let dossier_trace = app.path().app_data_dir().ok();
    trace_maj(
        dossier_trace.as_deref(),
        &format!(
            "maj : {} -> {version} : installation demandee",
            app.package_info().version
        ),
    );
    let chrono = std::time::Instant::now();
    let update = updater_wind(&app)?
        .check()
        .await
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "aucune mise à jour à installer".to_string())?;
    trace_maj(
        dossier_trace.as_deref(),
        &format!(
            "maj : manifeste verifie en {} ms",
            chrono.elapsed().as_millis()
        ),
    );
    // Le manifeste peut avoir bouge entre le bandeau et le clic : on
    // n'installe que la version ANNONCEE — jamais une autre en silence.
    // L'UI re-verifie sur cet echec et redit la version neuve.
    if update.version != version {
        return Err(format!(
            "la version proposée a changé ({version} → {}) ; vérifie à nouveau",
            update.version
        ));
    }
    let depart_telechargement = std::time::Instant::now();
    let octets = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|err| err.to_string())?;
    trace_maj(
        dossier_trace.as_deref(),
        &format!(
            "maj : {} octets telecharges en {} ms",
            octets.len(),
            depart_telechargement.elapsed().as_millis()
        ),
    );
    // Filet de format : le plugin sniffait zip/exe/msi (extract,
    // updater.rs:882) ; l'artefact de Wind est l'exe NSIS nu
    // (createUpdaterArtifacts: true, cible nsis seule). Tout autre
    // contenu signe partirait en erreur Windows cryptique au spawn.
    if !octets.starts_with(b"MZ") {
        return Err("le paquet téléchargé n'est pas un exécutable Windows".to_string());
    }
    // Ecriture (~6 Mo) et CreateProcess (scan antivirus synchrone
    // possible) sont bloquants : hors de la pompe (ADR 0019), comme
    // toute commande qui touche un fichier.
    hors_pompe(app, move |app| {
        // Repertoire NEUF par tentative — le regime du tempdir aleatoire
        // du plugin : pas de collision avec un installateur fantome
        // d'une tentative precedente, pas de chemin devinable longtemps
        // a l'avance entre l'ecriture et le lancement.
        let horodatage = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_nanos();
        let dossier =
            std::env::temp_dir().join(format!("wind-maj-{}-{horodatage}", std::process::id()));
        std::fs::create_dir_all(&dossier)
            .map_err(|err| format!("préparation du répertoire ({}) : {err}", dossier.display()))?;
        let temoin = dossier.join(format!("Wind_{}_maj-setup.exe", update.version));
        let depart_ecriture = std::time::Instant::now();
        std::fs::write(&temoin, &octets)
            .map_err(|err| format!("écriture de l'installateur ({}) : {err}", temoin.display()))?;
        trace_maj(
            dossier_trace.as_deref(),
            &format!(
                "maj : installateur ecrit en {} ms",
                depart_ecriture.elapsed().as_millis()
            ),
        );
        // Le spawn porte le scan antivirus synchrone et le verdict cloud
        // Smart App Control (par binaire) : c'est le suspect n°1 du
        // bandeau qui dure — la mesure le dira.
        let depart_spawn = std::time::Instant::now();
        commande_installateur(&temoin)
            .spawn()
            .map_err(|err| format!("lancement de l'installateur refusé par Windows : {err}"))?;
        trace_maj(
            dossier_trace.as_deref(),
            &format!(
                "maj : installateur lance en {} ms",
                depart_spawn.elapsed().as_millis()
            ),
        );
        // Lancement REUSSI seulement : l'installateur (mode /UPDATE)
        // attend la fin du processus pour remplacer le binaire, puis
        // /R relance Wind — la version neuve.
        app.exit(0);
        Ok(())
    })
    .await
}

/// L'updater de Wind — UNE construction pour les deux commandes.
/// Le plugin ne pose AUCUN timeout (`timeout: None`) : un transfert qui
/// cale rendrait `check` muet au démarrage et figerait le bandeau sur
/// « Installation… » pour toujours — les deux visages de la panne du
/// constat 2026-08-26. Dix minutes couvrent ~6 Mo sur un lien très
/// lent ; au-delà, l'échec remonte et se retente.
fn updater_wind(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    app.updater_builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|err| err.to_string())
}

/// Trace une etape de mise a jour : sur stderr (visible via
/// `lancer-wind.ps1`) ET en append date dans `maj.log`, a cote de la
/// base. L'app fenetree n'a pas de stderr : trois MAJ acceptees
/// (0.13.0 → 0.15.0) sont passees sans qu'aucune mesure ne survive —
/// le fichier rend la trace lisible APRES COUP, quel que soit le
/// lancement (constat terrain 2026-08-30). Quelques dizaines d'octets
/// en append, cinq fois par MAJ : rien de commun avec l'ecriture de
/// l'installateur (~6 Mo) que l'ADR 0019 envoie hors pompe. Toute
/// erreur s'ignore — la trace ne fait jamais echouer une installation.
fn trace_maj(dossier: Option<&Path>, ligne: &str) {
    eprintln!("{ligne}");
    let Some(dossier) = dossier else { return };
    let _ = std::fs::create_dir_all(dossier);
    let datee = format!(
        "{} {ligne}\n",
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ")
    );
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dossier.join("maj.log"))
        .and_then(|mut fichier| std::io::Write::write_all(&mut fichier, datee.as_bytes()));
}

/// L'invocation de l'installateur NSIS — la decision pure, figee par le
/// test `l_installateur_est_invoque_en_passif_relance_et_mise_a_jour`.
fn commande_installateur(temoin: &std::path::Path) -> std::process::Command {
    let mut commande = std::process::Command::new(temoin);
    commande.args(["/P", "/R", "/UPDATE"]);
    commande
}

#[cfg(test)]
mod tests {
    /// PLAN-AUDIT-V2 E8 : le chemin d'enregistrement d'une pièce vient de
    /// l'UI (le dialogue « Enregistrer sous ») ; on l'écrit avec des octets
    /// choisis par l'expéditeur. Défense en profondeur : absolu, sans
    /// remontée, dans un dossier qui existe.
    #[test]
    fn un_chemin_relatif_ou_a_remontee_est_refuse() {
        assert!(super::chemin_de_sortie("piece.pdf").is_err());
        assert!(super::chemin_de_sortie("C:\\Users\\x\\..\\..\\Windows\\piece.pdf").is_err());
        assert!(
            super::chemin_de_sortie("C:\\dossier-qui-n-existe-pas-du-tout\\piece.pdf").is_err()
        );
        let ici = std::env::temp_dir().join("piece.pdf");
        assert!(super::chemin_de_sortie(&ici.to_string_lossy()).is_ok());
    }

    use super::*;

    /// L'invocation de l'installateur (PLAN-SIGNATURE E4, D4) : le
    /// témoin lui-même, en mode passif (`/P`), relance de l'application
    /// après pose (`/R`), mode mise à jour (`/UPDATE`) — les arguments
    /// mêmes que le plugin construit pour `installMode` passif. Et
    /// JAMAIS de `/ARGS` : le plugin le fait suivre des arguments du
    /// binaire courant, Wind se lance sans argument — un `/ARGS` vide
    /// serait une hypothèse non mesurée sur le parseur NSIS.
    /// La mesure du bandeau de MAJ ne depend plus du lancement (constat
    /// terrain 2026-08-30 : trois MAJ acceptees sans capture — stderr
    /// d'une app fenetree est nul) : chaque etape s'append DATEE dans
    /// `maj.log`, lisible apres coup. Deux appels = deux lignes — le
    /// fichier s'append d'une MAJ a l'autre, il ne s'ecrase pas.
    #[test]
    fn la_trace_de_maj_survit_dans_maj_log_et_s_append() {
        let dossier = std::env::temp_dir().join(format!("wind-maj-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dossier);

        trace_maj(Some(&dossier), "maj : manifeste verifie en 42 ms");
        trace_maj(Some(&dossier), "maj : installateur lance en 7 ms");

        let contenu = std::fs::read_to_string(dossier.join("maj.log")).unwrap();
        let lignes: Vec<_> = contenu.lines().collect();
        assert_eq!(lignes.len(), 2, "chaque etape ajoute UNE ligne");
        assert!(lignes[0].ends_with("maj : manifeste verifie en 42 ms"));
        assert!(lignes[1].ends_with("maj : installateur lance en 7 ms"));
        // Datee : le fichier se relit des semaines plus tard, et les
        // MAJ successives s'y distinguent.
        assert!(
            lignes
                .iter()
                .all(|l| l.starts_with("20") && l.contains("Z maj : ")),
            "chaque ligne porte son horodatage UTC : {contenu:?}"
        );

        let _ = std::fs::remove_dir_all(&dossier);
    }

    #[test]
    fn l_installateur_est_invoque_en_passif_relance_et_mise_a_jour() {
        let temoin = std::path::Path::new("C:\\tmp\\Wind_0.10.2_x64-setup.exe");
        let commande = commande_installateur(temoin);
        assert_eq!(commande.get_program(), temoin.as_os_str());
        let arguments: Vec<_> = commande.get_args().collect();
        assert_eq!(arguments, ["/P", "/R", "/UPDATE"]);
    }

    /// La table de la retentative d'Envoyés (PLAN-REACTIVITE E2) : la
    /// copie asynchrone de Gmail peut suivre l'acceptation SMTP de
    /// quelques secondes — deux retentatives bornées, puis le silence
    /// (le cycle rattrapera). Un compteur à zéro n'existe pas par
    /// construction (la première tentative est la n°1) ; s'il arrivait,
    /// on s'arrête — jamais une boucle.
    #[test]
    fn la_retentative_est_bornee() {
        assert_eq!(retenter_apres(1), Some(Duration::from_secs(5)));
        assert_eq!(retenter_apres(2), Some(Duration::from_secs(15)));
        assert_eq!(retenter_apres(3), None);
        assert_eq!(retenter_apres(0), None);
        assert_eq!(retenter_apres(u32::MAX), None);
    }

    /// La table des corps à l'arrivée (PLAN-REACTIVITE E4, R-D2) : un
    /// lot courant emporte ses corps — la ligne naît avec son aperçu —,
    /// un lot qui déborde n'en emporte AUCUN : les lignes d'abord, la
    /// pompe fera les corps. Zéro arrivée = zéro corps, jamais un
    /// aller-retour pour rien.
    #[test]
    fn les_corps_suivent_le_lot_sans_le_retarder() {
        assert_eq!(corps_a_l_arrivee(0), 0);
        assert_eq!(corps_a_l_arrivee(1), 1);
        assert_eq!(
            corps_a_l_arrivee(CORPS_A_L_ARRIVEE_MAX),
            CORPS_A_L_ARRIVEE_MAX
        );
        assert_eq!(corps_a_l_arrivee(CORPS_A_L_ARRIVEE_MAX + 1), 0);
        assert_eq!(corps_a_l_arrivee(usize::MAX), 0);
    }

    /// La table du recul (complément P0) : rien avant deux échecs — la
    /// cadence de 5 min est déjà une politesse —, puis le délai double,
    /// plafonné à l'heure. Un débordement d'échecs (compteur fou) ne
    /// doit jamais faire paniquer le décalage de bits.
    #[test]
    fn le_recul_double_puis_plafonne() {
        assert_eq!(attente_apres_echecs(0), Duration::ZERO);
        assert_eq!(attente_apres_echecs(1), Duration::ZERO);
        assert_eq!(attente_apres_echecs(2), Duration::from_secs(600));
        assert_eq!(attente_apres_echecs(3), Duration::from_secs(1200));
        assert_eq!(attente_apres_echecs(4), Duration::from_secs(2400));
        assert_eq!(attente_apres_echecs(5), Duration::from_secs(3600));
        assert_eq!(attente_apres_echecs(u32::MAX), Duration::from_secs(3600));
    }

    /// Le cycle de vie complet : deux échecs posent un recul qui court,
    /// un succès l'efface entièrement — le compte repart confiant.
    #[test]
    fn un_succes_efface_le_recul() {
        let reculs = Mutex::new(HashMap::new());
        assert_eq!(recul_en_cours(&reculs, "a@exemple.fr"), None);

        noter_issue(&reculs, "a@exemple.fr", false);
        assert_eq!(
            recul_en_cours(&reculs, "a@exemple.fr"),
            None,
            "un seul échec ne recule pas : la cadence normale suffit"
        );

        noter_issue(&reculs, "a@exemple.fr", false);
        assert!(
            recul_en_cours(&reculs, "a@exemple.fr").is_some(),
            "deux échecs consécutifs posent le recul"
        );
        // L'autre compte n'est pas touché : le recul est PAR compte.
        assert_eq!(recul_en_cours(&reculs, "b@exemple.fr"), None);

        noter_issue(&reculs, "a@exemple.fr", true);
        assert_eq!(recul_en_cours(&reculs, "a@exemple.fr"), None);
    }

    /// Le type d'une pièce à joindre se déduit de l'extension, sans
    /// sensibilité à la casse ; l'inconnu part en flux d'octets — un
    /// en-tête honnête, jamais une décision.
    #[test]
    fn mime_for_name_follows_the_extension_and_falls_back_generic() {
        assert_eq!(mime_for_name("devis.pdf"), "application/pdf");
        assert_eq!(mime_for_name("PHOTO.JPG"), "image/jpeg");
        assert_eq!(mime_for_name("archive.tar.gz"), "application/octet-stream");
        assert_eq!(mime_for_name("notes.txt"), "text/plain");
        assert_eq!(mime_for_name("sans-extension"), "application/octet-stream");
        assert_eq!(mime_for_name(""), "application/octet-stream");
    }

    /// Un nom de pièce jointe est une chaîne choisie par l'EXPÉDITEUR.
    /// Écrit tel quel, il permet une écriture arbitraire de fichier
    /// déclenchée par un simple clic sur un message reçu. Ces cas ne
    /// sont pas théoriques : ce sont ceux des archives d'exploitation
    /// de clients mail.
    #[test]
    fn a_hostile_attachment_name_can_never_escape_its_folder() {
        assert_eq!(
            safe_file_name("../../.ssh/authorized_keys"),
            "authorized_keys"
        );
        assert_eq!(
            safe_file_name(r"..\..\Windows\System32\evil.dll"),
            "evil.dll"
        );
        assert_eq!(safe_file_name(r"C:\Windows\notepad.exe"), "notepad.exe");
        assert_eq!(safe_file_name("/etc/passwd"), "passwd");
        assert_eq!(safe_file_name(".."), "piece-jointe");
        assert_eq!(safe_file_name("/"), "piece-jointe");
        assert_eq!(safe_file_name(""), "piece-jointe");
    }

    /// Windows refuse ces noms quelle que soit l'extension : sans repli,
    /// l'enregistrement échouerait avec une erreur incompréhensible.
    #[test]
    fn windows_device_names_fall_back() {
        assert_eq!(safe_file_name("CON"), "piece-jointe");
        assert_eq!(safe_file_name("nul.txt"), "piece-jointe");
        assert_eq!(safe_file_name("COM1.pdf"), "piece-jointe");
        assert_eq!(safe_file_name("LPT9"), "piece-jointe");
        // Ni réservé, ni piégeux : doit passer tel quel.
        assert_eq!(safe_file_name("COM0.pdf"), "COM0.pdf");
        assert_eq!(safe_file_name("console.log"), "console.log");
    }

    /// Un nom légitime, même accentué, doit traverser INTACT — un filtre
    /// qui mutile les noms normaux serait payé tous les jours.
    #[test]
    fn a_legitimate_name_passes_through_untouched() {
        assert_eq!(safe_file_name("facture.pdf"), "facture.pdf");
        assert_eq!(safe_file_name("résumé 2026.docx"), "résumé 2026.docx");
        assert_eq!(
            safe_file_name("rapport-final_v2.xlsx"),
            "rapport-final_v2.xlsx"
        );
    }

    #[test]
    fn control_characters_and_wildcards_are_neutralised() {
        assert_eq!(safe_file_name("a<b>c.txt"), "a_b_c.txt");
        assert_eq!(safe_file_name("fac\u{7}ture?.pdf"), "fac_ture_.pdf");
    }

    /// Enregistrer deux fois la même pièce ne doit jamais écraser le
    /// premier fichier — la perte serait silencieuse.
    #[test]
    fn a_second_save_never_overwrites_the_first() {
        let dir = std::env::temp_dir().join(format!("wind-pj-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let _ = std::fs::remove_file(dir.join("facture.pdf"));

        let first = unique_path(&dir, "facture.pdf");
        assert_eq!(first.file_name().unwrap(), "facture.pdf");
        std::fs::write(&first, b"x").unwrap();

        let second = unique_path(&dir, "facture.pdf");
        assert_eq!(second.file_name().unwrap(), "facture (2).pdf");

        std::fs::remove_file(&first).unwrap();
        let _ = std::fs::remove_dir(&dir);
    }

    /// L'adresse déclarée devient la clé du compte ET l'identifiant
    /// XOAUTH2 : elle n'est vérifiable par personne d'autre avant le
    /// consentement. Un filtre minimal évite le compte fantôme, sans
    /// prétendre valider la RFC 5322 — le fournisseur tranchera.
    #[test]
    fn declared_address_must_be_plausible() {
        assert!(is_plausible_address("moi@exemple.fr"));
        assert!(is_plausible_address("prenom.nom@outlook.com"));

        assert!(!is_plausible_address(""), "vide");
        assert!(!is_plausible_address("moi"), "sans arobase");
        assert!(!is_plausible_address("@exemple.fr"), "sans partie locale");
        assert!(!is_plausible_address("moi@"), "sans domaine");
        assert!(!is_plausible_address("moi@exemple"), "domaine sans point");
        assert!(
            !is_plausible_address("moi@.fr"),
            "domaine commençant par un point"
        );
    }

    /// R1 (PLAN-RETOURS-8) : le repère d'un compte n'admet que le jeu
    /// dédié (D2, A3 « une icône, un sens ») et le nuancier mesuré
    /// (D1) — tout le reste est refusé, y compris une valeur corrompue
    /// relue de la base.
    #[test]
    fn repere_valide_est_une_allowlist() {
        assert!(repere_valide("home", "rouge"));
        assert!(repere_valide("music_note", "brun"));
        assert!(
            !repere_valide("download", "rouge"),
            "glyphe du produit, hors jeu dédié (A3)"
        );
        assert!(!repere_valide("home", "turquoise"), "teinte inconnue");
        assert!(!repere_valide("", ""));
    }

    /// Jamais posé -> None ; posé -> relu ; retiré -> None (les clés se
    /// vident, patron signature) ; corrompu en base -> None (l'allowlist
    /// tient aussi au retour).
    #[test]
    fn repere_absent_pose_retire_corrompu() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(repere_de(&store, 1).unwrap(), None);

        poser_repere(&mut store, 1, Some(("work", "bleu"))).unwrap();
        assert_eq!(
            repere_de(&store, 1).unwrap(),
            Some(("work".to_string(), "bleu".to_string()))
        );

        poser_repere(&mut store, 1, None).unwrap();
        assert_eq!(repere_de(&store, 1).unwrap(), None);

        store.set_text_pref("repere_icone.1", "delete").unwrap();
        store.set_text_pref("repere_teinte.1", "rouge").unwrap();
        assert_eq!(repere_de(&store, 1).unwrap(), None);
    }

    /// PLAN-RETOURS-9 (D3) : la décision pure du nom personnalisé.
    /// Espaces rognés, vide (ou blanc) = retiré, au-delà de 60
    /// caractères refusé — jamais tronqué en silence.
    #[test]
    fn nom_normalise_rogne_vide_et_plafonne() {
        assert_eq!(nom_normalise("  Boulot  "), Ok(Some("Boulot".to_string())));
        assert_eq!(nom_normalise(""), Ok(None));
        assert_eq!(nom_normalise("   "), Ok(None));
        assert_eq!(nom_normalise(&"x".repeat(60)), Ok(Some("x".repeat(60))));
        assert!(nom_normalise(&"x".repeat(61)).is_err());
    }

    /// Jamais posé -> None ; posé -> relu ; vidé -> None (la clé se
    /// vide, patron repère/signature) ; une coquille blanche en base ne
    /// sort jamais vers l'UI.
    #[test]
    fn nom_compte_absent_pose_retire() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(nom_de(&store, 1).unwrap(), None);

        poser_nom(&mut store, 1, Some("Boulot")).unwrap();
        assert_eq!(nom_de(&store, 1).unwrap(), Some("Boulot".to_string()));

        poser_nom(&mut store, 1, None).unwrap();
        assert_eq!(nom_de(&store, 1).unwrap(), None);

        store.set_text_pref("nom_compte.1", "   ").unwrap();
        assert_eq!(nom_de(&store, 1).unwrap(), None);
    }

    /// PLAN-AUDIT-V1 E5 : le vol d'une passe d'apres-geste est une GARDE.
    /// Avant, un `?` entre la prise et la liberation laissait `en_vol`
    /// leve a vie : toute passe suivante du compte etait absorbee jusqu'au
    /// redemarrage. RED sans enseignement (le comportement est celui du
    /// `Drop`) — le test dit le contrat.
    #[test]
    fn le_vol_retombe_quand_la_garde_est_relachee_meme_par_une_sortie_precoce() {
        let passes = Mutex::new(HashMap::<String, VolPasse>::new());
        let en_vol = |passes: &Mutex<HashMap<String, VolPasse>>| {
            passes
                .lock()
                .unwrap()
                .get("a@x.fr")
                .map(|v| v.en_vol)
                .unwrap_or(false)
        };

        let sortie_precoce = |passes: &Mutex<HashMap<String, VolPasse>>| -> Result<(), String> {
            let _vol = VolGarde::prendre(passes, "a@x.fr").expect("premiere prise");
            assert!(en_vol(passes));
            // Une seconde demande pendant le vol est absorbee et notee.
            assert!(VolGarde::prendre(passes, "a@x.fr").is_none());
            assert!(passes.lock().unwrap()["a@x.fr"].redemande);
            Err("panne au milieu de la passe".to_string())?;
            Ok(())
        };
        assert!(sortie_precoce(&passes).is_err());
        assert!(!en_vol(&passes), "la sortie precoce a relache le vol");

        // La redemande notee pendant le vol se consomme UNE fois.
        let vol = VolGarde::prendre(&passes, "a@x.fr").expect("le vol est libre");
        assert!(vol.redemande_consommee());
        assert!(!vol.redemande_consommee());
        drop(vol);
        assert!(!en_vol(&passes));
    }
}
