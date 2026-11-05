import math
from pathlib import Path

import pytest

import tinyusd

DATA = Path(__file__).resolve().parents[3] / "tests" / "data"
pytestmark = pytest.mark.skipif("physics" not in tinyusd.features(), reason="built without physics")


def by_path(items):
    return {item["prim"].path: item for item in items}


def test_physics_parse():
    stage = tinyusd.Stage.open(DATA / "physics_full.usda")
    p = stage.physics()
    assert (p["meters_per_unit"], p["kilograms_per_unit"], p["si_units"]) == (0.01, 0.001, False)
    assert p["scenes"][0]["gravity_direction"] == [0.0, -1.0, 0.0]
    joints = by_path(p["joints"])
    elbow = joints["/World/Robot/Elbow"]
    assert elbow["kind"] == "Revolute" and elbow["body1"].path == "/World/Robot/Link"
    assert elbow["limits"][0] == {"dof": "RotZ", "enabled": True, "lower": -45.0, "upper": 90.0}
    assert joints["/World/Generic"]["kind"] == "D6"
    bodies = by_path(p["rigid_bodies"])
    assert math.isclose(bodies["/World/Props/Crate"]["mass"]["mass"], 2000.0)
    shapes = by_path(p["shapes"])
    assert shapes["/World/Props/Can"]["materials"][0].path == "/World/Rubber"
    assert p["articulations"][0]["roots"][0].path == "/World/Robot/Base"


def test_physics_si_and_range():
    stage = tinyusd.Stage.open(DATA / "physics_full.usda")
    p = stage.physics(include=[stage.prim("/World/Props")], si_units=True)
    assert p["si_units"] and not p["scenes"]
    bodies = by_path(p["rigid_bodies"])
    assert set(bodies) == {"/World/Props/Can", "/World/Props/Cone", "/World/Props/Crate", "/World/Props/Free"}
    assert math.isclose(bodies["/World/Props/Crate"]["mass"]["mass"], 2.0)
    assert stage.physics(compute_mass=False)["rigid_bodies"][0]["mass"] is None
