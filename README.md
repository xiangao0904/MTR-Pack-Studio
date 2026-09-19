# MTR Pack Studio

A desktop editor for MTR 4 resource packs. Each editable project is stored in one portable `.mtrpack` file. The current milestone provides an English home screen, local project management, a project workspace, and train entries. Model and texture editing, 3D preview, and pack export are planned next.

## Stack

- Vue 3, TypeScript, Vite
- Tauri 2 and Rust
- Transactional `.mtrpack` project files with per-resource compression

## Development

```sh
pnpm install
pnpm tauri dev
```

Use `pnpm dev` for a browser preview of the interface. Browser preview uses local storage and does not create real project files. The Tauri app creates `.mtrpack` files on disk and tracks recently opened files in app configuration data.

```sh
pnpm build
cargo check --manifest-path src-tauri/Cargo.toml
```

## Project format

Each project is a custom binary `.mtrpack` container with a fixed header, two recovery checkpoints, independently compressed objects, and a MessagePack content index. BLAKE3 content hashes provide integrity checks and resource deduplication. Updates append changed objects and a new index, then switch the active checkpoint. See [`docs/mtrpack-format-v1.md`](docs/mtrpack-format-v1.md) for the version 1 layout.

The UI uses `src/i18n/index.ts` for copy. Only English is included in this release.
