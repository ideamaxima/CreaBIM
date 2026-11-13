//! Objects for the catalogue from other tools' files: a Blender (`.blend`),
//! FreeCAD (`.FCStd`) or glTF (`.gltf`, `.glb`) file read as one object,
//! each of its meshes a part in its own material, set on the floor with the
//! middle of its back edge at the origin. What it is (category, IFC class,
//! wall or floor) comes from an IFC class asked for in the file, else is
//! guessed from the names; all of it can be changed once it is in.

use crate::{ExchangeError, Incoming};
use creabim_core::objects::{
    self, CatalogueObject, IfcKind, Mount, ObjectCategory, ObjectPart, ObjectType,
};
use creabim_core::{Hatch, Material};
use std::path::Path;

/// The file kinds an object can be read from (lower-case extensions).
pub const EXTENSIONS: &[&str] = &["blend", "fcstd", "gltf", "glb"];

/// Read a file as a catalogue object keyed `key`.
pub fn read_file(path: &Path, key: &str) -> Result<CatalogueObject, ExchangeError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let (incoming, looks) = match ext.as_str() {
        "blend" => {
            let scene = creabim_blend::read_file(path)?;
            (
                crate::blend::incoming(&scene),
                crate::materials::read_blend(&scene),
            )
        }
        "fcstd" => (crate::freecad::read_file(path)?, vec![]),
        "gltf" | "glb" => crate::gltf_read::read_file(path)?,
        other => {
            return Err(ExchangeError::Other(format!(
                "CreaBIM can't read objects from .{other} files"
            )))
        }
    };
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Object")
        .replace(['_', '-'], " ");
    object(&incoming, &looks, &stem, key).ok_or_else(|| {
        ExchangeError::Other(format!(
            "{} has no meshes to make an object of",
            path.display()
        ))
    })
}

/// The object made of `incoming`, its materials' full looks taken from
/// `looks` by name when there.
pub fn object(
    incoming: &[Incoming],
    looks: &[(String, Material)],
    name: &str,
    key: &str,
) -> Option<CatalogueObject> {
    let mut parts = vec![];
    let mut materials = vec![];
    let mut class = None;
    for inc in incoming.iter().filter(|i| !i.mesh.faces.is_empty()) {
        if let Some(crate::Assign::Class(c)) = &inc.hint {
            let c = objects::class_of_type(c).unwrap_or(c);
            if !objects::predefined_types(c).is_empty() {
                class.get_or_insert(c.to_string());
            }
        }
        parts.push(ObjectPart {
            name: inc.name.clone(),
            mesh: inc.mesh.clone(),
            material: None,
        });
        materials.push(inc.material.as_ref().map(|(n, color)| {
            let m = looks
                .iter()
                .find(|(l, _)| l == n)
                .map(|(_, m)| m.clone())
                .unwrap_or_else(|| Material::new(Hatch::Empty, *color));
            (n.clone(), m)
        }));
    }
    if parts.is_empty() {
        return None;
    }
    objects::recentre(&mut parts);
    let words = std::iter::once(name)
        .chain(parts.iter().map(|p| p.name.as_str()))
        .collect::<Vec<_>>()
        .join(" ");
    let (category, guessed, mount) = guess(&words);
    let ifc = match class {
        Some(c) => IfcKind::new(&c, guessed.predefined.as_str()).normalized(),
        None => guessed,
    };
    let name = name.trim();
    let name = if name.is_empty() { "Object" } else { name };
    let mut chars = name.chars();
    let name: String = chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default();
    Some(CatalogueObject {
        key: key.to_string(),
        name,
        description: String::new(),
        object: ObjectType {
            category,
            ifc,
            mount,
            offset: 0.0,
            parts,
            symbol: vec![],
            catalogue: Some(key.to_string()),
        },
        materials,
    })
}

/// A plan-only object from a DXF drawing: its lines (blocks exploded,
/// arcs as short segments) are the plan symbol, its point (0,0) where the
/// object is placed from, and a plain box over the drawing, `height` tall,
/// stands for it in 3D. Text, hatches and dimensions are left out. Its
/// unit is `unit` mm when given, else the one its header names.
pub fn read_dxf(
    bytes: &[u8],
    name: &str,
    key: &str,
    height: f64,
    unit: Option<f64>,
) -> Result<CatalogueObject, ExchangeError> {
    let text = crate::titleblock::dxf_text(bytes);
    let items = crate::titleblock::dxf_items_in(&text, unit).map_err(ExchangeError::Other)?;
    let mut symbol = vec![];
    for it in items {
        let creabim_core::titleblock::TitleItem::Line { points, .. } = it else {
            continue;
        };
        // the reader gives millimetres
        let pts: Vec<glam::DVec2> = points.iter().map(|p| *p / 1000.0).collect();
        if pts.len() < 2 {
            continue;
        }
        let closed = pts.len() > 3 && pts[0].distance(pts[pts.len() - 1]) < 1e-9;
        if closed {
            symbol.push(objects::SymbolItem::Loop(pts[..pts.len() - 1].to_vec()));
        } else {
            for w in pts.windows(2) {
                symbol.push(objects::SymbolItem::Line(w[0], w[1]));
            }
        }
    }
    let all: Vec<glam::DVec2> = symbol
        .iter()
        .flat_map(|s| match s {
            objects::SymbolItem::Loop(p) => p.clone(),
            objects::SymbolItem::Line(a, b) => vec![*a, *b],
        })
        .collect();
    let Some(first) = all.first().copied() else {
        return Err(ExchangeError::Other(
            "the DXF has no lines, polylines, arcs or circles".into(),
        ));
    };
    let (lo, hi) = all
        .iter()
        .fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    let size = hi - lo;
    if size.x < 1e-6 || size.y < 1e-6 {
        return Err(ExchangeError::Other(
            "the drawing has no width or no depth".into(),
        ));
    }
    let height = height.max(0.001);
    let centre = (lo + hi) / 2.0;
    let mut mesh = creabim_core::EditMesh::cuboid(glam::DVec3::new(size.x, size.y, height));
    for v in &mut mesh.vertices {
        v.x += centre.x;
        v.y += centre.y;
    }
    let (category, ifc, mount) = guess(name);
    let mut chars = name.trim().chars();
    let shown: String = chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_else(|| "Object".into());
    let mut c = CatalogueObject {
        key: key.to_string(),
        name: shown,
        description: String::new(),
        object: ObjectType {
            category,
            ifc,
            mount,
            offset: 0.0,
            parts: vec![ObjectPart {
                name: objects::PLAN_BOX.into(),
                mesh,
                material: None,
            }],
            symbol: vec![],
            catalogue: Some(key.to_string()),
        },
        materials: vec![None],
    };
    c.object.symbol = symbol;
    Ok(c)
}

/// What a thing called `words` probably is.
pub fn guess(words: &str) -> (ObjectCategory, IfcKind, Mount) {
    use ObjectCategory as C;
    // whole words only, so a "floor plane" is no loo; plurals count
    // "WC01" is "wc 01": letters and figures apart
    let mut spaced = String::new();
    let mut last: Option<bool> = None;
    for ch in words.to_lowercase().chars() {
        let digit = ch.is_ascii_digit();
        if ch.is_alphanumeric() && last.is_some_and(|d| d != digit) {
            spaced.push(' ');
        }
        last = ch.is_alphanumeric().then_some(digit);
        spaced.push(if ch.is_alphanumeric() { ch } else { ' ' });
    }
    let lower = spaced;
    let list: Vec<&str> = lower.split(' ').filter(|w| !w.is_empty()).collect();
    let joined = format!(" {} ", list.join(" "));
    let has = |k: &[&str]| {
        k.iter().any(|k| {
            if k.contains(' ') {
                joined.contains(&format!(" {k} "))
            } else {
                list.iter()
                    .any(|w| *w == *k || w.strip_suffix('s').or(w.strip_suffix("es")) == Some(k))
            }
        })
    };
    let (c, class, p, m) = if has(&[
        "washbasin",
        "basin",
        "lavatory",
        "vanity",
        "lavabo",
        "lavandino",
        "lavello bagno",
    ]) {
        (
            C::Bathroom,
            "IfcSanitaryTerminal",
            "WASHHANDBASIN",
            Mount::Wall,
        )
    } else if has(&["toilet", "wc", "loo", "vaso", "bidet"]) {
        (C::Bathroom, "IfcSanitaryTerminal", "TOILETPAN", Mount::Wall)
    } else if has(&["bathtub", "bath tub", "tub", "bath", "vasca"]) {
        (C::Bathroom, "IfcSanitaryTerminal", "BATH", Mount::Wall)
    } else if has(&["shower", "doccia", "piatto doccia"]) {
        (C::Bathroom, "IfcSanitaryTerminal", "SHOWER", Mount::Wall)
    } else if has(&["sink", "lavello"]) {
        (C::Kitchen, "IfcSanitaryTerminal", "SINK", Mount::Wall)
    } else if has(&[
        "fridge",
        "refrigerator",
        "freezer",
        "frigo",
        "frigorifero",
        "congelatore",
    ]) {
        (
            C::Kitchen,
            "IfcElectricAppliance",
            "FRIDGE_FREEZER",
            Mount::Wall,
        )
    } else if has(&[
        "oven",
        "cooker",
        "hob",
        "stove",
        "range",
        "forno",
        "piano cottura",
        "cucina gas",
        "fornelli",
    ]) {
        (
            C::Kitchen,
            "IfcElectricAppliance",
            "ELECTRICCOOKER",
            Mount::Wall,
        )
    } else if has(&["dishwasher", "lavastoviglie"]) {
        (
            C::Kitchen,
            "IfcElectricAppliance",
            "DISHWASHER",
            Mount::Wall,
        )
    } else if has(&["socket", "outlet", "receptacle", "presa", "prese"]) {
        (C::Electrical, "IfcOutlet", "POWEROUTLET", Mount::Wall)
    } else if has(&["switch", "interruttore", "deviatore"]) {
        (
            C::Electrical,
            "IfcSwitchingDevice",
            "TOGGLESWITCH",
            Mount::Wall,
        )
    } else if has(&["consumer unit", "distribution board", "fuse box", "fusebox"]) {
        (
            C::Electrical,
            "IfcElectricDistributionBoard",
            "CONSUMERUNIT",
            Mount::Wall,
        )
    } else if has(&["smoke", "detector", "alarm"]) {
        (C::Electrical, "IfcSensor", "SMOKESENSOR", Mount::Ceiling)
    } else if has(&[
        "pendant",
        "downlight",
        "spotlight",
        "chandelier",
        "ceiling light",
        "ceiling lamp",
    ]) {
        (
            C::Lighting,
            "IfcLightFixture",
            "POINTSOURCE",
            Mount::Ceiling,
        )
    } else if has(&["wall light", "wall lamp", "sconce"]) {
        (
            C::Lighting,
            "IfcLightFixture",
            "DIRECTIONSOURCE",
            Mount::Wall,
        )
    } else if has(&["drain", "gully", "piletta"]) {
        (C::Plumbing, "IfcWasteTerminal", "FLOORWASTE", Mount::Floor)
    } else if has(&["boiler", "water heater", "caldaia", "scaldabagno"]) {
        (C::Plumbing, "IfcBoiler", "WATER", Mount::Wall)
    } else if has(&["tap", "faucet", "valve", "rubinetto"]) {
        (C::Plumbing, "IfcValve", "FAUCET", Mount::Wall)
    } else if has(&[
        "radiator",
        "heater",
        "convector",
        "radiatore",
        "termosifone",
        "scaldasalviette",
    ]) {
        (C::Hvac, "IfcSpaceHeater", "RADIATOR", Mount::Wall)
    } else if has(&["diffuser", "grille", "vent"]) {
        (C::Hvac, "IfcAirTerminal", "DIFFUSER", Mount::Ceiling)
    } else if has(&["air conditioner", "air conditioning", "ac unit", "split"]) {
        (C::Hvac, "IfcUnitaryEquipment", "SPLITSYSTEM", Mount::Wall)
    } else if has(&["fan"]) {
        (C::Hvac, "IfcFan", "PROPELLORAXIAL", Mount::Wall)
    } else if has(&["sofa", "couch", "settee", "divano"]) {
        (C::Living, "IfcFurniture", "SOFA", Mount::Wall)
    } else if has(&[
        "armchair", "chair", "stool", "seat", "sedia", "sedie", "poltrona", "sgabello",
    ]) {
        (C::Dining, "IfcFurniture", "CHAIR", Mount::Floor)
    } else if has(&["desk", "scrivania"]) {
        (C::Office, "IfcFurniture", "DESK", Mount::Wall)
    } else if has(&["bed", "letto", "letti"]) {
        (C::Bedroom, "IfcFurniture", "BED", Mount::Wall)
    } else if has(&["table", "tavolo", "tavolino"]) {
        (C::Dining, "IfcFurniture", "TABLE", Mount::Floor)
    } else if has(&[
        "wardrobe", "closet", "shelf", "shelves", "bookcase", "armadio", "libreria", "scaffale",
        "mensola",
    ]) {
        (C::Bedroom, "IfcFurniture", "SHELF", Mount::Wall)
    } else if has(&[
        "cabinet",
        "cupboard",
        "drawer",
        "unit",
        "mobile",
        "pensile",
        "cassettiera",
        "comodino",
    ]) {
        (C::Kitchen, "IfcFurniture", "FILECABINET", Mount::Wall)
    } else if has(&[
        "lamp", "light", "plant", "rug", "mirror", "lampada", "tappeto", "specchio",
    ]) {
        (C::Living, "IfcFurniture", "NOTDEFINED", Mount::Floor)
    } else {
        (C::Other, "IfcFurniture", "NOTDEFINED", Mount::Floor)
    };
    (c, IfcKind::new(class, p).normalized(), m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(rel: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(rel)
    }

    fn sits_on_floor(c: &CatalogueObject) {
        let (lo, hi) = c.object.bounds();
        assert!(lo.z.abs() < 1e-9, "{lo:?}");
        assert!(hi.y.abs() < 1e-9, "{hi:?}");
        assert!((lo.x + hi.x).abs() < 1e-9, "{lo:?} {hi:?}");
        assert_eq!(c.materials.len(), c.object.parts.len());
        c.object.validate().unwrap();
    }

    #[test]
    fn blender_and_freecad_files_become_objects() {
        let b = read_file(
            &fixture("creabim-blend/fixtures/scene_4_0.blend"),
            "user/scene",
        )
        .unwrap();
        assert!(!b.object.parts.is_empty());
        sits_on_floor(&b);
        assert_eq!(b.name, "Scene 4 0");
        let f = read_file(
            &fixture("creabim-freecad/fixtures/parts.FCStd"),
            "user/parts",
        )
        .unwrap();
        assert!(!f.object.parts.is_empty());
        sits_on_floor(&f);
        assert!(read_file(Path::new("chair.obj"), "x").is_err());
    }

    #[test]
    fn a_sofa_exported_to_gltf_comes_back_as_a_sofa() {
        let model = creabim_core::sample::furnished_house();
        let glb = crate::gltf::export_glb(&model);
        let (inc, looks) = crate::gltf_read::read(&glb, &|_| None).unwrap();
        let sofa: Vec<Incoming> = inc
            .into_iter()
            .filter(|i| i.name.starts_with("Sofa"))
            .collect();
        assert!(!sofa.is_empty());
        let back = object(&sofa, &looks, "sofa from blender", "user/sofa").unwrap();
        sits_on_floor(&back);
        assert_eq!(back.name, "Sofa from blender");
        assert_eq!(back.object.ifc.predefined, "SOFA");
        assert_eq!(back.object.category, ObjectCategory::Living);
        let starter = creabim_core::catalogue::starter("starter/sofa-3").unwrap();
        assert!((back.object.size().z - starter.object.size().z).abs() < 1e-3);
        assert!(back.materials.iter().any(|m| m.is_some()));
    }

    #[test]
    fn names_say_what_things_are_with_figures_and_in_italian() {
        assert_eq!(guess("WC01").1.class, "IfcSanitaryTerminal");
        assert_eq!(guess("WC_001").1.class, "IfcSanitaryTerminal");
        assert_eq!(guess("vaso sospeso").1.predefined, "TOILETPAN");
        assert_eq!(guess("piatto doccia 80x80").1.predefined, "SHOWER");
        assert_eq!(guess("Letto matrimoniale").1.predefined, "BED");
        assert_eq!(guess("armadio 2 ante").1.predefined, "SHELF");
        assert_eq!(guess("divano3posti").0, ObjectCategory::Living);
    }

    #[test]
    fn names_say_what_things_are() {
        assert_eq!(guess("Kitchen sink").1.predefined, "SINK");
        assert_eq!(guess("toilet").1.predefined, "TOILETPAN");
        assert_eq!(guess("Office desk").0, ObjectCategory::Office);
        assert_eq!(guess("Bar stool").2, Mount::Floor);
        assert_eq!(guess("Thing").1, IfcKind::default());
        // words, not letters inside them
        assert_eq!(guess("Floor lamp").1, IfcKind::default());
        assert_eq!(guess("Floor lamp").0, ObjectCategory::Living);
        // building services
        let (c, k, m) = guess("Double socket outlet");
        assert_eq!(
            (c, k.class.as_str(), m),
            (ObjectCategory::Electrical, "IfcOutlet", Mount::Wall)
        );
        let (c, k, m) = guess("Pendant_light");
        assert_eq!(
            (c, k.predefined.as_str(), m),
            (ObjectCategory::Lighting, "POINTSOURCE", Mount::Ceiling)
        );
        assert_eq!(
            guess("Consumer unit").1.class,
            "IfcElectricDistributionBoard"
        );
        assert_eq!(guess("Radiators").1.class, "IfcSpaceHeater");
        assert_eq!(guess("Kitchen_Sink.001").1.predefined, "SINK");
        assert_eq!(guess("Dining chairs").1.predefined, "CHAIR");
    }

    #[test]
    fn a_dxf_drawing_becomes_a_plan_symbol_on_a_box_from_its_own_origin() {
        // a 400 × 600 mm outline drawn from its back corner, a line and a
        // circle, with a text left out
        let dxf = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n90\n4\n70\n1\n10\n0\n20\n0\n10\n400\n20\n0\n10\n400\n20\n-600\n10\n0\n20\n-600\n0\nLINE\n10\n0\n20\n-100\n11\n400\n21\n-100\n0\nCIRCLE\n10\n200\n20\n-350\n40\n100\n0\nTEXT\n10\n5\n20\n5\n40\n4\n1\nWC\n0\nENDSEC\n0\nEOF\n";
        let c = read_dxf(dxf.as_bytes(), "toilet", "user/toilet", 0.4, None).unwrap();
        assert!(c
            .object
            .symbol
            .iter()
            .any(|s| matches!(s, objects::SymbolItem::Loop(p) if p.len() == 4)));
        assert!(c
            .object
            .symbol
            .iter()
            .any(|s| matches!(s, objects::SymbolItem::Line(..))));
        let (lo, hi) = c.object.bounds();
        assert!((lo.x - 0.0).abs() < 1e-9 && (hi.x - 0.4).abs() < 1e-9);
        assert!(
            (lo.y + 0.6).abs() < 1e-9 && hi.y.abs() < 1e-9,
            "kept at its origin"
        );
        assert!((hi.z - 0.4).abs() < 1e-9 && lo.z.abs() < 1e-9);
        assert_eq!(c.object.ifc.class, "IfcSanitaryTerminal");
        // the same file said to be in metres, whatever its header: 1000×
        let m = read_dxf(dxf.as_bytes(), "toilet", "user/toilet", 0.4, Some(1000.0)).unwrap();
        assert!((m.object.size().x - 400.0).abs() < 1e-6);
        assert!(read_dxf(
            b"0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n",
            "x",
            "user/x",
            1.0,
            None
        )
        .is_err());
    }
}
