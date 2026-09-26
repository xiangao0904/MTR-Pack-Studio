# Studio train acceptance fixture

> Updated 2026-09-27: built-in train models and textures were removed from the app.
> The desktop editor imports user-provided models. Any browser sample workflow
> below is historical; binary regression fixtures now live in `tests/fixtures/preview-glb/`.

Original, deliberately simple static metro geometry for testing MTR Pack Studio.
The geometry, generated textures and generator are dedicated to the public domain
under CC0 1.0. No third-party game models or textures are included.

## Desktop import

- Import `body.obj` as a body model. Keep `body.mtl`, `paint.png` and `checker.png`
  beside it. The body is 20 m long, 3 m wide and approximately 3 m high. Its
  floor starts 0.7 m above the ground to leave room for the bogies.
- Import `bogie.obj` into both bogie slots, with positions `7` and `-7` metres.
  Keep `bogie.mtl` beside it. Each bogie is centered at local Z = 0 and its wheel
  bottoms are near Y = 0. It measures 2.58 m across and 2.9 m long.
- Coordinates are +X left, +Y up, +Z forward. The front has a wider windscreen
  and two headlight panels; the rear has a smaller windscreen.
- Set the carriage length to `20`, width to `3`, and both coupling paddings to
  `0.25` to inspect spacing between repeated carriage instances.

`body.obj` has exactly two materials: `material-0` / `Paint` and `material-1` /
`Glass`. Both have independent image textures. The paint has an asymmetric blue
stripe near its lower edge so UV vertical flipping is visible. The glass uses a
subtle navy checker. Replace one texture binding and verify the other is retained.
`bogie.obj` uses two untextured materials so generated solid-color export textures
are also exercised.

There are 32 independently named body parts (384 triangles) and 7 bogie parts
(84 triangles). A part's stable ID is `part-` followed by its lowercase OBJ object
name, e.g. `part-front_windscreen`, `part-door_left_1`, `part-wheel_front_left`.
Select corresponding parts from the viewport and hierarchy. Applying a placement
override to one window or door provides a small, easy-to-inspect rules fixture.

## Browser fixture

The generator also creates these public files:

| File | Purpose |
| --- | --- |
| `/fixtures/studio-train.glb` | Body preview with embedded paint and glass PNGs |
| `/fixtures/studio-bogie.glb` | Bogie preview |
| `/fixtures/studio-train.json` | `AssetDefinition` + `materials` metadata; ID `fixture-studio-train` |
| `/fixtures/studio-bogie.json` | `AssetDefinition` + `materials` metadata; ID `fixture-studio-bogie` |
| `/fixtures/studio-train.document.json` | Canonical body `ModelDocument` |
| `/fixtures/studio-bogie.document.json` | Canonical bogie `ModelDocument` |
| `/fixtures/paint.png`, `/fixtures/checker.png` | Independent material texture fixtures |

GLB nodes carry `extras.partId` and GLB materials carry `extras.materialId`,
matching the native parser's IDs. Metadata hashes use BLAKE3 of the authored
source, normalized JSON bytes and GLB bytes. Native imports will produce their
own normalized document bytes and hashes.

## Regeneration and checks

Install Python 3 and the `blake3` package, then run:

```text
python src-tauri/tests/fixtures/studio-train/generate.py
```

The generator uses only the Python standard library plus BLAKE3. PNGs are
deterministically encoded RGBA files; GLBs use little-endian float and index
buffers, face normals, UV coordinates and four-byte chunk alignment.

Suggested acceptance workflow: import body and both bogies, change one texture,
move a bogie, select a door, save, reopen, build a repeated/reversed consist, and
export MTR 4 and MTR 3 + NTE. Inspect the generated archives and check the train
in the corresponding installed game versions before claiming in-game support.
