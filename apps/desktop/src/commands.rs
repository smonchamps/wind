//! Tauri commands: the gateway between the UI and the core.
//!
//! Multi-account (Phase 3): a message's identity is `(account, uid)` —
//! a UID alone is no longer enough. Every network operation goes through
//! the connection of ITS account; the loops (sync, outbox flush, drafts)
//! aggregate the connected accounts. Blocking work (OAuth, IMAP, SMTP)
//! goes through `spawn_blocking` so the window never freezes.

use std::collections::{BTreeMap, HashMap};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::account_work::{AccountWork, Lease, Ticket};
use crate::adoption::{Blocking, adopted_db};
use mail_auth::{AccountSession, Authenticated, Authenticator, GenericCredentials};
use mail_core::AccountConfig;
use mail_core::{Action, MailServer, OutboxState, Store, SyncEngine};
use mail_imap::ImapServer;
use mail_smtp::SmtpMailer;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::fault::CommandError;
use crate::{AppState, PassFlight};

/// INBOX's wire name — aliases `mail_core::cycle::INBOX` (PLAN-AUDIT-V3
/// E4: the poll policy moved into the core, this name is now ITS to
/// own; the shell, including `watcher.rs`, keeps using it through this
/// one alias so the two never drift apart).
pub(crate) const MAILBOX: &str = mail_core::cycle::INBOX;
const LIST_LIMIT_MAX: usize = 500;
const SEARCH_LIMIT: usize = 100;
/// Bodies backfilled per call, across all accounts. Capping the batch makes
/// interruption free: the UI simply stops calling back.
const BACKFILL_BUDGET: usize = 200;

#[derive(Serialize)]
pub struct AccountInfo {
    pub id: i64,
    pub email: String,
}

#[derive(Serialize)]
pub struct SyncSummary {
    /// Accounts synchronized successfully.
    pub accounts: usize,
    /// Accounts whose ENTIRE poll failed (E3, so-called partial failure):
    /// `errors` isn't enough to count them — it also carries the
    /// best-effort incidents of the accounts that succeeded.
    pub accounts_failed: usize,
    pub fetched: usize,
    pub deleted: usize,
    pub replayed: usize,
    pub elapsed_ms: u64,
    /// Failures per account — the other accounts aren't blocked.
    pub errors: Vec<String>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct MessageVersion {
    pub mailbox_id: i64,
    pub uid_validity: u32,
}

impl MessageVersion {
    fn identity(self, account_id: i64, mailbox: &str) -> mail_core::MailboxIdentity {
        mail_core::MailboxIdentity {
            account_id,
            mailbox: mailbox.to_string(),
            mailbox_id: self.mailbox_id,
            uid_validity: self.uid_validity,
        }
    }
    fn verify(self, store: &Store, account_id: i64, mailbox: &str) -> Result<(), mail_core::Error> {
        store.verify_mailbox_identity(&self.identity(account_id, mailbox))
    }
}

#[derive(Serialize)]
pub struct MessageRow {
    pub version: MessageVersion,
    pub account_id: i64,
    pub account_email: String,
    /// The mailbox that contains this message. **Essential**: UIDs are
    /// assigned per mailbox and restart at 1, so the UID alone no longer
    /// identifies a message as soon as an account synchronizes two of
    /// them. Every UI action sends it back.
    pub mailbox: String,
    pub uid: u32,
    pub subject: String,
    pub sender: String,
    pub date: String,
    pub seen: bool,
    pub flagged: bool,
    pub has_attachment: bool,
    /// The conversation this row belongs to — it's what the rest of the
    /// exchange is requested through.
    pub thread_id: Option<i64>,
    /// Messages of the conversation present in the mailbox. 1 = isolated.
    pub thread_size: u32,
    /// Unread count of the conversation: it's THIS that decides the bold,
    /// not the state of the single message shown.
    pub thread_unseen: u32,
    /// Unix seconds of the message — v2 formats the time client-side
    /// (“09:12”, “Yesterday”, “Aug 5”); `date` stays the raw string that
    /// v1 shows as is. 0 = unknown date.
    pub epoch: i64,
    /// HOW MANY attachments — the prototype's chip says “2 files”. 0 as
    /// long as the body hasn't been read.
    pub attachment_count: u32,
    /// The preview under the subject (screen 02 v2); `None` as long as
    /// the body hasn't been fetched or backfilled.
    pub preview: Option<String>,
    /// Raw sender address — the “From” line of screen 03 (`Name
    /// <address>`). `sender` stays the display string.
    pub sender_address: Option<String>,
    /// Raw To / Cc recipients (R4). In a sent folder — or for our own
    /// messages in a thread — the sender is SELF: it's the recipient
    /// that says who the message went to. Empty when the ENVELOPE didn't
    /// carry any (old sends not yet backfilled, received messages whose
    /// To wasn't stored).
    pub to_addrs: Vec<String>,
    pub cc_addrs: Vec<String>,
    /// R4 (PLAN-RETOURS-7): the row comes from the PINNED section of the
    /// Inbox (`pinned_rows`). Always false in the paginated flow — a
    /// pinned conversation is excluded from it (D5). The open thread
    /// SEEDS its pin state from this field (fil.svelte.js): it's what
    /// dresses “Pin”/“Unpin” without a round trip.
    pub pinned: bool,
    /// E5: is the thread SET ASIDE — seeded by the only source that
    /// knows (the pile): a row from an organized view is NEVER set
    /// aside (the core excludes it), a pile card always is. Same rule
    /// as `pinned` (2026-08-21 review: never a round trip on open).
    pub aside: bool,
    /// The thread's invitation (field R10/R11, PLAN-INVITATIONS): the
    /// chip row states it (reply given, cancellation) and carries the
    /// three gestures — reply without opening. `None` = ordinary row.
    pub invitation: Option<InvitationRowPayload>,
}

/// A row's invitation badge — the key to reply FROM the list targets the
/// invitation MESSAGE (not the thread head).
#[derive(Serialize)]
pub struct InvitationRowPayload {
    pub revision: i64,
    pub version: MessageVersion,
    pub mailbox: String,
    pub uid: u32,
    /// The meeting title — the reply's subject is built from it, never
    /// from the head's subject (“Re: …”).
    pub title: String,
    /// `accepted` | `tentative` | `declined` (wire words) — the row's chip.
    pub reply: Option<String>,
    pub cancelled: bool,
    pub can_reply: bool,
}

/// Outcome of a reconnection: what came back, and WHY the rest didn't.
/// A silent account is worse than an account in error — without this
/// list, the user sees a missing badge with no idea what to do.
#[derive(Serialize)]
pub struct ConnectReport {
    pub accounts: Vec<AccountInfo>,
    pub problems: Vec<String>,
}

/// Silent connection of ALL accounts in the registry. Empty registry
/// (database migrated from Phase 2): the legacy vault entry can reveal
/// the account — it is then migrated and the pending account claimed.
///
/// **Each account is isolated**: the missing configuration or expired
/// token of one must never prevent the others from coming back. Same
/// principle as [`sync_inbox`].
#[tauri::command]
pub async fn connect_accounts(app: AppHandle) -> Result<ConnectReport, CommandError> {
    let _registration = app.state::<AppState>().account_work.registration()?;
    // E2E hook: fake accounts (emails separated by commas), tokens
    // invalid by construction — offline guaranteed.
    if let Ok(list) = std::env::var("WIND_E2E_ACCOUNT") {
        return store_off_pump(app, move |app, store| {
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
                        access_token: "invalid-e2e-token".to_string(),
                    }),
                );
                infos.push(AccountInfo {
                    id,
                    email: email.to_string(),
                });
            }
            // E4: the fixture's accounts also get their watchers — same
            // path as the real ones, their connection failures are
            // bounded (P0 timeouts) and spaced out (doubling delay).
            crate::watcher::reconcile(app);
            Ok(ConnectReport {
                accounts: infos,
                problems: Vec::new(),
            })
        })
        .await;
    }

    let (path, accounts, legacy) = off_pump(app.clone(), |app| {
        let path = adopted_db(&app)?;
        let store = Store::open(&path).map_err(|err| err.to_string())?;
        let state = app.state::<AppState>();
        let known = store.accounts().map_err(|err| err.to_string())?;
        let legacy = known.is_empty();
        let jobs = known
            .into_iter()
            .filter_map(|account| {
                state
                    .account_work
                    .capture(account.id)
                    .ok()
                    .map(|ticket| (account, ticket))
            })
            .collect::<Vec<_>>();
        Ok::<_, String>((path, jobs, legacy))
    })
    .await?;

    let path_for_spawn = path.clone();
    let (connected, mut problems) = tauri::async_runtime::spawn_blocking(move || {
        let mut list = Vec::new();
        let mut problems: Vec<String> = Vec::new();
        for (account, ticket) in accounts {
            let Ok(_lease) = ticket.lease() else { continue };
            // A provider's missing OAuth configuration concerns ONLY its
            // accounts: it must never prevent any other one from coming
            // back.
            let outcome = match account.provider.as_str() {
                "imap" => connect_generic(&path_for_spawn, &account),
                kind => match mail_auth::for_account_kind(kind) {
                    Some(provider) => Authenticator::from_env(provider)
                        .and_then(|auth| auth.authenticate_silent(&account.email))
                        .map(|session| Some(AccountSession::OAuth(session)))
                        .map_err(|err| err.to_string()),
                    None => Err(format!("unknown provider: {kind}")),
                },
            };
            match outcome {
                Ok(Some(session)) => list.push((session, Some(ticket))),
                Ok(None) => problems.push(format!(
                    "{}: incomplete server configuration",
                    account.email
                )),
                Err(reason) => problems.push(format!("{}: {reason}", account.email)),
            }
        }
        // Legacy Phase 2 fallback: a Gmail account without an explicit
        // provider. Specific to Google — Phase 2 only knew that one.
        if legacy
            && let Ok(auth) = Authenticator::google_from_env()
            && let Ok(account) = auth.authenticate_silent_legacy()
        {
            list.push((AccountSession::OAuth(account), None));
        }
        (list, problems)
    })
    .await
    .map_err(|err| err.to_string())?;

    // E5: writing the accounts and setting the sessions under the
    // commands' lock — never again on the bare async worker.
    store_off_pump(app, move |app, store| {
        let state = app.state::<AppState>();
        let mut infos = Vec::new();
        for (session, ticket) in connected {
            if ticket
                .as_ref()
                .is_some_and(|ticket| !state.account_work.is_current(ticket))
            {
                continue;
            }
            let email = session.email().to_string();
            let provider = match &session {
                AccountSession::OAuth(auth) => auth.provider.account_kind,
                AccountSession::Generic(_) => "imap",
            };
            if let Some(known) = store
                .accounts()?
                .into_iter()
                .find(|known| known.email == email)
                && state.account_work.capture(known.id).is_err()
            {
                continue;
            }
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
        // E4: one IDLE watcher per reconnected account (ADR 0018) —
        // started HERE, after the sessions are set, never at boot
        // (nothing to watch without a session).
        crate::watcher::reconcile(app);
        Ok(ConnectReport {
            accounts: infos,
            problems,
        })
    })
    .await
}

/// Reconnects a generic IMAP account from the vault and its
/// configuration. `Ok(None)`: the server configuration is incomplete.
fn connect_generic(
    db_path: &Path,
    account: &mail_core::Account,
) -> Result<Option<AccountSession>, String> {
    let config = Store::open(db_path)
        .map_err(|err| err.to_string())?
        .account_config(account.id)
        .map_err(|err| err.to_string())?;
    let password =
        mail_auth::fetch_generic_password_slot(&account.email, config.credential_slot.as_deref())
            .map_err(|err| err.to_string())?;
    Ok(build_generic_session(&account.email, &password, &config))
}

#[derive(Serialize)]
pub struct ConsentStatus {
    url: String,
    manual: bool,
}

#[tauri::command]
pub async fn oauth_begin(state: State<'_, AppState>) -> Result<String, CommandError> {
    state.consent.begin().map_err(Into::into)
}

#[tauri::command]
pub async fn oauth_status(
    state: State<'_, AppState>,
    flow_id: String,
) -> Result<Option<ConsentStatus>, CommandError> {
    Ok(state.consent.status(&flow_id)?.map(|link| ConsentStatus {
        url: link.url,
        manual: link.manual,
    }))
}

#[tauri::command]
pub async fn oauth_cancel(
    state: State<'_, AppState>,
    flow_id: String,
) -> Result<bool, CommandError> {
    Ok(state.consent.cancel(&flow_id))
}

/// Adds a Gmail account — full browser flow, repeatable. Google delivers
/// the account's identity: nothing to declare.
#[tauri::command]
pub async fn add_account(
    app: AppHandle,
    state: State<'_, AppState>,
    horizon: Option<String>,
    flow_id: String,
) -> Result<AccountInfo, CommandError> {
    let _ = state;
    add_oauth_account(app, &mail_auth::GOOGLE, None, horizon, flow_id).await
}

/// Adds a Microsoft 365 / Outlook.com account.
///
/// The address is TYPED IN: within the scope of scopes measured by the
/// spike, Microsoft doesn't deliver the account's identity
/// ([`mail_auth::Identity`]).
#[tauri::command]
pub async fn add_microsoft_account(
    app: AppHandle,
    state: State<'_, AppState>,
    email: String,
    horizon: Option<String>,
    flow_id: String,
) -> Result<AccountInfo, CommandError> {
    let email = email.trim().to_string();
    // Validation at the boundary: the declared address becomes the
    // account's key AND the XOAUTH2 identifier. An empty entry would
    // produce a ghost account nothing could ever reach again.
    if !is_plausible_address(&email) {
        return Err("invalid address: enter the account's full address".into());
    }
    let _ = state;
    add_oauth_account(app, &mail_auth::MICROSOFT, Some(email), horizon, flow_id).await
}

/// The common trunk of OAuth2 additions: browser consent, then
/// registering the account under the key of ITS provider.
async fn add_oauth_account(
    app: AppHandle,
    provider: &'static mail_auth::Provider,
    declared_email: Option<String>,
    horizon: Option<String>,
    flow_id: String,
) -> Result<AccountInfo, CommandError> {
    let consent = app.state::<AppState>().consent.claim(&flow_id)?;
    let control = consent.control.clone();
    let _registration = app.state::<AppState>().account_work.registration()?;
    // Validation at the boundary, BEFORE the browser flow: refusing an
    // unreadable horizon after consent would leave an account created
    // under a gesture that failed.
    validate_horizon(horizon.as_deref())?;
    let account = tauri::async_runtime::spawn_blocking(move || {
        Authenticator::from_env(provider)
            .map_err(|err| err.to_string())?
            .authenticate_interactive_controlled(declared_email.as_deref(), None, &control)
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())??;

    store_off_pump(app, move |app, store| {
        let state = app.state::<AppState>();
        if let Some(known) = store
            .accounts()?
            .into_iter()
            .find(|known| known.email == account.email)
        {
            state.account_work.capture(known.id)?;
        }
        let id = store.adopt_or_create_account(&account.email, account.provider.account_kind)?;
        write_horizon_on_first_add(store, id, horizon.as_deref())?;
        let info = AccountInfo {
            id,
            email: account.email.clone(),
        };
        lock_accounts(&state)?.insert(account.email.clone(), AccountSession::OAuth(account));
        crate::watcher::reconcile(app);
        Ok(info)
    })
    .await
}

/// Reconnects a registry account whose token is dead — field finding of
/// 2026-08-20: `invalid_grant` (expired or revoked token) left the user
/// STRANDED, no gesture relaunched the consent flow. Same browser flow
/// as adding an account, on the EXISTING row: nothing is re-synchronized,
/// nothing is lost.
///
/// Identity guard: consent must come back with the address of the
/// targeted account. Google picks the identity at the browser — a
/// different choice must not silently connect ANOTHER account under the
/// “reconnect X” gesture; Microsoft receives the declared address, the
/// guard is structural there. A generic IMAP account has no token:
/// a plain refusal with what to do instead.
#[tauri::command]
pub async fn reconnect_account(
    app: AppHandle,
    account_id: i64,
    flow_id: String,
) -> Result<AccountInfo, CommandError> {
    let consent = app.state::<AppState>().consent.claim(&flow_id)?;
    let control = consent.control.clone();
    let _registration = app.state::<AppState>().account_work.registration()?;
    let (account, ticket, lease) = off_pump(app.clone(), move |app| {
        let store = Store::open(&adopted_db(&app)?).map_err(|err| err.to_string())?;
        let account = store
            .accounts()
            .map_err(|err| err.to_string())?
            .into_iter()
            .find(|account| account.id == account_id)
            .ok_or_else(|| "unknown account".to_string())?;
        let ticket = app.state::<AppState>().account_work.capture(account_id)?;
        let lease = ticket.lease()?;
        Ok::<_, String>((account, ticket, lease))
    })
    .await?;
    if account.provider == "imap" {
        return Err(
            "generic IMAP account: remove then re-add the account to re-enter the password".into(),
        );
    }
    let provider = mail_auth::for_account_kind(&account.provider)
        .ok_or_else(|| format!("unknown provider: {}", account.provider))?;
    // Google delivers the identity; Microsoft requires the declared address.
    let declared =
        (provider.account_kind != mail_auth::GOOGLE.account_kind).then(|| account.email.clone());
    let expected = account.email.clone();
    let session = tauri::async_runtime::spawn_blocking(move || {
        Authenticator::from_env(provider)
            .map_err(|err| err.to_string())?
            .authenticate_interactive_controlled(declared.as_deref(), Some(&expected), &control)
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())??;
    off_pump(app, move |app| {
        let _lease = lease;
        let state = app.state::<AppState>();
        if !state.account_work.is_current(&ticket) {
            return Err("account changed during reconnection".into());
        }
        lock_accounts(&state)?.insert(account.email.clone(), AccountSession::OAuth(session));
        // Same rule as the generic repair: a renewed connection releases the
        // refusals waiting for a manual retry without erasing the diagnostic.
        Store::open(&adopted_db(&app)?)
            .and_then(|store| store.retry_operations(account.id))
            .map_err(|err| err.to_string())?;
        // The account gets its IDLE watcher back without waiting for a restart.
        crate::watcher::reconcile(&app);
        Ok(AccountInfo {
            id: account.id,
            email: account.email,
        })
    })
    .await
}

/// Minimal address filter: what follows is checked by the provider
/// itself at consent time. We aren't trying to validate RFC 5322 here,
/// only to reject what manifestly cannot be an address.
fn is_plausible_address(email: &str) -> bool {
    match email.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty() && domain.contains('.') && !domain.starts_with('.')
        }
        None => false,
    }
}

/// Fields arrive from the UI in camelCase. Tauri only converts command
/// ARGUMENTS, not the fields of a nested struct: without this
/// `rename_all`, `imapHost` wouldn't find `imap_host`.
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

/// The command-side mirror of `Store::set_horizon_import`: reject a
/// value outside the vocabulary BEFORE any work (connection, consent).
fn validate_horizon(horizon: Option<&str>) -> Result<(), String> {
    let horizon = horizon.map(crate::wire::category_from_wire);
    match horizon.as_deref() {
        Some(h) if !mail_core::HORIZONS_IMPORT.contains(&h) => {
            Err(format!("unknown horizon: {h:?}"))
        }
        _ => Ok(()),
    }
}

/// The desk's horizon is only written on the FIRST add (2026-08-30
/// review): replaying the addition of an existing account (the adoption
/// path — a repair gesture) must not silently overwrite an already
/// chosen horizon (or the D4 deemed “everything”) with the picker's
/// default. Afterwards, the setting lives in Settings > Accounts (D3).
fn write_horizon_on_first_add(
    store: &Store,
    account_id: i64,
    horizon: Option<&str>,
) -> Result<(), String> {
    let Some(h) = horizon else { return Ok(()) };
    let already = store
        .text_pref(&format!("horizon_import.{account_id}"))
        .map_err(|err| err.to_string())?
        .is_some();
    if !already {
        store
            .set_horizon_import(account_id, &crate::wire::category_from_wire(h))
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn verify_generic_connections(
    imap: impl FnOnce() -> Result<(), String>,
    smtp: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let incoming = imap();
    let outgoing = smtp();
    let mut errors = Vec::new();
    if let Err(error) = incoming {
        errors.push(format!("IMAP: {error}"));
    }
    if let Err(error) = outgoing {
        errors.push(format!("SMTP: {error}"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn generic_credentials(input: GenericAccountInput) -> Result<GenericCredentials, String> {
    let email = input.email.trim().to_string();
    let username = input
        .username
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| email.clone())
        .trim()
        .to_string();
    if !is_plausible_address(&email)
        || input.imap_host.trim().is_empty()
        || input.smtp_host.trim().is_empty()
        || input.imap_port == 0
        || input.smtp_port == 0
    {
        return Err("enter the address, servers and valid ports".into());
    }
    Ok(GenericCredentials {
        email,
        username,
        password: input.password,
        imap_host: input.imap_host.trim().into(),
        imap_port: input.imap_port,
        smtp_host: input.smtp_host.trim().into(),
        smtp_port: input.smtp_port,
    })
}

fn probe_generic(creds: &GenericCredentials) -> Result<(), String> {
    verify_generic_connections(
        || {
            let server = mail_imap::ImapServer::connect_password(
                &creds.imap_host,
                creds.imap_port,
                &creds.username,
                &creds.password,
            )
            .map_err(|err| err.to_string())?;
            server.logout();
            Ok(())
        },
        || {
            SmtpMailer::connect_password(
                &creds.smtp_host,
                creds.smtp_port,
                &creds.username,
                &creds.password,
            )
            .map(|_| ())
            .map_err(|err| err.to_string())
        },
    )
}

fn publish_generic(
    store: &Store,
    target: Option<i64>,
    creds: &GenericCredentials,
    current_slot: Option<&str>,
    horizon: Option<&str>,
) -> Result<i64, CommandError> {
    let id =
        mail_auth::replace_generic_password(&creds.email, current_slot, &creds.password, |slot| {
            store
                .save_generic_connection(
                    target,
                    &creds.email,
                    &AccountConfig {
                        imap_host: Some(creds.imap_host.clone()),
                        imap_port: Some(creds.imap_port),
                        smtp_host: Some(creds.smtp_host.clone()),
                        smtp_port: Some(creds.smtp_port),
                        username: Some(creds.username.clone()),
                        credential_slot: Some(slot.into()),
                    },
                    horizon,
                )
                .map_err(|err| mail_auth::AuthError::Config(err.to_string()))
        })
        .map_err(|err| err.to_string())?;
    Ok(id)
}

/// Verify both protocols before staging a credential; publish its reference with SQL.
#[tauri::command]
pub async fn add_generic_account(
    app: AppHandle,
    input: GenericAccountInput,
    horizon: Option<String>,
) -> Result<AccountInfo, CommandError> {
    let _registration = app.state::<AppState>().account_work.registration()?;
    validate_horizon(horizon.as_deref())?;
    let creds = generic_credentials(input)?;
    if creds.password.is_empty() {
        return Err("enter the account password".into());
    }
    let creds = tauri::async_runtime::spawn_blocking(move || {
        probe_generic(&creds)?;
        Ok::<_, String>(creds)
    })
    .await
    .map_err(|err| err.to_string())??;
    store_off_pump(app, move |app, store| {
        let state = app.state::<AppState>();
        // Never stage into an existing account's active slot through the add door.
        if store
            .accounts()?
            .iter()
            .any(|account| account.email.eq_ignore_ascii_case(&creds.email))
        {
            return Err("account already exists; use its connection settings".into());
        }
        let mut accounts = lock_accounts(&state)?;
        let horizon = horizon.as_deref().map(crate::wire::category_from_wire);
        let id = publish_generic(store, None, &creds, None, horizon.as_deref())?;
        let info = AccountInfo {
            id,
            email: creds.email.clone(),
        };
        accounts.insert(creds.email.clone(), AccountSession::Generic(creds));
        drop(accounts);
        crate::watcher::reconcile(app);
        Ok(info)
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericConnectionSettings {
    email: String,
    username: String,
    imap_host: String,
    imap_port: u16,
    smtp_host: String,
    smtp_port: u16,
}

#[tauri::command]
pub async fn generic_connection_settings(
    app: AppHandle,
    account_id: i64,
) -> Result<Option<GenericConnectionSettings>, CommandError> {
    store_off_pump(app, move |_, store| {
        let account = store
            .accounts()?
            .into_iter()
            .find(|account| account.id == account_id)
            .ok_or("unknown account")?;
        if account.provider != "imap" {
            return Ok(None);
        }
        let config = store.account_config(account_id)?;
        Ok(Some(GenericConnectionSettings {
            email: account.email.clone(),
            username: config.username.unwrap_or(account.email),
            imap_host: config.imap_host.unwrap_or_default(),
            imap_port: config.imap_port.unwrap_or(993),
            smtp_host: config.smtp_host.unwrap_or_default(),
            smtp_port: config.smtp_port.unwrap_or(465),
        }))
    })
    .await
}

fn retire_generic_connection(
    registry: &crate::account_work::Registry,
    ticket: &Ticket,
    lease: Lease,
) -> Result<crate::account_work::Retirement, String> {
    if !registry.is_current(ticket) {
        return Err("account changed during verification".into());
    }
    let retirement = registry.retire(ticket.account_id)?;
    drop(lease);
    Ok(retirement)
}

#[tauri::command]
pub async fn repair_generic_account(
    app: AppHandle,
    account_id: i64,
    input: GenericAccountInput,
) -> Result<AccountInfo, CommandError> {
    let mut creds = generic_credentials(input)?;
    let (config, ticket, lease, creds) = off_pump(app.clone(), move |app| {
        let state = app.state::<AppState>();
        let store = Store::open(&adopted_db(&app)?)?;
        let account = store
            .accounts()?
            .into_iter()
            .find(|account| account.id == account_id)
            .ok_or("unknown account")?;
        let config = store.account_config(account_id)?;
        if account.provider != "imap"
            || account.email != creds.email
            || config.imap_host.as_deref() != Some(&creds.imap_host)
            || config.username.as_deref().unwrap_or(&account.email) != creds.username
        {
            return Err(
                "repair must keep the original address, IMAP server and login username".into(),
            );
        }
        let ticket = state.account_work.capture(account_id)?;
        let lease = ticket.lease()?;
        if creds.password.is_empty() {
            creds.password = mail_auth::fetch_generic_password_slot(
                &account.email,
                config.credential_slot.as_deref(),
            )
            .map_err(|err| err.to_string())?;
        }
        Ok::<_, CommandError>((config, ticket, lease, creds))
    })
    .await?;
    let creds = tauri::async_runtime::spawn_blocking(move || {
        probe_generic(&creds)?;
        Ok::<_, String>(creds)
    })
    .await
    .map_err(|err| err.to_string())??;
    let retirement = off_pump(app.clone(), move |app| {
        let state = app.state::<AppState>();
        let retirement = retire_generic_connection(&state.account_work, &ticket, lease)?;
        if let Some(alive) = recovered(&state.watchers).get(&creds.email) {
            alive.store(false, Ordering::Release);
        }
        Ok::<_, CommandError>((retirement, creds))
    })
    .await?;
    let (retirement, creds) = retirement;
    let worker_app = app.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let result = (|| -> Result<AccountInfo, CommandError> {
            retirement.wait(Duration::from_secs(30))?;
            let worker_app = Blocking::on_dedicated_thread(worker_app);
            let state = worker_app.state::<AppState>();
            let _commands = recovered(&state.commands);
            let store = Store::open(&adopted_db(&worker_app)?)?;
            if store.account_config(account_id)? != config {
                return Err("connection settings changed during verification".into());
            }
            let mut accounts = lock_accounts(&state)?;
            let id = publish_generic(
                &store,
                Some(account_id),
                &creds,
                config.credential_slot.as_deref(),
                None,
            )?;
            let info = AccountInfo {
                id,
                email: creds.email.clone(),
            };
            recovered(&state.sync_backoffs).remove(&creds.email);
            recovered(&state.gesture_passes).retain(|ticket, _| ticket.account_id != account_id);
            accounts.insert(creds.email.clone(), AccountSession::Generic(creds));
            Ok(info)
        })();
        if result.is_ok() {
            retirement.commit();
        }
        result
    })
    .await;
    crate::watcher::reconcile(&app);
    outcome.map_err(|err| err.to_string())?
}

/// Removes an account: its secrets leave the OS vault, its local data
/// the database, its session the memory. The server is NEVER touched —
/// the mail stays with the provider.
///
/// The order is a choice: the vault FIRST, the database next. If the
/// database failed after the vault, the next launch WOULD SAY SO
/// (“no token for…”) and the removal would be replayed; the reverse —
/// an orphaned token that survives the account — would stay invisible
/// forever.
#[tauri::command]
pub async fn remove_account(
    app: AppHandle,
    account_id: i64,
    forget: Option<bool>,
) -> Result<(), CommandError> {
    let forget = forget.unwrap_or(false);
    let (account, retirement) = off_pump(app.clone(), move |app| {
        let store = Store::open(&adopted_db(&app)?).map_err(|err| err.to_string())?;
        let account = store
            .accounts()
            .map_err(|err| err.to_string())?
            .into_iter()
            .find(|account| account.id == account_id)
            .ok_or_else(|| format!("unknown account: {account_id}"))?;
        let state = app.state::<AppState>();
        let retirement = state.account_work.retire(account_id)?;
        if let Some(alive) = recovered(&state.watchers).get(&account.email) {
            alive.store(false, Ordering::Release);
        }
        Ok::<_, CommandError>((account, retirement))
    })
    .await?;
    let email = account.email.clone();
    let work_app = app.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let result = (|| -> Result<(), CommandError> {
            retirement.wait(Duration::from_secs(30))?;
            let work_app = Blocking::on_dedicated_thread(work_app);
            let state = work_app.state::<AppState>();
            let _commands = recovered(&state.commands);
            let mut store = Store::open(&adopted_db(&work_app)?)?;
            // Admission is closed and every started operation has persisted its result.
            store.check_account_removal(account_id)?;
            mail_auth::forget_credentials(&account.provider, &account.email)
                .map_err(|err| err.to_string())?;
            // D4 (Lot 5 E14c): "also forget what Wind learned from this
            // account" — the rules and the image trust no other account
            // shares go with it.
            if forget {
                store.delete_account_forgetting(account_id)?;
            } else {
                store.delete_account(account_id)?;
            }
            lock_accounts(&state)?.remove(&account.email);
            recovered(&state.sync_backoffs).remove(&account.email);
            recovered(&state.gesture_passes).retain(|ticket, _| ticket.account_id != account_id);
            Ok(())
        })();
        if result.is_ok() {
            retirement.commit();
        } else {
            drop(retirement);
        }
        result
    })
    .await
    .map_err(|err| CommandError::from(err.to_string()))
    .and_then(|result| result);
    off_pump(app, move |app| {
        let state = app.state::<AppState>();
        recovered(&state.watchers).remove(&email);
        crate::watcher::reconcile(&app);
        Ok::<_, CommandError>(())
    })
    .await?;
    outcome
}

/// Builds a generic session from the password and the stored
/// configuration. Returns `None` if the configuration is incomplete.
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

/// Synchronizes ALL connected accounts — one account's failure doesn't
/// block the others (it's logged in the report).
///
/// The orchestration itself (backoff, per-account lock, activity
/// bookkeeping) is [`crate::poll::poll_cycle`] — shared with the light
/// pass below (PLAN-AUDIT-V3 E4). This command's own job is the
/// glue: gather the jobs, hand each account to
/// [`crate::poll::run_sync`], settle the sessions and the timestamp.
#[tauri::command]
pub async fn sync_inbox(app: AppHandle, state: State<'_, AppState>) -> Result<SyncSummary, String> {
    // The cadence tracks REALITY (PLAN-AUDIT-V3 E5): a cycle running
    // now — scheduler tick, UI startup, test — rearms the clock, so
    // the next tick never doubles it.
    recovered(&state.cadence).ran_full(Instant::now());
    let (path, jobs) = off_pump(app.clone(), |app| {
        Ok::<_, String>((adopted_db(&app)?, crate::poll::connected_jobs(&app)?))
    })
    .await?;
    let timer = Instant::now();
    let cycle = state.sync_cycle.clone();
    // The relay carries through the loop: bubbles go out PER ACCOUNT, as
    // soon as the INBOX poll is settled (P1) — no more end-of-cycle
    // aggregate, which always lost the race against the phone.
    let app_bubbles = app.clone();
    let backoffs = state.sync_backoffs.clone();
    let locks = state.poll_locks.clone();

    let mut tally = tauri::async_runtime::spawn_blocking(move || {
        let run_cycle = cycle.clone();
        crate::poll::poll_cycle(jobs, &cycle, &backoffs, &locks, false, {
            move |account_id, session| {
                crate::poll::run_sync(session, account_id, &path, &run_cycle, &app_bubbles)
                    .map(|outcome| (outcome.report, outcome.problems, outcome.refreshed))
            }
        })
    })
    .await
    .map_err(|err| err.to_string())?;

    reset_sessions(&state, std::mem::take(&mut tally.refreshed))?;
    crate::poll::settle_poll(&app, tally.accounts, &mut tally.errors).await?;

    Ok(SyncSummary {
        accounts: tally.accounts,
        accounts_failed: tally.accounts_failed,
        fetched: tally.fetched,
        deleted: tally.deleted,
        replayed: tally.replayed,
        elapsed_ms: timer.elapsed().as_millis() as u64,
        errors: tally.errors,
    })
}

/// Manual, scheduled and wake light sync: guarded INBOX polling and
/// complete remote-draft import on the same connection. Other folders
/// retain the full-cycle cadence; IDLE arrivals use the INBOX-only path.
#[tauri::command]
pub async fn sync_inbox_light(
    app: AppHandle,
    state: State<'_, AppState>,
    force: bool,
) -> Result<SyncSummary, String> {
    // Reality rearm, same rule as the full cycle (the button included).
    recovered(&state.cadence).ran_light(Instant::now());
    let (path, jobs) = off_pump(app.clone(), |app| {
        Ok::<_, String>((adopted_db(&app)?, crate::poll::connected_jobs(&app)?))
    })
    .await?;
    let timer = Instant::now();
    let cycle = state.sync_cycle.clone();
    let app_bubbles = app.clone();
    let backoffs = state.sync_backoffs.clone();
    let locks = state.poll_locks.clone();

    // The backoff also applies to the light pass (wake from sleep,
    // future IDLE) — EXCEPT on the manual gesture: the click is an
    // order, `force` always attempts.
    let mut tally = tauri::async_runtime::spawn_blocking(move || {
        let run_cycle = cycle.clone();
        crate::poll::poll_cycle(jobs, &cycle, &backoffs, &locks, force, {
            move |account_id, session| {
                let (mut server, fresh, _lease) = crate::poll::connect_imap(session)?;
                let mut store = Store::open(&path).map_err(|err| err.to_string())?;
                let mut problems = Vec::new();
                let hooks = crate::poll::ShellHooks::new(&run_cycle, app_bubbles.clone());
                let retry_partial = force
                    && store
                        .operation_issues()
                        .map_err(|err| err.to_string())?
                        .iter()
                        .any(|issue| issue.account_id == account_id);
                if retry_partial {
                    store
                        .retry_operations(account_id)
                        .map_err(|err| err.to_string())?;
                }
                let outcome = if retry_partial {
                    mail_core::cycle::run_sync(
                        &mut crate::poll::ShellServer(&mut server),
                        &mut store,
                        account_id,
                        &path,
                        &hooks,
                    )
                    .map(|outcome| {
                        problems.extend(outcome.problems);
                        outcome.report
                    })
                } else {
                    mail_core::cycle::run_light(
                        &mut crate::poll::ShellServer(&mut server),
                        &mut store,
                        account_id,
                        &hooks,
                        &mut problems,
                    )
                };
                server.logout();
                let report = outcome?;
                Ok((report, problems, fresh))
            }
        })
    })
    .await
    .map_err(|err| err.to_string())?;

    reset_sessions(&state, std::mem::take(&mut tally.refreshed))?;
    // The timestamp counts for the light pass too: every INBOX has just
    // been checked — it's mail polling in the prototype's sense, and a
    // button that would leave “12 minutes ago” after a successful click
    // would look broken. The folders, for their part, keep their own
    // cadence.
    crate::poll::settle_poll(&app, tally.accounts, &mut tally.errors).await?;

    Ok(SyncSummary {
        accounts: tally.accounts,
        accounts_failed: tally.accounts_failed,
        fetched: tally.fetched,
        deleted: tally.deleted,
        replayed: tally.replayed,
        elapsed_ms: timer.elapsed().as_millis() as u64,
        errors: tally.errors,
    })
}

// The page no LONGER carries a total (field finding 2026-08-20,
// PLAN-DEFILEMENT-PROFOND): counting a Gmail integral folder (a NOT
// EXISTS probe per row) costs ~240 ms on 200k — more than the page
// itself — and delayed every first render. The total lives in the
// separate command [`category_total`], requested by the front end
// AFTER the rows are displayed; a page shorter than its limit says by
// itself that the list has ended.
#[derive(Serialize)]
pub struct MessagePage {
    pub offset: usize,
    pub rows: Vec<MessageRow>,
    pub elapsed_us: u64,
}

/// The messages of a conversation, oldest to most recent.
///
/// Purely local: opening a thread never asks the network, same as
/// choosing a destination folder. That is the lesson of the folders,
/// which had been shipped by querying the server — unusable from the
/// first outage on.
#[tauri::command]
pub async fn thread_messages(
    app: AppHandle,
    thread_id: i64,
    account_id: i64,
    mailbox: String,
    uid: u32,
    version: MessageVersion,
) -> Result<Vec<MessageRow>, CommandError> {
    read_store_off_pump(app, move |_, store| {
        Ok(store
            .thread_messages_version(&version.identity(account_id, &mailbox), uid, thread_id)
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(to_message_row)
            .collect())
    })
    .await
}

/// Mapping shared between the unified mailbox and the search results.
fn to_message_row(row: mail_core::UnifiedRow) -> MessageRow {
    MessageRow {
        version: MessageVersion {
            mailbox_id: row.mailbox_id,
            uid_validity: row.uid_validity,
        },
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
            .unwrap_or_else(|| "(no subject)".to_string()),
        sender: row
            .envelope
            .sender
            .unwrap_or_else(|| "(unknown sender)".to_string()),
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
        aside: false,
        invitation: row.invitation.map(|rang| InvitationRowPayload {
            revision: rang.revision,
            version: MessageVersion {
                mailbox_id: rang.mailbox_id,
                uid_validity: rang.uid_validity,
            },
            mailbox: rang.mailbox,
            uid: rang.uid,
            title: rang.title,
            reply: rang.reply.as_deref().map(crate::wire::reply_to_wire),
            cancelled: rang.cancelled,
            can_reply: rang.can_reply,
        }),
    }
}

/// An account of nav v2 (screen 02), with its counters — canonical
/// folders resolved on the core side (`nav.rs`), the UI never sees a
/// network mailbox name.
#[derive(Serialize)]
pub struct NavAccount {
    pub account_id: i64,
    pub email: String,
    pub provider: String,
    // The nav says ONLY the unread count (A29): the 10 s probe only
    // pays for these two counters — the full inventory (`nav_counts`,
    // whose total for an integral folder probes at ~240 ms) is no
    // longer recomputed on the heartbeat (field finding 2026-08-20,
    // PLAN-DEFILEMENT-PROFOND).
    pub inbox_unread: u64,
    pub junk_unread: u64,
}

/// The full nav state in ONE call: accounts and counters per category.
/// "All mailboxes" is aggregated on the UI side.
fn read_nav(store: &Store) -> Result<Vec<NavAccount>, String> {
    // E2: in organized mode, the Inbox badge follows the shared
    // exclusion — the unread count of a held-back sender belongs to
    // the Screener badge, never to both.
    let organized = store.organized_mode().map_err(|err| err.to_string())?;
    let mut result = Vec::new();
    for account in store.accounts().map_err(|err| err.to_string())? {
        let folders = store
            .canonical_folders(account.id)
            .map_err(|err| err.to_string())?;
        let (inbox_unread, junk_unread) = store
            .nav_unread_counts(account.id, &folders, organized)
            .map_err(|err| err.to_string())?;
        result.push(NavAccount {
            account_id: account.id,
            email: account.email,
            provider: account.provider,
            inbox_unread,
            junk_unread,
        });
    }
    Ok(result)
}

#[tauri::command]
pub async fn nav_snapshot(app: AppHandle) -> Result<Vec<NavAccount>, CommandError> {
    store_off_pump(app, move |_, store| Ok(read_nav(store)?)).await
}

/// A page of one nav category, bounded or not to an account.
/// `reception` = the unified mailbox (conversations); the others = the
/// messages of the resolved canonical mailboxes, merged by date.
#[tauri::command]
pub async fn list_category(
    app: AppHandle,
    category: String,
    account_id: Option<i64>,
    unread: bool,
    offset: usize,
    limit: usize,
) -> Result<MessagePage, CommandError> {
    read_off_pump(app, move |app| {
        let category = crate::wire::category_from_wire(&category);
        let timer = Instant::now();
        let store = Store::open(&adopted_db(&app)?).map_err(|err| err.to_string())?;
        let limit = limit.min(LIST_LIMIT_MAX);
        if category == "reception" {
            // E2: in Organized mode, the Inbox HOLDS BACK the threads
            // of senders waiting at the Screener and those routed
            // elsewhere (flag + partial index — never a probe per row).
            // Classic mode goes through the HISTORICAL query, down to
            // the character: zero diff (e2e guard).
            let organized = store.organized_mode().map_err(|err| err.to_string())?;
            let mut rows = if organized {
                store
                    .organized_inbox_scoped(account_id, unread, offset, limit)
                    .map_err(|err| err.to_string())?
            } else {
                store
                    .unified_recent_scoped(account_id, unread, offset, limit)
                    .map_err(|err| err.to_string())?
            };
            // Field finding R10-R12: attachments summed per thread,
            // invitations on the row — a pass bounded to the PAGE, the
            // hot query pays nothing.
            store
                .enrich_rows(&mut rows)
                .map_err(|err| err.to_string())?;
            let rows = rows.into_iter().map(to_message_row).collect();
            return Ok(MessagePage {
                offset,
                rows,
                elapsed_us: timer.elapsed().as_micros() as u64,
            });
        }
        // PLAN-MODE-ORGANISE E1: the Feed and the Paper trail are views
        // of the unified flow filtered by sender routing — never
        // canonical mailboxes.
        if category == "kiosque" || category == "registre" {
            let mut rows = store
                .routing_unified_scoped(&category, account_id, unread, offset, limit)
                .map_err(|err| err.to_string())?;
            store
                .enrich_rows(&mut rows)
                .map_err(|err| err.to_string())?;
            let rows = rows.into_iter().map(to_message_row).collect();
            return Ok(MessagePage {
                offset,
                rows,
                elapsed_us: timer.elapsed().as_micros() as u64,
            });
        }
        let scope = resolve_category(&store, &category, account_id)?;
        // E3 (PLAN-REACTIVITE): the local echoes of gesture destinations
        // enter the page and the total — the Trash shows the deletion,
        // Sent shows the send, within the second of the gesture.
        let echoes = mail_core::ECHO_DESTINATIONS
            .contains(&category.as_str())
            .then_some((category.as_str(), scope.accounts.as_slice()));
        let mut rows = store
            .category_page(
                &scope.mailboxes,
                unread,
                &scope.excluded,
                echoes,
                offset,
                limit,
            )
            .map_err(|err| err.to_string())?;
        store
            .enrich_rows(&mut rows)
            .map_err(|err| err.to_string())?;
        let rows = rows.into_iter().map(to_message_row).collect();
        Ok(MessagePage {
            offset,
            rows,
            elapsed_us: timer.elapsed().as_micros() as u64,
        })
    })
    .await
}

/// Accounts in scope, resolved mailboxes and integral-folder exclusion
/// for a category other than the inbox — the resolution SHARED by the
/// page (`list_category`) and the count (`category_total`).
struct CategoryScope {
    accounts: Vec<i64>,
    mailboxes: Vec<i64>,
    excluded: Vec<i64>,
}

fn resolve_category(
    store: &Store,
    category: &str,
    account_id: Option<i64>,
) -> Result<CategoryScope, String> {
    let accounts: Vec<i64> = match account_id {
        Some(id) => vec![id],
        None => store
            .accounts()
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|account| account.id)
            .collect(),
    };
    let mut mailboxes = Vec::new();
    // The Archive of an INTEGRAL Gmail folder ("All Mail") strips the
    // category of the messages living in another canonical folder —
    // otherwise it shows the whole mailbox (field default, 2026-08-12).
    let mut excluded = Vec::new();
    for account in &accounts {
        let folders = store
            .canonical_folders(*account)
            .map_err(|err| err.to_string())?;
        if let Some(name) = folders.mailbox(category)
            && let Some(state) = store
                .sync_state(*account, &name)
                .map_err(|err| err.to_string())?
        {
            mailboxes.push(state.mailbox_id);
            if category == "archives" && folders.archives_full {
                excluded.extend(
                    store
                        .canonical_except_archive(*account, &folders)
                        .map_err(|err| err.to_string())?,
                );
            }
        }
    }
    Ok(CategoryScope {
        accounts,
        mailboxes,
        excluded,
    })
}

/// The total of a category — the SEPARATE command from the page
/// service (field finding 2026-08-20, PLAN-DEFILEMENT-PROFOND): counting
/// an integral folder (a NOT EXISTS probe per row, ~240 ms on 200k)
/// must never delay a first render — the front end calls it when its
/// page pump is at rest, and the scrollbar adjusts on arrival.
#[tauri::command]
pub async fn category_total(
    app: AppHandle,
    category: String,
    account_id: Option<i64>,
    unread: bool,
) -> Result<u64, CommandError> {
    read_off_pump(app, move |app| {
        let category = crate::wire::category_from_wire(&category);
        let store = Store::open(&adopted_db(&app)?).map_err(|err| err.to_string())?;
        if category == "reception" {
            // E2: the total follows the flow — exclusion SHARED with
            // the page (lesson of `pins`), and classic mode stays
            // intact.
            let organized = store.organized_mode().map_err(|err| err.to_string())?;
            return Ok(if organized {
                store.organized_inbox_count_scoped(account_id, unread)?
            } else {
                store.unified_count_scoped(account_id, unread)?
            });
        }
        if category == "kiosque" || category == "registre" {
            return Ok(store.routing_count_scoped(&category, account_id, unread)?);
        }
        let scope = resolve_category(&store, &category, account_id)?;
        let echoes = mail_core::ECHO_DESTINATIONS
            .contains(&category.as_str())
            .then_some((category.as_str(), scope.accounts.as_slice()));
        let (all, unread_only) = store
            .category_totals(&scope.mailboxes, &scope.excluded, echoes)
            .map_err(|err| err.to_string())?;
        Ok(if unread { unread_only } else { all })
    })
    .await
}

/// Catches up the preview of bodies written before the `preview`
/// column, in bounded batches — the UI calls it as it polls, down to
/// zero, never on the opening path. Returns the number remaining.
#[tauri::command]
pub async fn preview_catchup(app: AppHandle, limit: usize) -> Result<u64, CommandError> {
    store_off_pump(app, move |_, store| Ok(store.preview_catchup(limit)?)).await
}

/// The results of a search: the rendered rows (capped at
/// `SEARCH_LIMIT`) and the TOTAL number of matches — to say "100 of N"
/// when the render is capped.
#[derive(Serialize)]
pub struct SearchResults {
    pub rows: Vec<MessageRow>,
    pub total: u64,
}

/// Full-text search across all accounts, one slice at a time.
/// `offset` serves "load more": 0 while typing, then the number of
/// rows already shown. Triggering from 3 characters and the debounce
/// are the UI's responsibility.
#[tauri::command]
pub async fn search_messages(
    app: AppHandle,
    query: String,
    offset: usize,
) -> Result<SearchResults, CommandError> {
    read_store_off_pump(app, move |_, store| {
        // `search_capped` returns the slice `[offset, offset+SEARCH_LIMIT)` AND
        // the exact total, and switches to date sort past the wide-query
        // threshold (BM25 ranking there exceeds the budget and stops meaning
        // anything — ADR 0004). Since the sort only depends on the total, the
        // slices chain without a gap or a duplicate.
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
    pub images_message_allowed: bool,
    pub document: String,
    pub remote_images_blocked: usize,
    /// The attachment count AFTER-SCAN: the first opening of a message
    /// just wrote its attachments to the database (`load_body`), but
    /// the list row that led here carried the count from BEFORE —
    /// trusting it made freshly received attachments open on an empty
    /// row (CE field finding, 2026-08-14).
    pub attachment_count: usize,
    /// The message's invitation card, WHOLE (fresh `invitations` row
    /// from the same scan): it travels with the body — a second
    /// command to reread it cost an IPC round trip and a duplicate
    /// query per opening (review).
    pub invitation: Option<InvitationView>,
}

/// Body of a message: local cache first (no network), the account's
/// server otherwise. Auto-CSP document loaded in a `sandbox` iframe —
/// the three defense layers of Phase 0.
#[tauri::command]
pub async fn message_body(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    version: MessageVersion,
    show_images: bool,
) -> Result<BodyView, CommandError> {
    // Current path — cached body: ONE lock take, ONE opening (PLAN-AUDIT-V1
    // review: `raw_body` then a second `off_pump` used to take the
    // lock twice for nothing). Under the lock ONLY the SQLite reads;
    // the sanitize runs unlocked (Lot 5 E13d).
    let mailbox2 = mailbox.clone();
    let cached = read_off_pump(app.clone(), move |app| {
        let store = Store::open(&adopted_db(&app)?).map_err(|err| err.to_string())?;
        version
            .verify(&store, account_id, &mailbox2)
            .map_err(|err| err.to_string())?;
        match store
            .body(account_id, &mailbox2, uid)
            .map_err(|err| err.to_string())?
        {
            Some(html) => {
                let inputs = body_inputs(&store, account_id, &mailbox2, uid, show_images)?;
                Ok::<_, CommandError>(Some((html, inputs)))
            }
            None => Ok(None),
        }
    })
    .await?;
    let (html, inputs) = match cached {
        Some(found) => found,
        None => {
            // Body absent: bare network fetch, then the inputs under
            // the lock.
            let html = raw_body(&app, account_id, &mailbox, uid, version).await?;
            let mailbox2 = mailbox.clone();
            let inputs = read_store_off_pump(app.clone(), move |_, store| {
                version.verify(store, account_id, &mailbox2)?;
                Ok(body_inputs(store, account_id, &mailbox2, uid, show_images)?)
            })
            .await?;
            (html, inputs)
        }
    };
    // The sanitize (CPU-heavy: a 28 MB body, D-1) OFF the lock. A pure
    // read: the identity was verified with the body it describes, and
    // nothing is written after — no second look (the next gesture on
    // the message verifies again; review 2026-09-07 dropped the extra
    // lock take the spike had kept).
    unlocked(move || Ok::<_, CommandError>(render_body(inputs, &html))).await
}

/// The sanitized document of a body and how many remote images it
/// held back — the one renderer behind the reading pane, the echo and
/// the Feed (review 2026-09-07: it was written three times).
fn render_document(html: &str, images_granted: bool) -> (String, usize) {
    let policy = if images_granted {
        mail_render::ImagePolicy::AllowRemote
    } else {
        mail_render::ImagePolicy::BlockRemote
    };
    let sanitized = mail_render::sanitize_with(html, policy);
    // R3 (PLAN-RETOURS-4, D3, 2026-08-18): the body ALWAYS displays
    // on a light slate (`Palette::default` = dark ink / white
    // background), whatever the theme. A42's dark slate made
    // sender-colored text unreadable (common: newsletters designed
    // for a white background — field finding 2026-08-18); the email
    // reads as it was composed, like in mature clients. Text
    // WITHOUT its own color was already readable; text that carries
    // one now is too.
    (
        mail_render::email_document(&sanitized.html, policy, &mail_render::Palette::default()),
        sanitized.remote_images_blocked,
    )
}

/// The SQLite half of a body view: what must stay under the lock
/// (Lot 5 E13d) — image guard, attachments, invitation.
struct BodyInputs {
    images_granted: bool,
    images_message_allowed: bool,
    attachment_count: usize,
    invitation: Option<InvitationView>,
}

fn body_inputs(
    store: &Store,
    account_id: i64,
    mailbox: &str,
    uid: u32,
    show_images: bool,
) -> Result<BodyInputs, String> {
    {
        // R1 (PLAN-RETOURS-11, D1): the image guard's memory is consulted
        // HERE — the authority is the core, the UI decides nothing (it
        // only sees a `remote_images_blocked` of zero, hence no banner).
        // Three indexed reads at worst (point lookups on PK), and none
        // when `show_images` already settles it.
        let images_granted = if show_images {
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
            .map(invitation_view);
        let images_message_allowed = store
            .sync_state(account_id, mailbox)
            .map_err(|e| e.to_string())?
            .map(|m| store.images_allowed_message(m.mailbox_id, uid))
            .transpose()
            .map_err(|e| e.to_string())?
            .unwrap_or(false);
        Ok(BodyInputs {
            images_granted,
            images_message_allowed,
            attachment_count,
            invitation,
        })
    }
}

/// The CPU half of a body view — the sanitize and the document — needs
/// no store: it runs through `unlocked` (Lot 5 E13d).
fn render_body(inputs: BodyInputs, html: &str) -> BodyView {
    let BodyInputs {
        images_granted,
        images_message_allowed,
        attachment_count,
        invitation,
    } = inputs;
    let (document, remote_images_blocked) = render_document(html, images_granted);
    BodyView {
        images_message_allowed,
        document,
        remote_images_blocked,
        attachment_count,
        invitation,
    }
}

fn fetch_body(
    session: &AccountWork,
    db_path: &Path,
    account_id: i64,
    mailbox: String,
    uid: u32,
    version: MessageVersion,
) -> Result<String, CommandError> {
    let (mut server, _refreshed, _lease) = crate::poll::connect_imap(session)?;
    let mut store = Store::open(db_path).map_err(|err| err.to_string())?;
    let body = mail_core::load_body_version(
        &mut server,
        &mut store,
        &version.identity(account_id, &mailbox),
        uid,
    )?;
    server.logout();
    body.ok_or_else(|| mail_core::Error::MissingOnServer.into())
}

/// Raw HTML body of a message: local cache first (no network), the
/// account's server otherwise — path shared by read/reply/forward.
async fn raw_body(
    app: &AppHandle,
    account_id: i64,
    mailbox: &str,
    uid: u32,
    version: MessageVersion,
) -> Result<String, CommandError> {
    // E5: the cache read and the session under `off_pump` (database +
    // commands' lock); only the network fetch runs bare.
    let mailbox2 = mailbox.to_string();
    let cached: Result<String, (AccountWork, PathBuf)> = off_pump(app.clone(), move |app| {
        let path = adopted_db(&app)?;
        let store = Store::open(&path)?;
        let identity = version.identity(account_id, &mailbox2);
        let cached = store.body_version(&identity, uid)?;
        if cached.is_none()
            && let Some(limit) = store.body_download_limit(&identity, uid)?
        {
            return Err(mail_core::Error::RemoteMessageTooLarge { uid, limit }.into());
        }
        match cached {
            Some(html) => Ok::<_, CommandError>(Ok(html)),
            None => Ok(Err((auth_for(&app, account_id)?, path))),
        }
    })
    .await?;
    match cached {
        Ok(html) => Ok(html),
        Err((session, path)) => {
            // Owned copy: the closure runs on another thread.
            let mailbox = mailbox.to_string();
            let result = tauri::async_runtime::spawn_blocking(move || {
                fetch_body(&session, &path, account_id, mailbox, uid, version)
            })
            .await
            .map_err(|err| err.to_string())?;
            Ok(result?)
        }
    }
}

/// An attachment as the UI presents it.
#[derive(Serialize)]
pub struct AttachmentRow {
    pub index: usize,
    pub name: String,
    pub mime: String,
    pub size: String,
}

/// The attachments known for a message — LOCAL read, no network. Empty
/// until the body has been fetched: the same condition as full-text
/// search, and the catch-up lifts it.
#[tauri::command]
pub async fn message_attachments(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    version: MessageVersion,
) -> Result<Vec<AttachmentRow>, CommandError> {
    store_off_pump(app, move |_, store| {
        version.verify(store, account_id, &mailbox)?;
        let found = store
            .attachments(account_id, &mailbox, uid)
            .map_err(|err| err.to_string())?;
        version.verify(store, account_id, &mailbox)?;
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

/// A message's invitation card, as the UI presents it (PLAN-INVITATIONS).
/// Times are UTC epochs when resolved; otherwise the TEXT form is
/// authoritative (`all_day` or `floating_time` — the UI
/// displays the latter as-is, with the note "the organizer's local
/// time", D1 guard).
#[derive(Serialize)]
pub struct InvitationView {
    pub revision: i64,
    pub scheduling_state: String,
    pub needs_refresh: bool,
    /// `request` | `cancel` | `reply`.
    pub method: String,
    pub title: String,
    pub location: Option<String>,
    /// The organizer's display name, otherwise their address.
    pub organizer: Option<String>,
    pub start_epoch: Option<i64>,
    pub end_epoch: Option<i64>,
    pub start_text: Option<String>,
    pub end_text: Option<String>,
    pub all_day: bool,
    pub floating_time: bool,
    pub recurrent: bool,
    /// Our last reply sent from Wind (`accepted` | `tentative` |
    /// `declined`, wire words), otherwise the PARTSTAT read from the message.
    pub status: Option<String>,
    /// The replier of a received REPLY (name, otherwise address) and
    /// their status.
    pub attendee: Option<String>,
    pub attendee_status: Option<String>,
    /// The meeting is cancelled: true on the CANCEL itself AND on the
    /// REQUEST of the same meeting (cross-link, field finding R6) — the
    /// original card says the cancellation, wherever the user looks.
    pub cancelled: bool,
    /// The three gestures are possible: REQUEST with an organizer, not
    /// cancelled. Being in the ATTENDEE list is NOT required (field
    /// finding R8, CE verdict: a forwarded invitation IS an invitation —
    /// whoever forwards it takes responsibility for it).
    pub can_reply: bool,
}

fn invitation_view(stored: mail_core::StoredInvitation) -> InvitationView {
    let row = stored.row;
    let cancelled = row.cancelled;
    let can_reply = row.can_reply();
    InvitationView {
        revision: stored.revision,
        needs_refresh: row.metadata_version == 0,
        scheduling_state: row.scheduling_state,
        cancelled,
        // The D1 guard by ENDPOINT: an end with an unresolved TZID is
        // enough to say "the organizer's local time" (review — a
        // resolved-start/floating-end pair used to display a misleading
        // range).
        floating_time: (row.start_text.is_some() || row.end_text.is_some()) && !row.all_day,
        organizer: row.organizer_name.or(row.organizer_address),
        // D6: the displayed status follows the LAST reply sent from
        // Wind; the message's PARTSTAT is only the starting state.
        status: stored
            .reply
            .or(row.partstat)
            .as_deref()
            .map(crate::wire::reply_to_wire),
        attendee: row.attendee_name.or(row.attendee_address),
        attendee_status: row
            .attendee_status
            .as_deref()
            .map(crate::wire::reply_to_wire),
        method: row.method,
        title: row.title,
        location: row.location,
        start_epoch: row.start_epoch,
        end_epoch: row.end_epoch,
        start_text: row.start_text,
        end_text: row.end_text,
        all_day: row.all_day,
        recurrent: row.recurrent,
        can_reply,
    }
}

/// Re-extract one legacy calendar source; the cached body stays available on failure.
#[tauri::command]
pub async fn refresh_invitation(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    version: MessageVersion,
) -> Result<Option<InvitationView>, CommandError> {
    let folder = mailbox.clone();
    let ready = off_pump(app.clone(), move |app| {
        let path = adopted_db(&app)?;
        let store = Store::open(&path)?;
        version.verify(&store, account_id, &folder)?;
        let stored = store.invitation_version(&version.identity(account_id, &folder), uid)?;
        if stored.as_ref().is_none_or(|i| i.row.metadata_version != 0) {
            Ok::<_, CommandError>(Ok(stored.map(invitation_view)))
        } else {
            Ok(Err((auth_for(&app, account_id)?, path)))
        }
    })
    .await?;
    let (session, path) = match ready {
        Ok(view) => return Ok(view),
        Err(session) => session,
    };
    tauri::async_runtime::spawn_blocking(move || {
        let (mut server, _, _lease) = crate::poll::connect_imap(&session)?;
        let result = (|| {
            let store = Store::open(&path).map_err(|e| e.to_string())?;
            mail_core::refresh_invitation_version(
                &mut server,
                &store,
                &version.identity(account_id, &mailbox),
                uid,
            )
            .map(|stored| stored.map(invitation_view))
            .map_err(|e| e.to_string())
        })();
        server.logout();
        result
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(Into::into)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitationSelection {
    account_id: i64,
    mailbox: String,
    uid: u32,
    version: MessageVersion,
    revision: i64,
}

/// Replies to an invitation (PLAN-INVITATIONS, D5-D6): the iTIP
/// `METHOD:REPLY` email is LOGGED to the outbox (ADR 0003 golden rules —
/// offline, it goes out on next launch), the reply is recorded on the
/// card, and the up-to-date view is returned. The subject and body come
/// from the UI: it is the one that speaks the product's language.
#[tauri::command]
pub async fn reply_invitation(
    app: AppHandle,
    selection: InvitationSelection,
    reply: String,
    subject: String,
    body: String,
) -> Result<Option<InvitationView>, CommandError> {
    let InvitationSelection {
        account_id,
        mailbox,
        uid,
        version,
        revision,
    } = selection;
    off_pump(app, move |app| {
        // The UI speaks the wire word (`accepted`); the core and the
        // database keep the French stable string (D16).
        let reply = crate::wire::reply_from_wire(&reply);
        mail_core::participation_de_stable(&reply)
            .filter(|p| !matches!(p, mail_ical::Participation::NeedsAction))
            .ok_or_else(|| format!("unknown reply: {reply}"))?;
        let store = Store::open(&adopted_db(&app)?).map_err(|err| err.to_string())?;
        admit_account(&app, &store, account_id)?;
        version.verify(&store, account_id, &mailbox)?;
        let stored = store
            .invitation(account_id, &mailbox, uid)
            .map_err(|err| err.to_string())?
            .ok_or_else(|| "no invitation on this message".to_string())?;
        if !stored.row.can_reply() || stored.revision != revision {
            return Err(
                "this invitation has changed or cannot be answered; reopen the message".into(),
            );
        }
        let organizer = stored
            .row
            .organizer_address
            .clone()
            .ok_or_else(|| "invitation without an organizer: reply impossible".to_string())?;
        let from = account_email(&store, account_id)?;
        // The reply joins the invitation's conversation (thread).
        let in_reply_to = store
            .envelope(account_id, &mailbox, uid)
            .map_err(|err| err.to_string())?
            .and_then(|envelope| envelope.message_id);
        let mut draft = mail_core::compose(
            &from,
            &organizer,
            "",
            "",
            &subject,
            &body,
            in_reply_to.as_deref(),
        )
        .map_err(|err| err.to_string())?;
        // E7: the whole References chain (RFC 5322 §3.6.4).
        draft.references = store
            .references_of(account_id, &mailbox, uid)
            .map_err(|err| err.to_string())?;
        let queued = store
            .enqueue_invitation_reply(
                mail_core::InvitationReplyTarget {
                    identity: &version.identity(account_id, &mailbox),
                    uid,
                    revision,
                },
                &draft,
                &reply,
                chrono::Utc::now().timestamp(),
            )
            .map_err(|err| err.to_string())?;
        if queued.is_none() {
            return Err("the invitation changed or cannot be answered; nothing was queued".into());
        }
        let updated = store
            .invitation(account_id, &mailbox, uid)
            .map_err(|err| err.to_string())?;
        Ok(updated.map(invitation_view))
    })
    .await
}

#[derive(Serialize)]
pub struct ContactRow {
    pub address: String,
    pub name: Option<String>,
}

/// Address suggestions for a prefix typed in To/Cc/Bcc (PLAN-RETOURS-5,
/// D3/D4): the contacts directory — a small table, learned from mail
/// seen — ranked by recency + frequency. Local read, no network.
#[tauri::command]
pub async fn complete_addresses(
    app: AppHandle,
    prefix: String,
    limit: usize,
) -> Result<Vec<ContactRow>, CommandError> {
    store_off_pump(app, move |_, store| {
        let found = store
            .complete_addresses(&prefix, limit.min(16))
            .map_err(|err| err.to_string())?;
        Ok(found
            .into_iter()
            .map(|c| ContactRow {
                address: c.address,
                name: c.name,
            })
            .collect())
    })
    .await
}

/// The SUGGESTED save path for an attachment (R1, PLAN-RETOURS-4, D2):
/// the Downloads folder + a sanitized name made unique. The name comes
/// from the UI (already shown in the chip) — no point reopening the
/// database to reread it; `safe_file_name` remains the sanitization
/// authority for a name coming from the network (defense in depth, even
/// though the dialog then lets the user decide the final folder AND
/// name).
#[tauri::command]
pub async fn suggested_save_path(app: AppHandle, name: String) -> Result<String, CommandError> {
    off_pump(app, move |app| {
        let directory = app
            .path()
            .download_dir()
            .map_err(|err| format!("Downloads folder not found: {err}"))?;
        Ok(unique_path(&directory, &safe_file_name(&name))
            .to_string_lossy()
            .into_owned())
    })
    .await
}

/// Saves an attachment to the path CHOSEN by the user (R1,
/// PLAN-RETOURS-4, D2) and returns that path. The "Save as" dialog is
/// opened on the UI side (`plugin:dialog|save`); here we only fetch the
/// bytes — never cached, redownloaded on demand — and write them to the
/// wanted spot.
#[tauri::command]
pub async fn save_attachment(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    version: MessageVersion,
    index: usize,
    dest: String,
) -> Result<String, CommandError> {
    let mailbox_name = mailbox.clone();
    let (session, identity) = off_pump(app.clone(), move |app| {
        let store = Store::open(&adopted_db(&app)?).map_err(|err| err.to_string())?;
        let identity = version.identity(account_id, &mailbox_name);
        store.verify_mailbox_identity(&identity)?;
        Ok::<_, CommandError>((auth_for(&app, account_id)?, identity))
    })
    .await?;
    let completion_ticket = session.ticket.clone();
    let expected_generation = identity.uid_validity;
    let bytes = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<u8>, CommandError> {
        let (mut server, _refreshed, _lease) = crate::poll::connect_imap(&session)?;
        let bytes = mail_core::fetch_attachment_checked(
            &mut server,
            &mailbox,
            expected_generation,
            uid,
            index,
        )?;
        server.logout();
        bytes.ok_or_else(|| CommandError::new("attachment missing from the message"))
    })
    .await
    .map_err(|err| err.to_string())??;

    // E5: the disk write (bytes chosen by the sender, up to 25 MB) off
    // the bare async worker.
    store_off_pump(app, move |app, store| {
        if !app
            .state::<AppState>()
            .account_work
            .is_current(&completion_ticket)
        {
            return Err("account changed during download".into());
        }
        store
            .verify_mailbox_identity(&identity)
            .map_err(|err| err.to_string())?;
        let dest = output_path(&dest)?;
        std::fs::write(&dest, &bytes).map_err(|err| format!("write failed: {err}"))?;
        Ok(dest.to_string_lossy().into_owned())
    })
    .await
}

/// The path where a received attachment may be written (PLAN-AUDIT-V2
/// E8, defense in depth): coming from the webview dialog, it will be
/// written with bytes chosen by the sender — absolute, no `..`
/// traversal, in a folder that exists. Pure, tested decision.
fn output_path(dest: &str) -> Result<std::path::PathBuf, String> {
    let path = std::path::Path::new(dest);
    if !path.is_absolute() {
        return Err("relative save path refused".to_string());
    }
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("save path with traversal refused".to_string());
    }
    if path.file_name().is_none() {
        return Err("save path without a file name".to_string());
    }
    match path.parent() {
        Some(folder) if folder.is_dir() => Ok(path.to_path_buf()),
        _ => Err("save folder not found".to_string()),
    }
}

/// Reduces a name coming from the NETWORK to an inoffensive file name.
///
/// An attachment name is a string chosen by the sender. As-is,
/// `../../.ssh/authorized_keys` would write outside the intended
/// folder: that is an arbitrary file write, triggered by a simple click
/// on a received message. Nothing that follows is excessive caution.
fn safe_file_name(raw: &str) -> String {
    // Keep only the last segment: every separator, every `..`
    // traversal and every drive prefix disappears with it.
    let base = raw
        .rsplit(['/', '\\', ':'])
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('.');
    let cleaned: String = base
        .chars()
        .map(|c| match c {
            // Forbidden by Windows, plus control characters.
            '<' | '>' | '"' | '|' | '?' | '*' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .take(120)
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() || is_reserved_device_name(&cleaned) {
        return "attachment".to_string();
    }
    cleaned
}

/// Names reserved by Windows: a file named `CON` or `LPT1` is refused
/// by the OS, whatever the extension.
fn is_reserved_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.ends_with(|c: char| c.is_ascii_digit() && c != '0'))
}

/// Free path in `directory`: `invoice.pdf`, then `invoice (2).pdf`…
/// Saving twice must never overwrite the first file.
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

/// Archive: immediate local disappearance + logging, the account's
/// server will follow on the next sync.
#[tauri::command]
pub async fn archive_message(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<(), CommandError> {
    off_pump(app, move |app| {
        queue_removal(&app, account_id, mailbox, uid, Action::Archive)
    })
    .await
}

/// One more thing changed in a view (PLAN-AUDIT-V3 E7, closing D-48):
/// a WRITE outside the List's own paths — Settings removing a routing,
/// an e2e command, tomorrow a second window — bumps the same generation
/// the polls bump. The resting probe reads it within 5 s and the views
/// reload through ONE mechanism; the gesture handlers keep their
/// immediate local reloads on top.
pub(crate) fn bump_view_generation(app: &AppHandle) {
    app.state::<AppState>()
        .sync_cycle
        .generation
        .fetch_add(1, Ordering::Relaxed);
}

/// The state the UI polls at rest, in ONE command (PLAN-AUDIT-V2 E10):
/// nav, sync progress, outbox — three probes, three `Store::open` calls
/// and three passes through the serialized queue used to cost 10 s
/// before; a single one now, on a single connection.
#[derive(Serialize)]
pub struct UiState {
    pub nav: Vec<NavAccount>,
    pub connected: Vec<String>,
    pub connection_states: std::collections::HashMap<i64, crate::account_work::ConnectionState>,
    pub sync: SyncProgress,
    pub outbox: OutboxStatus,
    /// Revision of the drafts table (count, latest edit, largest id) —
    /// the UI fetches the ACTUAL list only when this moves, instead of
    /// polling `list_drafts` whole (bodies included) every ten seconds
    /// (PLAN-AUDIT-V3 E5, D-52 item 3).
    pub drafts_revision: (i64, i64, i64),
    /// Revision of the views (Lot 5 E13e, D-48): moved by any core
    /// write that changes a list outside the sync's generation — the
    /// UI reloads its views when it moves, whoever wrote.
    pub views_revision: i64,
}

#[tauri::command]
pub async fn ui_state(app: AppHandle, state: State<'_, AppState>) -> Result<UiState, CommandError> {
    let generation = state.sync_cycle.generation.load(Ordering::Relaxed);
    let in_progress = state.sync_cycle.in_progress.load(Ordering::Relaxed);
    read_store_off_pump(app, move |app, store| {
        let state = app.state::<AppState>();
        let connected = lock_accounts(&state)?.keys().cloned().collect();
        let connection_states = state.account_work.connection_states();
        Ok(UiState {
            nav: read_nav(store)?,
            connected,
            connection_states,
            sync: read_sync(store, generation, in_progress)?,
            outbox: read_sends(store)?,
            drafts_revision: store.drafts_revision()?,
            views_revision: store.views_revision()?,
        })
    })
    .await
}

/// A checked row, as the selection bar names it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetArg {
    pub account_id: i64,
    pub mailbox: String,
    pub uid: u32,
    pub thread_id: Option<i64>,
}

#[derive(Serialize)]
pub struct GroupOutcome {
    pub done: usize,
    pub total: usize,
}

/// The BULK gesture of the selection bar (PLAN-AUDIT-V2 E6): ONE call,
/// ONE transaction, all or nothing (D6) — the UI used to replay N × k
/// unit commands in series. `action`: the bar's keys (archiver,
/// supprimer, spam, nonspam, lu, nonlu).
#[tauri::command]
pub async fn act_on_group(
    app: AppHandle,
    targets: Vec<TargetArg>,
    action: String,
) -> Result<GroupOutcome, CommandError> {
    let gesture = match action.as_str() {
        "archive" => mail_core::GroupGesture::Archive,
        "delete" => mail_core::GroupGesture::Delete,
        "spam" => mail_core::GroupGesture::Spam,
        "not_spam" => mail_core::GroupGesture::NotSpam,
        "read" => mail_core::GroupGesture::Seen(true),
        "unread" => mail_core::GroupGesture::Seen(false),
        other => return Err(format!("unknown bulk gesture: {other}").into()),
    };
    let total = targets.len();
    store_off_pump(app, move |app, store| {
        let targets: Vec<mail_core::GestureTarget> = targets
            .into_iter()
            .map(|cible| mail_core::GestureTarget {
                account_id: cible.account_id,
                mailbox: cible.mailbox,
                uid: cible.uid,
                thread_id: cible.thread_id,
            })
            .collect();
        for account in targets
            .iter()
            .map(|target| target.account_id)
            .collect::<std::collections::BTreeSet<_>>()
        {
            admit_account(app, store, account)?;
        }
        let done = store
            .act_on_group(&targets, &gesture)
            .map_err(|err| err.to_string())?;
        bump_view_generation(app);
        Ok(GroupOutcome { done, total })
    })
    .await
}

/// A folder offered to the user.
#[derive(Serialize)]
pub struct FolderRow {
    /// NETWORK name — this is what the UI will send back for a move.
    pub wire: String,
    /// Readable name, decoded from modified UTF-7.
    pub display: String,
}

/// The folders of an account where a message can be moved.
///
/// **Purely local** read: the cache is filled by the sync. Moving a
/// message must work offline — the action is logged and replayed, like
/// archiving. Querying the server here would make sorting depend on the
/// network, which the product refuses (PLAN.md §1).
///
/// The current mailbox is excluded: "move to INBOX" from INBOX makes
/// no sense, and some servers refuse it.
#[tauri::command]
pub async fn list_folders(app: AppHandle, account_id: i64) -> Result<Vec<FolderRow>, CommandError> {
    read_store_off_pump(app, move |_, store| {
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

/// Moves a message: immediate local disappearance + logging, the server
/// will follow on the next sync — same loop as archiving.
#[tauri::command]
pub async fn move_message(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    folder: String,
) -> Result<(), CommandError> {
    off_pump(app, move |app| {
        // The name comes from the UI, which got it from `list_folders`:
        // it is already in network form. Decoding it here would make
        // the replay fail.
        if folder.trim().is_empty() {
            return Err("destination folder missing".into());
        }
        queue_removal(&app, account_id, mailbox, uid, Action::MoveTo(folder))
    })
    .await
}

/// Deletion: immediate local disappearance + logging, moved to the
/// account's server trash on the next sync.
#[tauri::command]
pub async fn delete_message(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<(), CommandError> {
    off_pump(app, move |app| {
        queue_removal(&app, account_id, mailbox, uid, Action::Delete)
    })
    .await
}

/// Reports a message as junk (R2, PLAN-RETOURS-3): it moves to the
/// server's Junk folder — it is THAT server that learns (Gmail trains
/// its filter on the move). Same loop as archiving: immediate local
/// disappearance, `MoveTo` action logged and replayed, the server
/// follows. The junk folder is resolved per account
/// (`canonical_folders`); without a recognized folder, the gesture
/// fails outright rather than inventing a destination.
#[tauri::command]
pub async fn report_spam(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<(), CommandError> {
    account_store_off_pump(app, account_id, move |app, store| {
        let folders = store
            .canonical_folders(account_id)
            .map_err(|err| err.to_string())?;
        let Some(spam) = folders.junk else {
            return Err("no junk folder recognized on this account".into());
        };
        // Already in Junk: nothing to do (the view does not offer the
        // gesture, but the guard avoids a move onto itself).
        if spam == mailbox {
            return Ok(());
        }
        queue_removal(app, account_id, mailbox, uid, Action::MoveTo(spam))
    })
    .await
}

/// The reverse (R2): a message wrongly classified as junk goes back to
/// the Inbox. Offered only from the Junk view. Same loop — `MoveTo(INBOX)`
/// logged, the server reconciles, the thread is reconstituted on the
/// next INBOX poll (ADR 0009).
#[tauri::command]
pub async fn mark_not_spam(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<(), CommandError> {
    off_pump(app, move |app| {
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
    app: &Blocking,
    account_id: i64,
    mailbox: String,
    uid: u32,
    action: Action,
) -> Result<(), CommandError> {
    let store = Store::open(&adopted_db(app)?)?;
    admit_account(app, &store, account_id)?;
    let Some(state) = store.sync_state(account_id, &mailbox)? else {
        return Ok(());
    };
    // E3 (PLAN-REACTIVITE, R-D1): the gesture is a MOVE — the message's
    // matter passes to the destination echo in the SAME transaction as
    // the action log and the source's disappearance. The destination
    // shows in < 1 s, offline included; the server reconciles behind it
    // (`sync_after_gesture`). A move to a free-form folder has no
    // canonical list: no echo.
    let destination = match &action {
        Action::Delete => Some("corbeille"),
        Action::Archive => Some("archives"),
        _ => None,
    };
    store.gesture_with_echo(state.mailbox_id, uid, action, destination)?;
    bump_view_generation(app);
    Ok(())
}

/// The body of a local echo (E3) for Reading: same sanitization as
/// `message_body` (S1 — the original HTML is the sender's, the sent
/// text is already escaped but goes through the same door). Purely
/// local — an echo has nothing to ask the server.
#[tauri::command]
pub async fn echo_body(
    app: AppHandle,
    id: i64,
    show_images: bool,
) -> Result<BodyView, CommandError> {
    let (html, attachment_count) = read_store_off_pump(app, move |_, store| {
        Ok(store
            .echo_view(id)
            .map_err(|err| err.to_string())?
            .ok_or_else(|| "echo already reconciled".to_string())?)
    })
    .await?;
    // Our own send, read once and written nowhere after: the sanitize
    // runs unlocked (Lot 5 E13d) and nothing needs a second look.
    unlocked(move || {
        // R3: light slate always (see `render_document`) — same door, S1.
        let (document, remote_images_blocked) = render_document(&html, show_images);
        Ok::<_, CommandError>(BodyView {
            images_message_allowed: false,
            document,
            remote_images_blocked,
            attachment_count,
            // An echo is OUR OWN send: never a received invitation.
            invitation: None,
        })
    })
    .await
}

/// The attachments of a send echo, metadata only (PLAN-RETOURS-5, D2):
/// name, mime, size from the send log — the bytes are purged at
/// `sent`, the chips stay inert during the reconciliation window. A
/// gesture echo returns an empty list.
#[tauri::command]
pub async fn echo_attachments(app: AppHandle, id: i64) -> Result<Vec<AttachmentRow>, CommandError> {
    store_off_pump(app, move |_, store| {
        let found = store.echo_attachments(id).map_err(|err| err.to_string())?;
        Ok(found
            .into_iter()
            .enumerate()
            .map(|(index, attachment)| AttachmentRow {
                index,
                size: mail_core::human_size(attachment.size),
                name: attachment.name,
                mime: attachment.mime,
            })
            .collect())
    })
    .await
}

/// Marks seen/unseen: immediate local application (UI optimism) +
/// logging — the account's next sync replays it to the server.
#[tauri::command]
pub async fn mark_seen(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    seen: bool,
) -> Result<(), CommandError> {
    account_store_off_pump(app, account_id, move |app, store| {
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
            // Review (wave 3): the bump follows a REAL change only —
            // re-marking read mail must not reload every view.
            bump_view_generation(app);
        }
        Ok(())
    })
    .await
}

/// Flag/unflag: same contract as seen/unseen, same replayable queue.
#[tauri::command]
pub async fn mark_flagged(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    flagged: bool,
) -> Result<(), CommandError> {
    account_store_off_pump(app, account_id, move |app, store| {
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
            // Review (wave 3): the bump follows a REAL change only —
            // re-marking read mail must not reload every view.
            bump_view_generation(app);
        }
        Ok(())
    })
    .await
}

/// R4 (PLAN-RETOURS-7): pins or unpins the message's conversation —
/// LOCAL data (IMAP has no such concept; `\Flagged` is the flag, a
/// different semantics). Returns the new state.
#[tauri::command]
pub async fn toggle_pin(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<bool, CommandError> {
    account_store_off_pump(app, account_id, move |app, store| {
        let Some(state) = store
            .sync_state(account_id, &mailbox)
            .map_err(|err| err.to_string())?
        else {
            return Ok(false);
        };
        let out = store.toggle_pin(state.mailbox_id, uid, epoch_now())?;
        bump_view_generation(app);
        Ok(out)
    })
    .await
}

fn epoch_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The epoch bound of the BODY pumps for an account (ADR 0029,
/// PLAN-HORIZON-NETTOYAGE D1): the import horizon read from the pref,
/// derived at READ time — the bound follows the clock. Envelopes and
/// thread headers stay whole, only the bodies are bounded. Best effort:
/// a failed read bounds nothing — never a silent loss on an error.
fn body_horizon(store: &Store, account_id: i64) -> i64 {
    match store.horizon_import(account_id) {
        Ok(value) => mail_core::horizon_epoch(&value, epoch_now()),
        Err(err) => {
            // §9: the failure is SAID (readable trace via run-wind.ps1),
            // even when the fallback is safe.
            crate::trace::trace(&format!(
                "horizon_import unreadable (account {account_id}): {err}; importing in full out of caution"
            ));
            mail_core::NO_HORIZON
        }
    }
}

/// R1 (PLAN-RETOURS-11, D1-D2): remembers "Show images" for THIS
/// message — envelope key, the guard will not ask again.
#[tauri::command]
pub async fn allow_images_message(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    version: MessageVersion,
) -> Result<(), CommandError> {
    account_store_off_pump(app, account_id, move |_, store| {
        store.set_images_message_checked(
            &version.identity(account_id, &mailbox),
            uid,
            true,
            epoch_now(),
        )?;
        Ok(())
    })
    .await
}

/// D3: "Always show images from this sender" — the address is resolved
/// from the ENVELOPE on the core side (the UI never parses an address),
/// normalized, workstation-wide. Returns the address set (None: an
/// envelope without an address, nothing is written).
#[tauri::command]
pub async fn allow_images_sender(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    version: MessageVersion,
) -> Result<Option<String>, CommandError> {
    account_store_off_pump(app, account_id, move |_, store| {
        Ok(store.allow_images_sender_checked(
            &version.identity(account_id, &mailbox),
            uid,
            epoch_now(),
        )?)
    })
    .await
}

/// Removes only the message grant; true means a sender rule still permits images.
#[tauri::command]
pub async fn revoke_images_message(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    version: MessageVersion,
) -> Result<bool, CommandError> {
    account_store_off_pump(app, account_id, move |_, store| {
        Ok(store.set_images_message_checked(
            &version.identity(account_id, &mailbox),
            uid,
            false,
            epoch_now(),
        )?)
    })
    .await
}

/// D4: the sender rules, for the Settings list.
#[tauri::command]
pub async fn images_senders(app: AppHandle) -> Result<Vec<String>, CommandError> {
    read_store_off_pump(app, move |_, store| Ok(store.images_senders()?)).await
}

/// D4: removes a sender rule — the exit door of "always".
#[tauri::command]
pub async fn revoke_images_sender(app: AppHandle, address: String) -> Result<(), CommandError> {
    store_off_pump(app, move |_, store| {
        Ok(store.revoke_images_sender(&address)?)
    })
    .await
}

/// PLAN-MODE-ORGANISE E1 — the state of organized mode (D2 amended:
/// SQLite `prefs`, the core reads the state) and its retention bound
/// (the epoch of first activation, D3 "arrivals only").
#[tauri::command]
pub async fn organized_mode_get(app: AppHandle) -> Result<bool, CommandError> {
    store_off_pump(app, move |_, store| Ok(store.organized_mode()?)).await
}

/// Toggles organized mode. The first-activation bound is written on the
/// core side, in the same gesture — the UI never carries the epoch.
#[tauri::command]
pub async fn organized_mode_set(app: AppHandle, active: bool) -> Result<(), CommandError> {
    store_off_pump(app, move |_, store| {
        Ok(store.set_organized_mode(active, epoch_now())?)
    })
    .await
}

/// The import horizon of an account (ADR 0029, D3: adjustable after the
/// fact). Absent = "everything" — the safe default, on the core side.
#[tauri::command]
pub async fn horizon_import_get(app: AppHandle, account_id: i64) -> Result<String, CommandError> {
    store_off_pump(app, move |_, store| {
        Ok(store
            .horizon_import(account_id)
            .map(|h| crate::wire::category_to_wire(&h))?)
    })
    .await
}

/// Sets the import horizon (closed vocabulary, rejected on the core
/// side). Extending it makes bodies eligible — the pump catches them up
/// on its next pass; narrowing it erases NOTHING already local.
#[tauri::command]
pub async fn horizon_import_set(
    app: AppHandle,
    account_id: i64,
    value: String,
) -> Result<(), CommandError> {
    account_store_off_pump(app, account_id, move |_, store| {
        Ok(store.set_horizon_import(account_id, &crate::wire::category_from_wire(&value))?)
    })
    .await
}

/// RETOURS-13 R5/R9 — the default actions of the Screener's Yes/No
/// buttons (shipped: Inbox / Trash), adjustable in Settings.
#[derive(serde::Serialize)]
pub struct ScreenerDefaults {
    pub yes: String,
    pub no: String,
}

#[tauri::command]
pub async fn screener_defaults_get(app: AppHandle) -> Result<ScreenerDefaults, CommandError> {
    store_off_pump(app, move |_, store| {
        let (yes, no) = store.screener_defaults().map_err(|err| err.to_string())?;
        Ok(ScreenerDefaults {
            yes: crate::wire::category_to_wire(&yes),
            no: crate::wire::no_default_to_wire(&no),
        })
    })
    .await
}

/// The vocabulary is closed and rejected on the core side — the UI
/// cannot write a broken default.
#[tauri::command]
pub async fn screener_defaults_set(
    app: AppHandle,
    yes: String,
    no: String,
) -> Result<(), CommandError> {
    store_off_pump(app, move |_, store| {
        Ok(store.set_screener_defaults(
            &crate::wire::category_from_wire(&yes),
            &crate::wire::no_default_from_wire(&no),
        )?)
    })
    .await
}

/// A row of the Screener's history, as the UI shows it.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingPayload {
    pub address: String,
    pub destination: String,
    pub rule: Option<String>,
    pub epoch: i64,
}

/// The Screener's verdict on a sender (bare/routed Yes, bare/ruled No,
/// "Move to…") — closed vocabulary, rejected on the core side before
/// any write.
#[tauri::command]
pub async fn route_sender(
    app: AppHandle,
    address: String,
    destination: String,
    rule: Option<String>,
) -> Result<(), CommandError> {
    store_off_pump(app, move |app, store| {
        let (destination, rule) =
            crate::wire::destination_rule_from_wire(&destination, rule.as_deref());
        store.route_sender(&address, &destination, rule.as_deref(), epoch_now())?;
        bump_view_generation(app);
        Ok(())
    })
    .await
}

/// "Move to…" (E1): the verdict is set FROM a message — the address is
/// resolved from the envelope on the core side, the UI never parses an
/// address (`allow_images_sender` pattern). Returns the routed address;
/// None = envelope without an address, nothing is written — a real
/// business case the UI must say.
#[tauri::command]
pub async fn route_sender_from(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    destination: String,
    rule: Option<String>,
) -> Result<Option<String>, CommandError> {
    account_store_off_pump(app, account_id, move |app, store| {
        let Some(state) = store
            .sync_state(account_id, &mailbox)
            .map_err(|err| err.to_string())?
        else {
            return Err(format!("unknown mailbox: {mailbox}").into());
        };
        let (destination, rule) =
            crate::wire::destination_rule_from_wire(&destination, rule.as_deref());
        let out = store.route_sender_of(
            state.mailbox_id,
            uid,
            &destination,
            rule.as_deref(),
            epoch_now(),
        )?;
        bump_view_generation(app);
        Ok(out)
    })
    .await
}

/// "Reinstate" from the Screener's history: the verdict disappears.
#[tauri::command]
pub async fn remove_routing(app: AppHandle, address: String) -> Result<(), CommandError> {
    store_off_pump(app, move |app, store| {
        store.remove_routing(&address)?;
        bump_view_generation(app);
        Ok(())
    })
    .await
}

/// The Screener's history — every decision, the most recent first.
#[tauri::command]
pub async fn routings(app: AppHandle) -> Result<Vec<RoutingPayload>, CommandError> {
    store_off_pump(app, move |_, store| {
        Ok(store
            .routings()
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|r| RoutingPayload {
                address: r.address,
                destination: crate::wire::category_to_wire(&r.destination),
                rule: r.rule.as_deref().map(crate::wire::rule_to_wire),
                epoch: r.epoch,
            })
            .collect())
    })
    .await
}

/// A row of the Screener's desk (E2): the waiting address — THE key
/// the verdict will take — and its latest message in list-row format.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenerRow {
    pub address: String,
    pub row: MessageRow,
}

/// The Screener's desk: one row per waiting sender, most recent first.
/// Empty as long as the mode has never been activated.
#[tauri::command]
pub async fn screener_waiting(app: AppHandle) -> Result<Vec<ScreenerRow>, CommandError> {
    read_store_off_pump(app, move |_, store| {
        Ok(store
            .screener_waiting()
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|entry| ScreenerRow {
                address: entry.address,
                row: to_message_row(entry.row),
            })
            .collect())
    })
    .await
}

/// The Screener badge: how many MESSAGES are waiting at the desk (the
/// prototype's design — nav and light reloads).
#[tauri::command]
pub async fn screener_total(app: AppHandle) -> Result<u64, CommandError> {
    store_off_pump(app, move |_, store| Ok(store.screener_total()?)).await
}

/// RETOURS-14 R4 (review) — the addresses waiting at the desk, bare:
/// the thread badge compares identities, it does not paint rows.
#[tauri::command]
pub async fn screener_addresses(app: AppHandle) -> Result<Vec<String>, CommandError> {
    store_off_pump(app, move |_, store| Ok(store.screener_addresses()?)).await
}

/// RETOURS-14 R7 (D8) — the Feed badge: how many cards have NEVER been
/// opened (the `kiosque_lus` memory, the page's own semantics — never
/// IMAP `unseen`). Global, like `screener_total`.
#[tauri::command]
pub async fn feed_unopened(app: AppHandle) -> Result<u64, CommandError> {
    store_off_pump(app, move |_, store| Ok(store.feed_unopened(None)?)).await
}

// ---------------------------------------------------------------------
// Spring cleaning (PLAN-HORIZON-NETTOYAGE part B) — the session, the
// groups, the group verdict. Closed vocabularies, rejected on the
// core side.
// ---------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupSessionPayload {
    pub range: String,
    pub scope: String,
    pub total: u64,
    pub processed: u64,
}

impl From<mail_core::CleanupSession> for CleanupSessionPayload {
    fn from(s: mail_core::CleanupSession) -> Self {
        CleanupSessionPayload {
            range: crate::wire::category_to_wire(&s.range),
            scope: crate::wire::scope_to_wire(&s.scope),
            total: s.total,
            processed: s.handled,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupGroupPayload {
    pub address: String,
    pub qui: Option<String>,
    pub messages: u64,
    pub last_epoch: i64,
    pub last_subject: Option<String>,
}

impl From<mail_core::CleanupGroup> for CleanupGroupPayload {
    fn from(g: mail_core::CleanupGroup) -> Self {
        CleanupGroupPayload {
            address: g.address,
            qui: g.who,
            messages: g.messages,
            last_epoch: g.last_epoch,
            last_subject: g.last_subject,
        }
    }
}

/// RETOURS-14 R6 (D7) — a group of the Paper trail: the sender, its
/// threads, the recency and the subject of the last message.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperTrailGroupPayload {
    pub address: String,
    pub qui: Option<String>,
    pub threads: u64,
    pub last_epoch: i64,
    pub last_subject: Option<String>,
}

impl From<mail_core::PaperTrailGroup> for PaperTrailGroupPayload {
    fn from(g: mail_core::PaperTrailGroup) -> Self {
        PaperTrailGroupPayload {
            address: g.address,
            qui: g.who,
            threads: g.threads,
            last_epoch: g.last_epoch,
            last_subject: g.last_subject,
        }
    }
}

/// The groups of the Paper trail — a sender × their threads, recency
/// at the top (D7, the pattern of Cleanup).
#[tauri::command]
pub async fn paper_trail_groups(
    app: AppHandle,
    account_id: Option<i64>,
) -> Result<Vec<PaperTrailGroupPayload>, CommandError> {
    read_store_off_pump(app, move |_, store| {
        Ok(store
            .paper_trail_groups(account_id)
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(Into::into)
            .collect())
    })
    .await
}

/// The page of a Paper trail group — the threads of THIS one sender,
/// enriched like any list page (invitations).
#[tauri::command]
pub async fn paper_trail_group_page(
    app: AppHandle,
    address: String,
    account_id: Option<i64>,
    offset: usize,
    limit: usize,
) -> Result<Vec<MessageRow>, CommandError> {
    store_off_pump(app, move |_, store| {
        let limit = limit.min(LIST_LIMIT_MAX);
        let mut rows = store
            .paper_trail_group_scoped(&address, account_id, offset, limit)
            .map_err(|err| err.to_string())?;
        store
            .enrich_rows(&mut rows)
            .map_err(|err| err.to_string())?;
        Ok(rows.into_iter().map(to_message_row).collect())
    })
    .await
}

/// The current session — `null`: nothing started (the intro screen).
#[tauri::command]
pub async fn cleanup_state(app: AppHandle) -> Result<Option<CleanupSessionPayload>, CommandError> {
    store_off_pump(app, move |_, store| {
        Ok(store
            .cleanup_state()
            .map_err(|err| err.to_string())?
            .map(Into::into))
    })
    .await
}

/// Starts (or replaces) the session: the bound freezes here.
#[tauri::command]
pub async fn cleanup_start(
    app: AppHandle,
    range: String,
    scope: String,
) -> Result<CleanupSessionPayload, CommandError> {
    store_off_pump(app, move |_, store| {
        Ok(store
            .cleanup_start(
                &crate::wire::category_from_wire(&range),
                &crate::wire::scope_from_wire(&scope),
                epoch_now(),
            )
            .map(Into::into)?)
    })
    .await
}

/// The remaining groups (sender × mail of the range), most recent
/// first.
#[tauri::command]
pub async fn cleanup_groups(app: AppHandle) -> Result<Vec<CleanupGroupPayload>, CommandError> {
    read_store_off_pump(app, move |_, store| {
        // The measurement due since HORIZON-NETTOYAGE ("cost on a real
        // 200k base"), readable afterwards in `wind.log` — count and
        // duration, never an address (§6.8).
        let start = std::time::Instant::now();
        let groups = store.cleanup_groups().map_err(|err| err.to_string())?;
        crate::trace::trace(&format!(
            "cleanup: {} groups in {} ms",
            groups.len(),
            start.elapsed().as_millis()
        ));
        Ok(groups.into_iter().map(Into::into).collect())
    })
    .await
}

/// The mail of a group — VIEW, never sort per message.
#[tauri::command]
pub async fn cleanup_messages(
    app: AppHandle,
    address: String,
) -> Result<Vec<MessageRow>, CommandError> {
    store_off_pump(app, move |_, store| {
        Ok(store
            .cleanup_messages(&address)
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(to_message_row)
            .collect())
    })
    .await
}

/// The GROUP verdict (D5: the stock of the range AND the future) —
/// returns the up-to-date state, the progress bar follows in the same
/// round trip.
#[tauri::command]
pub async fn cleanup_verdict(
    app: AppHandle,
    address: String,
    destination: String,
    rule: Option<String>,
) -> Result<Option<CleanupSessionPayload>, CommandError> {
    store_off_pump(app, move |app, store| {
        for account in store
            .cleanup_messages(&address)?
            .iter()
            .map(|row| row.account_id)
            .collect::<std::collections::BTreeSet<_>>()
        {
            admit_account(app, store, account)?;
        }
        let (destination, rule) =
            crate::wire::destination_rule_from_wire(&destination, rule.as_deref());
        store
            .cleanup_verdict(&address, &destination, rule.as_deref(), epoch_now())
            .map_err(|err| err.to_string())?;
        let out = store
            .cleanup_state()
            .map_err(|err| err.to_string())?
            .map(Into::into);
        bump_view_generation(app);
        Ok(out)
    })
    .await
}

/// Closes the session — the verdicts stay set (routing).
#[tauri::command]
pub async fn cleanup_finish(app: AppHandle) -> Result<(), CommandError> {
    store_off_pump(app, move |_, store| Ok(store.cleanup_finish()?)).await
}

/// E5 — the "Set aside / Resume" toggle: the state applies to the
/// THREAD (pattern of the pin), returned AFTER the gesture.
#[tauri::command]
pub async fn toggle_set_aside(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<bool, CommandError> {
    account_store_off_pump(app, account_id, move |app, store| {
        let Some(state) = store
            .sync_state(account_id, &mailbox)
            .map_err(|err| err.to_string())?
        else {
            return Err(format!("unknown mailbox: {mailbox}").into());
        };
        let out = store.toggle_set_aside(state.mailbox_id, uid, epoch_now())?;
        bump_view_generation(app);
        Ok(out)
    })
    .await
}

/// The pile (E5): the heads of the set-aside threads — the fan-out and
/// the table use them as they are.
#[tauri::command]
pub async fn set_aside_pile(app: AppHandle) -> Result<Vec<MessageRow>, CommandError> {
    store_off_pump(app, move |_, store| {
        let mut rows = store.set_aside_pile().map_err(|err| err.to_string())?;
        store
            .enrich_rows(&mut rows)
            .map_err(|err| err.to_string())?;
        Ok(rows
            .into_iter()
            .map(to_message_row)
            .map(|mut row| {
                row.aside = true;
                row
            })
            .collect())
    })
    .await
}

/// A Feed card (E5bis): the row AND its sanitized body — "the letters
/// arrive already open," scrolling reads without clicking. `document`
/// is the SAME auto-CSP document as the reading screen (mail_render,
/// iframe sandbox S1); None = body not cached yet (the card shows the
/// preview, the normal backfill will follow — D5: preloading is
/// bounded to the SERVED page, never one network call per card).
#[derive(Serialize)]
pub struct FeedCard {
    pub images_message_allowed: bool,
    pub row: MessageRow,
    pub document: Option<String>,
    pub remote_images_blocked: usize,
    /// RETOURS-13 R10: the card has already been read to the bottom —
    /// the "Previously read" section uses it when the page is SERVED
    /// (never in flight: a card doesn't jump while being read).
    pub read: bool,
}

/// The Feed page as CARDS (E5bis, D5/S3): the rows of the routed view
/// + their bodies read from the CACHE only (S3: 12.2 ms cold for a
/// page of 20), sanitized by THE reading gate — image guard consulted
/// per message (authority at the core, R1).
#[tauri::command]
pub async fn feed_cards(
    app: AppHandle,
    account_id: Option<i64>,
    offset: usize,
    limit: usize,
) -> Result<Vec<FeedCard>, CommandError> {
    // Under the lock, the SQLite reads only: rows, flags, cached bodies
    // and the image guard; the page's sanitizes run unlocked (Lot 5
    // E13d — a page of letters is the heaviest sanitize of the app). A
    // read snapshot that nothing writes after: no second look.
    let snapshot = read_store_off_pump(app, move |_, store| {
        let limit = limit.min(LIST_LIMIT_MAX);
        let mut rows = store
            .routing_unified_scoped("kiosque", account_id, false, offset, limit)
            .map_err(|err| err.to_string())?;
        store
            .enrich_rows(&mut rows)
            .map_err(|err| err.to_string())?;
        let mut inputs = Vec::with_capacity(rows.len());
        for row in rows {
            let row = to_message_row(row);
            row.version.verify(store, row.account_id, &row.mailbox)?;
            let mailbox_id = Some(row.version.mailbox_id);
            // R10: the "read" flag of the card — a PK probe, one per card.
            let read = mailbox_id
                .map(|id| store.feed_read(id, row.uid))
                .transpose()
                .map_err(|err| err.to_string())?
                .unwrap_or(false);
            // CACHE ONLY — an offline Feed reads as it is.
            let body = store
                .body(row.account_id, &row.mailbox, row.uid)
                .map_err(|err| err.to_string())?;
            let body = match body {
                Some(html) => {
                    let granted = mailbox_id
                        .map(|id| store.images_allowed(id, row.uid))
                        .transpose()
                        .map_err(|err| err.to_string())?
                        .unwrap_or(false);
                    Some((html, granted))
                }
                None => None,
            };
            let images_message_allowed = mailbox_id
                .map(|id| store.images_allowed_message(id, row.uid))
                .transpose()?
                .unwrap_or(false);
            inputs.push((row, read, body, images_message_allowed));
        }
        Ok(inputs)
    })
    .await?;
    unlocked(move || {
        let mut cards = Vec::with_capacity(snapshot.len());
        for (row, read, body, images_message_allowed) in snapshot {
            let (document, remote_images_blocked) = match body {
                Some((html, granted)) => {
                    let (document, blocked) = render_document(&html, granted);
                    (Some(document), blocked)
                }
                None => (None, 0),
            };
            cards.push(FeedCard {
                images_message_allowed,
                row,
                document,
                remote_images_blocked,
                read,
            });
        }
        Ok::<_, CommandError>(cards)
    })
    .await
}

/// RETOURS-13 R10 — a Feed card scrolled to the bottom marks itself
/// read (idempotent; addressing pattern of `toggle_set_aside`).
#[tauri::command]
pub async fn feed_mark_read(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<(), CommandError> {
    account_store_off_pump(app, account_id, move |app, store| {
        let Some(state) = store
            .sync_state(account_id, &mailbox)
            .map_err(|err| err.to_string())?
        else {
            return Err(format!("unknown mailbox: {mailbox}").into());
        };
        store.mark_feed_read(state.mailbox_id, uid, epoch_now())?;
        bump_view_generation(app);
        Ok(())
    })
    .await
}

/// The pinned conversations of the Inbox (D4: Inbox only), served
/// SEPARATELY — the front prepends them to page 0, the paginated flow
/// excludes them (D5).
#[tauri::command]
pub async fn pinned_rows(
    app: AppHandle,
    account_id: Option<i64>,
    unread: bool,
) -> Result<Vec<MessageRow>, CommandError> {
    store_off_pump(app, move |_, store| {
        // E2: the prepended section follows the shared exclusion of the
        // organized Inbox — a routed pinned item lives in its own view.
        let organized = store.organized_mode().map_err(|err| err.to_string())?;
        let mut rows = store
            .pinned_unified_scoped(account_id, unread, organized)
            .map_err(|err| err.to_string())?;
        store
            .enrich_rows(&mut rows)
            .map_err(|err| err.to_string())?;
        Ok(rows
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
// Compose, reply, send — the outbox (Phases 2-3).
// ---------------------------------------------------------------------

#[derive(Serialize)]
pub struct ComposeContext {
    pub account_id: i64,
    /// The mailbox of the message being replied to. It travels with the
    /// send: without it, the UID alone no longer suffices to find the
    /// `Message-ID` to cite.
    pub mailbox: String,
    pub uid: u32,
    /// Empty for a forward: the user picks the recipient.
    pub to: String,
    /// Pre-filled Cc — "Reply all" puts the original Cc back here (D3);
    /// empty for a plain reply or a forward.
    pub cc: String,
    pub subject: String,
    /// Pre-filled RICH citation (PLAN-COMPOSITION-HTML): attribution
    /// then a blockquote of the body sanitized `BlockRemote` (nothing
    /// placed back into the editor loads the network — §6.4); empty if
    /// the body is unreachable (we reply without a citation).
    /// The user writes above it (top-posting); the send's text/plain
    /// fallback is derived from the same HTML by `body_boundary`.
    pub body_html: String,
    /// `true`: the send will carry In-Reply-To (a reply within the
    /// thread).
    pub reply: bool,
}

#[derive(Serialize)]
pub struct OutboxSummary {
    pub sent: usize,
    pub deferred: usize,
    pub rejected: usize,
    pub quarantined: usize,
    /// Remaining in the queue after the flush (all accounts).
    pub queued: usize,
    /// SMTP connection impossible (offline, token…) — the queue waits.
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct OutboxEntry {
    pub id: i64,
    pub message_id: String,
    pub subject: String,
    pub to: String,
    pub state: String,
    pub attempts: u32,
    pub error: Option<String>,
    /// How many attachments the log carries for this send (PJ-D2) —
    /// quarantine and refusal must be able to say what would go out
    /// again.
    pub attachments: usize,
    /// R2: the due time of a scheduled send (epoch seconds) — `None`
    /// for an ordinary send. The UI derives "scheduled for {h}" and the
    /// cancel gesture from it.
    pub send_at_epoch: Option<i64>,
}

#[derive(Serialize)]
pub struct ActionIncidentRow {
    id: i64,
    account: String,
    subject: Option<String>,
    sender: Option<String>,
    source: String,
    destination: Option<String>,
    reason: String,
}

#[tauri::command]
pub async fn dismiss_action_incident(app: AppHandle, id: i64) -> Result<(), CommandError> {
    store_off_pump(app, move |_, store| Ok(store.dismiss_action_incident(id)?)).await
}

#[derive(Serialize)]
pub struct OutboxStatus {
    pub queued: usize,
    pub interrupted: usize,
    pub rejected: usize,
    /// Held after a restore from a copy (Lot 5 E14b): each one waits
    /// for the user's word before it goes out.
    pub held: usize,
    /// R2: scheduled sends NOT YET due — kept apart from `queued`,
    /// otherwise the status bar would say "waiting" about a send that
    /// is waiting for its time, not the network (a lie).
    pub scheduled: usize,
    /// The nearest due time among the scheduled sends — the front's
    /// probe triggers the flush once it passes.
    pub next_scheduled_epoch: Option<i64>,
    /// Everything but the successful sends, in emission order.
    pub entries: Vec<OutboxEntry>,
    /// PLAN-AUDIT-V1 E3 (D2): log actions in QUARANTINE (server
    /// refusal, or five failures) — all accounts. The slot says so; the
    /// intent is no longer lost in silence.
    pub refused_actions: u64,
    pub refused_action_reason: Option<String>,
    pub action_incidents: Vec<ActionIncidentRow>,
}

/// Pre-filling a reply: recipient = the sender's raw address, subject
/// prefixed with "Re: " exactly once, body cited. The citation is a
/// courtesy: an unreachable body means we reply without it.
#[tauri::command]
pub async fn reply_context(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    version: MessageVersion,
    edit_token: Option<String>,
) -> Result<ComposeContext, CommandError> {
    let (envelope, own) = enveloppe_et_compte(&app, account_id, &mailbox, uid, version).await?;
    let repondre_a = reply_to_of(&app, account_id, &mailbox, uid, version).await?;
    // Our own message? (the sender is the account) — the core's rule
    // (Lot 5 E13c). R4 (field finding): on one's own message, replying
    // targets the original recipients (the To); otherwise, the sender.
    let is_own = mail_core::is_own_message(envelope.sender_address.as_deref(), &own);
    let mut recipients = mail_core::reply_to(
        is_own,
        envelope.sender_address.as_deref(),
        &envelope.to_addrs,
        repondre_a.as_deref(),
    );
    // A send to oneself with no recipients in the database (old, not
    // backfilled): poll the server ONCE — same fallback as "reply
    // all," never an empty "To" on one's own message.
    if recipients.is_empty() && is_own {
        let session = off_pump(app.clone(), move |app| auth_for(&app, account_id)).await?;
        let mailbox_name = mailbox.clone();
        let fetched = tauri::async_runtime::spawn_blocking(move || {
            fetch_recipients_remote(&session, &mailbox_name, uid, version)
        })
        .await
        .map_err(|err| err.to_string())??;
        recipients = fetched.to;
    }
    if recipients.is_empty() {
        return Err("unknown recipient: resync the mailbox".into());
    }
    let to = recipients.join(", ");
    let body_html = citation_reply(&app, account_id, &mailbox, uid, version, &envelope).await;
    finish_reply_context(&app, account_id, &mailbox, uid, version, edit_token).await?;
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

async fn finish_reply_context(
    app: &AppHandle,
    account_id: i64,
    mailbox: &str,
    uid: u32,
    version: MessageVersion,
    edit_token: Option<String>,
) -> Result<(), CommandError> {
    let identity = version.identity(account_id, mailbox);
    store_off_pump(app.clone(), move |_, store| {
        match edit_token {
            Some(token) => store.set_draft_edit_reply(&token, &identity, uid)?,
            None => store.verify_mailbox_identity(&identity)?,
        }
        Ok(())
    })
    .await
}

/// The envelope of a message and its account's address, in ONE pass
/// under `off_pump` (E5) — the common matter of the three compose
/// contexts.
/// The message's `Reply-To`, read on demand (PLAN-AUDIT-V2 E5).
async fn reply_to_of(
    app: &AppHandle,
    account_id: i64,
    mailbox: &str,
    uid: u32,
    version: MessageVersion,
) -> Result<Option<String>, CommandError> {
    let mailbox_name = mailbox.to_string();
    store_off_pump(app.clone(), move |_, store| {
        version.verify(store, account_id, &mailbox_name)?;
        Ok(store.reply_to_of(account_id, &mailbox_name, uid)?)
    })
    .await
}

async fn enveloppe_et_compte(
    app: &AppHandle,
    account_id: i64,
    mailbox: &str,
    uid: u32,
    version: MessageVersion,
) -> Result<(mail_core::Envelope, String), CommandError> {
    let mailbox_name = mailbox.to_string();
    store_off_pump(app.clone(), move |_, store| {
        version.verify(store, account_id, &mailbox_name)?;
        let envelope = store
            .envelope(account_id, &mailbox_name, uid)?
            .ok_or_else(|| CommandError::new("message not found"))?;
        let own = account_email(store, account_id)?;
        Ok((envelope, own))
    })
    .await
}

/// The rich citation of a reply — an unreachable body yields an empty
/// citation (we reply without it). The cited body is sanitized
/// `BlockRemote`: this string will be PLACED BACK into the editor
/// (`innerHTML`, main document) — a remote image there would load the
/// message's tracking pixel at the mere click of "Reply" (§6.4).
async fn citation_reply(
    app: &AppHandle,
    account_id: i64,
    mailbox: &str,
    uid: u32,
    version: MessageVersion,
    envelope: &mail_core::Envelope,
) -> String {
    let Ok(html) = raw_body(app, account_id, mailbox, uid, version).await else {
        return String::new();
    };
    // Sanitizing is CPU-bound (a 28 MB body, D-1): on a blocking
    // worker, never on the async worker (E5) — and not under the
    // commands' lock either (Lot 5 E13d): the quote needs no store.
    let sender = envelope.sender.clone();
    let date = quote_date(envelope);
    unlocked(move || {
        Ok::<_, CommandError>(mail_core::quote_reply_html(
            sender.as_deref(),
            date.as_deref(),
            &mail_render::sanitize(&html).html,
        ))
    })
    .await
    .unwrap_or_default()
}

/// Pre-filling a "Reply all": sender + To + Cc of the original
/// message, without duplicates or one's own address.
///
/// The To/Cc recipients are now STORED in the envelope (R4, from the
/// same ENVELOPE as the sender): they are read first — instant, offline
/// included (R1, PLAN-RETOURS-MAIL). The field showed the cause of an
/// empty "To" lasting >10 s: the old path opened an authenticated IMAP
/// connection on EVERY click. We only fall back to it when the message
/// doesn't have its recipients in the database yet (old send, not
/// backfilled); there, the failure stays PLAIN — an amputated "reply
/// all" would send to fewer people than promised. The citation stays a
/// courtesy: an unreachable body means we reply without it.
#[tauri::command]
pub async fn reply_all_context(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    version: MessageVersion,
    edit_token: Option<String>,
) -> Result<ComposeContext, CommandError> {
    let (envelope, own) = enveloppe_et_compte(&app, account_id, &mailbox, uid, version).await?;
    let repondre_a = reply_to_of(&app, account_id, &mailbox, uid, version).await?;
    // Recipients known in the database: instant path, no network. Not
    // empty = "backfilled" (a receipt always carries at least oneself in
    // To); empty = not backfilled yet, poll the server once.
    let (to_list, cc_list) = if !envelope.to_addrs.is_empty() || !envelope.cc_addrs.is_empty() {
        (envelope.to_addrs.clone(), envelope.cc_addrs.clone())
    } else {
        let session = off_pump(app.clone(), move |app| auth_for(&app, account_id)).await?;
        let mailbox_name = mailbox.clone();
        let fetched = tauri::async_runtime::spawn_blocking(move || {
            fetch_recipients_remote(&session, &mailbox_name, uid, version)
        })
        .await
        .map_err(|err| err.to_string())??;
        (fetched.to, fetched.cc)
    };
    // The whole decision is the core's (Lot 5 E13c, audit A01): own
    // message, Reply-To precedence, To/Cc split, self-only fallback.
    let (to, cc) = mail_core::reply_all_recipients(
        envelope.sender_address.as_deref(),
        repondre_a.as_deref(),
        &to_list,
        &cc_list,
        &own,
    );
    if to.is_empty() && cc.is_empty() {
        return Err("unknown sender address: resync the mailbox".into());
    }
    let body_html = citation_reply(&app, account_id, &mailbox, uid, version, &envelope).await;
    finish_reply_context(&app, account_id, &mailbox, uid, version, edit_token).await?;
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
    session: &AccountWork,
    mailbox: &str,
    uid: u32,
    version: MessageVersion,
) -> Result<mail_core::MessageRecipients, String> {
    let (mut server, _refreshed, _lease) = crate::poll::connect_imap(session)?;
    // `fetch_recipients` is gone (PLAN-AUDIT-V3 E6, a unit duplicate of
    // this same ENVELOPE re-read): a one-UID `fetch_envelopes` carries
    // the same To/Cc, already parsed into `to_addrs`/`cc_addrs` (R4).
    mail_core::verify_mailbox_generation(&mut server, mailbox, version.uid_validity)
        .map_err(|err| err.to_string())?;
    let envelopes = server
        .fetch_envelopes(mailbox, &[uid])
        .map_err(|err| err.to_string());
    mail_core::verify_mailbox_generation(&mut server, mailbox, version.uid_validity)
        .map_err(|err| err.to_string())?;
    server.logout();
    let envelope = envelopes?
        .into_iter()
        .find(|envelope| envelope.uid == uid)
        .ok_or_else(|| "message not found on the server".to_string())?;
    Ok(mail_core::MessageRecipients {
        to: envelope.to_addrs,
        cc: envelope.cc_addrs,
    })
}

/// Pre-filling a forward: without a body, a forward would transmit
/// nothing — here the failure is blocking. New thread: no In-Reply-To.
/// Attachment retrieval is coordinated separately by the composer.
#[tauri::command]
pub async fn forward_context(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    version: MessageVersion,
) -> Result<ComposeContext, CommandError> {
    let (envelope, _own) = enveloppe_et_compte(&app, account_id, &mailbox, uid, version).await?;
    let html = raw_body(&app, account_id, &mailbox, uid, version).await?;
    let mailbox2 = mailbox.clone();
    store_off_pump(app, move |_, store| {
        Ok(version.verify(store, account_id, &mailbox2)?)
    })
    .await?;
    // The editor prepares inert image references before DOM insertion.
    // The sanitize of the quoted body runs unlocked (Lot 5 E13d): the
    // identity was verified above, nothing is written after.
    let subject = mail_core::forward_subject(envelope.subject.as_deref());
    let body_html = unlocked(move || {
        Ok::<_, CommandError>(mail_core::quote_forward_html(
            envelope.sender.as_deref(),
            quote_date(&envelope).as_deref(),
            envelope.subject.as_deref(),
            &mail_render::sanitize_with(&html, mail_render::ImagePolicy::AllowRemote).html,
        ))
    })
    .await?;
    Ok(ComposeContext {
        account_id,
        mailbox,
        uid,
        to: String::new(),
        cc: String::new(),
        subject,
        body_html,
        reply: false,
    })
}

/// Date formatted for the attribution line of a citation.
fn quote_date(envelope: &mail_core::Envelope) -> Option<String> {
    envelope
        .date
        .map(|date| date.format("%Y-%m-%d %H:%M").to_string())
}

/// Logs the send into the sending account's outbox — BEFORE any
/// network attempt (the "never a lost send" rule).
#[tauri::command]
// The arguments of a Tauri command are NAMED at the call site (a JS
// object): the silent swap the lint targets cannot happen here.
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
    image_sources: Option<BTreeMap<String, String>>,
    reply_to_mailbox: Option<String>,
    reply_to_uid: Option<u32>,
    draft_id: Option<i64>,
    important: bool,
    // R2: the due time (epoch seconds) of a deferred send — None =
    // right away, the historical path.
    send_at_epoch: Option<i64>,
    edit_token: Option<String>,
) -> Result<(), String> {
    // The wire stays FLAT (the IPC keys are a contract since E5a); the
    // arguments are packed HERE and travel as one value from here on
    // (audit 3.4 — `queue_send(DraftContent)`).
    let content = SendContent {
        to,
        cc,
        bcc,
        subject,
        body,
        body_html,
        image_sources,
        reply_to_mailbox,
        reply_to_uid,
        draft_id,
        important,
        send_at_epoch,
        edit_token,
    };
    queue_send_content(app, account_id, content)
        .await
        .map_err(String::from)
}

#[tauri::command]
pub async fn queued_draft_edit(
    app: AppHandle,
    account_id: i64,
    token: String,
) -> Result<Option<i64>, CommandError> {
    store_off_pump(app, move |_, store| {
        store
            .queued_draft_edit(&token, account_id)
            .map_err(CommandError::from)
    })
    .await
}

/// The full content of one queued send, packed off the wire.
struct SendContent {
    to: String,
    cc: String,
    bcc: String,
    subject: String,
    body: String,
    body_html: Option<String>,
    image_sources: Option<BTreeMap<String, String>>,
    reply_to_mailbox: Option<String>,
    reply_to_uid: Option<u32>,
    draft_id: Option<i64>,
    important: bool,
    send_at_epoch: Option<i64>,
    edit_token: Option<String>,
}

async fn queue_send_content(
    app: AppHandle,
    account_id: i64,
    content: SendContent,
) -> Result<(), CommandError> {
    let SendContent {
        to,
        cc,
        bcc,
        subject,
        body,
        body_html,
        image_sources,
        reply_to_mailbox,
        reply_to_uid,
        draft_id,
        important,
        send_at_epoch,
        edit_token,
    } = content;
    account_store_off_pump(app, account_id, move |_, store| {
        let from = account_email(store, account_id)?;
        // Rich body: THE boundary (`body_boundary`) — sanitized, text
        // derived. The `body` received serves only the text path.
        let (text_body, rich_body) = composition_boundary(
            body,
            body_html.as_deref(),
            &image_sources.unwrap_or_default(),
        );
        // Without the mailbox, we resolve NOTHING — we don't guess.
        //
        // A UID alone no longer designates a message now that the
        // account has two (ADR 0009): INBOX's #1 and "Sent"'s #1 are two
        // messages. Guessing would produce an `In-Reply-To` pointing at
        // a stranger, hence a reply grafted onto someone else's
        // conversation. Omitting it splits a thread — "a thread split in
        // two is repairable and honest; two unrelated messages merged
        // are not" (ADR 0008 §2).
        let parent = if edit_token.is_none() {
            reply_to_uid.zip(reply_to_mailbox)
        } else {
            None
        };
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
            &text_body,
            in_reply_to.as_deref(),
        )?;
        // E7: the entire References chain (RFC 5322 §3.6.4) — the core
        // knows it, the adapter copies it over.
        draft.references = parent.as_ref().and_then(|(uid, mailbox)| {
            store
                .references_of(account_id, mailbox, *uid)
                .ok()
                .flatten()
        });
        draft.body_html = rich_body;
        draft.important = important;
        // A due time already past counts as "right away": the guard
        // lives here, not in the UI — a datetime left open while the
        // clock kept running must not hold the send back for nothing.
        let due = send_at_epoch.filter(|epoch| *epoch > chrono::Utc::now().timestamp());
        // Anchor draft (attachments in the SAME transaction, PJ-D2) and
        // due time (R2) go through THE single queuing path.
        match edit_token {
            Some(token) => store.enqueue_draft_edit(&token, account_id, &draft, due)?,
            None => store.enqueue_outbox_full(account_id, &draft, draft_id, due)?,
        };
        Ok(())
    })
    .await
}

/// Flushes the outboxes of ALL connected accounts — each through ITS
/// OWN SMTP connection. Offline = a summary, not an error. Reentrancy
/// forbidden (lock).
#[tauri::command]
pub async fn flush_outbox(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<OutboxSummary, CommandError> {
    let (path, jobs) = off_pump(app.clone(), |app| {
        Ok::<_, String>((adopted_db(&app)?, crate::poll::connected_jobs(&app)?))
    })
    .await?;
    let lock = state.outbox_flush.clone();

    let (summary, refreshed) =
        tauri::async_runtime::spawn_blocking(move || run_flush_all(jobs, &path, &lock))
            .await
            .map_err(|err| err.to_string())??;

    reset_sessions(&state, refreshed)?;
    Ok(summary)
}

/// When to retry the targeted Sent poll that reported nothing — a pure
/// decision (PLAN-REACTIVITE E2). Gmail adds the copy ASYNCHRONOUSLY
/// after the SMTP acceptance: the first poll may run BEFORE it lands
/// and honestly report "nothing moved." Two bounded retries (+5 s then
/// +15 s), then silence — the full cycle will catch up; we don't
/// hammer a server that has nothing to give (the anti-hammering lesson
/// from the P0 supplement).
fn retry_after(attempt: u32) -> Option<Duration> {
    match attempt {
        1 => Some(Duration::from_secs(5)),
        2 => Some(Duration::from_secs(15)),
        _ => None,
    }
}

/// The outcome of the after-gesture pass — no more silence: incidents
/// surface to the UI like those of the cycle (field finding 0.1.5: the
/// investigation was blind, everything went into `eprintln` and the
/// `.catch(() => {})` swallowed the rest). `reconciled`: echoes
/// replaced by their real row; `swept`: echoes the server denied.
#[derive(Default, Serialize)]
pub struct PassReport {
    pub fetched: usize,
    pub deleted: usize,
    pub reconciled: usize,
    pub swept: usize,
    pub errors: Vec<String>,
}

/// The after-gesture pass (PLAN-REACTIVITE E3) — reconciling the local
/// echo: after a deletion, an archiving, a move, or a send, the server
/// must follow WITHOUT waiting for the cycle.
///
/// 1. **The intentions first**: mailboxes carrying logged actions get
///    polled — the replay starts NOW (INBOX through the shared
///    `poll_inbox` path: bubbles and counters, nothing gets told
///    twice).
/// 2. **The inventory**: LIST-STATUS (one round trip, E2c) —
///    `must_poll` designates the folders that moved: the gesture's
///    destination, and it alone in practice. The destination is NEVER
///    guessed (Trash RFC 6154, Gmail label: everything shows in
///    STATUS). INBOX stays with the watcher and the cycle — polling it
///    here would steal their bubbles. Fallback without LIST-STATUS:
///    targeted STATUS calls on the canonical destinations only, never
///    the ~50 folders.
/// 3. **The reconciliation**: the echo dies when the real row arrives
///    (same `message_id` in the destination) — the row doesn't jump
///    visibly.
/// 4. **The retry** (E2): echoes still waiting → +5 s then +15 s then
///    silence (asynchronous Gmail copy). Each attempt takes and
///    RELEASES the account lock: the pauses block nothing.
/// 5. **The sweep**: intention settled, destination polled CLEANLY and
///    still no copy → the echo is withdrawn, the incident is reported —
///    we don't display what the server denies. Never after a failed
///    attempt: a server that didn't answer denied nothing.
///
/// `account_id = None` (back online, R-D3): all accounts with work to
/// do — pending actions or echoes. One flight per account, coalesced:
/// archiving ten messages doesn't open ten passes.
#[tauri::command]
pub async fn sync_after_gesture(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: Option<i64>,
) -> Result<PassReport, CommandError> {
    let (path, targets): (PathBuf, Vec<i64>) = off_pump(app.clone(), move |app| {
        let path = adopted_db(&app)?;
        let targets = match account_id {
            Some(id) => vec![id],
            None => {
                let store = Store::open(&path).map_err(|err| err.to_string())?;
                store.accounts_with_work().map_err(|err| err.to_string())?
            }
        };
        Ok::<_, String>((path, targets))
    })
    .await?;
    let mut report = PassReport::default();
    for account in targets {
        let session = match off_pump(app.clone(), move |app| auth_for(&app, account)).await {
            Ok(session) => session,
            Err(reason) => {
                report.errors.push(reason);
                continue;
            }
        };
        let email = session.email().to_string();
        // One flight per account: a pass already in flight ABSORBS the
        // request — the flag will make it replay once, not ten times.
        // The flight is a GUARD (E5): a `?` in the middle of the pass
        // used to leave it in flight forever, and every later pass for
        // the account was absorbed until restart.
        let Some(flight) = FlightGuard::take(&state.gesture_passes, &session.ticket) else {
            continue;
        };
        loop {
            let outcome = {
                let path = path.clone();
                let session = session.clone();
                let cycle = state.sync_cycle.clone();
                let locks = state.poll_locks.clone();
                let app_bubbles = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    pass_after_gesture_account(
                        &path,
                        session,
                        account,
                        &cycle,
                        &locks,
                        &app_bubbles,
                    )
                })
                .await
                .map_err(|err| err.to_string())
                .and_then(|outcome| outcome)
            };
            match outcome {
                Ok((outcome, refreshed)) => {
                    report.fetched += outcome.fetched;
                    report.deleted += outcome.deleted;
                    report.reconciled += outcome.reconciled;
                    report.swept += outcome.swept;
                    report.errors.extend(outcome.errors);
                    reset_sessions(&state, refreshed)?;
                }
                Err(reason) => report.errors.push(format!("{email}: {reason}")),
            }
            // Replay ONCE if a gesture arrived during the pass.
            if !flight.rerequest_consumed() {
                break;
            }
        }
        drop(flight);
    }
    Ok(report)
}

/// The flight guard of an after-gesture pass (E5): `in_flight` falls back
/// down when it's released — by the explicit `drop` as well as by a
/// `?`.
struct FlightGuard<'a> {
    passes: &'a Mutex<HashMap<Ticket, PassFlight>>,
    ticket: Ticket,
}

impl<'a> FlightGuard<'a> {
    /// `None`: a pass is already in flight for this account — the
    /// request is absorbed (the flag will make it replay once).
    fn take(passes: &'a Mutex<HashMap<Ticket, PassFlight>>, ticket: &Ticket) -> Option<Self> {
        let mut table = recovered(passes);
        table.retain(|ticket, flight| flight.in_flight || ticket.check().is_ok());
        let flight = table.entry(ticket.clone()).or_default();
        if flight.in_flight {
            flight.rerequest = true;
            return None;
        }
        flight.in_flight = true;
        Some(Self {
            passes,
            ticket: ticket.clone(),
        })
    }

    /// Did a gesture arrive during the pass? Consumes the flag.
    fn rerequest_consumed(&self) -> bool {
        let mut table = recovered(self.passes);
        let flight = table.entry(self.ticket.clone()).or_default();
        std::mem::take(&mut flight.rerequest)
    }
}

impl Drop for FlightGuard<'_> {
    fn drop(&mut self) {
        let mut table = recovered(self.passes);
        if self.ticket.check().is_err() {
            table.remove(&self.ticket);
        } else if let Some(flight) = table.get_mut(&self.ticket) {
            flight.in_flight = false;
        }
    }
}

/// The guarded poll (ADR 0017): should this folder be polled? Any
/// uncertainty — poll refused by the server, unreadable marker — polls:
/// sobriety doesn't have the right to cost a message.
///
/// The after-gesture pass's own guarded-poll door — the DECISION and
/// the marker construction are the core's (`mail_core::must_poll`,
/// `cycle::local_marker`): review wave 3 collapsed the third
/// hand-synced marker copy this comment used to defend.
fn must_poll(
    store: &Store,
    account_id: i64,
    mailbox: &str,
    status: Option<&mail_core::FolderStatus>,
    problems: &mut Vec<String>,
) -> bool {
    let Some(status) = status else {
        return true;
    };
    match mail_core::cycle::local_marker(store, account_id, mailbox) {
        Ok(marker) => mail_core::must_poll(status, marker.as_ref()),
        Err(err) => {
            problems.push(format!("marker of \"{mailbox}\": {err}"));
            true
        }
    }
}

/// Settles the marker of a SUCCESSFUL poll: the UIDNEXT of the status
/// that preceded it. Never on a failed poll — a marker set on a folder
/// that wasn't caught up would wrongly make it skip at the next cycle.
fn settle_marker(
    store: &Store,
    account_id: i64,
    mailbox: &str,
    status: Option<&mail_core::FolderStatus>,
    problems: &mut Vec<String>,
) {
    let Some(uidnext) = status.and_then(|status| status.uid_next) else {
        return;
    };
    let outcome = store.sync_state(account_id, mailbox).and_then(|state| {
        if let Some(state) = state {
            store.set_remote_uidnext(state.mailbox_id, uidnext)?;
        }
        Ok(())
    });
    if let Err(err) = outcome {
        problems.push(format!("marker of \"{mailbox}\": {err}"));
    }
}

/// The pass of ONE account — the blocking body of `sync_after_gesture`.
fn pass_after_gesture_account(
    path: &Path,
    mut session: AccountWork,
    account_id: i64,
    cycle: &crate::SyncShared,
    locks: &Mutex<HashMap<String, Arc<Mutex<()>>>>,
    app: &AppHandle,
) -> Result<(PassReport, Vec<AccountWork>), String> {
    let mut report = PassReport::default();
    let mut sessions = Vec::new();
    let mut attempt = 0u32;
    // Set by EVERY loop turn before any exit: no starting value — the
    // compiler guarantees we never read an unset one.
    let mut last_clean;
    loop {
        attempt += 1;
        let errors_before = report.errors.len();
        // The mail of THIS attempt (excluding INBOX, which already
        // publishes its own via `poll_inbox`) — it's this count that
        // bumps the generation.
        let mut mail_this_attempt = 0usize;
        let total_timer = Instant::now();
        {
            let lock = crate::poll::account_lock(locks, session.email());
            let _poll = lock.lock();
            let (mut server, fresh, _lease) = crate::poll::connect_imap(&session)?;
            if let Some(fresh) = fresh {
                session = fresh.clone();
                sessions.push(fresh);
            }
            let mut store = Store::open(path).map_err(|err| err.to_string())?;
            // 1. The intentions: the replay starts NOW.
            let timer = Instant::now();
            let sources = store
                .mailboxes_with_actions(account_id)
                .map_err(|err| err.to_string())?;
            for mailbox in &sources {
                if mailbox == MAILBOX {
                    let hooks = crate::poll::ShellHooks::new(cycle, app.clone());
                    if let Err(reason) = mail_core::cycle::poll_inbox(
                        &mut server,
                        &mut store,
                        account_id,
                        &hooks,
                        &mut report.errors,
                    ) {
                        report.errors.push(format!("INBOX: {reason}"));
                    }
                } else {
                    let status = server.folder_status(mailbox).ok();
                    if must_poll(
                        &store,
                        account_id,
                        mailbox,
                        status.as_ref(),
                        &mut report.errors,
                    ) {
                        match SyncEngine::default().sync(
                            &mut server,
                            &mut store,
                            account_id,
                            mailbox,
                        ) {
                            Ok(sync_report) => {
                                report.fetched += sync_report.fetched;
                                report.deleted += sync_report.deleted;
                                mail_this_attempt += sync_report.fetched + sync_report.deleted;
                                settle_marker(
                                    &store,
                                    account_id,
                                    mailbox,
                                    status.as_ref(),
                                    &mut report.errors,
                                );
                            }
                            Err(reason) => report.errors.push(format!("source folder: {reason}")),
                        }
                    }
                }
            }
            let actions_duration = timer.elapsed();
            let n_sources = sources.len();
            // 2. The inventory: only the folders that MOVED get polled —
            // the gesture's destination, without ever guessing it.
            let timer = Instant::now();
            let mut polled = 0usize;
            match server.folders_with_status() {
                Ok(Some(with_status)) => {
                    for (folder, status) in with_status {
                        if !folder.selectable
                            || folder.wire == MAILBOX
                            || sources.contains(&folder.wire)
                        {
                            continue;
                        }
                        if poll_folder_pass(
                            &mut server,
                            &mut store,
                            account_id,
                            &folder.wire,
                            status.as_ref(),
                            &mut report,
                            &mut mail_this_attempt,
                        ) {
                            polled += 1;
                        }
                    }
                }
                Ok(None) => {
                    // Without LIST-STATUS: targeted STATUS calls on the
                    // canonical destinations only — never the ~50 folders.
                    let folders = store
                        .canonical_folders(account_id)
                        .map_err(|err| err.to_string())?;
                    for name in [folders.sent, folders.archives, folders.trash]
                        .into_iter()
                        .flatten()
                    {
                        if name == MAILBOX || sources.contains(&name) {
                            continue;
                        }
                        let status = server.folder_status(&name).ok();
                        if poll_folder_pass(
                            &mut server,
                            &mut store,
                            account_id,
                            &name,
                            status.as_ref(),
                            &mut report,
                            &mut mail_this_attempt,
                        ) {
                            polled += 1;
                        }
                    }
                }
                Err(reason) => report
                    .errors
                    .push(format!("LIST-STATUS inventory: {reason}")),
            }
            let inventory_duration = timer.elapsed();
            // 3. The reconciliation: the echo dies when the real row
            // arrives — the list doesn't jump visibly.
            let reconciled = store
                .reconcile_echos(account_id)
                .map_err(|err| err.to_string())?;
            report.reconciled += reconciled;
            mail_this_attempt += reconciled;
            server.logout();
            // The trace that will inform D-7 (§6.8: durations and counts
            // only) — read against the gesture's timestamp in the console.
            crate::trace::trace(&format!(
                "after-gesture pass account {account_id}: {n_sources} source(s) {:.1}s · inventory + {polled} polled {:.1}s · {reconciled} reconciled · total {:.1}s",
                actions_duration.as_secs_f32(),
                inventory_duration.as_secs_f32(),
                total_timer.elapsed().as_secs_f32(),
            ));
            if mail_this_attempt > 0 {
                cycle
                    .mail
                    .fetch_add(mail_this_attempt as u64, Ordering::Relaxed);
                cycle.generation.fetch_add(1, Ordering::Relaxed);
            }
        }
        last_clean = report.errors.len() == errors_before;
        let pending_lease = session.ticket.lease()?;
        let pending = Store::open(path)
            .map_err(|err| err.to_string())?
            .pending_echos(account_id)
            .map_err(|err| err.to_string())?;
        drop(pending_lease);
        if pending == 0 {
            break;
        }
        match retry_after(attempt) {
            Some(delay) => std::thread::sleep(delay),
            None => break,
        }
    }
    // 4. The sweep — after a CLEAN attempt only: a failed poll denied
    // nothing, the echo lives on (offline, backoff…).
    if last_clean {
        let _lease = session.ticket.lease()?;
        let store = Store::open(path).map_err(|err| err.to_string())?;
        let incidents = store
            .sweep_echos(account_id)
            .map_err(|err| err.to_string())?;
        if !incidents.is_empty() {
            report.swept += incidents.len();
            report.errors.extend(incidents);
            // Rows just disappeared: the list is served again.
            cycle.generation.fetch_add(1, Ordering::Relaxed);
        }
    }
    Ok((report, sessions))
}

/// A folder poll of the pass (inventory phase): gated by `must_poll`,
/// settled, counted. Returns true if the folder was polled.
#[allow(clippy::too_many_arguments)]
fn poll_folder_pass(
    server: &mut ImapServer,
    store: &mut Store,
    account_id: i64,
    mailbox: &str,
    status: Option<&mail_core::FolderStatus>,
    report: &mut PassReport,
    mail: &mut usize,
) -> bool {
    if !must_poll(store, account_id, mailbox, status, &mut report.errors) {
        return false;
    }
    match SyncEngine::default().sync(server, store, account_id, mailbox) {
        Ok(sync_report) => {
            report.fetched += sync_report.fetched;
            report.deleted += sync_report.deleted;
            *mail += sync_report.fetched + sync_report.deleted;
            settle_marker(store, account_id, mailbox, status, &mut report.errors);
            true
        }
        Err(reason) => {
            report
                .errors
                .push(format!("folder \"{mailbox}\": {reason}"));
            false
        }
    }
}

fn run_flush_all(
    jobs: Vec<(i64, AccountWork)>,
    db_path: &Path,
    lock: &Mutex<()>,
) -> Result<(OutboxSummary, Vec<AccountWork>), String> {
    // E5: a poisoned lock is reclaimed (the panic is logged, ADR 0014).
    let _guard = recovered(lock);
    let mut store = Store::open(db_path).map_err(|err| err.to_string())?;

    // An earlier crash is noted even offline: quarantine first.
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
            // Offline: this account's queue survives as is.
            Err(reason) => summary.error = Some(reason),
            Ok((mut mailer, refreshed, _lease)) => {
                let report =
                    mail_core::flush_outbox_while(&mut mailer, &mut store, account_id, &mut || {
                        session.ticket.check().is_ok()
                    })
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
    let remaining = store
        .outbox_in_state(OutboxState::Queued)
        .map_err(|err| err.to_string())?;
    summary.queued = remaining.len();
    // The field trace of the flush (§6.8 — readable via `2> file`, the
    // release app is a Windows subsystem): the summary, then the last
    // error of each send left in the queue — it's the one the status
    // bar doesn't show ("waiting" isn't at fault) and a field finding
    // must be able to read.
    crate::trace::trace(&format!(
        "flush: {} sent, {} deferred, {} rejected, {} quarantined, {} queued{}",
        summary.sent,
        summary.deferred,
        summary.rejected,
        summary.quarantined,
        summary.queued,
        summary
            .error
            .as_deref()
            .map(|err| format!(" · connection: {err}"))
            .unwrap_or_default(),
    ));
    for message in &remaining {
        if let Some(err) = &message.last_error {
            // §6.8: the id, the attempts, the error — NEVER the subject
            // (E9; before, the subject used to go into the trace).
            crate::trace::trace(&format!(
                "flush: send {} waiting ({} attempt(s)): {err}",
                message.id, message.attempts
            ));
        }
    }
    Ok((summary, refreshed_list))
}

/// The outbox state for the UI: everything that hasn't gone out, all
/// accounts combined.
fn read_sends(store: &Store) -> Result<OutboxStatus, String> {
    let accounts = store.accounts().map_err(|err| err.to_string())?;
    let mut status = OutboxStatus {
        queued: 0,
        interrupted: 0,
        rejected: 0,
        held: 0,
        scheduled: 0,
        next_scheduled_epoch: None,
        entries: Vec::new(),
        refused_actions: store.refused_actions().map_err(|err| err.to_string())?,
        refused_action_reason: store
            .refused_action_reason()
            .map_err(|err| err.to_string())?,
        action_incidents: store
            .action_incidents()
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|incident| ActionIncidentRow {
                id: incident.id,
                account: accounts
                    .iter()
                    .find(|account| account.id == incident.account_id)
                    .map(|account| account.email.clone())
                    .unwrap_or_default(),
                subject: incident.subject,
                sender: incident.sender,
                source: incident.source,
                destination: incident.destination,
                reason: incident.reason,
            })
            .collect(),
    };
    let now = chrono::Utc::now().timestamp();
    for message in store.outbox_metadata().map_err(|err| err.to_string())? {
        // R2: scheduled but not yet due — it isn't waiting on the
        // network, it's waiting on its time. Counted separately, and
        // the nearest due time bubbles up (the probe will trigger the
        // flush).
        let scheduled = message.state == OutboxState::Queued
            && message.send_at_epoch.is_some_and(|epoch| epoch > now);
        match message.state {
            OutboxState::Sent => continue,
            OutboxState::Queued if scheduled => {
                status.scheduled += 1;
                status.next_scheduled_epoch = match status.next_scheduled_epoch {
                    None => message.send_at_epoch,
                    Some(known) => Some(known.min(message.send_at_epoch.unwrap_or(known))),
                };
            }
            OutboxState::Queued | OutboxState::Sending => status.queued += 1,
            OutboxState::Interrupted => status.interrupted += 1,
            OutboxState::Rejected => status.rejected += 1,
            OutboxState::Held => status.held += 1,
        }
        status.entries.push(OutboxEntry {
            id: message.id,
            message_id: message.message_id,
            subject: message.subject,
            to: message.to.join(", "),
            state: message.state.as_str().to_string(),
            attempts: message.attempts,
            error: message.last_error,
            attachments: message.attachments.len(),
            send_at_epoch: message.send_at_epoch.filter(|epoch| *epoch > now),
        });
    }
    Ok(status)
}

#[tauri::command]
pub async fn outbox_status(app: AppHandle) -> Result<OutboxStatus, CommandError> {
    read_store_off_pump(app, move |_, store| Ok(read_sends(store)?)).await
}

/// Requeuing a quarantined or rejected send: THE explicit user decision
/// required by the "never a ghost send" rule.
#[tauri::command]
pub async fn outbox_requeue(
    app: AppHandle,
    id: i64,
    message_id: String,
) -> Result<(), CommandError> {
    store_off_pump(app, move |app, store| {
        let account_id = store.outbox_account(id, &message_id)?;
        admit_account(app, store, account_id)?;
        Ok(store.requeue_outbox(id)?)
    })
    .await
}

/// Abandoning a send (user decision); the `sent` history is preserved
/// by the core.
#[tauri::command]
pub async fn outbox_delete(
    app: AppHandle,
    id: i64,
    message_id: String,
) -> Result<(), CommandError> {
    store_off_pump(app, move |_, store| {
        store.outbox_account(id, &message_id)?;
        Ok(store.delete_outbox(id)?)
    })
    .await
}

/// The user's word on a send held after a restore (Lot 5 E14b): back
/// to the queue — the caller flushes.
#[tauri::command]
pub async fn outbox_release(
    app: AppHandle,
    id: i64,
    message_id: String,
) -> Result<(), CommandError> {
    store_off_pump(app, move |app, store| {
        let account_id = store.outbox_account(id, &message_id)?;
        admit_account(app, store, account_id)?;
        Ok(store.release_held(id)?)
    })
    .await
}

/// A copy of the database into the file the user named (Lot 5 E14b,
/// G02): consistent, compacted, secret-free (the keyring holds them).
/// E8: the path comes from the native dialog — absolute, in an existing
/// folder, and not an existing file (never a database overwritten).
/// A read of the database: no commands' lock (the copy of a large
/// database takes seconds, the reading pane must not wait it out).
#[tauri::command]
pub async fn backup_snapshot(app: AppHandle, dest: String) -> Result<String, CommandError> {
    let target = PathBuf::from(&dest);
    if !target.is_absolute() || target.parent().is_none_or(|p| !p.is_dir()) {
        return Err("the copy must go to an absolute path in an existing folder".into());
    }
    read_store_off_pump(app, move |_, store| {
        store.snapshot_into(&target)?;
        Ok(target.to_string_lossy().into_owned())
    })
    .await
}

/// Stages a copy for restoration (Lot 5 E14b): validated by the core's
/// probe, placed next to the database; the swap happens at the next
/// start, before anything opens the database (`restore::apply_pending`),
/// and the copy's sends are held at the adoption. The caller restarts.
#[tauri::command]
pub async fn restore_snapshot(app: AppHandle, source: String) -> Result<(), CommandError> {
    let source = PathBuf::from(&source);
    if !source.is_absolute() || !source.is_file() {
        return Err("the copy must be an existing file".into());
    }
    // The path under the token; the copy itself (a file of gigabytes)
    // off the commands' lock — nothing reads the database meanwhile.
    let db = off_pump(app, move |app| probe_path(&app)).await?;
    unlocked(move || {
        crate::restore::stage(&db, &source).map_err(|err| err.to_string())?;
        Ok::<_, CommandError>(())
    })
    .await
}

/// Relaunches Wind (after a staged restore). Pure: no database, no
/// file — the process ends here.
#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.restart();
}

/// R2, CE decision D2: cancels a scheduled send — the entry leaves the
/// log and a COMPLETE draft is reborn (recipients, body, marking,
/// attachments). Returns the id of the recreated draft, or `None` if
/// the flush already took it in the meantime: too late, the message is
/// going out — the UI says so honestly rather than promising a ghost
/// draft.
#[tauri::command]
pub async fn outbox_cancel_scheduled(
    app: AppHandle,
    id: i64,
    message_id: String,
) -> Result<Option<i64>, CommandError> {
    store_off_pump(app, move |app, store| {
        let account_id = store.outbox_account(id, &message_id)?;
        admit_account(app, store, account_id)?;
        Ok(store.cancel_scheduled_send(id)?)
    })
    .await
}

// ---------------------------------------------------------------------
// Signature per account (R1, PLAN-RETOURS-6, CE decisions D3/D4).
// ---------------------------------------------------------------------

/// An account's signature and its scope — what Settings edits and what
/// the composer inserts on open.
#[derive(Serialize)]
pub struct SignatureRow {
    /// Sanitized HTML (ammonia allowlist) — `None`: no signature.
    pub html: Option<String>,
    /// D4: the signature is ALSO inserted into replies and forwards.
    /// Default: new messages only.
    pub replies: bool,
    /// The signature the composition CARRIES, decided by the core from
    /// the `mode` the composer passed (`new`, `reply`, `forward`) and
    /// the scope above (Lot 5 E13c): `None` when the scope excludes it,
    /// when there is no signature, or when no mode was passed (Settings
    /// reads the row, it composes nothing).
    pub applicable: Option<String>,
}

#[tauri::command]
pub async fn signature_get(
    app: AppHandle,
    account_id: i64,
    mode: Option<String>,
) -> Result<SignatureRow, CommandError> {
    read_store_off_pump(app, move |_, store| {
        let html = store
            .text_pref(&format!("signature.{account_id}"))
            .map_err(|err| err.to_string())?
            .filter(|h| !h.trim().is_empty());
        let replies = store
            .bool_pref(&format!("signature_replies.{account_id}"), false)
            .map_err(|err| err.to_string())?;
        let applicable = mode
            .as_deref()
            .and_then(mail_core::ComposeMode::parse)
            .filter(|mode| mail_core::signature_applies(*mode, replies))
            .and(html.clone());
        Ok(SignatureRow {
            html,
            replies,
            applicable,
        })
    })
    .await
}

/// Saves an account's signature. The HTML goes through THE boundary
/// (`body_boundary`, ammonia allowlist) — a signature enters the
/// database like any body: sanitized, never taken raw. An HTML with an
/// no text and no image counts as "signature cleared."
#[tauri::command]
pub async fn signature_set(
    app: AppHandle,
    account_id: i64,
    html: Option<String>,
    image_sources: Option<BTreeMap<String, String>>,
    replies: bool,
) -> Result<(), CommandError> {
    account_store_off_pump(app, account_id, move |_, store| {
        let clean = html.as_deref().and_then(|h| {
            composition_boundary(String::new(), Some(h), &image_sources.unwrap_or_default()).1
        });
        store
            .set_text_pref(
                &format!("signature.{account_id}"),
                clean.as_deref().unwrap_or(""),
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
// Account marker (PLAN-RETOURS-8 R1): icon + hue per account, to tell
// mailboxes apart in the unified mailbox. Local preference (table
// `prefs`, signature pattern) — the server has no such concept.
// ---------------------------------------------------------------------

/// The icon set DEDICATED to accounts (D2): new glyphs from the
/// subset, reserved — never reused elsewhere (A3 "one icon, one
/// meaning").
const MARKER_ICONS: [&str; 12] = [
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

/// The measured swatch table (D1): 12 families, whose TWO variants
/// (light / night) live in `system.css` — here only the family name is
/// stored, never a hex value.
pub(crate) const MARKER_HUES: [&str; 12] = [
    "rouge", "orange", "ocre", "olive", "vert", "sapin", "bleu", "indigo", "violet", "magenta",
    "rose", "brun",
];

/// The pure decision: a marker only exists within the crossed allowlist
/// (dedicated set × swatch table). Everything else — a product glyph,
/// an unknown hue, an empty string — is refused, both on input and on
/// readback.
pub(crate) fn valid_marker(icon: &str, hue: &str) -> bool {
    MARKER_ICONS.contains(&icon) && MARKER_HUES.contains(&hue)
}

/// Rereads an account's marker; a value outside the allowlist
/// (corrupted database, older version) never reaches the UI.
pub(crate) fn marker_of(
    store: &Store,
    account_id: i64,
) -> Result<Option<(String, String)>, mail_core::Error> {
    let icon = store.text_pref(&format!("repere_icone.{account_id}"))?;
    let hue = store.text_pref(&format!("repere_teinte.{account_id}"))?;
    Ok(match (icon, hue) {
        (Some(i), Some(t)) if valid_marker(&i, &t) => Some((i, t)),
        _ => None,
    })
}

/// Sets or removes (None) the marker — removing clears the keys
/// (signature pattern: an empty pref means "never set"). BOTH keys go
/// out in ONE transaction: a half-written pair would be a marker no one
/// chose (2026-08-22 review).
pub(crate) fn set_marker(
    store: &mut Store,
    account_id: i64,
    marker: Option<(&str, &str)>,
) -> Result<(), mail_core::Error> {
    let (icon, hue) = marker.unwrap_or(("", ""));
    let icon_key = format!("repere_icone.{account_id}");
    let hue_key = format!("repere_teinte.{account_id}");
    store.set_text_prefs(&[(icon_key.as_str(), icon), (hue_key.as_str(), hue)])
}

#[derive(Serialize)]
pub struct MarkerRow {
    pub account_id: i64,
    pub icon: String,
    pub hue: String,
}

/// All the set markers — the UI loads them ONCE (nav + list) and
/// reloads them on change. An account without a marker has no row: its
/// default render (`person`, neutral token) depends on nothing.
#[tauri::command]
pub async fn markers_get(app: AppHandle) -> Result<Vec<MarkerRow>, CommandError> {
    read_store_off_pump(app, move |_, store| {
        let mut rows = Vec::new();
        for account in store.accounts().map_err(|err| err.to_string())? {
            if let Some((icon, hue)) =
                marker_of(store, account.id).map_err(|err| err.to_string())?
            {
                rows.push(MarkerRow {
                    account_id: account.id,
                    icon,
                    hue: crate::wire::hue_to_wire(&hue),
                });
            }
        }
        Ok(rows)
    })
    .await
}

/// Sets (icon + hue) or removes (both to None) an account's marker. A
/// value outside the allowlist is a plain error — the UI only offers
/// the dedicated set, any other call is a bug.
#[tauri::command]
pub async fn marker_set(
    app: AppHandle,
    account_id: i64,
    icon: Option<String>,
    hue: Option<String>,
) -> Result<(), CommandError> {
    off_pump(app, move |app| {
        // The UI speaks the wire hue (`blue`); the allowlist and the
        // database keep the French family name (D16).
        if hue
            .as_deref()
            .is_some_and(|h| !crate::wire::WIRE_HUES.contains(&h))
        {
            return Err("marker outside the dedicated set".into());
        }
        let hue = hue.map(|h| crate::wire::hue_from_wire(&h));
        let marker = match (icon.as_deref(), hue.as_deref()) {
            (None, None) => None,
            (Some(i), Some(t)) if valid_marker(i, t) => Some((i, t)),
            (Some(_), Some(_)) => return Err("marker outside the dedicated set".into()),
            _ => return Err("icon and hue go together".into()),
        };
        let mut store = Store::open(&adopted_db(&app)?).map_err(|err| err.to_string())?;
        Ok(set_marker(&mut store, account_id, marker)?)
    })
    .await
}

/// PLAN-RETOURS-9 (D3): the pure decision for an account's custom name.
/// Whitespace trimmed; empty = removed (None); beyond 60 characters
/// refused — never silently truncated.
pub(crate) fn normalized_name(raw: &str) -> Result<Option<String>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > 60 {
        return Err("name too long (60 characters at most)".to_string());
    }
    Ok(Some(trimmed.to_string()))
}

/// Rereads an account's name; a blank shell in the database (set
/// outside the UI, older version) never reaches the display.
pub(crate) fn name_of(store: &Store, account_id: i64) -> Result<Option<String>, mail_core::Error> {
    Ok(store
        .text_pref(&format!("nom_compte.{account_id}"))?
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty()))
}

/// Sets or removes (None) the name — removing clears the key
/// (signature/marker pattern: an empty pref means "never set").
pub(crate) fn set_name(
    store: &mut Store,
    account_id: i64,
    name: Option<&str>,
) -> Result<(), mail_core::Error> {
    let key = format!("nom_compte.{account_id}");
    store.set_text_prefs(&[(key.as_str(), name.unwrap_or(""))])
}

#[derive(Serialize)]
pub struct NameRow {
    pub account_id: i64,
    pub name: String,
}

/// All the set names — the UI loads them ONCE (nav + settings +
/// composer) and patches its table on gesture (marker pattern).
#[tauri::command]
pub async fn names_get(app: AppHandle) -> Result<Vec<NameRow>, CommandError> {
    read_store_off_pump(app, move |_, store| {
        let mut rows = Vec::new();
        for account in store.accounts().map_err(|err| err.to_string())? {
            if let Some(name) = name_of(store, account.id).map_err(|err| err.to_string())? {
                rows.push(NameRow {
                    account_id: account.id,
                    name,
                });
            }
        }
        Ok(rows)
    })
    .await
}

/// Sets or removes (empty string / None) an account's name. Returns the
/// NORMALIZED name actually written — that's what the UI displays.
#[tauri::command]
pub async fn name_set(
    app: AppHandle,
    account_id: i64,
    name: Option<String>,
) -> Result<Option<String>, CommandError> {
    off_pump(app, move |app| {
        let normalized = normalized_name(name.as_deref().unwrap_or(""))?;
        let mut store = Store::open(&adopted_db(&app)?).map_err(|err| err.to_string())?;
        set_name(&mut store, account_id, normalized.as_deref()).map_err(|err| err.to_string())?;
        Ok(normalized)
    })
    .await
}

// ---------------------------------------------------------------------
// Local drafts + per-account Gmail reflection (Phases 2-3).
// ---------------------------------------------------------------------

#[derive(Serialize)]
pub struct DraftRow {
    pub id: i64,
    pub incarnation: String,
    pub account_id: i64,
    pub to: String,
    pub cc: String,
    pub bcc: String,
    pub subject: String,
    pub body: String,
    /// Rich body as stored — `None` for a text draft: resuming converts
    /// it on open (escaping + line breaks), the exact inverse of the
    /// text derivation on the save side.
    pub body_html: Option<String>,
    pub reply_to_uid: Option<u32>,
    /// The mailbox that gives `reply_to_uid` its meaning (ADR 0009) —
    /// resuming must hand it back to the composer, or the reply → draft
    /// → resume chain loses the thread.
    pub reply_to_mailbox: Option<String>,
    /// The thread this draft replies to, resolved by the core — `None`
    /// for a free composition or a target that has vanished.
    pub thread_id: Option<i64>,
    /// Marked “important” (R3) — resuming restores the button's state.
    pub important: bool,
    /// The editor sends it back on save: that's what lets it detect
    /// that something else wrote in the meantime.
    pub updated_epoch: i64,
}

/// What a save did — the editor needs it for the next one.
#[derive(Serialize)]
pub struct DraftSavedRow {
    pub id: i64,
    pub updated_epoch: i64,
    /// The draft had changed elsewhere: the editor's text was kept
    /// aside. To be told to the user, never hidden.
    pub forked: bool,
    pub attachments: Vec<DraftAttachmentRow>,
}

/// Saves a draft — plain text, never validated: it's a net. The content
/// exactly as the editor sends it — grouped for the same reason as in
/// the core: four neighboring strings invite swapping two of them.
///
/// `camelCase`: Tauri only converts names at the first level of
/// arguments. Without this annotation, the UI would have to send
/// `reply_to_uid` here and `replyToUid` elsewhere — an inconsistency
/// that only shows up at runtime.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftContentArg {
    to: String,
    cc: String,
    bcc: String,
    subject: String,
    body: String,
    /// Rich body from the editor (PLAN-COMPOSITION-HTML) — absent or
    /// empty = text draft. Sanitized on the Rust side before any write.
    body_html: Option<String>,
    #[serde(default)]
    image_sources: BTreeMap<String, String>,
    reply_to_uid: Option<u32>,
    reply_to_mailbox: Option<String>,
    /// Marked “important” (R3, PLAN-RETOURS-6). `default`: a caller
    /// from before this field sends nothing — an ordinary draft.
    #[serde(default)]
    important: bool,
}

/// Stored HTML retains permitted image URLs; only the editor preparation may
/// insert it into the live document. The fallback text comes from the same HTML.
pub(crate) fn body_boundary(body: String, body_html: Option<&str>) -> (String, Option<String>) {
    composition_boundary(body, body_html, &BTreeMap::new())
}

fn composition_boundary(
    body: String,
    body_html: Option<&str>,
    images: &BTreeMap<String, String>,
) -> (String, Option<String>) {
    let rich = body_html
        .filter(|html| !html.trim().is_empty())
        .map(|html| mail_render::sanitize_composition(html, images).html);
    match rich {
        Some(html) => {
            let text = mail_render::body_text(&html);
            if text.trim().is_empty() && !html.contains("<img") {
                (body, None)
            } else {
                (text, Some(html))
            }
        }
        None => (body, None),
    }
}

#[derive(Serialize)]
pub struct ComposerHtml {
    html: String,
    image_sources: BTreeMap<String, String>,
}

#[tauri::command]
pub async fn prepare_composer_html(
    app: AppHandle,
    html: String,
    image_sources: Option<BTreeMap<String, String>>,
) -> Result<ComposerHtml, CommandError> {
    off_pump(app, move |_| {
        let html = match image_sources.filter(|images| !images.is_empty()) {
            Some(images) => mail_render::sanitize_composition(&html, &images).html,
            None => html,
        };
        let prepared = mail_render::sanitize_for_composer(&html);
        Ok(ComposerHtml {
            html: prepared.html,
            image_sources: prepared.image_sources,
        })
    })
    .await
}

#[tauri::command]
pub async fn save_draft(
    app: AppHandle,
    account_id: i64,
    id: Option<i64>,
    base_epoch: Option<i64>,
    content: DraftContentArg,
    edit_token: Option<String>,
) -> Result<Option<DraftSavedRow>, CommandError> {
    account_store_off_pump(app, account_id, move |_, store| {
        // Same boundary as sending (`body_boundary`): sanitized HTML,
        // derived text (previews and fallback).
        let (body_text, body_rich) = composition_boundary(
            content.body.clone(),
            content.body_html.as_deref(),
            &content.image_sources,
        );
        let content = mail_core::DraftContent {
            to_raw: &content.to,
            cc_raw: &content.cc,
            bcc_raw: &content.bcc,
            body_html: body_rich.as_deref(),
            subject: &content.subject,
            body: &body_text,
            reply_to_uid: content.reply_to_uid,
            reply_to_mailbox: content.reply_to_mailbox.as_deref(),
            important: content.important,
        };
        let saved = match edit_token {
            Some(token) => store.save_draft_edit(&token, account_id, content)?,
            None => Some(store.save_draft(account_id, id, base_epoch, content)?),
        };
        Ok(saved.map(draft_saved_row))
    })
    .await
}

#[tauri::command]
pub async fn list_drafts(app: AppHandle) -> Result<Vec<DraftRow>, CommandError> {
    read_store_off_pump(app, move |_, store| {
        Ok(store
            .drafts()
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(draft_row)
            .collect())
    })
    .await
}

fn draft_row(draft: mail_core::SavedDraft) -> DraftRow {
    DraftRow {
        updated_epoch: draft.updated_epoch,
        id: draft.id,
        incarnation: draft.incarnation,
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
    }
}

fn draft_saved_row(saved: mail_core::DraftSaved) -> DraftSavedRow {
    DraftSavedRow {
        id: saved.id,
        updated_epoch: saved.updated_epoch,
        forked: saved.forked,
        attachments: saved.attachments.into_iter().map(attachment_row).collect(),
    }
}

#[derive(Serialize)]
pub struct DraftEditRow {
    draft: Option<DraftRow>,
    attachments: Vec<DraftAttachmentRow>,
}

#[tauri::command]
pub async fn begin_draft_edit(
    app: AppHandle,
    token: String,
    account_id: i64,
    id: Option<i64>,
    incarnation: Option<String>,
    base_epoch: Option<i64>,
) -> Result<DraftEditRow, CommandError> {
    account_store_off_pump(app, account_id, move |_, store| {
        let edit =
            store.begin_draft_edit(&token, account_id, id, incarnation.as_deref(), base_epoch)?;
        Ok(DraftEditRow {
            draft: edit.draft.map(draft_row),
            attachments: edit.attachments.into_iter().map(attachment_row).collect(),
        })
    })
    .await
}

#[tauri::command]
pub async fn finish_draft_edit(
    app: AppHandle,
    token: String,
    discard: bool,
) -> Result<Option<DraftSavedRow>, CommandError> {
    store_off_pump(app, move |app, store| {
        if !discard && let Some(account) = store.active_draft_edit_account()? {
            admit_account(app, store, account)?;
        }
        Ok(store
            .finish_draft_edit(&token, discard)?
            .map(draft_saved_row))
    })
    .await
}

#[tauri::command]
pub async fn recover_draft_edit(app: AppHandle) -> Result<(), CommandError> {
    store_off_pump(app, move |app, store| {
        if let Some(account) = store.active_draft_edit_account()? {
            admit_account(app, store, account)?;
        }
        Ok(store.recover_draft_edit()?)
    })
    .await
}

#[tauri::command]
pub async fn detach_draft_edit_file(
    app: AppHandle,
    token: String,
    account_id: i64,
    attachment_id: i64,
) -> Result<Option<DraftSavedRow>, CommandError> {
    account_store_off_pump(app, account_id, move |_, store| {
        Ok(store
            .remove_draft_edit_attachment(&token, account_id, attachment_id)?
            .map(draft_saved_row))
    })
    .await
}

#[tauri::command]
pub async fn delete_draft(app: AppHandle, id: i64) -> Result<(), CommandError> {
    store_off_pump(app, move |_, store| Ok(store.delete_draft(id)?)).await
}

// ---------------------------------------------------------------------
// Composer attachments (PLAN-PIECES-JOINTES E2).
// ---------------------------------------------------------------------

/// One attachment of a draft, for the composer's chips. Metadata only —
/// the bytes leave the database only when building the MIME message.
#[derive(Serialize)]
pub struct DraftAttachmentRow {
    pub id: i64,
    pub name: String,
    pub mime: String,
    /// Decoded, raw bytes — the total weight is summed on the UI side.
    pub size: u64,
    /// Readable size, same form as the Reading pane (“2.4 MB”).
    pub human: String,
}

/// An attachment refused at the cap (PJ-D3) — the surface tells the
/// name and the remaining room, in readable form.
#[derive(Serialize)]
pub struct RefusedAttachment {
    pub name: String,
    pub remaining: String,
}

/// Outcome of the “Attach” gesture.
#[derive(Serialize)]
pub struct AttachReport {
    pub forked: bool,
    /// The anchor draft (created on the first file if needed, PJ-D1).
    /// `None`: nothing was entered AND no draft existed — the anchor
    /// created for nothing was reclaimed, no empty draft left lying
    /// around.
    pub draft_id: Option<i64>,
    /// `None` if no file was entered (all refused): the draft did not
    /// move, the editor keeps its marker.
    pub updated_epoch: Option<i64>,
    /// ALL of the draft's attachments after the gesture, in order.
    pub attachments: Vec<DraftAttachmentRow>,
    pub refused: Vec<RefusedAttachment>,
}

fn attachment_row(meta: mail_core::DraftAttachmentMeta) -> DraftAttachmentRow {
    DraftAttachmentRow {
        id: meta.id,
        name: meta.name,
        mime: meta.mime,
        size: meta.size,
        human: mail_core::human_size(meta.size),
    }
}

/// MIME type deduced from the extension — for the attachment header,
/// never for a decision: an unknown one goes out as
/// `application/octet-stream`, honest and universally accepted.
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

fn attachment_room(files: &[mail_core::DraftAttachmentMeta]) -> u64 {
    files
        .iter()
        .fold(mail_core::MAX_ATTACHMENTS_BYTES, |remaining, file| {
            remaining.saturating_sub(file.size)
        })
}

/// The file half of the "Attach" gesture, OFF the commands' lock (Lot 5
/// E13d): each path is checked and read against the room the draft had
/// when the gesture started. The room is looked at again by the core at
/// every insert (`AttachmentOverBudget`): a file that fit here but no
/// longer fits there is refused there, never stored over the cap.
struct ReadAttachments {
    files: Vec<(String, Vec<u8>)>,
    refused: Vec<RefusedAttachment>,
    /// The first hard read failure, kept for AFTER the files read
    /// before it are stored: the gesture fails as it always did, the
    /// files that read fine stay attached (review 2026-09-07 — the
    /// first cut dropped them with the failure).
    failure: Option<String>,
}

fn read_attachment_files(paths: &[String], mut remaining: u64) -> ReadAttachments {
    let mut read = ReadAttachments {
        files: Vec::with_capacity(paths.len()),
        refused: Vec::new(),
        failure: None,
    };
    for path in paths {
        let candidate = Path::new(path);
        let name = candidate
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        // A read failure is an outright failure of the gesture: files
        // already entered stay (the UI re-reads the chips), this one
        // has a problem the user must see, not a silence.
        match crate::attachment_file::read(candidate, remaining) {
            Ok(Some(bytes)) => {
                remaining = remaining.saturating_sub(bytes.len() as u64);
                read.files.push((name, bytes));
            }
            Ok(None) => read.refused.push(RefusedAttachment {
                name,
                remaining: mail_core::human_size(remaining),
            }),
            Err(err) => {
                read.failure = Some(format!("reading {path:?}: {err}"));
                break;
            }
        }
    }
    read
}

fn attach_edit_files(
    store: &Store,
    token: &str,
    account_id: i64,
    files: Vec<(String, Vec<u8>)>,
    mut refused: Vec<RefusedAttachment>,
) -> Result<AttachReport, CommandError> {
    let mut forked = false;
    for (name, bytes) in &files {
        match store.add_draft_edit_attachment(token, account_id, name, mime_for_name(name), bytes) {
            Ok(saved) => forked |= saved.forked,
            Err(mail_core::Error::AttachmentOverBudget {
                name, remaining, ..
            }) => refused.push(RefusedAttachment {
                name,
                remaining: mail_core::human_size(remaining),
            }),
            Err(err) => return Err(err.into()),
        }
    }
    let edit = store.draft_edit(token)?;
    Ok(AttachReport {
        forked,
        draft_id: edit.draft.as_ref().map(|draft| draft.id),
        updated_epoch: edit.draft.map(|draft| draft.updated_epoch),
        attachments: edit.attachments.into_iter().map(attachment_row).collect(),
        refused,
    })
}

/// Attaches files to the draft: reads each path, copies the bytes into
/// the database on the gesture (PJ-D1 — the picker returned paths, they
/// don't survive past this call), refuses at the cap without punishing
/// what was already gained.
///
/// `draft_id: None`: the anchor draft is created, empty of text — the
/// composer's autosave will fill it with the id and epoch returned here.
#[tauri::command]
pub async fn attach_files(
    app: AppHandle,
    account_id: i64,
    draft_id: Option<i64>,
    paths: Vec<String>,
    edit_token: Option<String>,
) -> Result<AttachReport, CommandError> {
    // Lock 1: the room left, read from the edit session or the draft.
    // Unlocked: the reads (a 25 MiB file, cold disk — never behind the
    // lock, Lot 5 E13d). Lock 2: the SQLite work, the core re-checking
    // the budget at every insert.
    let token = edit_token.clone();
    let room = account_store_off_pump(app.clone(), account_id, move |_, store| {
        Ok(match (&token, draft_id) {
            (Some(token), _) => attachment_room(&store.draft_edit(token)?.attachments),
            (None, Some(id)) => attachment_room(&store.draft_attachments_meta(id)?),
            (None, None) => mail_core::MAX_ATTACHMENTS_BYTES,
        })
    })
    .await?;
    let read = unlocked(move || Ok::<_, CommandError>(read_attachment_files(&paths, room))).await?;
    let ReadAttachments {
        files,
        refused,
        failure,
    } = read;
    account_store_off_pump(app, account_id, move |_, store| {
        let report = if let Some(token) = edit_token {
            attach_edit_files(store, &token, account_id, files, refused)
        } else {
            attach_draft_files(store, account_id, draft_id, files, refused)
        }?;
        match failure {
            Some(failure) => Err(failure.into()),
            None => Ok(report),
        }
    })
    .await
}

fn attach_draft_files(
    store: &mut Store,
    account_id: i64,
    draft_id: Option<i64>,
    files: Vec<(String, Vec<u8>)>,
    refused: Vec<RefusedAttachment>,
) -> Result<AttachReport, CommandError> {
    {
        let created = draft_id.is_none();
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
        let mut refused = refused;
        for (name, bytes) in &files {
            match store.add_draft_attachment(draft_id, name, mime_for_name(name), bytes) {
                Ok(saved) => updated_epoch = Some(saved.updated_epoch),
                Err(mail_core::Error::AttachmentOverBudget {
                    name, remaining, ..
                }) => refused.push(RefusedAttachment {
                    name,
                    remaining: mail_core::human_size(remaining),
                }),
                Err(err) => return Err(err.to_string().into()),
            }
        }
        // The anchor created for nothing (all refused) is reclaimed on
        // the spot: no phantom empty draft left in the folder.
        if created && updated_epoch.is_none() {
            store
                .delete_draft(draft_id)
                .map_err(|err| err.to_string())?;
            return Ok(AttachReport {
                forked: false,
                draft_id: None,
                updated_epoch: None,
                attachments: Vec::new(),
                refused,
            });
        }
        Ok(AttachReport {
            forked: false,
            draft_id: Some(draft_id),
            updated_epoch,
            attachments: store
                .draft_attachments_meta(draft_id)
                .map_err(|err| err.to_string())?
                .into_iter()
                .map(attachment_row)
                .collect(),
            refused,
        })
    }
}

/// Outcome of repatriating ONE attachment from the source message
/// (forward, PJ-D4). Two outcomes are named here, the third — a network
/// failure — is the command's error: the surface distinguishes them
/// (final refusal vs “Retry”).
#[derive(Serialize)]
pub struct FetchAttachmentReport {
    pub forked: bool,
    /// `None`: the attachment was refused AND no draft existed — the
    /// anchor created for nothing was reclaimed.
    pub draft_id: Option<i64>,
    pub updated_epoch: Option<i64>,
    /// The attachment added to the draft, if the repatriation succeeded.
    pub attachment: Option<DraftAttachmentRow>,
    /// The refusal at the cap (PJ-D3) — final, no “Retry”.
    pub refused: Option<RefusedAttachment>,
}

/// Repatriates an attachment from the source message and adds it to the
/// anchor draft (PJ-D4): the bytes come from the server
/// (`fetch_attachment`, the Reading pane's path), never from a local
/// file. One per call — the composer chains them, and each chip carries
/// its own state.
#[tauri::command]
// Named IPC arguments distinguish the source account from the draft owner.
#[allow(clippy::too_many_arguments)]
pub async fn fetch_source_attachment(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    version: MessageVersion,
    index: usize,
    draft_id: Option<i64>,
    edit_token: Option<String>,
    sender_account_id: Option<i64>,
) -> Result<FetchAttachmentReport, CommandError> {
    // E5: read (attachment + session) under `off_pump`, bare network,
    // then write under `off_pump` — never again a SQLite connection
    // held across the network wait, nor a draft written outside the
    // commands' lock (the `save_draft`/`delete_draft` TOCTOU of ADR
    // 0019).
    let mailbox_name = mailbox.clone();
    let (attachment, session, identity, sender_ticket) = off_pump(app.clone(), move |app| {
        let store = Store::open(&adopted_db(&app)?).map_err(|err| err.to_string())?;
        let identity = version.identity(account_id, &mailbox_name);
        store.verify_mailbox_identity(&identity)?;
        let attachment = store
            .attachments(account_id, &mailbox_name, uid)
            .map_err(|err| err.to_string())?
            .into_iter()
            .find(|candidate| candidate.index == index)
            .ok_or_else(|| "unknown attachment".to_string())?;
        let sender = sender_account_id.unwrap_or(account_id);
        admit_account(&app, &store, sender)?;
        let sender_ticket = app.state::<AppState>().account_work.capture(sender)?;
        Ok::<_, CommandError>((
            attachment,
            auth_for(&app, account_id)?,
            identity,
            sender_ticket,
        ))
    })
    .await?;

    let completion_ticket = session.ticket.clone();
    let expected_generation = identity.uid_validity;
    let bytes = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let (mut server, _refreshed, _lease) = crate::poll::connect_imap(&session)?;
        let bytes = mail_core::fetch_attachment_checked(
            &mut server,
            &mailbox,
            expected_generation,
            uid,
            index,
        )
        .map_err(|err| err.to_string())?;
        server.logout();
        bytes.ok_or_else(|| "attachment absent from the message".to_string())
    })
    .await
    .map_err(|err| err.to_string())??;

    store_off_pump(app, move |app, store| {
        if !app
            .state::<AppState>()
            .account_work
            .is_current(&completion_ticket)
        {
            return Err("account changed during download".into());
        }
        store
            .verify_mailbox_identity(&identity)
            .map_err(|err| err.to_string())?;
        if !app
            .state::<AppState>()
            .account_work
            .is_current(&sender_ticket)
        {
            return Err("sender account changed during download".into());
        }
        if let Some(token) = edit_token {
            return match store.add_draft_edit_source_attachment(
                &token,
                sender_account_id.unwrap_or(account_id),
                &identity,
                &mail_core::DraftAttachmentFull {
                    name: attachment.name,
                    mime: attachment.mime,
                    bytes,
                },
            ) {
                Ok(saved) => Ok(FetchAttachmentReport {
                    forked: saved.forked,
                    draft_id: Some(saved.id),
                    updated_epoch: Some(saved.updated_epoch),
                    attachment: saved.attachments.last().cloned().map(attachment_row),
                    refused: None,
                }),
                Err(mail_core::Error::AttachmentOverBudget {
                    name, remaining, ..
                }) => {
                    let edit = store.draft_edit(&token)?;
                    Ok(FetchAttachmentReport {
                        forked: false,
                        draft_id: edit.draft.as_ref().map(|draft| draft.id),
                        updated_epoch: edit.draft.map(|draft| draft.updated_epoch),
                        attachment: None,
                        refused: Some(RefusedAttachment {
                            name,
                            remaining: mail_core::human_size(remaining),
                        }),
                    })
                }
                Err(err) => Err(err.into()),
            };
        }
        let created = draft_id.is_none();
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
            Ok(saved) => Ok(FetchAttachmentReport {
                forked: false,
                draft_id: Some(draft_id),
                updated_epoch: Some(saved.updated_epoch),
                attachment: Some(attachment_row(saved.attachment)),
                refused: None,
            }),
            Err(mail_core::Error::AttachmentOverBudget {
                name, remaining, ..
            }) => {
                // The anchor created for nothing is reclaimed — same
                // rule as `attach_files`: no phantom empty draft.
                let draft_id = if created {
                    store
                        .delete_draft(draft_id)
                        .map_err(|err| err.to_string())?;
                    None
                } else {
                    Some(draft_id)
                };
                Ok(FetchAttachmentReport {
                    forked: false,
                    draft_id,
                    updated_epoch: None,
                    attachment: None,
                    refused: Some(RefusedAttachment {
                        name,
                        remaining: mail_core::human_size(remaining),
                    }),
                })
            }
            Err(err) => Err(err.to_string().into()),
        }
    })
    .await
}

/// Removes an attachment. Returns the draft's new `updated_epoch`, or
/// `None` if the attachment no longer existed (double click) — nothing
/// moved.
#[tauri::command]
pub async fn detach_file(app: AppHandle, attachment_id: i64) -> Result<Option<i64>, CommandError> {
    store_off_pump(app, move |_, store| {
        Ok(store.remove_draft_attachment(attachment_id)?)
    })
    .await
}

/// A draft's attachments — resuming redraws its chips.
#[tauri::command]
pub async fn draft_attachments(
    app: AppHandle,
    draft_id: Option<i64>,
    edit_token: Option<String>,
) -> Result<Vec<DraftAttachmentRow>, CommandError> {
    store_off_pump(app, move |_, store| {
        let files = match edit_token {
            Some(token) => store.draft_edit_attachments(&token)?,
            None => store.draft_attachments_meta(draft_id.ok_or(mail_core::Error::StaleDraft)?)?,
        };
        Ok(files.into_iter().map(attachment_row).collect())
    })
    .await
}

#[derive(Serialize)]
pub struct DraftSyncSummary {
    pub pushed: usize,
    pub purged: usize,
    /// Drafts that cannot be pushed as they stand — they stay local.
    pub kept_local: usize,
    /// Network unavailable — nothing changed, the next cycle will
    /// retry.
    pub error: Option<String>,
}

/// Mirrors the drafts of ALL connected accounts into their respective
/// Drafts folders (push only, v1). No work, no network. Reentrance
/// forbidden (lock).
#[tauri::command]
pub async fn sync_drafts(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DraftSyncSummary, CommandError> {
    let (path, jobs) = off_pump(app.clone(), |app| {
        Ok::<_, String>((adopted_db(&app)?, crate::poll::connected_jobs(&app)?))
    })
    .await?;
    let lock = state.drafts_push.clone();

    let (summary, refreshed) =
        tauri::async_runtime::spawn_blocking(move || run_draft_sync_all(jobs, &path, &lock))
            .await
            .map_err(|err| err.to_string())??;

    reset_sessions(&state, refreshed)?;
    Ok(summary)
}

fn run_draft_sync_all(
    jobs: Vec<(i64, AccountWork)>,
    db_path: &Path,
    lock: &Mutex<()>,
) -> Result<(DraftSyncSummary, Vec<AccountWork>), String> {
    // E5: a poisoned lock is recovered (the panic is logged, ADR 0014).
    let _guard = recovered(lock);
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

        let (mut server, refreshed, _lease) = match crate::poll::connect_imap(&session) {
            Ok(pair) => pair,
            Err(reason) => {
                summary.error = Some(reason);
                continue;
            }
        };
        if let Some(fresh) = refreshed {
            refreshed_list.push(fresh);
        }

        // Guarding the markers: UIDVALIDITY first, any purge after.
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
            // Attachments follow the text (PJ-D6): the remote mirror
            // shows the whole draft.
            let attachments = store
                .draft_attachments_full(draft.id)
                .map_err(|err| err.to_string())?;
            let bytes = match mail_smtp::draft_bytes(&mail_smtp::DraftMessage {
                from: session.email(),
                to_raw: &draft.to_raw,
                cc_raw: &draft.cc_raw,
                bcc_raw: &draft.bcc_raw,
                subject: &draft.subject,
                body: &draft.body,
                body_html: draft.body_html.as_deref(),
                attachments: &attachments,
                headers: &draft.thread_headers,
                important: draft.important,
            }) {
                Ok(bytes) => bytes,
                // Not pushable as it stands: the local copy stays the
                // reference.
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

        // The replacements of THIS cycle just created their
        // tombstones: purge immediately — no visible double copy.
        if summary.error.is_none() {
            purge_draft_tombstones(&mut server, &store, account_id, &mut summary)?;
        }
        server.logout();
    }
    Ok((summary, refreshed_list))
}

/// Purges the remote copies in tombstone for ONE account. Returns
/// `false` if the network dropped — the debt stays recorded for the
/// next cycle.
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
// Connections and shared state.
// ---------------------------------------------------------------------

/// Opens an SMTP connection matching the account type. For an OAuth2
/// account, a failure triggers a silent refresh; for a generic account,
/// the password is fixed (no retry possible).
///
/// The servers come from the session's provider, never from an
/// application constant: that's what makes a second provider possible
/// without touching this function.
fn connect_smtp(session: &AccountWork) -> Result<(SmtpMailer, Option<AccountWork>, Lease), String> {
    let lease = session.ticket.lease()?;
    let outcome = match &session.session {
        AccountSession::OAuth(auth) => {
            let smtp = auth.provider.smtp;
            match SmtpMailer::connect_xoauth2(smtp.host, smtp.port, &auth.email, &auth.access_token)
            {
                Ok(mailer) => Ok((mailer, None)),
                // E7: a NETWORK failure is not an authentication
                // refusal — redoing the OAuth session would change
                // nothing and would hammer the provider's endpoint (the
                // P0 defect already fixed on the IMAP side).
                Err(err @ mail_smtp::ConnectError::Connection(_)) => Err(err.to_string()),
                Err(mail_smtp::ConnectError::Authentication(_)) => {
                    session.ticket.check()?;
                    let fresh = Authenticator::from_env(auth.provider)
                        .map_err(|err| err.to_string())?
                        .authenticate_silent(&auth.email)
                        .map_err(|err| err.to_string())?;
                    session.ticket.check()?;
                    let mailer = SmtpMailer::connect_xoauth2(
                        smtp.host,
                        smtp.port,
                        &fresh.email,
                        &fresh.access_token,
                    )
                    .map_err(|err| err.to_string())?;
                    Ok((
                        mailer,
                        Some(session.refreshed(AccountSession::OAuth(fresh))),
                    ))
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
    };
    outcome.map(|(mailer, fresh)| (mailer, fresh, lease))
}

/// The session of an account — opens the database: UNDER `off_pump`
/// (E5).
fn auth_for(app: &Blocking, account_id: i64) -> Result<AccountWork, String> {
    let store = Store::open(&adopted_db(app)?).map_err(|err| err.to_string())?;
    let email = account_email(&store, account_id)?;
    let state = app.state::<AppState>();
    let ticket = state.account_work.capture(account_id)?;
    let session = lock_accounts(&state)?
        .get(&email)
        .cloned()
        .ok_or_else(|| format!("account not connected: {email}"))?;
    Ok(AccountWork { session, ticket })
}

// Delegates to `Store::account_email` (PLAN-INVITATIONS review): ONE
// single answer to "the address of account N" — reading invitations
// and sending their response must see the SAME truth (an empty address
// = a half-provisioned account = unknown, same as `Store::accounts`).
fn account_email(store: &Store, account_id: i64) -> Result<String, String> {
    store
        .account_email(account_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "unknown account".to_string())
}

pub(crate) fn lock_accounts<'a>(
    state: &'a State<'_, AppState>,
) -> Result<MutexGuard<'a, HashMap<String, AccountSession>>, String> {
    // E5: a poisoned lock (a command's panic) is RECOVERED — the panic
    // is already logged by telemetry (ADR 0014); condemning every
    // subsequent command until restart, as before, contradicted ADR
    // 0019.
    Ok(recovered(&state.accounts))
}

/// Puts back the sessions refreshed by a loop (renewed OAuth token) —
/// WITHOUT resurrecting an account removed while it was running: its
/// row in the database has disappeared, an orphaned session in memory
/// would make every subsequent cycle fail until restart.
pub(crate) fn reset_sessions(
    state: &State<'_, AppState>,
    refreshed: Vec<AccountWork>,
) -> Result<(), String> {
    let _commands = recovered(&state.commands);
    let mut accounts = lock_accounts(state)?;
    for fresh in refreshed {
        if state.account_work.is_current(&fresh.ticket) && accounts.contains_key(fresh.email()) {
            accounts.insert(fresh.email().to_string(), fresh.session);
        }
    }
    Ok(())
}

/// Locks a mutex, RECOVERING a poisoned one (the panic is logged by its
/// own thread, ADR 0014): the work under these locks holds no invariant
/// in shared memory. One name for the twelve copies of the same match
/// (D-49, PLAN-AUDIT-V3 E3).
pub(crate) fn recovered<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Runs a blocking job OFF the message pump and UNDER the commands'
/// global lock (PLAN-GELS).
///
/// The two halves are inseparable: `spawn_blocking` frees the pump (an
/// `async` command without it would block a tokio worker — the freeze
/// would leave the window only to reappear in the IPC queue on a
/// two-core machine); the lock restores the serialization the main
/// thread used to offer for free — without it, the commands'
/// read-decide-write pairs would interleave (local state against
/// `mark_flagged`'s action queue, drafts TOCTOU,
/// `SQLITE_BUSY_SNAPSHOT` that `busy_timeout` doesn't cover). A
/// poisoned lock is recovered (same choice as `account_lock`): the work
/// under the lock has no invariant in shared memory.
///
/// The closure receives a [`Blocking`] token (Lot 5 E13b): the adopted
/// database's path is only reachable through it, so an async body
/// cannot open the database by construction. The token derefs to the
/// `AppHandle`.
pub(crate) async fn off_pump<T, E, F>(app: AppHandle, work: F) -> Result<T, E>
where
    F: FnOnce(Blocking) -> Result<T, E> + Send + 'static,
    T: Send + 'static,
    E: From<String> + Send + 'static,
{
    let lock = app.state::<AppState>().commands.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let queued = Instant::now();
        let _guard = recovered(&lock);
        let waited = queued.elapsed();
        let started = Instant::now();
        let outcome = work(Blocking::from_off_pump(app));
        trace_slow::<F>("command", waited, started.elapsed());
        outcome
    })
    .await
    .map_err(|err| E::from(err.to_string()))?
}

/// A command that took more than a second — its lock wait and its
/// work, named by the closure's type (the enclosing function's path),
/// so a field trace says WHICH command waited and behind what (field
/// 2026-09-07: a grant waited a synchronization batch out, and the
/// trace had no line for it). No address, no subject: the name only.
fn trace_slow<F>(kind: &str, waited: Duration, worked: Duration) {
    const SLOW: Duration = Duration::from_secs(1);
    if waited < SLOW && worked < SLOW {
        return;
    }
    let name = std::any::type_name::<F>();
    let name = name
        .strip_suffix("::{{closure}}")
        .unwrap_or(name)
        .rsplit("::")
        .next()
        .unwrap_or(name);
    crate::trace::trace(&format!(
        "slow {kind} {name}: lock wait {} ms, work {} ms",
        waited.as_millis(),
        worked.as_millis()
    ));
}

/// Runs a PURE READ off the pump, WITHOUT the commands' lock (field
/// finding 2026-09-07, Lot 5 E13). In WAL a reader never waits for a
/// writer; what made the reading pane wait a synchronization batch out
/// was the commands' lock, held by a write command that itself waited
/// on SQLite's writer (busy_timeout 30 s). A read has no
/// read-decide-write pair to serialize: it opens its own connection and
/// reads. The closure gets `&Store`, never `&mut`, and the recovery of
/// action effects stays with `with_store` — a read writes nothing.
pub(crate) async fn read_off_pump<T, E, F>(app: AppHandle, work: F) -> Result<T, E>
where
    F: FnOnce(Blocking) -> Result<T, E> + Send + 'static,
    T: Send + 'static,
    E: From<String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let outcome = work(Blocking::from_off_pump(app));
        trace_slow::<F>("read", Duration::ZERO, started.elapsed());
        outcome
    })
    .await
    .map_err(|err| E::from(err.to_string()))?
}

/// `read_off_pump` + the adopted store, read-only — the standard body
/// of a pure-read command.
pub(crate) async fn read_store_off_pump<T>(
    app: AppHandle,
    work: impl FnOnce(&Blocking, &Store) -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError>
where
    T: Send + 'static,
{
    read_off_pump(app, move |app| {
        let store = Store::open(&adopted_db(&app)?)?;
        work(&app, &store)
    })
    .await
}

/// Runs CPU or file work on a bare blocking worker, OUTSIDE the
/// commands' lock (Lot 5 E13d, audit A02; measured in
/// `spikes/global-lock`: an open gesture waited 466 ms p50 behind a
/// 10 MB sanitize under the lock, 12 ms with the sanitize here). Owned
/// inputs in, owned outputs out: no [`Blocking`] token reaches the
/// closure, so no database is reachable from it by construction. A
/// read-decide-write that depends on the unlocked result re-takes the
/// lock and looks again (the A41 rule); a pure read does not.
pub(crate) async fn unlocked<T, E, F>(work: F) -> Result<T, E>
where
    F: FnOnce() -> Result<T, E> + Send + 'static,
    T: Send + 'static,
    E: From<String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|err| E::from(err.to_string()))?
}

/// Opens the store at the app's database and hands it to `work` — the
/// standard body of a blocking command already off the pump
/// (PLAN-AUDIT-V3 E3): one doorway in place of ~105 copies of
/// `Store::open(&adopted_db(&app)?).map_err(…)`. Commands that
/// deliberately batch several reads under ONE open (`ui_state`) call
/// it once with a bigger closure — the fusion stays theirs.
pub(crate) fn with_store<T>(
    app: &Blocking,
    work: impl FnOnce(&mut Store) -> Result<T, CommandError>,
) -> Result<T, CommandError> {
    let mut store = Store::open(&adopted_db(app)?)?;
    let state = app.state::<AppState>();
    if state.mutation_recovery.get().is_none() {
        store.recover_action_effects()?;
        let _ = state.mutation_recovery.set(());
    }
    work(&mut store)
}

/// `off_pump` + `with_store` in one call — the whole standard async
/// command body.
pub(crate) async fn store_off_pump<T>(
    app: AppHandle,
    work: impl FnOnce(&Blocking, &mut Store) -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError>
where
    T: Send + 'static,
{
    off_pump(app, move |app| with_store(&app, |store| work(&app, store))).await
}

fn admit_account(app: &AppHandle, store: &Store, account_id: i64) -> Result<(), CommandError> {
    if !store
        .accounts()?
        .iter()
        .any(|account| account.id == account_id)
    {
        return Err(format!("unknown account: {account_id}").into());
    }
    app.state::<AppState>().account_work.capture(account_id)?;
    Ok(())
}

async fn account_store_off_pump<T>(
    app: AppHandle,
    account_id: i64,
    work: impl FnOnce(&Blocking, &mut Store) -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError>
where
    T: Send + 'static,
{
    store_off_pump(app, move |app, store| {
        admit_account(app, store, account_id)?;
        work(app, store)
    })
    .await
}

/// The database's path for the READ-ONLY probes and the visible pass
/// (`lang_get`, `migration_check`, `migration_run`) — the ones allowed
/// before adoption. Everything else goes through
/// [`adoption::adopted_db`], which refuses until the file is adopted
/// and while it is not the adopted file (Lot 5 E13a).
pub(crate) fn probe_path(app: &AppHandle) -> Result<PathBuf, String> {
    // PLAN-AUDIT-V1 E5: computed ONCE (the folder is created on this
    // first call), then a pure read — 107 calls per session were each
    // doing their own `create_dir_all`.
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    if let Some(path) = PATH.get() {
        return Ok(path.clone());
    }
    // E2E hook: isolated database supplied by the test harness — the
    // real user database must never be touched by a test.
    let path = if let Ok(path) = std::env::var("WIND_DB_PATH") {
        PathBuf::from(path)
    } else {
        let dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        dir.join("wind.db")
    };
    Ok(PATH.get_or_init(|| path).clone())
}

// ---------------------------------------------------------------------
// Body backfill (ADR 0007, horizon lifted by ADR 0010).
// ---------------------------------------------------------------------

/// How many messages are waiting for their body, and how many can carry
/// one — all accounts and ALL mailboxes combined (ADR 0010 §1,
/// denominator R1 PLAN-RETOURS-3: `corpus - pending` = bodies present).
/// Purely local: no network connection.
/// (pending, corpus) in ONE pass: the account's horizon and its mailbox
/// list are read once for BOTH counters (2026-08-30 review: two
/// independent passes paid twice for the prefs and the lists — and on
/// an intermittent error, numerator and denominator could be computed
/// under DIFFERENT horizons). The horizon bounds both, JUST LIKE the
/// pump: without it, the bar of a bounded account would never reach
/// 100%.
fn body_totals(store: &Store) -> Result<(u64, u64), String> {
    let mut pending = 0;
    let mut corpus = 0;
    for account in store.accounts().map_err(|err| err.to_string())? {
        let horizon = body_horizon(store, account.id);
        for mailbox in store
            .mailbox_names(account.id)
            .map_err(|err| err.to_string())?
        {
            pending += store
                .bodies_pending_count(account.id, &mailbox, horizon)
                .map_err(|err| err.to_string())?;
            corpus += store
                .bodies_total_count(account.id, &mailbox, horizon)
                .map_err(|err| err.to_string())?;
        }
    }
    Ok((pending, corpus))
}

#[derive(Serialize)]
pub struct SyncIssue {
    pub account_id: i64,
    pub mailbox: String,
    pub operation: String,
    pub reason: String,
    pub retry_at: Option<i64>,
}

#[derive(Serialize)]
pub struct SyncProgress {
    pub issues: Vec<SyncIssue>,
    /// Messages in the database, all already-visited mailboxes
    /// combined.
    pub local: u64,
    /// Messages announced by the servers for these same mailboxes.
    pub remote: u64,
    /// `None` as long as no mailbox has been selected: the interface
    /// then shows nothing, rather than a “0%” that would suggest a
    /// broken sync.
    pub percent: Option<u8>,
    /// Epoch (seconds) of the last successful poll — `None` as long as
    /// no cycle has completed: the interface doesn't invent a
    /// timestamp (PLAN-SYNCHRO E1).
    pub last: Option<i64>,
    /// A cycle is running RIGHT NOW — whoever drives it. Since the
    /// scheduler moved shell-side (E5), a cycle can run without the
    /// UI having triggered it: the bar must still say so (field rule
    /// of 2026-08-13 — never "up to date" while the machine works).
    pub in_progress: bool,
    /// Mail generation, monotonic (E4): the UI reloads the list when it
    /// moves — that's how mail polled by an IDLE watcher shows up at
    /// rest, via polling (R0-S5).
    pub generation: u64,
}

/// Progress of the full synchronization (ADR 0010 §5).
///
/// Purely local — no network connection: the interface can call it in
/// a loop while a synchronization runs, at no round-trip cost.
fn read_sync(store: &Store, generation: u64, in_progress: bool) -> Result<SyncProgress, String> {
    let (local, remote) = store.sync_progress().map_err(|err| err.to_string())?;
    // An unreadable timestamp (corrupted pref) counts as "never": the
    // status bar falls back to the dateless text rather than showing
    // garbage.
    let last = store
        .text_pref(mail_core::PREF_LAST_SYNC)
        .map_err(|err| err.to_string())?
        .and_then(|value| value.parse::<i64>().ok());
    Ok(SyncProgress {
        issues: store
            .operation_issues()
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|issue| SyncIssue {
                account_id: issue.account_id,
                mailbox: issue.mailbox,
                operation: issue.operation,
                reason: issue.reason,
                retry_at: issue.retry_at,
            })
            .collect(),
        local,
        remote,
        percent: mail_core::sync_percent(local, remote),
        last,
        in_progress,
        generation,
    })
}

#[tauri::command]
pub async fn sync_progress(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SyncProgress, CommandError> {
    // `State` doesn't cross `spawn_blocking` (lifetime): we carry the
    // cycle's Arc, not the state.
    let generation = state.sync_cycle.generation.load(Ordering::Relaxed);
    let in_progress = state.sync_cycle.in_progress.load(Ordering::Relaxed);
    read_store_off_pump(app, move |_, store| {
        Ok(read_sync(store, generation, in_progress)?)
    })
    .await
}

/// P0-bis + E4: the UI reports the OS's network state
/// (`navigator.onLine`). Offline, the IDLE watchers sleep (reconnecting
/// in a loop with no network is pointless); on return, the backoffs are
/// cleared — the network is fresh, yesterday's failure was the outage,
/// not the server — and the watchers resume on their own.
#[tauri::command]
pub fn network_state(
    app: AppHandle,
    state: State<'_, AppState>,
    online: bool,
) -> Result<(), CommandError> {
    let was_online = state.online.swap(online, Ordering::Relaxed);
    if online && let Ok(mut reculs) = state.sync_backoffs.lock() {
        reculs.clear();
    }
    // The mail held back during the outage arrives on RETURN (P0-bis)
    // — a light pass leaves right away, and the cadence counts it
    // (PLAN-AUDIT-V3 E5: the trigger moved here with the clock; the
    // UI's listener now only reports the state).
    if online && !was_online {
        let due = {
            let mut cadence = recovered(&state.cadence);
            cadence.network_returned(Instant::now())
        };
        crate::poll::kick(&app, due);
    }
    Ok(())
}

#[derive(Serialize)]
pub struct SyncActivity {
    /// Accounts already settled in the current cycle.
    pub done: u64,
    pub total: u64,
    /// Address of the account currently being polled.
    pub account: String,
    /// Mailbox currently being processed WITHIN the account — empty
    /// between two mailboxes.
    pub mailbox: String,
    /// Step without a mailbox (`inventory`, `threads`, `drafts`) —
    /// catalog key, translated by the UI. Empty when a mailbox is
    /// named.
    pub phase: String,
    /// INBOX mail already in the database in THIS cycle (arrivals +
    /// removals, accumulated account after account) — P1: the probe
    /// reloads the list as soon as this counter moves, without waiting
    /// for the cycle to end.
    pub mail: u64,
}

/// The cycle in progress, for the status bar (PLAN-SYNCHRO E1).
///
/// Purely in memory — no network, no database: the UI probes it every
/// second WHILE the cycle runs at no cost to the loop (atomics, same
/// pattern as `migration_progress`). `None` at rest.
#[tauri::command]
pub fn sync_activity(state: State<'_, AppState>) -> Option<SyncActivity> {
    let cycle = &state.sync_cycle;
    if !cycle.in_progress.load(Ordering::Relaxed) {
        return None;
    }
    let account = cycle
        .account
        .lock()
        .map(|name| name.clone())
        .unwrap_or_default();
    let mailbox = cycle
        .mailbox
        .lock()
        .map(|name| name.clone())
        .unwrap_or_default();
    let phase = cycle
        .phase
        .lock()
        .map(|name| name.clone())
        .unwrap_or_default();
    Some(SyncActivity {
        done: cycle.done.load(Ordering::Relaxed),
        total: cycle.total.load(Ordering::Relaxed),
        account,
        mailbox,
        phase,
        mail: cycle.mail.load(Ordering::Relaxed),
    })
}

#[derive(Serialize)]
pub struct BackfillStatus {
    pub remaining: u64,
    /// The percentage of bodies ALREADY present over the corpus in
    /// scope (R1, PLAN-RETOURS-3) — `None` without a denominator (no
    /// message).
    pub percent: Option<u8>,
}

/// Backfill status, without downloading anything — enough to show
/// “N remaining · P%” before even starting.
#[tauri::command]
pub async fn backfill_status(app: AppHandle) -> Result<BackfillStatus, CommandError> {
    read_off_pump(app, move |app| {
        // Measurement milestones (feature `mesure` — never in the
        // shipped binary). The upstream span `wry::custom_protocol::handle`
        // gives the command's TOTAL and nothing more: measured cold on
        // 2026-08-26, it was 2,740 ms after the predicate fix, without
        // saying which of the three times carried it. We verified it
        // was NEITHER the queue (the lock was free) NOR `corpus_total`
        // (35 ms) — that left the opening, which no span covers. Hence
        // these three.
        let store = {
            #[cfg(feature = "mesure")]
            let _milestone = tracing::debug_span!("mesure::store_open").entered();
            Store::open(&adopted_db(&app)?).map_err(|err| err.to_string())?
        };
        let (remaining, total) = {
            #[cfg(feature = "mesure")]
            let _milestone = tracing::debug_span!("mesure::body_totals").entered();
            body_totals(&store)?
        };
        Ok(BackfillStatus {
            remaining,
            // `done = total - remaining`: the bodies already there. The
            // pure function caps at 99 as long as bodies remain (R1).
            percent: mail_core::backfill_percent(total.saturating_sub(remaining), total),
        })
    })
    .await
}

// ---------------------------------------------------------------------
// Visible and interruptible migration (Phase 5, ADR 0012).
//
// Each command opens its own connection: without this screen, it would
// be the FIRST command to come along that paid for adopting a legacy
// database — silently, in a UI freeze. The UI therefore calls
// `migration_check` BEFORE any command that touches the database; if
// there is work, it shows the screen, launches `migration_run`, polls
// `migration_progress`, and `migration_cancel` rewinds everything (§8
// of the handover: never a partial adoption persisted).
// ---------------------------------------------------------------------

#[derive(Serialize)]
pub struct MigrationCheck {
    /// Messages to adopt — `null` if the opening will be silent.
    pub pending: Option<u64>,
}

/// After the adoption of a restored copy (Lot 5 E14b): every send the
/// copy could still deliver is held; the marker says whether one was.
/// A failure is traced, never a refusal of the start.
fn hold_after_restore(path: &Path) {
    match crate::restore::hold_if_marked(path) {
        Ok(0) => {}
        Ok(held) => {
            crate::trace::trace(&format!("restore: {held} send(s) held for the user's word"))
        }
        Err(err) => crate::trace::trace(&format!("restore: could not hold the sends: {err}")),
    }
}

/// Read-only probe: nothing is triggered, nothing is created.
#[tauri::command]
pub async fn migration_check(app: AppHandle) -> Result<MigrationCheck, CommandError> {
    off_pump(app, move |app| {
        let path = probe_path(&app)?;
        let pending = Store::pending_adoption(&path).map_err(|err| err.to_string())?;
        if pending.is_none() {
            // Nothing to pay: this file is the one every command may
            // open from now on (Lot 5 E13a).
            app.state::<AppState>()
                .migration
                .adopted
                .adopt(&path)
                .map_err(|err| err.to_string())?;
            hold_after_restore(&path);
        }
        Ok(MigrationCheck { pending })
    })
    .await
}

#[derive(Serialize)]
pub struct MigrationProgress {
    pub done: u64,
    pub total: u64,
    /// `None` as long as the pass has announced nothing: the screen
    /// then shows nothing rather than a “0%” that would suggest a
    /// failure.
    pub percent: Option<u8>,
}

/// Progress of the current pass. Purely local and lock-free: the pass
/// writes atomics, polling never makes it wait.
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

/// Requests cancellation: the pass notices it at its next checkpoint
/// and rewinds everything — `migration_run` then returns `false`.
#[tauri::command]
pub fn migration_cancel(state: State<'_, AppState>) {
    state.migration.cancel.store(true, Ordering::Relaxed);
}

/// Runs the adoption pass, visible and interruptible.
///
/// Returns `true` if the database is migrated (or had nothing to do),
/// `false` if the user cancelled — everything is then undone,
/// `user_version` unchanged, and the whole pass replays on the next
/// launch.
#[tauri::command]
pub async fn migration_run(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, CommandError> {
    let path = probe_path(&app)?;
    let shared = state.migration.clone();
    shared.cancel.store(false, Ordering::Relaxed);
    shared.done.store(0, Ordering::Relaxed);
    shared.total.store(0, Ordering::Relaxed);

    tauri::async_runtime::spawn_blocking(move || {
        if let Some(required) = Store::draft_file_migration_headroom(&path).map_err(|err| err.to_string())? {
            let available = fs4::available_space(path.parent().unwrap_or(&path)).map_err(|err| err.to_string())?;
            if available < required { return Err(format!("not enough free space for draft migration: {required} bytes required, {available} available")); }
        }
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
            // The Store closes right away: the next commands will open
            // their own, as usual — but with no pass left to pay, it's
            // done. The file is adopted from here on (Lot 5 E13a).
            Ok(_store) => {
                shared.adopted.adopt(&path).map_err(|err| err.to_string())?;
                hold_after_restore(&path);
                Ok(true)
            }
            Err(mail_core::Error::Interrupted) => Ok(false),
            Err(err) => Err(err.to_string()),
        }
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(CommandError::from)
}

#[derive(Serialize)]
pub struct BackfillSummary {
    pub fetched: usize,
    pub scanned: usize,
    pub more: bool,
    pub remaining: u64,
    /// The percentage of bodies present after this batch (R1) — updates
    /// the status bar batch by batch, without re-polling from the UI.
    pub percent: Option<u8>,
    pub errors: Vec<String>,
}

/// ONE backfill batch, all connected accounts combined.
///
/// Deliberately bounded: the UI calls again as long as there is work
/// left, and stops when the user asks. Interruption is thus free — no
/// cancellation token to propagate — and an outage never costs more
/// than one batch.
#[tauri::command]
pub async fn backfill_bodies(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BackfillSummary, CommandError> {
    let (path, jobs) = off_pump(app.clone(), |app| {
        Ok::<_, String>((adopted_db(&app)?, crate::poll::connected_jobs(&app)?))
    })
    .await?;
    let lock = state.bodies_backfill.clone();

    let (summary, refreshed) =
        tauri::async_runtime::spawn_blocking(move || run_backfill_all(jobs, &path, &lock))
            .await
            .map_err(|err| err.to_string())??;

    reset_sessions(&state, refreshed)?;
    Ok(summary)
}

fn run_backfill_all(
    jobs: Vec<(i64, AccountWork)>,
    db_path: &Path,
    lock: &Mutex<()>,
) -> Result<(BackfillSummary, Vec<AccountWork>), String> {
    // E5: a poisoned lock is recovered (the panic is logged, ADR 0014).
    let _guard = recovered(lock);

    let mut store = Store::open(db_path).map_err(|err| err.to_string())?;
    let mut summary = BackfillSummary {
        fetched: 0,
        scanned: 0,
        more: false,
        remaining: 0,
        percent: None,
        errors: Vec::new(),
    };
    let mut refreshed_list = Vec::new();
    let mut budget = mail_core::WorkBudget::new(BACKFILL_BUDGET);
    let mut mailboxes = Vec::new();
    for (account_id, _) in &jobs {
        for name in store
            .mailbox_names(*account_id)
            .map_err(|err| err.to_string())?
        {
            if let Some(identity) = store
                .mailbox_identity(*account_id, &name)
                .map_err(|err| err.to_string())?
            {
                mailboxes.push(identity);
            }
        }
    }
    store
        .backfill_order("bodies", &mut mailboxes)
        .map_err(|err| err.to_string())?;
    let mut failed_accounts = std::collections::HashSet::new();
    let mut connection = None;
    let with_issue: std::collections::HashSet<(i64, String)> = store
        .operation_issues()
        .map_err(|err| err.to_string())?
        .into_iter()
        .filter(|issue| issue.operation == "bodies")
        .map(|issue| (issue.account_id, issue.mailbox))
        .collect();
    for identity in mailboxes {
        if budget.remaining() == 0 {
            summary.more = true;
            break;
        }
        let account_id = identity.account_id;
        if failed_accounts.contains(&account_id) {
            continue;
        }
        let Some((_, session)) = jobs.iter().find(|(id, _)| *id == account_id) else {
            continue;
        };
        let Ok(_turn_lease) = session.ticket.lease() else {
            continue;
        };
        let horizon = body_horizon(&store, account_id);
        if !store
            .operation_due(
                account_id,
                &identity.mailbox,
                "bodies",
                chrono::Utc::now().timestamp(),
            )
            .map_err(|err| err.to_string())?
        {
            continue;
        }
        if store
            .bodies_to_backfill(account_id, &identity.mailbox, horizon, 1)
            .map_err(|err| err.to_string())?
            .is_empty()
        {
            // Nothing eligible any more (served, removed or size-refused per
            // UID): a diagnostic left by an earlier pass has nothing to wait for.
            if with_issue.contains(&(account_id, identity.mailbox.clone())) {
                store
                    .settle_operation(
                        account_id,
                        &identity.mailbox,
                        "bodies",
                        chrono::Utc::now().timestamp(),
                        None,
                    )
                    .map_err(|err| err.to_string())?;
            }
            continue;
        }
        store
            .mark_backfill_turn("bodies", identity.mailbox_id)
            .map_err(|err| err.to_string())?;
        if connection
            .as_ref()
            .is_none_or(|(id, _, _)| *id != account_id)
        {
            if let Some((_, server, _lease)) = connection.take() {
                ImapServer::logout(server);
            }
            match crate::poll::connect_imap(session) {
                Ok((server, refreshed, lease)) => {
                    if let Some(fresh) = refreshed {
                        refreshed_list.push(fresh);
                    }
                    connection = Some((account_id, server, lease));
                }
                Err(reason) => {
                    // A connection failure is the ACCOUNT's state (connection
                    // notice, backoff), not a partial failure of each folder:
                    // recorded per mailbox it masked "Sync failed" offline with
                    // one "issue" per folder (gate finding, 2026-09-07).
                    summary
                        .errors
                        .push(format!("{}: {reason}", session.email()));
                    failed_accounts.insert(account_id);
                    continue;
                }
            }
        }
        if let Some((_, server, _lease)) = connection.as_mut() {
            match mail_core::backfill_bodies_budgeted(
                server,
                &mut store,
                account_id,
                &identity.mailbox,
                horizon,
                &mut budget,
            ) {
                Ok(report) => {
                    summary.more |= report.more;
                }
                Err(err) => {
                    summary.errors.push(format!(
                        "{}, {}: {err}",
                        session.email(),
                        identity.mailbox
                    ));
                    // The disk is shared by every mailbox: no point reconnecting
                    // folder after folder to be refused before any I/O.
                    if matches!(err, mail_core::Error::InsufficientDisk { .. }) {
                        break;
                    }
                    connection = None;
                }
            }
        }
    }
    if let Some((_, server, _lease)) = connection {
        server.logout();
    }

    summary.fetched = budget.saved();
    summary.scanned = budget.scanned();
    let (remaining, total) = body_totals(&store)?;
    summary.remaining = remaining;
    summary.percent = mail_core::backfill_percent(total.saturating_sub(summary.remaining), total);
    summary.errors.sort();
    Ok((summary, refreshed_list))
}

// ---------------------------------------------------------------------
// Signed automatic update (ADR 0013).
//
// Driven from Rust, like notifications: the webview never calls the
// updater API, only these two commands — the capabilities stay
// `core:default`. The minisign signature is verified by the plugin
// BEFORE any installation; without it, `download_and_install` fails
// rather than applying a forged package.
// ---------------------------------------------------------------------

use tauri_plugin_updater::UpdaterExt;

#[derive(Serialize)]
pub struct UpdateInfo {
    pub version: String,
    /// Release notes, if the Release carries any.
    pub notes: Option<String>,
    /// ISO 8601 publish date, as announced by the manifest.
    pub date: Option<String>,
}

/// Is there an update? `None` = up to date, or offline.
///
/// Called ONCE at startup, silently: a check the user had to request
/// wouldn't happen (lesson of ADR 0007). Offline, the endpoint is
/// unreachable — that's not a defect, so the error goes back up to the
/// UI which stays quiet rather than nagging; it is never SWALLOWED
/// (§9), only judged non-critical by the caller.
#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<Option<UpdateInfo>, CommandError> {
    // E2E tests talk to NO server (handover §7.5). Without this guard,
    // as soon as a Release exists, the `latest.json` endpoint would
    // answer and the banner would appear mid-test — a flake.
    // `WIND_DB_PATH` is only set by the harness: it's the same
    // isolation signal as the throwaway database.
    if std::env::var("WIND_DB_PATH").is_ok() {
        return Ok(None);
    }
    match wind_updater(&app)?
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

/// The installed version, for the "About" section of Settings.
/// A manifest read — no network, no database.
#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Opens a link from a message body in the SYSTEM application
/// (browser, mail client) — field finding 2026-08-15: without this
/// path, the click would navigate the sandboxed iframe to the site,
/// refused (X-Frame-Options / CSP), and WebView2 would replace the
/// body with its "This content has been blocked" page.
///
/// The GUARD lives here, not in the UI: only http, https and mailto go
/// through — any other scheme (file, smb, UNC paths…) is refused by
/// name. `open::that_detached` wraps ShellExecuteW without blocking the
/// command thread — ONLY with the crate's `shellexecute-on-windows`
/// feature (Cargo.toml); without it, it's a `powershell.exe` launched
/// synchronously (2026-09-01 audit).
#[tauri::command]
pub fn open_link(url: String) -> Result<(), CommandError> {
    let trimmed = url.trim();
    let lower = trimmed.to_ascii_lowercase();
    let allowed = lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:");
    if !allowed {
        return Err(format!("link scheme refused: {trimmed}").into());
    }
    Ok(open::that_detached(trimmed).map_err(|err| err.to_string())?)
}

/// Arrival notifications: the preference is READ to display it…
#[tauri::command]
pub async fn notif_pref_get(app: AppHandle) -> Result<bool, CommandError> {
    store_off_pump(app, move |_, store| {
        Ok(store.bool_pref(mail_core::PREF_ARRIVAL_BUBBLES, true)?)
    })
    .await
}

/// …and is SET from the Notifications group in Settings. Persisted in
/// the database (PLAN-REGLAGES, R-D2): it's the Rust shell that emits
/// the notifications, localStorage would be invisible to it.
#[tauri::command]
pub async fn notif_pref_set(app: AppHandle, enabled: bool) -> Result<(), CommandError> {
    store_off_pump(app, move |_, store| {
        Ok(store.set_bool_pref(mail_core::PREF_ARRIVAL_BUBBLES, enabled)?)
    })
    .await
}

/// Interface language (PLAN-LANGUES, A15): the preference is READ at
/// startup — `None` as long as it has never been set, the UI then
/// detects the system language and sets it after the migration modal.
/// READ-ONLY probe, not `Store::open`: this command runs BEFORE
/// `migration_check` (the language is restored before the first
/// render), and the full open would pay for adopting a legacy database
/// silently — without the modal, against ADR 0012 (field finding
/// 2026-08-15). And `off_pump` anyway (ADR 0019): the probe carries a
/// 30 s busy_timeout — a database in rollback under a writer would
/// otherwise freeze the pump.
#[tauri::command]
pub async fn lang_get(app: AppHandle) -> Result<Option<String>, CommandError> {
    off_pump(app, move |app| {
        Ok(Store::text_pref_readonly(
            &probe_path(&app)?,
            mail_core::PREF_LANG,
        )?)
    })
    .await
}

/// …and is SET from Settings > Display. In the database (not
/// localStorage), same reason as the bubbles: the shell will compose
/// notifications in this language (E2).
#[tauri::command]
pub async fn lang_set(app: AppHandle, lang: String) -> Result<(), CommandError> {
    store_off_pump(app, move |_, store| {
        Ok(store.set_text_pref(mail_core::PREF_LANG, &lang)?)
    })
    .await
}

/// The known names for a batch of addresses (thread header,
/// PLAN-RETOURS-12 R5): a pure read of the contacts directory, bounded
/// to the displayed page of messages. An unknown address is absent
/// from the outcome — the UI falls back to the bare address.
#[tauri::command]
pub async fn address_names(
    app: AppHandle,
    addresses: Vec<String>,
) -> Result<std::collections::HashMap<String, String>, CommandError> {
    store_off_pump(app, move |_, store| Ok(store.address_names(&addresses)?)).await
}

/// Downloads, verifies the signature, installs, and only QUITS IF
/// that install succeeded.
///
/// The download stays with the plugin: the minisign verification is on
/// that path (updater.rs:712) and stays there. On Windows the LAUNCH
/// is ours (PLAN-SIGNATURE E4, D4): the plugin's `install()` calls
/// `ShellExecuteW` without reading its return value then exits via
/// `exit(0)` — a Windows refusal (Smart App Control, field finding
/// 2026-08-26) closed the application without a word and without
/// installing anything. Here the refusal goes back up to the banner
/// (`erreur.maj`), which re-arms. On macOS the plugin's `install` IS
/// the flow (PLAN-MACOS D3) — see `install_downloaded`.
///
/// The database doesn't move from the app-data folder (NSIS, not MSIX
/// — ADR 0013; the macOS bundle swap never touches Application
/// Support): an update can never orphan the messages.
#[tauri::command]
pub async fn update_install(app: AppHandle, version: String) -> Result<(), CommandError> {
    // Same isolation guard as `update_check` (handover §7.5): a test
    // NEVER downloads or launches anything.
    if std::env::var("WIND_DB_PATH").is_ok() {
        return Err("update unavailable in test".into());
    }
    // One installation at a time, across every surface (banner AND
    // Settings): a second one would write the same marker and double
    // the launch.
    if UPDATE_IN_PROGRESS.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err("an installation is already in progress".into());
    }
    let result = download_and_launch(app, version).await;
    // On success the application quits: we only come back here on
    // failure.
    UPDATE_IN_PROGRESS.store(false, std::sync::atomic::Ordering::SeqCst);
    Ok(result?)
}

/// The installation never doubles up (banner + Settings are two
/// surfaces for the same action) — the flag is released on failure,
/// and no longer matters on success: the application quits.
static UPDATE_IN_PROGRESS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

async fn download_and_launch(app: AppHandle, version: String) -> Result<(), String> {
    // Instrumentation (PLAN-RETOURS-12 R2, decision D1): the package
    // size is measured FLAT across 12 releases (±1% since 0.7.0) — if
    // the "Downloading and installing…" banner drags on, the time goes
    // here: network (GitHub CDN), writing, or an antivirus scan/SAC
    // verdict at spawn. Each step is traced on stderr AND in `maj.log`
    // next to the database (`trace_update`): the measurement can be
    // read after the fact, whatever the launch mode.
    let trace_folder = app.path().app_data_dir().ok();
    trace_update(
        trace_folder.as_deref(),
        &format!(
            "update: {} -> {version}: installation requested",
            app.package_info().version
        ),
    );
    let stopwatch = std::time::Instant::now();
    let update = wind_updater(&app)?
        .check()
        .await
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "no update to install".to_string())?;
    trace_update(
        trace_folder.as_deref(),
        &format!(
            "update: manifest verified in {} ms",
            stopwatch.elapsed().as_millis()
        ),
    );
    // The manifest may have moved between the banner and the click: we
    // only install the ANNOUNCED version — never another one silently.
    // The UI re-checks on this failure and restates the new version.
    if update.version != version {
        return Err(format!(
            "the proposed version has changed ({version} → {}); check again",
            update.version
        ));
    }
    let download_start = std::time::Instant::now();
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|err| err.to_string())?;
    trace_update(
        trace_folder.as_deref(),
        &format!(
            "update: {} bytes downloaded in {} ms",
            bytes.len(),
            download_start.elapsed().as_millis()
        ),
    );
    install_downloaded(app, update, bytes, trace_folder).await
}

/// The Windows install (ADR 0013): Wind's artifact is the bare NSIS
/// exe, written to a fresh temp folder and launched with OUR
/// arguments — the plugin's internals are only trusted up to
/// `download()` (minisign verification), pinned at `=2.10.1`.
#[cfg(windows)]
async fn install_downloaded(
    app: AppHandle,
    update: tauri_plugin_updater::Update,
    bytes: Vec<u8>,
    trace_folder: Option<std::path::PathBuf>,
) -> Result<(), String> {
    // Format net: the plugin used to sniff zip/exe/msi (extract,
    // updater.rs:882); Wind's artifact is the bare NSIS exe
    // (createUpdaterArtifacts: true, nsis target only). Any other
    // signed content would fail with a cryptic Windows error at spawn.
    if !bytes.starts_with(b"MZ") {
        return Err("the downloaded package is not a Windows executable".to_string());
    }
    // Writing (~6 MB) and CreateProcess (a synchronous antivirus scan
    // is possible) are blocking: off the pump (ADR 0019), like every
    // command that touches a file.
    off_pump(app, move |app| {
        // A FRESH folder per attempt — the plugin's random-tempdir
        // regime: no collision with a phantom installer from a
        // previous attempt, no path guessable long in advance between
        // the write and the launch.
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_nanos();
        let folder =
            std::env::temp_dir().join(format!("wind-maj-{}-{timestamp}", std::process::id()));
        std::fs::create_dir_all(&folder)
            .map_err(|err| format!("preparing the directory ({}): {err}", folder.display()))?;
        let installer_path = folder.join(format!("Wind_{}_maj-setup.exe", update.version));
        let write_start = std::time::Instant::now();
        std::fs::write(&installer_path, &bytes).map_err(|err| {
            format!(
                "writing the installer ({}): {err}",
                installer_path.display()
            )
        })?;
        trace_update(
            trace_folder.as_deref(),
            &format!(
                "update: installer written in {} ms",
                write_start.elapsed().as_millis()
            ),
        );
        // The spawn carries the synchronous antivirus scan and the
        // Smart App Control cloud verdict (per binary): it's suspect
        // n°1 for the banner that drags on — the measurement will
        // tell.
        let spawn_start = std::time::Instant::now();
        installer_command(&installer_path)
            .spawn()
            .map_err(|err| format!("installer launch refused by Windows: {err}"))?;
        trace_update(
            trace_folder.as_deref(),
            &format!(
                "update: installer launched in {} ms",
                spawn_start.elapsed().as_millis()
            ),
        );
        // ONLY on a SUCCESSFUL launch: the installer (/UPDATE mode)
        // waits for the process to end to replace the binary, then /R
        // relaunches Wind — the new version.
        app.exit(0);
        Ok(())
    })
    .await
}

/// The macOS install (PLAN-MACOS D3, CE 2026-09-04): the artifact is
/// the `.app.tar.gz` the bundler emits, and the PLUGIN's own
/// `install` replaces the bundle in place — the standard flow, kept
/// standard on purpose (nothing NSIS-shaped exists here; the pinned
/// `=2.10.1` audit covers this path too). Signature already verified
/// inside `download()`. Extraction touches the disk: off the pump
/// (ADR 0019). Then relaunch — the new version.
#[cfg(target_os = "macos")]
async fn install_downloaded(
    app: AppHandle,
    update: tauri_plugin_updater::Update,
    bytes: Vec<u8>,
    trace_folder: Option<std::path::PathBuf>,
) -> Result<(), String> {
    off_pump(app, move |app| {
        let install_start = std::time::Instant::now();
        update.install(&bytes).map_err(|err| err.to_string())?;
        trace_update(
            trace_folder.as_deref(),
            &format!(
                "update: bundle replaced in {} ms",
                install_start.elapsed().as_millis()
            ),
        );
        // ONLY on a successful replacement: relaunch into the new
        // version (the Windows twin exits and lets NSIS relaunch).
        app.restart();
    })
    .await
}

/// Wind's updater — ONE build for both commands.
/// The plugin sets NO timeout (`timeout: None`): a stalled transfer
/// would leave `check` silent at startup and freeze the banner on
/// "Installing…" forever — the two faces of the 2026-08-26 field
/// finding. Ten minutes cover ~6 MB on a very slow link; beyond that,
/// the failure goes back up and can be retried.
fn wind_updater(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    app.updater_builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|err| err.to_string())
}

/// Traces one update step: on stderr (visible via `run-wind.ps1`) AND
/// appended with a date to `maj.log`, next to the database. The
/// windowed app has no stderr: three accepted updates (0.13.0 →
/// 0.15.0) went by without any measurement surviving — the file makes
/// the trace readable AFTER THE FACT, whatever the launch mode (field
/// finding 2026-08-30). A few dozen bytes appended, five times per
/// update: nothing in common with the installer write (~6 MB) that ADR
/// 0019 sends off the pump. Any error is ignored — the trace must never
/// make an installation fail.
fn trace_update(folder: Option<&Path>, line: &str) {
    eprintln!("{line}");
    let Some(folder) = folder else { return };
    let _ = std::fs::create_dir_all(folder);
    let dated = format!(
        "{} {line}\n",
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ")
    );
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(folder.join("maj.log"))
        .and_then(|mut file| std::io::Write::write_all(&mut file, dated.as_bytes()));
}

/// The NSIS installer invocation — the pure decision, pinned down by
/// the test `the_installer_is_invoked_passive_relaunching_and_updating`.
#[cfg(windows)]
fn installer_command(installer_path: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new(installer_path);
    command.args(["/P", "/R", "/UPDATE"]);
    command
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_backfill_connection_failure_is_reported_without_a_folder_issue() {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let peer = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .write_all(b"* BYE synthetic maintenance\r\n")
                .unwrap();
        });
        let path = std::env::temp_dir().join(format!(
            "wind-backfill-connection-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let mut store = Store::open(&path).unwrap();
        let account = store
            .adopt_or_create_account("fixture@example.test", "imap")
            .unwrap();
        let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(
                mailbox,
                &[mail_core::Envelope {
                    uid: 1,
                    subject: Some("fixture".into()),
                    sender: None,
                    sender_address: None,
                    to_addrs: Vec::new(),
                    cc_addrs: Vec::new(),
                    reply_to: None,
                    message_id: None,
                    in_reply_to: None,
                    date: None,
                    seen: false,
                    flagged: false,
                }],
            )
            .unwrap();
        drop(store);
        let registry = crate::account_work::Registry::default();
        let job = AccountWork {
            ticket: registry.capture(account).unwrap(),
            session: AccountSession::Generic(GenericCredentials {
                email: "fixture@example.test".into(),
                username: "fixture".into(),
                password: "synthetic".into(),
                imap_host: "127.0.0.1".into(),
                imap_port: port,
                smtp_host: "127.0.0.1".into(),
                smtp_port: port,
            }),
        };
        let (report, _) = run_backfill_all(vec![(account, job)], &path, &Mutex::new(())).unwrap();
        peer.join().unwrap();
        assert_eq!(report.errors.len(), 1);
        let store = Store::open(&path).unwrap();
        assert!(
            store.operation_issues().unwrap().is_empty(),
            "a connection failure is the account's state, not one issue per folder"
        );
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn generic_repair_drains_old_jobs_without_waiting_for_its_own_lease() {
        let registry = crate::account_work::Registry::default();
        let old = registry.capture(1).unwrap();
        let started = old.lease().unwrap();
        let own = old.lease().unwrap();
        let retirement = retire_generic_connection(&registry, &old, own).unwrap();
        assert!(old.lease().is_err());
        assert!(retirement.wait(Duration::from_millis(10)).is_err());
        drop(started);
        retirement.wait(Duration::from_millis(10)).unwrap();
        retirement.commit();
        let new = registry.capture(1).unwrap();
        assert!(new.lease().is_ok());
        assert!(!registry.is_current(&old));
        assert!(registry.is_current(&new));
    }

    #[test]
    fn generic_verification_checks_both_protocols_and_requires_both() {
        use std::cell::Cell;
        for imap_ok in [true, false] {
            for smtp_ok in [true, false] {
                let checks = Cell::new(0);
                let result = verify_generic_connections(
                    || {
                        checks.set(checks.get() + 1);
                        if imap_ok {
                            Ok(())
                        } else {
                            Err("receive refused".into())
                        }
                    },
                    || {
                        checks.set(checks.get() + 1);
                        if smtp_ok {
                            Ok(())
                        } else {
                            Err("send refused".into())
                        }
                    },
                );
                assert_eq!(checks.get(), 2, "both diagnostics must be available");
                assert_eq!(result.is_ok(), imap_ok && smtp_ok);
                if !imap_ok {
                    assert!(result.as_ref().unwrap_err().contains("IMAP"));
                }
                if !smtp_ok {
                    assert!(result.as_ref().unwrap_err().contains("SMTP"));
                }
            }
        }
    }

    #[test]
    fn body_boundary_preserves_image_only_content() {
        let (_, rich) = super::body_boundary(
            String::new(),
            Some(r#"<img src="data:image/png;base64,AA==">"#),
        );
        assert!(
            rich.is_some(),
            "an image-only draft must not become empty text"
        );
    }

    /// PLAN-AUDIT-V2 E8: the save path of an attachment comes from the
    /// UI (the "Save as" dialog); it is written with bytes chosen by
    /// the sender. Defense in depth: absolute, no traversal, in a
    /// folder that exists.
    #[test]
    fn a_relative_or_traversal_path_is_refused() {
        assert!(super::output_path("piece.pdf").is_err());
        assert!(super::output_path("C:\\Users\\x\\..\\..\\Windows\\piece.pdf").is_err());
        assert!(super::output_path("C:\\folder-that-does-not-exist-at-all\\piece.pdf").is_err());
        let here = std::env::temp_dir().join("piece.pdf");
        assert!(super::output_path(&here.to_string_lossy()).is_ok());
    }

    use super::*;

    /// The installer invocation (PLAN-SIGNATURE E4, D4): the installer
    /// path itself, in passive mode (`/P`), relaunching the application
    /// after applying (`/R`), update mode (`/UPDATE`) — the very same
    /// arguments the plugin builds for passive `installMode`. And
    /// NEVER `/ARGS`: the plugin makes it follow the current binary's
    /// arguments, Wind launches with no argument — an empty `/ARGS`
    /// would be an unmeasured assumption about the NSIS parser.
    /// The update-banner measurement no longer depends on the launch
    /// (field finding 2026-08-30: three updates accepted with no
    /// capture — a windowed app's stderr is null): each step is
    /// appended DATED to `maj.log`, readable after the fact. Two calls
    /// = two lines — the file appends from one update to the next, it
    /// never overwrites.
    #[test]
    fn the_update_trace_survives_in_maj_log_and_gets_appended() {
        let folder = std::env::temp_dir().join(format!("wind-maj-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);

        trace_update(Some(&folder), "update: manifest verified in 42 ms");
        trace_update(Some(&folder), "update: installer launched in 7 ms");

        let content = std::fs::read_to_string(folder.join("maj.log")).unwrap();
        let lines: Vec<_> = content.lines().collect();
        assert_eq!(lines.len(), 2, "each step adds ONE line");
        assert!(lines[0].ends_with("update: manifest verified in 42 ms"));
        assert!(lines[1].ends_with("update: installer launched in 7 ms"));
        // Dated: the file gets read again weeks later, and successive
        // updates are distinguishable in it.
        assert!(
            lines
                .iter()
                .all(|l| l.starts_with("20") && l.contains("Z update: ")),
            "each line carries its UTC timestamp: {content:?}"
        );

        let _ = std::fs::remove_dir_all(&folder);
    }

    #[cfg(windows)]
    #[test]
    fn the_installer_is_invoked_passive_relaunching_and_updating() {
        let installer_path = std::path::Path::new("C:\\tmp\\Wind_0.10.2_x64-setup.exe");
        let command = installer_command(installer_path);
        assert_eq!(command.get_program(), installer_path.as_os_str());
        let arguments: Vec<_> = command.get_args().collect();
        assert_eq!(arguments, ["/P", "/R", "/UPDATE"]);
    }

    /// The Sent-folder retry table (PLAN-REACTIVITE E2): Gmail's
    /// asynchronous copy can lag the SMTP acceptance by a few seconds
    /// — two bounded retries, then silence (the cycle will catch up). A
    /// counter at zero doesn't exist by construction (the first
    /// attempt is n°1); if it happened, we stop — never a loop.
    #[test]
    fn the_retry_is_bounded() {
        assert_eq!(retry_after(1), Some(Duration::from_secs(5)));
        assert_eq!(retry_after(2), Some(Duration::from_secs(15)));
        assert_eq!(retry_after(3), None);
        assert_eq!(retry_after(0), None);
        assert_eq!(retry_after(u32::MAX), None);
    }

    /// The type of an attachment is deduced from its extension, case
    /// insensitively; an unknown one goes out as a byte stream — an
    /// honest header, never a decision.
    #[test]
    fn mime_for_name_follows_the_extension_and_falls_back_generic() {
        assert_eq!(mime_for_name("devis.pdf"), "application/pdf");
        assert_eq!(mime_for_name("PHOTO.JPG"), "image/jpeg");
        assert_eq!(mime_for_name("archive.tar.gz"), "application/octet-stream");
        assert_eq!(mime_for_name("notes.txt"), "text/plain");
        assert_eq!(mime_for_name("no-extension"), "application/octet-stream");
        assert_eq!(mime_for_name(""), "application/octet-stream");
    }

    /// An attachment's name is a string chosen by the SENDER. Written
    /// as-is, it would allow an arbitrary file write triggered by a
    /// simple click on a received message. These cases are not
    /// theoretical: they come from mail-client exploitation archives.
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
        assert_eq!(safe_file_name(".."), "attachment");
        assert_eq!(safe_file_name("/"), "attachment");
        assert_eq!(safe_file_name(""), "attachment");
    }

    /// Windows refuses these names regardless of extension: without a
    /// fallback, saving would fail with an incomprehensible error.
    #[test]
    fn windows_device_names_fall_back() {
        assert_eq!(safe_file_name("CON"), "attachment");
        assert_eq!(safe_file_name("nul.txt"), "attachment");
        assert_eq!(safe_file_name("COM1.pdf"), "attachment");
        assert_eq!(safe_file_name("LPT9"), "attachment");
        // Neither reserved nor tricky: must pass through as-is.
        assert_eq!(safe_file_name("COM0.pdf"), "COM0.pdf");
        assert_eq!(safe_file_name("console.log"), "console.log");
    }

    /// A legitimate name, even accented, must pass through INTACT — a
    /// filter that mutilates normal names would be paid for every day.
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

    /// Saving the same attachment twice must never overwrite the first
    /// file — the loss would be silent.
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

    /// The declared address becomes both the account's key AND the
    /// XOAUTH2 identifier: it is not verifiable by anyone else before
    /// consent. A minimal filter avoids the phantom account, without
    /// pretending to validate RFC 5322 — the provider will decide.
    #[test]
    fn declared_address_must_be_plausible() {
        assert!(is_plausible_address("moi@exemple.fr"));
        assert!(is_plausible_address("prenom.nom@outlook.com"));

        assert!(!is_plausible_address(""), "empty");
        assert!(!is_plausible_address("moi"), "no at sign");
        assert!(!is_plausible_address("@exemple.fr"), "no local part");
        assert!(!is_plausible_address("moi@"), "no domain");
        assert!(!is_plausible_address("moi@exemple"), "domain with no dot");
        assert!(
            !is_plausible_address("moi@.fr"),
            "domain starting with a dot"
        );
    }

    /// R1 (PLAN-RETOURS-8): an account's marker only admits the
    /// dedicated set (D2, A3 "one icon, one meaning") and the measured
    /// palette (D1) — everything else is refused, including a
    /// corrupted value read back from the database.
    #[test]
    fn valid_marker_is_an_allowlist() {
        assert!(valid_marker("home", "rouge"));
        assert!(valid_marker("music_note", "brun"));
        assert!(
            !valid_marker("download", "rouge"),
            "a product glyph, outside the dedicated set (A3)"
        );
        assert!(!valid_marker("home", "turquoise"), "unknown hue");
        assert!(!valid_marker("", ""));
    }

    /// Never set -> None; set -> read back; removed -> None (the keys
    /// get cleared, same pattern as the signature); corrupted in the
    /// database -> None (the allowlist also holds on the way back).
    #[test]
    fn marker_absent_set_removed_corrupted() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(marker_of(&store, 1).unwrap(), None);

        set_marker(&mut store, 1, Some(("work", "bleu"))).unwrap();
        assert_eq!(
            marker_of(&store, 1).unwrap(),
            Some(("work".to_string(), "bleu".to_string()))
        );

        set_marker(&mut store, 1, None).unwrap();
        assert_eq!(marker_of(&store, 1).unwrap(), None);

        store.set_text_pref("repere_icone.1", "delete").unwrap();
        store.set_text_pref("repere_teinte.1", "rouge").unwrap();
        assert_eq!(marker_of(&store, 1).unwrap(), None);
    }

    /// PLAN-RETOURS-9 (D3): the pure decision behind the custom name.
    /// Spaces trimmed, empty (or blank) = removed, beyond 60 characters
    /// refused — never silently truncated.
    #[test]
    fn normalized_name_trims_empties_and_caps() {
        assert_eq!(
            normalized_name("  Boulot  "),
            Ok(Some("Boulot".to_string()))
        );
        assert_eq!(normalized_name(""), Ok(None));
        assert_eq!(normalized_name("   "), Ok(None));
        assert_eq!(normalized_name(&"x".repeat(60)), Ok(Some("x".repeat(60))));
        assert!(normalized_name(&"x".repeat(61)).is_err());
    }

    /// Never set -> None; set -> read back; cleared -> None (the key
    /// gets cleared, same pattern as marker/signature); a blank shell
    /// in the database never leaks out to the UI.
    #[test]
    fn account_name_absent_set_removed() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(name_of(&store, 1).unwrap(), None);

        set_name(&mut store, 1, Some("Boulot")).unwrap();
        assert_eq!(name_of(&store, 1).unwrap(), Some("Boulot".to_string()));

        set_name(&mut store, 1, None).unwrap();
        assert_eq!(name_of(&store, 1).unwrap(), None);

        store.set_text_pref("nom_compte.1", "   ").unwrap();
        assert_eq!(name_of(&store, 1).unwrap(), None);
    }

    /// PLAN-AUDIT-V1 E5: the in-flight state of a post-gesture pass is
    /// a GUARD. Before, a `?` between taking it and releasing it left
    /// `in_flight` raised for life: every subsequent pass for the account
    /// was absorbed until restart. RED with no lesson (the behavior is
    /// that of `Drop`) — the test states the contract.
    #[test]
    fn a_late_flight_cannot_absorb_or_release_a_new_account_incarnation() {
        let registry = crate::account_work::Registry::default();
        let old = registry.capture(1).unwrap();
        let flights = Mutex::new(HashMap::<Ticket, PassFlight>::new());
        let old_flight = FlightGuard::take(&flights, &old).unwrap();
        registry.retire(1).unwrap().commit();
        let new = registry.capture(1).unwrap();
        let new_flight = FlightGuard::take(&flights, &new).unwrap();
        drop(old_flight);
        assert!(FlightGuard::take(&flights, &new).is_none());
        assert!(new_flight.rerequest_consumed());
        assert!(!new_flight.rerequest_consumed());
        assert_eq!(recovered(&flights).len(), 1);
    }

    #[test]
    fn the_flight_falls_when_the_guard_is_released_even_by_an_early_exit() {
        let registry = crate::account_work::Registry::default();
        let ticket = registry.capture(1).unwrap();
        let flights = Mutex::new(HashMap::<Ticket, PassFlight>::new());
        let in_flight = |flights: &Mutex<HashMap<Ticket, PassFlight>>| {
            flights
                .lock()
                .unwrap()
                .get(&ticket)
                .map(|v| v.in_flight)
                .unwrap_or(false)
        };

        let early_exit = |flights: &Mutex<HashMap<Ticket, PassFlight>>| -> Result<(), String> {
            let _flight = FlightGuard::take(flights, &ticket).expect("first take");
            assert!(in_flight(flights));
            // A second request during the flight is absorbed and noted.
            assert!(FlightGuard::take(flights, &ticket).is_none());
            assert!(flights.lock().unwrap()[&ticket].rerequest);
            Err("failure mid-pass".to_string())?;
            Ok(())
        };
        assert!(early_exit(&flights).is_err());
        assert!(!in_flight(&flights), "the early exit released the flight");

        // The rerequest noted during the flight is consumed ONCE.
        let flight = FlightGuard::take(&flights, &ticket).expect("the flight is free");
        assert!(flight.rerequest_consumed());
        assert!(!flight.rerequest_consumed());
        drop(flight);
        assert!(!in_flight(&flights));
    }
}
