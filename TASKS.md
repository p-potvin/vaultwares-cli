# Project Roadmap: Multi-Agent & Security Enhancement

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
  1a [x] Relocate `LiveCli` struct and impl from `main.rs` to `app.rs`
  1b [x] Update module boundaries and imports
2 [x] Extract `parse_args` to `args.rs`
  2a [x] Move `parse_args` logic and `CliAction` enum to `args.rs`
  2b [x] Cleanup any argument parsing helpers from `main.rs`

## 2 TUI Components & HUD

3 [x] Terminal-size-aware Status Line (HUD)
  3a [x] Implement status bar formatter with model name and token counts
  3b [x] Use `crossterm::terminal::size()` to bind to bottom row
  3c [x] Wire live token counter to streaming events
4 [x] Interactive Session Picker
  4a [x] Implement fuzzy-filterable UI for session selection
  4b [x] Replace text-based `/session list` with interactive list

## 3 Themes & Polish

5 [~] Color Theme Configuration
  5a [x] Define theme traits (space, neon, catppuccin)
  5b [ ] Wire `Config` tool's theme setting to TUI rendering
  5c [ ] Add subtle micro-animations/transitions for HUD updates
  5d [ ] Implement contrast checking for all theme variants
  5e [ ] Sync theme tokens with `vault-themes` submodule rules

## 4 Post-Quantum Security & Privacy (Enterprise)

6 [x] Integrate Post-Quantum Key Encapsulation (ML-KEM)
  6a [x] Add `fips203` or `pqcrypto-kyber` to `Cargo.toml`
  6b [x] Implement ML-KEM-768 key generation and storage logic
  6c [x] Implement encrypted session persistence (Zero-Knowledge)
7 [x] Homomorphic Encryption (HE) Integration
  7a [x] Create `crates/vaultwares-fhe` crate and add `tfhe`
  7b [x] Implement a proof-of-concept for encrypted token summation
8 [~] Hardware Security & Keystore Integration
  8a [ ] Implement secure seed storage for Windows (DPAPI)
  8b [ ] Implement secure seed storage for Linux (Secret Service API)
  8c [ ] Implement secure seed storage for macOS (Keychain)
  8d [ ] Add fallback to encrypted local keystore if hardware is unavailable
  8e [ ] Write unit tests for all keystore interfaces

## 5 Security Audit & Penetration Testing

9 [~] Comprehensive Security Audit
  9a [ ] Perform static analysis on `vaultwares-cli` and `vaultwares-agentciation`
  9b [ ] Audit PQC integration against known vulnerability vectors
  9c [ ] Review `vaultwares-fhe` implementation for side-channel resistance
  9d [ ] Penetration test the Redis pub/sub mechanism in Multi-Agent workflows
  9e [ ] Check for hardcoded credentials and token leakage
  9f [ ] Ensure all local API communication enforces TLS 1.3
  9g [ ] Document security findings and mitigate critical issues

## 6 Code Quality & Architecture Hygiene

10 [~] Codebase Standardization
  10a [ ] Resolve all `ambiguous_glob_imports` in `crates/runtime`
  10b [ ] Resolve all `ambiguous_glob_imports` in `crates/plugins`
  10c [ ] Clean up unused variables and dead code across the monorepo
  10d [ ] Implement strict linting rules and run `cargo clippy --fix`
  10e [ ] Add unit tests for `session_picker.rs` and `status_bar.rs`

## 7 Multi-Agent Infrastructure Scaling

11 [~] Multi-Agent Scalability & Remote Execution
  11a [ ] Implement robust error recovery for missed Redis heartbeats
  11b [ ] Enable remote execution of subagents across LAN
  11c [ ] Add multi-user authentication layer to the manager
  11d [ ] Implement role-based access control (RBAC) for specific tools
  11e [ ] Write end-to-end integration tests for `lonely_manager.py` and `assign_tasks.py`

## 8 GUI Fixes & Enhancements

12 [~] Gradio & UI Improvements
  12a [ ] Debug and resolve the issue where GUI .exe launches but browser UI doesn't appear
  12b [ ] Enhance error logging and visual feedback in Gradio interface
  12c [ ] Add live workflow visualization tool (node graph representation)
  12d [ ] Create advanced workflow editing capabilities directly in the UI
  12e [ ] Write UI automated tests using Playwright

## 9 Model Integrations & Workflows

13 [~] Expanded Modalities & Integrations
  13a [ ] Implement deep text model integration with specific prompt engineering tools
  13b [ ] Add diffusers integration for live image generation subagents
  13c [ ] Add video processing subagents via FFmpeg integration
  13d [ ] Create seamless workflow export/import for ComfyUI integration
  13e [ ] Build cross-modality testing suite

## 10 Final Documentation

14 [~] Release Preparation
  14a [ ] Write comprehensive user instructions for the Multi-Agent system
  14b [ ] Finalize roadmap documentation (ADRs and architecture docs)
  14c [ ] Update all `README.md` and `DOCUMENTATION.md` files
  14d [ ] Record a demo video scenario of the complete system
  14e [ ] Prepare v1.0.0 release notes
