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

#[cfg(feature = "write")]
impl Stage {
    /// Writes a PointInstancer's prototypes and per-instance arrays from
    /// `instances`, the inverse of `instancer_instances` read without the
    /// prototypes' own transforms. Each transform is split into `positions`,
    /// `orientationsf` and `scales`, so it can't have shear. `ids` are written
    /// where given. A prim with no type becomes a PointInstancer.
    pub fn set_instances(&mut self, prim: Prim, instances: &Instances, time: Option<f64>) -> Result<()> {
        use crate::author::Pose;
        use crate::{BaseType, Error, Value};
        let n = instances.transforms.len();
        if instances.proto_indices.len() != n || !(instances.ids.is_empty() || instances.ids.len() == n) {
            return Err(Error::InvalidValue(format!(
                "{n} transforms need as many proto indices (not {}) and as many ids or none (not {})",
                instances.proto_indices.len(),
                instances.ids.len()
            )));
        }
        if let Some(bad) = instances
            .proto_indices
            .iter()
            .find(|&&i| i < 0 || i as usize >= instances.prototypes.len())
        {
            return Err(Error::InvalidValue(format!(
                "proto index {bad} is out of range for {} prototypes",
                instances.prototypes.len()
            )));
        }
        let mut positions = Vec::with_capacity(n);
        let mut orientations = Vec::with_capacity(n);
        let mut scales = Vec::with_capacity(n);
        for (i, m) in instances.transforms.iter().enumerate() {
            let pose = Pose::from_matrix(m).ok_or_else(|| {
                Error::InvalidValue(format!(
                    "instance {i}'s transform has shear or a zero scale, which an instancer can't hold"
                ))
            })?;
            positions.push(pose.translate.map(|v| v as f32));
            orientations.push(pose.orient.map(|v| v as f32));
            scales.push(pose.scale.map(|v| v as f32));
        }
        match self.type_name(prim)?.as_str() {
            "PointInstancer" => {}
            "" => self.set_type_name(prim, "PointInstancer")?,
            other => {
                return Err(Error::InvalidValue(format!(
                    "{} is a {other}, not a PointInstancer",
                    self.path(prim)?
                )));
            }
        }
        let targets: Vec<String> = instances
            .prototypes
            .iter()
            .map(|&p| self.path(p).map(str::to_owned))
            .collect::<Result<_>>()?;
        let targets: Vec<&str> = targets.iter().map(String::as_str).collect();
        self.set_targets(prim, "prototypes", &targets)?;
        let put = |s: &mut Self, name: &str, type_name: &str, value: Value| -> Result<()> {
            if s.declared_type(prim, name)?.is_none() {
                s.create_attribute(prim, name, type_name, false)?;
            }
            s.set(prim, name, &value, time)
        };
        put(
            self,
            "protoIndices",
            "int[]",
            Value::from_slice(BaseType::Int, true, &instances.proto_indices)?,
        )?;
        put(
            self,
            "positions",
            "point3f[]",
            Value::from_slice(BaseType::Float3, true, &positions)?,
        )?;
        put(
            self,
            "orientationsf",
            "quatf[]",
            Value::from_slice(BaseType::Quatf, true, &orientations)?,
        )?;
        put(
            self,
            "scales",
            "float3[]",
            Value::from_slice(BaseType::Float3, true, &scales)?,
        )?;
        if !instances.ids.is_empty() {
            put(
                self,
                "ids",
                "int64[]",
                Value::from_slice(BaseType::Int64, true, &instances.ids)?,
            )?;
        }
        Ok(())
    }
}
