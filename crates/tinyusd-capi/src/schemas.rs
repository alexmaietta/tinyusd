#![cfg_attr(not(feature = "schemas"), allow(unused_variables, unused_imports, dead_code))]

use std::ffi::{CString, c_char};
use std::path::PathBuf;
use std::ptr;

use super::write::strs;
use super::{
    Fail, TusdStatus, TusdStringList, TusdValue, core, guard, invalid, out_mut, set_error, str_arg, to_c_string,
};

#[cfg(feature = "schemas")]
macro_rules! schemas_api {
    ($body:block) => {
        guard(|| $body)
    };
}

#[cfg(not(feature = "schemas"))]
macro_rules! schemas_api {
    ($body:block) => {
        guard(|| Err(Fail::from(core::Error::FeatureDisabled("schemas"))))
    };
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TusdSchemaKind {
    AbstractBase = 0,
    AbstractTyped = 1,
    ConcreteTyped = 2,
    NonAppliedApi = 3,
    SingleApplyApi = 4,
    MultipleApplyApi = 5,
}

#[derive(Default)]
pub struct TusdDictionary {
    #[cfg(feature = "schemas")]
    dictionary: core::Dictionary,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdSchemaLibraryView {
    pub name: *const c_char,
    pub prefix: *const c_char,
    pub skip_code_generation: bool,
    pub global_data: *const TusdDictionary,
    pub class_count: usize,
    pub token_count: usize,
    pub layers: *const *const c_char,
    pub layer_count: usize,
    pub generated_schema: *const c_char,
    pub plug_info: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdSchemaClassView {
    pub name: *const c_char,
    pub cpp_name: *const c_char,
    pub kind: TusdSchemaKind,
    pub bases: *const *const c_char,
    pub base_count: usize,
    pub can_only_apply_to: *const *const c_char,
    pub can_only_apply_to_count: usize,
    pub auto_apply_to: *const *const c_char,
    pub auto_apply_to_count: usize,
    pub property_namespace_prefix: *const c_char,
    pub allowed_instance_names: *const *const c_char,
    pub allowed_instance_name_count: usize,
    pub api_schemas: *const *const c_char,
    pub api_schema_count: usize,
    pub documentation: *const c_char,
    pub custom_data: *const TusdDictionary,
    pub property_count: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdSchemaPropertyView {
    pub name: *const c_char,
    pub declared_by: *const c_char,
    pub relationship: bool,
    pub type_name: *const c_char,
    pub uniform: bool,
    pub fallback: *const TusdValue,
    pub allowed_tokens: *const *const c_char,
    pub allowed_token_count: usize,
    pub display_group: *const c_char,
    pub display_name: *const c_char,
    pub documentation: *const c_char,
    pub custom_data: *const TusdDictionary,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdLibraryTokenView {
    pub name: *const c_char,
    pub value: *const c_char,
    pub documentation: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdShaderNodeView {
    pub name: *const c_char,
    pub id: *const c_char,
    pub documentation: *const c_char,
    pub sdr_metadata: *const TusdDictionary,
    pub input_count: usize,
    pub output_count: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdShaderPortView {
    pub name: *const c_char,
    pub type_name: *const c_char,
    pub fallback: *const TusdValue,
    pub allowed_tokens: *const *const c_char,
    pub allowed_token_count: usize,
    pub interface_only: bool,
    pub documentation: *const c_char,
    pub sdr_metadata: *const TusdDictionary,
}

#[derive(Default)]
#[allow(clippy::vec_box)]
struct Arena {
    strings: Vec<CString>,
    lists: Vec<Vec<*const c_char>>,
    dictionaries: Vec<Box<TusdDictionary>>,
    values: Vec<Box<TusdValue>>,
}

#[cfg(feature = "schemas")]
impl Arena {
    fn text(&mut self, s: &str) -> *const c_char {
        self.strings.push(to_c_string(s));
        self.strings.last().map_or(ptr::null(), |c| c.as_ptr())
    }

    fn opt(&mut self, s: Option<&str>) -> *const c_char {
        s.map_or(ptr::null(), |s| self.text(s))
    }

    fn list(&mut self, items: &[String]) -> (*const *const c_char, usize) {
        let pointers: Vec<*const c_char> = items.iter().map(|s| self.text(s)).collect();
        self.lists.push(pointers);
        self.lists.last().map_or((ptr::null(), 0), |l| (l.as_ptr(), l.len()))
    }

    fn dictionary(&mut self, dictionary: &core::Dictionary) -> *const TusdDictionary {
        self.dictionaries.push(Box::new(TusdDictionary {
            dictionary: dictionary.clone(),
        }));
        self.dictionaries.last().map_or(ptr::null(), |d| &raw const **d)
    }

    fn value(&mut self, value: Option<&core::Value>) -> *const TusdValue {
        let Some(value) = value else {
            return ptr::null();
        };
        let mut boxed = Box::<TusdValue>::default();
        boxed.value = value.clone();
        boxed.sync();
        self.values.push(boxed);
        self.values.last().map_or(ptr::null(), |v| &raw const **v)
    }
}

pub struct TusdSchemaLibrary {
    _arena: Arena,
    #[cfg(feature = "schemas")]
    library: core::SchemaLibrary,
    view: TusdSchemaLibraryView,
    classes: Vec<TusdSchemaClassView>,
    properties: Vec<Vec<TusdSchemaPropertyView>>,
    tokens: Vec<TusdLibraryTokenView>,
}

pub struct TusdShaderDefs {
    _arena: Arena,
    nodes: Vec<TusdShaderNodeView>,
    inputs: Vec<Vec<TusdShaderPortView>>,
    outputs: Vec<Vec<TusdShaderPortView>>,
}

#[cfg(feature = "schemas")]
fn schema_kind(kind: core::SchemaKind) -> TusdSchemaKind {
    match kind {
        core::SchemaKind::AbstractBase => TusdSchemaKind::AbstractBase,
        core::SchemaKind::AbstractTyped => TusdSchemaKind::AbstractTyped,
        core::SchemaKind::ConcreteTyped => TusdSchemaKind::ConcreteTyped,
        core::SchemaKind::NonAppliedApi => TusdSchemaKind::NonAppliedApi,
        core::SchemaKind::SingleApplyApi => TusdSchemaKind::SingleApplyApi,
        core::SchemaKind::MultipleApplyApi => TusdSchemaKind::MultipleApplyApi,
    }
}

#[cfg(feature = "schemas")]
fn property_view(arena: &mut Arena, p: &core::SchemaProperty) -> TusdSchemaPropertyView {
    let (allowed_tokens, allowed_token_count) = arena.list(&p.allowed_tokens);
    TusdSchemaPropertyView {
        name: arena.text(&p.name),
        declared_by: arena.text(&p.declared_by),
        relationship: p.relationship,
        type_name: arena.text(&p.type_name),
        uniform: p.uniform,
        fallback: arena.value(p.fallback.as_ref()),
        allowed_tokens,
        allowed_token_count,
        display_group: arena.opt(p.display_group.as_deref()),
        display_name: arena.opt(p.display_name.as_deref()),
        documentation: arena.opt(p.documentation.as_deref()),
        custom_data: arena.dictionary(&p.custom_data),
    }
}

#[cfg(feature = "schemas")]
fn class_view(arena: &mut Arena, c: &core::SchemaClass) -> TusdSchemaClassView {
    let (bases, base_count) = arena.list(&c.bases);
    let (can_only_apply_to, can_only_apply_to_count) = arena.list(&c.can_only_apply_to);
    let (auto_apply_to, auto_apply_to_count) = arena.list(&c.auto_apply_to);
    let (allowed_instance_names, allowed_instance_name_count) = arena.list(&c.allowed_instance_names);
    let (api_schemas, api_schema_count) = arena.list(&c.api_schemas);
    TusdSchemaClassView {
        name: arena.text(&c.name),
        cpp_name: arena.text(&c.cpp_name),
        kind: schema_kind(c.kind),
        bases,
        base_count,
        can_only_apply_to,
        can_only_apply_to_count,
        auto_apply_to,
        auto_apply_to_count,
        property_namespace_prefix: arena.opt(c.property_namespace_prefix.as_deref()),
        allowed_instance_names,
        allowed_instance_name_count,
        api_schemas,
        api_schema_count,
        documentation: arena.opt(c.documentation.as_deref()),
        custom_data: arena.dictionary(&c.custom_data),
        property_count: c.properties.len(),
    }
}

#[cfg(feature = "schemas")]
fn wrap_library(library: core::SchemaLibrary) -> TusdSchemaLibrary {
    let mut arena = Arena::default();
    let classes = library.classes.iter().map(|c| class_view(&mut arena, c)).collect();
    let properties = library
        .classes
        .iter()
        .map(|c| c.properties.iter().map(|p| property_view(&mut arena, p)).collect())
        .collect();
    let tokens = library
        .tokens
        .iter()
        .map(|t| TusdLibraryTokenView {
            name: arena.text(&t.name),
            value: arena.text(&t.value),
            documentation: arena.opt(t.documentation.as_deref()),
        })
        .collect();
    let (layers, layer_count) = arena.list(&library.layers);
    let view = TusdSchemaLibraryView {
        name: arena.text(&library.name),
        prefix: arena.text(&library.prefix),
        skip_code_generation: library.skip_code_generation,
        global_data: arena.dictionary(&library.global_data),
        class_count: library.classes.len(),
        token_count: library.tokens.len(),
        layers,
        layer_count,
        generated_schema: arena.text(library.generated_schema()),
        plug_info: arena.text(library.plug_info()),
    };
    TusdSchemaLibrary {
        _arena: arena,
        library,
        view,
        classes,
        properties,
        tokens,
    }
}

#[cfg(feature = "schemas")]
fn port_view(arena: &mut Arena, p: &core::ShaderPort) -> TusdShaderPortView {
    let (allowed_tokens, allowed_token_count) = arena.list(&p.allowed_tokens);
    TusdShaderPortView {
        name: arena.text(&p.name),
        type_name: arena.text(&p.type_name),
        fallback: arena.value(p.fallback.as_ref()),
        allowed_tokens,
        allowed_token_count,
        interface_only: p.interface_only,
        documentation: arena.opt(p.documentation.as_deref()),
        sdr_metadata: arena.dictionary(&p.sdr_metadata),
    }
}

#[cfg(feature = "schemas")]
fn wrap_shader_defs(nodes: &[core::ShaderNode]) -> TusdShaderDefs {
    let mut arena = Arena::default();
    let views = nodes
        .iter()
        .map(|n| TusdShaderNodeView {
            name: arena.text(&n.name),
            id: arena.text(&n.id),
            documentation: arena.opt(n.documentation.as_deref()),
            sdr_metadata: arena.dictionary(&n.sdr_metadata),
            input_count: n.inputs.len(),
            output_count: n.outputs.len(),
        })
        .collect();
    let inputs = nodes
        .iter()
        .map(|n| n.inputs.iter().map(|p| port_view(&mut arena, p)).collect())
        .collect();
    let outputs = nodes
        .iter()
        .map(|n| n.outputs.iter().map(|p| port_view(&mut arena, p)).collect())
        .collect();
    TusdShaderDefs {
        _arena: arena,
        nodes: views,
        inputs,
        outputs,
    }
}

unsafe fn search_paths(items: *const *const c_char, count: usize) -> Result<Vec<PathBuf>, Fail> {
    Ok(unsafe { strs(items, count, "search_paths") }?
        .into_iter()
        .map(PathBuf::from)
        .collect())
}

fn item<T: Copy>(items: &[T], index: usize) -> Result<T, Fail> {
    items.get(index).copied().ok_or_else(|| {
        set_error(format!("index {index} out of range"));
        Fail(TusdStatus::InvalidArgument)
    })
}

unsafe fn library_ref<'a>(library: *const TusdSchemaLibrary) -> Result<&'a TusdSchemaLibrary, Fail> {
    unsafe { library.as_ref() }.ok_or_else(|| invalid("library must not be null"))
}

unsafe fn defs_ref<'a>(defs: *const TusdShaderDefs) -> Result<&'a TusdShaderDefs, Fail> {
    unsafe { defs.as_ref() }.ok_or_else(|| invalid("defs must not be null"))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_schema_library_read(
    schema_path: *const c_char,
    search_paths: *const *const c_char,
    search_path_count: usize,
    out_library: *mut *mut TusdSchemaLibrary,
) -> TusdStatus {
    schemas_api!({
        let out = unsafe { out_mut(out_library, "out_library") }?;
        *out = ptr::null_mut();
        let path = unsafe { str_arg(schema_path, "schema_path") }?;
        let paths = unsafe { self::search_paths(search_paths, search_path_count) }?;
        let library = core::SchemaLibrary::read(path, &paths)?;
        *out = Box::into_raw(Box::new(wrap_library(library)));
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_schema_library_free(library: *mut TusdSchemaLibrary) {
    if !library.is_null() {
        drop(unsafe { Box::from_raw(library) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_schema_library_info(
    library: *const TusdSchemaLibrary,
    out: *mut TusdSchemaLibraryView,
) -> TusdStatus {
    schemas_api!({
        let out = unsafe { out_mut(out, "out") }?;
        let l = unsafe { library_ref(library) }?;
        *out = l.view;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_schema_library_class(
    library: *const TusdSchemaLibrary,
    index: usize,
    out: *mut TusdSchemaClassView,
) -> TusdStatus {
    schemas_api!({
        let out = unsafe { out_mut(out, "out") }?;
        *out = item(&unsafe { library_ref(library) }?.classes, index)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_schema_library_find_class(
    library: *const TusdSchemaLibrary,
    name: *const c_char,
    out_found: *mut bool,
    out_index: *mut usize,
) -> TusdStatus {
    schemas_api!({
        let found = unsafe { out_mut(out_found, "out_found") }?;
        let index = unsafe { out_mut(out_index, "out_index") }?;
        let name = unsafe { str_arg(name, "name") }?;
        let l = unsafe { library_ref(library) }?;
        let position = l.library.classes.iter().position(|c| c.name == name);
        *found = position.is_some();
        if let Some(p) = position {
            *index = p;
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_schema_library_property(
    library: *const TusdSchemaLibrary,
    class_index: usize,
    property_index: usize,
    out: *mut TusdSchemaPropertyView,
) -> TusdStatus {
    schemas_api!({
        let out = unsafe { out_mut(out, "out") }?;
        let l = unsafe { library_ref(library) }?;
        let properties = l.properties.get(class_index).ok_or_else(|| {
            set_error(format!("class index {class_index} out of range"));
            Fail(TusdStatus::InvalidArgument)
        })?;
        *out = item(properties, property_index)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_schema_library_token(
    library: *const TusdSchemaLibrary,
    index: usize,
    out: *mut TusdLibraryTokenView,
) -> TusdStatus {
    schemas_api!({
        let out = unsafe { out_mut(out, "out") }?;
        *out = item(&unsafe { library_ref(library) }?.tokens, index)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_schema_library_write_plugin(
    library: *const TusdSchemaLibrary,
    dir: *const c_char,
) -> TusdStatus {
    schemas_api!({
        let dir = unsafe { str_arg(dir, "dir") }?;
        unsafe { library_ref(library) }?.library.write_plugin(dir)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_dictionary_get(
    dictionary: *const TusdDictionary,
    key: *const c_char,
    out_value: *mut TusdValue,
    out_found: *mut bool,
) -> TusdStatus {
    schemas_api!({
        let found = unsafe { out_mut(out_found, "out_found") }?;
        let v = unsafe { out_mut(out_value, "out_value") }?;
        let d = unsafe { dictionary.as_ref() }.ok_or_else(|| invalid("dictionary must not be null"))?;
        *found = d.dictionary.get(unsafe { str_arg(key, "key") }?, &mut v.value);
        v.sync();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_dictionary_keys(
    dictionary: *const TusdDictionary,
    key: *const c_char,
    out: *mut TusdStringList,
) -> TusdStatus {
    schemas_api!({
        let out = unsafe { out_mut(out, "out") }?;
        let d = unsafe { dictionary.as_ref() }.ok_or_else(|| invalid("dictionary must not be null"))?;
        out.set(d.dictionary.keys(unsafe { str_arg(key, "key") }?));
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_shader_defs_read(
    path: *const c_char,
    search_paths: *const *const c_char,
    search_path_count: usize,
    out_defs: *mut *mut TusdShaderDefs,
) -> TusdStatus {
    schemas_api!({
        let out = unsafe { out_mut(out_defs, "out_defs") }?;
        *out = ptr::null_mut();
        let path = unsafe { str_arg(path, "path") }?;
        let paths = unsafe { self::search_paths(search_paths, search_path_count) }?;
        let nodes = core::read_shader_defs(path, &paths)?;
        *out = Box::into_raw(Box::new(wrap_shader_defs(&nodes)));
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_shader_defs_free(defs: *mut TusdShaderDefs) {
    if !defs.is_null() {
        drop(unsafe { Box::from_raw(defs) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_shader_defs_node_count(defs: *const TusdShaderDefs) -> usize {
    unsafe { defs.as_ref() }.map_or(0, |d| d.nodes.len())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_shader_defs_node(
    defs: *const TusdShaderDefs,
    index: usize,
    out: *mut TusdShaderNodeView,
) -> TusdStatus {
    schemas_api!({
        let out = unsafe { out_mut(out, "out") }?;
        *out = item(&unsafe { defs_ref(defs) }?.nodes, index)?;
        Ok(())
    })
}

fn port(ports: &[Vec<TusdShaderPortView>], node: usize, index: usize) -> Result<TusdShaderPortView, Fail> {
    let ports = ports.get(node).ok_or_else(|| {
        set_error(format!("node index {node} out of range"));
        Fail(TusdStatus::InvalidArgument)
    })?;
    item(ports, index)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_shader_defs_input(
    defs: *const TusdShaderDefs,
    node_index: usize,
    input_index: usize,
    out: *mut TusdShaderPortView,
) -> TusdStatus {
    schemas_api!({
        let out = unsafe { out_mut(out, "out") }?;
        *out = port(&unsafe { defs_ref(defs) }?.inputs, node_index, input_index)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_shader_defs_output(
    defs: *const TusdShaderDefs,
    node_index: usize,
    output_index: usize,
    out: *mut TusdShaderPortView,
) -> TusdStatus {
    schemas_api!({
        let out = unsafe { out_mut(out, "out") }?;
        *out = port(&unsafe { defs_ref(defs) }?.outputs, node_index, output_index)?;
        Ok(())
    })
}
