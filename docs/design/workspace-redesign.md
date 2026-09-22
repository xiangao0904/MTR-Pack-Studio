# Silver graphite workspace redesign

The home screen is unchanged. Project pages and the train editor share a scoped workspace design system in `src/studio-design.css`. Surface, border, text, selection and typography tokens also cover dialogs, inspector controls and the viewport settings popover.

## Visual direction

- Neutral graphite surfaces, cool white primary actions and silver selections.
- 48 px application title bar, compact editing toolbar, consistent 32 px form controls.
- Project overview uses an editorial summary, small content cards and fine table separators.
- Train editor uses a compact carriage list, a large viewport and flat inspector sections.
- No invented project statistics, train dimensions or decorative controls from the concept are implemented.
- Selection, hierarchy controls, import, export, saving and rendering remain functional.

Reference: [Linear's UI redesign](https://linear.app/now/how-we-redesigned-the-linear-ui), particularly reducing visual noise and aligning navigation and panel hierarchy.

## Generated concept

`workspace-silver-concept.png` was generated before implementation using the built-in image generation tool. It is a design reference, not a runtime UI texture. Model appearance in the running application comes from the imported assets.

### Generation prompt

Use case: ui-mockup. Create a polished high fidelity UI design specification board for MTR Pack Studio, a professional desktop Minecraft railway resource editor. TWO large application screen mockups stacked vertically on a portrait canvas: upper Project overview, lower Train editor. Both use EXACT SAME design language inspired by Linear application UI and precision desktop Photoshop/Illustrator/After Effects/Blender workflows. Beautiful restrained monochrome dark graphite, cool white / silver-grey typography and primary actions, extremely subtle neutral hairline borders, small consistent corner radius, NO blue accent gradients, NO neon, NO decorative dashboard statistics, NO large hero image. Crisp Inter-like typography, excellent spacing. Each app frame widescreen 16:9. English only. Compact 48px titlebar brand MTR Pack Studio, breadcrumb Home / Urban Rail Collection, saved state, small Settings/Help/window controls. Upper: narrow quiet sidebar with project icon+name, Overview active, All Content, Trains, Eye candy Planned, PIDS Planned, Asset Library, Project Settings. Main elegant title Project overview, description, silver-white New Train button. Project description area is borderless editorial text 'Urban Rail Collection', small namespace urban_rail, concise description. Three compact content category cards Trains / Eye candy / PIDS. Refined Recently edited table with 3 train rows and small train thumbnails. Generous breathing space. Lower: titlebar breadcrumb includes Metro EMU, slim 40px toolbar with Undo/Redo, Perspective, Grid, Wireframe, Fit View, 3 tiny render-mode sphere/sphere/grass-cube icon buttons, right silver-white Export Pack. Left 210px panel Carriages, search, 3 compact train thumbnail rows with names A Car, B Car, C Car; active row neutral grey inset line, bottom duplicate/delete. Center large professional 3D viewport displays detailed silver subway carriage on dark neutral background, subdued grid. Below compact hierarchy Model Tree / Materials with real nested rows Body, Windows, Doors, Bogies, eye icons, one selected silver row. Right 280px inspector with small General / Models / Placement / MTR 3 tabs; General selected, train name and ID then Carriage dimensions in aligned COMPACT label/value rows, not tall boxes or nested outlined cards; section dividers. Neutral silver focus. Include no move rotate scale controls. This is final UI concept to implement, visually practical accurate proportions and sharp readable text, premium calm cohesive desktop tool, no posters or explanatory annotations outside frames.
