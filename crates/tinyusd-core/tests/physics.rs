#![cfg(feature = "physics")]

use tinyusd_core::{Axis, Dof, JointKind, MassInformation, OpenOptions, Physics, PhysicsOptions, ShapeKind, Stage};

fn load(si_units: bool) -> (Stage, Physics) {
    let path = format!("{}/../../tests/data/physics_full.usda", env!("CARGO_MANIFEST_DIR"));
    let mut stage = Stage::open(&path, &OpenOptions::default()).unwrap();
    let physics = stage
        .physics(&PhysicsOptions {
            si_units,
            ..PhysicsOptions::default()
        })
        .unwrap();
    (stage, physics)
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * b.abs().max(1e-12)
}

fn body<'a>(stage: &Stage, p: &'a Physics, path: &str) -> &'a tinyusd_core::RigidBody {
    p.rigid_bodies
        .iter()
        .find(|b| stage.path(b.prim).unwrap() == path)
        .unwrap()
}

#[test]
fn parses_every_object_kind() {
    let (stage, p) = load(false);
    assert_eq!((p.meters_per_unit, p.kilograms_per_unit), (0.01, 0.001));
    assert_eq!((p.scenes.len(), p.materials.len(), p.collision_groups.len()), (1, 1, 2));
    assert_eq!(
        (
            p.shapes.len(),
            p.rigid_bodies.len(),
            p.joints.len(),
            p.articulations.len()
        ),
        (8, 6, 5, 1)
    );
    let scene = &p.scenes[0];
    assert_eq!(scene.gravity_direction, [0.0, -1.0, 0.0]);
    assert!(close(f64::from(scene.gravity_magnitude), 981.0, 1e-6));

    let group = p
        .collision_groups
        .iter()
        .find(|g| g.merge_group_name == "robot")
        .unwrap();
    assert_eq!(group.merged_groups.len(), 2);
    assert!(!group.invert_filtered_groups);
    assert!(p.collision_groups.iter().any(|g| g.invert_filtered_groups));

    let art = &p.articulations[0];
    assert_eq!(stage.path(art.prim).unwrap(), "/World/Robot");
    assert_eq!(stage.path(art.roots[0]).unwrap(), "/World/Robot/Base");

    let link = body(&stage, &p, "/World/Robot/Link");
    assert_eq!(stage.path(link.filtered_collisions[0]).unwrap(), "/World/Robot/Base");
    assert_eq!(link.scale, [1.0, 2.0, 1.0]);

    let crate_shape = p.shapes.iter().find(|s| s.kind == ShapeKind::Mesh).unwrap();
    assert_eq!(
        (crate_shape.mesh_approximation.as_str(), crate_shape.mesh_scale),
        ("convexHull", [2.0, 1.0, 1.0])
    );
    let box_shape = p.shapes.iter().find(|s| s.kind == ShapeKind::Cube).unwrap();
    assert_eq!(box_shape.half_extents, [20.0, 10.0, 5.0]);
    let can = p.shapes.iter().find(|s| s.kind == ShapeKind::Cylinder).unwrap();
    assert_eq!((can.axis, can.radius, can.half_height), (Axis::Y, 3.0, 6.0));
    assert_eq!(stage.path(can.materials[0]).unwrap(), "/World/Rubber");
}

#[test]
fn joints_map_to_limits_and_drives() {
    let (stage, p) = load(false);
    let joint = |path: &str| p.joints.iter().find(|j| stage.path(j.prim).unwrap() == path).unwrap();
    let elbow = joint("/World/Robot/Elbow");
    assert_eq!(
        (elbow.kind, elbow.axis, elbow.type_name.as_str()),
        (JointKind::Revolute, Axis::Z, "PhysicsRevoluteJoint")
    );
    assert_eq!(
        (elbow.limits[0].dof, elbow.limits[0].lower, elbow.limits[0].upper),
        (Dof::RotZ, -45.0, 90.0)
    );
    assert_eq!(
        (elbow.drives[0].stiffness, elbow.drives[0].target_position),
        (50.0, 30.0)
    );
    assert_eq!(elbow.local_pos0, [0.0, 10.0, 0.0]);
    let slider = joint("/World/Slider");
    assert_eq!((slider.kind, slider.limits[0].dof), (JointKind::Prismatic, Dof::TransX));
    assert!(slider.drives[0].acceleration);
    let ball = joint("/World/Ball");
    assert_eq!(
        (ball.kind, ball.limits[0].lower, ball.limits[0].upper),
        (JointKind::Spherical, 30.0, 45.0)
    );
    let rope = joint("/World/Rope");
    assert_eq!(
        (rope.kind, rope.max_distance_enabled, rope.min_distance_enabled),
        (JointKind::Distance, true, false)
    );
    let generic = joint("/World/Generic");
    assert_eq!(generic.kind, JointKind::D6);
    let dofs: Vec<Dof> = generic.limits.iter().map(|l| l.dof).collect();
    assert_eq!(dofs, [Dof::TransX, Dof::RotY]);
    assert_eq!(generic.drives[0].dof, Dof::RotY);
}

#[test]
fn mass_properties_apply_usd_precedence() {
    let (mut stage, p) = load(false);
    let poses: std::collections::HashMap<_, _> =
        p.shapes.iter().map(|s| (s.prim, (s.local_pos, s.local_rot))).collect();
    let mut mass_of = |path: &str| {
        let body = stage.prim_at_path(path).unwrap().unwrap();
        stage
            .mass_properties(body, |collider| {
                poses.get(&collider).map(|(pos, rot)| MassInformation {
                    volume: 10.0,
                    inertia: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                    center_of_mass: [0.0; 3],
                    local_pos: pos.map(f64::from),
                    local_rot: rot.map(f64::from),
                })
            })
            .unwrap()
    };
    assert!(close(mass_of("/World/Props/Free").mass, 10.0, 1e-9));
    assert!(close(mass_of("/World/Props/Can").mass, 10.0 * 0.002, 1e-6));
    assert!(close(mass_of("/World/Robot/Link").mass, 2.0 * 10.0 * 0.001, 1e-6));
    let base = mass_of("/World/Robot/Base");
    assert_eq!(
        (base.mass, base.diagonal_inertia, base.center_of_mass),
        (5000.0, [10.0, 20.0, 30.0], [0.0, 5.0, 0.0])
    );
}

#[test]
fn si_conversion_scales_each_quantity() {
    let (stage, p) = load(true);
    assert!(p.si_units);
    assert!(close(f64::from(p.scenes[0].gravity_magnitude), 9.81, 1e-6));
    let link = body(&stage, &p, "/World/Robot/Link");
    assert!(close(f64::from(link.position[1]), 1.5, 1e-6));
    assert!(close(f64::from(link.linear_velocity[2]), 0.5, 1e-6));
    let rubber = &p.materials[0];
    assert!(close(f64::from(rubber.density), 2.0, 1e-6));
    let box_shape = p.shapes.iter().find(|s| s.kind == ShapeKind::Cube).unwrap();
    assert!(close(f64::from(box_shape.half_extents[0]), 0.2, 1e-6));
    let elbow = p.joints.iter().find(|j| j.kind == JointKind::Revolute).unwrap();
    assert_eq!((elbow.limits[0].lower, elbow.limits[0].upper), (-45.0, 90.0));
    assert!(close(f64::from(elbow.local_pos0[1]), 0.1, 1e-6));
    assert!(close(f64::from(elbow.drives[0].stiffness), 50.0 * 1e-7, 1e-6));
    let slider = p.joints.iter().find(|j| j.kind == JointKind::Prismatic).unwrap();
    assert!(close(f64::from(slider.limits[0].upper), 0.2, 1e-6));
    assert!(close(f64::from(slider.drives[0].target_position), 0.05, 1e-6));
    assert_eq!(slider.drives[0].stiffness, 10.0);
}

#[test]
fn include_and_exclude_limit_the_range() {
    let path = format!("{}/../../tests/data/physics_full.usda", env!("CARGO_MANIFEST_DIR"));
    let mut stage = Stage::open(&path, &OpenOptions::default()).unwrap();
    let props = stage.prim_at_path("/World/Props").unwrap().unwrap();
    let crate_prim = stage.prim_at_path("/World/Props/Crate").unwrap().unwrap();
    let p = stage
        .physics(&PhysicsOptions {
            include: vec![props],
            exclude: vec![crate_prim],
            ..PhysicsOptions::default()
        })
        .unwrap();
    let bodies: Vec<&str> = p.rigid_bodies.iter().map(|b| stage.path(b.prim).unwrap()).collect();
    assert_eq!(bodies, ["/World/Props/Can", "/World/Props/Cone", "/World/Props/Free"]);
}
