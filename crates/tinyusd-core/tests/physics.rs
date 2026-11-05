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

#[cfg(feature = "write")]
mod write {
    use tinyusd_core::{
        Axis, BodyMass, CollisionGroup, Dof, Joint, JointDrive, JointKind, JointLimit, OpenOptions, Physics,
        PhysicsMaterial, PhysicsOptions, PhysicsScene, PhysicsWriteOptions, RigidBody, Shape, ShapeKind, Stage,
    };

    fn data(name: &str) -> String {
        format!("{}/../../tests/data/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    fn assert_close(a: &str, b: &str) {
        fn split(s: &str) -> (String, Vec<f64>) {
            let (mut text, mut numbers, mut current) = (String::new(), Vec::new(), String::new());
            let flush = |current: &mut String, text: &mut String, numbers: &mut Vec<f64>| {
                if current.is_empty() {
                    return;
                }
                match current.parse::<f64>() {
                    Ok(n) => {
                        numbers.push(n);
                        text.push('#');
                    }
                    Err(_) => text.push_str(current),
                }
                current.clear();
            };
            for c in s.chars() {
                if c.is_ascii_digit() || c == '.' || c == 'e' || (c == '-' && current.is_empty()) {
                    current.push(c);
                } else {
                    flush(&mut current, &mut text, &mut numbers);
                    text.push(c);
                }
            }
            flush(&mut current, &mut text, &mut numbers);
            (text, numbers)
        }
        let ((ta, na), (tb, nb)) = (split(a), split(b));
        assert_eq!(ta, tb, "structure differs:\n{a}\n{b}");
        for (x, y) in na.iter().zip(&nb) {
            let tol = 1e-4 * x.abs().max(y.abs()).max(1.0);
            assert!((x - y).abs() <= tol, "{x} != {y} in\n{a}\n{b}");
        }
    }

    fn without_diagnostics(mut p: Physics) -> Physics {
        p.diagnostics.clear();
        p
    }

    #[test]
    fn parsed_physics_writes_back_unchanged() {
        for si_units in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let result = dir.path().join("result.usda").to_string_lossy().into_owned();
            let mut stage = Stage::create_new(&result, &OpenOptions::default()).unwrap();
            stage.add_sublayer(&data("physics_full.usda"), false).unwrap();
            stage.set_meters_per_unit(0.01).unwrap();
            stage.set_kilograms_per_unit(0.001).unwrap();
            stage.set_up_axis("Y").unwrap();
            let options = PhysicsOptions {
                si_units,
                ..PhysicsOptions::default()
            };
            let before = stage.physics(&options).unwrap();
            assert!(!before.joints.is_empty() && !before.shapes.is_empty());
            stage
                .write_physics(&before, &PhysicsWriteOptions { poses: true })
                .unwrap();
            let after = stage.physics(&options).unwrap();
            assert_close(
                &format!("{:?}", without_diagnostics(before.clone())),
                &format!("{:?}", without_diagnostics(after)),
            );
            let text = String::from_utf8(
                stage
                    .export_bytes(Some(&stage.edit_target()), tinyusd_core::LayerFormat::Usda)
                    .unwrap(),
            )
            .unwrap();
            assert!(text.contains("physics:lowerLimit") || text.contains("limit:"), "{text}");
        }
    }

    #[test]
    fn physics_built_from_scratch() {
        let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
        stage.set_meters_per_unit(1.0).unwrap();
        stage.set_kilograms_per_unit(1.0).unwrap();
        stage.set_up_axis("Z").unwrap();
        let scene = stage.define_prim("/World/Scene", "").unwrap();
        let rubber = stage.define_prim("/World/Rubber", "Material").unwrap();
        let group = stage.define_prim("/World/Group", "").unwrap();
        let base = stage.define_prim("/World/Base", "Xform").unwrap();
        let base_box = stage.define_prim("/World/Base/Box", "Cube").unwrap();
        let arm = stage.define_prim("/World/Arm", "Xform").unwrap();
        let arm_mesh = stage.define_prim("/World/Arm/Mesh", "Mesh").unwrap();
        stage
            .set_mesh(
                arm_mesh,
                &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                &[3, 3],
                &[0, 1, 2, 0, 2, 3],
                None,
            )
            .unwrap();

        let mut physics = Physics {
            scenes: vec![PhysicsScene {
                gravity_direction: [0.0, 0.0, -1.0],
                gravity_magnitude: 9.81,
                ..PhysicsScene::new(scene)
            }],
            collision_groups: vec![CollisionGroup {
                merge_group_name: "robot".to_owned(),
                ..CollisionGroup::new(group)
            }],
            materials: vec![PhysicsMaterial {
                static_friction: 0.9,
                dynamic_friction: 0.6,
                restitution: 0.25,
                density: 1200.0,
                ..PhysicsMaterial::new(rubber)
            }],
            shapes: vec![
                Shape {
                    materials: vec![rubber],
                    collision_groups: vec![group],
                    ..Shape::new(base_box)
                },
                Shape {
                    kind: ShapeKind::Mesh,
                    mesh_approximation: "convexHull".to_owned(),
                    filtered_collisions: vec![base_box],
                    ..Shape::new(arm_mesh)
                },
            ],
            rigid_bodies: vec![
                RigidBody {
                    position: [0.0, 0.0, 1.0],
                    kinematic: true,
                    ..RigidBody::new(base)
                },
                RigidBody {
                    position: [0.0, 0.0, 2.0],
                    rotation: [0.70710677, 0.0, 0.0, 0.70710677],
                    linear_velocity: [0.0, 0.0, -1.0],
                    ..RigidBody::new(arm)
                },
            ],
            ..Physics::default()
        };
        let mut joint = |path: &str, kind: JointKind| {
            let prim = stage.define_prim(path, "").unwrap();
            Joint {
                body0: Some(base),
                body1: Some(arm),
                local_pos0: [0.0, 0.0, 0.5],
                local_pos1: [0.0, 0.0, -0.5],
                ..Joint::new(prim, kind)
            }
        };
        let mut hinge = joint("/World/Hinge", JointKind::Revolute);
        hinge.axis = Axis::Y;
        hinge.limits.push(JointLimit {
            dof: Dof::RotY,
            enabled: true,
            lower: -45.0,
            upper: 90.0,
        });
        hinge.drives.push(JointDrive {
            dof: Dof::RotY,
            target_position: 10.0,
            target_velocity: 0.0,
            force_limit: 100.0,
            stiffness: 50.0,
            damping: 5.0,
            acceleration: false,
        });
        let mut slider = joint("/World/Slider", JointKind::Prismatic);
        slider.limits.push(JointLimit {
            dof: Dof::TransX,
            enabled: false,
            lower: 0.0,
            upper: 0.0,
        });
        let mut ball = joint("/World/Ball", JointKind::Spherical);
        ball.limits.push(JointLimit {
            dof: Dof::RotX,
            enabled: true,
            lower: 30.0,
            upper: 20.0,
        });
        let mut rope = joint("/World/Rope", JointKind::Distance);
        rope.max_distance_enabled = true;
        rope.limits.push(JointLimit {
            dof: Dof::Distance,
            enabled: false,
            lower: 0.0,
            upper: 3.0,
        });
        let mut d6 = joint("/World/D6", JointKind::D6);
        d6.limits = vec![
            JointLimit {
                dof: Dof::TransX,
                enabled: true,
                lower: 1.0,
                upper: -1.0,
            },
            JointLimit {
                dof: Dof::RotZ,
                enabled: true,
                lower: -10.0,
                upper: 10.0,
            },
        ];
        d6.drives.push(JointDrive {
            dof: Dof::RotZ,
            target_position: 0.0,
            target_velocity: 1.0,
            force_limit: f32::INFINITY,
            stiffness: 0.0,
            damping: 2.0,
            acceleration: true,
        });
        let fixed = joint("/World/Weld", JointKind::Fixed);
        physics.joints = vec![hinge, slider, ball, rope, d6, fixed];

        stage
            .write_physics(&physics, &PhysicsWriteOptions { poses: true })
            .unwrap();
        stage
            .write_body_mass(
                arm,
                &BodyMass {
                    mass: 3.0,
                    diagonal_inertia: [1.0, 2.0, 3.0],
                    center_of_mass: [0.0, 0.0, 0.1],
                    principal_axes: [1.0, 0.0, 0.0, 0.0],
                },
            )
            .unwrap();

        let parsed = stage.physics(&PhysicsOptions::default()).unwrap();
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        assert_eq!(parsed.scenes[0].gravity_magnitude, 9.81);
        assert_eq!(parsed.materials, physics.materials);
        assert_eq!(parsed.collision_groups[0].merge_group_name, "robot");
        let shape = |prim| parsed.shapes.iter().find(|s| s.prim == prim).unwrap();
        assert_eq!(shape(base_box).materials, [rubber]);
        assert_eq!(shape(base_box).collision_groups, [group]);
        assert_eq!(shape(base_box).rigid_body, Some(base));
        assert_eq!(shape(arm_mesh).mesh_approximation, "convexHull");
        assert_eq!(shape(arm_mesh).filtered_collisions, [base_box]);
        for (written, read) in physics.rigid_bodies.iter().zip(&parsed.rigid_bodies) {
            assert_close(
                &format!("{written:?}"),
                &format!(
                    "{:?}",
                    RigidBody {
                        collisions: Vec::new(),
                        ..read.clone()
                    }
                ),
            );
        }
        for written in &physics.joints {
            let read = parsed.joints.iter().find(|j| j.prim == written.prim).unwrap();
            let mut expected = written.clone();
            expected.rel0 = written.body0;
            expected.rel1 = written.body1;
            expected.type_name = read.type_name.clone();
            if expected.kind == JointKind::Revolute || expected.kind == JointKind::Prismatic {
                let l = &mut expected.limits[0];
                if !l.enabled {
                    (l.lower, l.upper) = (f32::NEG_INFINITY, f32::INFINITY);
                }
            }
            if expected.kind == JointKind::Distance {
                expected.limits[0].lower = -1.0;
            }
            if !matches!(
                expected.kind,
                JointKind::Revolute | JointKind::Prismatic | JointKind::Spherical
            ) {
                expected.axis = read.axis;
            }
            assert_close(&format!("{expected:?}"), &format!("{read:?}"));
        }
        let mass = stage.mass_properties(arm, |_| None).unwrap();
        assert!((mass.mass - 3.0).abs() < 1e-6 && (mass.diagonal_inertia[2] - 3.0).abs() < 1e-6);
    }
}
