use color_eyre;
use std::io::Write;

pub fn show_in_pager(content: &str) -> color_eyre::Result<()> {
    let pager = std::env::var("PAGER").unwrap_or_else(|_| "less".to_string());
    let mut child = std::process::Command::new(&pager)
        .stdin(std::process::Stdio::piped())
        .spawn()?;

    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(content.as_bytes())?;
    }

    child.wait()?;
    Ok(())
}
