# Filebrowser

A desktop file browser application built with [Angular](https://angular.dev/) and [Tauri 2](https://v2.tauri.app/).

## Prerequisites

- [Node.js](https://nodejs.org/) (v22+)
- [Rust](https://www.rust-lang.org/tools/install)
- Linux: WebKitGTK and related dev libraries (on Arch: `sudo pacman -S webkit2gtk-4.1`)

## Getting Started

1. Install dependencies:
   ```bash
   npm install
   ```

2. Run in development mode:
   ```bash
   npm run tauri dev
   ```
   The first run will take a few minutes to compile the Rust backend. Subsequent runs are fast.

   To open a folder automatically on startup:
   ```bash
   npm run tauri dev -- -- /path/to/folder
   ```

3. Build for production:
   ```bash
   npm run tauri build
   ```

## Development

### Angular CLI

- Generate a component: `npx ng generate component component-name`
- Run unit tests: `npm test`
- For a complete list of schematics: `npx ng generate --help`

### Project Structure

- `src/` — Angular frontend
- `src-tauri/` — Tauri/Rust backend
- `src-tauri/tauri.conf.json` — Tauri configuration

## Additional Resources

- [Angular CLI Overview](https://angular.dev/tools/cli)
- [Tauri 2 Documentation](https://v2.tauri.app/)
