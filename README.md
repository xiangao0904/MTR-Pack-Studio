# MTR Pack Studio

A desktop editor for MTR resource packs. Each editable project is stored in one portable `.mtrpack` file. The current milestone includes local project management, train, rail and static eye candy editors, a shared model/texture library, 3D preview, and ZIP export for MTR 4 or MTR 3 + NTE. PIDS remains planned. The interface ships in English.

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

See [train workflows](docs/train-workflow-acceptance.md), [rail export](docs/rail-export-compatibility.md), and [eye candy workflows](docs/eye-candy-workflow.md) for editor behavior, export contracts and acceptance limits.

Train models can also be imported from Blockbench (`.bbmodel`). Projects using them offer native BBMODEL or OBJ conversion at export; see [BBMODEL compatibility](docs/bbmodel-compatibility.md).
