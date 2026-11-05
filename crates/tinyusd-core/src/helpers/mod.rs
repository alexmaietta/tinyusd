mod instancer;
mod material;
mod primvar;
mod skel;
mod xform;

pub use instancer::Instances;
pub use material::BoundMaterial;
pub use primvar::{Interpolation, PrimvarInfo};
pub use skel::{BlendShapes, SkelBinding, SkeletonPose, SkinBinding, SkinningMethod, SubShape};

#[derive(Default)]
pub(crate) struct Caches {
    pub(crate) xforms: xform::XformCaches,
    #[cfg(feature = "shade")]
    pub(crate) materials: material::MaterialCaches,
    #[cfg(feature = "skel")]
    pub(crate) skel: skel::SkelCaches,
}
