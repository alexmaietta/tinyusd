from os import PathLike
from typing import Any, Optional, Protocol, Union

import numpy as np
import numpy.typing as npt

__version__: str

class Error(Exception): ...
class OpenError(Error): ...
class InvalidPathError(Error): ...
class AssetError(Error): ...
class UnsupportedTypeError(Error): ...
class FeatureDisabledError(Error): ...

class AssetPath:
    authored: str
    resolved: str

class AttributeInfo:
    type_name: str
    uniform: bool
    custom: bool
    has_value: bool
    has_authored_value: bool
    might_be_time_varying: bool

class Primvar:
    name: str
    type_name: str
    value: Any
    interpolation: str
    element_size: int
    indexed: bool
    source: Prim

class Instances:
    prototypes: list[Prim]
    proto_indices: npt.NDArray[np.int32]
    ids: npt.NDArray[np.int64]
    transforms: npt.NDArray[np.float64]

class SkelBinding:
    skeleton: Prim
    animation: Optional[Prim]
    skinned: list[Prim]

class SkeletonPose:
    joints: list[str]
    parents: npt.NDArray[np.int32]
    bind: npt.NDArray[np.float64]
    rest_local: npt.NDArray[np.float64]
    local: npt.NDArray[np.float64]
    skel: npt.NDArray[np.float64]
    skinning: npt.NDArray[np.float64]

class SkinBinding:
    skeleton: Prim
    joint_indices: npt.NDArray[np.int32]
    joint_weights: npt.NDArray[np.float32]
    influences_per_point: int
    rigid: bool
    geom_bind_transform: npt.NDArray[np.float64]
    skinning_method: str
    blend_shapes: list[str]
    blend_shape_weights: npt.NDArray[np.float32]

class SubShape:
    blend_shape: int
    weight: float
    point_offsets: npt.NDArray[np.float32]
    normal_offsets: npt.NDArray[np.float32]

class BlendShapes:
    point_indices: list[npt.NDArray[np.int32]]
    sub_shapes: list[SubShape]

class Prim:
    path: str
    name: str
    type_name: str
    kind: str
    specifier: str
    active: bool
    loaded: bool
    defined: bool
    abstract_: bool
    model: bool
    instanceable: bool
    instance: bool
    instance_proxy: bool
    prototype_root: bool
    in_prototype: bool
    parent: Optional[Prim]
    prototype: Optional[Prim]
    applied_schemas: list[str]
    def instances(self) -> list[Prim]: ...
    def children(
        self,
        *,
        include_inactive: bool = False,
        include_unloaded: bool = False,
        include_undefined: bool = False,
        include_abstract: bool = False,
        instance_proxies: bool = False,
    ) -> list[Prim]: ...
    def is_a(self, schema: str) -> bool: ...
    def has_api(self, schema: str) -> bool: ...
    def attribute_names(self, *, authored_only: bool = False) -> list[str]: ...
    def relationship_names(self, *, authored_only: bool = False) -> list[str]: ...
    def get(self, name: str, time: Optional[float] = None) -> Any: ...
    def attribute_info(self, name: str) -> Optional[AttributeInfo]: ...
    def time_samples(self, name: str) -> list[float]: ...
    def connections(self, name: str) -> list[str]: ...
    def targets(self, name: str, *, forwarded: bool = False) -> list[str]: ...
    def metadata(self, key: str) -> Any: ...
    def attribute_metadata(self, name: str, key: str) -> Any: ...
    def world_transform(self, time: Optional[float] = None) -> npt.NDArray[np.float64]: ...
    def local_transform(self, time: Optional[float] = None) -> tuple[npt.NDArray[np.float64], bool]: ...
    def primvar_names(self) -> list[str]: ...
    def primvar(self, name: str, time: Optional[float] = None) -> Optional[Primvar]: ...
    def bound_material(self, purpose: str = "") -> Optional[Prim]: ...
    def instancer_instances(
        self, time: Optional[float] = None, *, include_prototype_transform: bool = True, apply_visibility: bool = True
    ) -> Optional[Instances]: ...
    def skeleton_pose(self, time: Optional[float] = None) -> Optional[SkeletonPose]: ...
    def skin_binding(self, time: Optional[float] = None) -> Optional[SkinBinding]: ...
    def blend_shapes(self, time: Optional[float] = None) -> Optional[BlendShapes]: ...
    def skinned_points(self, time: Optional[float] = None) -> Optional[npt.NDArray[np.float32]]: ...

class Resolver(Protocol):
    def resolve(self, asset_path: str, anchor: Optional[str]) -> Optional[str]: ...
    def read(self, resolved_path: str) -> Optional[bytes]: ...

class Stage:
    pseudo_root: Prim
    default_prim: Optional[Prim]
    start_time_code: float
    end_time_code: float
    time_codes_per_second: float
    frames_per_second: float
    layer_identifiers: list[str]
    composition_errors: list[str]
    @staticmethod
    def open(
        path: Union[str, PathLike[str]], *, load_payloads: bool = True, resolver: Optional[Resolver] = None
    ) -> Stage: ...
    def load(self, prim: Prim, *, with_descendants: bool = True) -> None: ...
    def unload(self, prim: Prim) -> None: ...
    def loadable(self, root: Optional[Prim] = None) -> list[Prim]: ...
    def prim(self, path: str) -> Optional[Prim]: ...
    def traverse(
        self,
        root: Optional[Prim] = None,
        *,
        include_inactive: bool = False,
        include_unloaded: bool = False,
        include_undefined: bool = False,
        include_abstract: bool = False,
        instance_proxies: bool = False,
    ) -> list[Prim]: ...
    def prototypes(self) -> list[Prim]: ...
    def skel_bindings(self, root: Optional[Prim] = None) -> list[SkelBinding]: ...
    def metadata(self, key: str) -> Any: ...
    def has_authored_metadata(self, key: str) -> bool: ...
    def read_asset(self, resolved_path: str) -> bytes: ...

def features() -> list[str]: ...
