# Spike — option B : parseur iCalendar minimal maison + chrono-tz

Spike jetable (STANDARD §2.2-2.3). Départage contre l'option A (crate
`calcard`). Le corpus de référence vit dans le scratchpad de session
(`fixtures-ics/`, six .ics + ATTENDU.md) — il n'est pas copié ici.

## Contenu

- `src/parser.rs` — dépliage RFC 5545 §3.1, propriétés + paramètres
  (guillemets), déséchappement, extraction iTIP, TZID → UTC via
  chrono-tz + table Windows→IANA embarquée (24 entrées), repli heure
  flottante + drapeau `tz_non_resolue` si TZID hors table.
- `src/reply.rs` — génération METHOD:REPLY, pliage 75 octets, CRLF.
- `src/main.rs` — harnais : PASS/FAIL par fixture × champ, épreuve
  REPLY (re-parse), repli TZID exotique, chrono de parsing.
- `temoin/` — binaire sans dépendance (référence de taille).
- `temoin-chrono/` — binaire avec chrono seul (isole le poids de
  chrono-tz).

## Protocole (rejouer)

Machine de mesure : Snapdragon X X1E80100 (Windows 11 arm64),
rustc 1.97.1, profil release par défaut (pas de strip/LTO).

```powershell
cargo build --release
.\target\release\spike-ics-maison.exe <dossier-fixtures>   # sortie = toutes les mesures

# tailles
cd temoin; cargo build --release; cd ..
cd temoin-chrono; cargo build --release; cd ..

# variante chrono-tz filtrée (Europe + UTC seulement)
$env:CHRONO_TZ_TIMEZONE_FILTER = '(Europe/.*|Etc/UTC|UTC)'
cargo build --release --features chrono-tz/filter-by-regex --target-dir target-filtre
```

Les chiffres du 2026-08-22 sont dans le rapport du spike (transcript de
session) ; le harnais rejoue tout et sort code 0 si 0 FAIL.
