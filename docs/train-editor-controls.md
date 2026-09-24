# Train editor controls

- Select a carriage on the left. Select a model layer or part in the tree, or click a visible part in the viewport.
- Expand and collapse the carriage, model layers, and part groups independently. Collapse state is remembered per project, train, and carriage. Search reveals matching descendants.
- Eye buttons toggle a carriage's layers, an individual layer, a group, or a part. Hidden children remain hidden when their parent is shown again. Visibility is saved in the project.
- Export includes hidden content by default. Enable **Only export visible** in the export dialog to exclude hidden layers and parts. This option starts unchecked whenever the dialog opens and applies to both MTR exporters.
- The viewport selects parts and controls the camera. It does not offer Move, Rotate, or Scale editing. Existing project transforms remain readable and exportable.
- Undo (`Ctrl+Z`) and Redo (`Ctrl+Shift+Z` or `Ctrl+Y`) include document and visibility changes. Text fields retain normal text editing shortcuts. History is scoped to the current editor session; saves preserve revision checks.
- Choose Perspective, Front, Back, Left, Right, or Top. Grid and Wireframe are view settings. Fit View (`F`) frames the current carriage or consist.
- The translucent blue footprint marks each carriage's configured length and width in every render mode. It updates as those dimensions change and remains visible when Grid is off.
- Open the Consist panel to arrange repeated or reversed carriages and simulate placement rules.

## Render modes

Three SVG icon buttons immediately after Fit View select the render mode. Hover for the name and description; arrow keys move between modes. The adjacent settings button opens a popover. Escape and clicking outside close it.

- **Studio** uses neutral grey materials and soft camera-relative studio lights for inspecting shape. Texture alpha cutouts remain intact. There is no scenery or exported material change.
- **Material Preview** uses physical materials, a bundled procedural studio reflection environment, direct light, real-time self-shadowing and an optional neutral receiving floor.
- **Minecraft** shares the physical lighting pipeline and adds pixel grass, sky, block clouds and a sun aligned with the shadow-casting light. Every grass block and grid cell is one metre. Imported models are never rescaled.

Settings include grid and wireframe, standard/high shadow quality, light direction, elevation and intensity, environment intensity, and the Material Preview floor. Reset affects the current mode. Rendering preferences are stored in application preferences and do not dirty or change the project or exported pack. Old Unlit preferences migrate to Material Preview.

PBR source materials retain their supported roughness, metalness, normal, AO, emissive and transparency properties in preview. The environment renderer accepts standard/physical materials without flattening these channels. This release does not add a complete PBR authoring panel or new exporter mappings.

Minecraft uses the editor's real-time renderer; it does not execute Minecraft shaders. Shadow resolution has a finite budget, so very long consists trade close-up detail for coverage.

## Verification

Automated tests cover history boundaries, pending edits, grouping, collapsed search, backend transform maths, old document defaults, and visibility-aware exports in MTR 4 OBJ/MQO and MTR 3 NTE. Export tests also verify that default exports remain unchanged when visibility is toggled.

Browser interaction checks cover collapsing and reopening groups, hiding the body mesh in the viewport, undo/redo, retaining visibility after reopening the editor, orthographic views, and the unchecked export option. The browser uses built-in sample geometry; desktop import and in-game rendering remain separate acceptance checks.

Render-mode acceptance checks cover icon and keyboard switching, Studio grey shading, real ground shadows changing with light direction, optional material floor, Escape/outside dismissal, and mode/light/quality preferences restored after reloading. Automated environment tests check PBR channel preservation, source-resource ownership, shadow target disposal, sun/light alignment, one-metre ground tiles and long-consist shadow coverage.
