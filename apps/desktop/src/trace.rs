//! La trace terrain (PLAN-AUDIT-V1 E9, STANDARD §6.8) : une ligne datée
//! sur stderr ET en append dans `wind.log` à côté de la base, bornée à
//! un méga (décision CE D4 : tronquée, le fichier repart de zéro).
//!
//! Pourquoi un fichier : l'app livrée est sous-système *windows*, elle
//! n'a pas de stderr — trois mises à jour (0.13.0 → 0.15.0) sont passées
//! sans qu'aucune mesure ne survive, jusqu'au poka-yoke `maj.log`
//! (`trace_maj`). Le même patron, généralisé : relève, passe d'après-
//! geste, vidange, veilleurs, horizon illisible.
//!
//! Ce qui n'y entre JAMAIS (§6.8) : ni sujet, ni expéditeur, ni corps —
//! des identifiants, des durées, des décomptes, des erreurs.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Au-delà, le fichier est tronqué (D4).
pub(crate) const BORNE_OCTETS: u64 = 1_000_000;
const NOM: &str = "wind.log";

static DOSSIER: OnceLock<PathBuf> = OnceLock::new();

/// Le dossier de la base, posé UNE fois au démarrage — avant, la trace
/// ne sort que sur stderr.
pub(crate) fn initialiser(dossier: PathBuf) {
    let _ = DOSSIER.set(dossier);
}

/// Une ligne de trace : stderr (console d'un `cargo run`) + `wind.log`.
/// Toute erreur d'écriture s'ignore — une trace ne fait jamais échouer
/// le geste qu'elle décrit.
pub(crate) fn trace(ligne: &str) {
    eprintln!("{ligne}");
    if let Some(dossier) = DOSSIER.get() {
        ecrire_dans(dossier, ligne);
    }
}

pub(crate) fn ecrire_dans(dossier: &Path, ligne: &str) {
    let _ = std::fs::create_dir_all(dossier);
    let chemin = dossier.join(NOM);
    let trop_gros = std::fs::metadata(&chemin)
        .map(|m| m.len() >= BORNE_OCTETS)
        .unwrap_or(false);
    let datee = format!(
        "{} {ligne}\n",
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ")
    );
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(!trop_gros)
        .write(true)
        .truncate(trop_gros)
        .open(&chemin)
        .and_then(|mut fichier| fichier.write_all(datee.as_bytes()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dossier_temporaire(nom: &str) -> PathBuf {
        let dossier = std::env::temp_dir().join(format!("wind-trace-{nom}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dossier);
        std::fs::create_dir_all(&dossier).unwrap();
        dossier
    }

    /// D4 : passé un méga, le fichier repart de zéro — jamais un journal
    /// qui grossit à vie à côté de la base.
    #[test]
    fn la_trace_est_bornee_a_un_mega() {
        let dossier = dossier_temporaire("borne");
        let ligne = "x".repeat(10_000);
        for _ in 0..110 {
            ecrire_dans(&dossier, &ligne);
        }
        let taille = std::fs::metadata(dossier.join(NOM)).unwrap().len();
        assert!(
            taille < BORNE_OCTETS + 20_000,
            "tronquée au passage du méga : {taille} octets"
        );
        assert!(taille > 0);
        let _ = std::fs::remove_dir_all(&dossier);
    }

    /// Chaque ligne est datée en UTC ISO 8601 — lisible après coup,
    /// alignable sur l'horodatage d'un geste.
    #[test]
    fn chaque_ligne_est_datee() {
        let dossier = dossier_temporaire("datee");
        ecrire_dans(&dossier, "releve compte 1 : INBOX 0.4s");
        let contenu = std::fs::read_to_string(dossier.join(NOM)).unwrap();
        assert!(
            contenu.starts_with("20")
                && contenu.contains("T")
                && contenu.contains("Z releve compte 1"),
            "{contenu}"
        );
        let _ = std::fs::remove_dir_all(&dossier);
    }
}
