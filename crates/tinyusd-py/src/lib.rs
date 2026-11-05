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
create_exception!(_tinyusd, InvalidValueError, Error);
create_exception!(_tinyusd, WriteError, Error);

fn py_err(e: core::Error) -> PyErr {
    match e {
        core::Error::Open { .. } => OpenError::new_err(e.to_string()),
        core::Error::InvalidPath(_) => InvalidPathError::new_err(e.to_string()),
        core::Error::Asset { .. } => AssetError::new_err(e.to_string()),
        core::Error::FeatureDisabled(_) => FeatureDisabledError::new_err(e.to_string()),
        core::Error::Schema { .. } => SchemaError::new_err(e.to_string()),
        core::Error::InvalidValue(_) => InvalidValueError::new_err(e.to_string()),
        core::Error::Write { .. } => WriteError::new_err(e.to_string()),
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

    fn write(&self, resolved_path: &str, bytes: &[u8]) -> std::io::Result<()> {
        Python::attach(|py| {
            let object = self.object.bind(py);
            if !object.hasattr("write").unwrap_or(false) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    format!("the resolver has no write method for '{resolved_path}'"),
                ));
            }
            let result = object
                .call_method1("write", (resolved_path, PyBytes::new(py, bytes)))
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            if result.is_none() || result.is_truthy().unwrap_or(false) {
                Ok(())
            } else {
                Err(std::io::Error::other(format!(
                    "resolver could not write '{resolved_path}'"
                )))
            }
        })
    }
}

fn numpy_dtype(base: BaseType) -> &'static str {
    match base {
        BaseType::Bool => "bool",
        BaseType::UChar => "uint8",
        BaseType::Int | BaseType::Int2 | BaseType::Int3 | BaseType::Int4 => "int32",
        BaseType::UInt => "uint32",
        BaseType::Int64 => "int64",
        BaseType::UInt64 => "uint64",
        BaseType::Half | BaseType::Half2 | BaseType::Half3 | BaseType::Half4 | BaseType::Quath => "float16",
        BaseType::Float | BaseType::Float2 | BaseType::Float3 | BaseType::Float4 | BaseType::Quatf => "float32",
        _ => "float64",
    }
}

fn from_py(py: Python<'_>, value: &Bound<'_, PyAny>, type_name: &str) -> PyResult<core::Value> {
    let (base, array) = BaseType::from_type_name(type_name)
        .ok_or_else(|| InvalidValueError::new_err(format!("values of type '{type_name}' can't be written")))?;
    if base.is_string_like() {
        let one = |item: &Bound<'_, PyAny>| -> PyResult<String> {
            match item.cast::<AssetPath>() {
                Ok(asset) => Ok(asset.get().authored.clone()),
                Err(_) => item
                    .extract::<String>()
                    .map_err(|_| InvalidValueError::new_err(format!("'{type_name}' takes str values"))),
            }
        };
        let items = if array {
            if value.is_instance_of::<pyo3::types::PyString>() {
                return Err(InvalidValueError::new_err(format!("'{type_name}' takes a list of str")));
            }
            value
                .try_iter()?
                .map(|item| one(&item?))
                .collect::<PyResult<Vec<_>>>()?
        } else {
            vec![one(value)?]
        };
        return core::Value::from_strings(base, array, &items).map_err(py_err);
    }
    let np = py.import("numpy")?;
    let dtype = numpy_dtype(base);
    let typed = value.is_instance(&np.getattr("ndarray")?)? || value.is_instance(&np.getattr("generic")?)?;
    let converted = if typed {
        let have: String = value.getattr("dtype")?.getattr("name")?.extract()?;
        if have != dtype {
            return Err(InvalidValueError::new_err(format!(
                "'{type_name}' takes {dtype} data, got {have}; convert it with .astype(numpy.{dtype}) first"
            )));
        }
        np.call_method1("ascontiguousarray", (value,))?
    } else {
        let raw = np.call_method1("asarray", (value,))?;
        let kind: String = raw.getattr("dtype")?.getattr("kind")?.extract()?;
        let size: usize = raw.getattr("size")?.extract()?;
        let allowed = match dtype {
            "bool" => kind == "b",
            "float16" | "float32" | "float64" => matches!(kind.as_str(), "f" | "i" | "u"),
            _ => matches!(kind.as_str(), "i" | "u"),
        };
        if !allowed && size > 0 {
            return Err(InvalidValueError::new_err(format!(
                "'{type_name}' can't take {} values",
                raw.getattr("dtype")?.getattr("name")?
            )));
        }
        let kwargs = PyDict::new(py);
        kwargs.set_item("dtype", dtype)?;
        np.getattr("asarray")?.call((value,), Some(&kwargs))?
    };
    let bytes: Vec<u8> = converted.call_method0("tobytes")?.extract()?;
    core::Value::from_bytes(base, array, &bytes).map_err(py_err)
}

fn doubles<const N: usize>(py: Python<'_>, value: &Bound<'_, PyAny>, what: &str) -> PyResult<[f64; N]> {
    let v = from_py(py, value, "double[]")?;
    let items = v.as_slice::<f64>().unwrap_or(&[]);
    items
        .try_into()
        .map_err(|_| InvalidValueError::new_err(format!("{what} takes {N} numbers, got {}", items.len())))
}

fn interpolation(name: &str) -> PyResult<core::Interpolation> {
    Ok(match name {
        "constant" => core::Interpolation::Constant,
        "uniform" => core::Interpolation::Uniform,
        "varying" => core::Interpolation::Varying,
        "vertex" => core::Interpolation::Vertex,
        "faceVarying" => core::Interpolation::FaceVarying,
        other => {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "unknown interpolation {other:?}"
            )));
        }
    })
}

fn open_options(load_payloads: bool, resolver: Option<Py<PyAny>>, schemas: Vec<PathBuf>) -> core::OpenOptions {
    core::OpenOptions {
        load_payloads,
        resolver: resolver
            .map(|object| std::rc::Rc::new(PyResolver { object }) as std::rc::Rc<dyn core::AssetResolver>),
        schema_paths: schemas,
    }
}

#[pyclass(unsendable, module = "tinyusd._tinyusd")]
struct EditBatch {
    stage: Py<Stage>,
}

#[pymethods]
impl EditBatch {
    fn __enter__(slf: PyRef<'_, Self>) -> PyResult<PyRef<'_, Self>> {
        slf.stage
            .bind(slf.py())
            .borrow_mut()
            .inner
            .begin_edit()
            .map_err(py_err)?;
        Ok(slf)
    }

    fn __exit__(
        &self,
        py: Python<'_>,
        exc_type: Option<Bound<'_, PyAny>>,
        _exc: Option<Bound<'_, PyAny>>,
        _tb: Option<Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let mut stage = self.stage.bind(py).borrow_mut();
        if exc_type.is_some() {
            stage.inner.discard_edit();
        } else {
            stage.inner.commit_edit().map_err(py_err)?;
        }
        Ok(false)
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
        let options = open_options(load_payloads, resolver, schemas);
        let inner = core::Stage::open(&path, &options).map_err(py_err)?;
        Ok(Self { inner })
    }

    #[staticmethod]
    #[pyo3(signature = (path, *, resolver = None, schemas = Vec::new()))]
    fn create_new(path: PathBuf, resolver: Option<Py<PyAny>>, schemas: Vec<PathBuf>) -> PyResult<Self> {
        let path = path.to_string_lossy().into_owned();
        let options = open_options(true, resolver, schemas);
        let inner = core::Stage::create_new(&path, &options).map_err(py_err)?;
        Ok(Self { inner })
    }

    #[staticmethod]
    #[pyo3(signature = (*, resolver = None, schemas = Vec::new()))]
    fn create_in_memory(resolver: Option<Py<PyAny>>, schemas: Vec<PathBuf>) -> PyResult<Self> {
        let options = open_options(true, resolver, schemas);
        let inner = core::Stage::create_in_memory(&options).map_err(py_err)?;
        Ok(Self { inner })
    }

    fn save(&mut self) -> PyResult<()> {
        self.inner.save().map_err(py_err)
    }

    fn export(&self, path: PathBuf) -> PyResult<()> {
        self.inner.export(&path.to_string_lossy()).map_err(py_err)
    }

    #[pyo3(signature = (physics, *, poses = false))]
    fn write_physics(slf: &Bound<'_, Self>, physics: &Bound<'_, PyDict>, poses: bool) -> PyResult<()> {
        let p = physics_input::physics(slf, physics)?;
        slf.borrow_mut()
            .inner
            .write_physics(&p, &core::PhysicsWriteOptions { poses })
            .map_err(py_err)
    }

    fn write_body_mass(slf: &Bound<'_, Self>, prim: PyRef<'_, Prim>, mass: &Bound<'_, PyDict>) -> PyResult<()> {
        let handle = own(slf, &prim)?;
        let m = physics_input::body_mass(mass, slf)?;
        slf.borrow_mut().inner.write_body_mass(handle, &m).map_err(py_err)
    }

    #[pyo3(signature = (prim, new_parent, new_name = None))]
    fn move_prim(
        slf: &Bound<'_, Self>,
        prim: PyRef<'_, Prim>,
        new_parent: PyRef<'_, Prim>,
        new_name: Option<&str>,
    ) -> PyResult<()> {
        let (handle, parent) = (own(slf, &prim)?, own(slf, &new_parent)?);
        let mut stage = slf.borrow_mut();
        let name = match new_name {
            Some(n) => n.to_owned(),
            None => stage.inner.name(handle).map_err(py_err)?.to_owned(),
        };
        stage.inner.move_prim(handle, parent, &name).map_err(py_err)
    }

    fn export_package(&self, path: PathBuf) -> PyResult<()> {
        self.inner.export_package(&path.to_string_lossy()).map_err(py_err)
    }

    fn export_layer(&self, layer: &str, path: PathBuf) -> PyResult<()> {
        self.inner.export_layer(layer, &path.to_string_lossy()).map_err(py_err)
    }

    #[pyo3(signature = (format = "usda", layer = None))]
    fn export_bytes<'py>(&self, py: Python<'py>, format: &str, layer: Option<&str>) -> PyResult<Bound<'py, PyBytes>> {
        let format = match format {
            "usda" => core::LayerFormat::Usda,
            "usdc" => core::LayerFormat::Usdc,
            "usdz" => core::LayerFormat::Usdz,
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "format must be 'usda', 'usdc' or 'usdz', not {other:?}"
                )));
            }
        };
        let bytes = self.inner.export_bytes(layer, format).map_err(py_err)?;
        Ok(PyBytes::new(py, &bytes))
    }

    #[getter]
    fn edit_target(&self) -> String {
        self.inner.edit_target()
    }

    fn set_edit_target(&mut self, layer: &str) -> PyResult<()> {
        self.inner.set_edit_target(layer).map_err(py_err)
    }

    #[pyo3(signature = (asset_path, *, create = false))]
    fn add_sublayer(&mut self, asset_path: &str, create: bool) -> PyResult<String> {
        self.inner.add_sublayer(asset_path, create).map_err(py_err)
    }

    fn edit(slf: &Bound<'_, Self>) -> EditBatch {
        EditBatch {
            stage: slf.clone().unbind(),
        }
    }

    fn begin_edit(&mut self) -> PyResult<()> {
        self.inner.begin_edit().map_err(py_err)
    }

    fn commit_edit(&mut self) -> PyResult<()> {
        self.inner.commit_edit().map_err(py_err)
    }

    fn discard_edit(&mut self) {
        self.inner.discard_edit();
    }

    #[pyo3(signature = (path, type_name = ""))]
    fn define_prim(slf: &Bound<'_, Self>, path: &str, type_name: &str) -> PyResult<Prim> {
        let handle = slf.borrow_mut().inner.define_prim(path, type_name).map_err(py_err)?;
        Ok(Prim {
            stage: slf.clone().unbind(),
            handle,
        })
    }

    fn override_prim(slf: &Bound<'_, Self>, path: &str) -> PyResult<Prim> {
        let handle = slf.borrow_mut().inner.override_prim(path).map_err(py_err)?;
        Ok(Prim {
            stage: slf.clone().unbind(),
            handle,
        })
    }

    fn remove_prim(slf: &Bound<'_, Self>, prim: PyRef<'_, Prim>) -> PyResult<bool> {
        let handle = own(slf, &prim)?;
        slf.borrow_mut().inner.remove_prim(handle).map_err(py_err)
    }

    fn set_default_prim(slf: &Bound<'_, Self>, prim: PyRef<'_, Prim>) -> PyResult<()> {
        let handle = own(slf, &prim)?;
        slf.borrow_mut().inner.set_default_prim(handle).map_err(py_err)
    }

    fn set_up_axis(&mut self, axis: &str) -> PyResult<()> {
        self.inner.set_up_axis(axis).map_err(py_err)
    }

    fn set_meters_per_unit(&mut self, meters: f64) -> PyResult<()> {
        self.inner.set_meters_per_unit(meters).map_err(py_err)
    }

    fn set_kilograms_per_unit(&mut self, kilograms: f64) -> PyResult<()> {
        self.inner.set_kilograms_per_unit(kilograms).map_err(py_err)
    }

    #[pyo3(signature = (start, end, *, time_codes_per_second = None, frames_per_second = None))]
    fn set_time_range(
        &mut self,
        start: f64,
        end: f64,
        time_codes_per_second: Option<f64>,
        frames_per_second: Option<f64>,
    ) -> PyResult<()> {
        self.inner.set_time_code_range(start, end).map_err(py_err)?;
        if let Some(rate) = time_codes_per_second {
            self.inner.set_time_codes_per_second(rate).map_err(py_err)?;
        }
        if let Some(rate) = frames_per_second {
            self.inner.set_frames_per_second(rate).map_err(py_err)?;
        }
        Ok(())
    }

    fn set_documentation(&mut self, text: &str) -> PyResult<()> {
        self.inner.set_documentation(text).map_err(py_err)
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

    #[pyo3(signature = (*, include = None, exclude = None, si_units = false))]
    fn physics<'py>(
        slf: &Bound<'py, Self>,
        include: Option<Vec<PyRef<'py, Prim>>>,
        exclude: Option<Vec<PyRef<'py, Prim>>>,
        si_units: bool,
    ) -> PyResult<Bound<'py, PyDict>> {
        let py = slf.py();
        let own_all = |list: Option<Vec<PyRef<'py, Prim>>>| -> PyResult<Vec<core::Prim>> {
            list.unwrap_or_default().iter().map(|p| own(slf, p)).collect()
        };
        let options = core::PhysicsOptions {
            include: own_all(include)?,
            exclude: own_all(exclude)?,
            si_units,
        };
        let phys = slf.borrow_mut().inner.physics(&options).map_err(py_err)?;
        physics_dict(py, slf, &phys)
    }

    fn mass_colliders(slf: &Bound<'_, Self>, body: PyRef<'_, Prim>) -> PyResult<Vec<Prim>> {
        let handle = own(slf, &body)?;
        let colliders = slf.borrow_mut().inner.mass_colliders(handle).map_err(py_err)?;
        Ok(wrap(slf, colliders))
    }

    fn mass_properties<'py>(
        slf: &Bound<'py, Self>,
        body: PyRef<'py, Prim>,
        mass_info: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let py = slf.py();
        let handle = own(slf, &body)?;
        let colliders = slf.borrow_mut().inner.mass_colliders(handle).map_err(py_err)?;
        let defaults = core::MassInformation::default();
        let mut table = std::collections::HashMap::new();
        for collider in colliders {
            let prim = Py::new(
                py,
                Prim {
                    stage: slf.clone().unbind(),
                    handle: collider,
                },
            )?;
            let out = mass_info.call1((prim,))?;
            if out.is_none() {
                continue;
            }
            let raw = out.get_item("inertia")?;
            let inertia: Vec<f64> = match raw.extract::<Vec<Vec<f64>>>() {
                Ok(rows) => rows.into_iter().flatten().collect(),
                Err(_) => raw.extract::<Vec<f64>>()?,
            };
            let inertia: [f64; 9] = inertia
                .try_into()
                .map_err(|_| pyo3::exceptions::PyValueError::new_err("inertia must have 9 values"))?;
            let field3 = |key: &str, default: [f64; 3]| -> PyResult<[f64; 3]> {
                match out.get_item(key) {
                    Ok(v) => v.extract(),
                    Err(_) => Ok(default),
                }
            };
            let local_rot = match out.get_item("local_rot") {
                Ok(v) => v.extract()?,
                Err(_) => defaults.local_rot,
            };
            table.insert(
                collider,
                core::MassInformation {
                    volume: out.get_item("volume")?.extract()?,
                    inertia,
                    center_of_mass: field3("center_of_mass", defaults.center_of_mass)?,
                    local_pos: field3("local_pos", defaults.local_pos)?,
                    local_rot,
                },
            );
        }
        let mass = slf
            .borrow_mut()
            .inner
            .mass_properties(handle, |c| table.get(&c).copied())
            .map_err(py_err)?;
        let d = PyDict::new(py);
        d.set_item("mass", mass.mass)?;
        d.set_item("diagonal_inertia", mass.diagonal_inertia)?;
        d.set_item("center_of_mass", mass.center_of_mass)?;
        d.set_item("principal_axes", mass.principal_axes)?;
        Ok(d)
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
        Ok(self.with(py, |s, p| s.prototype(p))?.map(|h| self.sibling(py, h)))
    }

    fn instances(&self, py: Python<'_>) -> PyResult<Vec<Prim>> {
        Ok(self
            .with(py, |s, p| s.instances(p))?
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

    #[pyo3(signature = (name, value, time = None))]
    fn set(&self, py: Python<'_>, name: &str, value: &Bound<'_, PyAny>, time: Option<f64>) -> PyResult<()> {
        let declared = self.with(py, |s, p| s.declared_type(p, name))?.ok_or_else(|| {
            InvalidValueError::new_err(format!(
                "attribute '{name}' is not declared; create it with create_attribute first"
            ))
        })?;
        let value = from_py(py, value, &declared)?;
        self.with(py, |s, p| s.set(p, name, &value, time))
    }

    #[pyo3(signature = (name, type_name, *, uniform = false))]
    fn create_attribute(&self, py: Python<'_>, name: &str, type_name: &str, uniform: bool) -> PyResult<()> {
        self.with(py, |s, p| s.create_attribute(p, name, type_name, uniform))
    }

    fn block(&self, py: Python<'_>, name: &str) -> PyResult<()> {
        self.with(py, |s, p| s.block(p, name))
    }

    fn clear(&self, py: Python<'_>, name: &str) -> PyResult<()> {
        self.with(py, |s, p| s.clear(p, name))
    }

    fn set_connections(&self, py: Python<'_>, name: &str, targets: Vec<String>) -> PyResult<()> {
        let targets: Vec<&str> = targets.iter().map(String::as_str).collect();
        self.with(py, |s, p| s.set_connections(p, name, &targets))
    }

    fn set_targets(&self, py: Python<'_>, name: &str, targets: Vec<String>) -> PyResult<()> {
        let targets: Vec<&str> = targets.iter().map(String::as_str).collect();
        self.with(py, |s, p| s.set_targets(p, name, &targets))
    }

    fn apply_api(&self, py: Python<'_>, schema: &str) -> PyResult<()> {
        self.with(py, |s, p| s.apply_api(p, schema))
    }

    fn set_type_name(&self, py: Python<'_>, type_name: &str) -> PyResult<()> {
        self.with(py, |s, p| s.set_type_name(p, type_name))
    }

    fn set_active(&self, py: Python<'_>, active: bool) -> PyResult<()> {
        self.with(py, |s, p| s.set_active(p, active))
    }

    fn set_kind(&self, py: Python<'_>, kind: &str) -> PyResult<()> {
        self.with(py, |s, p| s.set_kind(p, kind))
    }

    fn set_instanceable(&self, py: Python<'_>, instanceable: bool) -> PyResult<()> {
        self.with(py, |s, p| s.set_instanceable(p, instanceable))
    }

    #[pyo3(signature = (asset_path = "", prim_path = ""))]
    fn add_reference(&self, py: Python<'_>, asset_path: &str, prim_path: &str) -> PyResult<()> {
        self.with(py, |s, p| s.add_reference(p, asset_path, prim_path))
    }

    #[pyo3(signature = (asset_path, prim_path = ""))]
    fn add_payload(&self, py: Python<'_>, asset_path: &str, prim_path: &str) -> PyResult<()> {
        self.with(py, |s, p| s.add_payload(p, asset_path, prim_path))
    }

    fn add_inherit(&self, py: Python<'_>, path: &str) -> PyResult<()> {
        self.with(py, |s, p| s.add_inherit(p, path))
    }

    fn add_specialize(&self, py: Python<'_>, path: &str) -> PyResult<()> {
        self.with(py, |s, p| s.add_specialize(p, path))
    }

    fn set_variant_selection(&self, py: Python<'_>, variant_set: &str, variant: &str) -> PyResult<()> {
        self.with(py, |s, p| s.set_variant_selection(p, variant_set, variant))
    }

    #[pyo3(signature = (matrix, time = None))]
    fn set_local_transform(&self, py: Python<'_>, matrix: &Bound<'_, PyAny>, time: Option<f64>) -> PyResult<()> {
        let m = doubles::<16>(py, matrix, "matrix")?;
        self.with(py, |s, p| s.set_local_transform(p, &m, time))
    }

    #[pyo3(signature = (matrix, time = None))]
    fn set_world_transform(&self, py: Python<'_>, matrix: &Bound<'_, PyAny>, time: Option<f64>) -> PyResult<()> {
        let m = doubles::<16>(py, matrix, "matrix")?;
        self.with(py, |s, p| s.set_world_transform(p, &m, time))
    }

    #[pyo3(signature = (translate, orient = None, scale = None, time = None))]
    fn set_trs(
        &self,
        py: Python<'_>,
        translate: &Bound<'_, PyAny>,
        orient: Option<&Bound<'_, PyAny>>,
        scale: Option<&Bound<'_, PyAny>>,
        time: Option<f64>,
    ) -> PyResult<()> {
        let t = doubles::<3>(py, translate, "translate")?;
        let o = orient.map_or(Ok([1.0, 0.0, 0.0, 0.0]), |o| doubles::<4>(py, o, "orient"))?;
        let sc = scale.map_or(Ok([1.0; 3]), |v| doubles::<3>(py, v, "scale"))?;
        self.with(py, |s, p| s.set_trs(p, t, o, sc, time))
    }

    #[pyo3(signature = (points, face_vertex_counts, face_vertex_indices, time = None))]
    fn set_mesh(
        &self,
        py: Python<'_>,
        points: &Bound<'_, PyAny>,
        face_vertex_counts: &Bound<'_, PyAny>,
        face_vertex_indices: &Bound<'_, PyAny>,
        time: Option<f64>,
    ) -> PyResult<()> {
        let points = from_py(py, points, "point3f[]")?;
        let counts = from_py(py, face_vertex_counts, "int[]")?;
        let indices = from_py(py, face_vertex_indices, "int[]")?;
        self.with(py, |s, p| {
            s.set_mesh(
                p,
                points.as_slice().unwrap_or(&[]),
                counts.as_slice().unwrap_or(&[]),
                indices.as_slice().unwrap_or(&[]),
                time,
            )
        })
    }

    #[pyo3(signature = (name, type_name, value, interpolation = "constant", indices = None, element_size = 0, time = None))]
    #[allow(clippy::too_many_arguments)]
    fn set_primvar(
        &self,
        py: Python<'_>,
        name: &str,
        type_name: &str,
        value: &Bound<'_, PyAny>,
        interpolation: &str,
        indices: Option<&Bound<'_, PyAny>>,
        element_size: i32,
        time: Option<f64>,
    ) -> PyResult<()> {
        let attr = if name.starts_with("primvars:") {
            name.to_owned()
        } else {
            format!("primvars:{name}")
        };
        let declared = self.with(py, |s, p| s.declared_type(p, &attr))?;
        let value = from_py(py, value, declared.as_deref().unwrap_or(type_name))?;
        let indices = indices.map(|i| from_py(py, i, "int[]")).transpose()?;
        let interpolation = self::interpolation(interpolation)?;
        self.with(py, |s, p| {
            s.set_primvar(
                p,
                &attr,
                type_name,
                &value,
                interpolation,
                indices.as_ref().map(|i| i.as_slice().unwrap_or(&[])),
                element_size,
                time,
            )
        })
    }

    fn remove_api(&self, py: Python<'_>, schema: &str) -> PyResult<()> {
        self.with(py, |s, p| s.remove_api(p, schema))
    }

    fn add_target(&self, py: Python<'_>, name: &str, target: &str) -> PyResult<()> {
        self.with(py, |s, p| s.add_target(p, name, target))
    }

    fn rename(&self, py: Python<'_>, new_name: &str) -> PyResult<()> {
        self.with(py, |s, p| s.rename_prim(p, new_name))
    }

    fn reparent(&self, py: Python<'_>, new_parent: PyRef<'_, Prim>) -> PyResult<()> {
        if !new_parent.stage.bind(py).is(self.stage.bind(py)) {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "prim belongs to a different stage",
            ));
        }
        let parent = new_parent.handle;
        self.with(py, |s, p| s.reparent_prim(p, parent))
    }

    fn edit_variant(&self, py: Python<'_>, variant_set: &str, variant: &str) -> PyResult<()> {
        self.with(py, |s, p| s.edit_variant(p, variant_set, variant))
    }

    #[getter]
    fn variant_sets<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let sets = self.with(py, |s, p| s.variant_sets(p))?;
        let out = PyDict::new(py);
        for (name, variants) in sets {
            out.set_item(name, variants)?;
        }
        Ok(out)
    }

    #[pyo3(signature = (material, purpose = ""))]
    fn bind_material(&self, py: Python<'_>, material: PyRef<'_, Prim>, purpose: &str) -> PyResult<()> {
        if !material.stage.bind(py).is(self.stage.bind(py)) {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "prim belongs to a different stage",
            ));
        }
        let target = material.handle;
        self.with(py, |s, p| s.bind_material(p, target, purpose))
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
        (core::Feature::Write, "write"),
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
    m.add("InvalidValueError", py.get_type::<InvalidValueError>())?;
    m.add("WriteError", py.get_type::<WriteError>())?;
    m.add_class::<Stage>()?;
    m.add_class::<EditBatch>()?;
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

mod physics_input {
    use pyo3::exceptions::PyValueError;
    use pyo3::prelude::*;
    use pyo3::types::PyDict;
    use tinyusd_core as core;

    use super::{Prim, Stage, own};

    struct Reader<'a, 'py> {
        stage: &'a Bound<'py, Stage>,
    }

    impl<'py> Reader<'_, 'py> {
        fn get<T: for<'a> FromPyObject<'a, 'py>>(&self, d: &Bound<'py, PyDict>, key: &str) -> PyResult<Option<T>> {
            match d.get_item(key)? {
                Some(v) if !v.is_none() => match v.extract::<T>() {
                    Ok(x) => Ok(Some(x)),
                    Err(e) => {
                        let e: PyErr = e.into();
                        Err(PyValueError::new_err(format!("{key}: {e}")))
                    }
                },
                _ => Ok(None),
            }
        }

        fn set<T: for<'a> FromPyObject<'a, 'py>>(
            &self,
            d: &Bound<'py, PyDict>,
            key: &str,
            out: &mut T,
        ) -> PyResult<()> {
            if let Some(v) = self.get(d, key)? {
                *out = v;
            }
            Ok(())
        }

        fn prim(&self, obj: &Bound<'py, PyAny>) -> PyResult<core::Prim> {
            let prim: PyRef<'_, Prim> = obj.extract()?;
            own(self.stage, &prim)
        }

        fn required_prim(&self, d: &Bound<'py, PyDict>) -> PyResult<core::Prim> {
            let obj = d
                .get_item("prim")?
                .ok_or_else(|| PyValueError::new_err("every physics item needs its 'prim'"))?;
            self.prim(&obj)
        }

        fn opt_prim(&self, d: &Bound<'py, PyDict>, key: &str, out: &mut Option<core::Prim>) -> PyResult<()> {
            match d.get_item(key)? {
                Some(v) if v.is_none() => *out = None,
                Some(v) => *out = Some(self.prim(&v)?),
                None => {}
            }
            Ok(())
        }

        fn prims(&self, d: &Bound<'py, PyDict>, key: &str, out: &mut Vec<core::Prim>) -> PyResult<()> {
            if let Some(list) = d.get_item(key)? {
                *out = list.try_iter()?.map(|p| self.prim(&p?)).collect::<PyResult<_>>()?;
            }
            Ok(())
        }

        fn named<T: Copy + std::fmt::Debug>(
            &self,
            d: &Bound<'py, PyDict>,
            key: &str,
            all: &[T],
            out: &mut T,
        ) -> PyResult<()> {
            if let Some(name) = self.get::<String>(d, key)? {
                *out = *all
                    .iter()
                    .find(|v| format!("{v:?}") == name)
                    .ok_or_else(|| PyValueError::new_err(format!("{key}: unknown value {name:?}")))?;
            }
            Ok(())
        }

        fn dicts(&self, d: &Bound<'py, PyDict>, key: &str) -> PyResult<Vec<Bound<'py, PyDict>>> {
            match d.get_item(key)? {
                None => Ok(Vec::new()),
                Some(list) => list
                    .try_iter()?
                    .map(|item| {
                        item?
                            .cast_into::<PyDict>()
                            .map_err(|_| PyValueError::new_err(format!("{key} must hold dicts")))
                    })
                    .collect(),
            }
        }
    }

    const AXES: [core::Axis; 3] = [core::Axis::X, core::Axis::Y, core::Axis::Z];
    const DOFS: [core::Dof; 7] = [
        core::Dof::Distance,
        core::Dof::TransX,
        core::Dof::TransY,
        core::Dof::TransZ,
        core::Dof::RotX,
        core::Dof::RotY,
        core::Dof::RotZ,
    ];

    pub(crate) fn scene(stage: &Bound<'_, Stage>, d: &Bound<'_, PyDict>) -> PyResult<core::PhysicsScene> {
        let r = Reader { stage };
        let mut x = core::PhysicsScene::new(r.required_prim(d)?);
        r.set(d, "gravity_direction", &mut x.gravity_direction)?;
        r.set(d, "gravity_magnitude", &mut x.gravity_magnitude)?;
        Ok(x)
    }

    pub(crate) fn group(stage: &Bound<'_, Stage>, d: &Bound<'_, PyDict>) -> PyResult<core::CollisionGroup> {
        let r = Reader { stage };
        let mut x = core::CollisionGroup::new(r.required_prim(d)?);
        r.set(d, "invert_filtered_groups", &mut x.invert_filtered_groups)?;
        r.prims(d, "filtered_groups", &mut x.filtered_groups)?;
        r.set(d, "merge_group_name", &mut x.merge_group_name)?;
        Ok(x)
    }

    pub(crate) fn material(stage: &Bound<'_, Stage>, d: &Bound<'_, PyDict>) -> PyResult<core::PhysicsMaterial> {
        let r = Reader { stage };
        let mut x = core::PhysicsMaterial::new(r.required_prim(d)?);
        r.set(d, "static_friction", &mut x.static_friction)?;
        r.set(d, "dynamic_friction", &mut x.dynamic_friction)?;
        r.set(d, "restitution", &mut x.restitution)?;
        r.set(d, "density", &mut x.density)?;
        Ok(x)
    }

    pub(crate) fn shape(stage: &Bound<'_, Stage>, d: &Bound<'_, PyDict>) -> PyResult<core::Shape> {
        use core::ShapeKind as K;
        let r = Reader { stage };
        let mut x = core::Shape::new(r.required_prim(d)?);
        r.prims(d, "materials", &mut x.materials)?;
        r.prims(d, "simulation_owners", &mut x.simulation_owners)?;
        r.prims(d, "filtered_collisions", &mut x.filtered_collisions)?;
        r.prims(d, "collision_groups", &mut x.collision_groups)?;
        r.set(d, "collision_enabled", &mut x.collision_enabled)?;
        let kinds = [
            K::Sphere,
            K::Cube,
            K::Capsule,
            K::Capsule1,
            K::Cylinder,
            K::Cylinder1,
            K::Cone,
            K::Plane,
            K::Mesh,
            K::SpherePoints,
            K::Custom,
        ];
        r.named(d, "kind", &kinds, &mut x.kind)?;
        r.set(d, "mesh_approximation", &mut x.mesh_approximation)?;
        Ok(x)
    }

    pub(crate) fn body(stage: &Bound<'_, Stage>, d: &Bound<'_, PyDict>) -> PyResult<core::RigidBody> {
        let r = Reader { stage };
        let mut x = core::RigidBody::new(r.required_prim(d)?);
        r.prims(d, "filtered_collisions", &mut x.filtered_collisions)?;
        r.prims(d, "simulation_owners", &mut x.simulation_owners)?;
        r.set(d, "position", &mut x.position)?;
        r.set(d, "rotation", &mut x.rotation)?;
        r.set(d, "scale", &mut x.scale)?;
        r.set(d, "enabled", &mut x.enabled)?;
        r.set(d, "kinematic", &mut x.kinematic)?;
        r.set(d, "starts_asleep", &mut x.starts_asleep)?;
        r.set(d, "linear_velocity", &mut x.linear_velocity)?;
        r.set(d, "angular_velocity", &mut x.angular_velocity)?;
        Ok(x)
    }

    pub(crate) fn joint(stage: &Bound<'_, Stage>, d: &Bound<'_, PyDict>) -> PyResult<core::Joint> {
        use core::JointKind as K;
        let r = Reader { stage };
        let mut kind = K::Fixed;
        let kinds = [
            K::Fixed,
            K::Revolute,
            K::Prismatic,
            K::Spherical,
            K::Distance,
            K::D6,
            K::Custom,
        ];
        r.named(d, "kind", &kinds, &mut kind)?;
        let mut x = core::Joint::new(r.required_prim(d)?, kind);
        r.set(d, "type_name", &mut x.type_name)?;
        r.opt_prim(d, "body0", &mut x.body0)?;
        r.opt_prim(d, "body1", &mut x.body1)?;
        r.set(d, "local_pos0", &mut x.local_pos0)?;
        r.set(d, "local_rot0", &mut x.local_rot0)?;
        r.set(d, "local_pos1", &mut x.local_pos1)?;
        r.set(d, "local_rot1", &mut x.local_rot1)?;
        r.set(d, "enabled", &mut x.enabled)?;
        r.set(d, "collision_enabled", &mut x.collision_enabled)?;
        r.set(d, "break_force", &mut x.break_force)?;
        r.set(d, "break_torque", &mut x.break_torque)?;
        r.set(d, "exclude_from_articulation", &mut x.exclude_from_articulation)?;
        r.named(d, "axis", &AXES, &mut x.axis)?;
        r.set(d, "min_distance_enabled", &mut x.min_distance_enabled)?;
        r.set(d, "max_distance_enabled", &mut x.max_distance_enabled)?;
        for l in r.dicts(d, "limits")? {
            let mut limit = core::JointLimit {
                dof: core::Dof::Distance,
                enabled: true,
                lower: f32::NEG_INFINITY,
                upper: f32::INFINITY,
            };
            r.named(&l, "dof", &DOFS, &mut limit.dof)?;
            r.set(&l, "enabled", &mut limit.enabled)?;
            r.set(&l, "lower", &mut limit.lower)?;
            r.set(&l, "upper", &mut limit.upper)?;
            x.limits.push(limit);
        }
        for v in r.dicts(d, "drives")? {
            let mut drive = core::JointDrive {
                dof: core::Dof::Distance,
                target_position: 0.0,
                target_velocity: 0.0,
                force_limit: f32::INFINITY,
                stiffness: 0.0,
                damping: 0.0,
                acceleration: false,
            };
            r.named(&v, "dof", &DOFS, &mut drive.dof)?;
            r.set(&v, "target_position", &mut drive.target_position)?;
            r.set(&v, "target_velocity", &mut drive.target_velocity)?;
            r.set(&v, "force_limit", &mut drive.force_limit)?;
            r.set(&v, "stiffness", &mut drive.stiffness)?;
            r.set(&v, "damping", &mut drive.damping)?;
            r.set(&v, "acceleration", &mut drive.acceleration)?;
            x.drives.push(drive);
        }
        Ok(x)
    }

    pub(crate) fn articulation(stage: &Bound<'_, Stage>, d: &Bound<'_, PyDict>) -> PyResult<core::Articulation> {
        let r = Reader { stage };
        let mut x = core::Articulation::new(r.required_prim(d)?);
        r.prims(d, "filtered_collisions", &mut x.filtered_collisions)?;
        Ok(x)
    }

    pub(crate) fn physics(stage: &Bound<'_, Stage>, d: &Bound<'_, PyDict>) -> PyResult<core::Physics> {
        let r = Reader { stage };
        let mut p = core::Physics {
            meters_per_unit: 1.0,
            kilograms_per_unit: 1.0,
            ..core::Physics::default()
        };
        r.set(d, "meters_per_unit", &mut p.meters_per_unit)?;
        r.set(d, "kilograms_per_unit", &mut p.kilograms_per_unit)?;
        r.set(d, "si_units", &mut p.si_units)?;
        p.scenes = r
            .dicts(d, "scenes")?
            .iter()
            .map(|x| scene(stage, x))
            .collect::<PyResult<_>>()?;
        p.collision_groups = r
            .dicts(d, "collision_groups")?
            .iter()
            .map(|x| group(stage, x))
            .collect::<PyResult<_>>()?;
        p.materials = r
            .dicts(d, "materials")?
            .iter()
            .map(|x| material(stage, x))
            .collect::<PyResult<_>>()?;
        p.shapes = r
            .dicts(d, "shapes")?
            .iter()
            .map(|x| shape(stage, x))
            .collect::<PyResult<_>>()?;
        p.rigid_bodies = r
            .dicts(d, "rigid_bodies")?
            .iter()
            .map(|x| body(stage, x))
            .collect::<PyResult<_>>()?;
        p.joints = r
            .dicts(d, "joints")?
            .iter()
            .map(|x| joint(stage, x))
            .collect::<PyResult<_>>()?;
        p.articulations = r
            .dicts(d, "articulations")?
            .iter()
            .map(|x| articulation(stage, x))
            .collect::<PyResult<_>>()?;
        Ok(p)
    }

    pub(crate) fn body_mass(d: &Bound<'_, PyDict>, stage: &Bound<'_, Stage>) -> PyResult<core::BodyMass> {
        let r = Reader { stage };
        let mut m = core::BodyMass {
            principal_axes: [1.0, 0.0, 0.0, 0.0],
            ..core::BodyMass::default()
        };
        r.set(d, "mass", &mut m.mass)?;
        r.set(d, "diagonal_inertia", &mut m.diagonal_inertia)?;
        r.set(d, "center_of_mass", &mut m.center_of_mass)?;
        r.set(d, "principal_axes", &mut m.principal_axes)?;
        Ok(m)
    }
}
