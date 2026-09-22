# Train editor controls

- Select a carriage on the left. Select a model layer or part in the tree, or click a visible part in the viewport.
- Expand and collapse the carriage, model layers, and part groups independently. Collapse state is remembered per project, train, and carriage. Search reveals matching descendants.
- Eye buttons toggle a carriage's layers, an individual layer, a group, or a part. Hidden children remain hidden when their parent is shown again. Visibility is saved in the project.
- Export includes hidden content by default. Enable **Only export visible** in the export dialog to exclude hidden layers and parts. This option starts unchecked whenever the dialog opens and applies to both MTR exporters.
- The viewport selects parts and controls the camera. It does not offer Move, Rotate, or Scale editing. Existing project transforms remain readable and exportable.
- Undo (`Ctrl+Z`) and Redo (`Ctrl+Shift+Z` or `Ctrl+Y`) include document and visibility changes. Text fields retain normal text editing shortcuts. History is scoped to the current editor session; saves preserve revision checks.
- Choose Perspective, Front, Back, Left, Right, or Top. Grid and Wireframe are view settings. Fit View (`F`) frames the current carriage or consist.
- Open the Consist panel to arrange repeated or reversed carriages and simulate placement rules.

## Render modes

- **Studio** uses the dark editor background and neutral lighting.
- **Unlit materials** displays material colours and textures without scene lighting.
- **Minecraft environment** uses a blue sky, block clouds, pixel grass and directional face shading. Every grass block and grid cell is exactly one metre; imported models are not automatically rescaled. This is a Minecraft-style preview, not the Minecraft engine, and cannot reproduce every resource pack or shader.

The render mode is an application preference. Environment geometry, sky and lighting are never added to the project model or exported pack.

## Verification

Automated tests cover history boundaries, pending edits, grouping, collapsed search, backend transform maths, old document defaults, and visibility-aware exports in MTR 4 OBJ/MQO and MTR 3 NTE. Export tests also verify that default exports remain unchanged when visibility is toggled.

Browser interaction checks cover collapsing and reopening groups, hiding the body mesh in the viewport, undo/redo, retaining visibility after reopening the editor, orthographic views, and the unchecked export option. The browser uses built-in sample geometry; desktop import and in-game rendering remain separate acceptance checks.
