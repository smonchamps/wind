//! Témoin chrono seul : parse une date et l'affiche.
use chrono::{NaiveDateTime, TimeZone, Utc};

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    let n = NaiveDateTime::parse_from_str(&arg, "%Y%m%dT%H%M%S")
        .unwrap_or_else(|_| Utc::now().naive_utc());
    println!("{}", Utc.from_utc_datetime(&n));
}
