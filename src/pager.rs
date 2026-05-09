use color_eyre::Result;
use std::process::{Command, Stdio};

pub fn open(content: &str) -> Result<()> {
    let pager = std::env::var("PAGER").unwrap_or_else(|_| "less".to_string());

    // Ensure less interprets ANSI color codes (-R), preserving user LESS settings.
    let less_env = std::env::var("LESS").unwrap_or_default();
    let less_env = if less_env.contains("-R") {
        less_env
    } else {
        format!("-R {}", less_env).trim().to_string()
    };

    let mut child = Command::new(&pager)
        .env("LESS", less_env)
        .stdin(Stdio::piped())
        .spawn()?;

    use std::io::Write;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(content.as_bytes())?;
    }

    child.wait()?;
    Ok(())
}
