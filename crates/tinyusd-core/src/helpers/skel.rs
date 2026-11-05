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
