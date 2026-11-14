# Changelog

Changes in this fork of [CreaBIM](https://github.com/chopinregis/CreaBIM)
(by Regis Benoit Brice Nde Tene), on top of CreaBIM 1.1.1.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Issue and pull request numbers refer to the upstream repository.

## [Unreleased]

### Changed
- The Catalogue window opens tall, just below the view's header and well
  above the bottom, its buttons always in sight below the list.
- Imported objects tell what they are from names with figures ("WC01")
  and from Italian names too (vaso, lavabo, doccia, letto, armadio…).
- Sets window: combinations on top, always in sight; the list of sets
  grows with the window, down to the bottom of the canvas.
- Two sets, two folders or two combinations can't share a name (capitals
  aside); the Sets window says so in red as you type.

### Added
- Guides at a typed distance: after clicking an edge or a point, type how
  far and press Enter; the pointer gives the side. Right-click a guide to
  delete all guides on its level.
- Masters: pick several items (Shift+click, box, circle or lasso, Ctrl+A)
  and move, nudge, delete, duplicate or rotate them all at once.
- Masters take any paper size: Width and Height typed in Properties, or
  A1's height with a length in A4 modules (210 mm each). Sheets made from
  the master take it.
- 3D mouse (3Dconnexion SpaceMouse) through spacenavd: pans, zooms and
  orbits the view under the pointer (plans pan and zoom), its left button
  fits everything. Settings → 3D mouse: speed, dead zone, pan and zoom
  only, a live reading, and each axis's action, direction and speed.
- An object's type takes a typed Width, Depth and Height in Properties: its
  shape and its plan drawing stretch from the point it is placed by.
- After importing a DXF as an object, the status line says its size and the
  unit the file names, and warns when the size looks wrong.
- Catalogue: a Units choice for DXF drawings (from the file, mm, cm or m),
  for files that name the wrong unit.
- Objects from DXF drawings: in the Catalogue, type a height and *Import DXF
  as plan symbol…*. The drawing (blocks exploded, text and hatches left
  out) is what plans show, its point 0,0 is where the object is placed
  from, and a plain box that tall stands for it in 3D and IFC. Its picture
  in the catalogue is the drawing, seen from above.
- Combinations of sets, like ArchiCAD's layer combinations: in the Sets
  window, save which sets are on and off in a view under a name, and pick
  one for the open view with its radio button. Switching sets by hand marks
  it *modified*; *Update* stores the change, and the views using it follow.
  *New view* copies the open view with a combination, leaving the view you
  work on whole. Duplicate and delete them from the ⋯ menu.
- Sets: your own groups of elements, of any category (a wall, a slab and a
  chair can share one), like layers. An element can be in several sets; a
  view leaves it out when any of them is switched off there. Make, rename,
  organise in folders, select, isolate, duplicate, merge and delete them in
  Manage → Sets; put the selection in them from the Sets button of the
  element action bar, choosing the folder of a new one; switch them on and
  off from the view's Visibility menu (Sets ▸) or its panel in Properties,
  a whole folder at once. Views, sheets and view templates keep what they
  switch off; the 3D view, for the session. Files with sets don't open in
  CreaBIM 1.1.1.
- A project file keeps the views that were open when it was saved, and
  reopens on them. Opening a file no longer takes the last project's areas
  along: a file saved before, a new project or the starter house open on
  one area with the first floor plan.
- The window opens at 70% of the screen the first time, then where and how
  large it was left.

### Fixed
- The view bar's Grid button turns the grid behind drawings off and on
  again, and a sheet's Picture button opens the picture dialog: both did
  nothing.
- Levels window: changing a storey's Height no longer pushes its row down
  (the Fit button sits beside the gap; Duplicate and Delete stay on one
  line).
- Plans: lines in the fine pen (furniture and fixtures) are drawn in the
  same light grey as closed outlines, so an object drawn with lines and one
  drawn with polylines look alike.
- Temporary dimensions' numbers read in every theme: their label sits on
  the drawing's own ground, its number coloured to stand out from it.
- Objects from DXF drawings show only their lines in plans, without the
  grey tint of their box.
- An object imported again into the catalogue (a DXF with the right units,
  say) is placed with its new shape: the project's type of it, and the
  objects already placed, take it too.
- A new project or a sample left as it came closes without asking to save.
- Running the tests no longer puts their sample file in Open recent.
- Hiding something in a view (a set, a category, Hide in view) takes it out
  of the selection: its grips and dimensions no longer stay over nothing.
- The view's Visibility panel lists the elements hidden in it even when the
  project has no grid systems (the list was kept inside the Grid systems
  section).
- Switching an area back to Floor plans (or Sections, Elevations…) from its
  header opens the view of that group you had open last, not the first in
  the browser: coming back from 3D no longer lands on another plan.
- The view's Duplicate button and Shift+D duplicate a view at once, as copy
  and paste does, instead of failing with "That can't be duplicated."
  ([#35](https://github.com/chopinregis/CreaBIM/issues/35),
  [#36](https://github.com/chopinregis/CreaBIM/pull/36))
- DXF references and title blocks: blocks (INSERT) and dimensions are
  exploded into their lines and text, and DXF files that are not UTF-8
  (AutoCAD 2004 and older, code page 1252) open.
  ([#37](https://github.com/chopinregis/CreaBIM/issues/37),
  [#38](https://github.com/chopinregis/CreaBIM/pull/38))
- A bar's tooltip no longer stays stuck over the drawing area.
  ([#39](https://github.com/chopinregis/CreaBIM/issues/39),
  [#40](https://github.com/chopinregis/CreaBIM/pull/40))
- Picking a master or profile in the browser brings its view to the front,
  and Properties shows it whatever else is picked.
  ([#41](https://github.com/chopinregis/CreaBIM/issues/41),
  [#42](https://github.com/chopinregis/CreaBIM/pull/42))
- The build stays clean with Rust 1.100's lints.
  ([#43](https://github.com/chopinregis/CreaBIM/issues/43),
  [#44](https://github.com/chopinregis/CreaBIM/pull/44))
- A master can be renamed: typing in its Name field no longer flickers and
  is lost.
  ([#45](https://github.com/chopinregis/CreaBIM/issues/45),
  [#46](https://github.com/chopinregis/CreaBIM/pull/46))
- A turned (e.g. vertical) text in a master is outlined and picked along its
  letters, its box covering it end to end.
  ([#47](https://github.com/chopinregis/CreaBIM/issues/47),
  [#48](https://github.com/chopinregis/CreaBIM/pull/48))
- A picked master text's words and the values of a master's own fields can
  be typed into.
  ([#49](https://github.com/chopinregis/CreaBIM/issues/49),
  [#50](https://github.com/chopinregis/CreaBIM/pull/50))
