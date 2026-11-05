from pathlib import Path

import pytest

import tinyusd

DATA = Path(__file__).resolve().parents[3] / "tests" / "data"


def test_registered_schema():
    stage = tinyusd.Stage.open(DATA / "custom.usda", schemas=[DATA / "schemas" / "demo"])
    widget = stage.prim("/World/Widget")
    assert widget.is_a("DemoWidget") and widget.is_a("Xformable") and widget.has_api("DemoTagAPI")
    assert widget.get("demo:tag") == 7
    defaults = stage.prim("/World/Defaults")
    assert defaults.get("demo:size") == 2.5 and defaults.get("demo:mode") == "fast"
    assert defaults.get("slot:right:weight") == 1.0
    info = defaults.attribute_info("demo:size")
    assert info.has_value and not info.has_authored_value


def test_without_schema():
    stage = tinyusd.Stage.open(DATA / "custom.usda")
    assert not stage.prim("/World/Widget").is_a("Xformable")
    assert stage.prim("/World/Defaults").get("demo:size") is None


def test_missing_schema():
    with pytest.raises(tinyusd.SchemaError):
        tinyusd.Stage.open(DATA / "custom.usda", schemas=[DATA / "schemas" / "nope"])
