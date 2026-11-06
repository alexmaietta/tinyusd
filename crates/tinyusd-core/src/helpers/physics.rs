use crate::{Prim, Result, Stage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum Axis {
    X = 0,
    Y = 1,
    #[default]
    Z = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum ShapeKind {
    #[default]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum JointKind {
    #[default]
    Fixed = 0,
    Revolute = 1,
    Prismatic = 2,
    Spherical = 3,
    Distance = 4,
    D6 = 5,
    Custom = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum Dof {
    #[default]
    Distance = 0,
    TransX = 1,
    TransY = 2,
    TransZ = 3,
    RotX = 4,
    RotY = 5,
    RotZ = 6,
}

impl Dof {
    pub const fn is_linear(self) -> bool {
        matches!(self, Dof::Distance | Dof::TransX | Dof::TransY | Dof::TransZ)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PhysicsScene {
    pub prim: Prim,
    pub gravity_direction: [f32; 3],
    pub gravity_magnitude: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CollisionGroup {
    pub prim: Prim,
    pub invert_filtered_groups: bool,
    pub filtered_groups: Vec<Prim>,
    pub merge_group_name: String,
    pub merged_groups: Vec<Prim>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PhysicsMaterial {
    pub prim: Prim,
    pub static_friction: f32,
    pub dynamic_friction: f32,
    pub restitution: f32,
    pub density: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Shape {
    pub prim: Prim,
    pub rigid_body: Option<Prim>,
    pub local_pos: [f32; 3],
    pub local_rot: [f32; 4],
    pub local_scale: [f32; 3],
    pub materials: Vec<Prim>,
    pub simulation_owners: Vec<Prim>,
    pub filtered_collisions: Vec<Prim>,
    pub collision_groups: Vec<Prim>,
    pub collision_enabled: bool,
    pub kind: ShapeKind,
    pub radius: f32,
    pub top_radius: f32,
    pub bottom_radius: f32,
    pub half_height: f32,
    pub half_extents: [f32; 3],
    pub axis: Axis,
    pub mesh_approximation: String,
    pub mesh_scale: [f32; 3],
    pub double_sided: bool,
    pub sphere_points: Vec<[f32; 4]>,
    pub custom_type: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassInformation {
    pub volume: f64,
    pub inertia: [f64; 9],
    pub center_of_mass: [f64; 3],
    pub local_pos: [f64; 3],
    pub local_rot: [f64; 4],
}

impl Default for MassInformation {
    fn default() -> Self {
        Self {
            volume: -1.0,
            inertia: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            center_of_mass: [0.0; 3],
            local_pos: [0.0; 3],
            local_rot: [1.0, 0.0, 0.0, 0.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BodyMass {
    pub mass: f64,
    pub diagonal_inertia: [f64; 3],
    pub center_of_mass: [f64; 3],
    pub principal_axes: [f64; 4],
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RigidBody {
    pub prim: Prim,
    pub collisions: Vec<Prim>,
    pub filtered_collisions: Vec<Prim>,
    pub simulation_owners: Vec<Prim>,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub enabled: bool,
    pub kinematic: bool,
    pub starts_asleep: bool,
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct JointLimit {
    pub dof: Dof,
    pub enabled: bool,
    pub lower: f32,
    pub upper: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct JointDrive {
    pub dof: Dof,
    pub target_position: f32,
    pub target_velocity: f32,
    pub force_limit: f32,
    pub stiffness: f32,
    pub damping: f32,
    pub acceleration: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Joint {
    pub prim: Prim,
    pub kind: JointKind,
    pub type_name: String,
    pub rel0: Option<Prim>,
    pub rel1: Option<Prim>,
    pub body0: Option<Prim>,
    pub body1: Option<Prim>,
    pub local_pos0: [f32; 3],
    pub local_rot0: [f32; 4],
    pub local_pos1: [f32; 3],
    pub local_rot1: [f32; 4],
    pub enabled: bool,
    pub collision_enabled: bool,
    pub break_force: f32,
    pub break_torque: f32,
    pub exclude_from_articulation: bool,
    pub axis: Axis,
    pub limits: Vec<JointLimit>,
    pub drives: Vec<JointDrive>,
    pub min_distance_enabled: bool,
    pub max_distance_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Articulation {
    pub prim: Prim,
    pub roots: Vec<Prim>,
    pub filtered_collisions: Vec<Prim>,
    pub joints: Vec<Prim>,
    pub bodies: Vec<Option<Prim>>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Physics {
    pub meters_per_unit: f64,
    pub kilograms_per_unit: f64,
    pub si_units: bool,
    pub scenes: Vec<PhysicsScene>,
    pub collision_groups: Vec<CollisionGroup>,
    pub materials: Vec<PhysicsMaterial>,
    pub shapes: Vec<Shape>,
    pub rigid_bodies: Vec<RigidBody>,
    pub joints: Vec<Joint>,
    pub articulations: Vec<Articulation>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PhysicsOptions {
    pub include: Vec<Prim>,
    pub exclude: Vec<Prim>,
    pub si_units: bool,
}

#[cfg(feature = "physics")]
pub(crate) mod enabled {
    use std::collections::HashMap;

    use openusd::usd;
    use openusd::{gf, sdf};
    use openusd_schemas::physics::{self, desc};

    use super::*;
    use crate::stage::usd_err;

    fn v3(v: gf::Vec3f) -> [f32; 3] {
        [v.x, v.y, v.z]
    }

    fn q4(q: gf::Quatf) -> [f32; 4] {
        [q.w, q.x, q.y, q.z]
    }

    fn token_axis(s: &str) -> Axis {
        match s {
            "X" => Axis::X,
            "Y" => Axis::Y,
            _ => Axis::Z,
        }
    }

    fn dof(d: physics::Dof) -> Dof {
        match d {
            physics::Dof::Distance => Dof::Distance,
            physics::Dof::TransX => Dof::TransX,
            physics::Dof::TransY => Dof::TransY,
            physics::Dof::TransZ => Dof::TransZ,
            physics::Dof::RotX => Dof::RotX,
            physics::Dof::RotY => Dof::RotY,
            physics::Dof::RotZ => Dof::RotZ,
        }
    }

    fn axis_dof(axis: Axis, rotational: bool) -> Dof {
        match (axis, rotational) {
            (Axis::X, false) => Dof::TransX,
            (Axis::Y, false) => Dof::TransY,
            (Axis::Z, false) => Dof::TransZ,
            (Axis::X, true) => Dof::RotX,
            (Axis::Y, true) => Dof::RotY,
            (Axis::Z, true) => Dof::RotZ,
        }
    }

    fn limit(d: Dof, l: &desc::Limit) -> JointLimit {
        JointLimit {
            dof: d,
            enabled: l.enabled,
            lower: l.lower,
            upper: l.upper,
        }
    }

    fn drive(d: Dof, v: &desc::Drive) -> JointDrive {
        JointDrive {
            dof: d,
            target_position: v.target_position,
            target_velocity: v.target_velocity,
            force_limit: v.force_limit,
            stiffness: v.stiffness,
            damping: v.damping,
            acceleration: v.acceleration,
        }
    }

    impl Stage {
        fn prim_opt(&mut self, p: &Option<sdf::Path>) -> Option<Prim> {
            p.as_ref().map(|p| self.intern(p))
        }

        fn prims(&mut self, paths: &[sdf::Path]) -> Vec<Prim> {
            paths.iter().map(|p| self.intern(p)).collect()
        }

        pub fn physics(&mut self, options: &PhysicsOptions) -> Result<Physics> {
            let include: Vec<sdf::Path> = if options.include.is_empty() {
                vec![sdf::Path::abs_root()]
            } else {
                options
                    .include
                    .iter()
                    .map(|&p| self.sdf_path(p).cloned())
                    .collect::<Result<_>>()?
            };
            let exclude = options
                .exclude
                .iter()
                .map(|&p| self.sdf_path(p).cloned())
                .collect::<Result<_>>()?;
            let parse_options = physics::ParseOptions {
                include,
                exclude,
                ..physics::ParseOptions::default()
            };
            let parsed = physics::parse(&self.inner, &parse_options).map_err(usd_err)?;
            let mut out = Physics {
                meters_per_unit: openusd_schemas::geom::stage_meters_per_unit(&self.inner).map_err(usd_err)?,
                kilograms_per_unit: physics::mass_units::stage_kilograms_per_unit(&self.inner).map_err(usd_err)?,
                diagnostics: parsed.diagnostics.iter().map(|d| format!("{d:?}")).collect(),
                ..Physics::default()
            };
            for s in &parsed.scenes {
                let prim = self.intern(&s.path);
                out.scenes.push(PhysicsScene {
                    prim,
                    gravity_direction: v3(s.gravity_direction),
                    gravity_magnitude: s.gravity_magnitude,
                });
            }
            for g in &parsed.collision_groups {
                let group = CollisionGroup {
                    prim: self.intern(&g.path),
                    invert_filtered_groups: g.invert_filtered_groups,
                    filtered_groups: self.prims(&g.filtered_groups),
                    merge_group_name: g.merge_group_name.clone(),
                    merged_groups: self.prims(&g.merged_groups),
                };
                out.collision_groups.push(group);
            }
            for m in &parsed.materials {
                let prim = self.intern(&m.path);
                out.materials.push(PhysicsMaterial {
                    prim,
                    static_friction: m.static_friction,
                    dynamic_friction: m.dynamic_friction,
                    restitution: m.restitution,
                    density: m.density,
                });
            }
            for s in &parsed.shapes {
                let mut shape = Shape {
                    prim: self.intern(&s.path),
                    rigid_body: self.prim_opt(&s.rigid_body),
                    local_pos: v3(s.local_pos),
                    local_rot: q4(s.local_rot),
                    local_scale: v3(s.local_scale),
                    materials: self.prims(&s.materials),
                    simulation_owners: self.prims(&s.simulation_owners),
                    filtered_collisions: self.prims(&s.filtered_collisions),
                    collision_groups: self.prims(&s.collision_groups),
                    collision_enabled: s.collision_enabled,
                    mesh_scale: [1.0; 3],
                    ..Shape::default()
                };
                match &s.geometry {
                    desc::Geometry::Sphere { radius } => {
                        shape.kind = ShapeKind::Sphere;
                        shape.radius = *radius;
                    }
                    desc::Geometry::Cube { half_extents } => {
                        shape.kind = ShapeKind::Cube;
                        shape.half_extents = v3(*half_extents);
                    }
                    desc::Geometry::Capsule {
                        radius,
                        half_height,
                        axis,
                    } => {
                        shape.kind = ShapeKind::Capsule;
                        (shape.radius, shape.half_height, shape.axis) =
                            (*radius, *half_height, token_axis(&format!("{axis:?}")));
                    }
                    desc::Geometry::Capsule1 {
                        top_radius,
                        bottom_radius,
                        half_height,
                        axis,
                    } => {
                        shape.kind = ShapeKind::Capsule1;
                        (shape.top_radius, shape.bottom_radius, shape.half_height) =
                            (*top_radius, *bottom_radius, *half_height);
                        shape.axis = token_axis(&format!("{axis:?}"));
                    }
                    desc::Geometry::Cylinder {
                        radius,
                        half_height,
                        axis,
                    } => {
                        shape.kind = ShapeKind::Cylinder;
                        (shape.radius, shape.half_height, shape.axis) =
                            (*radius, *half_height, token_axis(&format!("{axis:?}")));
                    }
                    desc::Geometry::Cylinder1 {
                        top_radius,
                        bottom_radius,
                        half_height,
                        axis,
                    } => {
                        shape.kind = ShapeKind::Cylinder1;
                        (shape.top_radius, shape.bottom_radius, shape.half_height) =
                            (*top_radius, *bottom_radius, *half_height);
                        shape.axis = token_axis(&format!("{axis:?}"));
                    }
                    desc::Geometry::Cone {
                        radius,
                        half_height,
                        axis,
                    } => {
                        shape.kind = ShapeKind::Cone;
                        (shape.radius, shape.half_height, shape.axis) =
                            (*radius, *half_height, token_axis(&format!("{axis:?}")));
                    }
                    desc::Geometry::Plane { axis } => {
                        shape.kind = ShapeKind::Plane;
                        shape.axis = token_axis(&format!("{axis:?}"));
                    }
                    desc::Geometry::Mesh {
                        approximation,
                        scale,
                        double_sided,
                    } => {
                        shape.kind = ShapeKind::Mesh;
                        shape.mesh_approximation = approximation.as_str().to_owned();
                        shape.mesh_scale = v3(*scale);
                        shape.double_sided = *double_sided;
                    }
                    desc::Geometry::SpherePoints(points) => {
                        shape.kind = ShapeKind::SpherePoints;
                        shape.sphere_points = points
                            .iter()
                            .map(|p| [p.center.x, p.center.y, p.center.z, p.radius])
                            .collect();
                    }
                    desc::Geometry::Custom(token) => {
                        shape.kind = ShapeKind::Custom;
                        shape.custom_type = token.as_str().to_owned();
                    }
                }
                out.shapes.push(shape);
            }
            for b in &parsed.rigid_bodies {
                let body = RigidBody {
                    prim: self.intern(&b.path),
                    collisions: self.prims(&b.collisions),
                    filtered_collisions: self.prims(&b.filtered_collisions),
                    simulation_owners: self.prims(&b.simulation_owners),
                    position: v3(b.position),
                    rotation: q4(b.rotation),
                    scale: v3(b.scale),
                    enabled: b.enabled,
                    kinematic: b.kinematic,
                    starts_asleep: b.starts_asleep,
                    linear_velocity: v3(b.linear_velocity),
                    angular_velocity: v3(b.angular_velocity),
                };
                out.rigid_bodies.push(body);
            }
            for j in &parsed.joints {
                let prim = self.intern(&j.path);
                let mut joint = Joint {
                    prim,
                    type_name: self.type_name(prim)?,
                    rel0: self.prim_opt(&j.rel0),
                    rel1: self.prim_opt(&j.rel1),
                    body0: self.prim_opt(&j.body0),
                    body1: self.prim_opt(&j.body1),
                    local_pos0: v3(j.local_pos0),
                    local_rot0: q4(j.local_rot0),
                    local_pos1: v3(j.local_pos1),
                    local_rot1: q4(j.local_rot1),
                    enabled: j.enabled,
                    collision_enabled: j.collision_enabled,
                    break_force: j.break_force,
                    break_torque: j.break_torque,
                    exclude_from_articulation: j.exclude_from_articulation,
                    ..Joint::default()
                };
                match &j.kind {
                    desc::JointKind::Fixed => joint.kind = JointKind::Fixed,
                    desc::JointKind::Revolute {
                        axis,
                        limit: l,
                        drive: d,
                    }
                    | desc::JointKind::Prismatic {
                        axis,
                        limit: l,
                        drive: d,
                    } => {
                        let rotational = matches!(j.kind, desc::JointKind::Revolute { .. });
                        joint.kind = if rotational {
                            JointKind::Revolute
                        } else {
                            JointKind::Prismatic
                        };
                        joint.axis = token_axis(&format!("{axis:?}"));
                        let dof = axis_dof(joint.axis, rotational);
                        joint.limits.push(limit(dof, l));
                        joint.drives.extend(d.iter().map(|d| drive(dof, d)));
                    }
                    desc::JointKind::Spherical { axis, limit: l } => {
                        joint.kind = JointKind::Spherical;
                        joint.axis = token_axis(&format!("{axis:?}"));
                        joint.limits.push(limit(axis_dof(joint.axis, true), l));
                    }
                    desc::JointKind::Distance {
                        min_enabled,
                        max_enabled,
                        limit: l,
                    } => {
                        joint.kind = JointKind::Distance;
                        (joint.min_distance_enabled, joint.max_distance_enabled) = (*min_enabled, *max_enabled);
                        joint.limits.push(limit(Dof::Distance, l));
                    }
                    desc::JointKind::D6 { limits, drives } => {
                        joint.kind = JointKind::D6;
                        joint.limits = limits.iter().map(|(d, l)| limit(dof(*d), l)).collect();
                        joint.drives = drives.iter().map(|(d, v)| drive(dof(*d), v)).collect();
                    }
                    desc::JointKind::Custom => joint.kind = JointKind::Custom,
                }
                out.joints.push(joint);
            }
            for a in &parsed.articulations {
                let art = Articulation {
                    prim: self.intern(&a.path),
                    roots: self.prims(&a.roots),
                    filtered_collisions: self.prims(&a.filtered_collisions),
                    joints: self.prims(&a.joints),
                    bodies: a.bodies.iter().map(|b| self.prim_opt(b)).collect(),
                };
                out.articulations.push(art);
            }
            if options.si_units {
                to_si(&mut out);
            }
            Ok(out)
        }

        pub fn mass_colliders(&mut self, body: Prim) -> Result<Vec<Prim>> {
            let body_prim = self.usd_prim(body)?;
            let mut paths = Vec::new();
            let mut walk = body_prim
                .filtered_subtree(usd::PrimPredicate::DEFAULT_PROXIES)
                .map_err(usd_err)?;
            while let Some(prim) = walk.next() {
                let prim = prim.map_err(usd_err)?;
                let schemas = prim.api_schemas().map_err(usd_err)?;
                let carries = |name: &str| schemas.iter().any(|s| s.as_str() == name);
                if prim != body_prim && carries("PhysicsRigidBodyAPI") {
                    walk.prune_children();
                } else if carries("PhysicsCollisionAPI") {
                    paths.push(prim.path().clone());
                }
            }
            Ok(paths.iter().map(|p| self.intern(p)).collect())
        }

        pub fn mass_properties(
            &mut self,
            body: Prim,
            mut mass_info: impl FnMut(Prim) -> Option<MassInformation>,
        ) -> Result<BodyMass> {
            let mut colliders = HashMap::new();
            for collider in self.mass_colliders(body)? {
                colliders.insert(self.sdf_path(collider)?.clone(), collider);
            }
            let body_prim = self.usd_prim(body)?;
            let api = physics::RigidBodyAPI::from_prim_unchecked(body_prim);
            let mass = api
                .mass_properties(|prim| {
                    let info = colliders.get(prim.path()).and_then(|&h| mass_info(h));
                    Ok(match info {
                        Some(m) => physics::MassInformation {
                            volume: m.volume,
                            inertia: gf::Mat3d(m.inertia),
                            center_of_mass: gf::Vec3d {
                                x: m.center_of_mass[0],
                                y: m.center_of_mass[1],
                                z: m.center_of_mass[2],
                            },
                            local_pos: gf::Vec3d {
                                x: m.local_pos[0],
                                y: m.local_pos[1],
                                z: m.local_pos[2],
                            },
                            local_rot: gf::Quatd {
                                w: m.local_rot[0],
                                x: m.local_rot[1],
                                y: m.local_rot[2],
                                z: m.local_rot[3],
                            },
                        },
                        None => physics::MassInformation::UNSUPPORTED,
                    })
                })
                .map_err(usd_err)?;
            Ok(BodyMass {
                mass: mass.mass,
                diagonal_inertia: [
                    mass.diagonal_inertia.x,
                    mass.diagonal_inertia.y,
                    mass.diagonal_inertia.z,
                ],
                center_of_mass: [mass.center_of_mass.x, mass.center_of_mass.y, mass.center_of_mass.z],
                principal_axes: [
                    mass.principal_axes.w,
                    mass.principal_axes.x,
                    mass.principal_axes.y,
                    mass.principal_axes.z,
                ],
            })
        }
    }

    fn scale3(v: &mut [f32; 3], k: f64) {
        for c in v.iter_mut() {
            *c = (f64::from(*c) * k) as f32;
        }
    }

    fn scale(v: &mut f32, k: f64) {
        *v = (f64::from(*v) * k) as f32;
    }

    fn to_si(p: &mut Physics) {
        let (l, m) = (p.meters_per_unit, p.kilograms_per_unit);
        p.si_units = true;
        rescale(p, l, m);
    }

    #[cfg(feature = "write")]
    pub(crate) fn from_si(p: &mut Physics) {
        let (l, m) = (p.meters_per_unit, p.kilograms_per_unit);
        p.si_units = false;
        rescale(p, 1.0 / l, 1.0 / m);
    }

    fn rescale(p: &mut Physics, l: f64, m: f64) {
        for s in &mut p.scenes {
            scale(&mut s.gravity_magnitude, l);
        }
        for mat in &mut p.materials {
            scale(&mut mat.density, m / (l * l * l));
        }
        for s in &mut p.shapes {
            scale3(&mut s.local_pos, l);
            for v in [
                &mut s.radius,
                &mut s.top_radius,
                &mut s.bottom_radius,
                &mut s.half_height,
            ] {
                scale(v, l);
            }
            scale3(&mut s.half_extents, l);
            scale3(&mut s.mesh_scale, l);
            for sp in &mut s.sphere_points {
                for c in sp.iter_mut() {
                    *c = (f64::from(*c) * l) as f32;
                }
            }
        }
        for b in &mut p.rigid_bodies {
            scale3(&mut b.position, l);
            scale3(&mut b.linear_velocity, l);
        }
        for j in &mut p.joints {
            scale3(&mut j.local_pos0, l);
            scale3(&mut j.local_pos1, l);
            if j.break_force.is_finite() {
                scale(&mut j.break_force, m * l);
            }
            if j.break_torque.is_finite() {
                scale(&mut j.break_torque, m * l * l);
            }
            for lim in &mut j.limits {
                if lim.dof.is_linear() {
                    for v in [&mut lim.lower, &mut lim.upper] {
                        if v.abs() < desc::Limit::SENTINEL {
                            scale(v, l);
                        }
                    }
                }
            }
            for d in &mut j.drives {
                let linear = d.dof.is_linear();
                if linear {
                    scale(&mut d.target_position, l);
                    scale(&mut d.target_velocity, l);
                }
                if !d.acceleration {
                    let force = if linear { m * l } else { m * l * l };
                    if d.force_limit.is_finite() {
                        scale(&mut d.force_limit, force);
                    }
                    let gain = if linear { m } else { m * l * l };
                    scale(&mut d.stiffness, gain);
                    scale(&mut d.damping, gain);
                }
            }
        }
    }
}

#[cfg(not(feature = "physics"))]
impl Stage {
    pub fn physics(&mut self, _options: &PhysicsOptions) -> Result<Physics> {
        Err(crate::Error::FeatureDisabled("physics"))
    }

    pub fn mass_colliders(&mut self, _body: Prim) -> Result<Vec<Prim>> {
        Err(crate::Error::FeatureDisabled("physics"))
    }

    pub fn mass_properties(
        &mut self,
        _body: Prim,
        _mass_info: impl FnMut(Prim) -> Option<MassInformation>,
    ) -> Result<BodyMass> {
        Err(crate::Error::FeatureDisabled("physics"))
    }
}

#[cfg(feature = "write")]
pub(crate) mod write {
    use super::{
        Articulation, Axis, BodyMass, CollisionGroup, Joint, JointKind, Physics, PhysicsMaterial, PhysicsScene,
        RigidBody, Shape,
    };
    #[cfg(feature = "physics")]
    use super::{Dof, ShapeKind};
    use crate::{Prim, Result, Stage};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct PhysicsWriteOptions {
        pub poses: bool,
    }

    impl PhysicsScene {
        pub fn new(prim: Prim) -> Self {
            Self {
                prim,
                gravity_direction: [0.0; 3],
                gravity_magnitude: f32::NEG_INFINITY,
            }
        }
    }

    impl CollisionGroup {
        pub fn new(prim: Prim) -> Self {
            Self {
                prim,
                ..Self::default()
            }
        }
    }

    impl PhysicsMaterial {
        pub fn new(prim: Prim) -> Self {
            Self {
                prim,
                ..Self::default()
            }
        }
    }

    impl Shape {
        pub fn new(prim: Prim) -> Self {
            Self {
                prim,
                local_rot: [1.0, 0.0, 0.0, 0.0],
                local_scale: [1.0; 3],
                mesh_scale: [1.0; 3],
                collision_enabled: true,
                ..Self::default()
            }
        }
    }

    impl RigidBody {
        pub fn new(prim: Prim) -> Self {
            Self {
                prim,
                rotation: [1.0, 0.0, 0.0, 0.0],
                scale: [1.0; 3],
                enabled: true,
                ..Self::default()
            }
        }
    }

    impl Joint {
        pub fn new(prim: Prim, kind: JointKind) -> Self {
            Self {
                prim,
                kind,
                local_rot0: [1.0, 0.0, 0.0, 0.0],
                local_rot1: [1.0, 0.0, 0.0, 0.0],
                enabled: true,
                break_force: f32::INFINITY,
                break_torque: f32::INFINITY,
                axis: Axis::X,
                ..Self::default()
            }
        }
    }

    impl Articulation {
        pub fn new(prim: Prim) -> Self {
            Self {
                prim,
                ..Self::default()
            }
        }
    }

    #[cfg(feature = "physics")]
    mod enabled {
        use super::*;
        use crate::{BaseType, Error, Value};

        const fn dof_token(dof: Dof) -> &'static str {
            match dof {
                Dof::Distance => "distance",
                Dof::TransX => "transX",
                Dof::TransY => "transY",
                Dof::TransZ => "transZ",
                Dof::RotX => "rotX",
                Dof::RotY => "rotY",
                Dof::RotZ => "rotZ",
            }
        }

        const fn axis_token(axis: Axis) -> &'static str {
            match axis {
                Axis::X => "X",
                Axis::Y => "Y",
                Axis::Z => "Z",
            }
        }

        fn float3(v: [f32; 3]) -> Result<Value> {
            Value::from_slice(BaseType::Float3, false, &[v])
        }

        fn quatf(v: [f32; 4]) -> Result<Value> {
            Value::from_slice(BaseType::Quatf, false, &[v])
        }

        fn trs_matrix(t: [f32; 3], q: [f32; 4], s: [f32; 3]) -> [f64; 16] {
            let [w, x, y, z] = q.map(f64::from);
            let len = (w * w + x * x + y * y + z * z).sqrt();
            let (w, x, y, z) = if len > 0.0 {
                (w / len, x / len, y / len, z / len)
            } else {
                (1.0, 0.0, 0.0, 0.0)
            };
            let rows = [
                [
                    1.0 - 2.0 * (y * y + z * z),
                    2.0 * (x * y + w * z),
                    2.0 * (x * z - w * y),
                ],
                [
                    2.0 * (x * y - w * z),
                    1.0 - 2.0 * (x * x + z * z),
                    2.0 * (y * z + w * x),
                ],
                [
                    2.0 * (x * z + w * y),
                    2.0 * (y * z - w * x),
                    1.0 - 2.0 * (x * x + y * y),
                ],
            ];
            let mut m = [0.0; 16];
            for (i, row) in rows.iter().enumerate() {
                for (k, v) in row.iter().enumerate() {
                    m[i * 4 + k] = v * f64::from(s[i]);
                }
            }
            m[12..15].copy_from_slice(&t.map(f64::from));
            m[15] = 1.0;
            m
        }

        impl Stage {
            fn put(&mut self, prim: Prim, name: &str, value: Result<Value>) -> Result<()> {
                self.set(prim, name, &value?, None)
            }

            fn ensure_api(&mut self, prim: Prim, schema: &str) -> Result<()> {
                if self.has_api(prim, schema)? {
                    return Ok(());
                }
                self.apply_api(prim, schema)
            }

            fn ensure_type(&mut self, prim: Prim, type_name: &str) -> Result<()> {
                let path = self.path(prim)?.to_owned();
                if self.prim_at_path(&path)?.is_some() && self.type_name(prim)? == type_name {
                    return Ok(());
                }
                self.define_prim(&path, type_name).map(drop)
            }

            fn rel(&mut self, prim: Prim, name: &str, targets: &[Prim]) -> Result<()> {
                if targets.is_empty() {
                    return Ok(());
                }
                let paths: Vec<String> = targets
                    .iter()
                    .map(|&t| self.path(t).map(str::to_owned))
                    .collect::<Result<_>>()?;
                let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
                self.set_targets(prim, name, &paths)
            }

            fn filtered_pairs(&mut self, prim: Prim, filtered: &[Prim]) -> Result<()> {
                if filtered.is_empty() {
                    return Ok(());
                }
                self.ensure_api(prim, "PhysicsFilteredPairsAPI")?;
                self.rel(prim, "physics:filteredPairs", filtered)
            }

            pub fn write_physics(&mut self, physics: &Physics, options: &PhysicsWriteOptions) -> Result<()> {
                let mut p = physics.clone();
                if p.si_units {
                    if !(p.meters_per_unit > 0.0 && p.kilograms_per_unit > 0.0) {
                        return Err(Error::InvalidValue(
                            "SI values need meters_per_unit and kilograms_per_unit to convert back".to_owned(),
                        ));
                    }
                    crate::helpers::physics::enabled::from_si(&mut p);
                }
                for scene in &p.scenes {
                    self.write_physics_scene(scene)?;
                }
                for group in &p.collision_groups {
                    self.write_collision_group(group)?;
                }
                for material in &p.materials {
                    self.write_physics_material(material)?;
                }
                for shape in &p.shapes {
                    self.write_shape(shape)?;
                }
                for body in &p.rigid_bodies {
                    self.write_rigid_body(body, options.poses)?;
                }
                for joint in &p.joints {
                    self.write_joint(joint)?;
                }
                for articulation in &p.articulations {
                    self.write_articulation(articulation)?;
                }
                Ok(())
            }

            pub fn write_physics_scene(&mut self, scene: &PhysicsScene) -> Result<()> {
                let prim = scene.prim;
                self.ensure_type(prim, "PhysicsScene")?;
                self.put(prim, "physics:gravityDirection", float3(scene.gravity_direction))?;
                self.put(
                    prim,
                    "physics:gravityMagnitude",
                    Ok(Value::float(scene.gravity_magnitude)),
                )
            }

            pub fn write_collision_group(&mut self, group: &CollisionGroup) -> Result<()> {
                let prim = group.prim;
                self.ensure_type(prim, "PhysicsCollisionGroup")?;
                self.put(
                    prim,
                    "physics:invertFilteredGroups",
                    Ok(Value::bool(group.invert_filtered_groups)),
                )?;
                if !group.merge_group_name.is_empty() {
                    self.put(prim, "physics:mergeGroup", Ok(Value::string(&group.merge_group_name)))?;
                }
                self.rel(prim, "physics:filteredGroups", &group.filtered_groups)
            }

            pub fn write_physics_material(&mut self, material: &PhysicsMaterial) -> Result<()> {
                let prim = material.prim;
                self.ensure_api(prim, "PhysicsMaterialAPI")?;
                self.put(
                    prim,
                    "physics:staticFriction",
                    Ok(Value::float(material.static_friction)),
                )?;
                self.put(
                    prim,
                    "physics:dynamicFriction",
                    Ok(Value::float(material.dynamic_friction)),
                )?;
                self.put(prim, "physics:restitution", Ok(Value::float(material.restitution)))?;
                self.put(prim, "physics:density", Ok(Value::float(material.density)))
            }

            pub fn write_shape(&mut self, shape: &Shape) -> Result<()> {
                let prim = shape.prim;
                self.ensure_api(prim, "PhysicsCollisionAPI")?;
                self.put(
                    prim,
                    "physics:collisionEnabled",
                    Ok(Value::bool(shape.collision_enabled)),
                )?;
                self.rel(prim, "physics:simulationOwner", &shape.simulation_owners)?;
                self.write_collider_materials(shape)?;
                self.filtered_pairs(prim, &shape.filtered_collisions)?;
                if shape.kind == ShapeKind::Mesh {
                    self.ensure_api(prim, "PhysicsMeshCollisionAPI")?;
                    if !shape.mesh_approximation.is_empty() {
                        self.put(
                            prim,
                            "physics:approximation",
                            Ok(Value::token(&shape.mesh_approximation)),
                        )?;
                    }
                }
                let path = self.path(prim)?.to_owned();
                for &group in &shape.collision_groups {
                    self.ensure_type(group, "PhysicsCollisionGroup")?;
                    let rel = "collection:colliders:includes";
                    if !self.targets(group, rel, false)?.contains(&path) {
                        self.add_target(group, rel, &path)?;
                    }
                }
                Ok(())
            }

            pub fn write_rigid_body(&mut self, body: &RigidBody, pose: bool) -> Result<()> {
                let prim = body.prim;
                self.ensure_api(prim, "PhysicsRigidBodyAPI")?;
                self.put(prim, "physics:rigidBodyEnabled", Ok(Value::bool(body.enabled)))?;
                self.put(prim, "physics:kinematicEnabled", Ok(Value::bool(body.kinematic)))?;
                self.put(prim, "physics:startsAsleep", Ok(Value::bool(body.starts_asleep)))?;
                self.put(prim, "physics:velocity", float3(body.linear_velocity))?;
                self.put(prim, "physics:angularVelocity", float3(body.angular_velocity))?;
                self.rel(prim, "physics:simulationOwner", &body.simulation_owners)?;
                self.filtered_pairs(prim, &body.filtered_collisions)?;
                if pose {
                    let m = trs_matrix(body.position, body.rotation, body.scale);
                    self.set_world_pose(prim, &m, None)?;
                }
                Ok(())
            }

            pub fn write_body_mass(&mut self, prim: Prim, mass: &BodyMass) -> Result<()> {
                self.ensure_api(prim, "PhysicsMassAPI")?;
                self.put(prim, "physics:mass", Ok(Value::float(mass.mass as f32)))?;
                self.put(
                    prim,
                    "physics:centerOfMass",
                    float3(mass.center_of_mass.map(|v| v as f32)),
                )?;
                self.put(
                    prim,
                    "physics:diagonalInertia",
                    float3(mass.diagonal_inertia.map(|v| v as f32)),
                )?;
                self.put(
                    prim,
                    "physics:principalAxes",
                    quatf(mass.principal_axes.map(|v| v as f32)),
                )
            }

            fn body_scale(&self, body: Prim) -> Result<[f32; 3]> {
                let m = self.world_transform(body, None)?;
                Ok(std::array::from_fn(|i| {
                    let row = &m[i * 4..i * 4 + 3];
                    let len = row.iter().map(|v| v * v).sum::<f64>().sqrt();
                    if len > 0.0 { len as f32 } else { 1.0 }
                }))
            }

            /// The face subsets of `prim` the reader takes physics materials
            /// from, in its order.
            fn material_subsets(&mut self, prim: Prim) -> Result<Vec<Prim>> {
                use crate::stage::usd_err;
                use openusd::usd::SchemaBase;
                use openusd_schemas::geom::{ElementType, Mesh, Subset};
                use openusd_schemas::physics::MaterialAPI;
                use openusd_schemas::shade::MaterialBindingAPI;
                let p = self.usd_prim(prim)?;
                let Some(mesh) = Mesh::from_prim(p).map_err(usd_err)? else {
                    return Ok(Vec::new());
                };
                let mut paths = Vec::new();
                for subset in Subset::in_family(&mesh, Some(ElementType::Face), None).map_err(usd_err)? {
                    let bound = match MaterialBindingAPI::from_prim(subset.prim().clone()).map_err(usd_err)? {
                        Some(binding) => MaterialAPI::bound(&binding).map_err(usd_err)?,
                        None => None,
                    };
                    if bound.is_some() {
                        paths.push(subset.prim().path().clone());
                    }
                }
                Ok(paths.iter().map(|p| self.intern(p)).collect())
            }

            fn physics_material(&mut self, prim: Prim) -> Result<Option<Prim>> {
                Ok(self.bound_material(prim, "physics")?.map(|b| b.material))
            }

            /// The inverse of how colliders' materials are read: the face
            /// subsets bound to a physics material take the first ones, in
            /// order, and one more is the collider's own.
            fn write_collider_materials(&mut self, shape: &Shape) -> Result<()> {
                let prim = shape.prim;
                let subsets = self.material_subsets(prim)?;
                let n = subsets.len();
                if shape.materials.len() > n + 1 {
                    return Err(Error::InvalidValue(format!(
                        "{} takes {} physics materials, one for each face subset bound to one and then its own, not {}",
                        self.path(prim)?,
                        n + 1,
                        shape.materials.len()
                    )));
                }
                if let Some(&own) = shape.materials.get(n)
                    && self.physics_material(prim)? != Some(own)
                {
                    self.bind_material(prim, own, "physics")?;
                }
                for (&subset, &material) in subsets.iter().zip(&shape.materials) {
                    if self.physics_material(subset)? != Some(material) {
                        self.bind_material(subset, material, "physics")?;
                    }
                }
                Ok(())
            }

            /// The rigid body a joint target belongs to, as the reader finds
            /// it: its nearest ancestor with `PhysicsRigidBodyAPI`, or else its
            /// outermost one with `PhysicsCollisionAPI`.
            fn body_of(&mut self, prim: Prim) -> Result<Option<Prim>> {
                let mut collision = None;
                let mut cursor = Some(prim);
                while let Some(p) = cursor.filter(|&p| p != Prim::PSEUDO_ROOT) {
                    if self.has_api(p, "PhysicsRigidBodyAPI")? {
                        return Ok(Some(p));
                    }
                    if self.has_api(p, "PhysicsCollisionAPI")? {
                        collision = Some(p);
                    }
                    cursor = self.parent(p)?;
                }
                Ok(collision)
            }

            /// The original target of a joint side, where the joint was read
            /// through a prim inside its body and still belongs to it.
            fn anchor_target(&mut self, rel: Option<Prim>, body: Option<Prim>) -> Result<Option<Prim>> {
                let Some(target) = rel.filter(|&r| Some(r) != body) else {
                    return Ok(None);
                };
                let path = self.path(target)?.to_owned();
                if self.prim_at_path(&path)?.is_none() {
                    return Ok(None);
                }
                Ok((self.body_of(target)? == body).then_some(target))
            }

            fn joint_side(&mut self, joint: &Joint, side: usize) -> Result<()> {
                let (rel, body, pos, rot) = if side == 0 {
                    (joint.rel0, joint.body0, joint.local_pos0, joint.local_rot0)
                } else {
                    (joint.rel1, joint.body1, joint.local_pos1, joint.local_rot1)
                };
                if let Some(target) = self.anchor_target(rel, body)? {
                    // The reader carries the anchor through the target's world
                    // transform into the body's frame, and scales it there.
                    use openusd::gf;
                    let body_world = match body {
                        Some(b) => gf::Mat4d(self.world_transform(b, None)?),
                        None => gf::Mat4d::IDENTITY,
                    };
                    let scale = body_world.factor(gf::Mat4d::FACTOR_EPSILON).scale;
                    let unscaled = gf::vec3f(
                        (f64::from(pos[0]) / scale.x) as f32,
                        (f64::from(pos[1]) / scale.y) as f32,
                        (f64::from(pos[2]) / scale.z) as f32,
                    );
                    let q = gf::Quatf {
                        w: rot[0],
                        x: rot[1],
                        y: rot[2],
                        z: rot[3],
                    };
                    let local = gf::Mat4d::from_trs(unscaled, q, gf::vec3f(1.0, 1.0, 1.0));
                    let target_world = gf::Mat4d(self.world_transform(target, None)?);
                    let anchor = (local * body_world * target_world.inverse_or_max()).remove_scale_shear();
                    let t = anchor.extract_translation();
                    let r = anchor.extract_rotation_quat();
                    let len = (r.w * r.w + r.x * r.x + r.y * r.y + r.z * r.z).sqrt();
                    self.rel(joint.prim, &format!("physics:body{side}"), &[target])?;
                    self.put(
                        joint.prim,
                        &format!("physics:localPos{side}"),
                        float3([t.x as f32, t.y as f32, t.z as f32]),
                    )?;
                    let r = [r.w / len, r.x / len, r.y / len, r.z / len].map(|v| v as f32);
                    return self.put(joint.prim, &format!("physics:localRot{side}"), quatf(r));
                }
                let mut pos = pos;
                if let Some(body) = body {
                    let scale = self.body_scale(body)?;
                    pos = std::array::from_fn(|i| pos[i] / scale[i]);
                    self.rel(joint.prim, &format!("physics:body{side}"), &[body])?;
                }
                self.put(joint.prim, &format!("physics:localPos{side}"), float3(pos))?;
                self.put(joint.prim, &format!("physics:localRot{side}"), quatf(rot))
            }

            fn drive(&mut self, prim: Prim, instance: &str, drive: &crate::JointDrive) -> Result<()> {
                self.ensure_api(prim, &format!("PhysicsDriveAPI:{instance}"))?;
                let attr = |name: &str| format!("drive:{instance}:physics:{name}");
                let kind = if drive.acceleration { "acceleration" } else { "force" };
                self.put(prim, &attr("type"), Ok(Value::token(kind)))?;
                self.put(prim, &attr("targetPosition"), Ok(Value::float(drive.target_position)))?;
                self.put(prim, &attr("targetVelocity"), Ok(Value::float(drive.target_velocity)))?;
                self.put(prim, &attr("maxForce"), Ok(Value::float(drive.force_limit)))?;
                self.put(prim, &attr("stiffness"), Ok(Value::float(drive.stiffness)))?;
                self.put(prim, &attr("damping"), Ok(Value::float(drive.damping)))
            }

            pub fn write_joint(&mut self, joint: &Joint) -> Result<()> {
                let prim = joint.prim;
                let type_name = match joint.kind {
                    JointKind::Fixed => "PhysicsFixedJoint",
                    JointKind::Revolute => "PhysicsRevoluteJoint",
                    JointKind::Prismatic => "PhysicsPrismaticJoint",
                    JointKind::Spherical => "PhysicsSphericalJoint",
                    JointKind::Distance => "PhysicsDistanceJoint",
                    JointKind::D6 => "PhysicsJoint",
                    JointKind::Custom if !joint.type_name.is_empty() => joint.type_name.as_str(),
                    JointKind::Custom => {
                        return Err(Error::InvalidValue("a custom joint needs its type_name".to_owned()));
                    }
                };
                self.ensure_type(prim, type_name)?;
                self.joint_side(joint, 0)?;
                self.joint_side(joint, 1)?;
                self.put(prim, "physics:jointEnabled", Ok(Value::bool(joint.enabled)))?;
                self.put(
                    prim,
                    "physics:collisionEnabled",
                    Ok(Value::bool(joint.collision_enabled)),
                )?;
                self.put(prim, "physics:breakForce", Ok(Value::float(joint.break_force)))?;
                self.put(prim, "physics:breakTorque", Ok(Value::float(joint.break_torque)))?;
                self.put(
                    prim,
                    "physics:excludeFromArticulation",
                    Ok(Value::bool(joint.exclude_from_articulation)),
                )?;
                let limit = joint.limits.first();
                let unbounded = (f32::NEG_INFINITY, f32::INFINITY);
                match joint.kind {
                    JointKind::Revolute | JointKind::Prismatic => {
                        self.put(prim, "physics:axis", Ok(Value::token(axis_token(joint.axis))))?;
                        if let Some(l) = limit {
                            let (lo, hi) = if l.enabled { (l.lower, l.upper) } else { unbounded };
                            self.put(prim, "physics:lowerLimit", Ok(Value::float(lo)))?;
                            self.put(prim, "physics:upperLimit", Ok(Value::float(hi)))?;
                        }
                        let instance = if joint.kind == JointKind::Revolute {
                            "angular"
                        } else {
                            "linear"
                        };
                        if let Some(d) = joint.drives.first() {
                            self.drive(prim, instance, d)?;
                        }
                    }
                    JointKind::Spherical => {
                        self.put(prim, "physics:axis", Ok(Value::token(axis_token(joint.axis))))?;
                        if let Some(l) = limit {
                            let (a0, a1) = if l.enabled { (l.lower, l.upper) } else { (-1.0, -1.0) };
                            self.put(prim, "physics:coneAngle0Limit", Ok(Value::float(a0)))?;
                            self.put(prim, "physics:coneAngle1Limit", Ok(Value::float(a1)))?;
                        }
                    }
                    JointKind::Distance => {
                        let (lo, hi) = limit.map_or((-1.0, -1.0), |l| (l.lower, l.upper));
                        let lo = if joint.min_distance_enabled { lo } else { -1.0 };
                        let hi = if joint.max_distance_enabled { hi } else { -1.0 };
                        self.put(prim, "physics:minDistance", Ok(Value::float(lo)))?;
                        self.put(prim, "physics:maxDistance", Ok(Value::float(hi)))?;
                    }
                    JointKind::D6 => {
                        for l in &joint.limits {
                            let dof = dof_token(l.dof);
                            self.ensure_api(prim, &format!("PhysicsLimitAPI:{dof}"))?;
                            let (lo, hi) = if l.enabled { (l.lower, l.upper) } else { unbounded };
                            self.put(prim, &format!("limit:{dof}:physics:low"), Ok(Value::float(lo)))?;
                            self.put(prim, &format!("limit:{dof}:physics:high"), Ok(Value::float(hi)))?;
                        }
                        for d in &joint.drives {
                            self.drive(prim, dof_token(d.dof), d)?;
                        }
                    }
                    JointKind::Fixed | JointKind::Custom => {}
                }
                Ok(())
            }

            pub fn write_articulation(&mut self, articulation: &Articulation) -> Result<()> {
                self.ensure_api(articulation.prim, "PhysicsArticulationRootAPI")?;
                self.filtered_pairs(articulation.prim, &articulation.filtered_collisions)
            }
        }
    }

    #[cfg(not(feature = "physics"))]
    impl Stage {
        pub fn write_physics(&mut self, _physics: &Physics, _options: &PhysicsWriteOptions) -> Result<()> {
            Err(crate::Error::FeatureDisabled("physics"))
        }
        pub fn write_physics_scene(&mut self, _scene: &PhysicsScene) -> Result<()> {
            Err(crate::Error::FeatureDisabled("physics"))
        }
        pub fn write_collision_group(&mut self, _group: &CollisionGroup) -> Result<()> {
            Err(crate::Error::FeatureDisabled("physics"))
        }
        pub fn write_physics_material(&mut self, _material: &PhysicsMaterial) -> Result<()> {
            Err(crate::Error::FeatureDisabled("physics"))
        }
        pub fn write_shape(&mut self, _shape: &Shape) -> Result<()> {
            Err(crate::Error::FeatureDisabled("physics"))
        }
        pub fn write_rigid_body(&mut self, _body: &RigidBody, _pose: bool) -> Result<()> {
            Err(crate::Error::FeatureDisabled("physics"))
        }
        pub fn write_body_mass(&mut self, _prim: Prim, _mass: &BodyMass) -> Result<()> {
            Err(crate::Error::FeatureDisabled("physics"))
        }
        pub fn write_joint(&mut self, _joint: &Joint) -> Result<()> {
            Err(crate::Error::FeatureDisabled("physics"))
        }
        pub fn write_articulation(&mut self, _articulation: &Articulation) -> Result<()> {
            Err(crate::Error::FeatureDisabled("physics"))
        }
    }
}
