use std::io;
use std::rc::Rc;

use tinyusd_core::{AssetResolver, OpenOptions, Stage, TraverseFlags, Value};

struct OneFile {
    path: String,
    bytes: Vec<u8>,
}

impl AssetResolver for OneFile {
    fn resolve(&self, asset_path: &str, _anchor: Option<&str>) -> Option<String> {
        (asset_path == self.path).then(|| self.path.clone())
    }

    fn read(&self, resolved_path: &str) -> io::Result<Vec<u8>> {
        if resolved_path == self.path {
            Ok(self.bytes.clone())
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, resolved_path.to_owned()))
        }
    }
}

pub fn exercise(extension: &str, data: &[u8]) {
    let path = format!("mem://fuzz.{extension}");
    let resolver = Rc::new(OneFile {
        path: path.clone(),
        bytes: data.to_vec(),
    });
    let Ok(mut stage) = Stage::open(
        &path,
        &OpenOptions {
            load_payloads: true,
            resolver: Some(resolver),
            ..OpenOptions::default()
        },
    ) else {
        return;
    };
    let flags = TraverseFlags::INCLUDE_INACTIVE
        | TraverseFlags::INCLUDE_UNLOADED
        | TraverseFlags::INCLUDE_UNDEFINED
        | TraverseFlags::INCLUDE_ABSTRACT
        | TraverseFlags::INSTANCE_PROXIES;
    let Ok(prims) = stage.traverse(flags) else {
        return;
    };
    let mut value = Value::new();
    for &prim in prims.iter().take(256) {
        let _ = stage.type_name(prim);
        let _ = stage.applied_schemas(prim);
        let _ = stage.flags(prim);
        if let Ok(names) = stage.attribute_names(prim, false) {
            for name in names.iter().take(64) {
                let _ = stage.get(prim, name, None, &mut value);
                let _ = stage.attribute_info(prim, name);
                if let Ok(times) = stage.time_samples(prim, name)
                    && let Some(&t) = times.first()
                {
                    let _ = stage.get(prim, name, Some(t + 0.5), &mut value);
                }
                let _ = stage.connections(prim, name);
            }
        }
        if let Ok(names) = stage.relationship_names(prim, false) {
            for name in names.iter().take(16) {
                let _ = stage.targets(prim, name, true);
            }
        }
        let _ = stage.world_transform(prim, None);
        let _ = stage.local_transform(prim, Some(1.0));
        if let Ok(names) = stage.primvar_names(prim) {
            for name in names.iter().take(16) {
                let _ = stage.primvar(prim, name, None, &mut value);
            }
        }
        let _ = stage.bound_material(prim, "");
    }
    let _ = stage.prototypes();
    let _ = stage.composition_errors();
}

struct Ops<'a> {
    data: &'a [u8],
}

impl Ops<'_> {
    fn byte(&mut self) -> Option<u8> {
        let (&b, rest) = self.data.split_first()?;
        self.data = rest;
        Some(b)
    }

    fn pick<'p, T>(&mut self, pool: &'p [T]) -> Option<&'p T> {
        Some(&pool[usize::from(self.byte()?) % pool.len()])
    }

    fn f64(&mut self) -> Option<f64> {
        Some(f64::from(i16::from_le_bytes([self.byte()?, self.byte()?])) / 16.0)
    }
}

const PATHS: [&str; 8] = ["/A", "/A/B", "/A/B/C", "/D", "/D/E", "/A/F", "/G", "/D/E/H"];
const TYPES: [&str; 8] = [
    "",
    "Xform",
    "Mesh",
    "Cube",
    "Sphere",
    "Scope",
    "PhysicsRevoluteJoint",
    "Material",
];
const ATTRS: [(&str, &str); 8] = [
    ("points", "point3f[]"),
    ("size", "double"),
    ("my:f", "float"),
    ("my:tokens", "token[]"),
    ("my:m", "matrix4d"),
    ("my:q", "quatf"),
    ("my:s", "string"),
    ("xformOp:translate", "double3"),
];
const APIS: [&str; 5] = [
    "PhysicsRigidBodyAPI",
    "PhysicsCollisionAPI",
    "PhysicsLimitAPI:rotX",
    "MaterialBindingAPI",
    "PhysicsMassAPI",
];
const NAMES: [&str; 4] = ["N1", "N2", "Renamed", "B"];

fn value(ops: &mut Ops<'_>) -> Option<Value> {
    use tinyusd_core::BaseType as B;
    let n = usize::from(ops.byte()? % 5);
    let pick = ops.byte()? % 9;
    let mut floats = Vec::new();
    for _ in 0..16 * n.max(1) {
        floats.push(ops.f64().unwrap_or(0.0));
    }
    let f32s: Vec<f32> = floats.iter().map(|&x| x as f32).collect();
    let v = match pick {
        0 => Value::from_slice(B::Float3, true, &f32s[..3 * n]),
        1 => Value::from_slice(B::Double, false, &floats[..1]),
        2 => Value::from_slice(B::Float, false, &f32s[..1]),
        3 => Value::from_strings(B::Token, true, &vec!["t"; n]),
        4 => Value::from_slice(B::Matrix4d, false, &floats[..16]),
        5 => Value::from_slice(B::Quatf, false, &f32s[..4]),
        6 => Value::from_strings(B::String, false, &["s"]),
        7 => Value::from_slice(B::Double3, false, &floats[..3]),
        _ => Value::from_slice(B::Int, true, &vec![1i32; n]),
    };
    v.ok()
}

pub fn edit_sequence(data: &[u8]) {
    let Ok(mut stage) = Stage::create_in_memory(&OpenOptions::default()) else {
        return;
    };
    let root = stage.edit_target();
    let mut ops = Ops { data };
    let mut steps = 0;
    while let Some(op) = ops.byte() {
        steps += 1;
        if steps > 200 {
            break;
        }
        let Some(path) = ops.pick(&PATHS) else { break };
        let prim = stage.prim_at_path(path).ok().flatten();
        let _ = match (op % 28, prim) {
            (0 | 1 | 2, _) => {
                let Some(t) = ops.pick(&TYPES) else { break };
                stage.define_prim(path, t).map(drop)
            }
            (3, _) => stage.override_prim(path).map(drop),
            (4, Some(p)) => stage.remove_prim(p).map(drop),
            (5 | 6, Some(p)) => {
                let Some(&(name, _)) = ops.pick(&ATTRS) else { break };
                let Some(v) = value(&mut ops) else { break };
                let time = if ops.byte().unwrap_or(0) % 2 == 0 {
                    None
                } else {
                    ops.f64()
                };
                stage.set(p, name, &v, time)
            }
            (7, Some(p)) => {
                let Some(&(name, ty)) = ops.pick(&ATTRS) else { break };
                stage.create_attribute(p, name, ty, false)
            }
            (8, Some(p)) => {
                let Some(target) = ops.pick(&PATHS) else { break };
                stage.set_targets(p, "physics:body0", &[target])
            }
            (9, Some(p)) => {
                let Some(api) = ops.pick(&APIS) else { break };
                stage.apply_api(p, api)
            }
            (10, Some(p)) => {
                let Some(api) = ops.pick(&APIS) else { break };
                stage.remove_api(p, api)
            }
            (11, Some(p)) => {
                let Some(name) = ops.pick(&NAMES) else { break };
                stage.rename_prim(p, name)
            }
            (12, Some(p)) => {
                let Some(parent) = ops.pick(&PATHS) else { break };
                match stage.prim_at_path(parent).ok().flatten() {
                    Some(parent) => stage.reparent_prim(p, parent),
                    None => Ok(()),
                }
            }
            (13, _) => match ops.byte().unwrap_or(0) % 3 {
                0 => stage.begin_edit(),
                1 => stage.commit_edit(),
                _ => {
                    stage.discard_edit();
                    Ok(())
                }
            },
            (14, Some(p)) => {
                let Some(target) = ops.pick(&PATHS) else { break };
                stage.add_reference(p, "", target)
            }
            (15, Some(p)) => {
                let Some(name) = ops.pick(&NAMES) else { break };
                stage.edit_variant(p, "v", name)
            }
            (16, _) => stage.set_edit_target(&root),
            (17, Some(p)) => {
                let t = [ops.f64().unwrap_or(0.0), 0.0, 1.0];
                let time = ops.f64();
                stage.set_trs(p, t, [1.0, 0.0, 0.0, 0.0], [1.0; 3], time)
            }
            (18, Some(p)) => {
                let Some(name) = ops.pick(&["size", "my:f", "points"]) else {
                    break;
                };
                if op % 2 == 0 {
                    stage.block(p, name)
                } else {
                    stage.clear(p, name)
                }
            }
            (19, Some(p)) => stage.set_mesh(p, &[[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], &[3], &[0, 1, 2], None),
            (20, Some(p)) => {
                let Some(key) = ops.pick(&["hidden", "displayName", "customData:a", "customData:a:b", "assetInfo"])
                else {
                    break;
                };
                match (op % 3, value(&mut ops)) {
                    (0, _) => stage.clear_metadata(p, key),
                    (1, Some(v)) => stage.set_metadata(p, key, &v),
                    (_, v) => {
                        let Some(&(name, _)) = ops.pick(&ATTRS) else { break };
                        match v {
                            Some(v) if op % 2 == 0 => stage.set_property_metadata(p, name, key, &v),
                            _ => stage.clear_property_metadata(p, name, key),
                        }
                    }
                }
            }
            (21, Some(p)) => {
                let Some(&(name, _)) = ops.pick(&ATTRS) else { break };
                stage.remove_property(p, name).map(drop)
            }
            (22, Some(p)) => {
                let Some(name) = ops.pick(&NAMES) else { break };
                stage.remove_variant(p, "v", name).map(drop)
            }
            (23, Some(p)) => {
                let Some(target) = ops.pick(&PATHS) else { break };
                if op % 2 == 0 {
                    let offset = ops.f64().unwrap_or(0.0);
                    let scale = ops.f64().unwrap_or(1.0);
                    stage.add_reference_with_offset(p, "", target, offset, scale)
                } else {
                    stage.remove_reference(p, "", target)
                }
            }
            (25, Some(p)) => stage.set_instanceable(p, op % 2 == 0),
            (26, Some(p)) => {
                let Some(target) = ops.pick(&PATHS) else { break };
                let protos: Vec<_> = stage.prim_at_path(target).ok().flatten().into_iter().collect();
                let n = usize::from(ops.byte().unwrap_or(0) % 4);
                let x = ops.f64().unwrap_or(0.0);
                let mut m = [0.0; 16];
                for i in 0..4 {
                    m[i * 5] = 1.0;
                }
                m[12] = x;
                let instances = tinyusd_core::Instances {
                    proto_indices: vec![0; if protos.is_empty() { 0 } else { n }],
                    transforms: vec![m; if protos.is_empty() { 0 } else { n }],
                    prototypes: protos,
                    ids: Vec::new(),
                };
                stage.set_instances(p, &instances, ops.f64())
            }
            (27, Some(p)) => {
                let joints = ["a", "a/b"];
                let mut m = [0.0; 16];
                for i in 0..4 {
                    m[i * 5] = ops.f64().unwrap_or(1.0);
                }
                if op % 2 == 0 {
                    stage.set_skel_animation(p, &joints, &[m, m], ops.f64())
                } else {
                    let skel = format!("{}/Skel", stage.path(p).unwrap_or("/X"));
                    match stage.define_skeleton(&skel, &joints, &[m, m], &[]) {
                        Ok(s) => stage.bind_skeleton(p, s, None),
                        Err(e) => Err(e),
                    }
                }
            }
            (24, _) => {
                let material = stage.define_preview_material(path);
                match (material, op % 3) {
                    (Ok(m), 0) => stage.set_preview_texture(m, "normal", "./n.png").map(drop),
                    (Ok(m), _) => match value(&mut ops) {
                        Some(v) => stage.set_preview_input(m, "diffuseColor", &v),
                        None => Ok(()),
                    },
                    (Err(e), _) => Err(e),
                }
            }
            _ => Ok(()),
        };
        let _ = stage.world_transform(prim.unwrap_or(stage.pseudo_root()), None);
    }
    if stage.in_edit() {
        let _ = stage.commit_edit();
    }
    let _ = stage.set_edit_target(&root);
    let Ok(live) = stage.traverse(TraverseFlags::DEFAULT) else {
        return;
    };
    let mut live: Vec<String> = live.iter().map(|&p| stage.path(p).unwrap_or("?").to_owned()).collect();
    live.sort();
    for (format, extension) in [
        (tinyusd_core::LayerFormat::Usda, "usda"),
        (tinyusd_core::LayerFormat::Usdc, "usdc"),
    ] {
        let bytes = stage
            .export_bytes(None, format)
            .unwrap_or_else(|e| panic!("flattening a stage the edits left failed: {e}"));
        let path = format!("mem://edited.{extension}");
        let resolver = Rc::new(OneFile {
            path: path.clone(),
            bytes,
        });
        let options = OpenOptions {
            resolver: Some(resolver),
            ..OpenOptions::default()
        };
        let mut reread = Stage::open(&path, &options).unwrap_or_else(|e| panic!("the export doesn't reopen: {e}"));
        let prims = reread.traverse(TraverseFlags::DEFAULT).expect("traverse the export");
        let mut paths: Vec<String> = prims.iter().map(|&p| reread.path(p).unwrap().to_owned()).collect();
        paths.sort();
        assert_eq!(live, paths, "the {extension} export composes other prims");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn edit_sequences() {
        let mut state = 0x2545_f491_4f6c_dd1du64;
        for len in 0..400 {
            let bytes: Vec<u8> = (0..len % 160)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state as u8
                })
                .collect();
            super::edit_sequence(&bytes);
        }
    }
}
