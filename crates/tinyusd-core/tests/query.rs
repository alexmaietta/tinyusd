use tinyusd_core::{BaseType, Error, OpenOptions, Prim, PrimFlags, Specifier, Stage, TraverseFlags, Value};

fn data(name: &str) -> String {
    format!("{}/../../tests/data/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn open(name: &str) -> Stage {
    Stage::open(&data(name), &OpenOptions::default()).unwrap()
}

fn prim(stage: &mut Stage, path: &str) -> Prim {
    stage
        .prim_at_path(path)
        .unwrap()
        .unwrap_or_else(|| panic!("missing prim {path}"))
}

fn paths(stage: &Stage, prims: &[Prim]) -> Vec<String> {
    prims.iter().map(|&p| stage.path(p).unwrap().to_owned()).collect()
}

fn get(stage: &Stage, p: Prim, name: &str) -> Value {
    let mut v = Value::new();
    assert!(stage.get(p, name, None, &mut v).unwrap(), "{name} has no value");
    v
}

fn for_each_format(f: impl Fn(&str)) {
    for name in ["query.usda", "query.usdc"] {
        f(name);
    }
}

#[test]
fn stage_metadata() {
    for_each_format(|file| {
        let mut stage = open(file);
        let mut v = Value::new();
        assert!(stage.stage_metadata("upAxis", &mut v).unwrap());
        assert_eq!((v.base_type(), v.strings()), (BaseType::Token, &["Z".to_owned()][..]));
        assert!(stage.stage_metadata("metersPerUnit", &mut v).unwrap());
        assert_eq!(v.as_scalar::<f64>(), Some(0.01));
        assert!(stage.stage_metadata("customLayerData", &mut v).unwrap());
        assert_eq!(v.base_type(), BaseType::Unsupported);
        assert!(!stage.stage_metadata("doesNotExist", &mut v).unwrap());
        assert!(stage.has_authored_stage_metadata("upAxis").unwrap());
        assert!(!stage.has_authored_stage_metadata("framesPerSecond").unwrap());
        assert_eq!(stage.start_time_code(), 1.0);
        assert_eq!(stage.end_time_code(), 10.0);
        assert_eq!(stage.time_codes_per_second(), 24.0);
        let default = stage.default_prim().unwrap().unwrap();
        assert_eq!(stage.path(default).unwrap(), "/World");
        assert!(stage.composition_errors().is_empty());
    });
}

#[test]
fn traversal_filters() {
    for_each_format(|file| {
        let mut stage = open(file);
        let prims = stage.traverse(TraverseFlags::DEFAULT).unwrap();
        assert_eq!(
            paths(&stage, &prims),
            [
                "/World",
                "/World/Types",
                "/World/Mesh",
                "/World/Mat",
                "/World/Mat/Shader",
                "/World/Variant",
                "/World/Variant/Geom",
                "/World/Instance1",
                "/World/Instance2",
                "/World/Payloaded",
            ]
        );
        let all = stage.traverse(TraverseFlags::ALL).unwrap();
        let all = paths(&stage, &all);
        for extra in ["/_Base", "/World/OverOnly", "/World/Inactive"] {
            assert!(all.iter().any(|p| p == extra), "{extra} missing from {all:?}");
        }
        assert!(!all.iter().any(|p| p == "/World/Inactive/Child"));
        let world = prim(&mut stage, "/World");
        let mat = prim(&mut stage, "/World/Mat");
        let sub = stage.traverse_from(mat, TraverseFlags::DEFAULT).unwrap();
        assert_eq!(paths(&stage, &sub), ["/World/Mat", "/World/Mat/Shader"]);
        assert_eq!(stage.children(world, TraverseFlags::DEFAULT).unwrap().len(), 7);
        let shader = prim(&mut stage, "/World/Mat/Shader");
        assert_eq!(stage.parent(shader).unwrap(), Some(mat));
        assert_eq!(stage.name(shader).unwrap(), "Shader");
        assert_eq!(stage.parent(stage.pseudo_root()).unwrap(), None);
    });
}

#[test]
fn prim_queries() {
    for_each_format(|file| {
        let mut stage = open(file);
        let world = prim(&mut stage, "/World");
        let mesh = prim(&mut stage, "/World/Mesh");
        assert_eq!(stage.type_name(mesh).unwrap(), "Mesh");
        assert_eq!(stage.kind(world).unwrap(), "assembly");
        assert!(stage.is_a(mesh, "Mesh").unwrap());
        assert!(stage.is_a(mesh, "Gprim").unwrap());
        assert!(!stage.is_a(mesh, "Xform").unwrap());
        assert!(stage.has_api(mesh, "PhysicsCollisionAPI").unwrap());
        assert!(stage.has_api(mesh, "SemanticsLabelsAPI").unwrap());
        assert!(stage.has_api(mesh, "SemanticsLabelsAPI:class").unwrap());
        assert!(!stage.has_api(mesh, "SemanticsLabelsAPI:other").unwrap());
        let schemas = stage.applied_schemas(mesh).unwrap();
        assert!(schemas.contains(&"SemanticsLabelsAPI:class".to_owned()), "{schemas:?}");
        assert!(schemas.contains(&"PhysicsCollisionAPI".to_owned()), "{schemas:?}");
        let flags = stage.flags(world).unwrap();
        assert!(flags.contains(PrimFlags::ACTIVE) && flags.contains(PrimFlags::DEFINED));
        assert!(flags.contains(PrimFlags::MODEL));
        let base = prim(&mut stage, "/_Base");
        assert_eq!(stage.specifier(base).unwrap(), Specifier::Class);
        assert!(stage.flags(base).unwrap().contains(PrimFlags::ABSTRACT));
        let over = prim(&mut stage, "/World/OverOnly");
        assert_eq!(stage.specifier(over).unwrap(), Specifier::Over);
        let inactive = prim(&mut stage, "/World/Inactive");
        assert!(!stage.flags(inactive).unwrap().contains(PrimFlags::ACTIVE));
        let geom = prim(&mut stage, "/World/Variant/Geom");
        assert_eq!(stage.type_name(geom).unwrap(), "Cube");
        let names = stage.attribute_names(mesh, true).unwrap();
        assert!(names.contains(&"points".to_owned()), "{names:?}");
        assert!(!names.contains(&"doubleSided".to_owned()));
        assert!(
            stage
                .attribute_names(mesh, false)
                .unwrap()
                .contains(&"doubleSided".to_owned())
        );
        assert!(stage.prim_at_path("/World/Nope").unwrap().is_none());
        assert!(matches!(stage.prim_at_path("not a path"), Err(Error::InvalidPath(_))));
    });
}

#[test]
fn scalar_and_vector_values() {
    for_each_format(|file| {
        let mut stage = open(file);
        let p = prim(&mut stage, "/World/Types");
        let v = get(&stage, p, "b");
        assert_eq!((v.base_type(), v.as_scalar::<u8>()), (BaseType::Bool, Some(1)));
        assert_eq!(get(&stage, p, "uc").as_scalar::<u8>(), Some(200));
        assert_eq!(get(&stage, p, "i").as_scalar::<i32>(), Some(-3));
        assert_eq!(get(&stage, p, "ui").as_scalar::<u32>(), Some(4_000_000_000));
        assert_eq!(get(&stage, p, "i64").as_scalar::<i64>(), Some(-9_000_000_000));
        assert_eq!(
            get(&stage, p, "u64").as_scalar::<u64>(),
            Some(18_000_000_000_000_000_000)
        );
        let h = get(&stage, p, "h");
        assert_eq!((h.base_type(), h.as_scalar::<u16>()), (BaseType::Half, Some(0x3e00)));
        assert_eq!(get(&stage, p, "f").as_scalar::<f32>(), Some(2.5));
        assert_eq!(get(&stage, p, "d").as_scalar::<f64>(), Some(3.25));
        let tc = get(&stage, p, "tc");
        assert_eq!(
            (tc.base_type(), tc.as_scalar::<f64>()),
            (BaseType::TimeCode, Some(12.0))
        );
        let s = get(&stage, p, "s");
        assert_eq!(
            (s.base_type(), s.strings()),
            (BaseType::String, &["hello".to_owned()][..])
        );
        assert_eq!(get(&stage, p, "t").strings(), ["tok"]);
        assert_eq!(get(&stage, p, "i2").as_slice::<i32>(), Some(&[1, 2][..]));
        assert_eq!(get(&stage, p, "i3").as_slice::<i32>(), Some(&[1, 2, 3][..]));
        assert_eq!(get(&stage, p, "i4").as_slice::<i32>(), Some(&[1, 2, 3, 4][..]));
        let h3 = get(&stage, p, "h3");
        assert_eq!(
            (h3.base_type(), h3.as_slice::<u16>()),
            (BaseType::Half3, Some(&[0x3800, 0x3c00, 0x4000][..]))
        );
        assert_eq!(get(&stage, p, "f2").as_slice::<f32>(), Some(&[1.0, 2.0][..]));
        assert_eq!(get(&stage, p, "f3").as_slice::<f32>(), Some(&[1.0, 2.0, 3.0][..]));
        assert_eq!(get(&stage, p, "f4").as_slice::<f32>(), Some(&[1.0, 2.0, 3.0, 4.0][..]));
        assert_eq!(get(&stage, p, "d3").as_slice::<f64>(), Some(&[1.0, 2.0, 3.0][..]));
        assert_eq!(get(&stage, p, "d4").as_slice::<f64>(), Some(&[1.0, 2.0, 3.0, 4.0][..]));
        assert_eq!(get(&stage, p, "qf").as_slice::<f32>(), Some(&[0.5, 0.5, 0.5, 0.5][..]));
        assert_eq!(get(&stage, p, "qd").as_slice::<f64>(), Some(&[1.0, 0.0, 0.0, 0.0][..]));
        assert_eq!(get(&stage, p, "qh").base_type(), BaseType::Quath);
        assert_eq!(get(&stage, p, "m2").as_slice::<f64>(), Some(&[1.0, 2.0, 3.0, 4.0][..]));
        assert_eq!(get(&stage, p, "m3").as_slice::<f64>().map(<[f64]>::len), Some(9));
        let m4 = get(&stage, p, "m4");
        assert_eq!(m4.base_type(), BaseType::Matrix4d);
        assert_eq!(&m4.as_slice::<f64>().unwrap()[12..15], &[5.0, 6.0, 7.0]);
        assert_eq!(get(&stage, p, "baseValue").as_scalar::<i32>(), Some(7));
    });
}

#[test]
fn array_values_and_type_names() {
    for_each_format(|file| {
        let mut stage = open(file);
        let p = prim(&mut stage, "/World/Types");
        let fa = get(&stage, p, "fa");
        assert!(fa.is_array());
        assert_eq!((fa.len(), fa.as_slice::<f32>()), (3, Some(&[1.0, 2.0, 3.0][..])));
        assert_eq!(fa.as_scalar::<f32>(), None);
        let pts = get(&stage, p, "pts");
        assert_eq!(
            (pts.base_type(), pts.len(), pts.type_name()),
            (BaseType::Float3, 2, "point3f[]")
        );
        assert_eq!(
            pts.as_slice::<[f32; 3]>(),
            Some(&[[0.0, 0.0, 0.0], [1.0, 2.0, 3.0]][..])
        );
        assert_eq!(get(&stage, p, "ta").strings(), ["a", "b"]);
        assert_eq!(get(&stage, p, "sa").strings(), ["x"]);
        assert_eq!(get(&stage, p, "ba").as_slice::<u8>(), Some(&[1, 0, 1][..]));
        let info = stage.attribute_info(p, "pts").unwrap().unwrap();
        assert_eq!(info.type_name, "point3f[]");
        assert!(info.custom && info.has_value && info.has_authored_value && !info.uniform);
        assert!(stage.attribute_info(p, "nope").unwrap().is_none());
    });
}

#[test]
fn assets_resolve_relative_to_their_layer() {
    for_each_format(|file| {
        let mut stage = open(file);
        let p = prim(&mut stage, "/World/Types");
        let a = get(&stage, p, "a");
        assert_eq!(
            (a.base_type(), a.strings()),
            (BaseType::Asset, &["./part.usda".to_owned()][..])
        );
        let expected = std::fs::canonicalize(data("part.usda")).unwrap();
        assert_eq!(std::fs::canonicalize(&a.resolved_paths()[0]).unwrap(), expected);
        let aa = get(&stage, p, "aa");
        assert_eq!(aa.strings(), ["./part.usda", "missing.png"]);
        assert!(aa.resolved_paths()[0].ends_with("part.usda"));
        assert_eq!(aa.resolved_paths()[1], "");
    });
}

#[test]
fn time_samples_and_blocks() {
    for_each_format(|file| {
        let mut stage = open(file);
        let p = prim(&mut stage, "/World/Types");
        assert_eq!(stage.time_samples(p, "animated").unwrap(), [1.0, 10.0]);
        let mut v = Value::new();
        assert!(stage.get(p, "animated", Some(5.5), &mut v).unwrap());
        assert_eq!(v.as_scalar::<f32>(), Some(4.5));
        assert!(stage.get(p, "animated", Some(100.0), &mut v).unwrap());
        assert_eq!(v.as_scalar::<f32>(), Some(9.0));
        assert!(
            stage
                .attribute_info(p, "animated")
                .unwrap()
                .unwrap()
                .might_be_time_varying
        );
        assert!(!stage.get(p, "blocked", None, &mut v).unwrap());
        assert!(v.is_empty());
        assert!(!stage.get(p, "doesNotExist", None, &mut v).unwrap());
    });
}

#[test]
fn metadata_relationships_and_connections() {
    for_each_format(|file| {
        let mut stage = open(file);
        let p = prim(&mut stage, "/World/Types");
        let mut v = Value::new();
        assert!(
            stage
                .attribute_metadata(p, "primvars:displayColor", "interpolation", &mut v)
                .unwrap()
        );
        assert_eq!(v.strings(), ["constant"]);
        let world = prim(&mut stage, "/World");
        assert!(stage.prim_metadata(world, "kind", &mut v).unwrap());
        assert_eq!(v.strings(), ["assembly"]);
        assert_eq!(stage.targets(p, "target", false).unwrap(), ["/World/Mesh"]);
        assert_eq!(stage.targets(p, "forward", false).unwrap(), ["/World/Types.target"]);
        assert_eq!(stage.targets(p, "forward", true).unwrap(), ["/World/Mesh"]);
        assert!(
            stage
                .relationship_names(p, true)
                .unwrap()
                .contains(&"target".to_owned())
        );
        let mat = prim(&mut stage, "/World/Mat");
        assert_eq!(
            stage.connections(mat, "outputs:surface").unwrap(),
            ["/World/Mat/Shader.outputs:surface"]
        );
        let shader = prim(&mut stage, "/World/Mat/Shader");
        let info = stage.attribute_info(shader, "info:id").unwrap().unwrap();
        assert!(info.uniform);
        assert_eq!(
            get(&stage, shader, "inputs:diffuseColor").as_slice::<f32>(),
            Some(&[1.0, 0.0, 0.0][..])
        );
    });
}

#[test]
fn instancing() {
    for_each_format(|file| {
        let mut stage = open(file);
        let i1 = prim(&mut stage, "/World/Instance1");
        let i2 = prim(&mut stage, "/World/Instance2");
        assert!(stage.flags(i1).unwrap().contains(PrimFlags::INSTANCE));
        let proto = stage.prototype(i1).unwrap().unwrap();
        assert_eq!(stage.prototype(i2).unwrap(), Some(proto));
        assert_eq!(stage.prototypes().unwrap(), [proto]);
        assert!(stage.flags(proto).unwrap().contains(PrimFlags::PROTOTYPE));
        let mut instances = stage.instances(proto).unwrap();
        instances.sort();
        assert_eq!(instances, [i1, i2]);
        assert_eq!(stage.type_name(i1).unwrap(), "Mesh");
    });
}

#[test]
fn payload_loading() {
    let mut loaded = open("query.usda");
    let p = prim(&mut loaded, "/World/Payloaded");
    assert_eq!(loaded.type_name(p).unwrap(), "Mesh");
    assert!(loaded.flags(p).unwrap().contains(PrimFlags::LOADED));

    let mut unloaded = Stage::open(
        &data("query.usda"),
        &OpenOptions {
            load_payloads: false,
            ..Default::default()
        },
    )
    .unwrap();
    let p = prim(&mut unloaded, "/World/Payloaded");
    assert!(!unloaded.flags(p).unwrap().contains(PrimFlags::LOADED));
    assert_eq!(unloaded.type_name(p).unwrap(), "");
    let visible = unloaded.traverse(TraverseFlags::DEFAULT).unwrap();
    assert!(!paths(&unloaded, &visible).contains(&"/World/Payloaded".to_owned()));
    let with_unloaded = unloaded.traverse(TraverseFlags::INCLUDE_UNLOADED).unwrap();
    assert!(paths(&unloaded, &with_unloaded).contains(&"/World/Payloaded".to_owned()));
}

#[test]
fn reads_assets_inside_usdz() {
    let mut stage = open("textured.usdz");
    let texture = prim(&mut stage, "/Root/Texture");
    let file = get(&stage, texture, "inputs:file");
    assert_eq!(file.strings(), ["./tex.png"]);
    let resolved = &file.resolved_paths()[0];
    assert!(resolved.ends_with("textured.usdz[tex.png]"), "{resolved}");
    assert_eq!(stage.read_asset(resolved).unwrap(), b"tinyusd-texture-bytes");
    assert!(matches!(
        stage.read_asset("/nonexistent/file.png"),
        Err(Error::Asset { .. })
    ));
}

#[test]
fn open_errors() {
    assert!(matches!(
        Stage::open(&data("missing.usda"), &OpenOptions::default()),
        Err(Error::Open { .. })
    ));
}

#[test]
fn invalid_handles_are_errors() {
    let stage = open("query.usda");
    assert!(matches!(stage.path(Prim::from_index(9999)), Err(Error::InvalidPrim)));
    assert!(matches!(
        stage.type_name(Prim::from_index(9999)),
        Err(Error::InvalidPrim)
    ));
}

#[test]
fn value_reuse_clears_previous_contents() {
    let mut stage = open("query.usda");
    let p = prim(&mut stage, "/World/Types");
    let mut v = Value::new();
    assert!(stage.get(p, "aa", None, &mut v).unwrap());
    assert!(stage.get(p, "fa", None, &mut v).unwrap());
    assert!(v.strings().is_empty() && v.resolved_paths().is_empty());
    assert_eq!(v.as_slice::<f32>(), Some(&[1.0, 2.0, 3.0][..]));
    assert!(stage.get(p, "t", None, &mut v).unwrap());
    assert_eq!(v.as_slice::<f32>(), None);
    assert_eq!(v.bytes().len(), 0);
}
