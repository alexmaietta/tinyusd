import importlib.util
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[3]


def load_example(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "examples" / "python" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_dataset_export(tmp_path):
    dataset = load_example("dataset")
    out = tmp_path / "scene.npz"
    summary = dataset.export(ROOT / "tests" / "data" / "scene.usda", out)
    assert [s[0] for s in summary] == ["/World/Table/Top", "/World/Part"]
    assert summary[0][3] == {"class": ["table", "furniture"]}
    data = np.load(out)
    assert data["World_Table_Top/triangles"].tolist() == [[0, 1, 2], [0, 2, 3]]
    assert data["World_Table_Top/points"][0].tolist() == [10.0, 0.0, 0.0]
    assert data["World_Part/labels/class"].tolist() == ["furniture"]
