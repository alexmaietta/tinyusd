use std::path::PathBuf;

use numpy::{Element, PyArray1, PyArrayMethods};
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyTuple};
use tinyusd_core::{self as core, BaseType, TraverseFlags};

create_exception!(_tinyusd, Error, PyException);
create_exception!(_tinyusd, OpenError, Error);
create_exception!(_tinyusd, InvalidPathError, Error);
create_exception!(_tinyusd, AssetError, Error);
create_exception!(_tinyusd, UnsupportedTypeError, Error);
create_exception!(_tinyusd, FeatureDisabledError, Error);
create_exception!(_tinyusd, SchemaError, Error);

fn py_err(e: core::Error) -> PyErr {
    match e {
        core::Error::Open { .. } => OpenError::new_err(e.to_string()),
        core::Error::InvalidPath(_) => InvalidPathError::new_err(e.to_string()),
        core::Error::Asset { .. } => AssetError::new_err(e.to_string()),
        core::Error::FeatureDisabled(_) => FeatureDisabledError::new_err(e.to_string()),
        core::Error::Schema { .. } => SchemaError::new_err(e.to_string()),
        core::Error::InvalidPrim | core::Error::Usd(_) => Error::new_err(e.to_string()),
    }
}

#[pyclass(frozen, eq, module = "tinyusd._tinyusd")]
#[derive(PartialEq)]
struct AssetPath {
    #[pyo3(get)]
    authored: String,
    #[pyo3(get)]
    resolved: String,
}

#[pymethods]
impl AssetPath {
    fn __repr__(&self) -> String {
        format!("AssetPath(authored={:?}, resolved={:?})", self.authored, self.resolved)
    }
}

#[pyclass(frozen, module = "tinyusd._tinyusd")]
struct AttributeInfo {
    #[pyo3(get)]
    type_name: String,
    #[pyo3(get)]
    uniform: bool,
    #[pyo3(get)]
    custom: bool,
    #[pyo3(get)]
    has_value: bool,
    #[pyo3(get)]
    has_authored_value: bool,
    #[pyo3(get)]
    might_be_time_varying: bool,
}

#[pymethods]
impl AttributeInfo {
    fn __repr__(&self) -> String {
        format!(
            "AttributeInfo(type_name={:?}, uniform={}, custom={}, has_value={}, might_be_time_varying={})",
            self.type_name, self.uniform, self.custom, self.has_value, self.might_be_time_varying
        )
    }
}

#[pyclass(frozen, module = "tinyusd._tinyusd")]
struct Primvar {
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    type_name: String,
    #[pyo3(get)]
    value: Py<PyAny>,
    #[pyo3(get)]
    interpolation: &'static str,
    #[pyo3(get)]
    element_size: i32,
    #[pyo3(get)]
    indexed: bool,
    #[pyo3(get)]
    source: Py<Prim>,
}

#[pymethods]
impl Primvar {
    fn __repr__(&self) -> String {
        format!(
            "Primvar(name={:?}, type_name={:?}, interpolation={:?}, element_size={}, indexed={})",
            self.name, self.type_name, self.interpolation, self.element_size, self.indexed
        )
    }
}

#[pyclass(frozen, get_all, module = "tinyusd._tinyusd")]
struct Instances {
    prototypes: Vec<Py<Prim>>,
    proto_indices: Py<PyAny>,
    ids: Py<PyAny>,
    transforms: Py<PyAny>,
}

#[pyclass(frozen, get_all, module = "tinyusd._tinyusd")]
struct SkelBinding {
    skeleton: Py<Prim>,
    animation: Option<Py<Prim>>,
    skinned: Vec<Py<Prim>>,
}

#[pyclass(frozen, get_all, module = "tinyusd._tinyusd")]
struct SkeletonPose {
    joints: Vec<String>,
    parents: Py<PyAny>,
    bind: Py<PyAny>,
    rest_local: Py<PyAny>,
    local: Py<PyAny>,
    skel: Py<PyAny>,
    skinning: Py<PyAny>,
}

#[pyclass(frozen, get_all, module = "tinyusd._tinyusd")]
struct SkinBinding {
    skeleton: Py<Prim>,
    joint_indices: Py<PyAny>,
    joint_weights: Py<PyAny>,
    influences_per_point: u32,
    rigid: bool,
    geom_bind_transform: Py<PyAny>,
    skinning_method: &'static str,
    blend_shapes: Vec<String>,
    blend_shape_weights: Py<PyAny>,
}

#[pyclass(frozen, get_all, module = "tinyusd._tinyusd")]
struct SubShape {
    blend_shape: u32,
    weight: f32,
    point_offsets: Py<PyAny>,
    normal_offsets: Py<PyAny>,
}

#[pyclass(frozen, get_all, module = "tinyusd._tinyusd")]
struct BlendShapes {
    point_indices: Vec<Py<PyAny>>,
    sub_shapes: Vec<Py<SubShape>>,
}

fn matrices(py: Python<'_>, m: &[[f64; 16]]) -> PyResult<Py<PyAny>> {
    Ok(PyArray1::from_slice(py, m.as_flattened())
        .reshape([m.len(), 4, 4])?
        .into_any()
        .unbind())
}

fn vec3s(py: Python<'_>, v: &[[f32; 3]]) -> PyResult<Py<PyAny>> {
    Ok(PyArray1::from_slice(py, v.as_flattened())
        .reshape([v.len(), 3])?
        .into_any()
        .unbind())
}

fn array<T: Element>(py: Python<'_>, v: &[T]) -> Py<PyAny> {
    PyArray1::from_slice(py, v).into_any().unbind()
}

fn matrix4<'py>(py: Python<'py>, m: &[f64; 16]) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyArray1::from_slice(py, m).reshape([4, 4])?.into_any())
}

fn numpy_array<'py, T: Element + bytemuck::Pod>(
    py: Python<'py>,
    v: &core::Value,
    shape: &[usize],
) -> PyResult<Bound<'py, PyAny>> {
    let data = v.as_slice::<T>().unwrap_or(&[]);
    Ok(PyArray1::from_slice(py, data).reshape(shape)?.into_any())
}

fn scalar_tuple<'py, T: bytemuck::Pod + IntoPyObject<'py>>(
    py: Python<'py>,
    v: &core::Value,
) -> PyResult<Bound<'py, PyAny>> {
    let items: Vec<T> = v.as_slice::<T>().unwrap_or(&[]).to_vec();
    let tuple = PyTuple::new(py, items)?;
    if tuple.len() == 1 {
        return tuple.get_item(0);
    }
    Ok(tuple.into_any())
}

fn half_tuple<'py>(py: Python<'py>, v: &core::Value) -> PyResult<Bound<'py, PyAny>> {
    let items: Vec<f32> = v
        .as_slice::<half::f16>()
        .unwrap_or(&[])
        .iter()
        .map(|h| h.to_f32())
        .collect();
    if items.len() == 1 {
        return Ok(items[0].into_pyobject(py)?.into_any());
    }
    Ok(PyTuple::new(py, items)?.into_any())
}

fn to_py<'py>(py: Python<'py>, v: &core::Value) -> PyResult<Option<Bound<'py, PyAny>>> {
    let base = v.base_type();
    let n = v.len();
    let array = v.is_array();
    let comps = base.component_count();
    let matrix = match base {
        BaseType::Matrix2d => Some(2),
        BaseType::Matrix3d => Some(3),
        BaseType::Matrix4d => Some(4),
        _ => None,
    };
    let shape: Vec<usize> = match (array, matrix) {
        (true, Some(m)) => vec![n, m, m],
        (false, Some(m)) => vec![m, m],
        (true, None) if comps > 1 => vec![n, comps],
        (true, None) => vec![n],
        (false, None) => vec![comps],
    };
    let obj = match base {
        BaseType::Empty => return Ok(None),
        BaseType::Unsupported => {
            return Err(UnsupportedTypeError::new_err(format!(
                "value of type '{}' is not supported",
                if v.type_name().is_empty() {
                    "unknown"
                } else {
                    v.type_name()
                }
            )));
        }
        BaseType::String | BaseType::Token | BaseType::Path => {
            if array || base == BaseType::Path {
                v.strings().into_pyobject(py)?.into_any()
            } else {
                v.strings()
                    .first()
                    .cloned()
                    .unwrap_or_default()
                    .into_pyobject(py)?
                    .into_any()
            }
        }
        BaseType::Asset => {
            let items: Vec<AssetPath> = v
                .strings()
                .iter()
                .zip(v.resolved_paths())
                .map(|(a, r)| AssetPath {
                    authored: a.clone(),
                    resolved: r.clone(),
                })
                .collect();
            if array {
                items.into_pyobject(py)?.into_any()
            } else {
                match items.into_iter().next() {
                    Some(a) => Bound::new(py, a)?.into_any(),
                    None => return Ok(None),
                }
            }
        }
        BaseType::Bool => {
            let bools: Vec<bool> = v.bytes().iter().map(|&b| b != 0).collect();
            if array {
                PyArray1::from_vec(py, bools).into_any()
            } else {
                bools
                    .first()
                    .copied()
                    .unwrap_or(false)
                    .into_pyobject(py)?
                    .to_owned()
                    .into_any()
            }
        }
        _ if matrix.is_some() => numpy_array::<f64>(py, v, &shape)?,
        BaseType::UChar if !array => scalar_tuple::<u8>(py, v)?,
        BaseType::UChar => numpy_array::<u8>(py, v, &shape)?,
        BaseType::Int | BaseType::Int2 | BaseType::Int3 | BaseType::Int4 if !array => scalar_tuple::<i32>(py, v)?,
        BaseType::Int | BaseType::Int2 | BaseType::Int3 | BaseType::Int4 => numpy_array::<i32>(py, v, &shape)?,
        BaseType::UInt if !array => scalar_tuple::<u32>(py, v)?,
        BaseType::UInt => numpy_array::<u32>(py, v, &shape)?,
        BaseType::Int64 if !array => scalar_tuple::<i64>(py, v)?,
        BaseType::Int64 => numpy_array::<i64>(py, v, &shape)?,
        BaseType::UInt64 if !array => scalar_tuple::<u64>(py, v)?,
        BaseType::UInt64 => numpy_array::<u64>(py, v, &shape)?,
        BaseType::Half | BaseType::Half2 | BaseType::Half3 | BaseType::Half4 | BaseType::Quath if !array => {
            half_tuple(py, v)?
        }
        BaseType::Half | BaseType::Half2 | BaseType::Half3 | BaseType::Half4 | BaseType::Quath => {
            numpy_array::<half::f16>(py, v, &shape)?
        }
        BaseType::Float | BaseType::Float2 | BaseType::Float3 | BaseType::Float4 | BaseType::Quatf if !array => {
            scalar_tuple::<f32>(py, v)?
        }
        BaseType::Float | BaseType::Float2 | BaseType::Float3 | BaseType::Float4 | BaseType::Quatf => {
            numpy_array::<f32>(py, v, &shape)?
        }
        _ if !array => scalar_tuple::<f64>(py, v)?,
        _ => numpy_array::<f64>(py, v, &shape)?,
    };
    Ok(Some(obj))
}

fn traverse_flags(
    include_inactive: bool,
    include_unloaded: bool,
    include_undefined: bool,
    include_abstract: bool,
    instance_proxies: bool,
) -> TraverseFlags {
    let mut bits = 0;
    for (on, flag) in [
        (include_inactive, TraverseFlags::INCLUDE_INACTIVE),
        (include_unloaded, TraverseFlags::INCLUDE_UNLOADED),
        (include_undefined, TraverseFlags::INCLUDE_UNDEFINED),
        (include_abstract, TraverseFlags::INCLUDE_ABSTRACT),
        (instance_proxies, TraverseFlags::INSTANCE_PROXIES),
    ] {
        if on {
            bits |= flag.bits();
        }
    }
    TraverseFlags::from_bits_truncate(bits)
}

struct PyResolver {
    object: Py<PyAny>,
}

impl core::AssetResolver for PyResolver {
    fn resolve(&self, asset_path: &str, anchor: Option<&str>) -> Option<String> {
        Python::attach(
            |py| match self.object.call_method1(py, "resolve", (asset_path, anchor)) {
                Ok(result) => result.extract::<Option<String>>(py).ok().flatten(),
                Err(e) => {
                    e.write_unraisable(py, Some(self.object.bind(py)));
                    None
                }
            },
        )
    }

    fn read(&self, resolved_path: &str) -> std::io::Result<Vec<u8>> {
        Python::attach(|py| {
            let result = self
                .object
                .call_method1(py, "read", (resolved_path,))
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            if result.is_none(py) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("resolver could not read '{resolved_path}'"),
                ));
            }
            result
                .extract::<Vec<u8>>(py)
                .map_err(|e| std::io::Error::other(e.to_string()))
        })
    }
}

#[pyclass(unsendable, module = "tinyusd._tinyusd")]
struct Stage {
    inner: core::Stage,
}

fn own(stage: &Bound<'_, Stage>, prim: &Prim) -> PyResult<core::Prim> {
    if prim.stage.bind(stage.py()).is(stage) {
        Ok(prim.handle)
    } else {
        Err(pyo3::exceptions::PyValueError::new_err(
            "prim belongs to a different stage",
        ))
    }
}

fn wrap(stage: &Bound<'_, Stage>, prims: Vec<core::Prim>) -> Vec<Prim> {
    prims
        .into_iter()
        .map(|p| Prim {
            stage: stage.clone().unbind(),
            handle: p,
        })
        .collect()
}

#[pymethods]
impl Stage {
    #[staticmethod]
    #[pyo3(signature = (path, *, load_payloads = true, resolver = None, schemas = Vec::new()))]
    fn open(path: PathBuf, load_payloads: bool, resolver: Option<Py<PyAny>>, schemas: Vec<PathBuf>) -> PyResult<Self> {
        let path = path.to_string_lossy().into_owned();
        let resolver =
            resolver.map(|object| std::rc::Rc::new(PyResolver { object }) as std::rc::Rc<dyn core::AssetResolver>);
        let options = core::OpenOptions {
            load_payloads,
            resolver,
            schema_paths: schemas,
        };
        let inner = core::Stage::open(&path, &options).map_err(py_err)?;
        Ok(Self { inner })
    }

    #[getter]
    fn pseudo_root(slf: &Bound<'_, Self>) -> Prim {
        Prim {
            stage: slf.clone().unbind(),
            handle: core::Prim::PSEUDO_ROOT,
        }
    }

    #[getter]
    fn default_prim(slf: &Bound<'_, Self>) -> PyResult<Option<Prim>> {
        let p = slf.borrow_mut().inner.default_prim().map_err(py_err)?;
        Ok(p.map(|handle| Prim {
            stage: slf.clone().unbind(),
            handle,
        }))
    }

    fn prim(slf: &Bound<'_, Self>, path: &str) -> PyResult<Option<Prim>> {
        let p = slf.borrow_mut().inner.prim_at_path(path).map_err(py_err)?;
        Ok(p.map(|handle| Prim {
            stage: slf.clone().unbind(),
            handle,
        }))
    }

    #[pyo3(signature = (root = None, *, include_inactive = false, include_unloaded = false, include_undefined = false, include_abstract = false, instance_proxies = false))]
    fn traverse(
        slf: &Bound<'_, Self>,
        root: Option<PyRef<'_, Prim>>,
        include_inactive: bool,
        include_unloaded: bool,
        include_undefined: bool,
        include_abstract: bool,
        instance_proxies: bool,
    ) -> PyResult<Vec<Prim>> {
        let flags = traverse_flags(
            include_inactive,
            include_unloaded,
            include_undefined,
            include_abstract,
            instance_proxies,
        );
        let root = root.map_or(Ok(core::Prim::PSEUDO_ROOT), |r| own(slf, &r))?;
        let prims = slf.borrow_mut().inner.traverse_from(root, flags).map_err(py_err)?;
        Ok(wrap(slf, prims))
    }

    #[pyo3(signature = (root = None))]
    fn skel_bindings(slf: &Bound<'_, Self>, root: Option<PyRef<'_, Prim>>) -> PyResult<Vec<SkelBinding>> {
        let py = slf.py();
        let root = root.map_or(Ok(core::Prim::PSEUDO_ROOT), |r| own(slf, &r))?;
        let bindings = slf.borrow_mut().inner.skel_bindings(root).map_err(py_err)?;
        let prim = |h| {
            Py::new(
                py,
                Prim {
                    stage: slf.clone().unbind(),
                    handle: h,
                },
            )
        };
        bindings
            .into_iter()
            .map(|b| {
                Ok(SkelBinding {
                    skeleton: prim(b.skeleton)?,
                    animation: b.animation.map(prim).transpose()?,
                    skinned: b.skinned.into_iter().map(prim).collect::<PyResult<_>>()?,
                })
            })
            .collect()
    }

    #[pyo3(signature = (*, include = None, exclude = None, compute_mass = true, si_units = false))]
    fn physics<'py>(
        slf: &Bound<'py, Self>,
        include: Option<Vec<PyRef<'py, Prim>>>,
        exclude: Option<Vec<PyRef<'py, Prim>>>,
        compute_mass: bool,
        si_units: bool,
    ) -> PyResult<Bound<'py, PyDict>> {
        let py = slf.py();
        let own_all = |list: Option<Vec<PyRef<'py, Prim>>>| -> PyResult<Vec<core::Prim>> {
            list.unwrap_or_default().iter().map(|p| own(slf, p)).collect()
        };
        let options = core::PhysicsOptions {
            include: own_all(include)?,
            exclude: own_all(exclude)?,
            compute_mass,
            si_units,
        };
        let phys = slf.borrow_mut().inner.physics(&options).map_err(py_err)?;
        physics_dict(py, slf, &phys)
    }

    fn prototypes(slf: &Bound<'_, Self>) -> PyResult<Vec<Prim>> {
        let prims = slf.borrow_mut().inner.prototypes().map_err(py_err)?;
        Ok(wrap(slf, prims))
    }

    fn metadata<'py>(&self, py: Python<'py>, key: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        let mut v = core::Value::new();
        if !self.inner.stage_metadata(key, &mut v).map_err(py_err)? {
            return Ok(None);
        }
        to_py(py, &v)
    }

    fn has_authored_metadata(&self, key: &str) -> PyResult<bool> {
        self.inner.has_authored_stage_metadata(key).map_err(py_err)
    }

    #[getter]
    fn start_time_code(&self) -> f64 {
        self.inner.start_time_code()
    }

    #[getter]
    fn end_time_code(&self) -> f64 {
        self.inner.end_time_code()
    }

    #[getter]
    fn time_codes_per_second(&self) -> f64 {
        self.inner.time_codes_per_second()
    }

    #[getter]
    fn frames_per_second(&self) -> f64 {
        self.inner.frames_per_second()
    }

    #[getter]
    fn layer_identifiers(&self) -> Vec<String> {
        self.inner.layer_identifiers()
    }

    #[getter]
    fn composition_errors(&self) -> Vec<String> {
        self.inner.composition_errors()
    }

    #[pyo3(signature = (prim, *, with_descendants = true))]
    fn load(slf: &Bound<'_, Self>, prim: PyRef<'_, Prim>, with_descendants: bool) -> PyResult<()> {
        let handle = own(slf, &prim)?;
        slf.borrow_mut().inner.load(handle, with_descendants).map_err(py_err)
    }

    fn unload(slf: &Bound<'_, Self>, prim: PyRef<'_, Prim>) -> PyResult<()> {
        let handle = own(slf, &prim)?;
        slf.borrow_mut().inner.unload(handle).map_err(py_err)
    }

    #[pyo3(signature = (root = None))]
    fn loadable(slf: &Bound<'_, Self>, root: Option<PyRef<'_, Prim>>) -> PyResult<Vec<Prim>> {
        let root = root.map_or(Ok(core::Prim::PSEUDO_ROOT), |r| own(slf, &r))?;
        let prims = slf.borrow_mut().inner.loadable(root).map_err(py_err)?;
        Ok(wrap(slf, prims))
    }

    fn read_asset<'py>(&self, py: Python<'py>, resolved_path: &str) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = self.inner.read_asset(resolved_path).map_err(py_err)?;
        Ok(PyBytes::new(py, &bytes))
    }
}

#[pyclass(frozen, module = "tinyusd._tinyusd")]
struct Prim {
    stage: Py<Stage>,
    handle: core::Prim,
}

impl Prim {
    fn with<R>(&self, py: Python<'_>, f: impl FnOnce(&mut core::Stage, core::Prim) -> core::Result<R>) -> PyResult<R> {
        let mut stage = self.stage.bind(py).borrow_mut();
        f(&mut stage.inner, self.handle).map_err(py_err)
    }

    fn sibling(&self, py: Python<'_>, handle: core::Prim) -> Prim {
        Prim {
            stage: self.stage.clone_ref(py),
            handle,
        }
    }

    fn flag(&self, py: Python<'_>, flag: core::PrimFlags) -> PyResult<bool> {
        self.with(py, |s, p| s.flags(p)).map(|f| f.contains(flag))
    }
}

#[pymethods]
impl Prim {
    #[getter]
    fn path(&self, py: Python<'_>) -> PyResult<String> {
        self.with(py, |s, p| s.path(p).map(str::to_owned))
    }

    #[getter]
    fn name(&self, py: Python<'_>) -> PyResult<String> {
        self.with(py, |s, p| s.name(p).map(str::to_owned))
    }

    #[getter]
    fn type_name(&self, py: Python<'_>) -> PyResult<String> {
        self.with(py, |s, p| s.type_name(p))
    }

    #[getter]
    fn kind(&self, py: Python<'_>) -> PyResult<String> {
        self.with(py, |s, p| s.kind(p))
    }

    #[getter]
    fn specifier(&self, py: Python<'_>) -> PyResult<&'static str> {
        Ok(match self.with(py, |s, p| s.specifier(p))? {
            core::Specifier::Def => "def",
            core::Specifier::Over => "over",
            core::Specifier::Class => "class",
        })
    }

    #[getter]
    fn active(&self, py: Python<'_>) -> PyResult<bool> {
        self.flag(py, core::PrimFlags::ACTIVE)
    }

    #[getter]
    fn loaded(&self, py: Python<'_>) -> PyResult<bool> {
        self.flag(py, core::PrimFlags::LOADED)
    }

    #[getter]
    fn defined(&self, py: Python<'_>) -> PyResult<bool> {
        self.flag(py, core::PrimFlags::DEFINED)
    }

    #[getter]
    fn abstract_(&self, py: Python<'_>) -> PyResult<bool> {
        self.flag(py, core::PrimFlags::ABSTRACT)
    }

    #[getter]
    fn model(&self, py: Python<'_>) -> PyResult<bool> {
        self.flag(py, core::PrimFlags::MODEL)
    }

    #[getter]
    fn instanceable(&self, py: Python<'_>) -> PyResult<bool> {
        self.flag(py, core::PrimFlags::INSTANCEABLE)
    }

    #[getter]
    fn instance(&self, py: Python<'_>) -> PyResult<bool> {
        self.flag(py, core::PrimFlags::INSTANCE)
    }

    #[getter]
    fn instance_proxy(&self, py: Python<'_>) -> PyResult<bool> {
        self.flag(py, core::PrimFlags::INSTANCE_PROXY)
    }

    #[getter]
    fn prototype_root(&self, py: Python<'_>) -> PyResult<bool> {
        self.flag(py, core::PrimFlags::PROTOTYPE)
    }

    #[getter]
    fn in_prototype(&self, py: Python<'_>) -> PyResult<bool> {
        self.flag(py, core::PrimFlags::IN_PROTOTYPE)
    }

    #[getter]
    fn parent(&self, py: Python<'_>) -> PyResult<Option<Prim>> {
        Ok(self.with(py, |s, p| s.parent(p))?.map(|h| self.sibling(py, h)))
    }

    #[getter]
    fn prototype(&self, py: Python<'_>) -> PyResult<Option<Prim>> {
        Ok(self.with(py, |s, p| s.prototype_of(p))?.map(|h| self.sibling(py, h)))
    }

    fn instances(&self, py: Python<'_>) -> PyResult<Vec<Prim>> {
        Ok(self
            .with(py, |s, p| s.instances_of(p))?
            .into_iter()
            .map(|h| self.sibling(py, h))
            .collect())
    }

    #[pyo3(signature = (*, include_inactive = false, include_unloaded = false, include_undefined = false, include_abstract = false, instance_proxies = false))]
    fn children(
        &self,
        py: Python<'_>,
        include_inactive: bool,
        include_unloaded: bool,
        include_undefined: bool,
        include_abstract: bool,
        instance_proxies: bool,
    ) -> PyResult<Vec<Prim>> {
        let flags = traverse_flags(
            include_inactive,
            include_unloaded,
            include_undefined,
            include_abstract,
            instance_proxies,
        );
        Ok(self
            .with(py, |s, p| s.children(p, flags))?
            .into_iter()
            .map(|h| self.sibling(py, h))
            .collect())
    }

    fn is_a(&self, py: Python<'_>, schema: &str) -> PyResult<bool> {
        self.with(py, |s, p| s.is_a(p, schema))
    }

    fn has_api(&self, py: Python<'_>, schema: &str) -> PyResult<bool> {
        self.with(py, |s, p| s.has_api(p, schema))
    }

    #[getter]
    fn applied_schemas(&self, py: Python<'_>) -> PyResult<Vec<String>> {
        self.with(py, |s, p| s.applied_schemas(p))
    }

    #[pyo3(signature = (*, authored_only = false))]
    fn attribute_names(&self, py: Python<'_>, authored_only: bool) -> PyResult<Vec<String>> {
        self.with(py, |s, p| s.attribute_names(p, authored_only))
    }

    #[pyo3(signature = (*, authored_only = false))]
    fn relationship_names(&self, py: Python<'_>, authored_only: bool) -> PyResult<Vec<String>> {
        self.with(py, |s, p| s.relationship_names(p, authored_only))
    }

    #[pyo3(signature = (name, time = None))]
    fn get<'py>(&self, py: Python<'py>, name: &str, time: Option<f64>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let mut v = core::Value::new();
        if !self.with(py, |s, p| s.get(p, name, time, &mut v))? {
            return Ok(None);
        }
        to_py(py, &v)
    }

    fn attribute_info(&self, py: Python<'_>, name: &str) -> PyResult<Option<AttributeInfo>> {
        Ok(self.with(py, |s, p| s.attribute_info(p, name))?.map(|i| AttributeInfo {
            type_name: i.type_name,
            uniform: i.uniform,
            custom: i.custom,
            has_value: i.has_value,
            has_authored_value: i.has_authored_value,
            might_be_time_varying: i.might_be_time_varying,
        }))
    }

    fn time_samples(&self, py: Python<'_>, name: &str) -> PyResult<Vec<f64>> {
        self.with(py, |s, p| s.time_samples(p, name))
    }

    fn connections(&self, py: Python<'_>, name: &str) -> PyResult<Vec<String>> {
        self.with(py, |s, p| s.connections(p, name))
    }

    #[pyo3(signature = (name, *, forwarded = false))]
    fn targets(&self, py: Python<'_>, name: &str, forwarded: bool) -> PyResult<Vec<String>> {
        self.with(py, |s, p| s.targets(p, name, forwarded))
    }

    fn metadata<'py>(&self, py: Python<'py>, key: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        let mut v = core::Value::new();
        if !self.with(py, |s, p| s.prim_metadata(p, key, &mut v))? {
            return Ok(None);
        }
        to_py(py, &v)
    }

    fn attribute_metadata<'py>(&self, py: Python<'py>, name: &str, key: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        let mut v = core::Value::new();
        if !self.with(py, |s, p| s.attribute_metadata(p, name, key, &mut v))? {
            return Ok(None);
        }
        to_py(py, &v)
    }

    #[pyo3(signature = (time = None))]
    fn world_transform<'py>(&self, py: Python<'py>, time: Option<f64>) -> PyResult<Bound<'py, PyAny>> {
        let m = self.with(py, |s, p| s.world_transform(p, time))?;
        matrix4(py, &m)
    }

    #[pyo3(signature = (time = None))]
    fn local_transform<'py>(&self, py: Python<'py>, time: Option<f64>) -> PyResult<(Bound<'py, PyAny>, bool)> {
        let (m, resets) = self.with(py, |s, p| s.local_transform(p, time))?;
        Ok((matrix4(py, &m)?, resets))
    }

    fn primvar_names(&self, py: Python<'_>) -> PyResult<Vec<String>> {
        self.with(py, |s, p| s.primvar_names(p))
    }

    #[pyo3(signature = (name, time = None))]
    fn primvar(&self, py: Python<'_>, name: &str, time: Option<f64>) -> PyResult<Option<Primvar>> {
        let mut v = core::Value::new();
        let Some(info) = self.with(py, |s, p| s.primvar(p, name, time, &mut v))? else {
            return Ok(None);
        };
        let value = to_py(py, &v)?.map_or_else(|| py.None(), Bound::unbind);
        Ok(Some(Primvar {
            name: info.name,
            type_name: info.type_name,
            value,
            interpolation: match info.interpolation {
                core::Interpolation::Constant => "constant",
                core::Interpolation::Uniform => "uniform",
                core::Interpolation::Varying => "varying",
                core::Interpolation::Vertex => "vertex",
                core::Interpolation::FaceVarying => "faceVarying",
            },
            element_size: info.element_size,
            indexed: info.indexed,
            source: Py::new(py, self.sibling(py, info.source))?,
        }))
    }

    #[pyo3(signature = (time = None, *, include_prototype_transform = true, apply_visibility = true))]
    fn instancer_instances(
        &self,
        py: Python<'_>,
        time: Option<f64>,
        include_prototype_transform: bool,
        apply_visibility: bool,
    ) -> PyResult<Option<Instances>> {
        let Some(i) = self.with(py, |s, p| {
            s.instancer_instances(p, time, include_prototype_transform, apply_visibility)
        })?
        else {
            return Ok(None);
        };
        Ok(Some(Instances {
            prototypes: i
                .prototypes
                .into_iter()
                .map(|h| Py::new(py, self.sibling(py, h)))
                .collect::<PyResult<_>>()?,
            proto_indices: array(py, &i.proto_indices),
            ids: array(py, &i.ids),
            transforms: matrices(py, &i.transforms)?,
        }))
    }

    #[pyo3(signature = (time = None))]
    fn skeleton_pose(&self, py: Python<'_>, time: Option<f64>) -> PyResult<Option<SkeletonPose>> {
        let Some(p) = self.with(py, |s, p| s.skeleton_pose(p, time))? else {
            return Ok(None);
        };
        Ok(Some(SkeletonPose {
            parents: array(py, &p.parents),
            bind: matrices(py, &p.bind)?,
            rest_local: matrices(py, &p.rest_local)?,
            local: matrices(py, &p.local)?,
            skel: matrices(py, &p.skel)?,
            skinning: matrices(py, &p.skinning)?,
            joints: p.joints,
        }))
    }

    #[pyo3(signature = (time = None))]
    fn skin_binding(&self, py: Python<'_>, time: Option<f64>) -> PyResult<Option<SkinBinding>> {
        let Some(b) = self.with(py, |s, p| s.skin_binding(p, time))? else {
            return Ok(None);
        };
        let k = b.influences_per_point.max(1) as usize;
        let rows = b.joint_indices.len() / k;
        Ok(Some(SkinBinding {
            skeleton: Py::new(py, self.sibling(py, b.skeleton))?,
            joint_indices: PyArray1::from_slice(py, &b.joint_indices)
                .reshape([rows, k])?
                .into_any()
                .unbind(),
            joint_weights: PyArray1::from_slice(py, &b.joint_weights)
                .reshape([rows, k])?
                .into_any()
                .unbind(),
            influences_per_point: b.influences_per_point,
            rigid: b.rigid,
            geom_bind_transform: matrix4(py, &b.geom_bind_transform)?.unbind(),
            skinning_method: match b.skinning_method {
                core::SkinningMethod::ClassicLinear => "classicLinear",
                core::SkinningMethod::DualQuaternion => "dualQuaternion",
            },
            blend_shape_weights: array(py, &b.blend_shape_weights),
            blend_shapes: b.blend_shapes,
        }))
    }

    #[pyo3(signature = (time = None))]
    fn blend_shapes(&self, py: Python<'_>, time: Option<f64>) -> PyResult<Option<BlendShapes>> {
        let Some(b) = self.with(py, |s, p| s.blend_shapes(p, time))? else {
            return Ok(None);
        };
        Ok(Some(BlendShapes {
            point_indices: b.point_indices.iter().map(|v| array(py, v)).collect(),
            sub_shapes: b
                .sub_shapes
                .iter()
                .map(|s| {
                    Py::new(
                        py,
                        SubShape {
                            blend_shape: s.blend_shape,
                            weight: s.weight,
                            point_offsets: vec3s(py, &s.point_offsets)?,
                            normal_offsets: vec3s(py, &s.normal_offsets)?,
                        },
                    )
                })
                .collect::<PyResult<_>>()?,
        }))
    }

    #[pyo3(signature = (time = None))]
    fn skinned_points(&self, py: Python<'_>, time: Option<f64>) -> PyResult<Option<Py<PyAny>>> {
        self.with(py, |s, p| s.skinned_points(p, time))?
            .map(|pts| vec3s(py, &pts))
            .transpose()
    }

    #[pyo3(signature = (purpose = ""))]
    fn bound_material(&self, py: Python<'_>, purpose: &str) -> PyResult<Option<Prim>> {
        Ok(self
            .with(py, |s, p| s.bound_material(p, purpose))?
            .map(|b| self.sibling(py, b.material)))
    }

    fn __eq__(&self, py: Python<'_>, other: &Prim) -> bool {
        self.stage.bind(py).is(other.stage.bind(py)) && self.handle == other.handle
    }

    fn __hash__(&self) -> u64 {
        u64::from(self.handle.index())
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!("Prim({:?})", self.path(py).unwrap_or_default())
    }
}

#[pyfunction]
fn features() -> Vec<&'static str> {
    [
        (core::Feature::Geom, "geom"),
        (core::Feature::Shade, "shade"),
        (core::Feature::Lux, "lux"),
        (core::Feature::Skel, "skel"),
        (core::Feature::Physics, "physics"),
        (core::Feature::Vol, "vol"),
        (core::Feature::Media, "media"),
        (core::Feature::Semantics, "semantics"),
    ]
    .into_iter()
    .filter(|(f, _)| core::has_feature(*f))
    .map(|(_, name)| name)
    .collect()
}

#[pymodule]
fn _tinyusd(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add("__version__", core::VERSION)?;
    m.add("Error", py.get_type::<Error>())?;
    m.add("OpenError", py.get_type::<OpenError>())?;
    m.add("InvalidPathError", py.get_type::<InvalidPathError>())?;
    m.add("AssetError", py.get_type::<AssetError>())?;
    m.add("UnsupportedTypeError", py.get_type::<UnsupportedTypeError>())?;
    m.add("FeatureDisabledError", py.get_type::<FeatureDisabledError>())?;
    m.add("SchemaError", py.get_type::<SchemaError>())?;
    m.add_class::<Stage>()?;
    m.add_class::<Prim>()?;
    m.add_class::<AssetPath>()?;
    m.add_class::<AttributeInfo>()?;
    m.add_class::<Primvar>()?;
    m.add_class::<Instances>()?;
    m.add_class::<SkelBinding>()?;
    m.add_class::<SkeletonPose>()?;
    m.add_class::<SkinBinding>()?;
    m.add_class::<SubShape>()?;
    m.add_class::<BlendShapes>()?;
    m.add_function(wrap_pyfunction!(features, m)?)?;
    Ok(())
}

fn physics_dict<'py>(py: Python<'py>, stage: &Bound<'py, Stage>, p: &core::Physics) -> PyResult<Bound<'py, PyDict>> {
    let prim = |h: core::Prim| {
        Py::new(
            py,
            Prim {
                stage: stage.clone().unbind(),
                handle: h,
            },
        )
    };
    let opt = |h: Option<core::Prim>| -> PyResult<Option<Py<Prim>>> { h.map(prim).transpose() };
    let prims = |hs: &[core::Prim]| -> PyResult<Vec<Py<Prim>>> { hs.iter().map(|&h| prim(h)).collect() };
    let name = |v: &dyn std::fmt::Debug| format!("{v:?}");
    let out = PyDict::new(py);
    out.set_item("meters_per_unit", p.meters_per_unit)?;
    out.set_item("kilograms_per_unit", p.kilograms_per_unit)?;
    out.set_item("si_units", p.si_units)?;
    let list = PyList::empty(py);
    for x in &p.scenes {
        let d = PyDict::new(py);
        d.set_item("prim", prim(x.prim)?)?;
        d.set_item("gravity_direction", x.gravity_direction)?;
        d.set_item("gravity_magnitude", x.gravity_magnitude)?;
        list.append(d)?;
    }
    out.set_item("scenes", list)?;
    let list = PyList::empty(py);
    for x in &p.collision_groups {
        let d = PyDict::new(py);
        d.set_item("prim", prim(x.prim)?)?;
        d.set_item("invert_filtered_groups", x.invert_filtered_groups)?;
        d.set_item("filtered_groups", prims(&x.filtered_groups)?)?;
        d.set_item("merge_group_name", &x.merge_group_name)?;
        d.set_item("merged_groups", prims(&x.merged_groups)?)?;
        list.append(d)?;
    }
    out.set_item("collision_groups", list)?;
    let list = PyList::empty(py);
    for x in &p.materials {
        let d = PyDict::new(py);
        d.set_item("prim", prim(x.prim)?)?;
        d.set_item("static_friction", x.static_friction)?;
        d.set_item("dynamic_friction", x.dynamic_friction)?;
        d.set_item("restitution", x.restitution)?;
        d.set_item("density", x.density)?;
        list.append(d)?;
    }
    out.set_item("materials", list)?;
    let list = PyList::empty(py);
    for x in &p.shapes {
        let d = PyDict::new(py);
        d.set_item("prim", prim(x.prim)?)?;
        d.set_item("rigid_body", opt(x.rigid_body)?)?;
        d.set_item("local_pos", x.local_pos)?;
        d.set_item("local_rot", x.local_rot)?;
        d.set_item("local_scale", x.local_scale)?;
        d.set_item("materials", prims(&x.materials)?)?;
        d.set_item("simulation_owners", prims(&x.simulation_owners)?)?;
        d.set_item("filtered_collisions", prims(&x.filtered_collisions)?)?;
        d.set_item("collision_groups", prims(&x.collision_groups)?)?;
        d.set_item("collision_enabled", x.collision_enabled)?;
        d.set_item("kind", name(&x.kind))?;
        d.set_item("radius", x.radius)?;
        d.set_item("top_radius", x.top_radius)?;
        d.set_item("bottom_radius", x.bottom_radius)?;
        d.set_item("half_height", x.half_height)?;
        d.set_item("half_extents", x.half_extents)?;
        d.set_item("axis", name(&x.axis))?;
        d.set_item("mesh_approximation", &x.mesh_approximation)?;
        d.set_item("mesh_scale", x.mesh_scale)?;
        d.set_item("double_sided", x.double_sided)?;
        d.set_item("sphere_points", x.sphere_points.clone())?;
        d.set_item("custom_type", &x.custom_type)?;
        list.append(d)?;
    }
    out.set_item("shapes", list)?;
    let list = PyList::empty(py);
    for x in &p.rigid_bodies {
        let d = PyDict::new(py);
        d.set_item("prim", prim(x.prim)?)?;
        d.set_item("collisions", prims(&x.collisions)?)?;
        d.set_item("filtered_collisions", prims(&x.filtered_collisions)?)?;
        d.set_item("simulation_owners", prims(&x.simulation_owners)?)?;
        d.set_item("position", x.position)?;
        d.set_item("rotation", x.rotation)?;
        d.set_item("scale", x.scale)?;
        d.set_item("enabled", x.enabled)?;
        d.set_item("kinematic", x.kinematic)?;
        d.set_item("starts_asleep", x.starts_asleep)?;
        d.set_item("linear_velocity", x.linear_velocity)?;
        d.set_item("angular_velocity", x.angular_velocity)?;
        match &x.mass {
            Some(m) => {
                let md = PyDict::new(py);
                md.set_item("mass", m.mass)?;
                md.set_item("diagonal_inertia", m.diagonal_inertia)?;
                md.set_item("center_of_mass", m.center_of_mass)?;
                md.set_item("principal_axes", m.principal_axes)?;
                d.set_item("mass", md)?;
            }
            None => d.set_item("mass", py.None())?,
        }
        list.append(d)?;
    }
    out.set_item("rigid_bodies", list)?;
    let list = PyList::empty(py);
    for x in &p.joints {
        let d = PyDict::new(py);
        d.set_item("prim", prim(x.prim)?)?;
        d.set_item("kind", name(&x.kind))?;
        d.set_item("type_name", &x.type_name)?;
        d.set_item("rel0", opt(x.rel0)?)?;
        d.set_item("rel1", opt(x.rel1)?)?;
        d.set_item("body0", opt(x.body0)?)?;
        d.set_item("body1", opt(x.body1)?)?;
        d.set_item("local_pos0", x.local_pos0)?;
        d.set_item("local_rot0", x.local_rot0)?;
        d.set_item("local_pos1", x.local_pos1)?;
        d.set_item("local_rot1", x.local_rot1)?;
        d.set_item("enabled", x.enabled)?;
        d.set_item("collision_enabled", x.collision_enabled)?;
        d.set_item("break_force", x.break_force)?;
        d.set_item("break_torque", x.break_torque)?;
        d.set_item("exclude_from_articulation", x.exclude_from_articulation)?;
        d.set_item("axis", name(&x.axis))?;
        let limits = PyList::empty(py);
        for l in &x.limits {
            let ld = PyDict::new(py);
            ld.set_item("dof", name(&l.dof))?;
            ld.set_item("enabled", l.enabled)?;
            ld.set_item("lower", l.lower)?;
            ld.set_item("upper", l.upper)?;
            limits.append(ld)?;
        }
        d.set_item("limits", limits)?;
        let drives = PyList::empty(py);
        for v in &x.drives {
            let dd = PyDict::new(py);
            dd.set_item("dof", name(&v.dof))?;
            dd.set_item("target_position", v.target_position)?;
            dd.set_item("target_velocity", v.target_velocity)?;
            dd.set_item("force_limit", v.force_limit)?;
            dd.set_item("stiffness", v.stiffness)?;
            dd.set_item("damping", v.damping)?;
            dd.set_item("acceleration", v.acceleration)?;
            drives.append(dd)?;
        }
        d.set_item("drives", drives)?;
        d.set_item("min_distance_enabled", x.min_distance_enabled)?;
        d.set_item("max_distance_enabled", x.max_distance_enabled)?;
        list.append(d)?;
    }
    out.set_item("joints", list)?;
    let list = PyList::empty(py);
    for x in &p.articulations {
        let d = PyDict::new(py);
        d.set_item("prim", prim(x.prim)?)?;
        d.set_item("roots", prims(&x.roots)?)?;
        d.set_item("filtered_collisions", prims(&x.filtered_collisions)?)?;
        d.set_item("joints", prims(&x.joints)?)?;
        d.set_item(
            "bodies",
            x.bodies.iter().map(|b| opt(*b)).collect::<PyResult<Vec<_>>>()?,
        )?;
        list.append(d)?;
    }
    out.set_item("articulations", list)?;
    out.set_item("diagnostics", p.diagnostics.clone())?;
    Ok(out)
}
