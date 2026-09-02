//! Banc du Nettoyage de printemps et du Portier (PLAN-AUDIT-V2 E4) : que
//! coûtent les lectures non bornées de l'audit — groupes, courrier d'un
//! groupe, attente du Portier, pile, routings — et le verdict sur le plus
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
    store.set_organized_mode(true, 0)?;
    // L'attente du Portier se remplit à l'ARRIVÉE (upsert sous mode
    // organisé) ; sur un décor déjà semé on la peuple à la main : tout
    // expéditeur en attente — le pire cas de la lecture mesurée.
    {
        let conn = rusqlite::Connection::open(&path)?;
        conn.execute_batch(
            "INSERT OR IGNORE INTO screener_waiting (address)
             SELECT DISTINCT sender_norm FROM envelopes WHERE sender_norm IS NOT NULL",
        )?;
    }

    let session = chrono("cleanup_start", || {
        store.cleanup_start("tout", "dossiersArchives", now)
    })?;
    println!("  {} groupes annoncés", session.total);
    let groupes = chrono("cleanup_groups", || store.cleanup_groups())?;
    println!("  {} groupes rendus", groupes.len());
    let gros = groupes
        .iter()
        .max_by_key(|groupe| groupe.messages)
        .ok_or("aucun groupe : la base est vide ?")?
        .clone();
    let messages = chrono("cleanup_messages (gros)", || {
        store.cleanup_messages(&gros.address)
    })?;
    println!("  {} messages dans le plus gros groupe", messages.len());
    let attente = chrono("screener_waiting", || store.screener_waiting())?;
    println!("  {} en attente", attente.len());
    let pile = chrono("set_aside_pile", || store.set_aside_pile())?;
    println!("  {} mis de côté", pile.len());
    let routings = chrono("routings", || store.routings())?;
    println!("  {} routings", routings.len());
    let traites = chrono("cleanup_verdict (gros)", || {
        store.cleanup_verdict(&gros.address, "ecarte", Some("archive"), now)
    })?;
    println!("  {traites} messages archivés par le verdict");
    store.cleanup_finish()?;
    Ok(())
}
