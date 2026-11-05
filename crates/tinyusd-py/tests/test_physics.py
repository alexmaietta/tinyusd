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
    poses = {s["prim"].path: (s["local_pos"], s["local_rot"]) for s in p["shapes"]}

    def info(collider):
        if collider.path not in poses:
            return None
        pos, rot = poses[collider.path]
        return {"volume": 10.0, "inertia": [[1, 0, 0], [0, 1, 0], [0, 0, 1]], "local_pos": pos, "local_rot": rot}

    assert math.isclose(stage.mass_properties(stage.prim("/World/Props/Free"), info)["mass"], 10.0)
    assert math.isclose(stage.mass_properties(stage.prim("/World/Props/Can"), info)["mass"], 0.02, rel_tol=1e-6)
    base = stage.mass_properties(stage.prim("/World/Robot/Base"), lambda c: None)
    assert base["mass"] == 5000.0 and base["center_of_mass"] == [0.0, 5.0, 0.0]
    with pytest.raises(ZeroDivisionError):
        stage.mass_properties(stage.prim("/World/Props/Free"), lambda c: 1 / 0)
    shapes = by_path(p["shapes"])
    assert shapes["/World/Props/Can"]["materials"][0].path == "/World/Rubber"
    assert p["articulations"][0]["roots"][0].path == "/World/Robot/Base"


def test_physics_si_and_range():
    stage = tinyusd.Stage.open(DATA / "physics_full.usda")
    p = stage.physics(include=[stage.prim("/World/Props")], si_units=True)
    assert p["si_units"] and not p["scenes"]
    bodies = by_path(p["rigid_bodies"])
    assert set(bodies) == {"/World/Props/Can", "/World/Props/Cone", "/World/Props/Crate", "/World/Props/Free"}
    assert math.isclose(bodies["/World/Props/Crate"]["position"][2], 0.4, rel_tol=1e-6)
