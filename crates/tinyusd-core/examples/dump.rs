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
    let time: Option<f64> = args
        .iter()
        .position(|a| a == "--time")
        .map(|i| args[i + 1].parse().expect("--time value"));
    let path = args
        .iter()
        .enumerate()
        .find(|&(i, a)| !a.starts_with("--") && (i == 0 || args[i - 1] != "--time"))
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
    let mut stage = Stage::open(&path, &OpenOptions::default()).unwrap_or_else(|e| {
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
    println!("{}", json!({ "metadata": metadata, "prims": prims }));
}
