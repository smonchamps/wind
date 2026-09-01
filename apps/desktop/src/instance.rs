//! Mono-instance par verrou fichier (PLAN-AUDIT-V1 E1, décision CE D1
//! du 2026-09-01) : `wind.lock` à côté de `wind.db`, pris en exclusif
//! par le premier processus, refusé au second. L'OS relâche le verrou
//! à la mort du processus — un crash ne laisse jamais de verrou
//! « collant », le fichier lui-même peut rester, il ne dit rien.
//!
//! Pourquoi un fichier et pas un plugin (single-instance) : `fs4` est
//! déjà là (garde d'espace disque), le verrou est une décision pure et
//! testable sans Tauri, et la seconde instance n'a rien d'autre à faire
//! que le dire et sortir (D1 : message puis sortie).
//!
//! `WIND_DB_PATH` (e2e, sonde de gel) place le verrou à côté de la base
//! jetable : les instances de test ne se voient pas entre elles ni ne
//! voient l'application réelle.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use fs4::fs_std::FileExt;

/// Le fichier de verrou, à côté de la base. Son contenu ne dit rien :
/// seul le verrou exclusif de l'OS compte.
pub(crate) const NOM_VERROU: &str = "wind.lock";

/// La garde de l'instance : tant qu'elle vit, le verrou est tenu. La
/// relâcher (ou mourir) le rend.
pub(crate) struct GardeInstance {
    _fichier: File,
}

/// Tente le verrou exclusif sur `dossier/wind.lock`. `Ok(None)` : une
/// autre instance le tient déjà. Le dossier est créé s'il manque
/// (premier lancement : `db_path` le créerait de toute façon).
pub(crate) fn verrouiller(dossier: &Path) -> io::Result<Option<GardeInstance>> {
    std::fs::create_dir_all(dossier)?;
    let fichier = File::options()
        .create(true)
        .write(true)
        .truncate(false)
        .open(dossier.join(NOM_VERROU))?;
    if fichier.try_lock_exclusive()? {
        Ok(Some(GardeInstance { _fichier: fichier }))
    } else {
        Ok(None)
    }
}

/// Le dossier de la base SANS `AppHandle` — le verrou se prend avant que
/// Tauri ne construise quoi que ce soit (la fenêtre naît avant `setup`,
/// tauri `app.rs` : une seconde instance qui vérifierait dans `setup`
/// ferait clignoter une fenêtre). Même règle que `commands::db_path` :
/// `WIND_DB_PATH` d'abord, sinon `%APPDATA%\<identifiant>` — ce que
/// `app_data_dir()` rend sur Windows.
pub(crate) fn dossier_de_la_base() -> Option<PathBuf> {
    if let Ok(chemin) = std::env::var("WIND_DB_PATH") {
        return PathBuf::from(chemin).parent().map(Path::to_path_buf);
    }
    let appdata = std::env::var("APPDATA").ok()?;
    Some(PathBuf::from(appdata).join(crate::demenagement::IDENTIFIANT))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dossier_temporaire(nom: &str) -> std::path::PathBuf {
        let dossier =
            std::env::temp_dir().join(format!("wind-instance-{nom}-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        dossier
    }

    /// Le cœur de la garde : deux prises sur le même dossier, la seconde
    /// est refusée tant que la première vit. Sur Windows, LockFileEx en
    /// exclusif se refuse PAR HANDLE — le test tient dans un processus.
    #[test]
    fn deux_verrous_sur_le_meme_dossier_le_second_est_refuse() {
        let dossier = dossier_temporaire("double");
        let premiere = verrouiller(&dossier).unwrap();
        assert!(
            premiere.is_some(),
            "la première instance doit obtenir le verrou"
        );
        let seconde = verrouiller(&dossier).unwrap();
        assert!(seconde.is_none(), "la seconde instance doit être refusée");
        drop(premiere);
        let _ = std::fs::remove_dir_all(&dossier);
    }

    /// Relâcher la garde (fin de la première instance) rend le verrou
    /// disponible : pas de verrou collant.
    #[test]
    fn un_verrou_relache_se_reprend() {
        let dossier = dossier_temporaire("relache");
        let premiere = verrouiller(&dossier).unwrap();
        assert!(premiere.is_some());
        drop(premiere);
        let suivante = verrouiller(&dossier).unwrap();
        assert!(
            suivante.is_some(),
            "après la fin de la première, la suivante obtient le verrou"
        );
        drop(suivante);
        let _ = std::fs::remove_dir_all(&dossier);
    }

    /// Le dossier de la base peut ne pas exister encore (premier
    /// lancement) : la garde le crée, comme `db_path` le fait.
    #[test]
    fn le_dossier_absent_est_cree() {
        let dossier = dossier_temporaire("absent").join("sous-dossier");
        assert!(!dossier.exists());
        let garde = verrouiller(&dossier).unwrap();
        assert!(garde.is_some());
        assert!(dossier.join(NOM_VERROU).is_file());
        drop(garde);
        let _ = std::fs::remove_dir_all(dossier.parent().unwrap());
    }
}
