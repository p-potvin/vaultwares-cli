use std::env;
use std::io::{self, IsTerminal, Write};
use std::process::{Command, Stdio};

pub(crate) fn output_needs_pager(text: &str, terminal_height: u16) -> bool {
    let visible_rows = usize::from(terminal_height.saturating_sub(2).max(1));
    text.lines().count() > visible_rows
}

pub(crate) fn print_report_or_page(text: &str) -> io::Result<()> {
    let mut stdout = io::stdout();
    if !stdout.is_terminal() {
        return print_plain(text, &mut stdout);
    }

    let terminal_height = crossterm::terminal::size()
        .map(|(_, height)| height)
        .unwrap_or(24);
    if !output_needs_pager(text, terminal_height) {
        return print_plain(text, &mut stdout);
    }

    if try_external_pager(text).is_ok() {
        return Ok(());
    }

    print_plain(text, &mut stdout)
}

fn print_plain(text: &str, out: &mut impl Write) -> io::Result<()> {
    write!(out, "{text}")?;
    if !text.ends_with('\n') {
        writeln!(out)?;
    }
    out.flush()
}

fn try_external_pager(text: &str) -> io::Result<()> {
    let pager = env::var("PAGER")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let mut command = match pager {
        Some(value) => pager_command(&value),
        None => default_pager_command(),
    };

    let mut child = command.stdin(Stdio::piped()).spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes())?;
        if !text.ends_with('\n') {
            stdin.write_all(b"\n")?;
        }
    }
    drop(child.stdin.take());
    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("pager exited unsuccessfully"))
    }
}

fn pager_command(value: &str) -> Command {
    let mut parts = value.split_whitespace();
    let executable = parts.next().unwrap_or(value);
    let mut command = Command::new(executable);
    command.args(parts);
    command
}

#[cfg(windows)]
fn default_pager_command() -> Command {
    Command::new("more.com")
}

#[cfg(not(windows))]
fn default_pager_command() -> Command {
    let mut command = Command::new("less");
    command.arg("-R");
    command
}

#[cfg(test)]
mod tests {
    use super::output_needs_pager;

    #[test]
    fn output_needs_pager_when_lines_exceed_terminal_height() {
        let text = (0..30)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(output_needs_pager(&text, 10));
        assert!(!output_needs_pager("short\nreport", 10));
    }
}
