use color_eyre::Result;
use std::process::{Command, Stdio};

pub fn open(content: &str) -> Result<()> {
    let pager = std::env::var("PAGER").unwrap_or_else(|_| "less".to_string());
    let mut child = Command::new(&pager).stdin(Stdio::piped()).spawn()?;

    use std::io::Write;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(content.as_bytes())?;
    }

    child.wait()?;
    Ok(())
}
