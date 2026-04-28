# Project Roadmap: Claw TUI Enhancement

## System Rules

- Review the entire roadmap before starting any task to ensure alignment with project goals and dependencies.
- All tasks are assigned to a single agent.
- Tasks use numbers: `1 [ ] Task`
- Subtasks use letters: `1a [ ] Subtask`
- Status indicators: `[ ]` (Free), `[~]` (In Progress), `[x]` (Finished)
- Agents in `RELAXING` state will be assigned the next available main task.
- Agents in `WAITING_FOR_INPUT` are locked until PRs are merged by human intervention and are manually reset.

---

## 1 Structural Cleanup (Foundation)

1 [x] Extract `LiveCli` to `app.rs`
  1a [ ] Relocate `LiveCli` struct and impl from `main.rs` to `app.rs`
  1b [ ] Update module boundaries and imports
2 [~] Extract `parse_args` to `args.rs`
  2a [ ] Move `parse_args` logic and `CliAction` enum to `args.rs`
  2b [ ] Cleanup any argument parsing helpers from `main.rs`

## 2 TUI Components & HUD

3 [~] Terminal-size-aware Status Line (HUD)
  3a [ ] Implement status bar formatter with model name and token counts
  3b [ ] Use `crossterm::terminal::size()` to bind to bottom row
  3c [ ] Wire live token counter to streaming events
4 [ ] Interactive Session Picker
  4a [ ] Implement fuzzy-filterable UI for session selection
  4b [ ] Replace text-based `/session list` with interactive list

## 3 Themes & Polish

5 [ ] Color Theme Configuration
  5a [ ] Define theme traits (space, neon, catppuccin)
  5b [ ] Wire `Config` tool's theme setting to TUI rendering
