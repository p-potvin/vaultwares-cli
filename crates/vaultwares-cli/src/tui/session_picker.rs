use std::io::{self, stdout, Write};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    style::{self, Color, Stylize},
    terminal::{self, ClearType},
};
use crate::session_mgr::ManagedSessionSummary;

pub(crate) fn pick_session(sessions: &[ManagedSessionSummary]) -> io::Result<Option<String>> {
    if sessions.is_empty() {
        println!("No sessions found.");
        return Ok(None);
    }

    let mut selected_index = 0;
    let mut filter = String::new();
    let mut stdout = stdout();

    terminal::enable_raw_mode()?;

    let result = loop {
        let filtered_sessions: Vec<_> = sessions
            .iter()
            .filter(|s| s.id.contains(&filter) || s.branch_name.as_deref().unwrap_or("").contains(&filter))
            .collect();

        if selected_index >= filtered_sessions.len() && !filtered_sessions.is_empty() {
            selected_index = filtered_sessions.len() - 1;
        }

        let (width, height) = terminal::size()?;
        let box_width = (width as usize).min(80);
        let box_height = (height as usize).min(20);
        
        execute!(
            stdout,
            cursor::Hide,
            cursor::MoveTo(0, 0),
            terminal::Clear(ClearType::All)
        )?;

        // Draw header
        let title = " 📂 VaultWares Session Picker ";
        let padding = box_width.saturating_sub(title.len() + 2) / 2;
        writeln!(stdout, "{}", "━".repeat(box_width).with(Color::Cyan))?;
        writeln!(stdout, "{}{}{}", "━".repeat(padding).with(Color::Cyan), title.bold().cyan(), "━".repeat(box_width - padding - title.len()).with(Color::Cyan))?;
        
        writeln!(stdout, "  Search: {}█", filter.clone().with(Color::White))?;
        writeln!(stdout, "{}", "─".repeat(box_width).with(Color::DarkGrey))?;

        // Draw list
        for (i, session) in filtered_sessions.iter().enumerate().take(box_height - 6) {
            let prefix = if i == selected_index { " ▶ " } else { "   " };
            let branch = session.branch_name.as_deref().unwrap_or("-");
            let id_display = format!("{:<20}", session.id);
            let msgs_display = format!("{:>3} msgs", session.message_count);
            let branch_display = format!(" branch: {:<15}", branch);
            
            let line = format!("{}{}{}{}", prefix, id_display.bold(), msgs_display.dim(), branch_display.italic().dark_grey());
            
            if i == selected_index {
                writeln!(stdout, "{}", line.with(Color::Black).on(Color::Cyan))?;
            } else {
                writeln!(stdout, "{}", line)?;
            }
        }

        // Draw footer
        cursor::MoveTo(0, (box_height - 1) as u16);
        let help = " [↑/↓] Navigate  [Enter] Select  [Esc] Cancel  [Type] Filter ";
        writeln!(stdout, "{}", help.with(Color::DarkGrey).italic())?;

        stdout.flush()?;

        if let Event::Key(KeyEvent { code, .. }) = event::read()? {
            match code {
                KeyCode::Char(c) => {
                    filter.push(c);
                    selected_index = 0;
                }
                KeyCode::Backspace => {
                    filter.pop();
                    selected_index = 0;
                }
                KeyCode::Up => {
                    if selected_index > 0 {
                        selected_index -= 1;
                    } else if !filtered_sessions.is_empty() {
                        selected_index = filtered_sessions.len() - 1;
                    }
                }
                KeyCode::Down => {
                    if !filtered_sessions.is_empty() {
                        selected_index = (selected_index + 1) % filtered_sessions.len();
                    }
                }
                KeyCode::Enter => {
                    if let Some(session) = filtered_sessions.get(selected_index) {
                        break Ok(Some(session.id.clone()));
                    }
                }
                KeyCode::Esc => {
                    break Ok(None);
                }
                _ => {}
            }
        }
    };

    terminal::disable_raw_mode()?;
    execute!(stdout, cursor::Show)?;
    execute!(stdout, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))?;

    result
}
