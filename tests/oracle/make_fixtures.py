import sys
from pathlib import Path

from pxr import Sdf, UsdUtils

data = Path(sys.argv[1] if len(sys.argv) > 1 else Path(__file__).resolve().parents[1] / "data")
Sdf.Layer.FindOrOpen(str(data / "query.usda")).Export(str(data / "query.usdc"))
out = data / "textured.usdz"
out.unlink(missing_ok=True)
if not UsdUtils.CreateNewUsdzPackage(Sdf.AssetPath(str(data / "textured.usda")), str(out)):
    sys.exit("failed to create textured.usdz")
