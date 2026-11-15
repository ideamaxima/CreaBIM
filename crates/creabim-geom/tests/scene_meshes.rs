//! Every mesh in a built scene names only points it has: a triangle
//! running off the end crashed picking in 3D (todo 11, after a Mass was
//! pushed and pulled).

use creabim_core::{
    sample, EditMesh, Element, ElementId, ElementKind, FreeMesh, Model, Transaction,
};
use creabim_geom::build_scene;
use glam::DVec3;

fn sound(name: &str, model: &Model) {
    for item in build_scene(model) {
        let n = item.mesh.positions.len();
        let bad = item.mesh.indices.iter().find(|&&i| i as usize >= n);
        let kind = model.get(item.id).map(|e| format!("{:?}", e.kind));
        assert!(
            bad.is_none(),
            "{name}: {:?} {} has index {:?} past {n} points",
            item.id,
            kind.unwrap_or_default()
                .chars()
                .take(60)
                .collect::<String>(),
            bad
        );
        assert_eq!(item.mesh.indices.len() % 3, 0, "{name}: {:?}", item.id);
    }
}

#[test]
fn the_sample_models_build_sound_meshes() {
    sound("sample", &sample::sample_house());
    sound("layered", &sample::layered_house());
    sound("typed", &sample::typed_house());
    sound("furnished", &sample::furnished_house());
    sound("buried beam", &sample::sample_with_buried_beam().0);
    sound("garden", &creabim_core::garden::garden_house());
}

#[test]
fn a_pushed_and_pulled_mass_builds_a_sound_mesh() {
    let mut model = sample::sample_house();
    let level = model
        .levels()
        .find(|(_, l)| l.elevation == 0.0)
        .unwrap()
        .0
        .id;
    let mut mesh = EditMesh::cuboid(DVec3::new(2.0, 4.0, 3.0));
    // as the log: face 2 pulled out 1.4 m, then pushed and extruded
    let verts: Vec<u32> = mesh.faces[2].clone();
    let n = mesh.face_normal(2);
    mesh.translate_vertices(&verts, n * 1.4);
    mesh.extrude_face(2, 0.5);
    mesh.extrude_face(0, -0.3);
    let mass = Element::new(
        "Mass",
        ElementKind::FreeMesh(FreeMesh::new(Some(level), DVec3::new(-4.7, 8.0, 0.0), mesh)),
    );
    let _: ElementId = mass.id;
    Transaction::new("Add mass")
        .insert(mass)
        .apply(&mut model)
        .unwrap();
    sound("mass", &model);
}
