use openusd::usd;
use openusd_schemas::geom::{self, PointInstancerSchema};

use crate::stage::usd_err;
use crate::{Prim, Result, Stage};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Instances {
    pub prototypes: Vec<Prim>,
    pub proto_indices: Vec<i32>,
    pub ids: Vec<i64>,
    pub transforms: Vec<[f64; 16]>,
}

impl Stage {
    pub fn instancer_instances(
        &mut self,
        prim: Prim,
        time: Option<f64>,
        include_prototype_transform: bool,
        apply_visibility: bool,
    ) -> Result<Option<Instances>> {
        let Some(instancer) = geom::PointInstancer::from_prim(self.usd_prim(prim)?).map_err(usd_err)? else {
            return Ok(None);
        };
        let t = time.map(usd::TimeCode::new);
        let inclusion = if include_prototype_transform {
            geom::ProtoXformInclusion::Include
        } else {
            geom::ProtoXformInclusion::Exclude
        };
        let mask_mode = if apply_visibility {
            geom::MaskApplication::Apply
        } else {
            geom::MaskApplication::Ignore
        };
        let prototype_paths = instancer.prototypes_rel().targets().map_err(usd_err)?;
        let prototypes = prototype_paths.iter().map(|p| self.intern(p)).collect();
        if instancer.positions_at(t).map_err(usd_err)?.is_none() {
            return Ok(Some(Instances {
                prototypes,
                ..Instances::default()
            }));
        }
        let transforms = instancer
            .instance_transforms_at(t, t, inclusion, mask_mode)
            .map_err(usd_err)?;
        if transforms.is_empty() {
            return Ok(Some(Instances {
                prototypes,
                ..Instances::default()
            }));
        }
        let mut proto_indices = instancer.proto_indices_at(t).map_err(usd_err)?.unwrap_or_default();
        let mut ids = match instancer.ids_at(t).map_err(usd_err)? {
            Some(ids) => ids,
            None => (0..proto_indices.len() as i64).collect(),
        };
        if apply_visibility {
            let mask = instancer.mask_at(t, None).map_err(usd_err)?;
            geom::PointInstancer::apply_mask(&mask, &mut proto_indices, 1).map_err(usd_err)?;
            geom::PointInstancer::apply_mask(&mask, &mut ids, 1).map_err(usd_err)?;
        }
        if proto_indices.len() != transforms.len() || ids.len() != transforms.len() {
            return Err(crate::Error::Usd(format!(
                "point instancer arrays disagree: {} transforms, {} protoIndices, {} ids",
                transforms.len(),
                proto_indices.len(),
                ids.len()
            )));
        }
        Ok(Some(Instances {
            prototypes,
            proto_indices,
            ids,
            transforms: transforms.into_iter().map(|m| m.0).collect(),
        }))
    }
}
