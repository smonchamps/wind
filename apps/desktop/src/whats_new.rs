//! The "What's new" window's content (PLAN-BATCH-2026-09 E2): the two
//! changelogs travel INSIDE the binary (`include_str!`) and the webview
//! only ever receives the parsed section for the running version — it
//! never touches the filesystem, the same doctrine as the updater
//! (`commands.rs`, ADR 0013 block).
//!
//! Pure decisions here, I/O elsewhere (STANDARD §4): the parser knows
//! nothing of Tauri or the database.

/// The English changelog — the reference (decision D5: English is the
/// fallback when a French entry is missing).
const CHANGELOG_EN: &str = include_str!("../../../CHANGELOG.md");
/// The French translation, maintained release by release since 0.20.0.
const CHANGELOG_FR: &str = include_str!("../../../CHANGELOG.fr.md");

/// The body of one version's section: everything between the
/// `## [version]` heading and the next `## [` heading (or the end of
/// the file), trimmed. `None` when the version has no entry — the
/// caller decides the fallback, never this parser.
pub fn changelog_section(changelog: &str, version: &str) -> Option<String> {
    let heading = format!("## [{version}]");
    let mut lines = changelog.lines();
    // The heading line: exactly `## [version]`, alone or followed by
    // the date — `## [0.2.0] - …` matches "0.2.0", never "0.2".
    lines.by_ref().find(|line| {
        line.strip_prefix(&heading)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    })?;
    let body: Vec<&str> = lines.take_while(|line| !line.starts_with("## [")).collect();
    let body = body.join("\n").trim().to_string();
    // A heading with no body is NO section (review 2026-09-08): a
    // French heading pending its translation must not mask the English
    // notes, and the window never opens on nothing.
    if body.is_empty() { None } else { Some(body) }
}

/// What the startup check should do (pure — review 2026-09-08). The
/// seen preference did not exist before this feature: an ABSENT pref
/// on a database that already has accounts is the pre-feature fleet
/// updating in — the debut update, not a fresh install. And only a
/// NEWER version opens the window: a downgrade (SAC blocks unsigned
/// updates per binary on this fleet) must not present weeks-old notes
/// as news, nor rewind the seen version into showing the next upgrade
/// twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Debut {
    /// Fresh install: remember the version silently, show nothing.
    SeedQuietly,
    /// Nothing new to tell.
    Quiet,
    /// The window opens on this version's notes.
    Show,
}

pub fn decision(seen: Option<&str>, has_accounts: bool, version: &str) -> Debut {
    match seen {
        None if has_accounts => Debut::Show,
        None => Debut::SeedQuietly,
        Some(seen) if is_newer(version, seen) => Debut::Show,
        Some(_) => Debut::Quiet,
    }
}

/// Numeric MAJOR.MINOR.PATCH comparison (§2.9's grammar). A seen value
/// that does not parse counts as older when the versions differ —
/// showing once too often beats never showing.
fn is_newer(candidate: &str, seen: &str) -> bool {
    fn parts(version: &str) -> Option<Vec<u64>> {
        version
            .split('.')
            .map(|part| part.parse::<u64>().ok())
            .collect()
    }
    match (parts(candidate), parts(seen)) {
        (Some(candidate), Some(seen)) => candidate > seen,
        _ => candidate != seen,
    }
}

/// The notes the window shows, in the interface's language: the French
/// section when the language is French AND the entry exists, the
/// English section otherwise. `None` = no entry anywhere (the window
/// does not open on nothing).
pub fn notes_for(version: &str, lang: mail_core::Lang) -> Option<String> {
    if matches!(lang, mail_core::Lang::Fr)
        && let Some(section) = changelog_section(CHANGELOG_FR, version)
    {
        return Some(section);
    }
    changelog_section(CHANGELOG_EN, version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mail_core::Lang;

    const SAMPLE: &str = "# Changelog\n\npreamble\n\n## [0.2.0] - 2026-01-02\n\nThe blurb.\n\n### Added\n\n- **One.** Thing.\n\n## [0.1.0] - 2026-01-01\n\nFirst.\n";

    #[test]
    fn the_section_runs_from_its_heading_to_the_next_version() {
        let section = changelog_section(SAMPLE, "0.2.0").unwrap();
        assert!(section.starts_with("The blurb."), "{section:?}");
        assert!(section.contains("- **One.** Thing."));
        assert!(!section.contains("First."), "bled into 0.1.0: {section:?}");
        assert!(!section.contains("## ["), "carried a heading: {section:?}");
    }

    #[test]
    fn the_last_section_runs_to_the_end_of_the_file() {
        assert_eq!(changelog_section(SAMPLE, "0.1.0").unwrap(), "First.");
    }

    #[test]
    fn an_unknown_version_has_no_section() {
        assert_eq!(changelog_section(SAMPLE, "9.9.9"), None);
        // A version that is a prefix of another never matches it.
        assert_eq!(changelog_section(SAMPLE, "0.2"), None);
    }

    /// Review 2026-09-08: a heading with no body yielded `Some("")` —
    /// a French heading appended before its translation would MASK the
    /// English notes, and the window could open on nothing.
    #[test]
    fn a_heading_only_section_is_no_section() {
        let pending = "# C\n\n## [0.3.0] - 2026-01-03\n\n## [0.2.0] - 2026-01-02\n\nDone.\n";
        assert_eq!(changelog_section(pending, "0.3.0"), None);
    }

    /// The update direction (review 2026-09-08): only a NEWER version
    /// opens the window. A downgrade (SAC blocking an unsigned update
    /// is a lived reality on this fleet) stays quiet — weeks-old notes
    /// presented as news would lie, and the ack would rewind the seen
    /// version into showing the next upgrade twice.
    #[test]
    fn the_window_debuts_and_stays_directional() {
        use Debut::*;
        // A truly fresh install: nothing to tell, seed silently.
        assert!(matches!(decision(None, false, "0.21.0"), SeedQuietly));
        // The pre-feature fleet updating in: the pref does not exist
        // yet, the accounts do — this IS the debut update.
        assert!(matches!(decision(None, true, "0.21.0"), Show));
        // Seen this one already; a downgrade; an upgrade.
        assert!(matches!(decision(Some("0.21.0"), true, "0.21.0"), Quiet));
        assert!(matches!(decision(Some("0.21.0"), true, "0.20.0"), Quiet));
        assert!(matches!(decision(Some("0.20.9"), true, "0.21.0"), Show));
        // Unparseable seen value: showing beats never showing.
        assert!(matches!(decision(Some("garbage"), true, "0.21.0"), Show));
    }

    /// The release net: the version this binary carries MUST have an
    /// English changelog entry — a release whose window would open
    /// empty fails here, before it ships (§2.9: the CHANGELOG entry
    /// always precedes `make-release.ps1`).
    #[test]
    fn the_running_version_has_an_english_entry() {
        let version = env!("CARGO_PKG_VERSION");
        assert!(
            notes_for(version, Lang::En).is_some(),
            "no CHANGELOG.md section for {version}"
        );
    }

    #[test]
    fn french_falls_back_to_english_when_the_entry_is_missing() {
        // 0.20.0 is translated: French serves French.
        let french = notes_for("0.20.0", Lang::Fr).unwrap();
        assert!(french.contains("Sauvegardez"), "{french:?}");
        // 0.19.0 predates CHANGELOG.fr.md: French serves the English
        // section rather than nothing.
        let fallback = notes_for("0.19.0", Lang::Fr);
        assert_eq!(fallback, changelog_section(CHANGELOG_EN, "0.19.0"));
        assert!(fallback.is_some());
    }
}
