use tinyusd_core::{BaseType, Interpolation, OpenOptions, Prim, Stage, Value};

fn open() -> Stage {
    let path = format!("{}/../../tests/data/resolve.usda", env!("CARGO_MANIFEST_DIR"));
    Stage::open(&path, &OpenOptions::default()).unwrap()
}

fn prim(stage: &mut Stage, path: &str) -> Prim {
    stage.prim_at_path(path).unwrap().unwrap()
}

fn close(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6)
}

#[test]
fn world_and_local_transforms() {
    let mut stage = open();
    let own = prim(&mut stage, "/World/Group/Own");
    let world = stage.world_transform(own, None).unwrap();
    assert!(close(&world[12..15], &[10.0, 0.0, 0.0]), "{world:?}");
    let at_five = stage.world_transform(own, Some(5.0)).unwrap();
    assert!(close(&at_five[12..15], &[10.0, 10.0, 0.0]), "{at_five:?}");
    let (local, resets) = stage.local_transform(own, Some(10.0)).unwrap();
    assert!(!resets && close(&local[12..15], &[0.0, 10.0, 0.0]));

    let reset = prim(&mut stage, "/World/Group/Reset");
    let (_, resets) = stage.local_transform(reset, None).unwrap();
    assert!(resets);
    assert!(close(
        &stage.world_transform(reset, None).unwrap()[12..15],
        &[1.0, 2.0, 3.0]
    ));

    let group = prim(&mut stage, "/World/Group");
    let g = stage.world_transform(group, None).unwrap();
    assert!(close(&g[0..3], &[0.0, 0.0, -2.0]), "{g:?}");
}

#[test]
fn primvars_flatten_and_inherit() {
    let mut stage = open();
    let own = prim(&mut stage, "/World/Group/Own");
    let mut names = stage.primvar_names(own).unwrap();
    names.sort();
    assert_eq!(names, ["displayColor", "roughness", "st", "weights"]);

    let mut v = Value::new();
    let st = stage.primvar(own, "st", None, &mut v).unwrap().unwrap();
    assert_eq!(
        (st.interpolation, st.indexed, st.element_size),
        (Interpolation::FaceVarying, true, 1)
    );
    assert_eq!(
        (v.base_type(), v.len(), v.type_name()),
        (BaseType::Float2, 7, "texCoord2f[]")
    );
    assert_eq!(&v.as_slice::<f32>().unwrap()[8..14], &[0.0, 0.0, 1.0, 1.0, 1.0, 0.0]);

    let pv = stage.primvar(own, "primvars:weights", None, &mut v).unwrap().unwrap();
    assert_eq!(
        (pv.interpolation, pv.element_size, v.len()),
        (Interpolation::Vertex, 2, 10)
    );

    let roughness = stage.primvar(own, "roughness", None, &mut v).unwrap().unwrap();
    assert_eq!(stage.path(roughness.source).unwrap(), "/World");
    assert_eq!(v.as_scalar::<f32>(), Some(0.25));

    let color = stage.primvar(own, "displayColor", None, &mut v).unwrap().unwrap();
    assert_eq!(
        (color.interpolation, stage.path(color.source).unwrap()),
        (Interpolation::Uniform, "/World/Group/Own")
    );

    let inherits = prim(&mut stage, "/World/Group/Inherits");
    let color = stage.primvar(inherits, "displayColor", None, &mut v).unwrap().unwrap();
    assert_eq!(stage.path(color.source).unwrap(), "/World");
    assert!(stage.primvar(inherits, "st", None, &mut v).unwrap().is_none());
    assert!(v.is_empty());
}

#[test]
#[cfg(feature = "shade")]
fn bound_materials() {
    let mut stage = open();
    let bound = |stage: &mut Stage, path: &str, purpose: &str| {
        let p = prim(stage, path);
        stage
            .bound_material(p, purpose)
            .unwrap()
            .map(|b| (stage.path(b.material).unwrap().to_owned(), b.binding))
    };
    assert_eq!(
        bound(&mut stage, "/World/Group/Own", ""),
        Some(("/World/Looks/Blue".into(), "/World/Group/Own.material:binding".into()))
    );
    assert_eq!(
        bound(&mut stage, "/World/Group/Own", "preview").unwrap().0,
        "/World/Looks/Preview"
    );
    assert_eq!(
        bound(&mut stage, "/World/Group/Inherits", "full").unwrap().0,
        "/World/Looks/Red"
    );
    assert_eq!(
        bound(&mut stage, "/World/Group/Own/Subset", "").unwrap().0,
        "/World/Looks/Red"
    );
    assert_eq!(bound(&mut stage, "/World/Strong", "").unwrap().0, "/World/Looks/Strong");
    assert_eq!(
        bound(&mut stage, "/World/ByCollection/Target", "").unwrap().0,
        "/World/Looks/Collection"
    );
    assert_eq!(
        bound(&mut stage, "/World/ByCollection/Other", "").unwrap().0,
        "/World/Looks/Red"
    );
}

#[test]
#[cfg(not(feature = "shade"))]
fn bound_material_needs_shade() {
    let mut stage = open();
    let p = prim(&mut stage, "/World");
    assert!(matches!(
        stage.bound_material(p, ""),
        Err(tinyusd_core::Error::FeatureDisabled("shade"))
    ));
}
