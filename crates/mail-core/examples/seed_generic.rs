//! Adds nonsecret generic settings to an isolated UI fixture; never writes a vault.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("expected fixture database path")?;
    let email = args.next().ok_or("expected fixture email")?;
    let store = mail_core::Store::open(std::path::Path::new(&path))?;
    store.create_generic_account(
        &email,
        "custom-login",
        "imap.example.fr",
        993,
        "smtp.example.fr",
        465,
    )?;
    Ok(())
}
