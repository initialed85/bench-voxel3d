use bevy::prelude::*;
use voxel_common::World;

pub type Cell = [i32; 3];

/// Amanatides & Woo voxel DDA. Returns the hit cell, the cell the ray came from
/// (where a block would be placed), and the distance to the face it entered.
pub fn raycast(world: &World, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<(Cell, Cell, f32)> {
    let mut cell = [
        origin.x.floor() as i32,
        origin.y.floor() as i32,
        origin.z.floor() as i32,
    ];
    let step = [
        if dir.x > 0.0 { 1 } else { -1 },
        if dir.y > 0.0 { 1 } else { -1 },
        if dir.z > 0.0 { 1 } else { -1 },
    ];
    let delta = [
        if dir.x != 0.0 {
            (1.0 / dir.x).abs()
        } else {
            f32::INFINITY
        },
        if dir.y != 0.0 {
            (1.0 / dir.y).abs()
        } else {
            f32::INFINITY
        },
        if dir.z != 0.0 {
            (1.0 / dir.z).abs()
        } else {
            f32::INFINITY
        },
    ];
    let mut t_max = [
        if dir.x > 0.0 {
            (cell[0] as f32 + 1.0 - origin.x) * delta[0]
        } else if dir.x < 0.0 {
            (origin.x - cell[0] as f32) * delta[0]
        } else {
            f32::INFINITY
        },
        if dir.y > 0.0 {
            (cell[1] as f32 + 1.0 - origin.y) * delta[1]
        } else if dir.y < 0.0 {
            (origin.y - cell[1] as f32) * delta[1]
        } else {
            f32::INFINITY
        },
        if dir.z > 0.0 {
            (cell[2] as f32 + 1.0 - origin.z) * delta[2]
        } else if dir.z < 0.0 {
            (origin.z - cell[2] as f32) * delta[2]
        } else {
            f32::INFINITY
        },
    ];

    let mut prev = cell;
    let mut t = 0.0;

    while t <= max_dist {
        if world.solid(cell[0], cell[1], cell[2]) {
            return Some((cell, prev, t));
        }
        prev = cell;
        if t_max[0] < t_max[1] && t_max[0] < t_max[2] {
            cell[0] += step[0];
            t = t_max[0];
            t_max[0] += delta[0];
        } else if t_max[1] < t_max[2] {
            cell[1] += step[1];
            t = t_max[1];
            t_max[1] += delta[1];
        } else {
            cell[2] += step[2];
            t = t_max[2];
            t_max[2] += delta[2];
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxel_common::{block, WORLD_X, WORLD_Y, WORLD_Z};

    fn flat_world() -> World {
        let mut w = World::generate(1);
        for x in 0..WORLD_X as i32 {
            for y in 0..WORLD_Y as i32 {
                for z in 0..WORLD_Z as i32 {
                    w.set(x, y, z, block::AIR);
                }
            }
        }
        w.set(10, 20, 10, block::STONE);
        w
    }

    #[test]
    fn ray_hits_block_below_origin() {
        let w = flat_world();
        let (cell, prev, _) = raycast(
            &w,
            Vec3::new(10.5, 25.0, 10.5),
            Vec3::new(0.0, -1.0, 0.0),
            20.0,
        )
        .unwrap();
        assert_eq!(cell, [10, 20, 10]);
        assert_eq!(prev, [10, 21, 10]);
    }

    #[test]
    fn ray_misses_empty_space() {
        let w = flat_world();
        assert!(raycast(
            &w,
            Vec3::new(30.5, 40.0, 30.5),
            Vec3::new(0.0, 1.0, 0.0),
            10.0
        )
        .is_none());
    }

    #[test]
    fn ray_stops_at_max_distance() {
        let w = flat_world();
        assert!(raycast(
            &w,
            Vec3::new(10.5, 24.0, 10.5),
            Vec3::new(0.0, -1.0, 0.0),
            2.0
        )
        .is_none());
    }

    #[test]
    fn ray_hits_side_and_reports_placement_cell() {
        let w = flat_world();
        let (cell, prev, _) = raycast(
            &w,
            Vec3::new(6.5, 20.5, 10.5),
            Vec3::new(1.0, 0.0, 0.0),
            20.0,
        )
        .unwrap();
        assert_eq!(cell, [10, 20, 10]);
        assert_eq!(prev, [9, 20, 10]);
    }

    #[test]
    fn hit_distance_is_positive_and_on_the_face() {
        let w = flat_world();
        let (cell, _, dist) = raycast(
            &w,
            Vec3::new(10.5, 25.0, 10.5),
            Vec3::new(0.0, -1.0, 0.0),
            20.0,
        )
        .unwrap();
        assert!(
            dist > 0.0,
            "distance must be forward along the ray, got {dist}"
        );
        let p = Vec3::new(10.5, 25.0, 10.5) + Vec3::new(0.0, -1.0, 0.0) * dist;
        assert!((p.y - (cell[1] as f32 + 1.0)).abs() < 1e-4);
        assert!((dist - 4.0).abs() < 1e-4);
    }

    #[test]
    fn negative_axis_distance_is_positive() {
        let w = flat_world();
        let (cell, _, dist) = raycast(
            &w,
            Vec3::new(10.5, 20.5, 14.5),
            Vec3::new(0.0, 0.0, -1.0),
            20.0,
        )
        .unwrap();
        assert_eq!(cell, [10, 20, 10]);
        assert!(
            (dist - 3.5).abs() < 1e-4,
            "expected the z=11 face, got {dist}"
        );
    }
}
