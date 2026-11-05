from pathlib import Path

import pytest

import tinyusd

DATA = Path(__file__).resolve().parents[3] / "tests" / "data"


class MemoryResolver:
    def __init__(self):
        self.files = {f"mem://assets/{n}": (DATA / n).read_bytes() for n in ["query.usda", "part.usda", "textured.usdz"]}
        self.reads = []

    def resolve(self, asset_path, anchor):
        if not asset_path.startswith("mem://"):
            if anchor is None:
                return None
            asset_path = anchor.rsplit("/", 1)[0] + "/" + asset_path.removeprefix("./")
        return asset_path if asset_path in self.files else None

    def read(self, resolved_path):
        self.reads.append(resolved_path)
        if resolved_path not in self.files:
            raise FileNotFoundError(resolved_path)
        return self.files[resolved_path]


def test_stage_from_memory():
    resolver = MemoryResolver()
    stage = tinyusd.Stage.open("mem://assets/query.usda", resolver=resolver)
    assert len(stage.traverse()) == 10
    assert stage.prim("/World/Payloaded").type_name == "Mesh"
    assert stage.prim("/World/Types").get("a").resolved == "mem://assets/part.usda"
    assert {"mem://assets/query.usda", "mem://assets/part.usda"} <= set(resolver.reads)
    with pytest.raises(tinyusd.AssetError, match="missing.png"):
        stage.read_asset("mem://assets/missing.png")


def test_package_from_memory():
    resolver = MemoryResolver()
    stage = tinyusd.Stage.open("mem://assets/textured.usdz", resolver=resolver)
    tex = stage.prim("/Root/Texture").get("inputs:file")
    assert tex.resolved == "mem://assets/textured.usdz[tex.png]"
    for _ in range(3):
        assert stage.read_asset(tex.resolved) == b"tinyusd-texture-bytes"
    assert resolver.reads == ["mem://assets/textured.usdz"]


def test_missing_root():
    with pytest.raises(tinyusd.OpenError):
        tinyusd.Stage.open("mem://assets/nope.usda", resolver=MemoryResolver())


def test_load_and_unload():
    stage = tinyusd.Stage.open(DATA / "query.usda", load_payloads=False)
    payloaded = stage.prim("/World/Payloaded")
    assert stage.loadable() == [payloaded] and not payloaded.loaded
    stage.load(payloaded)
    assert payloaded.loaded and payloaded.type_name == "Mesh"
    assert payloaded.get("points").shape == (3, 3)
    stage.unload(payloaded)
    assert not payloaded.loaded and payloaded.get("points") is None


def test_rejects_prims_from_other_stages():
    a = tinyusd.Stage.open(DATA / "query.usda", load_payloads=False)
    b = tinyusd.Stage.open(DATA / "query.usda", load_payloads=False)
    with pytest.raises(ValueError, match="different stage"):
        a.load(b.prim("/World/Payloaded"))
    with pytest.raises(ValueError):
        a.traverse(b.prim("/World"))
