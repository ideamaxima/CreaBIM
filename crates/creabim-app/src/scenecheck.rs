//! A check of the 3D scene each time it is built: a mesh with a triangle
//! naming a point it doesn't have would crash whatever reads it (picking
//! under the pointer, snaps, sections), so those triangles are dropped and
//! the element is named in the activity log, to find where it came from.

use creabim_core::Model;
use creabim_geom::SceneItem;

/// Drop the bad triangles of every item in `scene`, logging each element
/// that had any.
pub fn clean(model: &Model, scene: &mut [SceneItem]) {
    for item in scene.iter_mut() {
        let points = item.mesh.positions.len();
        let (dropped, max) = item.mesh.drop_bad_triangles();
        if dropped == 0 {
            continue;
        }
        let (kind, name) = model
            .get(item.id)
            .map_or(("?".to_string(), String::new()), |e| {
                let kind = format!("{:?}", e.kind);
                let kind = kind
                    .split(['(', ' ', '{'])
                    .next()
                    .unwrap_or("?")
                    .to_string();
                (kind, e.name.clone())
            });
        crate::actlog::line(format!(
            "BAD MESH: {kind} \"{name}\" {:?}: {dropped} triangles dropped ({points} points, largest index {max})",
            item.id
        ));
    }
}
