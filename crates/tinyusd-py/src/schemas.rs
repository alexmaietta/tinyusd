use std::path::PathBuf;

use pyo3::prelude::*;
use pyo3::types::PyDict;
use tinyusd_core::{self as core, BaseType};

use super::{py_err, to_py};

fn value(py: Python<'_>, v: Option<&core::Value>) -> PyResult<Py<PyAny>> {
    match v.filter(|v| v.base_type() != BaseType::Unsupported) {
        Some(v) => Ok(to_py(py, v)?.map_or_else(|| py.None(), Bound::unbind)),
        None => Ok(py.None()),
    }
}

fn dictionary(py: Python<'_>, d: &core::Dictionary, path: &str) -> PyResult<Py<PyDict>> {
    let out = PyDict::new(py);
    for key in d.keys(path) {
        let full = if path.is_empty() {
            key.clone()
        } else {
            format!("{path}:{key}")
        };
        if d.is_dictionary(&full) {
            out.set_item(&key, dictionary(py, d, &full)?)?;
        } else {
            let mut v = core::Value::new();
            d.get(&full, &mut v);
            out.set_item(&key, value(py, Some(&v))?)?;
        }
    }
    Ok(out.unbind())
}

#[pyclass(frozen, get_all, module = "tinyusd._tinyusd")]
pub(crate) struct SchemaProperty {
    name: String,
    declared_by: String,
    relationship: bool,
    type_name: String,
    uniform: bool,
    fallback: Py<PyAny>,
    allowed_tokens: Vec<String>,
    display_group: Option<String>,
    display_name: Option<String>,
    documentation: Option<String>,
    custom_data: Py<PyDict>,
}

#[pymethods]
impl SchemaProperty {
    fn __repr__(&self) -> String {
        format!(
            "SchemaProperty(name={:?}, type_name={:?}, declared_by={:?})",
            self.name, self.type_name, self.declared_by
        )
    }
}

#[pyclass(frozen, get_all, module = "tinyusd._tinyusd")]
pub(crate) struct SchemaClass {
    name: String,
    cpp_name: String,
    kind: &'static str,
    bases: Vec<String>,
    can_only_apply_to: Vec<String>,
    auto_apply_to: Vec<String>,
    property_namespace_prefix: Option<String>,
    allowed_instance_names: Vec<String>,
    api_schemas: Vec<String>,
    documentation: Option<String>,
    custom_data: Py<PyDict>,
    properties: Vec<Py<SchemaProperty>>,
}

#[pymethods]
impl SchemaClass {
    fn find_property(&self, py: Python<'_>, name: &str) -> Option<Py<SchemaProperty>> {
        self.properties
            .iter()
            .find(|p| p.get().name == name)
            .map(|p| p.clone_ref(py))
    }

    fn __repr__(&self) -> String {
        format!("SchemaClass(name={:?}, kind={:?})", self.name, self.kind)
    }
}

#[pyclass(frozen, get_all, module = "tinyusd._tinyusd")]
pub(crate) struct LibraryToken {
    name: String,
    value: String,
    documentation: Option<String>,
}

#[pyclass(frozen, module = "tinyusd._tinyusd")]
pub(crate) struct SchemaLibrary {
    inner: core::SchemaLibrary,
    #[pyo3(get)]
    global_data: Py<PyDict>,
    #[pyo3(get)]
    tokens: Vec<Py<LibraryToken>>,
    #[pyo3(get)]
    classes: Vec<Py<SchemaClass>>,
}

fn property(py: Python<'_>, p: &core::SchemaProperty) -> PyResult<SchemaProperty> {
    Ok(SchemaProperty {
        name: p.name.clone(),
        declared_by: p.declared_by.clone(),
        relationship: p.relationship,
        type_name: p.type_name.clone(),
        uniform: p.uniform,
        fallback: value(py, p.fallback.as_ref())?,
        allowed_tokens: p.allowed_tokens.clone(),
        display_group: p.display_group.clone(),
        display_name: p.display_name.clone(),
        documentation: p.documentation.clone(),
        custom_data: dictionary(py, &p.custom_data, "")?,
    })
}

fn class(py: Python<'_>, c: &core::SchemaClass) -> PyResult<SchemaClass> {
    Ok(SchemaClass {
        name: c.name.clone(),
        cpp_name: c.cpp_name.clone(),
        kind: c.kind.token(),
        bases: c.bases.clone(),
        can_only_apply_to: c.can_only_apply_to.clone(),
        auto_apply_to: c.auto_apply_to.clone(),
        property_namespace_prefix: c.property_namespace_prefix.clone(),
        allowed_instance_names: c.allowed_instance_names.clone(),
        api_schemas: c.api_schemas.clone(),
        documentation: c.documentation.clone(),
        custom_data: dictionary(py, &c.custom_data, "")?,
        properties: c
            .properties
            .iter()
            .map(|p| Py::new(py, property(py, p)?))
            .collect::<PyResult<_>>()?,
    })
}

#[pymethods]
impl SchemaLibrary {
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn prefix(&self) -> &str {
        &self.inner.prefix
    }

    #[getter]
    fn skip_code_generation(&self) -> bool {
        self.inner.skip_code_generation
    }

    #[getter]
    fn layers(&self) -> Vec<String> {
        self.inner.layers.clone()
    }

    #[getter]
    fn generated_schema(&self) -> &str {
        self.inner.generated_schema()
    }

    #[getter]
    fn plug_info(&self) -> &str {
        self.inner.plug_info()
    }

    fn find_class(&self, py: Python<'_>, name: &str) -> Option<Py<SchemaClass>> {
        self.classes
            .iter()
            .find(|c| c.get().name == name)
            .map(|c| c.clone_ref(py))
    }

    fn write_plugin(&self, dir: PathBuf) -> PyResult<()> {
        self.inner.write_plugin(dir).map_err(py_err)
    }

    fn __repr__(&self) -> String {
        format!(
            "SchemaLibrary(name={:?}, classes={})",
            self.inner.name,
            self.inner.classes.len()
        )
    }
}

#[pyfunction]
#[pyo3(signature = (path, search_paths = Vec::new()))]
pub(crate) fn read_schema_library(
    py: Python<'_>,
    path: PathBuf,
    search_paths: Vec<PathBuf>,
) -> PyResult<SchemaLibrary> {
    library(py, core::SchemaLibrary::read(path, &search_paths).map_err(py_err)?)
}

#[pyfunction]
pub(crate) fn read_schema_plugin(py: Python<'_>, path: PathBuf) -> PyResult<SchemaLibrary> {
    library(py, core::SchemaLibrary::read_plugin(path).map_err(py_err)?)
}

fn library(py: Python<'_>, inner: core::SchemaLibrary) -> PyResult<SchemaLibrary> {
    let classes = inner
        .classes
        .iter()
        .map(|c| Py::new(py, class(py, c)?))
        .collect::<PyResult<_>>()?;
    let tokens = inner
        .tokens
        .iter()
        .map(|t| {
            Py::new(
                py,
                LibraryToken {
                    name: t.name.clone(),
                    value: t.value.clone(),
                    documentation: t.documentation.clone(),
                },
            )
        })
        .collect::<PyResult<_>>()?;
    Ok(SchemaLibrary {
        global_data: dictionary(py, &inner.global_data, "")?,
        tokens,
        classes,
        inner,
    })
}

#[pyclass(frozen, get_all, module = "tinyusd._tinyusd")]
pub(crate) struct ShaderPort {
    name: String,
    type_name: String,
    fallback: Py<PyAny>,
    allowed_tokens: Vec<String>,
    interface_only: bool,
    documentation: Option<String>,
    sdr_metadata: Py<PyDict>,
}

#[pymethods]
impl ShaderPort {
    fn __repr__(&self) -> String {
        format!("ShaderPort(name={:?}, type_name={:?})", self.name, self.type_name)
    }
}

#[pyclass(frozen, get_all, module = "tinyusd._tinyusd")]
pub(crate) struct ShaderNode {
    name: String,
    id: String,
    documentation: Option<String>,
    sdr_metadata: Py<PyDict>,
    inputs: Vec<Py<ShaderPort>>,
    outputs: Vec<Py<ShaderPort>>,
}

#[pymethods]
impl ShaderNode {
    fn __repr__(&self) -> String {
        format!("ShaderNode(id={:?})", self.id)
    }
}

fn ports(py: Python<'_>, ports: &[core::ShaderPort]) -> PyResult<Vec<Py<ShaderPort>>> {
    ports
        .iter()
        .map(|p| {
            Py::new(
                py,
                ShaderPort {
                    name: p.name.clone(),
                    type_name: p.type_name.clone(),
                    fallback: value(py, p.fallback.as_ref())?,
                    allowed_tokens: p.allowed_tokens.clone(),
                    interface_only: p.interface_only,
                    documentation: p.documentation.clone(),
                    sdr_metadata: dictionary(py, &p.sdr_metadata, "")?,
                },
            )
        })
        .collect()
}

#[pyfunction]
#[pyo3(signature = (path, search_paths = Vec::new()))]
pub(crate) fn read_shader_defs(
    py: Python<'_>,
    path: PathBuf,
    search_paths: Vec<PathBuf>,
) -> PyResult<Vec<Py<ShaderNode>>> {
    core::read_shader_defs(path, &search_paths)
        .map_err(py_err)?
        .iter()
        .map(|n| {
            Py::new(
                py,
                ShaderNode {
                    name: n.name.clone(),
                    id: n.id.clone(),
                    documentation: n.documentation.clone(),
                    sdr_metadata: dictionary(py, &n.sdr_metadata, "")?,
                    inputs: ports(py, &n.inputs)?,
                    outputs: ports(py, &n.outputs)?,
                },
            )
        })
        .collect()
}
