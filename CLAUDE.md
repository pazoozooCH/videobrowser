# Filebrowser — Angular 21 + Tauri 2

## Project Structure

- `src/` — Angular frontend (standalone components, signals)
- `src-tauri/` — Rust backend (Tauri commands for all FS operations)
- Python reference source: `~/prog/private/python-ui/filebrowser/`

## Architecture

Three layers:
1. **Rust backend** (Tauri commands): All filesystem I/O and `.dat_` encoding logic
2. **Angular services**: State management (signals), Tauri command invocation
3. **Angular components**: Tree view, context menu, dialogs

Custom Rust commands for all FS operations (not Tauri's fs plugin) — the `.dat_` encoding logic is domain-specific.

## Conventions

- **Dependency injection**: Always use `inject()`, never constructor injection
- **Angular**: Standalone components, signals for state, no NgModules
- **Rust**: `serde(rename_all = "camelCase")` on all structs sent to frontend
- **Encoding**: `.dat_` prefix + base64 encoded original name (see `src-tauri/src/encoding/`)

## Workflow
- Always commit automatically after completing a phase/task — don't ask

## Commands

- `npm run tauri dev` — Run the app in development mode
- `npm run build` — Build Angular frontend
- `cargo check` (in `src-tauri/`) — Check Rust backend compiles
