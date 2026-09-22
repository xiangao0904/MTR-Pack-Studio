# Export compatibility contract

The exporters target MTR 4.0.5 on Minecraft 1.20.4 and MTR 3 with Nemo's Transit Expansion (NTE) on the Minecraft versions listed in the export dialog. Editor preview consists are not game train configurations and never affect exported resources.

## Upstream evidence

- MTR 4.0.5: [commit c7199b24917f6d26e13216769725180ca06a69b5](https://github.com/Minecraft-Transit-Railway/Minecraft-Transit-Railway/tree/c7199b24917f6d26e13216769725180ca06a69b5). The `gradle.properties` at this revision declares version 4.0.5 and Minecraft 1.20.4. Unmodified resource schemas used by the contract tests live in `src-tauri/tests/fixtures/mtr4-4.0.5/`, with the upstream MIT license.
- MTR 4 [ModelResourceLoader](https://github.com/Minecraft-Transit-Railway/Minecraft-Transit-Railway/blob/c7199b24917f6d26e13216769725180ca06a69b5/fabric/src/main/java/org/mtr/mod/resource/ModelResourceLoader.java) resolves `default.png` to the vehicle model's `textureResource`. Each exported material group therefore becomes a separate model layer with a neutral material and `default.png` binding.
- MTR 4 [ModelPropertiesPart](https://github.com/Minecraft-Transit-Railway/Minecraft-Transit-Railway/blob/c7199b24917f6d26e13216769725180ca06a69b5/fabric/src/main/java/org/mtr/mod/resource/ModelPropertiesPart.java) creates instances by iterating named position definitions. Every exported part references an explicit `origin` containing `(0, 0, 0)`.
- MTR 4 [MqoModelConverter](https://github.com/Minecraft-Transit-Railway/Minecraft-Transit-Railway/blob/c7199b24917f6d26e13216769725180ca06a69b5/fabric/src/main/java/org/mtr/mod/resource/MqoModelConverter.java) scales centimetres by 0.01, reverses polygon winding, and preserves UV pairs. The exporter applies the inverse conversion, including UVs in face vertex order.
- MTR 3 [DynamicTrainModel](https://github.com/Minecraft-Transit-Railway/Minecraft-Transit-Railway/blob/e9ebfc26c531c31d7649868a71e8509747c44807/common/src/main/java/mtr/client/DynamicTrainModel.java) evaluates periodic rules as `(oneBasedPosition + offset) % interval == 0`. A matching exact position has strength 3, a periodic match has strength 2, and no match has strength 0. A part is hidden only when the blacklist has greater strength. Presets compile to a whitelist and a `%1` blacklist so unmatched positions remain hidden. Custom expressions use native strength semantics; negative offsets are serialized as `%N+-K` because the native parser splits on `+`.
- NTE [DynamicTrainModelLoader](https://github.com/zbx1425/mtr-nte/blob/c5d1443f79b4e19135c03a00fe28217f9c477ea0/common/src/main/java/cn/zbx1425/mtrsteamloco/render/integration/DynamicTrainModelLoader.java) expects pairs of `model|whitelist;blacklist;attributes`. Model-specific part names are `filename.obj/group`. Every exported part uses one qualified stable name and a nonempty `positions: [[0, 0]]`, avoiding duplicate inherited/overridden geometry. Bogie offsets are baked into the static geometry for MTR 3.
- NTE [ObjModelLoader](https://github.com/zbx1425/mtr-nte/blob/c5d1443f79b4e19135c03a00fe28217f9c477ea0/common/src/main/java/cn/zbx1425/sowcerext/model/loader/ObjModelLoader.java) reads `map_Kd`, multiplies diffuse color, groups by OBJ material, and uses UV values directly. [ResourceUtil](https://github.com/zbx1425/mtr-nte/blob/c5d1443f79b4e19135c03a00fe28217f9c477ea0/common/src/main/java/cn/zbx1425/sowcerext/util/ResourceUtil.java) supports fully qualified texture resource identifiers.

## Coordinate, texture and placement contract

Canonical geometry uses metres with +X pointing left. OBJ and MQO export undo the X reflection and restore the appropriate face winding; MQO also converts metres to centimetres. Canonical UVs preserve imported values. Preview GLB converts V to `1 - V`. MTR 4 uses `flipTextureV` with the same baseline, and MTR 3 bakes that conversion in the emitted OBJ. The editor's per-layer V toggle reverses this conversion in each path.

JPEG, WebP and PNG texture bytes are decoded and encoded as real PNG files. Replacement material bindings resolve directly to container blob hashes. Material colors multiply texture pixels once, with neutral exported MTL/MQO material colors. Solid colors use generated PNGs. Textures are addressed by their encoded content hash, allowing repeated resources and solid colors to share one ZIP entry.

Model and part names use stable hashes to prevent collisions caused by punctuation, repeated part names, carriage IDs across trains, or ambiguous concatenation. Export validation reports train, carriage, layer and field for missing textures, invalid geometry, broken material references and incompatible MTR 3 settings. MTR 3 uses its selected base train for runtime movement and dimensions; bogies are static geometry and do not receive MTR 4's independent curve articulation.

## Automated acceptance

Run `cargo test --locked` from `src-tauri`. Tests cover:

- Required fields, types, enum values and resource references against the pinned MTR 4 schemas.
- OBJ and MQO packs with body and bogie layers, textures, part groups and origin instances.
- MTR 3 model pairs, qualified part properties and nonempty placement instances.
- JPEG conversion, material overrides, color tinting and actual PNG payloads.
- UV channel offsets, face winding, MQO centimetres and asymmetric UV coordinates.
- Presets evaluated by the native MTR 3 match-strength algorithm.
- Byte-identical ZIP output after changes to editor-only preview direction.
- Missing resources and unsupported configurations blocking export.
- Unsafe ZIP paths rejected before touching the destination, with atomic archive replacement.

The small fixture is synthetic and intentionally asymmetric. Passing these checks establishes the serialization and resource contracts; it does not replace a visual game test.

## Game acceptance status (2026-09-22)

The standard `%APPDATA%/.minecraft/versions` directory contained vanilla 26.2 and 26.3-snapshot-5. No matching Minecraft 1.20.4 MTR 4 or MTR 3 + NTE profile was found in the standard location or the checked `D:/Minecraft` and `D:/Games/Minecraft` locations. No game visual acceptance run was performed, and no game installation, account state or worlds were changed.

For a game acceptance run, import a real body and bogie model with an asymmetric marked texture, replace one material, and export the same project as MTR 4 OBJ, MTR 4 MQO and MTR 3 + NTE. Check front direction, readable texture markings, metre scale, bogie spacing, material colors, all part instances, and first/last/periodic/custom rules. MTR 4 placement should follow the game's selected vehicle entries; MTR 3 placement should follow the configured rules. The editor's preview consist must not change either archive.
