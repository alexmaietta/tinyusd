use tinyusd_core::{BaseType, Interpolation, OpenOptions, Prim, Projection, Purpose, Stage, Value};

fn open() -> Stage {
    let path = format!("{}/../../tests/data/render.usda", env!("CARGO_MANIFEST_DIR"));
    Stage::open(&path, &OpenOptions::default()).unwrap()
}

fn find(stage: &mut Stage, path: &str) -> Prim {
    stage
        .prim_at_path(path)
        .unwrap()
        .unwrap_or_else(|| panic!("missing {path}"))
}

fn close(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6)
}

#[test]
fn visibility_and_purpose_inherit() {
    let mut stage = open();
    let hidden_box = find(&mut stage, "/World/Hidden/Box");
    assert!(!stage.visible(hidden_box, None).unwrap());
    let crate_prim = find(&mut stage, "/World/Props/Crate");
    assert!(stage.visible(crate_prim, None).unwrap());
    let ball = find(&mut stage, "/World/Props/Animated/Ball");
    assert!(stage.visible(ball, Some(5.0)).unwrap());
    assert!(!stage.visible(ball, Some(10.0)).unwrap());
    assert!(stage.visible(stage.pseudo_root(), None).unwrap());

    assert_eq!(stage.purpose(crate_prim).unwrap(), Purpose::Default);
    let proxy = find(&mut stage, "/World/Props/Proxy");
    assert_eq!(stage.purpose(proxy).unwrap(), Purpose::Proxy);
    let helper = find(&mut stage, "/World/Props/Guides/Helper");
    assert_eq!(stage.purpose(helper).unwrap(), Purpose::Guide);
}

#[test]
fn bounds_follow_purpose_visibility_and_transforms() {
    let mut stage = open();
    let crate_prim = find(&mut stage, "/World/Props/Crate");
    let world = stage.world_bound(crate_prim, None, &[]).unwrap().unwrap();
    assert!(close(&world.aligned_min, &[9.0, -2.0, -3.0]), "{world:?}");
    assert!(close(&world.aligned_max, &[11.0, 2.0, 3.0]));
    assert!(close(&world.min, &[-1.0, -1.0, -1.0]));
    let local = stage.local_bound(crate_prim, None, &[]).unwrap().unwrap();
    assert!(close(&local.aligned_max, &[1.0, 2.0, 3.0]));
    let raw = stage.untransformed_bound(crate_prim, None, &[]).unwrap().unwrap();
    assert!(close(&raw.aligned_max, &[1.0, 1.0, 1.0]));

    let props = find(&mut stage, "/World/Props");
    let default = stage.world_bound(props, None, &[Purpose::Default]).unwrap().unwrap();
    assert!(close(&default.aligned_min, &[9.0, -2.0, -3.0]), "{default:?}");
    let with_proxy = stage
        .world_bound(props, None, &[Purpose::Default, Purpose::Proxy])
        .unwrap()
        .unwrap();
    assert!(close(&with_proxy.aligned_min, &[6.0, -4.0, -4.0]), "{with_proxy:?}");
    let guides = stage.world_bound(props, None, &[Purpose::Guide]).unwrap().unwrap();
    assert!(close(&guides.aligned_max, &[60.0, 50.0, 50.0]), "{guides:?}");
    let hidden = find(&mut stage, "/World/Hidden");
    let queried = stage.world_bound(hidden, None, &[]).unwrap().unwrap();
    assert!(
        close(&queried.aligned_max, &[1.0, 1.0, 1.0]),
        "a queried prim counts, as in OpenUSD"
    );
    if cfg!(feature = "lux") {
        let world = find(&mut stage, "/World");
        let all = stage.world_bound(world, None, &[]).unwrap().unwrap();
        assert!(close(&all.aligned_min, &[-2.0, -2.0, -3.0]), "{all:?}");
        assert!(close(&all.aligned_max, &[11.0, 2.0, 3.0]));
    }
}

#[test]
fn cameras_resolve_projection_and_frame() {
    let mut stage = open();
    let shot = find(&mut stage, "/World/Shot");
    let c = stage.camera(shot, None).unwrap().unwrap();
    assert_eq!(c.projection, Projection::Perspective);
    assert_eq!((c.focal_length, c.near, c.far), (35.0, 0.5, 500.0));
    assert_eq!((c.horizontal_aperture, c.vertical_aperture), (36.0, 24.0));
    let fov = 2.0 * (18.0f32 / 35.0).atan().to_degrees();
    assert!((c.horizontal_fov - fov).abs() < 1e-3, "{}", c.horizontal_fov);
    assert!(close(&c.transform[12..15], &[0.0, -10.0, 2.0]));
    assert!(c.exposure_scale > 0.0);
    assert_ne!(c.projection_matrix, [0.0; 16]);
    let top = find(&mut stage, "/World/Top");
    assert_eq!(
        stage.camera(top, None).unwrap().unwrap().projection,
        Projection::Orthographic
    );
    let world = find(&mut stage, "/World");
    assert!(stage.camera(world, None).unwrap().is_none());
}

#[cfg(feature = "lux")]
#[test]
fn lights_resolve_radiance_and_shape() {
    let mut stage = open();
    let key = find(&mut stage, "/World/Key");
    let l = stage.light(key, None).unwrap().unwrap();
    assert_eq!(l.type_name, "SphereLight");
    assert_eq!(l.radiance, [40.0, 20.0, 10.0]);
    assert!(l.normalize);
    assert_eq!((l.radius, l.width, l.cone_angle), (Some(0.5), None, Some(45.0)));
    let fill = find(&mut stage, "/World/Fill");
    let f = stage.light(fill, None).unwrap().unwrap();
    assert_eq!(f.color_temperature, Some(3000.0));
    assert!(f.radiance[0] > f.radiance[2], "warm light is redder: {:?}", f.radiance);
    assert_eq!((f.width, f.height), (Some(4.0), Some(2.0)));
    let sun = find(&mut stage, "/World/Sun");
    let s = stage.light(sun, None).unwrap().unwrap();
    assert_eq!((s.intensity, s.angle), (50000.0, Some(0.53)));
    let sky = find(&mut stage, "/World/Sky");
    assert!(
        stage
            .light(sky, None)
            .unwrap()
            .unwrap()
            .texture
            .unwrap()
            .ends_with("tex.png")
    );
    let world = find(&mut stage, "/World");
    assert!(stage.light(world, None).unwrap().is_none());
}

#[test]
fn meshes_triangulate_with_holes_and_winding() {
    let mut stage = open();
    let grid = find(&mut stage, "/World/Grid");
    let subsets = stage.geom_subsets(grid, None).unwrap();
    let mut summary: Vec<_> = subsets
        .iter()
        .map(|s| {
            (
                stage.name(s.prim).unwrap().to_owned(),
                s.family.clone(),
                s.element_type.clone(),
                s.indices.clone(),
            )
        })
        .collect();
    summary.sort();
    assert_eq!(
        summary,
        [
            (
                "Left".to_owned(),
                "materialBind".to_owned(),
                "face".to_owned(),
                vec![0, 2]
            ),
            (
                "Right".to_owned(),
                "materialBind".to_owned(),
                "face".to_owned(),
                vec![1, 3]
            ),
        ]
    );

    let t = stage.triangulate(grid, None).unwrap().unwrap();
    assert_eq!(t.faces, [0, 0, 2, 3, 3, 3]);
    assert_eq!(t.point_indices[..2], [[0, 1, 4], [0, 4, 3]]);
    assert_eq!(t.corner_indices[..2], [[0, 1, 2], [0, 2, 3]]);
    let flipped = find(&mut stage, "/World/Flipped");
    assert_eq!(
        stage.triangulate(flipped, None).unwrap().unwrap().point_indices,
        [[0, 2, 1]]
    );

    let n = stage.mesh_normals(grid, None).unwrap().unwrap();
    assert!(!n.authored && n.interpolation == Interpolation::Vertex);
    assert_eq!(n.normals[0], [0.0, 0.0, 1.0]);
    assert_eq!(
        stage.mesh_normals(flipped, None).unwrap().unwrap().normals[0],
        [0.0, 0.0, -1.0]
    );
    let normaled = find(&mut stage, "/World/Normaled");
    let a = stage.mesh_normals(normaled, None).unwrap().unwrap();
    assert!(a.authored && a.interpolation == Interpolation::Constant && a.normals == [[0.0, 0.0, 1.0]]);
    assert!(stage.triangulate(stage.pseudo_root(), None).unwrap().is_none());
}

#[test]
fn dictionary_metadata_reads_by_key_path() {
    let mut stage = open();
    let world = find(&mut stage, "/World");
    let mut v = Value::new();
    assert!(
        stage
            .prim_metadata(world, "customData:pipeline:version", &mut v)
            .unwrap()
    );
    assert_eq!((v.base_type(), v.as_scalar::<i32>()), (BaseType::Int, Some(3)));
    assert!(stage.prim_metadata(world, "customData:owner", &mut v).unwrap());
    assert_eq!(v.strings(), ["props"]);
    assert!(!stage.prim_metadata(world, "customData:missing", &mut v).unwrap());
    assert_eq!(
        stage.prim_metadata_keys(world, "customData").unwrap(),
        ["owner", "pipeline"]
    );
    assert_eq!(
        stage.prim_metadata_keys(world, "customData:pipeline").unwrap(),
        ["stage", "version"]
    );
    assert!(stage.prim_metadata_keys(world, "kind").unwrap().is_empty());

    assert!(stage.stage_metadata("customLayerData:build:number", &mut v).unwrap());
    assert_eq!(v.as_scalar::<i32>(), Some(42));
    assert_eq!(stage.stage_metadata_keys("customLayerData").unwrap(), ["build", "tool"]);
}
