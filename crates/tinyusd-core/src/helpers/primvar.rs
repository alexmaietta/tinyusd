use openusd::usd;
use openusd::vt;
use openusd_schemas::geom;

use crate::stage::usd_err;
use crate::{Prim, Result, Stage, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum Interpolation {
    #[default]
    Constant = 0,
    Uniform = 1,
    Varying = 2,
    Vertex = 3,
    FaceVarying = 4,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimvarInfo {
    pub name: String,
    pub type_name: String,
    pub interpolation: Interpolation,
    pub element_size: i32,
    pub indexed: bool,
    pub source: Prim,
}

fn interpolation(i: geom::Interpolation) -> Interpolation {
    match i {
        geom::Interpolation::Constant => Interpolation::Constant,
        geom::Interpolation::Uniform => Interpolation::Uniform,
        geom::Interpolation::Varying => Interpolation::Varying,
        geom::Interpolation::Vertex => Interpolation::Vertex,
        geom::Interpolation::FaceVarying => Interpolation::FaceVarying,
    }
}

impl Stage {
    pub fn primvar_names(&self, prim: Prim) -> Result<Vec<String>> {
        let api = geom::PrimvarsAPI::from_prim_unchecked(self.usd_prim(prim)?);
        let primvars = api.primvars_with_inheritance().map_err(usd_err)?;
        Ok(primvars.iter().map(|p| p.primvar_name().to_owned()).collect())
    }

    pub fn primvar(
        &mut self,
        prim: Prim,
        name: &str,
        time: Option<f64>,
        out: &mut Value,
    ) -> Result<Option<PrimvarInfo>> {
        let api = geom::PrimvarsAPI::from_prim_unchecked(self.usd_prim(prim)?);
        let Some(primvar) = api.primvar_with_inheritance(name).map_err(usd_err)? else {
            out.assign(vt::Value::None);
            return Ok(None);
        };
        let attr = primvar.attribute();
        let type_name = attr
            .type_name()
            .map_err(usd_err)?
            .map(|t| t.as_str().to_owned())
            .unwrap_or_default();
        out.set_type_name(&type_name);
        let value = primvar
            .flattened::<vt::Value>(time.map(usd::TimeCode::new))
            .map_err(usd_err)?;
        if !out.assign(value.unwrap_or(vt::Value::None)) {
            return Ok(None);
        }
        let source_path = attr.prim().path().clone();
        Ok(Some(PrimvarInfo {
            name: primvar.primvar_name().to_owned(),
            type_name,
            interpolation: interpolation(primvar.interpolation().map_err(usd_err)?),
            element_size: primvar.element_size().map_err(usd_err)?,
            indexed: primvar.is_indexed().map_err(usd_err)?,
            source: self.intern(&source_path),
        }))
    }
}
