#![allow(unsafe_code, clippy::missing_safety_doc)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use tinyusd_core as core;

pub const TUSD_ABI_VERSION: u32 = 7;

pub type TusdPrim = u32;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TusdStatus {
    Ok = 0,
    InvalidArgument = 1,
    OpenFailed = 2,
    InvalidPath = 3,
    InvalidPrim = 4,
    AssetFailed = 5,
    UsdError = 6,
    Panic = 7,
    FeatureDisabled = 8,
    SchemaFailed = 9,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TusdFeature {
    Geom = 1 << 0,
    Shade = 1 << 1,
    Lux = 1 << 2,
    Skel = 1 << 3,
    Physics = 1 << 4,
    Vol = 1 << 5,
    Media = 1 << 6,
    Semantics = 1 << 7,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TusdTraverseFlags {
    Default = 0,
    IncludeInactive = 1 << 0,
    IncludeUnloaded = 1 << 1,
    IncludeUndefined = 1 << 2,
    IncludeAbstract = 1 << 3,
    InstanceProxies = 1 << 4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TusdPrimFlags {
    Active = 1 << 0,
    Loaded = 1 << 1,
    Defined = 1 << 2,
    Abstract = 1 << 3,
    Model = 1 << 4,
    Instanceable = 1 << 5,
    Instance = 1 << 6,
    InstanceProxy = 1 << 7,
    Prototype = 1 << 8,
    InPrototype = 1 << 9,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TusdSpecifier {
    Def = 0,
    Over = 1,
    Class = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TusdValueType {
    Empty = 0,
    Bool = 1,
    Uchar = 2,
    Int = 3,
    Uint = 4,
    Int64 = 5,
    Uint64 = 6,
    Half = 7,
    Float = 8,
    Double = 9,
    TimeCode = 10,
    String = 11,
    Token = 12,
    Asset = 13,
    Path = 14,
    Int2 = 15,
    Int3 = 16,
    Int4 = 17,
    Half2 = 18,
    Half3 = 19,
    Half4 = 20,
    Float2 = 21,
    Float3 = 22,
    Float4 = 23,
    Double2 = 24,
    Double3 = 25,
    Double4 = 26,
    Quath = 27,
    Quatf = 28,
    Quatd = 29,
    Matrix2d = 30,
    Matrix3d = 31,
    Matrix4d = 32,
    Unsupported = 33,
}

fn value_type(base: core::BaseType) -> TusdValueType {
    use core::BaseType as B;
    match base {
        B::Empty => TusdValueType::Empty,
        B::Bool => TusdValueType::Bool,
        B::UChar => TusdValueType::Uchar,
        B::Int => TusdValueType::Int,
        B::UInt => TusdValueType::Uint,
        B::Int64 => TusdValueType::Int64,
        B::UInt64 => TusdValueType::Uint64,
        B::Half => TusdValueType::Half,
        B::Float => TusdValueType::Float,
        B::Double => TusdValueType::Double,
        B::TimeCode => TusdValueType::TimeCode,
        B::String => TusdValueType::String,
        B::Token => TusdValueType::Token,
        B::Asset => TusdValueType::Asset,
        B::Path => TusdValueType::Path,
        B::Int2 => TusdValueType::Int2,
        B::Int3 => TusdValueType::Int3,
        B::Int4 => TusdValueType::Int4,
        B::Half2 => TusdValueType::Half2,
        B::Half3 => TusdValueType::Half3,
        B::Half4 => TusdValueType::Half4,
        B::Float2 => TusdValueType::Float2,
        B::Float3 => TusdValueType::Float3,
        B::Float4 => TusdValueType::Float4,
        B::Double2 => TusdValueType::Double2,
        B::Double3 => TusdValueType::Double3,
        B::Double4 => TusdValueType::Double4,
        B::Quath => TusdValueType::Quath,
        B::Quatf => TusdValueType::Quatf,
        B::Quatd => TusdValueType::Quatd,
        B::Matrix2d => TusdValueType::Matrix2d,
        B::Matrix3d => TusdValueType::Matrix3d,
        B::Matrix4d => TusdValueType::Matrix4d,
        B::Unsupported => TusdValueType::Unsupported,
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TusdInterpolation {
    Constant = 0,
    Uniform = 1,
    Varying = 2,
    Vertex = 3,
    FaceVarying = 4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdPrimvarInfo {
    pub name: *const c_char,
    pub type_name: *const c_char,
    pub interpolation: TusdInterpolation,
    pub element_size: i32,
    pub indexed: bool,
    pub source: TusdPrim,
}

#[derive(Default)]
pub struct TusdOutput {
    data: Option<Vec<u8>>,
    error: Option<String>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdResolverCallbacks {
    pub struct_size: u32,
    pub user_data: *mut c_void,
    pub resolve: Option<
        unsafe extern "C" fn(
            user_data: *mut c_void,
            asset_path: *const c_char,
            anchor: *const c_char,
            out: *mut TusdOutput,
        ) -> bool,
    >,
    pub read: Option<
        unsafe extern "C" fn(user_data: *mut c_void, resolved_path: *const c_char, out: *mut TusdOutput) -> bool,
    >,
    pub release: Option<unsafe extern "C" fn(user_data: *mut c_void)>,
}

struct CallbackResolver {
    callbacks: TusdResolverCallbacks,
}

impl Drop for CallbackResolver {
    fn drop(&mut self) {
        if let Some(release) = self.callbacks.release {
            unsafe { release(self.callbacks.user_data) };
        }
    }
}

impl core::AssetResolver for CallbackResolver {
    fn resolve(&self, asset_path: &str, anchor: Option<&str>) -> Option<String> {
        let resolve = self.callbacks.resolve?;
        let asset_path = CString::new(asset_path).ok()?;
        let anchor = anchor.and_then(|a| CString::new(a).ok());
        let mut out = TusdOutput::default();
        let anchor_ptr = anchor.as_ref().map_or(ptr::null(), |a| a.as_ptr());
        if !unsafe { resolve(self.callbacks.user_data, asset_path.as_ptr(), anchor_ptr, &mut out) } {
            return None;
        }
        String::from_utf8(out.data?).ok()
    }

    fn read(&self, resolved_path: &str) -> std::io::Result<Vec<u8>> {
        let read = self
            .callbacks
            .read
            .ok_or_else(|| std::io::Error::other("no read callback"))?;
        let path = CString::new(resolved_path).map_err(std::io::Error::other)?;
        let mut out = TusdOutput::default();
        if unsafe { read(self.callbacks.user_data, path.as_ptr(), &mut out) } {
            Ok(out.data.unwrap_or_default())
        } else {
            let message = out
                .error
                .unwrap_or_else(|| format!("host could not read '{resolved_path}'"));
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, message))
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdOpenOptions {
    pub struct_size: u32,
    pub load_payloads: bool,
    pub resolver: *const TusdResolverCallbacks,
    pub schema_paths: *const *const c_char,
    pub schema_path_count: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdTimeRange {
    pub start_time_code: f64,
    pub end_time_code: f64,
    pub time_codes_per_second: f64,
    pub frames_per_second: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdAttributeInfo {
    pub type_name: *const c_char,
    pub uniform: bool,
    pub custom: bool,
    pub has_value: bool,
    pub has_authored_value: bool,
    pub might_be_time_varying: bool,
}

pub struct TusdStage {
    stage: core::Stage,
    paths: Vec<Option<CString>>,
    interned: HashMap<String, CString>,
}

impl TusdStage {
    fn intern(&mut self, value: &str) -> *const c_char {
        if let Some(s) = self.interned.get(value) {
            return s.as_ptr();
        }
        let c = to_c_string(value);
        let p = c.as_ptr();
        self.interned.insert(value.to_owned(), c);
        p
    }

    fn path_ptr(&mut self, prim: TusdPrim) -> core::Result<*const c_char> {
        let index = prim as usize;
        if self.paths.len() <= index {
            self.paths.resize(index + 1, None);
        }
        if self.paths[index].is_none() {
            self.paths[index] = Some(to_c_string(self.stage.path(core::Prim::from_index(prim))?));
        }
        Ok(self.paths[index].as_ref().map_or(ptr::null(), |c| c.as_ptr()))
    }
}

#[derive(Default)]
pub struct TusdValue {
    value: core::Value,
    strings: Vec<CString>,
    resolved: Vec<CString>,
    type_name: CString,
}

impl TusdValue {
    fn sync(&mut self) {
        self.strings = self.value.strings().iter().map(|s| to_c_string(s)).collect();
        self.resolved = self.value.resolved_paths().iter().map(|s| to_c_string(s)).collect();
        self.type_name = to_c_string(self.value.type_name());
    }

    fn set_bytes(&mut self, bytes: Vec<u8>) {
        self.value.set_uchar_array(&bytes);
        self.sync();
    }

    fn set_doubles(&mut self, values: &[f64]) {
        self.value.set_double_array(values);
        self.sync();
    }
}

#[derive(Default)]
pub struct TusdPrimList {
    prims: Vec<TusdPrim>,
}

#[derive(Default)]
pub struct TusdStringList {
    strings: Vec<CString>,
}

impl TusdStringList {
    fn set(&mut self, items: impl IntoIterator<Item = impl AsRef<str>>) {
        self.strings.clear();
        self.strings.extend(items.into_iter().map(|s| to_c_string(s.as_ref())));
    }
}

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

fn to_c_string(value: &str) -> CString {
    CString::new(value.replace('\0', " ")).unwrap_or_default()
}

fn set_error(message: impl Into<String>) {
    let message = to_c_string(&message.into());
    LAST_ERROR.with(|e| *e.borrow_mut() = message);
}

fn clear_error() {
    LAST_ERROR.with(|e| *e.borrow_mut() = CString::default());
}

fn status_of(error: &core::Error) -> TusdStatus {
    match error {
        core::Error::Open { .. } => TusdStatus::OpenFailed,
        core::Error::InvalidPath(_) => TusdStatus::InvalidPath,
        core::Error::InvalidPrim => TusdStatus::InvalidPrim,
        core::Error::Asset { .. } => TusdStatus::AssetFailed,
        core::Error::Usd(_) => TusdStatus::UsdError,
        core::Error::FeatureDisabled(_) => TusdStatus::FeatureDisabled,
        core::Error::Schema { .. } => TusdStatus::SchemaFailed,
    }
}

struct Fail(TusdStatus);

impl From<core::Error> for Fail {
    fn from(e: core::Error) -> Self {
        set_error(e.to_string());
        Fail(status_of(&e))
    }
}

fn invalid(message: &str) -> Fail {
    set_error(message);
    Fail(TusdStatus::InvalidArgument)
}

fn guard(f: impl FnOnce() -> Result<(), Fail>) -> TusdStatus {
    clear_error();
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => TusdStatus::Ok,
        Ok(Err(Fail(status))) => status,
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_owned());
            set_error(format!("internal error: {message}"));
            TusdStatus::Panic
        }
    }
}

unsafe fn str_arg<'a>(s: *const c_char, what: &str) -> Result<&'a str, Fail> {
    if s.is_null() {
        return Err(invalid(&format!("{what} must not be null")));
    }
    unsafe { CStr::from_ptr(s) }
        .to_str()
        .map_err(|_| invalid(&format!("{what} is not valid UTF-8")))
}

unsafe fn stage_mut<'a>(stage: *mut TusdStage) -> Result<&'a mut TusdStage, Fail> {
    unsafe { stage.as_mut() }.ok_or_else(|| invalid("stage must not be null"))
}

unsafe fn out_mut<'a, T>(out: *mut T, what: &str) -> Result<&'a mut T, Fail> {
    unsafe { out.as_mut() }.ok_or_else(|| invalid(&format!("{what} must not be null")))
}

unsafe fn time_arg(time: *const f64) -> Option<f64> {
    unsafe { time.as_ref() }.copied()
}

unsafe fn opt_outs<'a, T>(found: *mut bool, out: *mut T) -> Result<(&'a mut bool, &'a mut T), Fail> {
    Ok((unsafe { out_mut(found, "out_found") }?, unsafe { out_mut(out, "out") }?))
}

fn write_opt<T>((found, out): (&mut bool, &mut T), value: Option<T>) -> Result<(), Fail> {
    *found = value.is_some();
    if let Some(v) = value {
        *out = v;
    }
    Ok(())
}

fn flags(bits: u32) -> core::TraverseFlags {
    core::TraverseFlags::from_bits_truncate(bits)
}

fn prim(p: TusdPrim) -> core::Prim {
    core::Prim::from_index(p)
}

fn handle(p: core::Prim) -> TusdPrim {
    p.index()
}

#[unsafe(no_mangle)]
pub extern "C" fn tusd_abi_version() -> u32 {
    TUSD_ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn tusd_version_string() -> *const c_char {
    static VERSION: &CStr = match CStr::from_bytes_with_nul(concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes()) {
        Ok(v) => v,
        Err(_) => c"",
    };
    VERSION.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn tusd_features() -> u32 {
    core::features()
}

#[unsafe(no_mangle)]
pub extern "C" fn tusd_has_feature(feature: u32) -> bool {
    feature != 0 && core::features() & feature == feature
}

#[unsafe(no_mangle)]
pub extern "C" fn tusd_last_error() -> *const c_char {
    LAST_ERROR.with(|e| e.borrow().as_ptr())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_open_options_init(options: *mut TusdOpenOptions) {
    if let Some(o) = unsafe { options.as_mut() } {
        *o = TusdOpenOptions {
            struct_size: size_of::<TusdOpenOptions>() as u32,
            load_payloads: true,
            resolver: ptr::null(),
            schema_paths: ptr::null(),
            schema_path_count: 0,
        };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_open(
    path: *const c_char,
    options: *const TusdOpenOptions,
    out_stage: *mut *mut TusdStage,
) -> TusdStatus {
    guard(|| {
        let out = unsafe { out_mut(out_stage, "out_stage") }?;
        *out = ptr::null_mut();
        let path = unsafe { str_arg(path, "path") }?;
        let mut opts = core::OpenOptions::default();
        if let Some(o) = unsafe { options.as_ref() } {
            let size = o.struct_size as usize;
            if size < std::mem::offset_of!(TusdOpenOptions, resolver) {
                return Err(invalid("options.struct_size is too small; call tusd_open_options_init"));
            }
            opts.load_payloads = o.load_payloads;
            let fits = |end: usize| size >= end;
            if fits(std::mem::offset_of!(TusdOpenOptions, schema_paths))
                && let Some(callbacks) = unsafe { o.resolver.as_ref() }
            {
                let resolver = CallbackResolver { callbacks: *callbacks };
                if (callbacks.struct_size as usize) < size_of::<TusdResolverCallbacks>() {
                    return Err(invalid("resolver.struct_size is too small"));
                }
                if callbacks.resolve.is_none() || callbacks.read.is_none() {
                    return Err(invalid("resolver.resolve and resolver.read must be set"));
                }
                opts.resolver = Some(std::rc::Rc::new(resolver));
            }
            if fits(size_of::<TusdOpenOptions>()) && o.schema_path_count > 0 {
                if o.schema_paths.is_null() {
                    return Err(invalid("schema_paths must not be null when schema_path_count is set"));
                }
                let paths = unsafe { std::slice::from_raw_parts(o.schema_paths, o.schema_path_count) };
                for &p in paths {
                    opts.schema_paths
                        .push(std::path::PathBuf::from(unsafe { str_arg(p, "schema path") }?));
                }
            }
        }
        let stage = core::Stage::open(path, &opts)?;
        *out = Box::into_raw(Box::new(TusdStage {
            stage,
            paths: Vec::new(),
            interned: HashMap::new(),
        }));
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_output_set_bytes(out: *mut TusdOutput, data: *const c_void, size: usize) {
    if let Some(out) = unsafe { out.as_mut() } {
        out.data = Some(if data.is_null() || size == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size) }.to_vec()
        });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_output_set_string(out: *mut TusdOutput, value: *const c_char) {
    if let Some(out) = unsafe { out.as_mut() } {
        out.data = (!value.is_null()).then(|| unsafe { CStr::from_ptr(value) }.to_bytes().to_vec());
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_output_set_error(out: *mut TusdOutput, message: *const c_char) {
    if let Some(out) = unsafe { out.as_mut() } {
        out.error = (!message.is_null()).then(|| unsafe { CStr::from_ptr(message) }.to_string_lossy().into_owned());
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_load(stage: *mut TusdStage, p: TusdPrim, with_descendants: bool) -> TusdStatus {
    guard(|| {
        unsafe { stage_mut(stage) }?.stage.load(prim(p), with_descendants)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_unload(stage: *mut TusdStage, p: TusdPrim) -> TusdStatus {
    guard(|| {
        unsafe { stage_mut(stage) }?.stage.unload(prim(p))?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_loadable_prims(
    stage: *mut TusdStage,
    root: TusdPrim,
    out: *mut TusdPrimList,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let out = unsafe { out_mut(out, "out") }?;
        out.prims = s.stage.loadable(prim(root))?.into_iter().map(handle).collect();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_free(stage: *mut TusdStage) {
    if !stage.is_null() {
        drop(unsafe { Box::from_raw(stage) });
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tusd_stage_pseudo_root() -> TusdPrim {
    handle(core::Prim::PSEUDO_ROOT)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_prim_at_path(
    stage: *mut TusdStage,
    path: *const c_char,
    out_prim: *mut TusdPrim,
    out_found: *mut bool,
) -> TusdStatus {
    guard(|| {
        let outs = unsafe { opt_outs(out_found, out_prim) }?;
        let s = unsafe { stage_mut(stage) }?;
        let path = unsafe { str_arg(path, "path") }?;
        let found = s.stage.prim_at_path(path)?.map(handle);
        write_opt(outs, found)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_default_prim(
    stage: *mut TusdStage,
    out_prim: *mut TusdPrim,
    out_found: *mut bool,
) -> TusdStatus {
    guard(|| {
        let outs = unsafe { opt_outs(out_found, out_prim) }?;
        let s = unsafe { stage_mut(stage) }?;
        let found = s.stage.default_prim()?.map(handle);
        write_opt(outs, found)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_traverse(
    stage: *mut TusdStage,
    root: TusdPrim,
    flags_bits: u32,
    out: *mut TusdPrimList,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let out = unsafe { out_mut(out, "out") }?;
        let prims = s.stage.traverse_from(prim(root), flags(flags_bits))?;
        out.prims = prims.into_iter().map(handle).collect();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_prototypes(stage: *mut TusdStage, out: *mut TusdPrimList) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let out = unsafe { out_mut(out, "out") }?;
        out.prims = s.stage.prototypes()?.into_iter().map(handle).collect();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_metadata(
    stage: *mut TusdStage,
    key: *const c_char,
    out_value: *mut TusdValue,
    out_found: *mut bool,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let key = unsafe { str_arg(key, "key") }?;
        let v = unsafe { out_mut(out_value, "out_value") }?;
        let found = unsafe { out_mut(out_found, "out_found") }?;
        *found = s.stage.stage_metadata(key, &mut v.value)?;
        v.sync();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_has_authored_metadata(
    stage: *mut TusdStage,
    key: *const c_char,
    out: *mut bool,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let key = unsafe { str_arg(key, "key") }?;
        let out = unsafe { out_mut(out, "out") }?;
        *out = s.stage.has_authored_stage_metadata(key)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_time_range(stage: *mut TusdStage, out: *mut TusdTimeRange) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let out = unsafe { out_mut(out, "out") }?;
        *out = TusdTimeRange {
            start_time_code: s.stage.start_time_code(),
            end_time_code: s.stage.end_time_code(),
            time_codes_per_second: s.stage.time_codes_per_second(),
            frames_per_second: s.stage.frames_per_second(),
        };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_layer_identifiers(stage: *mut TusdStage, out: *mut TusdStringList) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        unsafe { out_mut(out, "out") }?.set(s.stage.layer_identifiers());
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_composition_errors(stage: *mut TusdStage, out: *mut TusdStringList) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        unsafe { out_mut(out, "out") }?.set(s.stage.composition_errors());
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_read_asset(
    stage: *mut TusdStage,
    resolved_path: *const c_char,
    out_bytes: *mut TusdValue,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let path = unsafe { str_arg(resolved_path, "resolved_path") }?;
        let out = unsafe { out_mut(out_bytes, "out_bytes") }?;
        out.set_bytes(s.stage.read_asset(path)?);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_path(stage: *mut TusdStage, p: TusdPrim) -> *const c_char {
    let mut out = ptr::null();
    let status = guard(|| {
        out = unsafe { stage_mut(stage) }?.path_ptr(p)?;
        Ok(())
    });
    if status == TusdStatus::Ok { out } else { ptr::null() }
}

fn interned_string(stage: *mut TusdStage, f: impl FnOnce(&mut core::Stage) -> core::Result<String>) -> *const c_char {
    let mut out = ptr::null();
    let status = guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let value = f(&mut s.stage)?;
        out = s.intern(&value);
        Ok(())
    });
    if status == TusdStatus::Ok { out } else { ptr::null() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_name(stage: *mut TusdStage, p: TusdPrim) -> *const c_char {
    interned_string(stage, |s| s.name(prim(p)).map(str::to_owned))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_type_name(stage: *mut TusdStage, p: TusdPrim) -> *const c_char {
    interned_string(stage, |s| s.type_name(prim(p)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_kind(stage: *mut TusdStage, p: TusdPrim) -> *const c_char {
    interned_string(stage, |s| s.kind(prim(p)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_specifier(
    stage: *mut TusdStage,
    p: TusdPrim,
    out: *mut TusdSpecifier,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let out = unsafe { out_mut(out, "out") }?;
        *out = match s.stage.specifier(prim(p))? {
            core::Specifier::Def => TusdSpecifier::Def,
            core::Specifier::Over => TusdSpecifier::Over,
            core::Specifier::Class => TusdSpecifier::Class,
        };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_flags(stage: *mut TusdStage, p: TusdPrim, out: *mut u32) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let out = unsafe { out_mut(out, "out") }?;
        *out = s.stage.flags(prim(p))?.bits();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_parent(
    stage: *mut TusdStage,
    p: TusdPrim,
    out_prim: *mut TusdPrim,
    out_found: *mut bool,
) -> TusdStatus {
    guard(|| {
        let outs = unsafe { opt_outs(out_found, out_prim) }?;
        let s = unsafe { stage_mut(stage) }?;
        let parent = s.stage.parent(prim(p))?.map(handle);
        write_opt(outs, parent)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_children(
    stage: *mut TusdStage,
    p: TusdPrim,
    flags_bits: u32,
    out: *mut TusdPrimList,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let out = unsafe { out_mut(out, "out") }?;
        out.prims = s
            .stage
            .children(prim(p), flags(flags_bits))?
            .into_iter()
            .map(handle)
            .collect();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_prototype(
    stage: *mut TusdStage,
    p: TusdPrim,
    out_prim: *mut TusdPrim,
    out_found: *mut bool,
) -> TusdStatus {
    guard(|| {
        let outs = unsafe { opt_outs(out_found, out_prim) }?;
        let s = unsafe { stage_mut(stage) }?;
        let proto = s.stage.prototype_of(prim(p))?.map(handle);
        write_opt(outs, proto)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_instances(stage: *mut TusdStage, p: TusdPrim, out: *mut TusdPrimList) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let out = unsafe { out_mut(out, "out") }?;
        out.prims = s.stage.instances_of(prim(p))?.into_iter().map(handle).collect();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_is_a(
    stage: *mut TusdStage,
    p: TusdPrim,
    schema: *const c_char,
    out: *mut bool,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let schema = unsafe { str_arg(schema, "schema") }?;
        let out = unsafe { out_mut(out, "out") }?;
        *out = s.stage.is_a(prim(p), schema)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_has_api(
    stage: *mut TusdStage,
    p: TusdPrim,
    schema: *const c_char,
    out: *mut bool,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let schema = unsafe { str_arg(schema, "schema") }?;
        let out = unsafe { out_mut(out, "out") }?;
        *out = s.stage.has_api(prim(p), schema)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_applied_schemas(
    stage: *mut TusdStage,
    p: TusdPrim,
    out: *mut TusdStringList,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        unsafe { out_mut(out, "out") }?.set(s.stage.applied_schemas(prim(p))?);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_attribute_names(
    stage: *mut TusdStage,
    p: TusdPrim,
    authored_only: bool,
    out: *mut TusdStringList,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        unsafe { out_mut(out, "out") }?.set(s.stage.attribute_names(prim(p), authored_only)?);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_relationship_names(
    stage: *mut TusdStage,
    p: TusdPrim,
    authored_only: bool,
    out: *mut TusdStringList,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        unsafe { out_mut(out, "out") }?.set(s.stage.relationship_names(prim(p), authored_only)?);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_metadata(
    stage: *mut TusdStage,
    p: TusdPrim,
    key: *const c_char,
    out_value: *mut TusdValue,
    out_found: *mut bool,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let key = unsafe { str_arg(key, "key") }?;
        let v = unsafe { out_mut(out_value, "out_value") }?;
        let found = unsafe { out_mut(out_found, "out_found") }?;
        *found = s.stage.prim_metadata(prim(p), key, &mut v.value)?;
        v.sync();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_attribute_get(
    stage: *mut TusdStage,
    p: TusdPrim,
    name: *const c_char,
    time: *const f64,
    out_value: *mut TusdValue,
    out_has_value: *mut bool,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let name = unsafe { str_arg(name, "name") }?;
        let v = unsafe { out_mut(out_value, "out_value") }?;
        let has = unsafe { out_mut(out_has_value, "out_has_value") }?;
        *has = s.stage.get(prim(p), name, unsafe { time_arg(time) }, &mut v.value)?;
        v.sync();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_attribute_info(
    stage: *mut TusdStage,
    p: TusdPrim,
    name: *const c_char,
    out_info: *mut TusdAttributeInfo,
    out_found: *mut bool,
) -> TusdStatus {
    guard(|| {
        let outs = unsafe { opt_outs(out_found, out_info) }?;
        let s = unsafe { stage_mut(stage) }?;
        let name = unsafe { str_arg(name, "name") }?;
        let info = s.stage.attribute_info(prim(p), name)?;
        let info = info.map(|i| TusdAttributeInfo {
            type_name: s.intern(&i.type_name),
            uniform: i.uniform,
            custom: i.custom,
            has_value: i.has_value,
            has_authored_value: i.has_authored_value,
            might_be_time_varying: i.might_be_time_varying,
        });
        write_opt(outs, info)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_attribute_time_samples(
    stage: *mut TusdStage,
    p: TusdPrim,
    name: *const c_char,
    out_times: *mut TusdValue,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let name = unsafe { str_arg(name, "name") }?;
        let out = unsafe { out_mut(out_times, "out_times") }?;
        out.set_doubles(&s.stage.time_samples(prim(p), name)?);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_attribute_connections(
    stage: *mut TusdStage,
    p: TusdPrim,
    name: *const c_char,
    out: *mut TusdStringList,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let name = unsafe { str_arg(name, "name") }?;
        unsafe { out_mut(out, "out") }?.set(s.stage.connections(prim(p), name)?);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_attribute_metadata(
    stage: *mut TusdStage,
    p: TusdPrim,
    name: *const c_char,
    key: *const c_char,
    out_value: *mut TusdValue,
    out_found: *mut bool,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let name = unsafe { str_arg(name, "name") }?;
        let key = unsafe { str_arg(key, "key") }?;
        let v = unsafe { out_mut(out_value, "out_value") }?;
        let found = unsafe { out_mut(out_found, "out_found") }?;
        *found = s.stage.attribute_metadata(prim(p), name, key, &mut v.value)?;
        v.sync();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_relationship_targets(
    stage: *mut TusdStage,
    p: TusdPrim,
    name: *const c_char,
    forwarded: bool,
    out: *mut TusdStringList,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let name = unsafe { str_arg(name, "name") }?;
        unsafe { out_mut(out, "out") }?.set(s.stage.targets(prim(p), name, forwarded)?);
        Ok(())
    })
}

unsafe fn matrix_out<'a>(out: *mut f64) -> Result<&'a mut [f64], Fail> {
    if out.is_null() {
        return Err(invalid("out_matrix must not be null"));
    }
    Ok(unsafe { std::slice::from_raw_parts_mut(out, 16) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_world_transform(
    stage: *mut TusdStage,
    p: TusdPrim,
    time: *const f64,
    out_matrix: *mut f64,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let out = unsafe { matrix_out(out_matrix) }?;
        out.copy_from_slice(&s.stage.world_transform(prim(p), unsafe { time_arg(time) })?);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_local_transform(
    stage: *mut TusdStage,
    p: TusdPrim,
    time: *const f64,
    out_matrix: *mut f64,
    out_resets_xform_stack: *mut bool,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        let out = unsafe { matrix_out(out_matrix) }?;
        let resets = unsafe { out_mut(out_resets_xform_stack, "out_resets_xform_stack") }?;
        let (m, r) = s.stage.local_transform(prim(p), unsafe { time_arg(time) })?;
        out.copy_from_slice(&m);
        *resets = r;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_primvar_names(
    stage: *mut TusdStage,
    p: TusdPrim,
    out: *mut TusdStringList,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { stage_mut(stage) }?;
        unsafe { out_mut(out, "out") }?.set(s.stage.primvar_names(prim(p))?);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_primvar(
    stage: *mut TusdStage,
    p: TusdPrim,
    name: *const c_char,
    time: *const f64,
    out_value: *mut TusdValue,
    out_info: *mut TusdPrimvarInfo,
    out_found: *mut bool,
) -> TusdStatus {
    guard(|| {
        let outs = unsafe { opt_outs(out_found, out_info) }?;
        let s = unsafe { stage_mut(stage) }?;
        let name = unsafe { str_arg(name, "name") }?;
        let v = unsafe { out_mut(out_value, "out_value") }?;
        let info = s
            .stage
            .primvar(prim(p), name, unsafe { time_arg(time) }, &mut v.value)?;
        v.sync();
        let info = info.map(|i| TusdPrimvarInfo {
            name: s.intern(&i.name),
            type_name: s.intern(&i.type_name),
            interpolation: match i.interpolation {
                core::Interpolation::Constant => TusdInterpolation::Constant,
                core::Interpolation::Uniform => TusdInterpolation::Uniform,
                core::Interpolation::Varying => TusdInterpolation::Varying,
                core::Interpolation::Vertex => TusdInterpolation::Vertex,
                core::Interpolation::FaceVarying => TusdInterpolation::FaceVarying,
            },
            element_size: i.element_size,
            indexed: i.indexed,
            source: handle(i.source),
        });
        write_opt(outs, info)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_bound_material(
    stage: *mut TusdStage,
    p: TusdPrim,
    purpose: *const c_char,
    out_material: *mut TusdPrim,
    out_binding: *mut *const c_char,
    out_found: *mut bool,
) -> TusdStatus {
    guard(|| {
        let outs = unsafe { opt_outs(out_found, out_material) }?;
        let s = unsafe { stage_mut(stage) }?;
        let purpose = if purpose.is_null() {
            ""
        } else {
            unsafe { str_arg(purpose, "purpose") }?
        };
        let bound = s.stage.bound_material(prim(p), purpose)?;
        if let Some(binding_out) = unsafe { out_binding.as_mut() } {
            *binding_out = match &bound {
                Some(b) => s.intern(&b.binding),
                None => ptr::null(),
            };
        }
        write_opt(outs, bound.map(|b| handle(b.material)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn tusd_value_new() -> *mut TusdValue {
    Box::into_raw(Box::default())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_value_free(value: *mut TusdValue) {
    if !value.is_null() {
        drop(unsafe { Box::from_raw(value) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_value_type(value: *const TusdValue) -> TusdValueType {
    unsafe { value.as_ref() }.map_or(TusdValueType::Empty, |v| value_type(v.value.base_type()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_value_is_array(value: *const TusdValue) -> bool {
    unsafe { value.as_ref() }.is_some_and(|v| v.value.is_array())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_value_len(value: *const TusdValue) -> usize {
    unsafe { value.as_ref() }.map_or(0, |v| v.value.len())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_value_data(value: *const TusdValue) -> *const c_void {
    unsafe { value.as_ref() }.map_or(ptr::null(), |v| {
        let bytes = v.value.bytes();
        if bytes.is_empty() {
            ptr::null()
        } else {
            bytes.as_ptr().cast()
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_value_data_size(value: *const TusdValue) -> usize {
    unsafe { value.as_ref() }.map_or(0, |v| v.value.bytes().len())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_value_type_name(value: *const TusdValue) -> *const c_char {
    unsafe { value.as_ref() }.map_or(ptr::null(), |v| v.type_name.as_ptr())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_value_string(value: *const TusdValue, index: usize) -> *const c_char {
    unsafe { value.as_ref() }
        .and_then(|v| v.strings.get(index))
        .map_or(ptr::null(), |s| s.as_ptr())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_value_resolved_path(value: *const TusdValue, index: usize) -> *const c_char {
    unsafe { value.as_ref() }
        .and_then(|v| v.resolved.get(index))
        .map_or(ptr::null(), |s| s.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn tusd_prim_list_new() -> *mut TusdPrimList {
    Box::into_raw(Box::default())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_list_free(list: *mut TusdPrimList) {
    if !list.is_null() {
        drop(unsafe { Box::from_raw(list) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_list_len(list: *const TusdPrimList) -> usize {
    unsafe { list.as_ref() }.map_or(0, |l| l.prims.len())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_list_data(list: *const TusdPrimList) -> *const TusdPrim {
    unsafe { list.as_ref() }.map_or(ptr::null(), |l| l.prims.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn tusd_string_list_new() -> *mut TusdStringList {
    Box::into_raw(Box::default())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_string_list_free(list: *mut TusdStringList) {
    if !list.is_null() {
        drop(unsafe { Box::from_raw(list) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_string_list_len(list: *const TusdStringList) -> usize {
    unsafe { list.as_ref() }.map_or(0, |l| l.strings.len())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_string_list_get(list: *const TusdStringList, index: usize) -> *const c_char {
    unsafe { list.as_ref() }
        .and_then(|l| l.strings.get(index))
        .map_or(ptr::null(), |s| s.as_ptr())
}

#[derive(Default)]
pub struct TusdInstances {
    instances: core::Instances,
    prototypes: Vec<TusdPrim>,
}

#[unsafe(no_mangle)]
pub extern "C" fn tusd_instances_new() -> *mut TusdInstances {
    Box::into_raw(Box::default())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_instances_free(instances: *mut TusdInstances) {
    if !instances.is_null() {
        drop(unsafe { Box::from_raw(instances) });
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdInstancesView {
    pub prototypes: *const TusdPrim,
    pub prototype_count: usize,
    pub proto_indices: *const i32,
    pub ids: *const i64,
    pub transforms: *const f64,
    pub count: usize,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_instancer_instances(
    stage: *mut TusdStage,
    p: TusdPrim,
    time: *const f64,
    include_prototype_transform: bool,
    apply_visibility: bool,
    out: *mut TusdInstances,
    out_found: *mut bool,
) -> TusdStatus {
    guard(|| {
        let out = unsafe { out_mut(out, "out") }?;
        let found = unsafe { out_mut(out_found, "out_found") }?;
        let s = unsafe { stage_mut(stage) }?;
        let result = s.stage.instancer_instances(
            prim(p),
            unsafe { time_arg(time) },
            include_prototype_transform,
            apply_visibility,
        )?;
        *found = result.is_some();
        out.instances = result.unwrap_or_default();
        out.prototypes = out.instances.prototypes.iter().map(|&p| handle(p)).collect();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_instances_view(
    instances: *const TusdInstances,
    out: *mut TusdInstancesView,
) -> TusdStatus {
    guard(|| {
        let i = unsafe { instances.as_ref() }.ok_or_else(|| invalid("instances must not be null"))?;
        let out = unsafe { out_mut(out, "out") }?;
        *out = TusdInstancesView {
            prototypes: i.prototypes.as_ptr(),
            prototype_count: i.prototypes.len(),
            proto_indices: i.instances.proto_indices.as_ptr(),
            ids: i.instances.ids.as_ptr(),
            transforms: i.instances.transforms.as_ptr().cast(),
            count: i.instances.transforms.len(),
        };
        Ok(())
    })
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TusdSkinningMethod {
    ClassicLinear = 0,
    DualQuaternion = 1,
}

#[derive(Default)]
pub struct TusdSkelBindings {
    bindings: Vec<(TusdPrim, Option<TusdPrim>, Vec<TusdPrim>)>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdSkelBindingView {
    pub skeleton: TusdPrim,
    pub has_animation: bool,
    pub animation: TusdPrim,
    pub skinned: *const TusdPrim,
    pub skinned_count: usize,
}

#[unsafe(no_mangle)]
pub extern "C" fn tusd_skel_bindings_new() -> *mut TusdSkelBindings {
    Box::into_raw(Box::default())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_skel_bindings_free(bindings: *mut TusdSkelBindings) {
    if !bindings.is_null() {
        drop(unsafe { Box::from_raw(bindings) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_skel_bindings(
    stage: *mut TusdStage,
    root: TusdPrim,
    out: *mut TusdSkelBindings,
) -> TusdStatus {
    guard(|| {
        let out = unsafe { out_mut(out, "out") }?;
        let s = unsafe { stage_mut(stage) }?;
        out.bindings = s
            .stage
            .skel_bindings(prim(root))?
            .into_iter()
            .map(|b| {
                (
                    handle(b.skeleton),
                    b.animation.map(handle),
                    b.skinned.into_iter().map(handle).collect(),
                )
            })
            .collect();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_skel_bindings_len(bindings: *const TusdSkelBindings) -> usize {
    unsafe { bindings.as_ref() }.map_or(0, |b| b.bindings.len())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_skel_bindings_get(
    bindings: *const TusdSkelBindings,
    index: usize,
    out: *mut TusdSkelBindingView,
) -> TusdStatus {
    guard(|| {
        let b = unsafe { bindings.as_ref() }.ok_or_else(|| invalid("bindings must not be null"))?;
        let out = unsafe { out_mut(out, "out") }?;
        let Some((skeleton, animation, skinned)) = b.bindings.get(index) else {
            set_error(format!("binding index {index} out of range"));
            return Err(Fail(TusdStatus::InvalidArgument));
        };
        *out = TusdSkelBindingView {
            skeleton: *skeleton,
            has_animation: animation.is_some(),
            animation: animation.unwrap_or(0),
            skinned: skinned.as_ptr(),
            skinned_count: skinned.len(),
        };
        Ok(())
    })
}

#[derive(Default)]
pub struct TusdSkeletonPose {
    pose: core::SkeletonPose,
    joints: Vec<CString>,
    joint_ptrs: Vec<*const c_char>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdSkeletonPoseView {
    pub joint_count: usize,
    pub joints: *const *const c_char,
    pub parents: *const i32,
    pub bind: *const f64,
    pub bind_count: usize,
    pub rest_local: *const f64,
    pub rest_local_count: usize,
    pub local: *const f64,
    pub local_count: usize,
    pub skel: *const f64,
    pub skel_count: usize,
    pub skinning: *const f64,
    pub skinning_count: usize,
}

#[unsafe(no_mangle)]
pub extern "C" fn tusd_skeleton_pose_new() -> *mut TusdSkeletonPose {
    Box::into_raw(Box::default())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_skeleton_pose_free(pose: *mut TusdSkeletonPose) {
    if !pose.is_null() {
        drop(unsafe { Box::from_raw(pose) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_skeleton_pose(
    stage: *mut TusdStage,
    p: TusdPrim,
    time: *const f64,
    out: *mut TusdSkeletonPose,
    out_found: *mut bool,
) -> TusdStatus {
    guard(|| {
        let out = unsafe { out_mut(out, "out") }?;
        let found = unsafe { out_mut(out_found, "out_found") }?;
        let s = unsafe { stage_mut(stage) }?;
        let pose = s.stage.skeleton_pose(prim(p), unsafe { time_arg(time) })?;
        *found = pose.is_some();
        out.pose = pose.unwrap_or_default();
        out.joints = out.pose.joints.iter().map(|j| to_c_string(j)).collect();
        out.joint_ptrs = out.joints.iter().map(|j| j.as_ptr()).collect();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_skeleton_pose_view(
    pose: *const TusdSkeletonPose,
    out: *mut TusdSkeletonPoseView,
) -> TusdStatus {
    guard(|| {
        let p = unsafe { pose.as_ref() }.ok_or_else(|| invalid("pose must not be null"))?;
        let out = unsafe { out_mut(out, "out") }?;
        let m = |v: &Vec<[f64; 16]>| (v.as_ptr().cast::<f64>(), v.len());
        let (bind, bind_count) = m(&p.pose.bind);
        let (rest_local, rest_local_count) = m(&p.pose.rest_local);
        let (local, local_count) = m(&p.pose.local);
        let (skel, skel_count) = m(&p.pose.skel);
        let (skinning, skinning_count) = m(&p.pose.skinning);
        *out = TusdSkeletonPoseView {
            joint_count: p.joint_ptrs.len(),
            joints: p.joint_ptrs.as_ptr(),
            parents: p.pose.parents.as_ptr(),
            bind,
            bind_count,
            rest_local,
            rest_local_count,
            local,
            local_count,
            skel,
            skel_count,
            skinning,
            skinning_count,
        };
        Ok(())
    })
}

#[derive(Default)]
pub struct TusdSkinBinding {
    binding: Option<core::SkinBinding>,
    skeleton: TusdPrim,
    shapes: Vec<CString>,
    shape_ptrs: Vec<*const c_char>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdSkinBindingView {
    pub skeleton: TusdPrim,
    pub joint_indices: *const i32,
    pub joint_weights: *const f32,
    pub influence_count: usize,
    pub influences_per_point: u32,
    pub rigid: bool,
    pub geom_bind_transform: *const f64,
    pub skinning_method: TusdSkinningMethod,
    pub blend_shapes: *const *const c_char,
    pub blend_shape_weights: *const f32,
    pub blend_shape_count: usize,
}

#[unsafe(no_mangle)]
pub extern "C" fn tusd_skin_binding_new() -> *mut TusdSkinBinding {
    Box::into_raw(Box::default())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_skin_binding_free(binding: *mut TusdSkinBinding) {
    if !binding.is_null() {
        drop(unsafe { Box::from_raw(binding) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_skin_binding(
    stage: *mut TusdStage,
    p: TusdPrim,
    time: *const f64,
    out: *mut TusdSkinBinding,
    out_found: *mut bool,
) -> TusdStatus {
    guard(|| {
        let out = unsafe { out_mut(out, "out") }?;
        let found = unsafe { out_mut(out_found, "out_found") }?;
        let s = unsafe { stage_mut(stage) }?;
        let binding = s.stage.skin_binding(prim(p), unsafe { time_arg(time) })?;
        *found = binding.is_some();
        out.skeleton = binding.as_ref().map_or(0, |b| handle(b.skeleton));
        out.shapes = binding
            .iter()
            .flat_map(|b| b.blend_shapes.iter().map(|n| to_c_string(n)))
            .collect();
        out.shape_ptrs = out.shapes.iter().map(|n| n.as_ptr()).collect();
        out.binding = binding;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_skin_binding_view(
    binding: *const TusdSkinBinding,
    out: *mut TusdSkinBindingView,
) -> TusdStatus {
    guard(|| {
        let b = unsafe { binding.as_ref() }.ok_or_else(|| invalid("binding must not be null"))?;
        let out = unsafe { out_mut(out, "out") }?;
        let Some(inner) = &b.binding else {
            return Err(invalid("binding is empty; check out_found from tusd_prim_skin_binding"));
        };
        *out = TusdSkinBindingView {
            skeleton: b.skeleton,
            joint_indices: inner.joint_indices.as_ptr(),
            joint_weights: inner.joint_weights.as_ptr(),
            influence_count: inner.joint_indices.len(),
            influences_per_point: inner.influences_per_point,
            rigid: inner.rigid,
            geom_bind_transform: inner.geom_bind_transform.as_ptr(),
            skinning_method: match inner.skinning_method {
                core::SkinningMethod::ClassicLinear => TusdSkinningMethod::ClassicLinear,
                core::SkinningMethod::DualQuaternion => TusdSkinningMethod::DualQuaternion,
            },
            blend_shapes: b.shape_ptrs.as_ptr(),
            blend_shape_weights: inner.blend_shape_weights.as_ptr(),
            blend_shape_count: b.shape_ptrs.len(),
        };
        Ok(())
    })
}

#[derive(Default)]
pub struct TusdBlendShapes {
    shapes: core::BlendShapes,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdSubShapeView {
    pub blend_shape: u32,
    pub weight: f32,
    pub point_offsets: *const f32,
    pub point_offset_count: usize,
    pub normal_offsets: *const f32,
    pub normal_offset_count: usize,
}

#[unsafe(no_mangle)]
pub extern "C" fn tusd_blend_shapes_new() -> *mut TusdBlendShapes {
    Box::into_raw(Box::default())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_blend_shapes_free(shapes: *mut TusdBlendShapes) {
    if !shapes.is_null() {
        drop(unsafe { Box::from_raw(shapes) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_blend_shapes(
    stage: *mut TusdStage,
    p: TusdPrim,
    time: *const f64,
    out: *mut TusdBlendShapes,
    out_found: *mut bool,
) -> TusdStatus {
    guard(|| {
        let out = unsafe { out_mut(out, "out") }?;
        let found = unsafe { out_mut(out_found, "out_found") }?;
        let s = unsafe { stage_mut(stage) }?;
        let shapes = s.stage.blend_shapes(prim(p), unsafe { time_arg(time) })?;
        *found = shapes.is_some();
        out.shapes = shapes.unwrap_or_default();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_blend_shapes_count(shapes: *const TusdBlendShapes) -> usize {
    unsafe { shapes.as_ref() }.map_or(0, |s| s.shapes.point_indices.len())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_blend_shapes_point_indices(
    shapes: *const TusdBlendShapes,
    blend_shape: usize,
    out_count: *mut usize,
) -> *const i32 {
    let indices = unsafe { shapes.as_ref() }.and_then(|s| s.shapes.point_indices.get(blend_shape));
    if let Some(count) = unsafe { out_count.as_mut() } {
        *count = indices.map_or(0, Vec::len);
    }
    indices.map_or(ptr::null(), |v| v.as_ptr())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_blend_shapes_sub_shape_count(shapes: *const TusdBlendShapes) -> usize {
    unsafe { shapes.as_ref() }.map_or(0, |s| s.shapes.sub_shapes.len())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_blend_shapes_sub_shape(
    shapes: *const TusdBlendShapes,
    index: usize,
    out: *mut TusdSubShapeView,
) -> TusdStatus {
    guard(|| {
        let s = unsafe { shapes.as_ref() }.ok_or_else(|| invalid("shapes must not be null"))?;
        let out = unsafe { out_mut(out, "out") }?;
        let Some(sub) = s.shapes.sub_shapes.get(index) else {
            set_error(format!("sub-shape index {index} out of range"));
            return Err(Fail(TusdStatus::InvalidArgument));
        };
        *out = TusdSubShapeView {
            blend_shape: sub.blend_shape,
            weight: sub.weight,
            point_offsets: sub.point_offsets.as_ptr().cast(),
            point_offset_count: sub.point_offsets.len(),
            normal_offsets: sub.normal_offsets.as_ptr().cast(),
            normal_offset_count: sub.normal_offsets.len(),
        };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_skinned_points(
    stage: *mut TusdStage,
    p: TusdPrim,
    time: *const f64,
    out_points: *mut TusdValue,
    out_found: *mut bool,
) -> TusdStatus {
    guard(|| {
        let out = unsafe { out_mut(out_points, "out_points") }?;
        let found = unsafe { out_mut(out_found, "out_found") }?;
        let s = unsafe { stage_mut(stage) }?;
        match s.stage.skinned_points(prim(p), unsafe { time_arg(time) })? {
            Some(points) => {
                *found = true;
                out.value.set_float3_array(&points);
            }
            None => {
                *found = false;
                out.value.clear();
            }
        }
        out.sync();
        Ok(())
    })
}

pub const TUSD_NO_PRIM: TusdPrim = u32::MAX;

fn prims_ptr(prims: &[core::Prim]) -> *const TusdPrim {
    prims.as_ptr().cast()
}

fn opt_prim(p: Option<core::Prim>) -> TusdPrim {
    p.map_or(TUSD_NO_PRIM, handle)
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TusdAxis {
    X = 0,
    Y = 1,
    Z = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TusdShapeKind {
    Sphere = 0,
    Cube = 1,
    Capsule = 2,
    Capsule1 = 3,
    Cylinder = 4,
    Cylinder1 = 5,
    Cone = 6,
    Plane = 7,
    Mesh = 8,
    SpherePoints = 9,
    Custom = 10,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TusdJointKind {
    Fixed = 0,
    Revolute = 1,
    Prismatic = 2,
    Spherical = 3,
    Distance = 4,
    D6 = 5,
    Custom = 6,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TusdDof {
    Distance = 0,
    TransX = 1,
    TransY = 2,
    TransZ = 3,
    RotX = 4,
    RotY = 5,
    RotZ = 6,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TusdPhysicsKind {
    Scene = 0,
    CollisionGroup = 1,
    Material = 2,
    Shape = 3,
    RigidBody = 4,
    Joint = 5,
    Articulation = 6,
    Diagnostic = 7,
}

fn axis(a: core::Axis) -> TusdAxis {
    match a {
        core::Axis::X => TusdAxis::X,
        core::Axis::Y => TusdAxis::Y,
        core::Axis::Z => TusdAxis::Z,
    }
}

fn dof(d: core::Dof) -> TusdDof {
    match d {
        core::Dof::Distance => TusdDof::Distance,
        core::Dof::TransX => TusdDof::TransX,
        core::Dof::TransY => TusdDof::TransY,
        core::Dof::TransZ => TusdDof::TransZ,
        core::Dof::RotX => TusdDof::RotX,
        core::Dof::RotY => TusdDof::RotY,
        core::Dof::RotZ => TusdDof::RotZ,
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdPhysicsOptions {
    pub struct_size: u32,
    pub include: *const TusdPrim,
    pub include_count: usize,
    pub exclude: *const TusdPrim,
    pub exclude_count: usize,
    pub compute_mass: bool,
    pub si_units: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdPhysicsSceneView {
    pub prim: TusdPrim,
    pub gravity_direction: [f32; 3],
    pub gravity_magnitude: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdCollisionGroupView {
    pub prim: TusdPrim,
    pub invert_filtered_groups: bool,
    pub filtered_groups: *const TusdPrim,
    pub filtered_group_count: usize,
    pub merge_group_name: *const c_char,
    pub merged_groups: *const TusdPrim,
    pub merged_group_count: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdPhysicsMaterialView {
    pub prim: TusdPrim,
    pub static_friction: f32,
    pub dynamic_friction: f32,
    pub restitution: f32,
    pub density: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdShapeView {
    pub prim: TusdPrim,
    pub rigid_body: TusdPrim,
    pub local_pos: [f32; 3],
    pub local_rot: [f32; 4],
    pub local_scale: [f32; 3],
    pub materials: *const TusdPrim,
    pub material_count: usize,
    pub simulation_owners: *const TusdPrim,
    pub simulation_owner_count: usize,
    pub filtered_collisions: *const TusdPrim,
    pub filtered_collision_count: usize,
    pub collision_groups: *const TusdPrim,
    pub collision_group_count: usize,
    pub collision_enabled: bool,
    pub kind: TusdShapeKind,
    pub radius: f32,
    pub top_radius: f32,
    pub bottom_radius: f32,
    pub half_height: f32,
    pub half_extents: [f32; 3],
    pub axis: TusdAxis,
    pub mesh_approximation: *const c_char,
    pub mesh_scale: [f32; 3],
    pub double_sided: bool,
    pub sphere_points: *const f32,
    pub sphere_point_count: usize,
    pub custom_type: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdRigidBodyView {
    pub prim: TusdPrim,
    pub collisions: *const TusdPrim,
    pub collision_count: usize,
    pub filtered_collisions: *const TusdPrim,
    pub filtered_collision_count: usize,
    pub simulation_owners: *const TusdPrim,
    pub simulation_owner_count: usize,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub enabled: bool,
    pub kinematic: bool,
    pub starts_asleep: bool,
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub has_mass: bool,
    pub mass: f64,
    pub diagonal_inertia: [f64; 3],
    pub center_of_mass: [f64; 3],
    pub principal_axes: [f64; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdJointLimit {
    pub dof: TusdDof,
    pub enabled: bool,
    pub lower: f32,
    pub upper: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdJointDrive {
    pub dof: TusdDof,
    pub target_position: f32,
    pub target_velocity: f32,
    pub force_limit: f32,
    pub stiffness: f32,
    pub damping: f32,
    pub acceleration: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdJointView {
    pub prim: TusdPrim,
    pub kind: TusdJointKind,
    pub type_name: *const c_char,
    pub rel0: TusdPrim,
    pub rel1: TusdPrim,
    pub body0: TusdPrim,
    pub body1: TusdPrim,
    pub local_pos0: [f32; 3],
    pub local_rot0: [f32; 4],
    pub local_pos1: [f32; 3],
    pub local_rot1: [f32; 4],
    pub enabled: bool,
    pub collision_enabled: bool,
    pub break_force: f32,
    pub break_torque: f32,
    pub exclude_from_articulation: bool,
    pub axis: TusdAxis,
    pub limits: *const TusdJointLimit,
    pub limit_count: usize,
    pub drives: *const TusdJointDrive,
    pub drive_count: usize,
    pub min_distance_enabled: bool,
    pub max_distance_enabled: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TusdArticulationView {
    pub prim: TusdPrim,
    pub roots: *const TusdPrim,
    pub root_count: usize,
    pub filtered_collisions: *const TusdPrim,
    pub filtered_collision_count: usize,
    pub joints: *const TusdPrim,
    pub joint_count: usize,
    pub bodies: *const TusdPrim,
    pub body_count: usize,
}

#[derive(Default)]
pub struct TusdPhysics {
    physics: core::Physics,
    strings: Vec<CString>,
    group_names: Vec<usize>,
    shape_strings: Vec<(usize, usize)>,
    joint_types: Vec<usize>,
    diagnostics: Vec<usize>,
    limits: Vec<Vec<TusdJointLimit>>,
    drives: Vec<Vec<TusdJointDrive>>,
    articulation_bodies: Vec<Vec<TusdPrim>>,
}

impl TusdPhysics {
    fn keep(&mut self, s: &str) -> usize {
        self.strings.push(to_c_string(s));
        self.strings.len() - 1
    }

    fn rebuild(&mut self) {
        self.strings.clear();
        let names: Vec<String> = self
            .physics
            .collision_groups
            .iter()
            .map(|g| g.merge_group_name.clone())
            .collect();
        self.group_names = names.iter().map(|n| self.keep(n)).collect();
        let shapes: Vec<(String, String)> = self
            .physics
            .shapes
            .iter()
            .map(|s| (s.mesh_approximation.clone(), s.custom_type.clone()))
            .collect();
        self.shape_strings = shapes.iter().map(|(a, c)| (self.keep(a), self.keep(c))).collect();
        let types: Vec<String> = self.physics.joints.iter().map(|j| j.type_name.clone()).collect();
        self.joint_types = types.iter().map(|t| self.keep(t)).collect();
        let diagnostics = self.physics.diagnostics.clone();
        self.diagnostics = diagnostics.iter().map(|d| self.keep(d)).collect();
        self.limits = self
            .physics
            .joints
            .iter()
            .map(|j| {
                j.limits
                    .iter()
                    .map(|l| TusdJointLimit {
                        dof: dof(l.dof),
                        enabled: l.enabled,
                        lower: l.lower,
                        upper: l.upper,
                    })
                    .collect()
            })
            .collect();
        self.drives = self
            .physics
            .joints
            .iter()
            .map(|j| {
                j.drives
                    .iter()
                    .map(|d| TusdJointDrive {
                        dof: dof(d.dof),
                        target_position: d.target_position,
                        target_velocity: d.target_velocity,
                        force_limit: d.force_limit,
                        stiffness: d.stiffness,
                        damping: d.damping,
                        acceleration: d.acceleration,
                    })
                    .collect()
            })
            .collect();
        self.articulation_bodies = self
            .physics
            .articulations
            .iter()
            .map(|a| a.bodies.iter().map(|b| opt_prim(*b)).collect())
            .collect();
    }

    fn string(&self, index: usize) -> *const c_char {
        self.strings.get(index).map_or(ptr::null(), |s| s.as_ptr())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tusd_physics_new() -> *mut TusdPhysics {
    Box::into_raw(Box::default())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_free(physics: *mut TusdPhysics) {
    if !physics.is_null() {
        drop(unsafe { Box::from_raw(physics) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_options_init(options: *mut TusdPhysicsOptions) {
    if let Some(o) = unsafe { options.as_mut() } {
        *o = TusdPhysicsOptions {
            struct_size: size_of::<TusdPhysicsOptions>() as u32,
            include: ptr::null(),
            include_count: 0,
            exclude: ptr::null(),
            exclude_count: 0,
            compute_mass: true,
            si_units: false,
        };
    }
}

unsafe fn prim_slice<'a>(ptr: *const TusdPrim, count: usize, what: &str) -> Result<&'a [TusdPrim], Fail> {
    if count == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(invalid(&format!("{what} must not be null when its count is set")));
    }
    Ok(unsafe { std::slice::from_raw_parts(ptr, count) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_physics(
    stage: *mut TusdStage,
    options: *const TusdPhysicsOptions,
    out: *mut TusdPhysics,
) -> TusdStatus {
    guard(|| {
        let out = unsafe { out_mut(out, "out") }?;
        let s = unsafe { stage_mut(stage) }?;
        let mut opts = core::PhysicsOptions {
            compute_mass: true,
            ..core::PhysicsOptions::default()
        };
        if let Some(o) = unsafe { options.as_ref() } {
            if (o.struct_size as usize) < size_of::<TusdPhysicsOptions>() {
                return Err(invalid(
                    "options.struct_size is too small; call tusd_physics_options_init",
                ));
            }
            opts.include = unsafe { prim_slice(o.include, o.include_count, "include") }?
                .iter()
                .map(|&p| prim(p))
                .collect();
            opts.exclude = unsafe { prim_slice(o.exclude, o.exclude_count, "exclude") }?
                .iter()
                .map(|&p| prim(p))
                .collect();
            opts.compute_mass = o.compute_mass;
            opts.si_units = o.si_units;
        }
        out.physics = s.stage.physics(&opts)?;
        out.rebuild();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_units(
    physics: *const TusdPhysics,
    out_meters_per_unit: *mut f64,
    out_kilograms_per_unit: *mut f64,
    out_si_units: *mut bool,
) -> TusdStatus {
    guard(|| {
        let p = unsafe { physics.as_ref() }.ok_or_else(|| invalid("physics must not be null"))?;
        *unsafe { out_mut(out_meters_per_unit, "out_meters_per_unit") }? = p.physics.meters_per_unit;
        *unsafe { out_mut(out_kilograms_per_unit, "out_kilograms_per_unit") }? = p.physics.kilograms_per_unit;
        *unsafe { out_mut(out_si_units, "out_si_units") }? = p.physics.si_units;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_count(physics: *const TusdPhysics, kind: TusdPhysicsKind) -> usize {
    let Some(p) = (unsafe { physics.as_ref() }) else {
        return 0;
    };
    let p = &p.physics;
    match kind {
        TusdPhysicsKind::Scene => p.scenes.len(),
        TusdPhysicsKind::CollisionGroup => p.collision_groups.len(),
        TusdPhysicsKind::Material => p.materials.len(),
        TusdPhysicsKind::Shape => p.shapes.len(),
        TusdPhysicsKind::RigidBody => p.rigid_bodies.len(),
        TusdPhysicsKind::Joint => p.joints.len(),
        TusdPhysicsKind::Articulation => p.articulations.len(),
        TusdPhysicsKind::Diagnostic => p.diagnostics.len(),
    }
}

fn physics_item<'a, T>(
    physics: *const TusdPhysics,
    index: usize,
    pick: impl Fn(&core::Physics) -> &[T],
) -> Result<(&'a TusdPhysics, &'a T), Fail> {
    let p = unsafe { physics.as_ref() }.ok_or_else(|| invalid("physics must not be null"))?;
    let items = pick(&p.physics);
    let Some(item) = items.get(index) else {
        set_error(format!("index {index} out of range"));
        return Err(Fail(TusdStatus::InvalidArgument));
    };
    let item: *const T = item;
    Ok((p, unsafe { &*item }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_scene(
    physics: *const TusdPhysics,
    index: usize,
    out: *mut TusdPhysicsSceneView,
) -> TusdStatus {
    guard(|| {
        let out = unsafe { out_mut(out, "out") }?;
        let (_, s) = physics_item(physics, index, |p| &p.scenes)?;
        *out = TusdPhysicsSceneView {
            prim: handle(s.prim),
            gravity_direction: s.gravity_direction,
            gravity_magnitude: s.gravity_magnitude,
        };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_collision_group(
    physics: *const TusdPhysics,
    index: usize,
    out: *mut TusdCollisionGroupView,
) -> TusdStatus {
    guard(|| {
        let out = unsafe { out_mut(out, "out") }?;
        let (p, g) = physics_item(physics, index, |p| &p.collision_groups)?;
        *out = TusdCollisionGroupView {
            prim: handle(g.prim),
            invert_filtered_groups: g.invert_filtered_groups,
            filtered_groups: prims_ptr(&g.filtered_groups),
            filtered_group_count: g.filtered_groups.len(),
            merge_group_name: p.string(p.group_names[index]),
            merged_groups: prims_ptr(&g.merged_groups),
            merged_group_count: g.merged_groups.len(),
        };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_material(
    physics: *const TusdPhysics,
    index: usize,
    out: *mut TusdPhysicsMaterialView,
) -> TusdStatus {
    guard(|| {
        let out = unsafe { out_mut(out, "out") }?;
        let (_, m) = physics_item(physics, index, |p| &p.materials)?;
        *out = TusdPhysicsMaterialView {
            prim: handle(m.prim),
            static_friction: m.static_friction,
            dynamic_friction: m.dynamic_friction,
            restitution: m.restitution,
            density: m.density,
        };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_shape(
    physics: *const TusdPhysics,
    index: usize,
    out: *mut TusdShapeView,
) -> TusdStatus {
    guard(|| {
        let out = unsafe { out_mut(out, "out") }?;
        let (p, s) = physics_item(physics, index, |p| &p.shapes)?;
        let (approximation, custom) = p.shape_strings[index];
        *out = TusdShapeView {
            prim: handle(s.prim),
            rigid_body: opt_prim(s.rigid_body),
            local_pos: s.local_pos,
            local_rot: s.local_rot,
            local_scale: s.local_scale,
            materials: prims_ptr(&s.materials),
            material_count: s.materials.len(),
            simulation_owners: prims_ptr(&s.simulation_owners),
            simulation_owner_count: s.simulation_owners.len(),
            filtered_collisions: prims_ptr(&s.filtered_collisions),
            filtered_collision_count: s.filtered_collisions.len(),
            collision_groups: prims_ptr(&s.collision_groups),
            collision_group_count: s.collision_groups.len(),
            collision_enabled: s.collision_enabled,
            kind: match s.kind {
                core::ShapeKind::Sphere => TusdShapeKind::Sphere,
                core::ShapeKind::Cube => TusdShapeKind::Cube,
                core::ShapeKind::Capsule => TusdShapeKind::Capsule,
                core::ShapeKind::Capsule1 => TusdShapeKind::Capsule1,
                core::ShapeKind::Cylinder => TusdShapeKind::Cylinder,
                core::ShapeKind::Cylinder1 => TusdShapeKind::Cylinder1,
                core::ShapeKind::Cone => TusdShapeKind::Cone,
                core::ShapeKind::Plane => TusdShapeKind::Plane,
                core::ShapeKind::Mesh => TusdShapeKind::Mesh,
                core::ShapeKind::SpherePoints => TusdShapeKind::SpherePoints,
                core::ShapeKind::Custom => TusdShapeKind::Custom,
            },
            radius: s.radius,
            top_radius: s.top_radius,
            bottom_radius: s.bottom_radius,
            half_height: s.half_height,
            half_extents: s.half_extents,
            axis: axis(s.axis),
            mesh_approximation: p.string(approximation),
            mesh_scale: s.mesh_scale,
            double_sided: s.double_sided,
            sphere_points: s.sphere_points.as_ptr().cast(),
            sphere_point_count: s.sphere_points.len(),
            custom_type: p.string(custom),
        };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_rigid_body(
    physics: *const TusdPhysics,
    index: usize,
    out: *mut TusdRigidBodyView,
) -> TusdStatus {
    guard(|| {
        let out = unsafe { out_mut(out, "out") }?;
        let (_, b) = physics_item(physics, index, |p| &p.rigid_bodies)?;
        let mass = b.mass.unwrap_or_default();
        *out = TusdRigidBodyView {
            prim: handle(b.prim),
            collisions: prims_ptr(&b.collisions),
            collision_count: b.collisions.len(),
            filtered_collisions: prims_ptr(&b.filtered_collisions),
            filtered_collision_count: b.filtered_collisions.len(),
            simulation_owners: prims_ptr(&b.simulation_owners),
            simulation_owner_count: b.simulation_owners.len(),
            position: b.position,
            rotation: b.rotation,
            scale: b.scale,
            enabled: b.enabled,
            kinematic: b.kinematic,
            starts_asleep: b.starts_asleep,
            linear_velocity: b.linear_velocity,
            angular_velocity: b.angular_velocity,
            has_mass: b.mass.is_some(),
            mass: mass.mass,
            diagonal_inertia: mass.diagonal_inertia,
            center_of_mass: mass.center_of_mass,
            principal_axes: mass.principal_axes,
        };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_joint(
    physics: *const TusdPhysics,
    index: usize,
    out: *mut TusdJointView,
) -> TusdStatus {
    guard(|| {
        let out = unsafe { out_mut(out, "out") }?;
        let (p, j) = physics_item(physics, index, |p| &p.joints)?;
        *out = TusdJointView {
            prim: handle(j.prim),
            kind: match j.kind {
                core::JointKind::Fixed => TusdJointKind::Fixed,
                core::JointKind::Revolute => TusdJointKind::Revolute,
                core::JointKind::Prismatic => TusdJointKind::Prismatic,
                core::JointKind::Spherical => TusdJointKind::Spherical,
                core::JointKind::Distance => TusdJointKind::Distance,
                core::JointKind::D6 => TusdJointKind::D6,
                core::JointKind::Custom => TusdJointKind::Custom,
            },
            type_name: p.string(p.joint_types[index]),
            rel0: opt_prim(j.rel0),
            rel1: opt_prim(j.rel1),
            body0: opt_prim(j.body0),
            body1: opt_prim(j.body1),
            local_pos0: j.local_pos0,
            local_rot0: j.local_rot0,
            local_pos1: j.local_pos1,
            local_rot1: j.local_rot1,
            enabled: j.enabled,
            collision_enabled: j.collision_enabled,
            break_force: j.break_force,
            break_torque: j.break_torque,
            exclude_from_articulation: j.exclude_from_articulation,
            axis: axis(j.axis),
            limits: p.limits[index].as_ptr(),
            limit_count: p.limits[index].len(),
            drives: p.drives[index].as_ptr(),
            drive_count: p.drives[index].len(),
            min_distance_enabled: j.min_distance_enabled,
            max_distance_enabled: j.max_distance_enabled,
        };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_articulation(
    physics: *const TusdPhysics,
    index: usize,
    out: *mut TusdArticulationView,
) -> TusdStatus {
    guard(|| {
        let out = unsafe { out_mut(out, "out") }?;
        let (p, a) = physics_item(physics, index, |p| &p.articulations)?;
        let bodies = &p.articulation_bodies[index];
        *out = TusdArticulationView {
            prim: handle(a.prim),
            roots: prims_ptr(&a.roots),
            root_count: a.roots.len(),
            filtered_collisions: prims_ptr(&a.filtered_collisions),
            filtered_collision_count: a.filtered_collisions.len(),
            joints: prims_ptr(&a.joints),
            joint_count: a.joints.len(),
            bodies: bodies.as_ptr(),
            body_count: bodies.len(),
        };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_diagnostic(physics: *const TusdPhysics, index: usize) -> *const c_char {
    unsafe { physics.as_ref() }
        .and_then(|p| p.diagnostics.get(index).map(|&i| p.string(i)))
        .unwrap_or(ptr::null())
}
