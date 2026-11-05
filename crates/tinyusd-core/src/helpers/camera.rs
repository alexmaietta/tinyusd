use openusd::{gf, usd};
use openusd_schemas::geom;

#[cfg(feature = "lux")]
use crate::Value;
use crate::stage::usd_err;
use crate::{Prim, Result, Stage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum Projection {
    #[default]
    Perspective = 0,
    Orthographic = 1,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraInfo {
    pub transform: [f64; 16],
    pub projection: Projection,
    pub horizontal_aperture: f32,
    pub vertical_aperture: f32,
    pub horizontal_aperture_offset: f32,
    pub vertical_aperture_offset: f32,
    pub focal_length: f32,
    pub near: f32,
    pub far: f32,
    pub clipping_planes: Vec<[f32; 4]>,
    pub f_stop: f32,
    pub focus_distance: f32,
    pub exposure_scale: f32,
    pub horizontal_fov: f32,
    pub vertical_fov: f32,
    pub view_matrix: [f64; 16],
    pub projection_matrix: [f64; 16],
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LightInfo {
    pub type_name: String,
    pub color: [f32; 3],
    pub intensity: f32,
    pub exposure: f32,
    pub normalize: bool,
    pub color_temperature: Option<f32>,
    pub radiance: [f32; 3],
    pub radius: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub length: Option<f32>,
    pub angle: Option<f32>,
    pub texture: Option<String>,
    pub cone_angle: Option<f32>,
    pub cone_softness: Option<f32>,
    pub focus: Option<f32>,
}

impl Stage {
    pub fn camera(&self, prim: Prim, time: Option<f64>) -> Result<Option<CameraInfo>> {
        let Some(camera) = geom::Camera::from_prim(self.usd_prim(prim)?).map_err(usd_err)? else {
            return Ok(None);
        };
        let t = time.map(usd::TimeCode::new);
        let c = camera.camera_at(t).map_err(usd_err)?;
        let frustum = c.frustum();
        Ok(Some(CameraInfo {
            transform: c.transform.0,
            projection: match c.projection {
                gf::Projection::Orthographic => Projection::Orthographic,
                _ => Projection::Perspective,
            },
            horizontal_aperture: c.horizontal_aperture,
            vertical_aperture: c.vertical_aperture,
            horizontal_aperture_offset: c.horizontal_aperture_offset,
            vertical_aperture_offset: c.vertical_aperture_offset,
            focal_length: c.focal_length,
            near: c.clipping_range.min(),
            far: c.clipping_range.max(),
            clipping_planes: c.clipping_planes.iter().map(|p| [p.x, p.y, p.z, p.w]).collect(),
            f_stop: c.f_stop,
            focus_distance: c.focus_distance,
            exposure_scale: camera.linear_exposure_scale(t).map_err(usd_err)?,
            horizontal_fov: c.field_of_view(gf::FovDirection::Horizontal),
            vertical_fov: c.field_of_view(gf::FovDirection::Vertical),
            view_matrix: frustum.view_matrix().0,
            projection_matrix: frustum.projection_matrix().0,
        }))
    }

    #[cfg(feature = "lux")]
    fn input_f32(&self, prim: Prim, name: &str, time: Option<f64>, declared: bool) -> Result<Option<f32>> {
        if declared && self.attribute_info(prim, name)?.is_none() {
            return Ok(None);
        }
        let mut v = Value::new();
        if !self.get(prim, name, time, &mut v)? {
            return Ok(None);
        }
        Ok(v.as_scalar::<f32>())
    }

    #[cfg(feature = "lux")]
    pub fn light(&self, prim: Prim, time: Option<f64>) -> Result<Option<LightInfo>> {
        if !self.has_api(prim, "LightAPI")? {
            return Ok(None);
        }
        let mut v = Value::new();
        let color = if self.get(prim, "inputs:color", time, &mut v)? {
            v.as_scalar::<[f32; 3]>().unwrap_or([1.0; 3])
        } else {
            [1.0; 3]
        };
        let flag = |name: &str, v: &mut Value| -> Result<bool> {
            Ok(self.get(prim, name, time, v)? && v.as_scalar::<u8>() == Some(1))
        };
        let intensity = self.input_f32(prim, "inputs:intensity", time, false)?.unwrap_or(1.0);
        let exposure = self.input_f32(prim, "inputs:exposure", time, false)?.unwrap_or(0.0);
        let normalize = flag("inputs:normalize", &mut v)?;
        let color_temperature = if flag("inputs:enableColorTemperature", &mut v)? {
            Some(
                self.input_f32(prim, "inputs:colorTemperature", time, false)?
                    .unwrap_or(6500.0),
            )
        } else {
            None
        };
        let tint = color_temperature.map_or([1.0; 3], |k| {
            let rgb = openusd_schemas::lux::blackbody_rgb(k);
            [rgb.x, rgb.y, rgb.z]
        });
        let scale = intensity * exposure.exp2();
        let texture = if self.attribute_info(prim, "inputs:texture:file")?.is_some()
            && self.get(prim, "inputs:texture:file", time, &mut v)?
        {
            v.resolved_paths()
                .first()
                .filter(|p| !p.is_empty())
                .cloned()
                .or_else(|| v.strings().first().cloned())
        } else {
            None
        };
        Ok(Some(LightInfo {
            type_name: self.type_name(prim)?,
            color,
            intensity,
            exposure,
            normalize,
            color_temperature,
            radiance: std::array::from_fn(|i| color[i] * tint[i] * scale),
            radius: self.input_f32(prim, "inputs:radius", time, true)?,
            width: self.input_f32(prim, "inputs:width", time, true)?,
            height: self.input_f32(prim, "inputs:height", time, true)?,
            length: self.input_f32(prim, "inputs:length", time, true)?,
            angle: self.input_f32(prim, "inputs:angle", time, true)?,
            texture,
            cone_angle: self.input_f32(prim, "inputs:shaping:cone:angle", time, true)?,
            cone_softness: self.input_f32(prim, "inputs:shaping:cone:softness", time, true)?,
            focus: self.input_f32(prim, "inputs:shaping:focus", time, true)?,
        }))
    }

    #[cfg(not(feature = "lux"))]
    pub fn light(&self, _prim: Prim, _time: Option<f64>) -> Result<Option<LightInfo>> {
        Err(crate::Error::FeatureDisabled("lux"))
    }
}
