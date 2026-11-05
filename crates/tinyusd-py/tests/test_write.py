import shutil
from pathlib import Path

import numpy as np
import pytest

import tinyusd

DATA = Path(__file__).resolve().parents[3] / "tests" / "data"
pytestmark = pytest.mark.skipif("write" not in tinyusd.features(), reason="built without write")

POINTS = np.array([[0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0]], dtype=np.float32)


def build(stage):
    world = stage.define_prim("/World", "Xform")
    stage.set_default_prim(world)
    stage.set_up_axis("Z")
    stage.set_meters_per_unit(0.01)
    mesh = stage.define_prim("/World/Quad", "Mesh")
    mesh.set("points", POINTS)
    mesh.set("faceVertexCounts", [4])
    mesh.set("faceVertexIndices", [0, 1, 2, 3])
    mesh.set("subdivisionScheme", "none")
    return mesh


def check(stage):
    mesh = stage.prim("/World/Quad")
    assert mesh.is_a("Mesh")
    np.testing.assert_array_equal(mesh.get("points"), POINTS)
    assert list(mesh.get("faceVertexIndices")) == [0, 1, 2, 3]
    assert mesh.get("subdivisionScheme") == "none"
    assert stage.default_prim.path == "/World"
    assert stage.metadata("metersPerUnit") == 0.01


@pytest.mark.parametrize("name", ["scene.usda", "scene.usdc"])
def test_create_save_reopen(tmp_path, name):
    path = tmp_path / name
    stage = tinyusd.Stage.create_new(path)
    build(stage)
    stage.save()
    del stage
    check(tinyusd.Stage.open(path))


def test_export(tmp_path):
    stage = tinyusd.Stage.create_in_memory()
    build(stage)
    for name in ["flat.usda", "flat.usdc", "flat.usdz"]:
        stage.export(tmp_path / name)
        check(tinyusd.Stage.open(tmp_path / name))
    assert b'def Mesh "Quad"' in stage.export_bytes()
    assert stage.export_bytes("usdc", layer=stage.edit_target).startswith(b"PXR-USDC")
    with pytest.raises(ValueError):
        stage.export_bytes("obj")


def test_strict_values():
    stage = tinyusd.Stage.create_in_memory()
    mesh = build(stage)
    with pytest.raises(tinyusd.InvalidValueError, match="float32"):
        mesh.set("points", POINTS.astype(np.float64))
    with pytest.raises(tinyusd.InvalidValueError):
        mesh.set("faceVertexIndices", [0.5, 1.0])
    with pytest.raises(tinyusd.InvalidValueError):
        mesh.set("faceVertexIndices", [True])
    with pytest.raises(tinyusd.InvalidValueError, match="not declared"):
        mesh.set("my:weight", 1.0)
    with pytest.raises(tinyusd.InvalidValueError):
        mesh.set("points", POINTS[:, :2].copy())
    mesh.create_attribute("my:weight", "double")
    mesh.set("my:weight", 2)
    assert mesh.get("my:weight") == 2.0
    mesh.create_attribute("my:flag", "bool")
    mesh.set("my:flag", True)
    assert mesh.get("my:flag") is True
    mesh.create_attribute("my:pose", "matrix4d")
    mesh.set("my:pose", np.eye(4))
    np.testing.assert_array_equal(mesh.get("my:pose"), np.eye(4))
    mesh.create_attribute("my:texture", "asset")
    mesh.set("my:texture", "./wood.png")
    assert mesh.get("my:texture").authored == "./wood.png"


def test_batch_records_samples():
    stage = tinyusd.Stage.create_in_memory()
    body = stage.define_prim("/Body", "Xform")
    body.create_attribute("xformOp:translate", "double3")
    body.set("xformOpOrder", ["xformOp:translate"])
    with stage.edit():
        for frame in range(50):
            body.set("xformOp:translate", (0.0, 0.0, float(frame)), time=frame)
        assert body.time_samples("xformOp:translate") == []
    assert len(body.time_samples("xformOp:translate")) == 50
    assert body.world_transform(20.0)[3][2] == 20.0

    with pytest.raises(ZeroDivisionError):
        with stage.edit():
            stage.define_prim("/Never", "Xform")
            1 / 0
    assert stage.prim("/Never") is None

    with stage.edit():
        mesh = stage.define_prim("/Batched", "Mesh")
        mesh.set("points", POINTS)
    np.testing.assert_array_equal(stage.prim("/Batched").get("points"), POINTS)


def test_schemas_and_relationships():
    stage = tinyusd.Stage.create_in_memory()
    body = stage.define_prim("/World/Body", "Cube")
    body.apply_api("PhysicsRigidBodyAPI")
    body.apply_api("PhysicsMassAPI")
    body.set("physics:mass", 3.0)
    joint = stage.define_prim("/World/Hinge", "PhysicsRevoluteJoint")
    joint.set_targets("physics:body1", ["/World/Body"])
    joint.apply_api("PhysicsLimitAPI:rotX")
    assert joint.has_api("PhysicsLimitAPI:rotX")
    physics = stage.physics()
    assert physics["rigid_bodies"][0]["prim"].path == "/World/Body"
    assert physics["joints"][0]["body1"].path == "/World/Body"
    inst = stage.define_prim("/World/Inst")
    stage.define_prim("/Proto", "Sphere")
    inst.add_reference(prim_path="/Proto")
    assert inst.is_a("Sphere")
    assert stage.remove_prim(inst)
    assert stage.prim("/World/Inst") is None


def test_overlay_keeps_source(tmp_path):
    source = tmp_path / "robot.usda"
    shutil.copy(DATA / "physics.usda", source)
    before = source.read_bytes()
    stage = tinyusd.Stage.create_new(tmp_path / "result.usda")
    stage.add_sublayer("./robot.usda")
    base = stage.prim("/World/Base")
    with stage.edit():
        for frame in range(5):
            base.set("xformOp:translate", (0, 0, frame), time=frame)
    stage.set_time_range(0, 4, time_codes_per_second=24)
    stage.save()
    assert source.read_bytes() == before
    reopened = tinyusd.Stage.open(tmp_path / "result.usda")
    assert reopened.prim("/World/Base").world_transform(3.0)[3][2] == 3.0
    assert reopened.end_time_code == 4.0


class MemoryFs:
    def __init__(self):
        self.files = {}

    def resolve(self, asset_path, anchor):
        return asset_path if asset_path in self.files else None

    def read(self, resolved_path):
        return self.files.get(resolved_path)

    def write(self, resolved_path, data):
        self.files[resolved_path] = data


def test_resolver_writes():
    fs = MemoryFs()
    stage = tinyusd.Stage.create_new("mem://scene.usda", resolver=fs)
    build(stage)
    stage.save()
    assert b"def Mesh" in fs.files["mem://scene.usda"]
    check(tinyusd.Stage.open("mem://scene.usda", resolver=fs))

    class ReadOnly:
        def resolve(self, asset_path, anchor):
            return None

        def read(self, resolved_path):
            return None

    with pytest.raises(tinyusd.WriteError):
        tinyusd.Stage.create_new("mem://other.usda", resolver=ReadOnly())


def test_helpers(tmp_path):
    stage = tinyusd.Stage.create_in_memory()
    parent = stage.define_prim("/P", "Xform")
    parent.set_trs((1, 2, 3))
    child = stage.define_prim("/P/Quad", "Mesh")
    target = np.eye(4)
    target[3, :3] = (5, 6, 7)
    child.set_world_transform(target)
    np.testing.assert_allclose(child.world_transform(), target)
    child.set_mesh(POINTS, [4], [0, 1, 2, 3])
    np.testing.assert_array_equal(child.get("extent"), [[0, 0, 0], [1, 1, 0]])
    with pytest.raises(tinyusd.InvalidValueError):
        child.set_mesh(POINTS, [4], [0, 1, 2, 7])
    child.set_primvar("st", "texCoord2f[]", [[0, 0], [1, 1]], "faceVarying", indices=[0, 1, 1, 0])
    st = child.primvar("st")
    assert st.indexed and st.interpolation == "faceVarying" and st.value.shape == (4, 2)
    look = stage.define_prim("/Looks/Red", "Material")
    child.bind_material(look, "preview")
    assert child.bound_material("preview").path == "/Looks/Red"

    (tmp_path / "tex.png").write_bytes(b"png")
    packaged = tinyusd.Stage.create_new(tmp_path / "pkg.usda")
    tex = packaged.define_prim("/Tex", "Shader")
    tex.create_attribute("inputs:file", "asset")
    tex.set("inputs:file", "./tex.png")
    packaged.export_package(tmp_path / "pkg.usdz")
    file = tinyusd.Stage.open(tmp_path / "pkg.usdz").prim("/Tex").get("inputs:file")
    assert ".usdz[" in file.resolved


@pytest.mark.skipif("physics" not in tinyusd.features(), reason="built without physics")
def test_write_physics_round_trip(tmp_path):
    stage = tinyusd.Stage.create_new(tmp_path / "result.usda")
    stage.add_sublayer(str(DATA / "physics_full.usda"))
    stage.set_meters_per_unit(0.01)
    stage.set_kilograms_per_unit(0.001)
    stage.set_up_axis("Y")
    before = stage.physics(si_units=True)
    stage.write_physics(before, poses=True)
    after = stage.physics(si_units=True)
    assert len(after["joints"]) == len(before["joints"])
    for a, b in zip(before["joints"], after["joints"]):
        assert a["kind"] == b["kind"] and len(a["limits"]) == len(b["limits"])
        for la, lb in zip(a["limits"], b["limits"]):
            assert (la["dof"], la["enabled"]) == (lb["dof"], lb["enabled"])
            assert [la["lower"], la["upper"]] == pytest.approx([lb["lower"], lb["upper"]], rel=1e-5)
        np.testing.assert_allclose(a["local_pos1"], b["local_pos1"], rtol=1e-5, atol=1e-6)
    for a, b in zip(before["rigid_bodies"], after["rigid_bodies"]):
        np.testing.assert_allclose(a["position"], b["position"], rtol=1e-5, atol=1e-6)


@pytest.mark.skipif("physics" not in tinyusd.features(), reason="built without physics")
def test_write_physics_from_partial_dicts():
    stage = tinyusd.Stage.create_in_memory()
    base = stage.define_prim("/Base", "Cube")
    arm = stage.define_prim("/Arm", "Cube")
    hinge = stage.define_prim("/Hinge")
    stage.write_physics(
        {
            "rigid_bodies": [{"prim": arm, "position": [0, 0, 2]}, {"prim": base, "kinematic": True}],
            "joints": [
                {
                    "prim": hinge,
                    "kind": "Revolute",
                    "axis": "Y",
                    "body0": base,
                    "body1": arm,
                    "limits": [{"dof": "RotY", "lower": -30, "upper": 30}],
                    "drives": [{"dof": "RotY", "stiffness": 100, "damping": 10}],
                }
            ],
        },
        poses=True,
    )
    stage.write_body_mass(arm, {"mass": 2.0})
    p = stage.physics()
    bodies = {b["prim"].path: b for b in p["rigid_bodies"]}
    assert bodies["/Base"]["kinematic"] and bodies["/Arm"]["position"] == [0, 0, 2]
    joint = p["joints"][0]
    assert joint["kind"] == "Revolute" and joint["axis"] == "Y"
    assert joint["limits"][0]["upper"] == 30 and joint["drives"][0]["stiffness"] == 100
    assert arm.get("physics:mass") == 2.0
    with pytest.raises(ValueError, match="kind"):
        stage.write_physics({"joints": [{"prim": hinge, "kind": "Hinge"}]})


def test_remove_rename_and_variants():
    stage = tinyusd.Stage.create_in_memory()
    body = stage.define_prim("/World/Body", "Cube")
    body.apply_api("PhysicsRigidBodyAPI")
    body.remove_api("PhysicsRigidBodyAPI")
    assert not body.has_api("PhysicsRigidBodyAPI")
    joint = stage.define_prim("/World/Joint", "PhysicsFixedJoint")
    joint.add_target("physics:body0", "/World/Body")
    body.rename("Crate")
    assert body.path == "/World/Crate"
    assert joint.targets("physics:body0") == ["/World/Crate"]
    shelf = stage.define_prim("/World/Shelf", "Xform")
    stage.move_prim(body, shelf, "Box")
    assert body.path == "/World/Shelf/Box"
    body.reparent(stage.prim("/World"))
    assert body.path == "/World/Box"

    asset = stage.define_prim("/Asset", "Xform")
    asset.edit_variant("lod", "high")
    stage.define_prim("/Asset/Geo", "Sphere")
    asset.edit_variant("lod", "low")
    stage.define_prim("/Asset/Geo", "Cube")
    stage.set_edit_target(stage.edit_target)
    assert asset.variant_sets == {"lod": ["high", "low"]}
    asset.set_variant_selection("lod", "high")
    assert stage.prim("/Asset/Geo").type_name == "Sphere"
