# Eye candy workflow and export contract

Eye candy is editable project content for static station props and environmental
objects. Create an entry from **Eye candy → New Eye Candy**, open it, and import
an OBJ, FBX, MQO or BBMODEL model, or choose a model from the Asset Library. Objects may
contain multiple layers. Layers can be renamed, reordered, duplicated, hidden or
removed. The model tree supports picking and hiding individual parts.

The inspector edits the name, export ID, description and MTR 4 selector color.
Position uses metres, rotation uses degrees in the UI (radians in storage), and
scale uses positive ratios. Transforms apply to the selected layer or part.
Material and texture controls share the train editor's existing implementation.
Studio, Material Preview and Minecraft modes use the same renderer.

Edits save after an 800 ms pause. Ctrl+S, leaving the editor, changing projects,
exporting and closing the desktop app flush pending edits. Save failures retain
the editor and allow retry or explicit reload. Undo and redo are scoped to the
editor session. The document, imported assets and replacement textures live in
the portable `.mtrpack` file. Asset Library reference counts include eye candy
and prevent deletion of resources still used by an object.

## Export

- **MTR 4.0.5 / Minecraft 1.20.4:** `objects` in the project's
  `mtr_custom_resources.json`, with OBJ or MQO models, named textures and all
  required object fields. Transforms and per-layer UV flipping are baked into
  geometry; resource transforms are identity.
- **MTR 3 + NTE:** a map in
  `assets/mtrsteamloco/eyecandies/<namespace>.json`. Keys include the project
  namespace to avoid collisions across packs. Models are OBJ/MTL in the project's
  namespace, with adjacent PNG textures. Geometry is merged into a single static
  object while retaining separate parts and materials.

Hidden layers and parts are exported by default. **Only export visible** filters
them. Validation identifies the eye candy and model layer for invalid IDs,
missing models/textures, invalid transforms or mesh data, and empty objects.
Train placement rules cannot be applied to static objects.

Upstream contracts are pinned to the same versions as the train exporter:
[MTR object schema](https://github.com/Minecraft-Transit-Railway/Minecraft-Transit-Railway/blob/c7199b24917f6d26e13216769725180ca06a69b5/buildSrc/src/main/resources/schema/resource/objectResource.json),
[MTR object loader](https://github.com/Minecraft-Transit-Railway/Minecraft-Transit-Railway/blob/c7199b24917f6d26e13216769725180ca06a69b5/fabric/src/main/java/org/mtr/mod/resource/ObjectResource.java),
[NTE registry](https://github.com/zbx1425/mtr-nte/blob/c5d1443f79b4e19135c03a00fe28217f9c477ea0/common/src/main/java/cn/zbx1425/mtrsteamloco/data/EyeCandyRegistry.java).

## Verification

Rust tests cover eye-candy-only packs in all three export variants, multiple
layers/materials, transforms, UV handling, deterministic archives, visibility,
error locations, untouched export destinations on failure, revision conflicts,
failed-import/commit rollback, resource references, compaction, reopening and
deletion. Existing train and rail contracts also run.

Browser checks exercised creation, Asset Library attachment, layer translation
and rotation, save, undo/redo and retained properties after reopening. Real model
import and ZIP output require the desktop app. Game visual acceptance remains
pending; these checks verify editing and serialization, not rendering in a
running Minecraft instance.
