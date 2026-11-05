import argparse
import json
import math
import os
import subprocess
import sys
from pathlib import Path

from pxr import Gf, Sdf, Usd, UsdGeom, UsdShade, UsdSkel

METADATA_KEYS = ["upAxis", "metersPerUnit", "defaultPrim", "startTimeCode", "endTimeCode", "timeCodesPerSecond"]
DEFAULT_DUMP = Path(__file__).resolve().parents[2] / "target" / "debug" / "examples" / "dump"


def flatten(value):
    if isinstance(value, bool):
        return [float(value)]
    if isinstance(value, (int, float)):
        return [float(value)]
    if isinstance(value, Sdf.TimeCode):
        return [float(value.GetValue())]
    if isinstance(value, (Gf.Quath, Gf.Quatf, Gf.Quatd)):
        return [float(value.GetReal()), *map(float, value.GetImaginary())]
    if isinstance(value, (Gf.Matrix2d, Gf.Matrix3d, Gf.Matrix4d)):
        n = {Gf.Matrix2d: 2, Gf.Matrix3d: 3, Gf.Matrix4d: 4}[type(value)]
        return [float(value[r][c]) for r in range(n) for c in range(n)]
    if hasattr(value, "__len__") and not isinstance(value, str):
        out = []
        for item in value:
            out.extend(flatten(item))
        return out
    raise TypeError(type(value))


def encode(value, type_name):
    if value is None:
        return None
    scalar = type_name.scalarType
    if scalar == Sdf.ValueTypeNames.Asset:
        items = list(value) if type_name.isArray else [value]
        return {"authored": [a.path for a in items], "resolved": [a.resolvedPath or "" for a in items]}
    if scalar in (Sdf.ValueTypeNames.String, Sdf.ValueTypeNames.Token):
        return [str(v) for v in value] if type_name.isArray else [str(value)]
    try:
        return flatten(value)
    except TypeError:
        return "<unsupported>"


def encode_metadata(value):
    if isinstance(value, str):
        return [value]
    return flatten(value)


def matrix(m):
    return [float(m[r][c]) for r in range(4) for c in range(4)]


def flat(matrices):
    return [v for m in matrices for v in matrix(m)]


def skel_info(prim, code, skel_cache):
    root = next((a for a in iter_ancestors(prim) if a.IsA(UsdSkel.Root)), None)
    if root is None:
        return None, None
    skel_root = UsdSkel.Root(root)
    skel_cache.Populate(skel_root, Usd.TraverseInstanceProxies(Usd.PrimDefaultPredicate))
    query = skel_cache.GetSkinningQuery(prim)
    if not query:
        return None, None
    skeleton = None
    for binding in skel_cache.ComputeSkelBindings(skel_root, Usd.TraverseInstanceProxies(Usd.PrimDefaultPredicate)):
        if any(t.GetPrim() == prim for t in binding.GetSkinningTargets()):
            skeleton = binding.GetSkeleton()
    if skeleton is None:
        return None, None
    skel_query = skel_cache.GetSkelQuery(skeleton)
    influences = query.ComputeJointInfluences(code)
    indices, weights = ([], []) if not influences else (list(influences[0]), list(influences[1]))
    order = query.GetJointOrder()
    if order is not None:
        skel_joints = list(skel_query.GetJointOrder())
        remapped_i, remapped_w = [], []
        for i, w in zip(indices, weights):
            name = order[i] if 0 <= i < len(order) else None
            if name in skel_joints:
                remapped_i.append(skel_joints.index(name))
                remapped_w.append(float(w))
            else:
                remapped_i.append(0)
                remapped_w.append(0.0)
        indices, weights = remapped_i, remapped_w
    shapes = list(query.GetBlendShapeOrder() or [])
    shape_weights = [0.0] * len(shapes)
    anim = skel_query.GetAnimQuery()
    if shapes and anim:
        anim_weights = anim.ComputeBlendShapeWeights(code)
        mapper = query.GetBlendShapeMapper()
        if anim_weights is not None and mapper:
            shape_weights = list(mapper.Remap(anim_weights))
    skin = {
        "skeleton": str(skeleton.GetPath()),
        "joint_indices": indices,
        "joint_weights": [float(w) for w in weights],
        "influences_per_point": query.GetNumInfluencesPerComponent(),
        "rigid": query.IsRigidlyDeformed(),
        "geom_bind_transform": matrix(query.GetGeomBindTransform(code)),
        "blend_shapes": [str(s) for s in shapes],
        "blend_shape_weights": [float(w) for w in shape_weights],
    }
    points = prim.GetAttribute("points").Get(code)
    xforms = skel_query.ComputeSkinningTransforms(code)
    skinned = None
    if points is not None and xforms is not None:
        points = list(points)
        if shapes and any(shape_weights):
            bq = UsdSkel.BlendShapeQuery(UsdSkel.BindingAPI(prim))
            sub_weights, blend_indices, sub_indices = bq.ComputeSubShapeWeights(shape_weights)
            pts = Vt_points(points)
            bq.ComputeDeformedPoints(sub_weights, blend_indices, sub_indices, bq.ComputeBlendShapePointIndices(), bq.ComputeSubShapePointOffsets(), pts)
            points = list(pts)
        if query.IsRigidlyDeformed():
            m = query.ComputeSkinnedTransform(xforms, code)
            skinned = [float(c) for p in points for c in m.Transform(Gf.Vec3d(p))]
        else:
            pts = Vt_points(points)
            if query.ComputeSkinnedPoints(xforms, pts, code):
                skinned = [float(c) for p in pts for c in p]
    return skin, skinned


def Vt_points(points):
    from pxr import Vt
    return Vt.Vec3fArray([Gf.Vec3f(*p) for p in points])


def iter_ancestors(prim):
    while prim and prim.GetPath() != Sdf.Path.absoluteRootPath:
        yield prim
        prim = prim.GetParent()


def pose_info(prim, code, skel_cache):
    if not prim.IsA(UsdSkel.Skeleton):
        return None
    q = skel_cache.GetSkelQuery(UsdSkel.Skeleton(prim))
    if not q:
        return None
    topo = q.GetTopology()
    return {
        "joints": [str(j) for j in q.GetJointOrder()],
        "parents": [int(topo.GetParent(i)) for i in range(topo.GetNumJoints())],
        "bind": flat(UsdSkel.Skeleton(prim).GetBindTransformsAttr().Get() or []),
        "rest_local": flat(UsdSkel.Skeleton(prim).GetRestTransformsAttr().Get() or []),
        "local": flat(q.ComputeJointLocalTransforms(code) or []),
        "skel": flat(q.ComputeJointSkelTransforms(code) or []),
        "skinning": flat(q.ComputeSkinningTransforms(code) or []),
    }


def instances_info(prim, code):
    if not prim.IsA(UsdGeom.PointInstancer):
        return None
    pi = UsdGeom.PointInstancer(prim)
    xforms = pi.ComputeInstanceTransformsAtTime(code, code)
    prototypes = [str(p) for p in pi.GetPrototypesRel().GetTargets()]
    if not xforms:
        return {"prototypes": prototypes, "proto_indices": [], "ids": [], "transforms": []}
    proto = list(pi.GetProtoIndicesAttr().Get(code) or [])
    ids = pi.GetIdsAttr().Get(code)
    ids = list(ids) if ids is not None else list(range(len(proto)))
    mask = list(pi.ComputeMaskAtTime(code))
    if mask:
        proto = [p for p, keep in zip(proto, mask) if keep]
        ids = [i for i, keep in zip(ids, mask) if keep]
    return {"prototypes": prototypes, "proto_indices": proto, "ids": ids, "transforms": flat(xforms)}


def resolved(prim, time, xforms, skel_cache):
    code = Usd.TimeCode.Default() if time is None else Usd.TimeCode(time)
    skin, skinned = skel_info(prim, code, skel_cache)
    local, resets = xforms.GetLocalTransformation(prim)
    primvars = {}
    for pv in UsdGeom.PrimvarsAPI(prim).FindPrimvarsWithInheritance():
        value = pv.ComputeFlattened(code)
        if value is None:
            continue
        primvars[pv.GetPrimvarName()] = {
            "value": encode(value, pv.GetTypeName()),
            "interpolation": str(pv.GetInterpolation()),
            "element_size": pv.GetElementSize(),
            "indexed": pv.IsIndexed(),
            "source": str(pv.GetAttr().GetPrim().GetPath()),
        }
    materials = {}
    for purpose in ["", "full", "preview"]:
        material = UsdShade.MaterialBindingAPI(prim).ComputeBoundMaterial(purpose)[0]
        materials[purpose] = str(material.GetPath()) if material else None
    return {
        "instances": instances_info(prim, code),
        "skin": skin,
        "skinned_points": skinned,
        "pose": pose_info(prim, code, skel_cache),
        "world": matrix(xforms.GetLocalToWorldTransform(prim)),
        "local": matrix(local),
        "resets": resets,
        "primvars": primvars,
        "materials": materials,
    }


def reference_dump(path, proxies, helpers=False, time=None):
    stage = Usd.Stage.Open(str(path))
    predicate = Usd.TraverseInstanceProxies(Usd.PrimAllPrimsPredicate) if proxies else Usd.PrimAllPrimsPredicate
    layer_md = {}
    for key in METADATA_KEYS:
        if stage.HasAuthoredMetadata(key):
            layer_md[key] = encode_metadata(stage.GetMetadata(key))
    prims = []
    xforms = UsdGeom.XformCache(Usd.TimeCode.Default() if time is None else Usd.TimeCode(time))
    skel_cache = UsdSkel.Cache()
    for prim in stage.Traverse(predicate):
        attributes = {}
        for attr in prim.GetAuthoredAttributes():
            type_name = attr.GetTypeName()
            samples = {}
            for t in attr.GetTimeSamples():
                samples[repr(float(t)).removesuffix(".0") if float(t).is_integer() else repr(float(t))] = encode(
                    attr.Get(t), type_name
                )
            attributes[attr.GetName()] = {
                "type": str(type_name),
                "default": encode(attr.Get(), type_name),
                "samples": samples,
                "connections": [str(p) for p in attr.GetConnections()],
            }
        relationships = {r.GetName(): [str(p) for p in r.GetTargets()] for r in prim.GetAuthoredRelationships()}
        prims.append(
            {
                "resolved": resolved(prim, time, xforms, skel_cache) if helpers else None,
                "path": str(prim.GetPath()),
                "type": str(prim.GetTypeName()),
                "specifier": {Sdf.SpecifierDef: "def", Sdf.SpecifierOver: "over", Sdf.SpecifierClass: "class"}[
                    prim.GetSpecifier()
                ],
                "kind": str(Usd.ModelAPI(prim).GetKind() or ""),
                "active": prim.IsActive(),
                "instance": prim.IsInstance(),
                "applied": sorted(prim.GetAppliedSchemas()),
                "attributes": attributes,
                "relationships": relationships,
            }
        )
    return {"metadata": layer_md, "prims": prims}


def same(a, b, rtol, atol, resolved):
    if isinstance(a, dict) and isinstance(b, dict) and set(a) == {"authored", "resolved"}:
        if a["authored"] != b.get("authored"):
            return False
        if not resolved:
            return True
        norm = lambda paths: [os.path.normpath(p) if p else "" for p in paths]
        return norm(a["resolved"]) == norm(b.get("resolved", []))
    if isinstance(a, dict) and isinstance(b, dict):
        return set(a) == set(b) and all(same(a[k], b[k], rtol, atol, resolved) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            return False
        return all(same(x, y, rtol, atol, resolved) for x, y in zip(a, b))
    if isinstance(a, float) and isinstance(b, float):
        return math.isclose(a, b, rel_tol=rtol, abs_tol=atol)
    return a == b


def diff(ours, ref, rtol, atol, resolved):
    errors = []
    for key in sorted(set(ours["metadata"]) | set(ref["metadata"])):
        if not same(ours["metadata"].get(key), ref["metadata"].get(key), rtol, atol, resolved):
            errors.append(f"metadata {key}: {ours['metadata'].get(key)} != {ref['metadata'].get(key)}")
    ours_prims = {p["path"]: p for p in ours["prims"]}
    ref_prims = {p["path"]: p for p in ref["prims"]}
    for path in [p["path"] for p in ref["prims"]]:
        if path not in ours_prims:
            errors.append(f"{path}: missing")
    for path in [p["path"] for p in ours["prims"]]:
        if path not in ref_prims:
            errors.append(f"{path}: unexpected")
    if [p for p in ours_prims if p in ref_prims] != [p for p in ref_prims if p in ours_prims]:
        errors.append("traversal order differs")
    for path, rp in ref_prims.items():
        op = ours_prims.get(path)
        if op is None:
            continue
        for field in ["type", "specifier", "kind", "active", "instance", "applied", "relationships"]:
            if op[field] != rp[field]:
                errors.append(f"{path} {field}: {op[field]} != {rp[field]}")
        if rp["resolved"] is not None:
            ores, rres = op["resolved"], rp["resolved"]
            for field in ["world", "local", "resets", "materials", "instances", "skin", "skinned_points", "pose"]:
                if not same(ores[field], rres[field], rtol, atol, resolved):
                    errors.append(f"{path} {field}: {ores[field]} != {rres[field]}")
            for name in sorted(set(ores["primvars"]) | set(rres["primvars"])):
                a, b = ores["primvars"].get(name), rres["primvars"].get(name)
                if a is None or b is None:
                    errors.append(f"{path} primvar {name}: {'missing' if a is None else 'unexpected'}")
                    continue
                for field in ["value", "interpolation", "element_size", "indexed", "source"]:
                    if not same(a[field], b[field], rtol, atol, resolved):
                        errors.append(f"{path} primvar {name} {field}: {a[field]} != {b[field]}")
        for name in sorted(set(op["attributes"]) | set(rp["attributes"])):
            oa, ra = op["attributes"].get(name), rp["attributes"].get(name)
            if oa is None or ra is None:
                errors.append(f"{path}.{name}: {'missing' if oa is None else 'unexpected'}")
                continue
            for field in ["type", "connections"]:
                if oa[field] != ra[field]:
                    errors.append(f"{path}.{name} {field}: {oa[field]} != {ra[field]}")
            if not same(oa["default"], ra["default"], rtol, atol, resolved):
                errors.append(f"{path}.{name} default: {oa['default']} != {ra['default']}")
            if sorted(oa["samples"]) != sorted(ra["samples"]):
                errors.append(f"{path}.{name} sample times: {sorted(oa['samples'])} != {sorted(ra['samples'])}")
            else:
                for t in ra["samples"]:
                    if not same(oa["samples"][t], ra["samples"][t], rtol, atol, resolved):
                        errors.append(f"{path}.{name} @ {t}: {oa['samples'][t]} != {ra['samples'][t]}")
    return errors


def main():
    parser = argparse.ArgumentParser(description="Diff a whole composed stage between tinyusd and OpenUSD (usd-core).")
    parser.add_argument("paths", nargs="+", type=Path)
    parser.add_argument("--dump", type=Path, default=DEFAULT_DUMP, help="path to the tinyusd dump example binary")
    parser.add_argument("--rtol", type=float, default=1e-6)
    parser.add_argument("--atol", type=float, default=1e-6)
    parser.add_argument("--no-resolved", action="store_true", help="skip comparing resolved asset paths")
    parser.add_argument("--max-errors", type=int, default=20)
    parser.add_argument("--instance-proxies", action="store_true", help="traverse into instance proxies")
    parser.add_argument("--helpers", action="store_true", help="also compare transforms, primvars and materials")
    parser.add_argument("--time", type=float, default=None, help="time code for the helper comparison")
    args = parser.parse_args()
    failed = 0
    for path in args.paths:
        cmd = [str(args.dump), *(["--instance-proxies"] if args.instance_proxies else [])]
        cmd += ["--helpers"] if args.helpers else []
        cmd += ["--time", repr(args.time)] if args.time is not None else []
        cmd.append(str(path))
        run = subprocess.run(cmd, capture_output=True, text=True)
        if run.returncode != 0:
            print(f"FAIL {path}\n    tinyusd: {run.stderr.strip()}")
            failed += 1
            continue
        errors = diff(json.loads(run.stdout), reference_dump(path, args.instance_proxies, args.helpers, args.time), args.rtol, args.atol, not args.no_resolved)
        print(f"{'ok' if not errors else 'FAIL'} {path}")
        for e in errors[: args.max_errors]:
            print(f"    {e}")
        if len(errors) > args.max_errors:
            print(f"    ... {len(errors) - args.max_errors} more")
        failed += bool(errors)
    print(f"{len(args.paths) - failed}/{len(args.paths)} matched")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
