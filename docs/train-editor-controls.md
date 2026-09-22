# Train editor controls

- Select a carriage on the left. Select a model layer or part in the tree, or click a visible part in the viewport.
- Expand and collapse the carriage, model layers, and part groups independently. Collapse state is remembered per project, train, and carriage. Search reveals matching descendants.
- Eye buttons toggle a carriage's layers, an individual layer, a group, or a part. Hidden children remain hidden when their parent is shown again. Visibility is saved in the project.
- Export includes hidden content by default. Enable **Only export visible** in the export dialog to exclude hidden layers and parts. This option starts unchecked whenever the dialog opens and applies to both MTR exporters.
- Use Select (`V`), Move (`G`), Rotate (`R`), and Scale (`S`). Transform handles and numeric fields edit the selected model layer or part. Positions use metres; the inspector displays rotation in degrees. Scale must remain positive. Reset transform restores the selected target's identity transform.
- Undo (`Ctrl+Z`) and Redo (`Ctrl+Shift+Z` or `Ctrl+Y`) include visibility and transform changes. Text fields retain normal text editing shortcuts. History is scoped to the current editor session; saves preserve revision checks.
- Choose Perspective, Front, Back, Left, Right, or Top. Grid and Wireframe are view settings. Fit View (`F`) frames the current carriage or consist.
- Open the Consist panel to arrange repeated or reversed carriages and simulate placement rules.

## Verification

Automated tests cover history boundaries, pending edits, grouping, collapsed search, backend transform maths, old document defaults, and visibility-aware exports in MTR 4 OBJ/MQO and MTR 3 NTE. Export tests also verify that default exports remain unchanged when visibility is toggled.

Browser interaction checks cover collapsing and reopening groups, hiding the body mesh in the viewport, undo/redo, retaining visibility after reopening the editor, orthographic views, numeric transforms and undo, and the unchecked export option. The browser uses built-in sample geometry; desktop import and in-game rendering remain separate acceptance checks.
