# Blockbench train models

The desktop model picker accepts `.bbmodel` alongside OBJ, FBX and MQO. The
Asset Library can import and reuse these models in train body and bogie layers.
Imports retain the original JSON, texture dependencies and canonical preview
geometry in the portable project file.

This importer follows MTR's native cuboid loader: 16 Blockbench units equal one
metre, box UVs use the model resolution and `uv_offset`, and top-level groups
become selectable parts. Element origins, rotations, rounded dimensions,
inflation and mirrored UVs are retained. Blockbench 5's separate group metadata
is resolved with its outliner. Embedded base64 images, relative texture paths,
adjacent textures and manually located dependencies are supported.

MTR uses the first texture for all native cuboids and ignores Blockbench timeline
animations and custom per-face UV layouts. Imports report those conditions.
Mesh elements are rejected with guidance to export OBJ from Blockbench.
Preview coordinates follow the native loader, including its ignored root group
rotations. Original models are never implicitly rescaled to train dimensions.

## Export choice

When a project's trains use a BBMODEL asset, **Export Pack** shows
**BBMODEL train models**:

- **Convert to OBJ:** imported BBMODEL train layers are converted to OBJ with
  their current material bindings, visibility and transforms. Other model
  formats follow the existing MTR 4 OBJ/MQO choice.
- **Keep BBMODEL:** native cuboids, origins, element rotations and named groups
  are retained, with model/texture/property references generated for MTR.
  Original file paths and embedded texture payloads are removed from the output;
  textures are exported separately as PNG. A layer's V flip is baked into the
  texture because the native Blockbench loader does not read the OBJ V-flip flag.

**MTR 4** supports native BBMODEL layers alongside OBJ/MQO layers. Exported
top-level group names exactly match the generated model properties. Carriage
body and independent bogie slots remain separate.

**MTR 3 + NTE** requires every exported layer in a native train to use BBMODEL.
Its paired multi-model syntax is reserved for OBJ. Native export therefore
combines the carriages, body layers and bogies into one BBMODEL and assembles
their textures into one atlas. Stable group names and per-part placement filters
retain carriage rules. Bogie offsets are baked into cuboid positions. The atlas
is limited to 8192 pixels per dimension; each cuboid's box UVs must fit within
its source resolution so it cannot sample a neighboring atlas tile. Repeating
UVs and mixed-format trains must use OBJ. Other OBJ-only trains in the project
keep their normal export path.

Native export reports an error for edited layer/part transforms it cannot
preserve; OBJ bakes those transforms. MTR 3 also reports an error for nested
group rotations, which its native loader ignores. These errors identify the
train, carriage and model layer before writing the destination.

## Upstream evidence

The implementation uses the same revisions as the existing export contract:

- [MTR 4 BlockbenchElement](https://github.com/Minecraft-Transit-Railway/Minecraft-Transit-Railway/blob/c7199b24917f6d26e13216769725180ca06a69b5/fabric/src/main/java/org/mtr/mod/resource/BlockbenchElement.java)
  defines cube units, box UVs, element transforms and mirroring.
- [MTR 4 BlockbenchModel](https://github.com/Minecraft-Transit-Railway/Minecraft-Transit-Railway/blob/c7199b24917f6d26e13216769725180ca06a69b5/fabric/src/main/java/org/mtr/mod/resource/BlockbenchModel.java)
  resolves Blockbench 5 group metadata.
- [MTR 4 DynamicVehicleModel](https://github.com/Minecraft-Transit-Railway/Minecraft-Transit-Railway/blob/c7199b24917f6d26e13216769725180ca06a69b5/fabric/src/main/java/org/mtr/mod/render/DynamicVehicleModel.java)
  binds top-level groups to property names.
- [MTR 3 DynamicTrainModel](https://github.com/Minecraft-Transit-Railway/Minecraft-Transit-Railway/blob/e9ebfc26c531c31d7649868a71e8509747c44807/common/src/main/java/mtr/client/DynamicTrainModel.java)
  loads one native model, with box UVs and placement filters.
- [NTE DynamicTrainModelLoader](https://github.com/zbx1425/mtr-nte/blob/c5d1443f79b4e19135c03a00fe28217f9c477ea0/common/src/main/java/cn/zbx1425/mtrsteamloco/render/integration/DynamicTrainModelLoader.java)
  provides the separate OBJ model path.

`src-tauri/tests/fixtures/mtr4-4.0.5/bogie_1.bbmodel` and `bogie_1.png` are
unmodified upstream fixtures from the pinned MTR 4 revision, covered by that
directory's MIT license. The source contains one half of a bogie; duplicating or
mirroring it is an authoring choice and is not inferred by the importer.

## Verification and review artifacts

Run `cargo test --locked --manifest-path src-tauri/Cargo.toml`. Tests check
one-metre dimensions, axes, asymmetric box UVs, element rotations, actual MTR
geometry, embedded/renamed textures, Blockbench 5 groups, invalid meshes,
native/OBJ export choices, resolved MTR 4 resources, native MTR 3 atlas pixels,
bogie offsets, placement names, deterministic archives and mixed-format errors.
An integration test also compacts and reopens a project with both a BBMODEL train
and eye candy and exports all four target/format combinations.

Generated artifacts are under `src-tauri/target/acceptance/`:

- `BBMODEL and Eye Candy Acceptance.mtrpack`
- `bbmodel-mtr4-bbmodel.zip` and `bbmodel-mtr4-obj.zip`
- `bbmodel-mtr3_nte-bbmodel.zip` and `bbmodel-mtr3_nte-obj.zip`

Copy the sample project before making edits that must survive a test rerun.
Tests verify authoring, persistence and resource contracts. Visual acceptance in
a running Minecraft instance remains pending.
