from pathlib import Path

import numpy as np
import pytest

import tinyusd

DATA = Path(__file__).resolve().parents[3] / "tests" / "data"


def test_instancer():
    stage = tinyusd.Stage.open(DATA / "instancer.usda")
    trees = stage.prim("/World/Trees").instancer_instances()
    assert [p.path for p in trees.prototypes] == ["/World/Trees/Prototypes/Pine", "/World/Trees/Prototypes/Oak"]
    assert trees.ids.tolist() == [10, 12] and trees.proto_indices.tolist() == [0, 1]
    assert trees.transforms.shape == (2, 4, 4)
    assert trees.transforms[1, 3, :3].tolist() == [10.0, 0.0, 0.0] and trees.transforms[1, 1, 1] == 3.0
    everything = stage.prim("/World/Trees").instancer_instances(include_prototype_transform=False, apply_visibility=False)
    assert everything.ids.tolist() == [10, 11, 12, 13]
    rocks = stage.prim("/World/Rocks")
    assert rocks.instancer_instances().transforms.shape == (0, 4, 4)
    assert rocks.instancer_instances(5.0).transforms[1, 3, :3].tolist() == [1.0, 5.0, 0.0]
    assert stage.prim("/World").instancer_instances() is None


@pytest.mark.skipif("skel" not in tinyusd.features(), reason="built without skel")
def test_skel():
    stage = tinyusd.Stage.open(DATA / "skel.usda")
    (binding,) = stage.skel_bindings()
    assert binding.skeleton.path == "/Character/Skel" and binding.animation.path == "/Character/Skel/Anim"
    assert [p.path for p in binding.skinned] == ["/Character/Arm", "/Character/Hand"]
    pose = binding.skeleton.skeleton_pose(10.0)
    assert pose.joints == ["root", "root/elbow"] and pose.parents.tolist() == [-1, 0]
    assert pose.skinning.shape == (2, 4, 4)
    np.testing.assert_allclose(pose.skinning[1, 3, :2], [1.0, 1.0], atol=1e-6)

    arm = stage.prim("/Character/Arm")
    skin = arm.skin_binding(5.0)
    assert skin.joint_indices.shape == (6, 2) and skin.joint_weights.dtype == np.float32
    assert skin.joint_indices[0].tolist() == [0, 1]
    assert skin.blend_shapes == ["bulge"] and skin.blend_shape_weights.tolist() == [0.5]
    assert skin.skinning_method == "classicLinear" and skin.geom_bind_transform.shape == (4, 4)
    shapes = arm.blend_shapes(5.0)
    assert [s.weight for s in shapes.sub_shapes] == [0.0, 1.0, 0.0]
    assert shapes.sub_shapes[1].point_offsets.shape == (2, 3)

    hand = stage.prim("/Character/Hand")
    assert hand.skin_binding().rigid
    np.testing.assert_allclose(hand.skinned_points(10.0)[0], [-1.0, 1.0, 0.0], atol=1e-5)
    assert stage.prim("/Character/Skel").skin_binding() is None
