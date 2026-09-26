# Rail export contract

Rails are independent project contents. A rail may contain multiple model layers; export merges visible or all geometry into a single static model, as selected by the export dialog. Layer transforms, hidden parts, texture overrides, UV conventions and material colors follow the train exporter. Repeat intervals are measured in metres and must be positive.

## MTR 4.0.5

The pinned [rail schema](https://github.com/Minecraft-Transit-Railway/Minecraft-Transit-Railway/blob/c7199b24917f6d26e13216769725180ca06a69b5/buildSrc/src/main/resources/schema/resource/railResource.json) declares a single model resource, repeat interval, model Y offset, display name, ID, color and texture fallback. The exporter appends `rails` to the project manifest and supports both OBJ and MQO. Color defaults to `777777` and Y offset is baked into model geometry.

[ModelResourceLoader](https://github.com/Minecraft-Transit-Railway/Minecraft-Transit-Railway/blob/c7199b24917f6d26e13216769725180ca06a69b5/fabric/src/main/java/org/mtr/mod/resource/ModelResourceLoader.java) resolves named MTL/MQO textures relative to the model. Textures are stored alongside the rail model with content-hash names. This preserves multiple textures without an atlas. UV flips are baked per layer before merging, and the resource flip flag is false.

## MTR 3 with NTE

[NTE RailModelRegistry](https://github.com/zbx1425/mtr-nte/blob/c5d1443f79b4e19135c03a00fe28217f9c477ea0/common/src/main/java/cn/zbx1425/mtrsteamloco/data/RailModelRegistry.java) scans `assets/mtrsteamloco/rails/*.json`. The exporter writes one map there, keyed by project namespace and rail export ID. Models and adjacent PNG textures remain in the project's own namespace. Each entry contains `name`, `model`, `repeatInterval`, `yOffset: 0` and `flipV: false`. Models use OBJ/MTL.

[Author documentation](https://wiki.zbx1425.cn/mtr-nte:railmodel) describes repeat intervals and notes that the runtime places rigid repeated segments along curves. Authors should allow a little overlap in their rail geometry to avoid curve gaps.

## Verification

Rust contract tests cover rail-only projects, both targets and both MTR 4 model formats, multiple textures, deterministic ZIP bytes, schema fields, missing assets, invalid repeat intervals and the opt-in visibility filter. These are serialization tests; visual game acceptance still requires installed MTR/NTE profiles.
