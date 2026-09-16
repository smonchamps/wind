//! PLAN-THROTTLE-2026-09 — an account that was told "not now" BREATHES.
//!
//! Field 2026-09-08 (tester T2, Gmail, the P0 account): Gmail throttled the
//! account, Wind typed it as a definitive refusal (three of her own gestures
//! quarantined for good) and kept knocking within the hour, every hour —
//! re-spending the daily download quota as soon as the suspension lifted.
//!
//! The cooldown is a per-account PREF (`throttle.{id}` = `{until}:{strikes}`),
//! persisted so a relaunch does not knock again. It is read at the one
//! chokepoint every server-bound operation already passes through —
//! [`Store::operation_due`] — and written where the typed error is still
//! typed: [`Store::settle_operation`]. The shell reads it before opening a
//! connection at all (cycle, light pass, IDLE watcher, backfill pump).

use crate::{Error, Store};
use std::time::Duration;

const HOUR: u64 = 3600;

/// How long the account breathes after `strikes` CONSECUTIVE throttles —
/// pure decision (Chief Engineer decision D2, 2026-09-15: option B).
/// Google documents a suspension of 1 h, up to 24 h: one hour on the first
/// strike, doubling, capped at a day; reset by the first success. Zero
/// strikes: nothing.
pub fn wait_after_throttle(strikes: u32) -> Duration {
    if strikes == 0 {
        return Duration::ZERO;
    }
    let factor = 1u64 << (strikes - 1).min(5);
    Duration::from_secs((HOUR * factor).min(24 * HOUR))
}

fn key(account: i64) -> String {
    format!("throttle.{account}")
}

/// The backfill's daily download budget for a Gmail account (Chief
/// Engineer decision D4, 2026-09-15): Google documents a 2,500 MB/day IMAP
/// download cap, past which the account is suspended for 1 h to 24 h —
/// INBOX polls included. 2,000 MB keeps 500 MB of room for the arrivals'
/// bodies (fetched by the cycle, never budgeted), the reads on click and
/// the user's other clients. No other provider documents such a cap.
pub const GMAIL_DAILY_DOWNLOAD_BUDGET: u64 = 2_000 * 1024 * 1024;

/// How many bytes the backfill may still download today for this account
/// — `None` when the provider has no budget (unbounded). Pure decision.
pub fn daily_download_left(provider: &str, spent: u64) -> Option<u64> {
    (provider == "gmail").then(|| GMAIL_DAILY_DOWNLOAD_BUDGET.saturating_sub(spent))
}

/// Spent for the day: less than one pass's worth left (a budget with a
/// few bytes left used to run a full pass — review, 2026-09-15). A provider
/// without a budget is never spent.
pub fn daily_budget_spent(provider: &str, spent: u64) -> bool {
    daily_download_left(provider, spent).is_some_and(|left| left < crate::PASS_BYTES)
}

fn download_key(account: i64) -> String {
    format!("download.{account}")
}

fn parse(value: &str) -> Option<(i64, u32)> {
    let (until, strikes) = value.split_once(':')?;
    Some((until.parse().ok()?, strikes.parse().ok()?))
}

impl Store {
    /// Records a throttle at `now` and returns when the account may knock
    /// again. ONE strike per knock: a second throttle inside a running
    /// cooldown (several mailboxes of the same cycle) keeps the same
    /// deadline and adds nothing.
    pub fn note_throttle(&self, account: i64, now: i64) -> Result<i64, Error> {
        let current = self.text_pref(&key(account))?.as_deref().and_then(parse);
        if let Some((until, _)) = current
            && now < until
        {
            return Ok(until);
        }
        let strikes = current.map_or(1, |(_, strikes)| strikes.saturating_add(1));
        let until = now.saturating_add(wait_after_throttle(strikes).as_secs() as i64);
        self.set_text_pref(&key(account), &format!("{until}:{strikes}"))?;
        Ok(until)
    }

    /// The deadline of a RUNNING cooldown, if any.
    pub fn cooldown_until(&self, account: i64, now: i64) -> Result<Option<i64>, Error> {
        Ok(self
            .text_pref(&key(account))?
            .as_deref()
            .and_then(parse)
            .map(|(until, _)| until)
            .filter(|until| now < *until))
    }

    /// Every account in a running cooldown, with its deadline — for the
    /// status bar and Settings. One query: this runs on every 5 s probe.
    pub fn cooldowns(&self, now: i64) -> Result<Vec<(i64, i64)>, Error> {
        let mut stmt = self
            .conn()
            .prepare("SELECT key, value FROM prefs WHERE key LIKE 'throttle.%' ORDER BY key")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (key, value) = row?;
            let Some(account) = key
                .strip_prefix("throttle.")
                .and_then(|id| id.parse::<i64>().ok())
            else {
                continue;
            };
            if let Some((until, _)) = parse(&value)
                && now < until
            {
                out.push((account, until));
            }
        }
        Ok(out)
    }

    /// The manual gesture is an order: the wait ends NOW, the strikes stay
    /// — the next throttle escalates as if the wait had run (review B5: a
    /// click must not re-arm a one-hour clock forever).
    pub fn lift_throttle(&self, account: i64, now: i64) -> Result<(), Error> {
        if let Some((_, strikes)) = self.text_pref(&key(account))?.as_deref().and_then(parse) {
            self.set_text_pref(&key(account), &format!("{now}:{strikes}"))?;
        }
        Ok(())
    }

    /// Bytes the backfill downloaded for this account on `day` (a LOCAL
    /// `YYYY-MM-DD`, the day the user reads on the "resumes tomorrow"
    /// line). Only the current day is kept: another day reads zero.
    pub fn daily_download(&self, account: i64, day: &str) -> Result<u64, Error> {
        Ok(self
            .text_pref(&download_key(account))?
            .as_deref()
            .and_then(|value| value.split_once(':'))
            .filter(|(kept, _)| *kept == day)
            .and_then(|(_, bytes)| bytes.parse().ok())
            .unwrap_or(0))
    }

    pub fn add_daily_download(&self, account: i64, day: &str, bytes: u64) -> Result<(), Error> {
        let total = self.daily_download(account, day)?.saturating_add(bytes);
        self.set_text_pref(&download_key(account), &format!("{day}:{total}"))
    }

    /// The first success AFTER the wait clears the strikes: the server is
    /// serving again. A success inside a running cooldown is no evidence
    /// (review C2: the INBOX settle right behind a throttled body fetch
    /// would erase the cooldown just written) and leaves it untouched. A
    /// read first — this runs on every settled operation.
    pub fn clear_throttle(&self, account: i64, now: i64) -> Result<(), Error> {
        if let Some((until, _)) = self.text_pref(&key(account))?.as_deref().and_then(parse)
            && now >= until
        {
            self.conn()
                .execute("DELETE FROM prefs WHERE key = ?1", [key(account)])?;
        }
        Ok(())
    }

    /// The cooldown read without opening a full store — the IDLE watcher
    /// peeks before every reconnection attempt.
    pub fn cooldown_until_readonly(
        path: &std::path::Path,
        account: i64,
        now: i64,
    ) -> Result<Option<i64>, Error> {
        Ok(Self::text_pref_readonly(path, &key(account))?
            .as_deref()
            .and_then(parse)
            .map(|(until, _)| until)
            .filter(|until| now < *until))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(store: &Store) -> i64 {
        store
            .adopt_or_create_account("breathe@example.invalid", "gmail")
            .unwrap()
    }

    /// The table of Chief Engineer decision D2: 1 h, doubling, 24 h cap.
    #[test]
    fn the_cooldown_doubles_from_one_hour_to_a_day() {
        assert_eq!(wait_after_throttle(0), Duration::ZERO);
        assert_eq!(wait_after_throttle(1), Duration::from_secs(3600));
        assert_eq!(wait_after_throttle(2), Duration::from_secs(7200));
        assert_eq!(wait_after_throttle(3), Duration::from_secs(14400));
        assert_eq!(wait_after_throttle(5), Duration::from_secs(57600));
        assert_eq!(wait_after_throttle(6), Duration::from_secs(86400));
        assert_eq!(wait_after_throttle(u32::MAX), Duration::from_secs(86400));
    }

    /// One strike per KNOCK, not per failing mailbox; expiry brings the
    /// next strike; success resets everything.
    #[test]
    fn strikes_count_knocks_and_a_success_resets_them() {
        let store = Store::open_in_memory().unwrap();
        let id = account(&store);
        assert_eq!(store.cooldown_until(id, 1_000).unwrap(), None);

        assert_eq!(store.note_throttle(id, 1_000).unwrap(), 4_600);
        assert_eq!(
            store.note_throttle(id, 1_010).unwrap(),
            4_600,
            "a second mailbox throttled in the same cycle adds no strike"
        );
        assert_eq!(store.cooldown_until(id, 4_599).unwrap(), Some(4_600));
        assert_eq!(store.cooldown_until(id, 4_600).unwrap(), None);

        assert_eq!(
            store.note_throttle(id, 5_000).unwrap(),
            5_000 + 7_200,
            "the knock after expiry is the second strike: two hours"
        );
        assert_eq!(store.cooldowns(5_001).unwrap(), vec![(id, 12_200)]);

        // A success INSIDE a running cooldown is no evidence (review C2: the
        // INBOX settle right after a throttled body fetch would erase the
        // cooldown just written): the strikes stay.
        store.clear_throttle(id, 6_000).unwrap();
        assert_eq!(store.cooldown_until(id, 6_000).unwrap(), Some(12_200));
        // A success after expiry is: the server serves again.
        store.clear_throttle(id, 12_200).unwrap();
        assert_eq!(store.cooldown_until(id, 12_200).unwrap(), None);
        assert_eq!(
            store.note_throttle(id, 13_000).unwrap(),
            16_600,
            "after a success the next throttle starts again at one hour"
        );
    }

    /// The chokepoints: a throttled operation is settled INTO the cooldown
    /// (its own retry follows the deadline), every operation of the account
    /// is "not due" while it runs, and the first settled success clears it.
    #[test]
    fn a_throttled_operation_puts_the_whole_account_on_hold() {
        let store = Store::open_in_memory().unwrap();
        let id = account(&store);
        store
            .settle_operation(
                id,
                "INBOX",
                "sync",
                1_000,
                Some(&Error::Throttled("[THROTTLED]".into())),
            )
            .unwrap();
        assert_eq!(store.cooldown_until(id, 1_001).unwrap(), Some(4_600));
        assert!(
            store.operation_issues().unwrap().is_empty(),
            "a throttle is the cooldown, never an issue: no alert, no raw reason (D5)"
        );
        assert!(
            !store.operation_due(id, "Archive", "bodies", 2_000).unwrap(),
            "another mailbox, another operation: not due either"
        );
        assert!(store.operation_due(id, "Archive", "bodies", 4_600).unwrap());

        store
            .settle_operation(id, "INBOX", "sync", 4_700, None)
            .unwrap();
        assert_eq!(store.cooldown_until(id, 4_701).unwrap(), None);
        assert!(store.operation_issues().unwrap().is_empty());
    }

    /// Chief Engineer decision D4: Gmail's documented cap is 2,500 MB per
    /// day; the backfill keeps 500 MB of room for INBOX polls, reads on
    /// click and the user's other clients. Other providers: no documented
    /// cap, no budget.
    #[test]
    fn only_gmail_has_a_daily_download_budget() {
        assert_eq!(daily_download_left("gmail", 0), Some(2_000 * 1024 * 1024));
        assert_eq!(
            daily_download_left("gmail", 1_999 * 1024 * 1024),
            Some(1024 * 1024)
        );
        assert_eq!(daily_download_left("gmail", 2_000 * 1024 * 1024), Some(0));
        assert_eq!(daily_download_left("gmail", u64::MAX), Some(0));
        assert_eq!(daily_download_left("microsoft", u64::MAX), None);
        assert_eq!(daily_download_left("imap", 0), None);
    }

    /// The day's tally per account: it adds up within the day and starts
    /// from zero on the next one — the day is the LOCAL date, the same
    /// the user reads on the "resumes tomorrow" line.
    #[test]
    fn the_daily_tally_adds_up_and_resets_with_the_day() {
        let store = Store::open_in_memory().unwrap();
        let id = account(&store);
        assert_eq!(store.daily_download(id, "2026-09-15").unwrap(), 0);
        store.add_daily_download(id, "2026-09-15", 700).unwrap();
        store.add_daily_download(id, "2026-09-15", 300).unwrap();
        assert_eq!(store.daily_download(id, "2026-09-15").unwrap(), 1_000);
        assert_eq!(
            store.daily_download(id, "2026-09-16").unwrap(),
            0,
            "a new day reads zero"
        );
        store.add_daily_download(id, "2026-09-16", 5).unwrap();
        assert_eq!(store.daily_download(id, "2026-09-16").unwrap(), 5);
        assert_eq!(
            store.daily_download(id, "2026-09-15").unwrap(),
            0,
            "only the current day is kept"
        );
    }

    /// The manual retry (Settings, the Sync button) is an order: it lifts
    /// the cooldown along with the diagnostics' waits — but keeps the
    /// strikes (review B5): an impatient click must not re-arm a one-hour
    /// clock forever; the next throttle escalates as if the wait had run.
    #[test]
    fn a_manual_retry_lifts_the_cooldown_but_keeps_the_strikes() {
        let store = Store::open_in_memory().unwrap();
        let id = account(&store);
        store.note_throttle(id, 1_000).unwrap();
        store.retry_operations(id).unwrap();
        let now = chrono::Utc::now().timestamp();
        assert_eq!(store.cooldown_until(id, now).unwrap(), None);
        assert_eq!(
            store.note_throttle(id, now + 10).unwrap(),
            now + 10 + 7_200,
            "second strike: two hours"
        );
    }

    /// The pump's own line: less than one pass's worth left counts as spent
    /// (review, altitude 5: a budget with 1 byte left used to run a full
    /// 64 MB pass).
    #[test]
    fn a_budget_short_of_one_pass_is_spent() {
        assert!(!daily_budget_spent("gmail", 0));
        assert!(!daily_budget_spent(
            "gmail",
            GMAIL_DAILY_DOWNLOAD_BUDGET - crate::PASS_BYTES
        ));
        assert!(daily_budget_spent(
            "gmail",
            GMAIL_DAILY_DOWNLOAD_BUDGET - crate::PASS_BYTES + 1
        ));
        assert!(daily_budget_spent("gmail", u64::MAX));
        assert!(!daily_budget_spent("microsoft", u64::MAX));
    }
}
