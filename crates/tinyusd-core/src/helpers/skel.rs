#[cfg(feature = "skel")]
use std::cell::RefCell;
#[cfg(feature = "skel")]
use std::collections::{HashMap, HashSet};

#[cfg(feature = "skel")]
use openusd::usd::{self, SchemaBase};
#[cfg(feature = "skel")]
use openusd::{gf, sdf};
#[cfg(feature = "skel")]
use openusd_schemas::skel;

#[cfg(feature = "skel")]
use crate::stage::usd_err;
use crate::{Prim, Result, Stage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkelBinding {
    pub skeleton: Prim,
    pub animation: Option<Prim>,
    pub skinned: Vec<Prim>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SkeletonPose {
    pub joints: Vec<String>,
    pub parents: Vec<i32>,
    pub bind: Vec<[f64; 16]>,
    pub rest_local: Vec<[f64; 16]>,
    pub local: Vec<[f64; 16]>,
    pub skel: Vec<[f64; 16]>,
    pub skinning: Vec<[f64; 16]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum SkinningMethod {
    #[default]
    ClassicLinear = 0,
    DualQuaternion = 1,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkinBinding {
    pub skeleton: Prim,
    pub joint_indices: Vec<i32>,
    pub joint_weights: Vec<f32>,
    pub influences_per_point: u32,
    pub rigid: bool,
    pub geom_bind_transform: [f64; 16],
    pub skinning_method: SkinningMethod,
    pub blend_shapes: Vec<String>,
    pub blend_shape_weights: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SubShape {
    pub blend_shape: u32,
    pub weight: f32,
    pub point_offsets: Vec<[f32; 3]>,
    pub normal_offsets: Vec<[f32; 3]>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BlendShapes {
    pub point_indices: Vec<Vec<i32>>,
    pub sub_shapes: Vec<SubShape>,
}

#[cfg(feature = "skel")]
mod enabled {
    use super::*;

    type RootBinding = (sdf::Path, Vec<sdf::Path>);

    #[derive(Default)]
    pub(crate) struct SkelCaches {
        cache: RefCell<skel::Cache>,
        populated: RefCell<HashSet<sdf::Path>>,
        bindings: RefCell<HashMap<sdf::Path, Vec<RootBinding>>>,
        skeleton_of: RefCell<HashMap<sdf::Path, sdf::Path>>,
    }

    const PREDICATE: usd::PrimPredicate = usd::PrimPredicate::DEFAULT_PROXIES;

    fn vec3(v: &[gf::Vec3f]) -> Vec<[f32; 3]> {
        v.iter().map(|p| [p.x, p.y, p.z]).collect()
    }

    fn mats(v: Option<&[gf::Mat4d]>) -> Vec<[f64; 16]> {
        v.map(|m| m.iter().map(|x| x.0).collect()).unwrap_or_default()
    }

    impl Stage {
        fn skel_root_bindings(&mut self, root: &usd::Prim) -> Result<Vec<RootBinding>> {
            let caches = &self.caches.skel;
            if let Some(found) = caches.bindings.borrow().get(root.path()) {
                return Ok(found.clone());
            }
            let Some(skel_root) = skel::Root::from_prim(root.clone()).map_err(usd_err)? else {
                return Ok(Vec::new());
            };
            let mut cache = caches.cache.borrow_mut();
            if caches.populated.borrow_mut().insert(root.path().clone()) {
                cache.populate(&skel_root, PREDICATE).map_err(usd_err)?;
            }
            let mut out = Vec::new();
            for binding in cache.skel_bindings(&skel_root, PREDICATE).map_err(usd_err)? {
                let skeleton = binding.skeleton().prim().path().clone();
                let targets: Vec<sdf::Path> = binding
                    .skinning_targets()
                    .iter()
                    .map(|q| q.prim().path().clone())
                    .collect();
                let mut skeleton_of = caches.skeleton_of.borrow_mut();
                for target in &targets {
                    skeleton_of.insert(target.clone(), skeleton.clone());
                }
                out.push((skeleton, targets));
            }
            caches.bindings.borrow_mut().insert(root.path().clone(), out.clone());
            Ok(out)
        }

        fn skeleton_query(&self, skeleton: &usd::Prim) -> Result<Option<skel::SkeletonQuery>> {
            let Some(schema) = skel::Skeleton::from_prim(skeleton.clone()).map_err(usd_err)? else {
                return Ok(None);
            };
            self.caches.skel.cache.borrow_mut().skel_query(&schema).map_err(usd_err)
        }

        pub fn skel_bindings(&mut self, root: Prim) -> Result<Vec<SkelBinding>> {
            let flags = crate::TraverseFlags::INSTANCE_PROXIES;
            let mut roots = Vec::new();
            for p in self.traverse_from(root, flags)? {
                if self.is_a(p, "SkelRoot")? {
                    roots.push(p);
                }
            }
            let mut out = Vec::new();
            for r in roots {
                let usd_root = self.usd_prim(r)?;
                for (skeleton_path, targets) in self.skel_root_bindings(&usd_root)? {
                    let skeleton_prim = self.inner.prim(skeleton_path.clone()).map_err(usd_err)?;
                    let animation = match self.skeleton_query(&skeleton_prim)? {
                        Some(q) => q.anim_query().map(|a| a.animation().prim().path().clone()),
                        None => None,
                    };
                    out.push(SkelBinding {
                        skeleton: self.intern(&skeleton_path),
                        animation: animation.map(|a| self.intern(&a)),
                        skinned: targets.iter().map(|t| self.intern(t)).collect(),
                    });
                }
            }
            Ok(out)
        }

        pub fn skeleton_pose(&mut self, skeleton: Prim, time: Option<f64>) -> Result<Option<SkeletonPose>> {
            let Some(q) = self.skeleton_query(&self.usd_prim(skeleton)?)? else {
                return Ok(None);
            };
            let t = time.map(usd::TimeCode::new);
            Ok(Some(SkeletonPose {
                joints: q.joint_order().to_vec(),
                parents: q.topology().parents().to_vec(),
                bind: mats(q.bind_transforms()),
                rest_local: mats(q.rest_local_transforms()),
                local: mats(q.joint_local_transforms(t).map_err(usd_err)?.as_deref()),
                skel: mats(q.joint_skel_transforms(t).map_err(usd_err)?.as_deref()),
                skinning: mats(q.skinning_transforms(t).map_err(usd_err)?.as_deref()),
            }))
        }

        fn skinning_context(&mut self, prim: Prim) -> Result<Option<(usd::Prim, sdf::Path)>> {
            let usd_prim = self.usd_prim(prim)?;
            let Some(root) = usd_prim.ancestors().find(|a| a.is_a("SkelRoot").unwrap_or(false)) else {
                return Ok(None);
            };
            self.skel_root_bindings(&root)?;
            let skeleton = self.caches.skel.skeleton_of.borrow().get(usd_prim.path()).cloned();
            Ok(skeleton.map(|s| (usd_prim, s)))
        }

        fn blend_shape_weights(
            query: &skel::SkinningQuery,
            skeleton: Option<&skel::SkeletonQuery>,
            time: Option<usd::TimeCode>,
        ) -> Result<Vec<f32>> {
            let Some(order) = query.blend_shape_order() else {
                return Ok(Vec::new());
            };
            let anim_weights = match skeleton.and_then(|sq| sq.anim_query()) {
                Some(anim) => anim.blend_shape_weights(time).map_err(usd_err)?,
                None => None,
            };
            Ok(match (anim_weights, query.blend_shape_mapper()) {
                (Some(weights), Some(mapper)) => mapper.remap(&weights, 0.0),
                _ => vec![0.0; order.len()],
            })
        }

        pub fn skin_binding(&mut self, prim: Prim, time: Option<f64>) -> Result<Option<SkinBinding>> {
            let Some((usd_prim, skeleton_path)) = self.skinning_context(prim)? else {
                return Ok(None);
            };
            let t = time.map(usd::TimeCode::new);
            let skeleton_prim = self.inner.prim(skeleton_path.clone()).map_err(usd_err)?;
            let skeleton_query = self.skeleton_query(&skeleton_prim)?;
            let cache = self.caches.skel.cache.borrow();
            let Some(query) = cache.skinning_query(&usd_prim) else {
                return Ok(None);
            };
            let (mut indices, mut weights) = query.joint_influences(t).map_err(usd_err)?.unwrap_or_default();
            if let Some(mapper) = query.joint_mapper().filter(|m| !m.is_identity()) {
                for (index, weight) in indices.iter_mut().zip(weights.iter_mut()) {
                    match usize::try_from(*index).ok().and_then(|i| mapper.source_index(i)) {
                        Some(source) => *index = source as i32,
                        None => {
                            *index = 0;
                            *weight = 0.0;
                        }
                    }
                }
            }
            let skinning_method = match query.skinning_method().map_err(usd_err)? {
                Some(skel::SkinningMethod::DualQuaternion) => SkinningMethod::DualQuaternion,
                _ => SkinningMethod::ClassicLinear,
            };
            let blend_shapes = query.blend_shape_order().map(<[String]>::to_vec).unwrap_or_default();
            let blend_shape_weights = Self::blend_shape_weights(query, skeleton_query.as_ref(), t)?;
            let binding = SkinBinding {
                skeleton: Prim::PSEUDO_ROOT,
                joint_indices: indices,
                joint_weights: weights,
                influences_per_point: query.influences_per_component() as u32,
                rigid: query.is_rigidly_deformed(),
                geom_bind_transform: query.geom_bind_transform(t).map_err(usd_err)?.0,
                skinning_method,
                blend_shapes,
                blend_shape_weights,
            };
            drop(cache);
            Ok(Some(SkinBinding {
                skeleton: self.intern(&skeleton_path),
                ..binding
            }))
        }

        pub fn blend_shapes(&mut self, prim: Prim, time: Option<f64>) -> Result<Option<BlendShapes>> {
            let Some(binding) = self.skin_binding(prim, time)? else {
                return Ok(None);
            };
            if binding.blend_shapes.is_empty() {
                return Ok(None);
            }
            let api = skel::BindingAPI::from_prim_unchecked(self.usd_prim(prim)?);
            let query = skel::BlendShapeQuery::new(&api).map_err(usd_err)?;
            let point_indices = query.blend_shape_point_indices().map_err(usd_err)?;
            let offsets = query.sub_shape_point_offsets().map_err(usd_err)?;
            let normal_offsets = query.sub_shape_normal_offsets().map_err(usd_err)?;
            let weights = query
                .flattened_sub_shape_weights(&binding.blend_shape_weights)
                .unwrap_or_default();
            let sub_shapes = (0..query.num_sub_shapes())
                .map(|i| SubShape {
                    blend_shape: query.blend_shape_index(i) as u32,
                    weight: weights.get(i).copied().unwrap_or(0.0),
                    point_offsets: offsets.get(i).map(|o| vec3(o)).unwrap_or_default(),
                    normal_offsets: normal_offsets.get(i).map(|o| vec3(o)).unwrap_or_default(),
                })
                .collect();
            Ok(Some(BlendShapes {
                point_indices,
                sub_shapes,
            }))
        }

        pub fn skinned_points(&mut self, prim: Prim, time: Option<f64>) -> Result<Option<Vec<[f32; 3]>>> {
            let Some((usd_prim, skeleton_path)) = self.skinning_context(prim)? else {
                return Ok(None);
            };
            let t = time.map(usd::TimeCode::new);
            let Some(mut points) = usd_prim
                .attribute("points")
                .get_at::<Vec<gf::Vec3f>>(t)
                .map_err(usd_err)?
            else {
                return Ok(None);
            };
            let skeleton_prim = self.inner.prim(skeleton_path).map_err(usd_err)?;
            let Some(sq) = self.skeleton_query(&skeleton_prim)? else {
                return Ok(None);
            };
            let Some(xforms) = sq.skinning_transforms(t).map_err(usd_err)? else {
                return Ok(None);
            };
            let pose = skel::Pose::from(xforms);
            let cache = self.caches.skel.cache.borrow();
            let Some(query) = cache.skinning_query(&usd_prim) else {
                return Ok(None);
            };
            if query.has_blend_shapes() {
                let weights = Self::blend_shape_weights(query, Some(&sq), t)?;
                let shapes = skel::BlendShapeQuery::new(&skel::BindingAPI::from_prim_unchecked(usd_prim.clone()))
                    .map_err(usd_err)?;
                if let Some(sub_weights) = shapes.sub_shape_weights(&weights) {
                    let indices = shapes.blend_shape_point_indices().map_err(usd_err)?;
                    let offsets = shapes.sub_shape_point_offsets().map_err(usd_err)?;
                    if let Some(deformed) =
                        skel::BlendShapeQuery::deformed_points(&sub_weights, &indices, &offsets, &points)
                    {
                        points = deformed;
                    }
                }
            }
            if query.is_rigidly_deformed() {
                let Some(m) = query.skinned_transform(&pose, t).map_err(usd_err)? else {
                    return Ok(None);
                };
                return Ok(Some(points.iter().map(|p| transform_point(&m.0, p)).collect()));
            }
            Ok(query
                .skinned_points(&pose, &points, t)
                .map_err(usd_err)?
                .map(|p| vec3(&p)))
        }
    }

    fn transform_point(m: &[f64; 16], p: &gf::Vec3f) -> [f32; 3] {
        let (x, y, z) = (f64::from(p.x), f64::from(p.y), f64::from(p.z));
        let mut out = [0.0f32; 3];
        for (c, o) in out.iter_mut().enumerate() {
            *o = (x * m[c] + y * m[4 + c] + z * m[8 + c] + m[12 + c]) as f32;
        }
        out
    }
}

#[cfg(feature = "skel")]
pub(crate) use enabled::SkelCaches;

#[cfg(not(feature = "skel"))]
impl Stage {
    pub fn skel_bindings(&mut self, _root: Prim) -> Result<Vec<SkelBinding>> {
        Err(crate::Error::FeatureDisabled("skel"))
    }

    pub fn skeleton_pose(&mut self, _skeleton: Prim, _time: Option<f64>) -> Result<Option<SkeletonPose>> {
        Err(crate::Error::FeatureDisabled("skel"))
    }

    pub fn skin_binding(&mut self, _prim: Prim, _time: Option<f64>) -> Result<Option<SkinBinding>> {
        Err(crate::Error::FeatureDisabled("skel"))
    }

    pub fn blend_shapes(&mut self, _prim: Prim, _time: Option<f64>) -> Result<Option<BlendShapes>> {
        Err(crate::Error::FeatureDisabled("skel"))
    }

    pub fn skinned_points(&mut self, _prim: Prim, _time: Option<f64>) -> Result<Option<Vec<[f32; 3]>>> {
        Err(crate::Error::FeatureDisabled("skel"))
    }
}

#[cfg(all(feature = "skel", feature = "write"))]
impl Stage {
    fn ensure_typed(&mut self, prim: Prim, type_name: &str) -> Result<()> {
        match self.type_name(prim)?.as_str() {
            t if t == type_name => Ok(()),
            "" => self.set_type_name(prim, type_name),
            other => Err(crate::Error::InvalidValue(format!(
                "{} is a {other}, not a {type_name}",
                self.path(prim)?
            ))),
        }
    }

    fn put_attr(
        &mut self,
        prim: Prim,
        name: &str,
        type_name: &str,
        value: &crate::Value,
        time: Option<f64>,
    ) -> Result<()> {
        if self.declared_type(prim, name)?.is_none() {
            self.create_attribute(prim, name, type_name, false)?;
        }
        self.set(prim, name, value, time)
    }

    /// Defines a Skeleton at `path` with its joints, as paths such as
    /// `hips/spine`, each after its parent, their world-space bind transforms
    /// and, where given, their local rest transforms.
    pub fn define_skeleton(
        &mut self,
        path: &str,
        joints: &[&str],
        bind_transforms: &[[f64; 16]],
        rest_transforms: &[[f64; 16]],
    ) -> Result<Prim> {
        use crate::{BaseType, Error, Value};
        check_joints(joints)?;
        if bind_transforms.len() != joints.len()
            || !(rest_transforms.is_empty() || rest_transforms.len() == joints.len())
        {
            return Err(Error::InvalidValue(format!(
                "{} joints need as many bind transforms (not {}) and as many rest transforms or none (not {})",
                joints.len(),
                bind_transforms.len(),
                rest_transforms.len()
            )));
        }
        let skeleton = self.define_prim(path, "Skeleton")?;
        self.set(
            skeleton,
            "joints",
            &Value::from_strings(BaseType::Token, true, joints)?,
            None,
        )?;
        let bind = Value::from_slice(BaseType::Matrix4d, true, bind_transforms)?;
        self.set(skeleton, "bindTransforms", &bind, None)?;
        if !rest_transforms.is_empty() {
            let rest = Value::from_slice(BaseType::Matrix4d, true, rest_transforms)?;
            self.set(skeleton, "restTransforms", &rest, None)?;
        }
        Ok(skeleton)
    }

    /// Writes a SkelAnimation's joints and, at `time`, their local transforms,
    /// split into translations, rotations and scales. A prim with no type
    /// becomes a SkelAnimation.
    pub fn set_skel_animation(
        &mut self,
        prim: Prim,
        joints: &[&str],
        local_transforms: &[[f64; 16]],
        time: Option<f64>,
    ) -> Result<()> {
        use crate::author::Pose;
        use crate::{BaseType, Error, Value};
        check_joints(joints)?;
        if local_transforms.len() != joints.len() {
            return Err(Error::InvalidValue(format!(
                "{} joints need as many local transforms, not {}",
                joints.len(),
                local_transforms.len()
            )));
        }
        let mut translations = Vec::with_capacity(joints.len());
        let mut rotations = Vec::with_capacity(joints.len());
        let mut scales = Vec::with_capacity(joints.len());
        for (joint, m) in joints.iter().zip(local_transforms) {
            let pose = Pose::from_matrix(m).ok_or_else(|| {
                Error::InvalidValue(format!(
                    "joint {joint}'s transform has shear or a zero scale, which an animation can't hold"
                ))
            })?;
            translations.push(pose.translate.map(|v| v as f32));
            rotations.push(pose.orient.map(|v| v as f32));
            scales.push(pose.scale.map(|v| gf::f16::from_f64(v).to_bits()));
        }
        self.ensure_typed(prim, "SkelAnimation")?;
        self.put_attr(
            prim,
            "joints",
            "token[]",
            &Value::from_strings(BaseType::Token, true, joints)?,
            None,
        )?;
        self.put_attr(
            prim,
            "translations",
            "float3[]",
            &Value::from_slice(BaseType::Float3, true, &translations)?,
            time,
        )?;
        self.put_attr(
            prim,
            "rotations",
            "quatf[]",
            &Value::from_slice(BaseType::Quatf, true, &rotations)?,
            time,
        )?;
        self.put_attr(
            prim,
            "scales",
            "half3[]",
            &Value::from_slice(BaseType::Half3, true, &scales)?,
            time,
        )
    }

    /// Binds `prim`, a mesh or the SkelRoot above several, to `skeleton`, and
    /// where given drives the skeleton with `animation`.
    pub fn bind_skeleton(&mut self, prim: Prim, skeleton: Prim, animation: Option<Prim>) -> Result<()> {
        if !self.has_api(prim, "SkelBindingAPI")? {
            self.apply_api(prim, "SkelBindingAPI")?;
        }
        let target = self.path(skeleton)?.to_owned();
        self.set_targets(prim, "skel:skeleton", &[&target])?;
        if let Some(animation) = animation {
            if !self.has_api(skeleton, "SkelBindingAPI")? {
                self.apply_api(skeleton, "SkelBindingAPI")?;
            }
            let source = self.path(animation)?.to_owned();
            self.set_targets(skeleton, "skel:animationSource", &[&source])?;
        }
        Ok(())
    }

    /// Writes how `prim`'s points follow the joints: `influences_per_point`
    /// joint indices and weights for each point, or for the whole prim when it
    /// moves rigidly with one set. `joints` reorders the skeleton's joints for
    /// these indices where given, and `geom_bind_transform` places the prim
    /// when it was bound.
    pub fn set_skinning(
        &mut self,
        prim: Prim,
        joint_indices: &[i32],
        joint_weights: &[f32],
        influences_per_point: u32,
        geom_bind_transform: Option<&[f64; 16]>,
        joints: &[&str],
    ) -> Result<()> {
        use crate::{BaseType, Error, Interpolation, Value};
        let per = influences_per_point as usize;
        if per == 0 || joint_indices.len() != joint_weights.len() || !joint_indices.len().is_multiple_of(per) {
            return Err(Error::InvalidValue(format!(
                "{} joint indices and {} weights don't make whole sets of {influences_per_point}",
                joint_indices.len(),
                joint_weights.len()
            )));
        }
        if joint_indices.iter().any(|&i| i < 0) {
            return Err(Error::InvalidValue("joint indices can't be negative".to_owned()));
        }
        let mut points = Value::new();
        let point_count = self
            .get(prim, "points", None, &mut points)
            .ok()
            .filter(|&found| found)
            .map(|_| points.len());
        let interpolation = match point_count {
            Some(n) if joint_indices.len() == n * per => Interpolation::Vertex,
            _ if joint_indices.len() == per => Interpolation::Constant,
            None => Interpolation::Vertex,
            Some(n) => {
                return Err(Error::InvalidValue(format!(
                    "{} points need {} joint indices, or {per} for a rigid bind, not {}",
                    n,
                    n * per,
                    joint_indices.len()
                )));
            }
        };
        if !self.has_api(prim, "SkelBindingAPI")? {
            self.apply_api(prim, "SkelBindingAPI")?;
        }
        let element = influences_per_point as i32;
        let indices = Value::from_slice(BaseType::Int, true, joint_indices)?;
        self.set_primvar(
            prim,
            "skel:jointIndices",
            "int[]",
            &indices,
            interpolation,
            None,
            element,
            None,
        )?;
        let weights = Value::from_slice(BaseType::Float, true, joint_weights)?;
        self.set_primvar(
            prim,
            "skel:jointWeights",
            "float[]",
            &weights,
            interpolation,
            None,
            element,
            None,
        )?;
        if let Some(m) = geom_bind_transform {
            let value = Value::from_slice(BaseType::Matrix4d, false, &[*m])?;
            self.put_attr(prim, "primvars:skel:geomBindTransform", "matrix4d", &value, None)?;
        }
        if !joints.is_empty() {
            check_joints(joints)?;
            let value = Value::from_strings(BaseType::Token, true, joints)?;
            if self.declared_type(prim, "skel:joints")?.is_none() {
                self.create_attribute(prim, "skel:joints", "token[]", true)?;
            }
            self.set(prim, "skel:joints", &value, None)?;
        }
        Ok(())
    }
}

#[cfg(all(feature = "skel", feature = "write"))]
impl Stage {
    fn check_offsets(offsets: &[[f32; 3]], normal_offsets: &[[f32; 3]], what: &str) -> Result<()> {
        if !(normal_offsets.is_empty() || normal_offsets.len() == offsets.len()) {
            return Err(crate::Error::InvalidValue(format!(
                "{what} has {} offsets, so it takes as many normal offsets or none, not {}",
                offsets.len(),
                normal_offsets.len()
            )));
        }
        Ok(())
    }

    /// Defines a BlendShape at `path`: the point `offsets` it adds at full
    /// weight, their `normal_offsets` where given, and the `point_indices`
    /// they apply to, or every point where empty.
    pub fn define_blend_shape(
        &mut self,
        path: &str,
        offsets: &[[f32; 3]],
        normal_offsets: &[[f32; 3]],
        point_indices: &[i32],
    ) -> Result<Prim> {
        use crate::{BaseType, Error, Value};
        Self::check_offsets(offsets, normal_offsets, path)?;
        if !point_indices.is_empty() {
            if point_indices.len() != offsets.len() {
                return Err(Error::InvalidValue(format!(
                    "{} offsets need as many point indices or none, not {}",
                    offsets.len(),
                    point_indices.len()
                )));
            }
            let mut seen = HashSet::new();
            if let Some(bad) = point_indices.iter().find(|&&i| i < 0 || !seen.insert(i)) {
                return Err(Error::InvalidValue(format!(
                    "point index {bad} is negative or listed twice"
                )));
            }
        }
        let shape = self.define_prim(path, "BlendShape")?;
        self.set(
            shape,
            "offsets",
            &Value::from_slice(BaseType::Float3, true, offsets)?,
            None,
        )?;
        if !normal_offsets.is_empty() {
            let value = Value::from_slice(BaseType::Float3, true, normal_offsets)?;
            self.set(shape, "normalOffsets", &value, None)?;
        }
        if !point_indices.is_empty() {
            let value = Value::from_slice(BaseType::Int, true, point_indices)?;
            self.set(shape, "pointIndices", &value, None)?;
        }
        Ok(shape)
    }

    /// Adds the inbetween shape `name` to `blend_shape`, the offsets it
    /// reaches at `weight`, between 0 and 1 or beyond, with their normal
    /// offsets where given.
    pub fn set_blend_shape_inbetween(
        &mut self,
        blend_shape: Prim,
        name: &str,
        weight: f32,
        offsets: &[[f32; 3]],
        normal_offsets: &[[f32; 3]],
    ) -> Result<()> {
        use crate::{BaseType, Error, Value};
        if name.is_empty() || name.ends_with(":normalOffsets") || !name.split(':').all(sdf::Path::is_valid_identifier) {
            return Err(Error::InvalidValue(format!("'{name}' is not an inbetween name")));
        }
        if !weight.is_finite() || weight == 0.0 || weight == 1.0 {
            return Err(Error::InvalidValue(format!(
                "an inbetween's weight can't be {weight}; 0 and 1 are the base and the full shape"
            )));
        }
        Self::check_offsets(offsets, normal_offsets, name)?;
        if self.type_name(blend_shape)? != "BlendShape" {
            return Err(Error::InvalidValue(format!(
                "{} is not a BlendShape",
                self.path(blend_shape)?
            )));
        }
        let attr = format!("inbetweens:{name}");
        if self.declared_type(blend_shape, &attr)?.is_none() {
            self.create_attribute(blend_shape, &attr, "point3f[]", true)?;
        }
        self.set(
            blend_shape,
            &attr,
            &Value::from_slice(BaseType::Float3, true, offsets)?,
            None,
        )?;
        self.set_attribute_metadata(blend_shape, &attr, "weight", openusd::vt::Value::Float(weight))?;
        if !normal_offsets.is_empty() {
            let normals = format!("{attr}:normalOffsets");
            if self.declared_type(blend_shape, &normals)?.is_none() {
                self.create_attribute(blend_shape, &normals, "vector3f[]", true)?;
            }
            self.set(
                blend_shape,
                &normals,
                &Value::from_slice(BaseType::Float3, true, normal_offsets)?,
                None,
            )?;
        }
        Ok(())
    }

    /// Binds blend shapes to `prim`: `names`, as the animation's
    /// `blendShapes` call them, each driving the BlendShape in `targets` at
    /// the same place.
    pub fn set_blend_shapes(&mut self, prim: Prim, names: &[&str], targets: &[Prim]) -> Result<()> {
        use crate::{BaseType, Error, Value};
        check_names(names)?;
        if names.len() != targets.len() {
            return Err(Error::InvalidValue(format!(
                "{} blend shape names need as many targets, not {}",
                names.len(),
                targets.len()
            )));
        }
        let paths: Vec<String> = targets
            .iter()
            .map(|&t| self.path(t).map(str::to_owned))
            .collect::<Result<_>>()?;
        if !self.has_api(prim, "SkelBindingAPI")? {
            self.apply_api(prim, "SkelBindingAPI")?;
        }
        self.set(
            prim,
            "skel:blendShapes",
            &Value::from_strings(BaseType::Token, true, names)?,
            None,
        )?;
        let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
        self.set_targets(prim, "skel:blendShapeTargets", &paths)
    }

    /// Writes a SkelAnimation's blend shape `names` and, at `time`, their
    /// weights.
    pub fn set_blend_shape_weights(
        &mut self,
        animation: Prim,
        names: &[&str],
        weights: &[f32],
        time: Option<f64>,
    ) -> Result<()> {
        use crate::{BaseType, Error, Value};
        check_names(names)?;
        if names.len() != weights.len() {
            return Err(Error::InvalidValue(format!(
                "{} blend shape names need as many weights, not {}",
                names.len(),
                weights.len()
            )));
        }
        self.ensure_typed(animation, "SkelAnimation")?;
        self.put_attr(
            animation,
            "blendShapes",
            "token[]",
            &Value::from_strings(BaseType::Token, true, names)?,
            None,
        )?;
        self.put_attr(
            animation,
            "blendShapeWeights",
            "float[]",
            &Value::from_slice(BaseType::Float, true, weights)?,
            time,
        )
    }
}

/// Blend shape names must be identifiers, each listed once.
#[cfg(all(feature = "skel", feature = "write"))]
fn check_names(names: &[&str]) -> Result<()> {
    let mut seen = HashSet::new();
    for name in names {
        if !sdf::Path::is_valid_identifier(name) || !seen.insert(*name) {
            return Err(crate::Error::InvalidValue(format!(
                "blend shape name '{name}' is not an identifier or is listed twice"
            )));
        }
    }
    Ok(())
}

/// Joint paths must be valid and unique, each after its parent.
#[cfg(all(feature = "skel", feature = "write"))]
fn check_joints(joints: &[&str]) -> Result<()> {
    let mut seen = HashSet::new();
    for joint in joints {
        let valid = !joint.is_empty() && joint.split('/').all(sdf::Path::is_valid_identifier);
        if !valid {
            return Err(crate::Error::InvalidValue(format!(
                "'{joint}' is not a joint path such as hips/spine"
            )));
        }
        if let Some((parent, _)) = joint.rsplit_once('/')
            && !seen.contains(parent)
        {
            return Err(crate::Error::InvalidValue(format!(
                "joint {joint} comes before its parent {parent}"
            )));
        }
        if !seen.insert(*joint) {
            return Err(crate::Error::InvalidValue(format!("joint {joint} is listed twice")));
        }
    }
    Ok(())
}

#[cfg(all(not(feature = "skel"), feature = "write"))]
impl Stage {
    pub fn define_skeleton(
        &mut self,
        _path: &str,
        _joints: &[&str],
        _bind_transforms: &[[f64; 16]],
        _rest_transforms: &[[f64; 16]],
    ) -> Result<Prim> {
        Err(crate::Error::FeatureDisabled("skel"))
    }

    pub fn set_skel_animation(
        &mut self,
        _prim: Prim,
        _joints: &[&str],
        _local_transforms: &[[f64; 16]],
        _time: Option<f64>,
    ) -> Result<()> {
        Err(crate::Error::FeatureDisabled("skel"))
    }

    pub fn bind_skeleton(&mut self, _prim: Prim, _skeleton: Prim, _animation: Option<Prim>) -> Result<()> {
        Err(crate::Error::FeatureDisabled("skel"))
    }

    pub fn define_blend_shape(
        &mut self,
        _path: &str,
        _offsets: &[[f32; 3]],
        _normal_offsets: &[[f32; 3]],
        _point_indices: &[i32],
    ) -> Result<Prim> {
        Err(crate::Error::FeatureDisabled("skel"))
    }

    pub fn set_blend_shape_inbetween(
        &mut self,
        _blend_shape: Prim,
        _name: &str,
        _weight: f32,
        _offsets: &[[f32; 3]],
        _normal_offsets: &[[f32; 3]],
    ) -> Result<()> {
        Err(crate::Error::FeatureDisabled("skel"))
    }

    pub fn set_blend_shapes(&mut self, _prim: Prim, _names: &[&str], _targets: &[Prim]) -> Result<()> {
        Err(crate::Error::FeatureDisabled("skel"))
    }

    pub fn set_blend_shape_weights(
        &mut self,
        _animation: Prim,
        _names: &[&str],
        _weights: &[f32],
        _time: Option<f64>,
    ) -> Result<()> {
        Err(crate::Error::FeatureDisabled("skel"))
    }

    pub fn set_skinning(
        &mut self,
        _prim: Prim,
        _joint_indices: &[i32],
        _joint_weights: &[f32],
        _influences_per_point: u32,
        _geom_bind_transform: Option<&[f64; 16]>,
        _joints: &[&str],
    ) -> Result<()> {
        Err(crate::Error::FeatureDisabled("skel"))
    }
}
