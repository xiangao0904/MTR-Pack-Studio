# MTR Pack Studio

A desktop editor for MTR 4 resource packs. The current milestone provides an English home screen, local project management, a project workspace, and train entries. Model and texture editing, 3D preview, and pack export are planned next.

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

Each project is a directory containing `mtrpack.project.json` and an `assets/` directory. New projects use schema version 2 with `name`, `target` (`mtr4`), `description`, and a typed `content` index. Train entries live in `content/trains/`. Version 1 projects still open; they are upgraded to version 2 when a train is created. The train document currently holds placeholders for model, texture, and properties until the editor is implemented.

The UI uses `src/i18n/index.ts` for copy. Only English is included in this release.
