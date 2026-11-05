#![cfg_attr(not(feature = "write"), allow(unused_variables, unused_imports, dead_code))]

use std::ffi::{c_char, c_void};
use std::ptr;

use super::{
    Fail, TusdPrim, TusdStage, TusdStatus, TusdValue, core, guard, invalid, open_options, out_mut, prim, stage_mut,
    str_arg, time_arg, wrap_stage,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TusdLayerFormat {
    Usda = 0,
    Usdc = 1,
    Usdz = 2,
}

#[cfg(feature = "write")]
macro_rules! write_api {
    ($body:block) => {
        guard(|| $body)
    };
}

#[cfg(not(feature = "write"))]
macro_rules! write_api {
    ($body:block) => {
        guard(|| Err(Fail::from(core::Error::FeatureDisabled("write"))))
    };
}

fn base_type(raw: u32) -> Result<core::BaseType, Fail> {
    use core::BaseType as B;
    const TYPES: [core::BaseType; 33] = [
        B::Empty,
        B::Bool,
        B::UChar,
        B::Int,
        B::UInt,
        B::Int64,
        B::UInt64,
        B::Half,
        B::Float,
        B::Double,
        B::TimeCode,
        B::String,
        B::Token,
        B::Asset,
        B::Path,
        B::Int2,
        B::Int3,
        B::Int4,
        B::Half2,
        B::Half3,
        B::Half4,
        B::Float2,
        B::Float3,
        B::Float4,
        B::Double2,
        B::Double3,
        B::Double4,
        B::Quath,
        B::Quatf,
        B::Quatd,
        B::Matrix2d,
        B::Matrix3d,
        B::Matrix4d,
    ];
    match TYPES.get(raw as usize) {
        Some(&t) if t != B::Empty => Ok(t),
        _ => Err(invalid(&format!("{raw} is not a writable TusdValueType"))),
    }
}

unsafe fn strs<'a>(items: *const *const c_char, count: usize, what: &str) -> Result<Vec<&'a str>, Fail> {
    if count == 0 {
        return Ok(Vec::new());
    }
    if items.is_null() {
        return Err(invalid(&format!("{what} must not be null when its count is set")));
    }
    unsafe { std::slice::from_raw_parts(items, count) }
        .iter()
        .map(|&s| unsafe { str_arg(s, what) })
        .collect()
}

unsafe fn opt_str<'a>(s: *const c_char, what: &str) -> Result<&'a str, Fail> {
    if s.is_null() {
        Ok("")
    } else {
        unsafe { str_arg(s, what) }
    }
}

#[cfg(feature = "write")]
fn layer_format(format: u32) -> Result<core::LayerFormat, Fail> {
    match format {
        0 => Ok(core::LayerFormat::Usda),
        1 => Ok(core::LayerFormat::Usdc),
        2 => Ok(core::LayerFormat::Usdz),
        _ => Err(invalid(&format!("{format} is not a TusdLayerFormat"))),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_value_set_data(
    value: *mut TusdValue,
    r#type: u32,
    is_array: bool,
    data: *const c_void,
    count: usize,
) -> TusdStatus {
    write_api!({
        let v = unsafe { out_mut(value, "value") }?;
        let base = base_type(r#type)?;
        let size = count * base.element_size();
        if size > 0 && data.is_null() {
            return Err(invalid("data must not be null when count is set"));
        }
        let bytes = if size == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size) }
        };
        v.value = core::Value::from_bytes(base, is_array, bytes)?;
        v.sync();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_value_set_strings(
    value: *mut TusdValue,
    r#type: u32,
    is_array: bool,
    strings: *const *const c_char,
    count: usize,
) -> TusdStatus {
    write_api!({
        let v = unsafe { out_mut(value, "value") }?;
        let base = base_type(r#type)?;
        let items = unsafe { strs(strings, count, "strings") }?;
        v.value = core::Value::from_strings(base, is_array, &items)?;
        v.sync();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_create_new(
    path: *const c_char,
    options: *const super::TusdOpenOptions,
    out_stage: *mut *mut TusdStage,
) -> TusdStatus {
    write_api!({
        let out = unsafe { out_mut(out_stage, "out_stage") }?;
        *out = ptr::null_mut();
        let path = unsafe { str_arg(path, "path") }?;
        let opts = unsafe { open_options(options) }?;
        *out = wrap_stage(core::Stage::create_new(path, &opts)?);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_create_in_memory(
    options: *const super::TusdOpenOptions,
    out_stage: *mut *mut TusdStage,
) -> TusdStatus {
    write_api!({
        let out = unsafe { out_mut(out_stage, "out_stage") }?;
        *out = ptr::null_mut();
        let opts = unsafe { open_options(options) }?;
        *out = wrap_stage(core::Stage::create_in_memory(&opts)?);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_save(stage: *mut TusdStage) -> TusdStatus {
    write_api!({
        unsafe { stage_mut(stage) }?.stage.save()?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_export(stage: *mut TusdStage, path: *const c_char) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        s.stage.export(unsafe { str_arg(path, "path") }?)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_export_layer(
    stage: *mut TusdStage,
    layer: *const c_char,
    path: *const c_char,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let layer = unsafe { str_arg(layer, "layer") }?;
        s.stage.export_layer(layer, unsafe { str_arg(path, "path") }?)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_export_bytes(
    stage: *mut TusdStage,
    layer: *const c_char,
    format: u32,
    out_bytes: *mut TusdValue,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let out = unsafe { out_mut(out_bytes, "out_bytes") }?;
        let layer = if layer.is_null() {
            None
        } else {
            Some(unsafe { str_arg(layer, "layer") }?)
        };
        let bytes = s.stage.export_bytes(layer, layer_format(format)?)?;
        out.set_bytes(bytes);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_edit_target(stage: *mut TusdStage) -> *const c_char {
    #[cfg(feature = "write")]
    if let Some(s) = unsafe { stage.as_mut() } {
        let target = s.stage.edit_target();
        return s.intern(&target);
    }
    ptr::null()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_set_edit_target(stage: *mut TusdStage, layer: *const c_char) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        s.stage.set_edit_target(unsafe { str_arg(layer, "layer") }?)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_add_sublayer(
    stage: *mut TusdStage,
    asset_path: *const c_char,
    create: bool,
    out_identifier: *mut *const c_char,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let id = s
            .stage
            .add_sublayer(unsafe { str_arg(asset_path, "asset_path") }?, create)?;
        if let Some(out) = unsafe { out_identifier.as_mut() } {
            *out = s.intern(&id);
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_begin_edit(stage: *mut TusdStage) -> TusdStatus {
    write_api!({
        unsafe { stage_mut(stage) }?.stage.begin_edit()?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_commit_edit(stage: *mut TusdStage) -> TusdStatus {
    write_api!({
        unsafe { stage_mut(stage) }?.stage.commit_edit()?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_discard_edit(stage: *mut TusdStage) -> TusdStatus {
    write_api!({
        unsafe { stage_mut(stage) }?.stage.discard_edit();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_set_default_prim(stage: *mut TusdStage, p: TusdPrim) -> TusdStatus {
    write_api!({
        unsafe { stage_mut(stage) }?.stage.set_default_prim(prim(p))?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_set_up_axis(stage: *mut TusdStage, axis: *const c_char) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        s.stage.set_up_axis(unsafe { str_arg(axis, "axis") }?)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_set_meters_per_unit(stage: *mut TusdStage, meters: f64) -> TusdStatus {
    write_api!({
        unsafe { stage_mut(stage) }?.stage.set_meters_per_unit(meters)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_set_kilograms_per_unit(stage: *mut TusdStage, kilograms: f64) -> TusdStatus {
    write_api!({
        unsafe { stage_mut(stage) }?.stage.set_kilograms_per_unit(kilograms)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_set_time_range(
    stage: *mut TusdStage,
    range: *const super::TusdTimeRange,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let r = unsafe { range.as_ref() }.ok_or_else(|| invalid("range must not be null"))?;
        s.stage.set_time_code_range(r.start_time_code, r.end_time_code)?;
        if r.time_codes_per_second > 0.0 {
            s.stage.set_time_codes_per_second(r.time_codes_per_second)?;
        }
        if r.frames_per_second > 0.0 {
            s.stage.set_frames_per_second(r.frames_per_second)?;
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_set_documentation(stage: *mut TusdStage, text: *const c_char) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        s.stage.set_documentation(unsafe { str_arg(text, "text") }?)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_define_prim(
    stage: *mut TusdStage,
    path: *const c_char,
    type_name: *const c_char,
    out_prim: *mut TusdPrim,
) -> TusdStatus {
    write_api!({
        let out = unsafe { out_mut(out_prim, "out_prim") }?;
        let s = unsafe { stage_mut(stage) }?;
        let path = unsafe { str_arg(path, "path") }?;
        let type_name = unsafe { opt_str(type_name, "type_name") }?;
        *out = s.stage.define_prim(path, type_name)?.index();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_override_prim(
    stage: *mut TusdStage,
    path: *const c_char,
    out_prim: *mut TusdPrim,
) -> TusdStatus {
    write_api!({
        let out = unsafe { out_mut(out_prim, "out_prim") }?;
        let s = unsafe { stage_mut(stage) }?;
        *out = s.stage.override_prim(unsafe { str_arg(path, "path") }?)?.index();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_remove(stage: *mut TusdStage, p: TusdPrim, out_removed: *mut bool) -> TusdStatus {
    write_api!({
        let removed = unsafe { stage_mut(stage) }?.stage.remove_prim(prim(p))?;
        if let Some(out) = unsafe { out_removed.as_mut() } {
            *out = removed;
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_set_type_name(
    stage: *mut TusdStage,
    p: TusdPrim,
    type_name: *const c_char,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        s.stage
            .set_type_name(prim(p), unsafe { opt_str(type_name, "type_name") }?)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_set_active(stage: *mut TusdStage, p: TusdPrim, active: bool) -> TusdStatus {
    write_api!({
        unsafe { stage_mut(stage) }?.stage.set_active(prim(p), active)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_set_kind(stage: *mut TusdStage, p: TusdPrim, kind: *const c_char) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        s.stage.set_kind(prim(p), unsafe { str_arg(kind, "kind") }?)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_set_instanceable(
    stage: *mut TusdStage,
    p: TusdPrim,
    instanceable: bool,
) -> TusdStatus {
    write_api!({
        unsafe { stage_mut(stage) }?
            .stage
            .set_instanceable(prim(p), instanceable)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_apply_api(stage: *mut TusdStage, p: TusdPrim, schema: *const c_char) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        s.stage.apply_api(prim(p), unsafe { str_arg(schema, "schema") }?)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_add_reference(
    stage: *mut TusdStage,
    p: TusdPrim,
    asset_path: *const c_char,
    prim_path: *const c_char,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let asset = unsafe { opt_str(asset_path, "asset_path") }?;
        let target = unsafe { opt_str(prim_path, "prim_path") }?;
        s.stage.add_reference(prim(p), asset, target)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_add_payload(
    stage: *mut TusdStage,
    p: TusdPrim,
    asset_path: *const c_char,
    prim_path: *const c_char,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let asset = unsafe { opt_str(asset_path, "asset_path") }?;
        let target = unsafe { opt_str(prim_path, "prim_path") }?;
        s.stage.add_payload(prim(p), asset, target)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_add_inherit(stage: *mut TusdStage, p: TusdPrim, path: *const c_char) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        s.stage.add_inherit(prim(p), unsafe { str_arg(path, "path") }?)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_add_specialize(
    stage: *mut TusdStage,
    p: TusdPrim,
    path: *const c_char,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        s.stage.add_specialize(prim(p), unsafe { str_arg(path, "path") }?)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_set_variant_selection(
    stage: *mut TusdStage,
    p: TusdPrim,
    variant_set: *const c_char,
    variant: *const c_char,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let set = unsafe { str_arg(variant_set, "variant_set") }?;
        let variant = unsafe { str_arg(variant, "variant") }?;
        s.stage.set_variant_selection(prim(p), set, variant)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_attribute_create(
    stage: *mut TusdStage,
    p: TusdPrim,
    name: *const c_char,
    type_name: *const c_char,
    uniform: bool,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let name = unsafe { str_arg(name, "name") }?;
        let type_name = unsafe { str_arg(type_name, "type_name") }?;
        s.stage.create_attribute(prim(p), name, type_name, uniform)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_attribute_set(
    stage: *mut TusdStage,
    p: TusdPrim,
    name: *const c_char,
    time: *const f64,
    value: *const TusdValue,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let name = unsafe { str_arg(name, "name") }?;
        let v = unsafe { value.as_ref() }.ok_or_else(|| invalid("value must not be null"))?;
        s.stage.set(prim(p), name, &v.value, unsafe { time_arg(time) })?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_attribute_block(stage: *mut TusdStage, p: TusdPrim, name: *const c_char) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        s.stage.block(prim(p), unsafe { str_arg(name, "name") }?)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_attribute_clear(stage: *mut TusdStage, p: TusdPrim, name: *const c_char) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        s.stage.clear(prim(p), unsafe { str_arg(name, "name") }?)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_attribute_set_connections(
    stage: *mut TusdStage,
    p: TusdPrim,
    name: *const c_char,
    targets: *const *const c_char,
    count: usize,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let name = unsafe { str_arg(name, "name") }?;
        let targets = unsafe { strs(targets, count, "targets") }?;
        s.stage.set_connections(prim(p), name, &targets)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_relationship_set_targets(
    stage: *mut TusdStage,
    p: TusdPrim,
    name: *const c_char,
    targets: *const *const c_char,
    count: usize,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let name = unsafe { str_arg(name, "name") }?;
        let targets = unsafe { strs(targets, count, "targets") }?;
        s.stage.set_targets(prim(p), name, &targets)?;
        Ok(())
    })
}

unsafe fn fixed<'a, T, const N: usize>(ptr: *const T, what: &str) -> Result<&'a [T; N], Fail> {
    unsafe { ptr.cast::<[T; N]>().as_ref() }.ok_or_else(|| invalid(&format!("{what} must not be null")))
}

unsafe fn slice<'a, T>(ptr: *const T, len: usize, what: &str) -> Result<&'a [T], Fail> {
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(invalid(&format!("{what} must not be null when its count is set")));
    }
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_set_local_transform(
    stage: *mut TusdStage,
    p: TusdPrim,
    matrix: *const f64,
    time: *const f64,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let m = unsafe { fixed::<f64, 16>(matrix, "matrix") }?;
        s.stage.set_local_transform(prim(p), m, unsafe { time_arg(time) })?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_set_world_transform(
    stage: *mut TusdStage,
    p: TusdPrim,
    matrix: *const f64,
    time: *const f64,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let m = unsafe { fixed::<f64, 16>(matrix, "matrix") }?;
        s.stage.set_world_transform(prim(p), m, unsafe { time_arg(time) })?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_set_trs(
    stage: *mut TusdStage,
    p: TusdPrim,
    translate: *const f64,
    orient: *const f64,
    scale: *const f64,
    time: *const f64,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let t = unsafe { fixed::<f64, 3>(translate, "translate") }?;
        let o = unsafe { fixed::<f64, 4>(orient, "orient") }?;
        let sc = unsafe { fixed::<f64, 3>(scale, "scale") }?;
        s.stage.set_trs(prim(p), *t, *o, *sc, unsafe { time_arg(time) })?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn tusd_prim_set_mesh(
    stage: *mut TusdStage,
    p: TusdPrim,
    points: *const f32,
    point_count: usize,
    face_vertex_counts: *const i32,
    face_count: usize,
    face_vertex_indices: *const i32,
    index_count: usize,
    time: *const f64,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let pts = unsafe { slice(points.cast::<[f32; 3]>(), point_count, "points") }?;
        let counts = unsafe { slice(face_vertex_counts, face_count, "face_vertex_counts") }?;
        let indices = unsafe { slice(face_vertex_indices, index_count, "face_vertex_indices") }?;
        s.stage
            .set_mesh(prim(p), pts, counts, indices, unsafe { time_arg(time) })?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn tusd_prim_set_primvar(
    stage: *mut TusdStage,
    p: TusdPrim,
    name: *const c_char,
    type_name: *const c_char,
    value: *const TusdValue,
    interpolation: u32,
    indices: *const i32,
    index_count: usize,
    element_size: i32,
    time: *const f64,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let name = unsafe { str_arg(name, "name") }?;
        let type_name = unsafe { str_arg(type_name, "type_name") }?;
        let v = unsafe { value.as_ref() }.ok_or_else(|| invalid("value must not be null"))?;
        let interpolation = match interpolation {
            0 => core::Interpolation::Constant,
            1 => core::Interpolation::Uniform,
            2 => core::Interpolation::Varying,
            3 => core::Interpolation::Vertex,
            4 => core::Interpolation::FaceVarying,
            other => return Err(invalid(&format!("{other} is not a TusdInterpolation"))),
        };
        let indices = if indices.is_null() {
            None
        } else {
            Some(unsafe { slice(indices, index_count, "indices") }?)
        };
        s.stage.set_primvar(
            prim(p),
            name,
            type_name,
            &v.value,
            interpolation,
            indices,
            element_size,
            unsafe { time_arg(time) },
        )?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_bind_material(
    stage: *mut TusdStage,
    p: TusdPrim,
    material: TusdPrim,
    purpose: *const c_char,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let purpose = unsafe { opt_str(purpose, "purpose") }?;
        s.stage.bind_material(prim(p), prim(material), purpose)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_export_package(stage: *mut TusdStage, path: *const c_char) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        s.stage.export_package(unsafe { str_arg(path, "path") }?)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_remove_api(stage: *mut TusdStage, p: TusdPrim, schema: *const c_char) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        s.stage.remove_api(prim(p), unsafe { str_arg(schema, "schema") }?)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_relationship_add_target(
    stage: *mut TusdStage,
    p: TusdPrim,
    name: *const c_char,
    target: *const c_char,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let name = unsafe { str_arg(name, "name") }?;
        s.stage
            .add_target(prim(p), name, unsafe { str_arg(target, "target") }?)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_move(
    stage: *mut TusdStage,
    p: TusdPrim,
    new_parent: TusdPrim,
    new_name: *const c_char,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let name = if new_name.is_null() {
            s.stage.name(prim(p))?.to_owned()
        } else {
            unsafe { str_arg(new_name, "new_name") }?.to_owned()
        };
        s.stage.move_prim(prim(p), prim(new_parent), &name)?;
        s.paths.clear();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_rename(stage: *mut TusdStage, p: TusdPrim, new_name: *const c_char) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        s.stage
            .rename_prim(prim(p), unsafe { str_arg(new_name, "new_name") }?)?;
        s.paths.clear();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_edit_variant(
    stage: *mut TusdStage,
    p: TusdPrim,
    variant_set: *const c_char,
    variant: *const c_char,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let set = unsafe { str_arg(variant_set, "variant_set") }?;
        let variant = unsafe { str_arg(variant, "variant") }?;
        s.stage.edit_variant(prim(p), set, variant)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_variant_names(
    stage: *mut TusdStage,
    p: TusdPrim,
    variant_set: *const c_char,
    out: *mut super::TusdStringList,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let out = unsafe { out_mut(out, "out") }?;
        let sets = s.stage.variant_sets(prim(p))?;
        if variant_set.is_null() {
            out.set(sets.iter().map(|(name, _)| name.as_str()));
        } else {
            let wanted = unsafe { str_arg(variant_set, "variant_set") }?;
            let variants = sets.into_iter().find(|(name, _)| name == wanted).map(|(_, v)| v);
            out.set(variants.unwrap_or_default());
        }
        Ok(())
    })
}

#[cfg(feature = "write")]
mod physics_in {
    use std::ffi::c_char;
    use std::ptr::addr_of;

    use super::super::{
        TUSD_NO_PRIM, TusdArticulationView, TusdCollisionGroupView, TusdJointDrive, TusdJointLimit, TusdJointView,
        TusdPhysicsMaterialView, TusdPhysicsSceneView, TusdPrim, TusdRigidBodyView, TusdShapeView, core,
    };
    use super::{Fail, invalid, opt_str, slice};

    pub(super) fn opt(p: TusdPrim) -> Option<core::Prim> {
        (p != TUSD_NO_PRIM).then(|| core::Prim::from_index(p))
    }

    pub(super) unsafe fn prims(ptr: *const TusdPrim, count: usize, what: &str) -> Result<Vec<core::Prim>, Fail> {
        Ok(unsafe { slice(ptr, count, what) }?
            .iter()
            .map(|&p| core::Prim::from_index(p))
            .collect())
    }

    unsafe fn text(ptr: *const c_char, what: &str) -> Result<String, Fail> {
        Ok(unsafe { opt_str(ptr, what) }?.to_owned())
    }

    unsafe fn raw<T>(field: *const T) -> u32 {
        unsafe { field.cast::<u32>().read_unaligned() }
    }

    fn axis(raw: u32) -> Result<core::Axis, Fail> {
        Ok(match raw {
            0 => core::Axis::X,
            1 => core::Axis::Y,
            2 => core::Axis::Z,
            other => return Err(invalid(&format!("{other} is not a TusdAxis"))),
        })
    }

    fn dof(raw: u32) -> Result<core::Dof, Fail> {
        use core::Dof as D;
        [D::Distance, D::TransX, D::TransY, D::TransZ, D::RotX, D::RotY, D::RotZ]
            .get(raw as usize)
            .copied()
            .ok_or_else(|| invalid(&format!("{raw} is not a TusdDof")))
    }

    pub(super) unsafe fn scene(v: *const TusdPhysicsSceneView) -> Result<core::PhysicsScene, Fail> {
        let v = unsafe { v.as_ref() }.ok_or_else(|| invalid("scene must not be null"))?;
        Ok(core::PhysicsScene {
            prim: core::Prim::from_index(v.prim),
            gravity_direction: v.gravity_direction,
            gravity_magnitude: v.gravity_magnitude,
        })
    }

    pub(super) unsafe fn group(v: *const TusdCollisionGroupView) -> Result<core::CollisionGroup, Fail> {
        let v = unsafe { v.as_ref() }.ok_or_else(|| invalid("group must not be null"))?;
        Ok(core::CollisionGroup {
            prim: core::Prim::from_index(v.prim),
            invert_filtered_groups: v.invert_filtered_groups,
            filtered_groups: unsafe { prims(v.filtered_groups, v.filtered_group_count, "filtered_groups") }?,
            merge_group_name: unsafe { text(v.merge_group_name, "merge_group_name") }?,
            merged_groups: Vec::new(),
        })
    }

    pub(super) unsafe fn material(v: *const TusdPhysicsMaterialView) -> Result<core::PhysicsMaterial, Fail> {
        let v = unsafe { v.as_ref() }.ok_or_else(|| invalid("material must not be null"))?;
        Ok(core::PhysicsMaterial {
            prim: core::Prim::from_index(v.prim),
            static_friction: v.static_friction,
            dynamic_friction: v.dynamic_friction,
            restitution: v.restitution,
            density: v.density,
        })
    }

    pub(super) unsafe fn shape(v: *const TusdShapeView) -> Result<core::Shape, Fail> {
        if v.is_null() {
            return Err(invalid("shape must not be null"));
        }
        use core::ShapeKind as K;
        let kind = [
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
        ]
        .get(unsafe { raw(addr_of!((*v).kind)) } as usize)
        .copied()
        .ok_or_else(|| invalid("shape.kind is not a TusdShapeKind"))?;
        let shape_axis = axis(unsafe { raw(addr_of!((*v).axis)) })?;
        let v = unsafe { &*v };
        let points = unsafe { slice(v.sphere_points, v.sphere_point_count * 4, "sphere_points") }?;
        Ok(core::Shape {
            prim: core::Prim::from_index(v.prim),
            rigid_body: opt(v.rigid_body),
            local_pos: v.local_pos,
            local_rot: v.local_rot,
            local_scale: v.local_scale,
            materials: unsafe { prims(v.materials, v.material_count, "materials") }?,
            simulation_owners: unsafe { prims(v.simulation_owners, v.simulation_owner_count, "simulation_owners") }?,
            filtered_collisions: unsafe {
                prims(v.filtered_collisions, v.filtered_collision_count, "filtered_collisions")
            }?,
            collision_groups: unsafe { prims(v.collision_groups, v.collision_group_count, "collision_groups") }?,
            collision_enabled: v.collision_enabled,
            kind,
            radius: v.radius,
            top_radius: v.top_radius,
            bottom_radius: v.bottom_radius,
            half_height: v.half_height,
            half_extents: v.half_extents,
            axis: shape_axis,
            mesh_approximation: unsafe { text(v.mesh_approximation, "mesh_approximation") }?,
            mesh_scale: v.mesh_scale,
            double_sided: v.double_sided,
            sphere_points: points.as_chunks::<4>().0.to_vec(),
            custom_type: unsafe { text(v.custom_type, "custom_type") }?,
        })
    }

    pub(super) unsafe fn body(v: *const TusdRigidBodyView) -> Result<core::RigidBody, Fail> {
        let v = unsafe { v.as_ref() }.ok_or_else(|| invalid("body must not be null"))?;
        Ok(core::RigidBody {
            prim: core::Prim::from_index(v.prim),
            collisions: Vec::new(),
            filtered_collisions: unsafe {
                prims(v.filtered_collisions, v.filtered_collision_count, "filtered_collisions")
            }?,
            simulation_owners: unsafe { prims(v.simulation_owners, v.simulation_owner_count, "simulation_owners") }?,
            position: v.position,
            rotation: v.rotation,
            scale: v.scale,
            enabled: v.enabled,
            kinematic: v.kinematic,
            starts_asleep: v.starts_asleep,
            linear_velocity: v.linear_velocity,
            angular_velocity: v.angular_velocity,
        })
    }

    pub(super) unsafe fn joint(v: *const TusdJointView) -> Result<core::Joint, Fail> {
        if v.is_null() {
            return Err(invalid("joint must not be null"));
        }
        use core::JointKind as K;
        let kind = [
            K::Fixed,
            K::Revolute,
            K::Prismatic,
            K::Spherical,
            K::Distance,
            K::D6,
            K::Custom,
        ]
        .get(unsafe { raw(addr_of!((*v).kind)) } as usize)
        .copied()
        .ok_or_else(|| invalid("joint.kind is not a TusdJointKind"))?;
        let joint_axis = axis(unsafe { raw(addr_of!((*v).axis)) })?;
        let v = unsafe { &*v };
        let limits = unsafe { slice::<TusdJointLimit>(v.limits, v.limit_count, "limits") }?
            .iter()
            .map(|l| {
                Ok(core::JointLimit {
                    dof: dof(unsafe { raw(addr_of!(l.dof)) })?,
                    enabled: l.enabled,
                    lower: l.lower,
                    upper: l.upper,
                })
            })
            .collect::<Result<_, Fail>>()?;
        let drives = unsafe { slice::<TusdJointDrive>(v.drives, v.drive_count, "drives") }?
            .iter()
            .map(|d| {
                Ok(core::JointDrive {
                    dof: dof(unsafe { raw(addr_of!(d.dof)) })?,
                    target_position: d.target_position,
                    target_velocity: d.target_velocity,
                    force_limit: d.force_limit,
                    stiffness: d.stiffness,
                    damping: d.damping,
                    acceleration: d.acceleration,
                })
            })
            .collect::<Result<_, Fail>>()?;
        Ok(core::Joint {
            prim: core::Prim::from_index(v.prim),
            kind,
            type_name: unsafe { text(v.type_name, "type_name") }?,
            rel0: opt(v.rel0),
            rel1: opt(v.rel1),
            body0: opt(v.body0),
            body1: opt(v.body1),
            local_pos0: v.local_pos0,
            local_rot0: v.local_rot0,
            local_pos1: v.local_pos1,
            local_rot1: v.local_rot1,
            enabled: v.enabled,
            collision_enabled: v.collision_enabled,
            break_force: v.break_force,
            break_torque: v.break_torque,
            exclude_from_articulation: v.exclude_from_articulation,
            axis: joint_axis,
            limits,
            drives,
            min_distance_enabled: v.min_distance_enabled,
            max_distance_enabled: v.max_distance_enabled,
        })
    }

    pub(super) unsafe fn articulation(v: *const TusdArticulationView) -> Result<core::Articulation, Fail> {
        let v = unsafe { v.as_ref() }.ok_or_else(|| invalid("articulation must not be null"))?;
        Ok(core::Articulation {
            prim: core::Prim::from_index(v.prim),
            filtered_collisions: unsafe {
                prims(v.filtered_collisions, v.filtered_collision_count, "filtered_collisions")
            }?,
            ..core::Articulation::new(core::Prim::from_index(v.prim))
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_write_physics(
    stage: *mut TusdStage,
    physics: *const super::TusdPhysics,
    poses: bool,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let p = unsafe { physics.as_ref() }.ok_or_else(|| invalid("physics must not be null"))?;
        s.stage
            .write_physics(&p.physics, &core::PhysicsWriteOptions { poses })?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_write_physics_scene(
    stage: *mut TusdStage,
    view: *const super::TusdPhysicsSceneView,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let item = unsafe { physics_in::scene(view) }?;
        s.stage.write_physics_scene(&item)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_write_collision_group(
    stage: *mut TusdStage,
    view: *const super::TusdCollisionGroupView,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let item = unsafe { physics_in::group(view) }?;
        s.stage.write_collision_group(&item)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_write_physics_material(
    stage: *mut TusdStage,
    view: *const super::TusdPhysicsMaterialView,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let item = unsafe { physics_in::material(view) }?;
        s.stage.write_physics_material(&item)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_write_shape(
    stage: *mut TusdStage,
    view: *const super::TusdShapeView,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let item = unsafe { physics_in::shape(view) }?;
        s.stage.write_shape(&item)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_write_joint(
    stage: *mut TusdStage,
    view: *const super::TusdJointView,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let item = unsafe { physics_in::joint(view) }?;
        s.stage.write_joint(&item)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_write_articulation(
    stage: *mut TusdStage,
    view: *const super::TusdArticulationView,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let item = unsafe { physics_in::articulation(view) }?;
        s.stage.write_articulation(&item)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_stage_write_rigid_body(
    stage: *mut TusdStage,
    view: *const super::TusdRigidBodyView,
    pose: bool,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let body = unsafe { physics_in::body(view) }?;
        s.stage.write_rigid_body(&body, pose)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_prim_write_body_mass(
    stage: *mut TusdStage,
    p: TusdPrim,
    mass: *const super::TusdBodyMass,
) -> TusdStatus {
    write_api!({
        let s = unsafe { stage_mut(stage) }?;
        let m = unsafe { mass.as_ref() }.ok_or_else(|| invalid("mass must not be null"))?;
        s.stage.write_body_mass(
            prim(p),
            &core::BodyMass {
                mass: m.mass,
                diagonal_inertia: m.diagonal_inertia,
                center_of_mass: m.center_of_mass,
                principal_axes: m.principal_axes,
            },
        )?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_set_units(
    physics: *mut super::TusdPhysics,
    meters_per_unit: f64,
    kilograms_per_unit: f64,
    si_units: bool,
) -> TusdStatus {
    write_api!({
        let p = unsafe { out_mut(physics, "physics") }?;
        p.physics.meters_per_unit = meters_per_unit;
        p.physics.kilograms_per_unit = kilograms_per_unit;
        p.physics.si_units = si_units;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_add_scene(
    physics: *mut super::TusdPhysics,
    view: *const super::TusdPhysicsSceneView,
) -> TusdStatus {
    write_api!({
        let p = unsafe { out_mut(physics, "physics") }?;
        let item = unsafe { physics_in::scene(view) }?;
        p.physics.scenes.push(item);
        p.rebuild();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_add_collision_group(
    physics: *mut super::TusdPhysics,
    view: *const super::TusdCollisionGroupView,
) -> TusdStatus {
    write_api!({
        let p = unsafe { out_mut(physics, "physics") }?;
        let item = unsafe { physics_in::group(view) }?;
        p.physics.collision_groups.push(item);
        p.rebuild();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_add_material(
    physics: *mut super::TusdPhysics,
    view: *const super::TusdPhysicsMaterialView,
) -> TusdStatus {
    write_api!({
        let p = unsafe { out_mut(physics, "physics") }?;
        let item = unsafe { physics_in::material(view) }?;
        p.physics.materials.push(item);
        p.rebuild();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_add_shape(
    physics: *mut super::TusdPhysics,
    view: *const super::TusdShapeView,
) -> TusdStatus {
    write_api!({
        let p = unsafe { out_mut(physics, "physics") }?;
        let item = unsafe { physics_in::shape(view) }?;
        p.physics.shapes.push(item);
        p.rebuild();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_add_rigid_body(
    physics: *mut super::TusdPhysics,
    view: *const super::TusdRigidBodyView,
) -> TusdStatus {
    write_api!({
        let p = unsafe { out_mut(physics, "physics") }?;
        let item = unsafe { physics_in::body(view) }?;
        p.physics.rigid_bodies.push(item);
        p.rebuild();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_add_joint(
    physics: *mut super::TusdPhysics,
    view: *const super::TusdJointView,
) -> TusdStatus {
    write_api!({
        let p = unsafe { out_mut(physics, "physics") }?;
        let item = unsafe { physics_in::joint(view) }?;
        p.physics.joints.push(item);
        p.rebuild();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tusd_physics_add_articulation(
    physics: *mut super::TusdPhysics,
    view: *const super::TusdArticulationView,
) -> TusdStatus {
    write_api!({
        let p = unsafe { out_mut(physics, "physics") }?;
        let item = unsafe { physics_in::articulation(view) }?;
        p.physics.articulations.push(item);
        p.rebuild();
        Ok(())
    })
}
