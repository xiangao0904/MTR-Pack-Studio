# MTR Pack Studio

A desktop editor for MTR 4 resource packs. The first milestone provides an English home screen and local project management. Train editing and 3D preview are planned next.

## Stack

- Vue 3, TypeScript, Vite
- Tauri 2 and Rust
- Local project folders with `mtrpack.project.json`

## Development

```sh
pnpm install
pnpm tauri dev
```

Use `pnpm dev` for a browser preview of the interface. Browser preview uses local storage and does not create real project folders. The Tauri app creates projects on disk and tracks recently opened folders in app configuration data.

```sh
pnpm build
cargo check --manifest-path src-tauri/Cargo.toml
```

## Project format

Each project is a directory containing `mtrpack.project.json` and an `assets/` directory. The manifest currently includes `schemaVersion`, `name`, `target` (`mtr4`) and an empty `trains` array. The schema will grow alongside the train editor.

The UI uses `src/i18n/index.ts` for copy. Only English is included in this release.
