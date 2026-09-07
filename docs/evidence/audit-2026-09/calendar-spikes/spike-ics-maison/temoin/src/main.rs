//! Binaire témoin : mêmes usages « de base » de la stdlib que le harnais
//! (lecture de fichier, formatage, horloge) mais AUCUNE dépendance.
//! Sert de référence pour mesurer le poids de chrono + chrono-tz.
use std::time::Instant;

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    let content = std::fs::read_to_string(&arg).unwrap_or_default();
    let t0 = Instant::now();
    let n = content.replace("\r\n", "\n").split('\n').count();
    println!("{} lignes en {:?}", n, t0.elapsed());
}
