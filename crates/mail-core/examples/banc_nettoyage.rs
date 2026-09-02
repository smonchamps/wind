//! Banc du Nettoyage de printemps et du Portier (PLAN-AUDIT-V2 E4) : que
//! coûtent les lectures non bornées de l'audit — groupes, courrier d'un
//! groupe, attente du Portier, pile, routages — et le verdict sur le plus
//! gros groupe, sur une base donnée ? Durées et décomptes seuls : aucun
//! sujet, aucun expéditeur imprimé.
//!
//! ⚠️ MUTE la base : mode organisé posé, session de nettoyage ouverte
//! puis close, le plus gros groupe ARCHIVÉ. À jouer sur un décor, jamais
//! sur une vraie base.
//!
//! ```powershell
//! cargo run -p mail-core --example banc_nettoyage --release -- <chemin.db>
//! ```

use std::time::Instant;

use mail_core::Store;

fn chrono<T>(
    etiquette: &str,
    f: impl FnOnce() -> Result<T, mail_core::Error>,
) -> Result<T, mail_core::Error> {
    let depart = Instant::now();
    let valeur = f()?;
    println!(
        "{etiquette:<26} {:>9.2} ms",
        depart.elapsed().as_secs_f64() * 1000.0
    );
    Ok(valeur)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage : banc_nettoyage <chemin.db>")?;
    let mut store = Store::open(std::path::Path::new(&path))?;
    let now = chrono::Utc::now().timestamp();
    store.set_mode_organise(true, 0)?;
    // L'attente du Portier se remplit à l'ARRIVÉE (upsert sous mode
    // organisé) ; sur un décor déjà semé on la peuple à la main : tout
    // expéditeur en attente — le pire cas de la lecture mesurée.
    {
        let conn = rusqlite::Connection::open(&path)?;
        conn.execute_batch(
            "INSERT OR IGNORE INTO portier_attente (address)
             SELECT DISTINCT sender_norm FROM envelopes WHERE sender_norm IS NOT NULL",
        )?;
    }

    let session = chrono("nettoyage_demarrer", || {
        store.nettoyage_demarrer("tout", "dossiersArchives", now)
    })?;
    println!("  {} groupes annoncés", session.total);
    let groupes = chrono("nettoyage_groupes", || store.nettoyage_groupes())?;
    println!("  {} groupes rendus", groupes.len());
    let gros = groupes
        .iter()
        .max_by_key(|groupe| groupe.messages)
        .ok_or("aucun groupe : la base est vide ?")?
        .clone();
    let messages = chrono("nettoyage_messages (gros)", || {
        store.nettoyage_messages(&gros.address)
    })?;
    println!("  {} messages dans le plus gros groupe", messages.len());
    let attente = chrono("portier_attente", || store.portier_attente())?;
    println!("  {} en attente", attente.len());
    let pile = chrono("pile_mis_de_cote", || store.pile_mis_de_cote())?;
    println!("  {} mis de côté", pile.len());
    let routages = chrono("routages", || store.routages())?;
    println!("  {} routages", routages.len());
    let traites = chrono("nettoyage_verdict (gros)", || {
        store.nettoyage_verdict(&gros.address, "ecarte", Some("archive"), now)
    })?;
    println!("  {traites} messages archivés par le verdict");
    store.nettoyage_terminer()?;
    Ok(())
}
