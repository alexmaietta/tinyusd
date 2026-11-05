import argparse
from pathlib import Path

import numpy as np

import tinyusd


def triangulate(counts, indices):
    tris = []
    start = 0
    for count in counts:
        for k in range(1, count - 1):
            tris.append((indices[start], indices[start + k], indices[start + k + 1]))
        start += count
    return np.asarray(tris, dtype=np.int32).reshape(-1, 3)


def labels(prim):
    out = {}
    for schema in prim.applied_schemas:
        name, _, taxonomy = schema.partition(":")
        if name == "SemanticsLabelsAPI" and taxonomy:
            out[taxonomy] = prim.get(f"semantics:labels:{taxonomy}") or []
    return out


def inherited_labels(prim):
    merged = {}
    node = prim
    while node is not None:
        for taxonomy, values in labels(node).items():
            merged.setdefault(taxonomy, [])
            merged[taxonomy] += [v for v in values if v not in merged[taxonomy]]
        node = node.parent
    return merged


def export(path, out, time=None):
    stage = tinyusd.Stage.open(path)
    arrays = {}
    summary = []
    for prim in stage.traverse(instance_proxies=True):
        if not prim.is_a("Mesh"):
            continue
        points = prim.get("points", time)
        counts = prim.get("faceVertexCounts", time)
        indices = prim.get("faceVertexIndices", time)
        if points is None or counts is None or indices is None:
            continue
        world = prim.world_transform(time)
        homogeneous = np.c_[points.astype(np.float64), np.ones(len(points))]
        key = prim.path.strip("/").replace("/", "_")
        arrays[f"{key}/points"] = (homogeneous @ world)[:, :3].astype(np.float32)
        arrays[f"{key}/triangles"] = triangulate(counts, indices)
        tags = inherited_labels(prim)
        for taxonomy, values in tags.items():
            arrays[f"{key}/labels/{taxonomy}"] = np.asarray(values)
        summary.append((prim.path, len(points), len(arrays[f"{key}/triangles"]), tags))
    np.savez_compressed(out, **arrays)
    return summary


def main():
    parser = argparse.ArgumentParser(description="Export world-space meshes and semantic labels to an .npz file.")
    parser.add_argument("usd", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--time", type=float, default=None)
    args = parser.parse_args()
    for path, points, triangles, tags in export(args.usd, args.out, args.time):
        print(f"{path}: {points} points, {triangles} triangles, labels {tags}")


if __name__ == "__main__":
    main()
