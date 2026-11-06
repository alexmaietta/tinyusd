#![cfg(feature = "write")]

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use tinyusd_core::{
    AssetResolver, BaseType, Error, Instances, LayerFormat, OpenOptions, Prim, Stage, TraverseFlags, Value,
};

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
    assert!(stage.save().is_err(), "saving needs the batch committed");
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

fn meta(stage: &Stage, prim: Prim, key: &str) -> Option<Value> {
    let mut v = Value::new();
    stage.prim_metadata(prim, key, &mut v).unwrap().then_some(v)
}

fn attr_meta(stage: &Stage, prim: Prim, name: &str, key: &str) -> Option<Value> {
    let mut v = Value::new();
    stage.attribute_metadata(prim, name, key, &mut v).unwrap().then_some(v)
}

#[test]
fn metadata_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = path_in(&dir, "meta.usda");
    let mut stage = Stage::create_new(&path, &OpenOptions::default()).unwrap();
    let prim = stage.define_prim("/World", "Xform").unwrap();
    stage.set_metadata(prim, "hidden", &Value::bool(true)).unwrap();
    stage
        .set_metadata(prim, "displayName", &Value::string("The World"))
        .unwrap();
    stage
        .set_metadata(prim, "customData:tool:version", &Value::int(3))
        .unwrap();
    stage
        .set_metadata(prim, "customData:note", &Value::string("hi"))
        .unwrap();
    stage
        .set_metadata(prim, "assetInfo:name", &Value::string("world"))
        .unwrap();
    stage.create_attribute(prim, "color", "color3f", false).unwrap();
    stage
        .set_property_metadata(prim, "color", "colorSpace", &Value::token("lin_rec709"))
        .unwrap();
    stage
        .set_property_metadata(prim, "color", "customData:source", &Value::string("scan"))
        .unwrap();
    stage.set_targets(prim, "proxy", &["/World"]).unwrap();
    stage
        .set_property_metadata(prim, "proxy", "documentation", &Value::string("doc"))
        .unwrap();

    assert!(matches!(
        stage.set_metadata(prim, "hidden", &Value::int(1)),
        Err(Error::InvalidValue(_))
    ));
    assert!(matches!(
        stage.set_metadata(prim, "typeName", &Value::token("Mesh")),
        Err(Error::InvalidValue(_))
    ));
    assert!(matches!(
        stage.set_metadata(prim, "customData", &Value::int(1)),
        Err(Error::InvalidValue(_))
    ));
    assert!(matches!(
        stage.set_property_metadata(prim, "missing", "hidden", &Value::bool(true)),
        Err(Error::InvalidValue(_))
    ));
    stage.save().unwrap();

    let mut stage = Stage::open(&path, &OpenOptions::default()).unwrap();
    let prim = find(&mut stage, "/World");
    assert!(meta(&stage, prim, "hidden").unwrap().as_scalar::<u8>().unwrap() != 0);
    assert_eq!(meta(&stage, prim, "displayName").unwrap().strings(), ["The World"]);
    assert_eq!(
        meta(&stage, prim, "customData:tool:version")
            .unwrap()
            .as_scalar::<i32>(),
        Some(3)
    );
    assert_eq!(stage.prim_metadata_keys(prim, "customData").unwrap(), ["note", "tool"]);
    assert_eq!(meta(&stage, prim, "assetInfo:name").unwrap().strings(), ["world"]);
    assert_eq!(
        attr_meta(&stage, prim, "color", "colorSpace").unwrap().strings(),
        ["lin_rec709"]
    );
    assert_eq!(
        attr_meta(&stage, prim, "color", "customData:source").unwrap().strings(),
        ["scan"]
    );

    stage.clear_metadata(prim, "hidden").unwrap();
    stage.clear_metadata(prim, "customData:tool").unwrap();
    stage.clear_metadata(prim, "assetInfo").unwrap();
    stage
        .clear_property_metadata(prim, "color", "customData:source")
        .unwrap();
    stage.clear_property_metadata(prim, "proxy", "documentation").unwrap();
    assert!(meta(&stage, prim, "hidden").is_none());
    assert_eq!(stage.prim_metadata_keys(prim, "customData").unwrap(), ["note"]);
    assert!(meta(&stage, prim, "assetInfo").is_none());
    assert!(attr_meta(&stage, prim, "color", "customData").is_none());
    let text = String::from_utf8(stage.export_bytes(None, LayerFormat::Usda).unwrap()).unwrap();
    assert!(!text.contains("documentation"), "{text}");
}

#[test]
fn metadata_queues_in_a_batch() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let prim = stage.define_prim("/A", "Xform").unwrap();
    stage
        .edit(|s| {
            s.create_attribute(prim, "w", "float", false)?;
            s.set_property_metadata(prim, "w", "displayGroup", &Value::string("Weights"))?;
            s.set_property_metadata(prim, "w", "customData:k", &Value::int(1))?;
            s.set_metadata(prim, "hidden", &Value::bool(true))?;
            assert!(
                s.set_metadata(prim, "hidden", &Value::int(1)).is_err(),
                "checked when queued"
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(
        attr_meta(&stage, prim, "w", "customData:k").unwrap().as_scalar::<i32>(),
        Some(1)
    );
    assert!(meta(&stage, prim, "hidden").is_some());
    assert_eq!(
        attr_meta(&stage, prim, "w", "displayGroup").unwrap().strings(),
        ["Weights"]
    );
}

#[test]
fn removals() {
    let dir = tempfile::tempdir().unwrap();
    let asset = path_in(&dir, "asset.usda");
    let mut a = Stage::create_new(&asset, &OpenOptions::default()).unwrap();
    let root = a.define_prim("/Asset", "Xform").unwrap();
    a.define_prim("/Asset/Child", "Xform").unwrap();
    a.set_default_prim(root).unwrap();
    a.save().unwrap();

    let base = path_in(&dir, "base.usda");
    let mut b = Stage::create_new(&base, &OpenOptions::default()).unwrap();
    let thing = b.define_prim("/Thing", "Xform").unwrap();
    b.add_reference_with_offset(thing, "./asset.usda", "", 10.0, 2.0)
        .unwrap();
    b.save().unwrap();

    let top = path_in(&dir, "top.usda");
    let mut stage = Stage::create_new(&top, &OpenOptions::default()).unwrap();
    stage.add_sublayer("./base.usda", false).unwrap();
    let thing = find(&mut stage, "/Thing");
    assert!(stage.prim_at_path("/Thing/Child").unwrap().is_some());
    stage.remove_reference(thing, "./asset.usda", "").unwrap();
    assert!(
        stage.prim_at_path("/Thing/Child").unwrap().is_none(),
        "a weaker layer's reference with an offset is deleted"
    );

    stage.create_attribute(thing, "a", "float", false).unwrap();
    stage.set(thing, "a", &Value::float(1.0), None).unwrap();
    assert!(stage.remove_property(thing, "a").unwrap());
    assert!(!stage.remove_property(thing, "a").unwrap());
    assert!(stage.attribute_info(thing, "a").unwrap().is_none());

    stage.set_targets(thing, "r", &["/A", "/B", "/C"]).unwrap();
    stage.remove_target(thing, "r", "/B").unwrap();
    assert_eq!(stage.targets(thing, "r", false).unwrap(), ["/A", "/C"]);

    stage.create_attribute(thing, "in", "float", false).unwrap();
    stage.set_connections(thing, "in", &["/X.out", "/Y.out"]).unwrap();
    stage.remove_connection(thing, "in", "/X.out").unwrap();
    assert_eq!(stage.connections(thing, "in").unwrap(), ["/Y.out"]);

    stage.add_inherit(thing, "/_class").unwrap();
    stage.remove_inherit(thing, "/_class").unwrap();
    stage.add_specialize(thing, "/_base").unwrap();
    stage.remove_specialize(thing, "/_base").unwrap();
    let text = String::from_utf8(stage.export_bytes(Some(&top), LayerFormat::Usda).unwrap()).unwrap();
    assert!(!text.contains("prepend inherits"), "{text}");
    assert!(!text.contains("prepend specializes"), "{text}");

    let v = stage.define_prim("/V", "Xform").unwrap();
    for name in ["red", "blue"] {
        stage.edit_variant(v, "color", name).unwrap();
        stage.define_prim(&format!("/V/{name}"), "Xform").unwrap();
    }
    stage.set_edit_target(&top).unwrap();
    stage.set_variant_selection(v, "color", "red").unwrap();
    assert!(stage.remove_variant(v, "color", "red").unwrap());
    let text = String::from_utf8(stage.export_bytes(Some(&top), LayerFormat::Usda).unwrap()).unwrap();
    assert!(
        !text.contains("color = \"red\""),
        "the layer's selection of the removed variant goes too: {text}"
    );
    assert!(!stage.remove_variant(v, "color", "red").unwrap());
    assert_eq!(
        stage.variant_sets(v).unwrap(),
        [("color".to_owned(), vec!["blue".to_owned()])]
    );
    assert!(stage.prim_at_path("/V/red").unwrap().is_none());
    stage.set_variant_selection(v, "color", "blue").unwrap();
    assert!(stage.prim_at_path("/V/blue").unwrap().is_some());
    stage.save().unwrap();
    let text = std::fs::read_to_string(&top).unwrap();
    assert!(!text.contains("\"red\" {"), "{text}");
}

#[test]
fn references_take_a_layer_offset() {
    let dir = tempfile::tempdir().unwrap();
    let anim = path_in(&dir, "anim.usda");
    let mut a = Stage::create_new(&anim, &OpenOptions::default()).unwrap();
    let root = a.define_prim("/Anim", "Xform").unwrap();
    a.set_default_prim(root).unwrap();
    a.create_attribute(root, "v", "double", false).unwrap();
    a.set(root, "v", &Value::double(0.0), Some(0.0)).unwrap();
    a.set(root, "v", &Value::double(100.0), Some(10.0)).unwrap();
    a.save().unwrap();

    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let late = stage.define_prim("/Late", "Xform").unwrap();
    stage.add_reference_with_offset(late, &anim, "", 5.0, 2.0).unwrap();
    let lazy = stage.define_prim("/Lazy", "Xform").unwrap();
    stage.add_payload_with_offset(lazy, &anim, "/Anim", 5.0, 1.0).unwrap();
    assert!(stage.add_reference_with_offset(late, &anim, "", 0.0, 0.0).is_err());
    // Stage time t reads the asset at (t - offset) / scale.
    assert_eq!(get(&stage, late, "v", Some(25.0)).as_scalar::<f64>(), Some(100.0));
    assert_eq!(get(&stage, lazy, "v", Some(15.0)).as_scalar::<f64>(), Some(100.0));
    assert_eq!(stage.time_samples(late, "v").unwrap(), [5.0, 25.0]);
    stage.remove_payload(lazy, &anim, "/Anim").unwrap();
    assert!(stage.attribute_info(lazy, "v").unwrap().is_none());
}

#[test]
fn host_saves_skip_unchanged_layers() {
    #[derive(Default)]
    struct Counting {
        fs: MemoryFs,
        writes: RefCell<usize>,
    }
    impl AssetResolver for Counting {
        fn resolve(&self, asset_path: &str, anchor: Option<&str>) -> Option<String> {
            self.fs.resolve(asset_path, anchor)
        }
        fn read(&self, resolved_path: &str) -> std::io::Result<Vec<u8>> {
            self.fs.read(resolved_path)
        }
        fn write(&self, resolved_path: &str, bytes: &[u8]) -> std::io::Result<()> {
            *self.writes.borrow_mut() += 1;
            self.fs.write(resolved_path, bytes)
        }
    }
    let host = Rc::new(Counting::default());
    let options = OpenOptions {
        resolver: Some(host.clone()),
        ..OpenOptions::default()
    };
    let mut stage = Stage::create_new("mem://scene.usda", &options).unwrap();
    let start = *host.writes.borrow();
    build_quad(&mut stage);
    stage.save().unwrap();
    assert_eq!(*host.writes.borrow(), start + 1);
    stage.save().unwrap();
    assert_eq!(
        *host.writes.borrow(),
        start + 1,
        "nothing changed, so nothing is written"
    );
    let world = find(&mut stage, "/World");
    stage.set_kind(world, "assembly").unwrap();
    stage.save().unwrap();
    assert_eq!(*host.writes.borrow(), start + 2);
}

#[test]
fn preview_materials() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let mesh = build_quad(&mut stage);
    let material = stage.define_preview_material("/World/Looks/Red").unwrap();
    let red = Value::from_slice(BaseType::Float3, false, &[[1.0f32, 0.0, 0.0]]).unwrap();
    stage.set_preview_input(material, "diffuseColor", &red).unwrap();
    stage
        .set_preview_input(material, "roughness", &Value::float(0.25))
        .unwrap();
    assert!(
        stage
            .set_preview_input(material, "roughness", &Value::double(0.25))
            .is_err()
    );
    assert!(
        stage
            .set_preview_input(material, "shininess", &Value::float(1.0))
            .is_err()
    );
    let texture = stage.set_preview_texture(material, "normal", "./normal.png").unwrap();
    stage.set_preview_texture(material, "metallic", "./metal.png").unwrap();
    assert_eq!(stage.path(texture).unwrap(), "/World/Looks/Red/NormalTexture");
    stage.bind_material(mesh, material, "").unwrap();
    assert_eq!(Stage::preview_input_type("diffuseColor"), Some("color3f"));

    let shader = find(&mut stage, "/World/Looks/Red/PreviewSurface");
    assert_eq!(get(&stage, shader, "info:id", None).strings(), ["UsdPreviewSurface"]);
    assert_eq!(
        stage.connections(material, "outputs:surface").unwrap(),
        ["/World/Looks/Red/PreviewSurface.outputs:surface"]
    );
    assert_eq!(
        stage.connections(shader, "inputs:normal").unwrap(),
        ["/World/Looks/Red/NormalTexture.outputs:rgb"]
    );
    assert_eq!(
        stage.connections(shader, "inputs:metallic").unwrap(),
        ["/World/Looks/Red/MetallicTexture.outputs:r"]
    );
    let reader = find(&mut stage, "/World/Looks/Red/TexCoordReader");
    let varname = get(&stage, reader, "inputs:varname", None);
    assert_eq!(
        (varname.type_name(), varname.strings()),
        ("string", &["st".to_owned()][..])
    );
    assert_eq!(
        get(&stage, texture, "inputs:bias", None)
            .as_slice::<[f32; 4]>()
            .unwrap(),
        &[[-1.0, -1.0, -1.0, 0.0]]
    );
    let bound = stage.bound_material(mesh, "").unwrap();
    assert!(bound.is_some());

    let plain = stage.define_prim("/World/Looks/Plain", "Material").unwrap();
    assert!(stage.set_preview_input(plain, "roughness", &Value::float(0.5)).is_err());
}

#[test]
fn batches_queue_schemas_arcs_and_removals() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let proto = stage.define_prim("/Proto", "Sphere").unwrap();
    stage.create_attribute(proto, "old", "float", false).unwrap();
    stage.set(proto, "old", &Value::float(1.0), None).unwrap();
    stage.begin_edit().unwrap();
    let body = stage.define_prim("/Body", "Cube").unwrap();
    stage.apply_api(body, "PhysicsMassAPI").unwrap();
    // The applied schema declares physics:mass, so it can be set in the same batch.
    stage.set(body, "physics:mass", &Value::float(3.0), None).unwrap();
    stage.apply_api(body, "PhysicsDriveAPI:angular").unwrap();
    stage
        .set(body, "drive:angular:physics:stiffness", &Value::float(5.0), None)
        .unwrap();
    stage.set_kind(body, "component").unwrap();
    stage
        .set_metadata(body, "customData:note", &Value::string("queued"))
        .unwrap();
    let inst = stage.define_prim("/Inst", "").unwrap();
    stage.add_reference(inst, "", "/Proto").unwrap();
    assert!(
        !stage.remove_property(proto, "old").unwrap(),
        "nothing is removed until the commit"
    );
    stage.create_attribute(body, "seq", "float", false).unwrap();
    stage.set(body, "seq", &Value::float(1.0), Some(0.0)).unwrap();
    stage.remove_property(body, "seq").unwrap();
    stage.create_attribute(body, "seq", "float", false).unwrap();
    stage.set(body, "seq", &Value::float(2.0), Some(1.0)).unwrap();
    assert!(
        !stage.has_api(body, "PhysicsMassAPI").unwrap(),
        "reads see the stage before the batch"
    );
    stage.commit_edit().unwrap();

    assert!(stage.has_api(body, "PhysicsMassAPI").unwrap());
    assert_eq!(get(&stage, body, "physics:mass", None).as_scalar::<f32>(), Some(3.0));
    assert_eq!(
        get(&stage, body, "drive:angular:physics:stiffness", None).as_scalar::<f32>(),
        Some(5.0)
    );
    assert_eq!(stage.kind(body).unwrap(), "component");
    assert_eq!(get(&stage, inst, "radius", None).type_name(), "double");
    assert!(stage.attribute_info(proto, "old").unwrap().is_none());
    assert_eq!(
        stage.time_samples(body, "seq").unwrap(),
        [1.0],
        "edits after a deferred one keep their order"
    );
}

#[test]
fn a_failed_deferred_edit_rolls_the_batch_back() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let keep = stage.define_prim("/Keep", "Xform").unwrap();
    let before = stage
        .export_bytes(Some(&stage.edit_target()), LayerFormat::Usda)
        .unwrap();
    let err = stage
        .edit(|s| {
            let a = s.define_prim("/A", "Xform")?;
            s.create_attribute(a, "v", "float", false)?;
            s.set(a, "v", &Value::float(1.0), None)?;
            s.apply_api(keep, "PhysicsRigidBodyAPI")?;
            s.set_kind(keep, "assembly")?;
            // Queued fine, but fails at commit: the attribute is never declared.
            s.set_connections(keep, "inputs:missing", &["/A.v"])?;
            s.define_prim("/B", "Xform").map(drop)
        })
        .unwrap_err();
    assert!(matches!(err, Error::InvalidValue(_)), "{err}");
    assert!(stage.prim_at_path("/A").unwrap().is_none());
    assert!(stage.prim_at_path("/B").unwrap().is_none());
    assert!(!stage.has_api(keep, "PhysicsRigidBodyAPI").unwrap());
    assert_eq!(stage.kind(keep).unwrap(), "");
    let after = stage
        .export_bytes(Some(&stage.edit_target()), LayerFormat::Usda)
        .unwrap();
    assert_eq!(String::from_utf8(before).unwrap(), String::from_utf8(after).unwrap());
}

fn translation(x: f64, y: f64, z: f64) -> [f64; 16] {
    let mut m = [0.0; 16];
    for i in 0..4 {
        m[i * 5] = 1.0;
    }
    m[12..15].copy_from_slice(&[x, y, z]);
    m
}

/// A rotation of `degrees` about Z, scaled by `s`, at `t`.
fn turned(degrees: f64, s: f64, t: [f64; 3]) -> [f64; 16] {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let mut m = [0.0; 16];
    m[0] = cos * s;
    m[1] = sin * s;
    m[4] = -sin * s;
    m[5] = cos * s;
    m[10] = s;
    m[12..15].copy_from_slice(&t);
    m[15] = 1.0;
    m
}

fn near(a: &[f64], b: &[f64], tolerance: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tolerance)
}

#[test]
fn point_instancers_round_trip() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let tree = stage.define_prim("/World/Protos/Tree", "Cone").unwrap();
    let rock = stage.define_prim("/World/Protos/Rock", "Sphere").unwrap();
    let field = stage.define_prim("/World/Field", "").unwrap();
    let written = Instances {
        prototypes: vec![tree, rock],
        proto_indices: vec![0, 1, 0],
        ids: vec![10, 11, 12],
        transforms: vec![
            translation(1.0, 0.0, 0.0),
            turned(90.0, 2.0, [0.0, 3.0, 0.0]),
            turned(-45.0, 0.5, [5.0, 5.0, 1.0]),
        ],
    };
    stage.set_instances(field, &written, None).unwrap();
    assert_eq!(stage.type_name(field).unwrap(), "PointInstancer");
    let read = stage.instancer_instances(field, None, false, true).unwrap().unwrap();
    assert_eq!(read.prototypes, written.prototypes);
    assert_eq!(read.proto_indices, written.proto_indices);
    assert_eq!(read.ids, written.ids);
    for (a, b) in read.transforms.iter().zip(&written.transforms) {
        assert!(near(a, b, 1e-5), "{a:?} != {b:?}");
    }

    let moved = Instances {
        transforms: vec![translation(0.0, 0.0, 9.0); 3],
        ids: Vec::new(),
        ..written.clone()
    };
    stage.set_instances(field, &moved, Some(4.0)).unwrap();
    let later = stage
        .instancer_instances(field, Some(4.0), false, true)
        .unwrap()
        .unwrap();
    assert_eq!(later.transforms[2][14], 9.0);

    let mut bad = written.clone();
    bad.proto_indices[1] = 7;
    assert!(stage.set_instances(field, &bad, None).is_err());
    let mut sheared = written.clone();
    sheared.transforms[0][4] = 0.5;
    assert!(stage.set_instances(field, &sheared, None).is_err());
    assert!(
        stage.set_instances(tree, &written, None).is_err(),
        "a Cone isn't an instancer"
    );
}

#[cfg(feature = "skel")]
#[test]
fn skeletons_animations_and_skinning_round_trip() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let root = stage.define_prim("/Character", "SkelRoot").unwrap();
    let joints = ["hips", "hips/spine"];
    let bind = [translation(0.0, 0.0, 0.0), translation(0.0, 1.0, 0.0)];
    let rest = [translation(0.0, 0.0, 0.0), translation(0.0, 1.0, 0.0)];
    let skeleton = stage.define_skeleton("/Character/Skel", &joints, &bind, &rest).unwrap();
    let anim = stage.define_prim("/Character/Skel/Walk", "").unwrap();
    let posed = [translation(0.0, 0.0, 0.0), turned(90.0, 1.0, [0.0, 1.0, 0.0])];
    stage.set_skel_animation(anim, &joints, &posed, Some(1.0)).unwrap();
    assert_eq!(stage.type_name(anim).unwrap(), "SkelAnimation");

    let mesh = stage.define_prim("/Character/Body", "Mesh").unwrap();
    let points = [[0.0f32, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 2.0, 0.0]];
    stage.set_mesh(mesh, &points, &[3], &[0, 1, 2], None).unwrap();
    stage
        .set_skinning(
            mesh,
            &[0, 1, 0, 1, 1, 0],
            &[1.0, 0.0, 0.5, 0.5, 1.0, 0.0],
            2,
            Some(&translation(0.0, 0.0, 0.0)),
            &[],
        )
        .unwrap();
    stage.bind_skeleton(root, skeleton, Some(anim)).unwrap();
    assert!(stage.set_skinning(mesh, &[0, 1], &[1.0], 2, None, &[]).is_err());
    assert!(
        stage
            .set_skinning(mesh, &[0, 1, 0, 1], &[1.0; 4], 2, None, &[])
            .is_err(),
        "neither per point nor rigid"
    );
    assert!(
        stage
            .define_skeleton("/Bad", &["hips/spine", "hips"], &bind, &[])
            .is_err(),
        "child before parent"
    );

    let pose = stage.skeleton_pose(skeleton, Some(1.0)).unwrap().unwrap();
    assert_eq!(pose.joints, joints);
    assert_eq!(pose.parents, [-1, 0]);
    assert!(near(&pose.local[1], &posed[1], 1e-3), "{:?}", pose.local[1]);
    let skin = stage.skin_binding(mesh, Some(1.0)).unwrap().unwrap();
    assert_eq!(skin.influences_per_point, 2);
    assert_eq!(skin.joint_indices, [0, 1, 0, 1, 1, 0]);
    let skinned = stage.skinned_points(mesh, Some(1.0)).unwrap().unwrap();
    // The third point follows the spine, turned 90 degrees about its joint at y = 1.
    assert!(near(&skinned[2].map(f64::from), &[-1.0, 1.0, 0.0], 1e-3), "{skinned:?}");
    assert!(near(&skinned[0].map(f64::from), &[0.0, 0.0, 0.0], 1e-6));

    let rigid = stage.define_prim("/Character/Hat", "Mesh").unwrap();
    stage.set_mesh(rigid, &points, &[3], &[0, 1, 2], None).unwrap();
    stage.set_skinning(rigid, &[1], &[1.0], 1, None, &[]).unwrap();
    assert!(stage.skin_binding(rigid, Some(1.0)).unwrap().unwrap().rigid);
}

#[test]
fn value_clips_play_their_layers() {
    let dir = tempfile::tempdir().unwrap();
    let options = OpenOptions::default();
    for (i, base) in [(1, 0.0), (2, 100.0)] {
        let path = dir.path().join(format!("clip{i}.usda")).to_string_lossy().into_owned();
        let mut clip = Stage::create_new(&path, &options).unwrap();
        let anim = clip.define_prim("/Anim", "Xform").unwrap();
        clip.create_attribute(anim, "v", "double", false).unwrap();
        for t in 0..=10 {
            clip.set(anim, "v", &Value::double(base + f64::from(t)), Some(f64::from(t)))
                .unwrap();
        }
        clip.save().unwrap();
    }
    let main = dir.path().join("main.usda").to_string_lossy().into_owned();
    let mut stage = Stage::create_new(&main, &options).unwrap();
    let thing = stage.define_prim("/Thing", "Xform").unwrap();
    // Clips give values to the attributes the scene declares.
    stage.create_attribute(thing, "v", "double", false).unwrap();
    stage
        .set_value_clips(
            thing,
            "default",
            &["./clip1.usda", "./clip2.usda"],
            "/Anim",
            &[[0.0, 0.0], [10.0, 1.0]],
            &[[0.0, 0.0], [10.0, 10.0], [10.0, 0.0], [20.0, 10.0]],
            "./manifest.usda",
        )
        .unwrap();
    let manifest = dir.path().join("manifest.usda").to_string_lossy().into_owned();
    stage.export_clip_manifest(thing, "default", &manifest).unwrap();
    stage.save().unwrap();
    assert!(std::fs::read_to_string(&manifest).unwrap().contains("double v"));
    assert!(
        stage
            .set_value_clips(thing, "default", &["./clip1.usda"], "/Anim", &[[0.0, 3.0]], &[], "")
            .is_err(),
        "clip 3 doesn't exist"
    );

    let mut stage = Stage::open(&main, &options).unwrap();
    let thing = find(&mut stage, "/Thing");
    let mut v = Value::new();
    assert!(stage.get(thing, "v", Some(5.0), &mut v).unwrap());
    assert_eq!(v.as_scalar::<f64>(), Some(5.0));
    assert!(stage.get(thing, "v", Some(15.0), &mut v).unwrap());
    assert_eq!(v.as_scalar::<f64>(), Some(105.0));
}

fn snapshot(stage: &mut Stage) -> Vec<String> {
    let flags = TraverseFlags::INSTANCE_PROXIES;
    let mut out = Vec::new();
    for prim in stage.traverse(flags).unwrap() {
        let path = stage.path(prim).unwrap().to_owned();
        let type_name = stage.type_name(prim).unwrap();
        let mut line = format!("{path} {type_name}");
        for name in stage.attribute_names(prim, true).unwrap() {
            let mut v = Value::new();
            // Asset paths are rewritten for the export's own location.
            if stage.get(prim, &name, None, &mut v).unwrap() && v.base_type() != BaseType::Asset {
                line.push_str(&format!(" {name}={:?}{:?}", v.bytes(), v.strings()));
            }
        }
        out.push(line);
    }
    out
}

#[test]
fn flattening_keeps_instances() {
    let mut stage = Stage::open(&data("query.usda"), &OpenOptions::default()).unwrap();
    assert!(!stage.prototypes().unwrap().is_empty());
    let before = snapshot(&mut stage);
    let bytes = stage.export_bytes(None, LayerFormat::Usda).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert_eq!(text.matches("\"Flattened_Prototype_1\"").count(), 1, "{text}");
    assert!(text.contains("class Mesh \"Flattened_Prototype_1\""), "{text}");
    assert!(text.contains("</Flattened_Prototype_1>"), "{text}");

    let dir = tempfile::tempdir().unwrap();
    let flat = dir.path().join("flat.usdc").to_string_lossy().into_owned();
    stage.export(&flat).unwrap();
    let mut reopened = Stage::open(&flat, &OpenOptions::default()).unwrap();
    assert_eq!(reopened.prototypes().unwrap().len(), stage.prototypes().unwrap().len());
    assert_eq!(snapshot(&mut reopened), before);
}

#[test]
fn edits_in_a_batch_include_the_new_helpers() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let field = stage.define_prim("/Field", "PointInstancer").unwrap();
    let proto = stage.define_prim("/Field/Proto", "Cube").unwrap();
    stage
        .edit(|s| {
            for frame in 0..3 {
                let instances = Instances {
                    prototypes: vec![proto],
                    proto_indices: vec![0, 0],
                    ids: Vec::new(),
                    transforms: vec![translation(f64::from(frame), 0.0, 0.0), translation(0.0, 1.0, 0.0)],
                };
                s.set_instances(field, &instances, Some(f64::from(frame)))?;
            }
            Ok(())
        })
        .unwrap();
    let read = stage
        .instancer_instances(field, Some(2.0), false, true)
        .unwrap()
        .unwrap();
    assert_eq!(read.transforms[0][12], 2.0);
    let mut v = Value::new();
    assert!(stage.get(field, "positions", Some(1.0), &mut v).unwrap());
    assert_eq!(v.base_type(), BaseType::Float3);
}

#[cfg(feature = "skel")]
#[test]
fn blend_shapes_round_trip() {
    let mut stage = Stage::create_in_memory(&OpenOptions::default()).unwrap();
    let root = stage.define_prim("/Face", "SkelRoot").unwrap();
    let identity = translation(0.0, 0.0, 0.0);
    let skeleton = stage
        .define_skeleton("/Face/Skel", &["head"], &[identity], &[identity])
        .unwrap();
    let anim = stage.define_prim("/Face/Skel/Talk", "").unwrap();
    stage.set_skel_animation(anim, &["head"], &[identity], None).unwrap();
    let mesh = stage.define_prim("/Face/Mesh", "Mesh").unwrap();
    let points = [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    stage.set_mesh(mesh, &points, &[3], &[0, 1, 2], None).unwrap();
    stage.set_skinning(mesh, &[0], &[1.0], 1, None, &[]).unwrap();
    stage.bind_skeleton(root, skeleton, Some(anim)).unwrap();

    let smile = stage
        .define_blend_shape("/Face/Mesh/Smile", &[[0.0, 1.0, 0.0]], &[], &[2])
        .unwrap();
    stage
        .set_blend_shape_inbetween(smile, "half", 0.5, &[[0.0, 0.25, 0.0]], &[])
        .unwrap();
    let blink = stage
        .define_blend_shape("/Face/Mesh/Blink", &[[0.0, 0.0, 1.0]; 3], &[[0.0, 0.0, 0.0]; 3], &[])
        .unwrap();
    stage
        .set_blend_shapes(mesh, &["smile", "blink"], &[smile, blink])
        .unwrap();
    stage
        .set_blend_shape_weights(anim, &["blink", "smile"], &[0.0, 0.5], Some(1.0))
        .unwrap();

    assert!(stage.define_blend_shape("/Bad", &[[0.0; 3]], &[], &[0, 1]).is_err());
    assert!(
        stage
            .set_blend_shape_inbetween(smile, "full", 1.0, &[[0.0; 3]], &[])
            .is_err()
    );
    assert!(stage.set_blend_shapes(mesh, &["a", "a"], &[smile, blink]).is_err());
    assert!(stage.set_blend_shape_weights(anim, &["a"], &[], None).is_err());

    let skin = stage.skin_binding(mesh, Some(1.0)).unwrap().unwrap();
    assert_eq!(skin.blend_shapes, ["smile", "blink"]);
    assert_eq!(skin.blend_shape_weights, [0.5, 0.0], "weights follow the mesh's order");
    let shapes = stage.blend_shapes(mesh, Some(1.0)).unwrap().unwrap();
    assert_eq!(shapes.point_indices, [vec![2], vec![]]);
    let inbetween = shapes
        .sub_shapes
        .iter()
        .find(|s| s.blend_shape == 0 && s.point_offsets == [[0.0, 0.25, 0.0]])
        .expect("the inbetween is a sub-shape of smile");
    assert_eq!(inbetween.weight, 1.0, "smile at 0.5 sits exactly on its inbetween");
    let skinned = stage.skinned_points(mesh, Some(1.0)).unwrap().unwrap();
    assert!(near(&skinned[2].map(f64::from), &[0.0, 1.25, 0.0], 1e-5), "{skinned:?}");
}
