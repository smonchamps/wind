//! release-verify -- the cryptographic half of STANDARD §2.10, done by the
//! workspace itself (audit lot 4, E12c / B31).
//!
//! Tauri writes a `.sig` asset and the manifest's `signature` field as the
//! **base64 of the minisign signature text**, and `tauri.conf.json` carries
//! the public key in the same wrapper. `verify-release.ps1` used to hand the
//! raw wrapper to `minisign -x` (which cannot parse it) and, when the tool
//! was absent, printed "NOT PROVEN" without failing. This binary decodes
//! both wrappers and verifies with `minisign-verify`, the crate the shipped
//! updater uses -- a PASS here is the check the installed app performs.
//!
//!   release-verify verify --pubkey <base64> --signature <file.sig> --artifact <file>
//!
//! Exit 0 on a valid signature; 2 on any refusal (bad wrapper, wrong key,
//! tampered bytes, missing file), with the reason on stderr. Nothing here
//! is "not proven": an absent proof is a failure.

use base64::Engine;
use std::path::Path;

fn decode_wrapper(label: &str, wrapped: &str) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(wrapped.trim())
        .map_err(|err| format!("{label}: not the Tauri base64 wrapper ({err})"))?;
    String::from_utf8(bytes).map_err(|_| format!("{label}: wrapper does not decode to text"))
}

/// The decision: the artifact bytes carry a valid minisign signature from
/// the given public key. Pure -- no I/O -- so the refusals are testable.
pub fn verify(
    pubkey_wrapped: &str,
    signature_wrapped: &str,
    artifact: &[u8],
) -> Result<(), String> {
    let pubkey_text = decode_wrapper("public key", pubkey_wrapped)?;
    let signature_text = decode_wrapper("signature", signature_wrapped)?;
    let public_key = minisign_verify::PublicKey::decode(&pubkey_text)
        .map_err(|err| format!("public key: not a minisign key ({err})"))?;
    let signature = minisign_verify::Signature::decode(&signature_text)
        .map_err(|err| format!("signature: not a minisign signature ({err})"))?;
    public_key
        .verify(artifact, &signature, false)
        .map_err(|err| format!("signature does not match the artifact or the key ({err})"))
}

fn read(label: &str, path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|err| format!("{label} {}: {err}", path.display()))
}

fn arg<'a>(args: &'a [String], name: &str) -> Result<&'a str, String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
        .ok_or_else(|| format!("missing {name} <value>"))
}

fn run(args: &[String]) -> Result<String, String> {
    match args.first().map(String::as_str) {
        Some("verify") => {
            let pubkey = arg(args, "--pubkey")?;
            let signature_path = Path::new(arg(args, "--signature")?);
            let artifact_path = Path::new(arg(args, "--artifact")?);
            let signature = read("signature", signature_path)?;
            let signature = String::from_utf8(signature)
                .map_err(|_| "signature: the .sig file is not text".to_string())?;
            let artifact = read("artifact", artifact_path)?;
            verify(pubkey, &signature, &artifact)?;
            Ok(format!(
                "VALID  {} ({} bytes)",
                artifact_path.display(),
                artifact.len()
            ))
        }
        _ => Err(
            "usage: release-verify verify --pubkey <base64> --signature <file.sig> --artifact <file>"
                .into(),
        ),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(line) => println!("{line}"),
        Err(reason) => {
            eprintln!("REFUSED  {reason}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARTIFACT: &[u8] = include_bytes!("../fixtures/artifact.bin");
    const SIGNATURE: &str = include_str!("../fixtures/artifact.bin.sig");
    const SIGNER: &str = include_str!("../fixtures/signer.pub");
    const OTHER: &str = include_str!("../fixtures/other.pub");

    #[test]
    fn the_tauri_wrapper_is_decoded_and_the_fixture_verifies() {
        verify(SIGNER, SIGNATURE, ARTIFACT).unwrap();
    }

    #[test]
    fn the_raw_wrapper_handed_to_a_minisign_parser_is_refused_not_proven() {
        // The old script's mistake, made explicit: the base64 text is not a
        // minisign signature.
        assert!(minisign_verify::Signature::decode(SIGNATURE).is_err());
    }

    #[test]
    fn a_tampered_artifact_is_refused() {
        let mut tampered = ARTIFACT.to_vec();
        tampered[0] ^= 0x01;
        let reason = verify(SIGNER, SIGNATURE, &tampered).unwrap_err();
        assert!(reason.contains("does not match"), "{reason}");
    }

    #[test]
    fn a_signature_from_another_key_is_refused() {
        let reason = verify(OTHER, SIGNATURE, ARTIFACT).unwrap_err();
        assert!(reason.contains("does not match"), "{reason}");
    }

    #[test]
    fn a_broken_wrapper_or_a_missing_file_is_a_refusal_never_a_pass() {
        assert!(verify(SIGNER, "not base64!!", ARTIFACT).is_err());
        assert!(verify("not base64!!", SIGNATURE, ARTIFACT).is_err());
        assert!(verify(SIGNER, "", ARTIFACT).is_err());
        let missing = run(&[
            "verify".into(),
            "--pubkey".into(),
            SIGNER.into(),
            "--signature".into(),
            "fixtures/absent.sig".into(),
            "--artifact".into(),
            "fixtures/artifact.bin".into(),
        ]);
        assert!(missing.is_err());
        assert!(
            run(&["verify".into()]).is_err(),
            "no silent pass without arguments"
        );
    }
}
