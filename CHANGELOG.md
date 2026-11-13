# Changelog

Changes in this fork of [CreaBIM](https://github.com/chopinregis/CreaBIM)
(by Regis Benoit Brice Nde Tene), on top of CreaBIM 1.1.1.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Issue and pull request numbers refer to the upstream repository.

## [Unreleased]

### Added
- Sets: your own groups of elements, of any category (a wall, a slab and a
  chair can share one), like layers. An element can be in several sets; a
  view leaves it out when any of them is switched off there. Make, rename,
  organise in folders, select, isolate, duplicate, merge and delete them in
  Manage → Sets; put the selection in them from the Sets button of the
  element action bar; switch them on and off from the view's Visibility
  menu (Sets ▸). Views, sheets and view templates keep what they switch off.
- The window opens at 70% of the screen the first time, then where and how
  large it was left.

### Fixed
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
