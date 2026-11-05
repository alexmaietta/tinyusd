from pathlib import Path

import numpy as np
import pytest

import tinyusd

DATA = Path(__file__).resolve().parents[3] / "tests" / "data"


@pytest.fixture
def stage():
    return tinyusd.Stage.open(DATA / "resolve.usda")


def test_transforms(stage):
    own = stage.prim("/World/Group/Own")
    world = own.world_transform()
    assert world.shape == (4, 4) and world.dtype == np.float64
    assert world[3, :3].tolist() == [10.0, 0.0, 0.0]
    assert own.world_transform(5.0)[3, :3].tolist() == [10.0, 10.0, 0.0]
    local, resets = stage.prim("/World/Group/Reset").local_transform()
    assert resets and local[3, :3].tolist() == [1.0, 2.0, 3.0]
    np.testing.assert_allclose(stage.prim("/World/Group").world_transform()[0, :3], [0.0, 0.0, -2.0], atol=1e-9)


def test_primvars(stage):
    own = stage.prim("/World/Group/Own")
    assert sorted(own.primvar_names()) == ["displayColor", "roughness", "st", "weights"]
    st = own.primvar("st")
    assert (st.name, st.interpolation, st.indexed, st.type_name) == ("st", "faceVarying", True, "texCoord2f[]")
    assert st.value.shape == (7, 2) and st.value[5].tolist() == [1.0, 1.0]
    weights = own.primvar("primvars:weights")
    assert (weights.interpolation, weights.element_size, weights.value.shape) == ("vertex", 2, (10,))
    rough = own.primvar("roughness")
    assert rough.value == 0.25 and rough.source.path == "/World"
    inherits = stage.prim("/World/Group/Inherits")
    assert inherits.primvar("displayColor").source.path == "/World"
    assert inherits.primvar("st") is None


@pytest.mark.skipif("shade" not in tinyusd.features(), reason="built without shade")
def test_bound_material(stage):
    own = stage.prim("/World/Group/Own")
    assert own.bound_material().path == "/World/Looks/Blue"
    assert own.bound_material("preview").path == "/World/Looks/Preview"
    assert stage.prim("/World/Group/Own/Subset").bound_material().path == "/World/Looks/Red"
    assert stage.prim("/World/ByCollection/Target").bound_material().path == "/World/Looks/Collection"
    assert stage.prim("/World/ByCollection/Other").bound_material("full").path == "/World/Looks/Red"
