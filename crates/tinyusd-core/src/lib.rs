#[cfg(feature = "write")]
mod author;
mod helpers;
mod host;
mod schemas;
mod stage;
mod value;

#[cfg(feature = "write")]
pub use author::LayerFormat;
#[cfg(feature = "write")]
pub use helpers::PhysicsWriteOptions;
pub use helpers::{
    Articulation, Axis, BlendShapes, BodyMass, BoundMaterial, CollisionGroup, Dof, Instances, Interpolation, Joint,
    JointDrive, JointKind, JointLimit, MassInformation, Physics, PhysicsMaterial, PhysicsOptions, PhysicsScene,
    PrimvarInfo, RigidBody, Shape, ShapeKind, SkelBinding, SkeletonPose, SkinBinding, SkinningMethod, SubShape,
};
pub use host::AssetResolver;
pub use stage::{AttributeInfo, OpenOptions, Prim, PrimFlags, Specifier, Stage, TraverseFlags};
pub use value::{BaseType, Value};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to open stage '{path}': {message}")]
    Open { path: String, message: String },
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("invalid prim handle")]
    InvalidPrim,
    #[error("failed to read asset '{path}': {message}")]
    Asset { path: String, message: String },
    #[error("{0}")]
    Usd(String),
    #[error("failed to load schema '{path}': {message}")]
    Schema { path: String, message: String },
    #[error("tinyusd was built without the '{0}' feature")]
    FeatureDisabled(&'static str),
    #[error("invalid value: {0}")]
    InvalidValue(String),
    #[error("failed to write '{path}': {message}")]
    Write { path: String, message: String },
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Feature {
    Geom = 1 << 0,
    Shade = 1 << 1,
    Lux = 1 << 2,
    Skel = 1 << 3,
    Physics = 1 << 4,
    Vol = 1 << 5,
    Media = 1 << 6,
    Semantics = 1 << 7,
    Write = 1 << 8,
}

pub const fn features() -> u32 {
    let mut bits = Feature::Geom as u32;
    if cfg!(feature = "shade") {
        bits |= Feature::Shade as u32;
    }
    if cfg!(feature = "lux") {
        bits |= Feature::Lux as u32;
    }
    if cfg!(feature = "skel") {
        bits |= Feature::Skel as u32;
    }
    if cfg!(feature = "physics") {
        bits |= Feature::Physics as u32;
    }
    if cfg!(feature = "vol") {
        bits |= Feature::Vol as u32;
    }
    if cfg!(feature = "media") {
        bits |= Feature::Media as u32;
    }
    if cfg!(feature = "semantics") {
        bits |= Feature::Semantics as u32;
    }
    if cfg!(feature = "write") {
        bits |= Feature::Write as u32;
    }
    bits
}

pub fn has_feature(feature: Feature) -> bool {
    features() & feature as u32 != 0
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
