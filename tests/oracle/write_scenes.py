import shutil
import sys
from pathlib import Path

import numpy as np

import tinyusd

DATA = Path(__file__).resolve().parents[1] / "data"
POINTS = np.array([[0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0]], dtype=np.float32)


def scene(stage):
    world = stage.define_prim("/World", "Xform")
    stage.set_default_prim(world)
    stage.set_up_axis("Z")
    stage.set_meters_per_unit(0.01)
    stage.set_kilograms_per_unit(1.0)
    stage.set_time_range(0, 10, time_codes_per_second=24)
    world.set_kind("assembly")

    mesh = stage.define_prim("/World/Quad", "Mesh")
    mesh.set_mesh(POINTS, [4], [0, 1, 2, 3])
    mesh.set("subdivisionScheme", "none")
    mesh.set("doubleSided", True)
    mesh.set_primvar("st", "texCoord2f[]", [[0, 0], [1, 0], [1, 1], [0, 1]], "faceVarying", indices=[0, 1, 2, 3])
    mesh.set_primvar("displayColor", "color3f[]", [[1, 0, 0]], "constant")
    mesh.set_trs((1, 2, 3), (0.70710677, 0, 0, 0.70710677), (2, 2, 2))
    mesh.create_attribute("my:weights", "float[]")
    mesh.set("my:weights", np.linspace(0, 1, 4, dtype=np.float32))
    mesh.create_attribute("my:label", "string", uniform=True)
    mesh.set("my:label", "quad")
    mesh.create_attribute("my:pose", "matrix4d")
    mesh.set("my:pose", np.eye(4))
    mesh.create_attribute("my:half", "half3")
    mesh.set("my:half", np.array([1, 2, 3], dtype=np.float16))
    mesh.create_attribute("my:ids", "int64[]")
    mesh.set("my:ids", np.array([1, 2, 3], dtype=np.int64))

    look = stage.define_prim("/World/Looks/Red", "Material")
    shader = stage.define_prim("/World/Looks/Red/Surface", "Shader")
    shader.create_attribute("info:id", "token", uniform=True)
    shader.set("info:id", "UsdPreviewSurface")
    shader.create_attribute("inputs:diffuseColor", "color3f")
    shader.set("inputs:diffuseColor", (1, 0, 0))
    shader.create_attribute("outputs:surface", "token")
    look.create_attribute("outputs:surface", "token")
    look.set_connections("outputs:surface", ["/World/Looks/Red/Surface.outputs:surface"])
    mesh.bind_material(look)

    body = stage.define_prim("/World/Body", "Cube")
    body.apply_api("PhysicsRigidBodyAPI")
    body.apply_api("PhysicsMassAPI")
    body.apply_api("PhysicsCollisionAPI")
    body.set("physics:mass", 2.5)
    body.set("size", 0.5)
    with stage.edit():
        for frame in range(11):
            body.set_trs((0, 0, 10 - 0.05 * frame * frame), time=frame)
    joint = stage.define_prim("/World/Hinge", "PhysicsRevoluteJoint")
    joint.set_targets("physics:body1", ["/World/Body"])
    joint.set("physics:axis", "Z")
    joint.apply_api("PhysicsLimitAPI:rotZ")
    joint.set("limit:rotZ:physics:low", -45.0)

    stage.define_prim("/Proto", "Sphere").set("radius", 0.25)
    inst = stage.define_prim("/World/Instance")
    inst.add_reference(prim_path="/Proto")
    inst.set_instanceable(True)
    hidden = stage.define_prim("/World/Hidden", "Xform")
    hidden.set_active(False)


def physics_scene(stage):
    stage.set_up_axis("Z")
    stage.set_meters_per_unit(1.0)
    stage.set_kilograms_per_unit(1.0)
    world = stage.define_prim("/World", "Xform")
    stage.set_default_prim(world)
    ground = stage.define_prim("/World/Ground", "Cube")
    ground.set_trs((0, 0, -0.5), scale=(10, 10, 1))
    rubber = stage.define_prim("/World/Rubber", "Material")
    group = stage.define_prim("/World/Robot")
    base = stage.define_prim("/World/Base", "Cube")
    arm = stage.define_prim("/World/Arm", "Capsule")
    arm.set("height", 1.0)
    links = [stage.define_prim(f"/World/Joints/{name}") for name in ["Hinge", "Slider", "Ball", "Rope", "D6"]]
    pose = {"body0": base, "body1": arm, "local_pos0": [0, 0, 0.5], "local_pos1": [0, 0, -0.5]}
    stage.write_physics(
        {
            "scenes": [{"prim": stage.define_prim("/World/Scene"), "gravity_direction": [0, 0, -1], "gravity_magnitude": 9.81}],
            "collision_groups": [{"prim": group, "merge_group_name": "robot"}],
            "materials": [{"prim": rubber, "static_friction": 0.9, "dynamic_friction": 0.7, "restitution": 0.2, "density": 1100}],
            "shapes": [
                {"prim": ground, "materials": [rubber]},
                {"prim": base, "collision_groups": [group]},
                {"prim": arm, "collision_groups": [group], "filtered_collisions": [ground]},
            ],
            "rigid_bodies": [
                {"prim": base, "position": [0, 0, 0.5], "kinematic": True},
                {"prim": arm, "position": [0, 0, 1.5], "linear_velocity": [0, 0, -1]},
            ],
            "joints": [
                {"prim": links[0], "kind": "Revolute", "axis": "Y", **pose,
                 "limits": [{"dof": "RotY", "lower": -45, "upper": 90}],
                 "drives": [{"dof": "RotY", "target_position": 10, "stiffness": 50, "damping": 5}]},
                {"prim": links[1], "kind": "Prismatic", **pose, "limits": [{"dof": "TransX", "lower": 0, "upper": 0.3}]},
                {"prim": links[2], "kind": "Spherical", **pose, "limits": [{"dof": "RotX", "lower": 30, "upper": 20}]},
                {"prim": links[3], "kind": "Distance", **pose, "max_distance_enabled": True,
                 "limits": [{"dof": "Distance", "lower": 0, "upper": 2}]},
                {"prim": links[4], "kind": "D6", **pose,
                 "limits": [{"dof": "TransX", "lower": 1, "upper": -1}, {"dof": "RotZ", "lower": -10, "upper": 10}],
                 "drives": [{"dof": "RotZ", "target_velocity": 1, "damping": 2, "acceleration": True}]},
            ],
            "articulations": [{"prim": world}],
        },
        poses=True,
    )
    stage.write_body_mass(arm, {"mass": 3.0, "center_of_mass": [0, 0, 0.1], "diagonal_inertia": [1, 1, 0.5]})


def variants_and_moves(stage):
    stage.set_up_axis("Z")
    stage.set_meters_per_unit(1.0)
    world = stage.define_prim("/World", "Xform")
    stage.set_default_prim(world)
    asset = stage.define_prim("/World/Asset", "Xform")
    root = stage.edit_target
    asset.edit_variant("lod", "high")
    stage.define_prim("/World/Asset/Geo", "Sphere").set("radius", 2.0)
    asset.edit_variant("lod", "low")
    stage.define_prim("/World/Asset/Geo", "Cube")
    stage.set_edit_target(root)
    asset.set_variant_selection("lod", "high")
    crate = stage.define_prim("/World/Crate", "Cube")
    crate.apply_api("PhysicsRigidBodyAPI")
    crate.apply_api("PhysicsMassAPI")
    crate.remove_api("PhysicsMassAPI")
    joint = stage.define_prim("/World/Weld", "PhysicsFixedJoint")
    joint.set_targets("physics:body0", ["/World/Crate"])
    crate.rename("Box")
    stage.move_prim(crate, asset)


def main(out):
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    for name in ["written.usda", "written.usdc"]:
        stage = tinyusd.Stage.create_new(out / name)
        scene(stage)
        stage.save()
    stage = tinyusd.Stage.create_in_memory()
    scene(stage)
    stage.export(out / "flattened.usdz")

    shutil.copy(DATA / "physics.usda", out / "robot.usda")
    overlay = tinyusd.Stage.create_new(out / "overlay.usda")
    overlay.add_sublayer("./robot.usda")
    overlay.set_up_axis("Z")
    overlay.set_meters_per_unit(1.0)
    overlay.set_kilograms_per_unit(1.0)
    overlay.set_default_prim(overlay.prim("/World"))
    base = overlay.prim("/World/Base")
    with overlay.edit():
        for frame in range(5):
            base.set("xformOp:translate", (0, 0, frame), time=frame)
    overlay.save()

    stage = tinyusd.Stage.create_new(out / "physics_written.usda")
    physics_scene(stage)
    stage.save()

    shutil.copy(DATA / "physics_full.usda", out / "physics_source.usda")
    rewritten = tinyusd.Stage.create_new(out / "physics_rewritten.usda")
    rewritten.add_sublayer("./physics_source.usda")
    rewritten.set_up_axis("Y")
    rewritten.set_meters_per_unit(0.01)
    rewritten.set_kilograms_per_unit(0.001)
    rewritten.set_default_prim(rewritten.prim("/World"))
    rewritten.write_physics(rewritten.physics(si_units=True), poses=True)
    rewritten.save()

    stage = tinyusd.Stage.create_new(out / "variants.usda")
    variants_and_moves(stage)
    stage.save()
    print("\n".join(str(p) for p in sorted(out.glob("*.usd*"))))


if __name__ == "__main__":
    main(sys.argv[1])
