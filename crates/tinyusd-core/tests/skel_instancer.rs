#[cfg(feature = "skel")]
use tinyusd_core::Value;
use tinyusd_core::{OpenOptions, Prim, Stage};

fn open(name: &str) -> Stage {
    Stage::open(
        &format!("{}/../../tests/data/{name}", env!("CARGO_MANIFEST_DIR")),
        &OpenOptions::default(),
    )
    .unwrap()
}

fn prim(stage: &mut Stage, path: &str) -> Prim {
    stage.prim_at_path(path).unwrap().unwrap()
}

#[cfg(feature = "skel")]
fn close(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
}

#[test]
fn point_instancer_filters_and_transforms() {
    let mut stage = open("instancer.usda");
    let trees = prim(&mut stage, "/World/Trees");
    let i = stage.instancer_instances(trees, None, true, true).unwrap().unwrap();
    let protos: Vec<_> = i
        .prototypes
        .iter()
        .map(|&p| stage.path(p).unwrap().to_owned())
        .collect();
    assert_eq!(protos, ["/World/Trees/Prototypes/Pine", "/World/Trees/Prototypes/Oak"]);
    assert_eq!(i.ids, [10, 12]);
    assert_eq!(i.proto_indices, [0, 1]);
    assert_eq!(&i.transforms[0][12..15], &[0.0, 1.0, 0.0]);
    assert_eq!(&i.transforms[1][12..15], &[10.0, 0.0, 0.0]);
    assert_eq!(i.transforms[1][5], 3.0);

    let all = stage.instancer_instances(trees, None, false, false).unwrap().unwrap();
    assert_eq!(all.ids, [10, 11, 12, 13]);
    assert_eq!(&all.transforms[0][12..15], &[0.0, 0.0, 0.0]);

    let rocks = prim(&mut stage, "/World/Rocks");
    let none = stage.instancer_instances(rocks, None, true, true).unwrap().unwrap();
    assert!(none.transforms.is_empty() && none.ids.is_empty() && none.prototypes.len() == 1);
    let at_five = stage
        .instancer_instances(rocks, Some(5.0), true, true)
        .unwrap()
        .unwrap();
    assert_eq!(at_five.ids, [0, 1]);
    assert_eq!(&at_five.transforms[1][12..15], &[1.0, 5.0, 0.0]);

    let world = prim(&mut stage, "/World");
    assert!(stage.instancer_instances(world, None, true, true).unwrap().is_none());
}

#[test]
#[cfg(feature = "skel")]
fn skeleton_bindings_and_pose() {
    let mut stage = open("skel.usda");
    let root = stage.pseudo_root();
    let bindings = stage.skel_bindings(root).unwrap();
    assert_eq!(bindings.len(), 1);
    let b = &bindings[0];
    assert_eq!(stage.path(b.skeleton).unwrap(), "/Character/Skel");
    assert_eq!(stage.path(b.animation.unwrap()).unwrap(), "/Character/Skel/Anim");
    let skinned: Vec<_> = b.skinned.iter().map(|&p| stage.path(p).unwrap().to_owned()).collect();
    assert_eq!(skinned, ["/Character/Arm", "/Character/Hand"]);

    let pose = stage.skeleton_pose(b.skeleton, Some(10.0)).unwrap().unwrap();
    assert_eq!(pose.joints, ["root", "root/elbow"]);
    assert_eq!(pose.parents, [-1, 0]);
    assert_eq!((pose.bind.len(), pose.local.len(), pose.skinning.len()), (2, 2, 2));
    assert!((pose.skinning[1][12] - 1.0).abs() < 1e-6 && (pose.skinning[1][13] - 1.0).abs() < 1e-6);
}

#[test]
#[cfg(feature = "skel")]
fn skin_binding_remaps_joints_and_blend_shapes() {
    let mut stage = open("skel.usda");
    let arm = prim(&mut stage, "/Character/Arm");
    let skin = stage.skin_binding(arm, Some(5.0)).unwrap().unwrap();
    assert_eq!(stage.path(skin.skeleton).unwrap(), "/Character/Skel");
    assert_eq!((skin.influences_per_point, skin.rigid), (2, false));
    assert_eq!(&skin.joint_indices[..4], &[0, 1, 0, 1]);
    assert_eq!(skin.blend_shapes, ["bulge"]);
    assert!((skin.blend_shape_weights[0] - 0.5).abs() < 1e-6);

    let shapes = stage.blend_shapes(arm, Some(5.0)).unwrap().unwrap();
    assert_eq!(shapes.point_indices, [vec![2, 3]]);
    let offsets: Vec<usize> = shapes.sub_shapes.iter().map(|s| s.point_offsets.len()).collect();
    assert_eq!(offsets, [0, 2, 2]);
    let weights: Vec<f32> = shapes.sub_shapes.iter().map(|s| s.weight).collect();
    assert_eq!(weights, [0.0, 1.0, 0.0]);
    assert!(close(&shapes.sub_shapes[1].point_offsets[0], &[0.05, 0.0, 0.0]));

    let hand = prim(&mut stage, "/Character/Hand");
    let rigid = stage.skin_binding(hand, None).unwrap().unwrap();
    assert!(rigid.rigid && rigid.joint_indices == [1]);
    assert!(stage.blend_shapes(hand, None).unwrap().is_none());
    let moved = stage.skinned_points(hand, Some(10.0)).unwrap().unwrap();
    assert!(close(&moved[0], &[-1.0, 1.0, 0.0]), "{moved:?}");

    let points = stage.skinned_points(arm, Some(0.0)).unwrap().unwrap();
    let mut rest = Value::new();
    assert!(stage.get(arm, "points", Some(0.0), &mut rest).unwrap());
    assert!(close(&points.concat(), rest.as_slice::<f32>().unwrap()));
    let skel = prim(&mut stage, "/Character/Skel");
    assert!(stage.skin_binding(skel, None).unwrap().is_none());
}
