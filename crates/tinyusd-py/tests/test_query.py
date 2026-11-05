from pathlib import Path

import numpy as np
import pytest

import tinyusd

DATA = Path(__file__).resolve().parents[3] / "tests" / "data"
FORMATS = ["query.usda", "query.usdc"]


@pytest.fixture(params=FORMATS)
def stage(request):
    return tinyusd.Stage.open(DATA / request.param)


def test_stage_metadata(stage):
    assert stage.metadata("upAxis") == "Z"
    assert stage.metadata("metersPerUnit") == 0.01
    assert stage.metadata("doesNotExist") is None
    assert stage.has_authored_metadata("upAxis")
    assert (stage.start_time_code, stage.end_time_code, stage.time_codes_per_second) == (1.0, 10.0, 24.0)
    assert stage.default_prim.path == "/World"
    assert stage.composition_errors == []
    with pytest.raises(tinyusd.UnsupportedTypeError):
        stage.metadata("customLayerData")


def test_traversal(stage):
    paths = [p.path for p in stage.traverse()]
    assert paths[0] == "/World" and paths[-1] == "/World/Payloaded" and len(paths) == 10
    every = [
        p.path
        for p in stage.traverse(
            include_inactive=True, include_unloaded=True, include_undefined=True, include_abstract=True
        )
    ]
    assert {"/_Base", "/World/OverOnly", "/World/Inactive"} <= set(every)
    mat = stage.prim("/World/Mat")
    assert [p.path for p in stage.traverse(mat)] == ["/World/Mat", "/World/Mat/Shader"]
    assert len(stage.prim("/World").children()) == 7
    assert stage.prim("/World/Mat/Shader").parent == mat
    assert stage.prim("/World/Nope") is None
    with pytest.raises(tinyusd.InvalidPathError):
        stage.prim("not a path")


def test_prim_queries(stage):
    mesh = stage.prim("/World/Mesh")
    assert (mesh.name, mesh.type_name) == ("Mesh", "Mesh")
    assert mesh.is_a("Gprim") and not mesh.is_a("Xform")
    assert mesh.has_api("SemanticsLabelsAPI:class") and mesh.has_api("PhysicsCollisionAPI")
    assert "SemanticsLabelsAPI:class" in mesh.applied_schemas
    assert "points" in mesh.attribute_names(authored_only=True)
    assert "doubleSided" in mesh.attribute_names()
    world = stage.prim("/World")
    assert world.kind == "assembly" and world.model and world.active
    assert stage.prim("/_Base").specifier == "class"
    assert not stage.prim("/World/Inactive").active
    assert stage.prim("/World/Variant/Geom").type_name == "Cube"
    assert len({mesh, stage.prim("/World/Mesh")}) == 1


def test_scalar_values(stage):
    p = stage.prim("/World/Types")
    assert p.get("b") is True
    assert p.get("uc") == 200
    assert p.get("i") == -3
    assert p.get("u64") == 18000000000000000000
    assert p.get("h") == 1.5
    assert p.get("f") == 2.5
    assert p.get("d") == 3.25
    assert p.get("tc") == 12.0
    assert p.get("s") == "hello"
    assert p.get("t") == "tok"
    assert p.get("i3") == (1, 2, 3)
    assert p.get("h3") == (0.5, 1.0, 2.0)
    assert p.get("f4") == (1.0, 2.0, 3.0, 4.0)
    assert p.get("qf") == (0.5, 0.5, 0.5, 0.5)
    assert p.get("qd") == (1.0, 0.0, 0.0, 0.0)
    m4 = p.get("m4")
    assert m4.shape == (4, 4) and m4.dtype == np.float64 and m4[3, :3].tolist() == [5.0, 6.0, 7.0]
    assert p.get("baseValue") == 7


def test_array_values(stage):
    p = stage.prim("/World/Types")
    fa = p.get("fa")
    assert fa.dtype == np.float32 and fa.tolist() == [1.0, 2.0, 3.0]
    pts = p.get("pts")
    assert pts.shape == (2, 3) and pts.dtype == np.float32 and pts[1].tolist() == [1.0, 2.0, 3.0]
    assert p.get("ta") == ["a", "b"]
    assert p.get("ba").dtype == np.bool_ and p.get("ba").tolist() == [True, False, True]
    info = p.attribute_info("pts")
    assert info.type_name == "point3f[]" and info.custom and not info.uniform
    assert p.attribute_info("nope") is None


def test_assets(stage):
    p = stage.prim("/World/Types")
    a = p.get("a")
    assert a.authored == "./part.usda" and a.resolved.endswith("tests/data/part.usda")
    aa = p.get("aa")
    assert [x.authored for x in aa] == ["./part.usda", "missing.png"]
    assert aa[1].resolved == ""


def test_time_samples_and_blocks(stage):
    p = stage.prim("/World/Types")
    assert p.time_samples("animated") == [1.0, 10.0]
    assert p.get("animated", 5.5) == 4.5
    assert p.attribute_info("animated").might_be_time_varying
    assert p.get("blocked") is None
    assert p.get("doesNotExist") is None


def test_relationships_connections_metadata(stage):
    p = stage.prim("/World/Types")
    assert p.targets("target") == ["/World/Mesh"]
    assert p.targets("forward") == ["/World/Types.target"]
    assert p.targets("forward", forwarded=True) == ["/World/Mesh"]
    assert p.attribute_metadata("primvars:displayColor", "interpolation") == "constant"
    assert stage.prim("/World").metadata("kind") == "assembly"
    assert stage.prim("/World/Mat").connections("outputs:surface") == ["/World/Mat/Shader.outputs:surface"]


def test_instancing(stage):
    i1, i2 = stage.prim("/World/Instance1"), stage.prim("/World/Instance2")
    assert i1.instance and i1.instanceable
    proto = i1.prototype
    assert proto == i2.prototype and proto.prototype_root
    assert stage.prototypes() == [proto]
    assert {p.path for p in proto.instances()} == {"/World/Instance1", "/World/Instance2"}


def test_payloads():
    stage = tinyusd.Stage.open(DATA / "query.usda", load_payloads=False)
    p = stage.prim("/World/Payloaded")
    assert not p.loaded and p.type_name == ""
    assert "/World/Payloaded" in [x.path for x in stage.traverse(include_unloaded=True)]


def test_usdz_assets():
    stage = tinyusd.Stage.open(DATA / "textured.usdz")
    file = stage.prim("/Root/Texture").get("inputs:file")
    assert file.resolved.endswith("textured.usdz[tex.png]")
    assert stage.read_asset(file.resolved) == b"tinyusd-texture-bytes"
    with pytest.raises(tinyusd.AssetError):
        stage.read_asset("/nonexistent/file.png")


def test_errors_and_features():
    with pytest.raises(tinyusd.OpenError, match="missing.usda"):
        tinyusd.Stage.open(DATA / "missing.usda")
    assert set(tinyusd.features()) >= {"geom", "shade", "lux", "skel", "physics", "vol", "media", "semantics"}
