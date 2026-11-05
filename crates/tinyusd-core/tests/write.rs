#![cfg(feature = "write")]

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use tinyusd_core::{AssetResolver, BaseType, Error, LayerFormat, OpenOptions, Prim, Stage, TraverseFlags, Value};

fn data(name: &str) -> String {
    format!("{}/../../tests/data/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn path_in(dir: &tempfile::TempDir, name: &str) -> String {
    dir.path().join(name).to_string_lossy().into_owned()
}

fn get(stage: &Stage, prim: Prim, name: &str, time: Option<f64>) -> Value {
    let mut v = Value::new();
    assert!(stage.get(prim, name, time, &mut v).unwrap(), "{name} has no value");
    v
}

fn find(stage: &mut Stage, path: &str) -> Prim {
    stage
        .prim_at_path(path)
        .unwrap()
        .unwrap_or_else(|| panic!("missing {path}"))
}

const POINTS: [[f32; 3]; 4] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];

fn build_quad(stage: &mut Stage) -> Prim {
    let world = stage.define_prim("/World", "Xform").unwrap();
    stage.set_default_prim(world).unwrap();
    stage.set_up_axis("Z").unwrap();
    stage.set_meters_per_unit(0.01).unwrap();
    let mesh = stage.define_prim("/World/Quad", "Mesh").unwrap();
    let points = Value::from_slice(BaseType::Float3, true, &POINTS).unwrap();
    stage.set(mesh, "points", &points, None).unwrap();
    let counts = Value::from_slice(BaseType::Int, true, &[4i32]).unwrap();
    stage.set(mesh, "faceVertexCounts", &counts, None).unwrap();
    let indices = Value::from_slice(BaseType::Int, true, &[0i32, 1, 2, 3]).unwrap();
    stage.set(mesh, "faceVertexIndices", &indices, None).unwrap();
    mesh
}

fn check_quad(stage: &mut Stage) {
    let mesh = find(stage, "/World/Quad");
    assert!(stage.is_a(mesh, "Mesh").unwrap());
    let points = get(stage, mesh, "points", None);
    assert_eq!(points.type_name(), "point3f[]");
    assert_eq!(points.as_slice::<[f32; 3]>().unwrap(), &POINTS);
    assert_eq!(
        get(stage, mesh, "faceVertexIndices", None).as_slice::<i32>().unwrap(),
        &[0, 1, 2, 3]
    );
    let world = stage.default_prim().unwrap().unwrap();
    assert_eq!(stage.path(world).unwrap(), "/World");
    let mut v = Value::new();
    assert!(stage.stage_metadata("metersPerUnit", &mut v).unwrap());
    assert_eq!(v.as_scalar::<f64>(), Some(0.01));
}

#[test]
fn create_save_and_reopen_in_every_format() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["scene.usda", "scene.usdc", "scene.usd"] {
        let path = path_in(&dir, name);
        let mut stage = Stage::create_new(&path, &OpenOptions::default()).unwrap();
        build_quad(&mut stage);
        stage.save().unwrap();
        drop(stage);
        let mut reopened = Stage::open(&path, &OpenOptions::default()).unwrap();
        check_quad(&mut reopened);
    }
    let err = Stage::create_new(&path_in(&dir, "scene.usdz"), &OpenOptions::default())
        .err()
        .unwrap();
    assert!(matches!(err, Error::Write { .. }), "{err}");
}

#[test]
fn export_flattened_and_layers() {
    let dir = tempfile::tempdir().unwrap();
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    build_quad(&mut stage);
    assert!(stage.save().is_ok(), "nothing on disk to save");
    for name in ["out.usda", "out.usdc", "out.usdz"] {
        let path = path_in(&dir, name);
        stage.export(&path).unwrap();
        check_quad(&mut Stage::open(&path, &OpenOptions::default()).unwrap());
    }
    let text = String::from_utf8(stage.export_bytes(None, LayerFormat::Usda).unwrap()).unwrap();
    assert!(text.starts_with("#usda 1.0"), "{text}");
    assert!(text.contains("def Mesh \"Quad\""), "{text}");
    let root = stage.edit_target();
    let crate_bytes = stage.export_bytes(Some(&root), LayerFormat::Usdc).unwrap();
    assert!(crate_bytes.starts_with(b"PXR-USDC"));
}

#[test]
fn values_are_strictly_typed() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let mesh = build_quad(&mut stage);
    let doubles = Value::from_slice(BaseType::Double3, true, &[[0.0f64; 3]; 4]).unwrap();
    let err = stage.set(mesh, "points", &doubles, None).unwrap_err();
    assert!(matches!(err, Error::InvalidValue(_)), "{err}");
    let err = stage
        .set(mesh, "subdivisionScheme", &Value::string("none"), None)
        .unwrap_err();
    assert!(matches!(err, Error::InvalidValue(_)), "{err}");
    stage
        .set(mesh, "subdivisionScheme", &Value::token("none"), None)
        .unwrap();

    let err = stage.set(mesh, "my:weight", &Value::float(2.0), None).unwrap_err();
    assert!(matches!(err, Error::InvalidValue(_)), "{err}");
    stage.create_attribute(mesh, "my:weight", "float", false).unwrap();
    stage.set(mesh, "my:weight", &Value::float(2.0), None).unwrap();
    assert_eq!(get(&stage, mesh, "my:weight", None).as_scalar::<f32>(), Some(2.0));
    assert!(stage.attribute_info(mesh, "my:weight").unwrap().unwrap().custom);
    let err = stage.create_attribute(mesh, "my:bad", "float7", false).unwrap_err();
    assert!(matches!(err, Error::InvalidValue(_)), "{err}");

    assert!(Value::from_slice(BaseType::Float3, true, &[1.0f32, 2.0]).is_err());
    assert!(Value::from_slice(BaseType::Float, false, &[1.0f32, 2.0]).is_err());
}

#[test]
fn time_samples_blocks_and_clears() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let cube = stage.define_prim("/Cube", "Cube").unwrap();
    for frame in 0..3 {
        stage
            .set(
                cube,
                "size",
                &Value::double(f64::from(frame) + 1.0),
                Some(f64::from(frame)),
            )
            .unwrap();
    }
    assert_eq!(stage.time_samples(cube, "size").unwrap(), vec![0.0, 1.0, 2.0]);
    assert_eq!(get(&stage, cube, "size", Some(1.5)).as_scalar::<f64>(), Some(2.5));
    stage.block(cube, "size").unwrap();
    assert!(stage.time_samples(cube, "size").unwrap().is_empty());
    assert_eq!(
        get(&stage, cube, "size", Some(1.0)).as_scalar::<f64>(),
        Some(2.0),
        "schema fallback"
    );
    stage.create_attribute(cube, "my:v", "int", false).unwrap();
    stage.set(cube, "my:v", &Value::int(4), None).unwrap();
    stage.block(cube, "my:v").unwrap();
    let mut v = Value::new();
    assert!(!stage.get(cube, "my:v", None, &mut v).unwrap());
    stage.set(cube, "size", &Value::double(5.0), None).unwrap();
    stage.clear(cube, "size").unwrap();
    assert_eq!(
        get(&stage, cube, "size", None).as_scalar::<f64>(),
        Some(2.0),
        "schema fallback"
    );
}

#[test]
fn transform_helpers_see_edits() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let xform = stage.define_prim("/A", "Xform").unwrap();
    let child = stage.define_prim("/A/B", "Xform").unwrap();
    stage
        .create_attribute(xform, "xformOp:translate", "double3", false)
        .unwrap();
    stage
        .set(
            xform,
            "xformOp:translate",
            &Value::from_slice(BaseType::Double3, false, &[[1.0f64, 2.0, 3.0]]).unwrap(),
            None,
        )
        .unwrap();
    let order = Value::from_strings(BaseType::Token, true, &["xformOp:translate"]).unwrap();
    stage.set(xform, "xformOpOrder", &order, None).unwrap();
    assert_eq!(&stage.world_transform(child, None).unwrap()[12..15], &[1.0, 2.0, 3.0]);
    stage
        .set(
            xform,
            "xformOp:translate",
            &Value::from_slice(BaseType::Double3, false, &[[4.0f64, 5.0, 6.0]]).unwrap(),
            None,
        )
        .unwrap();
    assert_eq!(&stage.world_transform(child, None).unwrap()[12..15], &[4.0, 5.0, 6.0]);
}

#[test]
fn prims_schemas_arcs_and_relationships() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let body = stage.define_prim("/World/Body", "Cube").unwrap();
    stage.apply_api(body, "PhysicsRigidBodyAPI").unwrap();
    stage.apply_api(body, "PhysicsMassAPI").unwrap();
    stage.set(body, "physics:mass", &Value::float(3.0), None).unwrap();
    assert!(stage.has_api(body, "PhysicsRigidBodyAPI").unwrap());
    let joint = stage.define_prim("/World/Joint", "PhysicsRevoluteJoint").unwrap();
    stage.apply_api(joint, "PhysicsLimitAPI:rotX").unwrap();
    assert!(stage.has_api(joint, "PhysicsLimitAPI:rotX").unwrap());
    stage.set_targets(joint, "physics:body0", &["/World/Body"]).unwrap();
    assert_eq!(
        stage.targets(joint, "physics:body0", false).unwrap(),
        vec!["/World/Body"]
    );

    let material = stage.define_prim("/World/Mat", "Material").unwrap();
    let shader = stage.define_prim("/World/Mat/Surface", "Shader").unwrap();
    stage
        .create_attribute(shader, "outputs:surface", "token", false)
        .unwrap();
    stage
        .create_attribute(material, "outputs:surface", "token", false)
        .unwrap();
    stage
        .set_connections(material, "outputs:surface", &["/World/Mat/Surface.outputs:surface"])
        .unwrap();
    assert_eq!(
        stage.connections(material, "outputs:surface").unwrap(),
        vec!["/World/Mat/Surface.outputs:surface"]
    );
    stage.apply_api(body, "MaterialBindingAPI").unwrap();
    stage.set_targets(body, "material:binding", &["/World/Mat"]).unwrap();
    let bound = stage.bound_material(body, "").unwrap().unwrap();
    assert_eq!(stage.path(bound.material).unwrap(), "/World/Mat");

    stage.define_prim("/Proto", "Sphere").unwrap();
    let inst = stage.define_prim("/World/Inst", "").unwrap();
    stage.add_reference(inst, "", "/Proto").unwrap();
    assert!(stage.is_a(inst, "Sphere").unwrap());
    stage.set_kind(inst, "component").unwrap();
    assert_eq!(stage.kind(inst).unwrap(), "component");
    stage.set_active(inst, false).unwrap();
    assert!(!stage.traverse(TraverseFlags::DEFAULT).unwrap().contains(&inst));
    assert!(stage.remove_prim(inst).unwrap());
    assert!(stage.prim_at_path("/World/Inst").unwrap().is_none());
}

#[test]
fn batches_write_once_and_resolve_schema_types() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    stage.begin_edit().unwrap();
    let mesh = stage.define_prim("/Mesh", "Mesh").unwrap();
    let points = Value::from_slice(BaseType::Float3, true, &POINTS).unwrap();
    stage.set(mesh, "points", &points, None).unwrap();
    stage.create_attribute(mesh, "my:temp", "float", false).unwrap();
    for frame in 0..100 {
        stage
            .set(mesh, "my:temp", &Value::float(frame as f32), Some(f64::from(frame)))
            .unwrap();
    }
    stage.set(mesh, "my:temp", &Value::float(-1.0), Some(5.0)).unwrap();
    let err = stage.set(mesh, "my:undeclared", &Value::float(1.0), None).unwrap_err();
    assert!(matches!(err, Error::InvalidValue(_)), "{err}");
    assert!(stage.apply_api(mesh, "PhysicsCollisionAPI").is_err(), "not batchable");
    assert!(
        stage.prim_at_path("/Mesh").unwrap().is_none(),
        "nothing written before commit"
    );
    stage.commit_edit().unwrap();

    assert_eq!(
        get(&stage, mesh, "points", None).as_slice::<[f32; 3]>().unwrap(),
        &POINTS
    );
    assert_eq!(stage.time_samples(mesh, "my:temp").unwrap().len(), 100);
    assert_eq!(get(&stage, mesh, "my:temp", Some(5.0)).as_scalar::<f32>(), Some(-1.0));

    let err = stage
        .edit(|s| {
            s.define_prim("/Other", "Xform")?;
            s.set(mesh, "points", &Value::float(1.0), None)
        })
        .unwrap_err();
    assert!(matches!(err, Error::InvalidValue(_)), "{err}");
    assert!(
        stage.prim_at_path("/Other").unwrap().is_none(),
        "a failed batch writes nothing"
    );
    assert!(!stage.in_edit());
}

#[test]
fn overlay_layer_leaves_the_source_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let source = path_in(&dir, "robot.usda");
    std::fs::copy(data("physics.usda"), &source).unwrap();
    let original = std::fs::read(&source).unwrap();

    let result = path_in(&dir, "result.usda");
    let mut stage = Stage::create_new(&result, &OpenOptions::default()).unwrap();
    let robot = stage.add_sublayer("./robot.usda", false).unwrap();
    assert!(robot.ends_with("robot.usda"), "{robot}");
    let base = find(&mut stage, "/World/Base");
    stage.begin_edit().unwrap();
    for frame in 0..10 {
        let t = Value::from_slice(BaseType::Double3, false, &[[0.0f64, 0.0, f64::from(frame)]]).unwrap();
        stage
            .set(base, "xformOp:translate", &t, Some(f64::from(frame)))
            .unwrap();
    }
    stage.commit_edit().unwrap();
    stage.set_time_code_range(0.0, 9.0).unwrap();
    assert_eq!(stage.world_transform(base, Some(7.0)).unwrap()[14], 7.0);

    let extra = stage.add_sublayer("./extra.usda", true).unwrap();
    stage.set_edit_target(&extra).unwrap();
    assert_eq!(stage.edit_target(), extra);
    stage.define_prim("/World/Probe", "Sphere").unwrap();
    let world = find(&mut stage, "/World");
    stage.set_default_prim(world).unwrap();
    stage.save().unwrap();
    drop(stage);

    assert_eq!(std::fs::read(&source).unwrap(), original, "the source is never written");
    let text = std::fs::read_to_string(&result).unwrap();
    assert!(
        text.contains("@./extra.usda@") && text.contains("@./robot.usda@"),
        "{text}"
    );
    assert!(
        text.contains("timeSamples") && text.contains("endTimeCode = 9"),
        "{text}"
    );
    assert!(
        text.contains("defaultPrim"),
        "stage metadata goes to the root layer: {text}"
    );
    let extra_text = std::fs::read_to_string(path_in(&dir, "extra.usda")).unwrap();
    assert!(extra_text.contains("def Sphere \"Probe\""), "{extra_text}");

    let mut reopened = Stage::open(&result, &OpenOptions::default()).unwrap();
    let base = find(&mut reopened, "/World/Base");
    assert_eq!(reopened.world_transform(base, Some(7.0)).unwrap()[14], 7.0);
    assert!(reopened.prim_at_path("/World/Probe").unwrap().is_some());
    assert!(reopened.add_sublayer("./missing.usda", false).is_err());
}

#[derive(Default)]
struct MemoryFs {
    files: RefCell<HashMap<String, Vec<u8>>>,
}

impl AssetResolver for MemoryFs {
    fn resolve(&self, asset_path: &str, anchor: Option<&str>) -> Option<String> {
        let path = match (asset_path.strip_prefix("./"), anchor) {
            (Some(rel), Some(anchor)) => format!("{}/{rel}", anchor.rsplit_once('/')?.0),
            _ => asset_path.to_owned(),
        };
        self.files.borrow().contains_key(&path).then_some(path)
    }

    fn read(&self, resolved_path: &str) -> std::io::Result<Vec<u8>> {
        self.files
            .borrow()
            .get(resolved_path)
            .cloned()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, resolved_path.to_owned()))
    }

    fn write(&self, resolved_path: &str, bytes: &[u8]) -> std::io::Result<()> {
        self.files.borrow_mut().insert(resolved_path.to_owned(), bytes.to_vec());
        Ok(())
    }
}

#[test]
fn host_resolvers_receive_writes() {
    let fs = Rc::new(MemoryFs::default());
    let options = OpenOptions {
        resolver: Some(fs.clone()),
        ..OpenOptions::default()
    };
    let mut stage = Stage::create_new("mem://level/scene.usda", &options).unwrap();
    build_quad(&mut stage);
    stage.save().unwrap();
    stage.export("mem://level/flat.usdc").unwrap();
    drop(stage);
    assert!(fs.files.borrow().contains_key("mem://level/flat.usdc"));
    let text = String::from_utf8(fs.files.borrow()["mem://level/scene.usda"].clone()).unwrap();
    assert!(text.contains("def Mesh \"Quad\""), "{text}");
    check_quad(&mut Stage::open("mem://level/scene.usda", &options).unwrap());
    check_quad(&mut Stage::open("mem://level/flat.usdc", &options).unwrap());
}

fn close(a: &[f64], b: &[f64]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
}

#[test]
fn transform_helpers_round_trip() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let parent = stage.define_prim("/P", "Xform").unwrap();
    let child = stage.define_prim("/P/C", "Xform").unwrap();
    let half = std::f64::consts::FRAC_1_SQRT_2;
    stage
        .set_trs(parent, [1.0, 2.0, 3.0], [half, 0.0, 0.0, half], [2.0, 2.0, 2.0], None)
        .unwrap();
    let world = stage.world_transform(parent, None).unwrap();
    assert!(close(&world[12..15], &[1.0, 2.0, 3.0]));
    assert!(close(&world[0..3], &[0.0, 2.0, 0.0]), "{world:?}");
    let mut order = Value::new();
    assert!(stage.get(parent, "xformOpOrder", None, &mut order).unwrap());
    assert_eq!(
        order.strings(),
        ["xformOp:translate", "xformOp:orient", "xformOp:scale"]
    );

    let target = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 10.0, 20.0, 30.0, 1.0,
    ];
    stage.set_world_transform(child, &target, None).unwrap();
    assert!(close(&stage.world_transform(child, None).unwrap(), &target));
    stage.set_local_transform(child, &target, Some(4.0)).unwrap();
    assert!(close(&stage.local_transform(child, Some(4.0)).unwrap().0, &target));

    let body = stage.define_prim("/Body", "Xform").unwrap();
    stage.create_attribute(body, "xformOp:orient", "quatf", false).unwrap();
    stage
        .edit(|s| {
            for frame in 0..3 {
                s.set_trs(
                    body,
                    [0.0, 0.0, f64::from(frame)],
                    [1.0, 0.0, 0.0, 0.0],
                    [1.0; 3],
                    Some(f64::from(frame)),
                )?;
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(
        stage.attribute_info(body, "xformOp:orient").unwrap().unwrap().type_name,
        "quatf"
    );
    assert_eq!(stage.world_transform(body, Some(2.0)).unwrap()[14], 2.0);
}

#[test]
fn mesh_primvar_and_material_helpers() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let mesh = stage.define_prim("/Quad", "Mesh").unwrap();
    assert!(matches!(
        stage.set_mesh(mesh, &POINTS, &[4], &[0, 1, 2], None),
        Err(Error::InvalidValue(_))
    ));
    assert!(matches!(
        stage.set_mesh(mesh, &POINTS, &[4], &[0, 1, 2, 9], None),
        Err(Error::InvalidValue(_))
    ));
    stage.set_mesh(mesh, &POINTS, &[4], &[0, 1, 2, 3], None).unwrap();
    let extent = get(&stage, mesh, "extent", None);
    assert_eq!(
        extent.as_slice::<[f32; 3]>().unwrap(),
        &[[0.0, 0.0, 0.0], [1.0, 1.0, 0.0]]
    );

    let uvs = Value::from_slice(BaseType::Float2, true, &[[0.0f32, 0.0], [1.0, 1.0]]).unwrap();
    stage
        .set_primvar(
            mesh,
            "st",
            "texCoord2f[]",
            &uvs,
            tinyusd_core::Interpolation::FaceVarying,
            Some(&[0, 1, 1, 0]),
            0,
            None,
        )
        .unwrap();
    let mut out = Value::new();
    let info = stage.primvar(mesh, "st", None, &mut out).unwrap().unwrap();
    assert_eq!(info.interpolation, tinyusd_core::Interpolation::FaceVarying);
    assert!(info.indexed);
    assert_eq!(
        out.as_slice::<[f32; 2]>().unwrap(),
        &[[0.0, 0.0], [1.0, 1.0], [1.0, 1.0], [0.0, 0.0]]
    );
    assert_eq!(info.type_name, "texCoord2f[]");

    let color = Value::from_slice(BaseType::Float3, true, &[[1.0f32, 0.0, 0.0]]).unwrap();
    stage
        .edit(|s| {
            s.set_primvar(
                mesh,
                "displayColor",
                "color3f[]",
                &color,
                tinyusd_core::Interpolation::Constant,
                None,
                0,
                None,
            )
        })
        .unwrap();
    let info = stage.primvar(mesh, "displayColor", None, &mut out).unwrap().unwrap();
    assert_eq!(info.interpolation, tinyusd_core::Interpolation::Constant);

    let material = stage.define_prim("/Looks/Red", "Material").unwrap();
    stage.bind_material(mesh, material, "").unwrap();
    stage.bind_material(mesh, material, "preview").unwrap();
    let bound = stage.bound_material(mesh, "preview").unwrap().unwrap();
    assert_eq!(stage.path(bound.material).unwrap(), "/Looks/Red");
}

#[test]
fn packages_carry_their_assets() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(data("tex.png"), dir.path().join("tex.png")).unwrap();
    let mut stage = Stage::create_new(&path_in(&dir, "scene.usda"), &OpenOptions::default()).unwrap();
    let shader = stage.define_prim("/Looks/Tex", "Shader").unwrap();
    stage.create_attribute(shader, "inputs:file", "asset", false).unwrap();
    stage
        .set(shader, "inputs:file", &Value::asset("./tex.png"), None)
        .unwrap();
    let pkg = path_in(&dir, "scene.usdz");
    stage.export_package(&pkg).unwrap();
    assert!(stage.export_package(&path_in(&dir, "scene.usda")).is_err());

    let mut packaged = Stage::open(&pkg, &OpenOptions::default()).unwrap();
    let shader = find(&mut packaged, "/Looks/Tex");
    let file = get(&packaged, shader, "inputs:file", None);
    assert!(
        file.resolved_paths()[0].contains(".usdz["),
        "{:?}",
        file.resolved_paths()
    );
    assert_eq!(
        packaged.read_asset(&file.resolved_paths()[0]).unwrap(),
        std::fs::read(data("tex.png")).unwrap()
    );
}

#[test]
fn remove_api_hides_weaker_opinions() {
    let dir = tempfile::tempdir().unwrap();
    let mut stage = Stage::create_new(&path_in(&dir, "result.usda"), &OpenOptions::default()).unwrap();
    stage.add_sublayer(&data("physics.usda"), false).unwrap();
    let base = find(&mut stage, "/World/Base");
    assert!(stage.has_api(base, "PhysicsRigidBodyAPI").unwrap());
    stage.remove_api(base, "PhysicsRigidBodyAPI").unwrap();
    assert!(!stage.has_api(base, "PhysicsRigidBodyAPI").unwrap());
    assert!(stage.has_api(base, "PhysicsMassAPI").unwrap());

    let local = stage.define_prim("/Local", "Xform").unwrap();
    stage.apply_api(local, "PhysicsLimitAPI:rotX").unwrap();
    stage.apply_api(local, "PhysicsLimitAPI:rotY").unwrap();
    stage.remove_api(local, "PhysicsLimitAPI:rotX").unwrap();
    assert_eq!(stage.applied_schemas(local).unwrap(), ["PhysicsLimitAPI:rotY"]);
    stage.apply_api(local, "PhysicsLimitAPI:rotX").unwrap();
    assert!(
        stage.has_api(local, "PhysicsLimitAPI:rotX").unwrap(),
        "applying again cancels the removal"
    );
}

#[test]
fn rename_and_reparent_follow_handles() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let a = stage.define_prim("/World/A", "Xform").unwrap();
    let child = stage.define_prim("/World/A/Child", "Cube").unwrap();
    let b = stage.define_prim("/World/B", "Xform").unwrap();
    let joint = stage.define_prim("/World/Joint", "PhysicsFixedJoint").unwrap();
    stage.set_targets(joint, "physics:body0", &["/World/A/Child"]).unwrap();
    stage.set(child, "size", &Value::double(3.0), None).unwrap();

    stage.rename_prim(a, "Renamed").unwrap();
    assert_eq!(stage.path(a).unwrap(), "/World/Renamed");
    assert_eq!(stage.path(child).unwrap(), "/World/Renamed/Child");
    assert!(stage.prim_at_path("/World/A").unwrap().is_none());
    assert_eq!(get(&stage, child, "size", None).as_scalar::<f64>(), Some(3.0));
    assert_eq!(
        stage.targets(joint, "physics:body0", false).unwrap(),
        ["/World/Renamed/Child"]
    );

    stage.reparent_prim(child, b).unwrap();
    assert_eq!(stage.path(child).unwrap(), "/World/B/Child");
    assert_eq!(stage.prim_at_path("/World/B/Child").unwrap(), Some(child));
    assert!(stage.is_a(child, "Cube").unwrap());
    assert!(stage.rename_prim(stage.pseudo_root(), "x").is_err());
}

#[test]
fn variants_hold_what_is_authored_inside_them() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let asset = stage.define_prim("/Asset", "Xform").unwrap();
    stage.edit_variant(asset, "lod", "high").unwrap();
    let mesh = stage.define_prim("/Asset/Body", "Sphere").unwrap();
    stage.set(mesh, "radius", &Value::double(2.0), None).unwrap();
    stage.edit_variant(asset, "lod", "low").unwrap();
    let mesh_low = stage.define_prim("/Asset/Body", "Cube").unwrap();
    assert_eq!(mesh, mesh_low);
    let root = stage.edit_target();
    stage.set_edit_target(&root).unwrap();
    assert_eq!(
        stage.variant_sets(asset).unwrap(),
        [("lod".to_owned(), vec!["high".to_owned(), "low".to_owned()])]
    );

    assert!(stage.is_a(mesh, "Cube").unwrap(), "the last variant edited is selected");
    stage.set_variant_selection(asset, "lod", "high").unwrap();
    assert!(stage.is_a(mesh, "Sphere").unwrap());
    assert_eq!(get(&stage, mesh, "radius", None).as_scalar::<f64>(), Some(2.0));
    let text = String::from_utf8(stage.export_bytes(Some(&root), LayerFormat::Usda).unwrap()).unwrap();
    assert!(text.contains("variantSet \"lod\""), "{text}");
    assert!(text.contains("variants = {"), "{text}");
}
