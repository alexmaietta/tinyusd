use serde_json::{Map, Value as Json, json};
use tinyusd_core::{BaseType, Interpolation, OpenOptions, PrimFlags, Specifier, Stage, TraverseFlags, Value};

fn numbers(v: &Value) -> Option<Vec<f64>> {
    let b = v.bytes();
    let out = match v.base_type() {
        BaseType::Bool | BaseType::UChar => b.iter().map(|&x| f64::from(x)).collect(),
        BaseType::Int | BaseType::Int2 | BaseType::Int3 | BaseType::Int4 => {
            v.as_slice::<i32>()?.iter().map(|&x| f64::from(x)).collect()
        }
        BaseType::UInt => v.as_slice::<u32>()?.iter().map(|&x| f64::from(x)).collect(),
        BaseType::Int64 => v.as_slice::<i64>()?.iter().map(|&x| x as f64).collect(),
        BaseType::UInt64 => v.as_slice::<u64>()?.iter().map(|&x| x as f64).collect(),
        BaseType::Half | BaseType::Half2 | BaseType::Half3 | BaseType::Half4 | BaseType::Quath => v
            .as_slice::<u16>()?
            .iter()
            .map(|&x| f64::from(half::f16::from_bits(x)))
            .collect(),
        BaseType::Float | BaseType::Float2 | BaseType::Float3 | BaseType::Float4 | BaseType::Quatf => {
            v.as_slice::<f32>()?.iter().map(|&x| f64::from(x)).collect()
        }
        _ => v.as_slice::<f64>()?.to_vec(),
    };
    Some(out)
}

fn encode(v: &Value) -> Json {
    match v.base_type() {
        BaseType::Empty => Json::Null,
        BaseType::Unsupported => json!("<unsupported>"),
        BaseType::Asset => json!({ "authored": v.strings(), "resolved": v.resolved_paths() }),
        b if b.is_string_like() => json!(v.strings()),
        _ => json!(numbers(v)),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let proxies = args.iter().any(|a| a == "--instance-proxies");
    let helpers = args.iter().any(|a| a == "--helpers");
    let physics = args.iter().any(|a| a == "--physics");
    let time: Option<f64> = args
        .iter()
        .position(|a| a == "--time")
        .map(|i| args[i + 1].parse().expect("--time value"));
    let path = args
        .iter()
        .enumerate()
        .find(|&(i, a)| !a.starts_with("--") && (i == 0 || (args[i - 1] != "--time" && args[i - 1] != "--schemas")))
        .map(|(_, a)| a.clone())
        .expect("usage: dump [--instance-proxies] [--helpers] [--time T] <file>");
    let flags = TraverseFlags::INCLUDE_INACTIVE
        | TraverseFlags::INCLUDE_UNLOADED
        | TraverseFlags::INCLUDE_UNDEFINED
        | TraverseFlags::INCLUDE_ABSTRACT
        | if proxies {
            TraverseFlags::INSTANCE_PROXIES
        } else {
            TraverseFlags::DEFAULT
        };
    let schema_paths: Vec<std::path::PathBuf> = args
        .iter()
        .enumerate()
        .filter(|(i, _)| *i > 0 && args[i - 1] == "--schemas")
        .map(|(_, a)| a.into())
        .collect();
    let options = OpenOptions {
        schema_paths,
        ..OpenOptions::default()
    };
    let mut stage = Stage::open(&path, &options).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });
    let mut v = Value::new();
    let mut metadata = Map::new();
    for key in [
        "upAxis",
        "metersPerUnit",
        "defaultPrim",
        "startTimeCode",
        "endTimeCode",
        "timeCodesPerSecond",
    ] {
        if stage.has_authored_stage_metadata(key).unwrap() && stage.stage_metadata(key, &mut v).unwrap() {
            metadata.insert(key.to_owned(), encode(&v));
        }
    }
    let mut prims = Vec::new();
    for prim in stage.traverse(flags).unwrap() {
        let mut attributes = Map::new();
        for name in stage.attribute_names(prim, true).unwrap() {
            let info = stage.attribute_info(prim, &name).unwrap().unwrap_or_default();
            let default = if stage.get(prim, &name, None, &mut v).unwrap() {
                encode(&v)
            } else {
                Json::Null
            };
            let mut samples = Map::new();
            for t in stage.time_samples(prim, &name).unwrap() {
                let value = if stage.get(prim, &name, Some(t), &mut v).unwrap() {
                    encode(&v)
                } else {
                    Json::Null
                };
                samples.insert(format!("{t}"), value);
            }
            let connections = stage.connections(prim, &name).unwrap();
            attributes.insert(
                name,
                json!({ "type": info.type_name, "default": default, "samples": samples, "connections": connections }),
            );
        }
        let mut relationships = Map::new();
        for name in stage.relationship_names(prim, true).unwrap() {
            relationships.insert(name.clone(), json!(stage.targets(prim, &name, false).unwrap()));
        }
        let mut applied = stage.applied_schemas(prim).unwrap();
        applied.sort();
        let mut resolved = Json::Null;
        if helpers {
            let (local, resets) = stage.local_transform(prim, time).unwrap();
            let mut primvars = Map::new();
            for name in stage.primvar_names(prim).unwrap() {
                if let Some(info) = stage.primvar(prim, &name, time, &mut v).unwrap() {
                    let interpolation = match info.interpolation {
                        Interpolation::Constant => "constant",
                        Interpolation::Uniform => "uniform",
                        Interpolation::Varying => "varying",
                        Interpolation::Vertex => "vertex",
                        Interpolation::FaceVarying => "faceVarying",
                    };
                    primvars.insert(
                        name,
                        json!({
                            "value": encode(&v),
                            "interpolation": interpolation,
                            "element_size": info.element_size,
                            "indexed": info.indexed,
                            "source": stage.path(info.source).unwrap(),
                        }),
                    );
                }
            }
            let mut materials = Map::new();
            for purpose in ["", "full", "preview"] {
                let bound = stage.bound_material(prim, purpose).unwrap();
                materials.insert(
                    purpose.to_owned(),
                    json!(bound.map(|b| stage.path(b.material).unwrap().to_owned())),
                );
            }
            let instances = stage.instancer_instances(prim, time, true, true).unwrap().map(|i| {
                json!({
                    "prototypes": i.prototypes.iter().map(|p| stage.path(*p).unwrap().to_owned()).collect::<Vec<_>>(),
                    "proto_indices": i.proto_indices,
                    "ids": i.ids,
                    "transforms": i.transforms.iter().flatten().collect::<Vec<_>>(),
                })
            });
            let skin = stage.skin_binding(prim, time).unwrap().map(|b| {
                json!({
                    "skeleton": stage.path(b.skeleton).unwrap(),
                    "joint_indices": b.joint_indices,
                    "joint_weights": b.joint_weights,
                    "influences_per_point": b.influences_per_point,
                    "rigid": b.rigid,
                    "geom_bind_transform": b.geom_bind_transform,
                    "blend_shapes": b.blend_shapes,
                    "blend_shape_weights": b.blend_shape_weights,
                })
            });
            let skinned = stage
                .skinned_points(prim, time)
                .unwrap()
                .map(|p| p.into_iter().flatten().collect::<Vec<_>>());
            let pose = if stage.is_a(prim, "Skeleton").unwrap() {
                stage.skeleton_pose(prim, time).unwrap().map(|p| {
                    json!({
                        "joints": p.joints,
                        "parents": p.parents,
                        "bind": p.bind.iter().flatten().collect::<Vec<_>>(),
                        "rest_local": p.rest_local.iter().flatten().collect::<Vec<_>>(),
                        "local": p.local.iter().flatten().collect::<Vec<_>>(),
                        "skel": p.skel.iter().flatten().collect::<Vec<_>>(),
                        "skinning": p.skinning.iter().flatten().collect::<Vec<_>>(),
                    })
                })
            } else {
                None
            };
            resolved = json!({
                "instances": instances,
                "skin": skin,
                "skinned_points": skinned,
                "pose": pose,
                "world": stage.world_transform(prim, time).unwrap(),
                "local": local,
                "resets": resets,
                "primvars": primvars,
                "materials": materials,
            });
        }
        prims.push(json!({
            "resolved": resolved,
            "path": stage.path(prim).unwrap(),
            "type": stage.type_name(prim).unwrap(),
            "specifier": match stage.specifier(prim).unwrap() {
                Specifier::Def => "def",
                Specifier::Over => "over",
                Specifier::Class => "class",
            },
            "kind": stage.kind(prim).unwrap(),
            "active": stage.flags(prim).unwrap().contains(PrimFlags::ACTIVE),
            "instance": stage.flags(prim).unwrap().contains(PrimFlags::INSTANCE),
            "applied": applied,
            "attributes": attributes,
            "relationships": relationships,
        }));
    }
    let physics_json = if physics { dump_physics(&mut stage) } else { Json::Null };
    println!(
        "{}",
        json!({ "metadata": metadata, "prims": prims, "physics": physics_json })
    );
}

fn paths(stage: &Stage, prims: &[tinyusd_core::Prim]) -> Vec<String> {
    prims.iter().map(|&p| stage.path(p).unwrap().to_owned()).collect()
}

fn opt(stage: &Stage, prim: Option<tinyusd_core::Prim>) -> Json {
    prim.map_or(Json::Null, |p| json!(stage.path(p).unwrap()))
}

fn dump_physics(stage: &mut Stage) -> Json {
    use tinyusd_core::{Axis, MassInformation, PhysicsOptions, ShapeKind};
    let p = stage.physics(&PhysicsOptions::default()).unwrap();
    let poses: std::collections::HashMap<tinyusd_core::Prim, ([f32; 3], [f32; 4])> =
        p.shapes.iter().map(|s| (s.prim, (s.local_pos, s.local_rot))).collect();
    let axis = |a: Axis| format!("{a:?}");
    let mut shapes = Map::new();
    for s in &p.shapes {
        let geometry = match s.kind {
            ShapeKind::Sphere => json!({ "radius": s.radius }),
            ShapeKind::Cube => json!({ "half_extents": s.half_extents }),
            ShapeKind::Capsule | ShapeKind::Cylinder | ShapeKind::Cone => {
                json!({ "radius": s.radius, "half_height": s.half_height, "axis": axis(s.axis) })
            }
            ShapeKind::Plane => json!({ "axis": axis(s.axis) }),
            ShapeKind::Mesh => {
                json!({ "approximation": s.mesh_approximation, "scale": s.mesh_scale, "double_sided": s.double_sided })
            }
            _ => json!({}),
        };
        shapes.insert(
            stage.path(s.prim).unwrap().to_owned(),
            json!({
                "kind": format!("{:?}", s.kind),
                "rigid_body": opt(stage, s.rigid_body),
                "local_pos": s.local_pos, "local_rot": s.local_rot, "local_scale": s.local_scale,
                "materials": paths(stage, &s.materials),
                "filtered_collisions": paths(stage, &s.filtered_collisions),
                "collision_groups": paths(stage, &s.collision_groups),
                "collision_enabled": s.collision_enabled,
                "geometry": geometry,
            }),
        );
    }
    let mut bodies = Map::new();
    for b in &p.rigid_bodies {
        let m = stage
            .mass_properties(b.prim, |collider| {
                poses.get(&collider).map(|(pos, rot)| MassInformation {
                    volume: 2.0,
                    inertia: [1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0],
                    center_of_mass: [0.1, 0.0, 0.0],
                    local_pos: pos.map(f64::from),
                    local_rot: rot.map(f64::from),
                })
            })
            .unwrap();
        let mass = json!({
            "mass": m.mass, "diagonal_inertia": m.diagonal_inertia,
            "center_of_mass": m.center_of_mass, "principal_axes": m.principal_axes,
        });
        bodies.insert(
            stage.path(b.prim).unwrap().to_owned(),
            json!({
                "collisions": paths(stage, &b.collisions),
                "filtered_collisions": paths(stage, &b.filtered_collisions),
                "position": b.position, "rotation": b.rotation, "scale": b.scale,
                "enabled": b.enabled, "kinematic": b.kinematic, "starts_asleep": b.starts_asleep,
                "linear_velocity": b.linear_velocity, "angular_velocity": b.angular_velocity,
                "mass": mass,
            }),
        );
    }
    let mut joints = Map::new();
    for j in &p.joints {
        let limits: Vec<Json> = j
            .limits
            .iter()
            .map(|l| json!([format!("{:?}", l.dof), l.enabled, l.lower, l.upper]))
            .collect();
        let drives: Vec<Json> = j
            .drives
            .iter()
            .map(|d| {
                json!([
                    format!("{:?}", d.dof),
                    d.target_position,
                    d.target_velocity,
                    d.force_limit,
                    d.stiffness,
                    d.damping,
                    d.acceleration
                ])
            })
            .collect();
        joints.insert(
            stage.path(j.prim).unwrap().to_owned(),
            json!({
                "kind": format!("{:?}", j.kind),
                "body0": opt(stage, j.body0), "body1": opt(stage, j.body1),
                "rel0": opt(stage, j.rel0), "rel1": opt(stage, j.rel1),
                "local_pos0": j.local_pos0, "local_rot0": j.local_rot0,
                "local_pos1": j.local_pos1, "local_rot1": j.local_rot1,
                "enabled": j.enabled, "collision_enabled": j.collision_enabled,
                "break_force": j.break_force, "break_torque": j.break_torque,
                "exclude_from_articulation": j.exclude_from_articulation,
                "axis": axis(j.axis), "limits": limits, "drives": drives,
                "min_enabled": j.min_distance_enabled, "max_enabled": j.max_distance_enabled,
            }),
        );
    }
    json!({
        "scenes": p.scenes.iter().map(|s| json!({ "path": stage.path(s.prim).unwrap(), "gravity_direction": s.gravity_direction, "gravity_magnitude": s.gravity_magnitude })).collect::<Vec<_>>(),
        "collision_groups": p.collision_groups.iter().map(|g| json!({
            "path": stage.path(g.prim).unwrap(), "invert": g.invert_filtered_groups,
            "filtered": paths(stage, &g.filtered_groups), "merge": g.merge_group_name, "merged": paths(stage, &g.merged_groups),
        })).collect::<Vec<_>>(),
        "materials": p.materials.iter().map(|m| json!({
            "path": stage.path(m.prim).unwrap(), "static_friction": m.static_friction,
            "dynamic_friction": m.dynamic_friction, "restitution": m.restitution, "density": m.density,
        })).collect::<Vec<_>>(),
        "articulations": p.articulations.iter().map(|a| json!({
            "path": stage.path(a.prim).unwrap(), "roots": paths(stage, &a.roots),
            "joints": paths(stage, &a.joints),
            "bodies": a.bodies.iter().map(|b| opt(stage, *b)).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "shapes": shapes,
        "rigid_bodies": bodies,
        "joints": joints,
    })
}
