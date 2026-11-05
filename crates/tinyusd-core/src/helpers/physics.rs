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
    pub mass: Option<BodyMass>,
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
    pub compute_mass: bool,
    pub si_units: bool,
}

#[cfg(feature = "physics")]
#[allow(clippy::needless_range_loop)]
mod enabled {
    use std::collections::HashMap;
    use std::f64::consts::PI;

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

    struct MassShape {
        volume: f64,
        inertia: [[f64; 3]; 3],
        center: [f64; 3],
    }

    fn diag(v: f64, i: [f64; 3]) -> MassShape {
        MassShape {
            volume: v,
            inertia: [[i[0], 0.0, 0.0], [0.0, i[1], 0.0], [0.0, 0.0, i[2]]],
            center: [0.0; 3],
        }
    }

    fn along(axis: Axis, axial: f64, across: f64) -> [f64; 3] {
        match axis {
            Axis::X => [axial, across, across],
            Axis::Y => [across, axial, across],
            Axis::Z => [across, across, axial],
        }
    }

    fn sphere(r: f64) -> MassShape {
        let v = 4.0 / 3.0 * PI * r.powi(3);
        let i = 0.4 * v * r * r;
        diag(v, [i; 3])
    }

    fn combine(parts: Vec<MassShape>) -> Option<MassShape> {
        let volume: f64 = parts.iter().map(|p| p.volume).sum();
        if volume <= 0.0 {
            return None;
        }
        let mut center = [0.0; 3];
        for p in &parts {
            for k in 0..3 {
                center[k] += p.center[k] * p.volume / volume;
            }
        }
        let mut inertia = [[0.0; 3]; 3];
        for p in &parts {
            let d = [
                p.center[0] - center[0],
                p.center[1] - center[1],
                p.center[2] - center[2],
            ];
            let dd = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            for r in 0..3 {
                for c in 0..3 {
                    let kron = if r == c { 1.0 } else { 0.0 };
                    inertia[r][c] += p.inertia[r][c] + p.volume * (dd * kron - d[r] * d[c]);
                }
            }
        }
        Some(MassShape {
            volume,
            inertia,
            center,
        })
    }

    fn mesh_shape(prim: &usd::Prim, scale: [f32; 3]) -> Option<MassShape> {
        let points = prim.attribute("points").get::<Vec<gf::Vec3f>>().ok()??;
        let counts = prim.attribute("faceVertexCounts").get::<Vec<i32>>().ok()??;
        let indices = prim.attribute("faceVertexIndices").get::<Vec<i32>>().ok()??;
        let p = |i: i32| -> Option<[f64; 3]> {
            let v = points.get(usize::try_from(i).ok()?)?;
            Some([
                f64::from(v.x * scale[0]),
                f64::from(v.y * scale[1]),
                f64::from(v.z * scale[2]),
            ])
        };
        let (mut volume, mut first, mut cov) = (0.0, [0.0; 3], [[0.0; 3]; 3]);
        let mut start = 0usize;
        for &count in &counts {
            let count = usize::try_from(count).ok()?;
            let face = indices.get(start..start + count)?;
            for k in 1..count.saturating_sub(1) {
                let (a, b, c) = (p(face[0])?, p(face[k])?, p(face[k + 1])?);
                let det = a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                    + a[2] * (b[0] * c[1] - b[1] * c[0]);
                volume += det / 6.0;
                let s = [a[0] + b[0] + c[0], a[1] + b[1] + c[1], a[2] + b[2] + c[2]];
                for r in 0..3 {
                    first[r] += det / 24.0 * s[r];
                    for col in 0..3 {
                        cov[r][col] += det / 120.0 * (a[r] * a[col] + b[r] * b[col] + c[r] * c[col] + s[r] * s[col]);
                    }
                }
            }
            start += count;
        }
        if volume.abs() < 1e-12 {
            return None;
        }
        let sign = volume.signum();
        let (volume, first) = (volume * sign, first.map(|f| f * sign));
        let center = first.map(|f| f / volume);
        let mut c = [[0.0; 3]; 3];
        for r in 0..3 {
            for col in 0..3 {
                c[r][col] = cov[r][col] * sign - volume * center[r] * center[col];
            }
        }
        let trace = c[0][0] + c[1][1] + c[2][2];
        let mut inertia = [[0.0; 3]; 3];
        for r in 0..3 {
            for col in 0..3 {
                inertia[r][col] = if r == col { trace } else { 0.0 } - c[r][col];
            }
        }
        Some(MassShape {
            volume,
            inertia,
            center,
        })
    }

    fn mass_shape(shape: &Shape, prim: &usd::Prim) -> std::result::Result<Option<MassShape>, String> {
        let r = f64::from(shape.radius);
        let h = f64::from(shape.half_height);
        Ok(match shape.kind {
            ShapeKind::Sphere => Some(sphere(r)),
            ShapeKind::Cube => {
                let [a, b, c] = shape.half_extents.map(f64::from);
                let v = 8.0 * a * b * c;
                Some(diag(
                    v,
                    [
                        v * (b * b + c * c) / 3.0,
                        v * (a * a + c * c) / 3.0,
                        v * (a * a + b * b) / 3.0,
                    ],
                ))
            }
            ShapeKind::Cylinder => {
                let l = 2.0 * h;
                let v = PI * r * r * l;
                Some(diag(
                    v,
                    along(shape.axis, v * r * r / 2.0, v * (3.0 * r * r + l * l) / 12.0),
                ))
            }
            ShapeKind::Capsule => {
                let l = 2.0 * h;
                let vc = PI * r * r * l;
                let vh = 4.0 / 3.0 * PI * r.powi(3);
                let axial = vc * r * r / 2.0 + vh * 0.4 * r * r;
                let across = vc * (l * l / 12.0 + r * r / 4.0) + vh * (0.4 * r * r + l * l / 4.0 + 3.0 * l * r / 8.0);
                Some(diag(vc + vh, along(shape.axis, axial, across)))
            }
            ShapeKind::Cone => {
                let height = 2.0 * h;
                let v = PI * r * r * height / 3.0;
                let mut s = diag(
                    v,
                    along(
                        shape.axis,
                        0.3 * v * r * r,
                        v * (3.0 * r * r / 20.0 + 3.0 * height * height / 80.0),
                    ),
                );
                s.center[shape.axis as usize] = -h / 2.0;
                Some(s)
            }
            ShapeKind::SpherePoints => combine(
                shape
                    .sphere_points
                    .iter()
                    .map(|p| MassShape {
                        center: [p[0], p[1], p[2]].map(f64::from),
                        ..sphere(f64::from(p[3]))
                    })
                    .collect(),
            ),
            ShapeKind::Mesh => mesh_shape(prim, shape.mesh_scale),
            ShapeKind::Capsule1 | ShapeKind::Cylinder1 | ShapeKind::Plane | ShapeKind::Custom => None,
        })
    }

    fn mat3(m: [[f64; 3]; 3]) -> gf::Mat3d {
        gf::Mat3d([
            m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2], m[2][0], m[2][1], m[2][2],
        ])
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
                    mass: None,
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
            if options.compute_mass {
                self.compute_masses(&mut out)?;
            }
            if options.si_units {
                to_si(&mut out);
            }
            Ok(out)
        }

        fn compute_masses(&mut self, out: &mut Physics) -> Result<()> {
            let shapes: HashMap<Prim, Shape> = out.shapes.iter().map(|s| (s.prim, s.clone())).collect();
            let mut by_path = HashMap::new();
            for s in &out.shapes {
                by_path.insert(self.sdf_path(s.prim)?.clone(), s.prim);
            }
            for body in &mut out.rigid_bodies {
                let api = openusd_schemas::physics::RigidBodyAPI::from_prim_unchecked(self.usd_prim(body.prim)?);
                let mut unsupported = Vec::new();
                let mass = api
                    .mass_properties(|prim| {
                        let Some(shape) = by_path.get(prim.path()).and_then(|p| shapes.get(p)) else {
                            unsupported.push(prim.path().to_string());
                            return Ok(physics::MassInformation::UNSUPPORTED);
                        };
                        let computed = mass_shape(shape, prim).unwrap_or(None);
                        let Some(m) = computed else {
                            unsupported.push(prim.path().to_string());
                            return Ok(physics::MassInformation::UNSUPPORTED);
                        };
                        let [w, x, y, z] = shape.local_rot.map(f64::from);
                        let [px, py, pz] = shape.local_pos.map(f64::from);
                        Ok(physics::MassInformation {
                            volume: m.volume,
                            inertia: mat3(m.inertia),
                            center_of_mass: gf::Vec3d {
                                x: m.center[0],
                                y: m.center[1],
                                z: m.center[2],
                            },
                            local_pos: gf::Vec3d { x: px, y: py, z: pz },
                            local_rot: gf::Quatd { w, x, y, z },
                        })
                    })
                    .map_err(usd_err)?;
                for path in unsupported {
                    out.diagnostics.push(format!("MassNotComputed({path})"));
                }
                body.mass = Some(BodyMass {
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
                });
            }
            Ok(())
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
            if let Some(mass) = &mut b.mass {
                if mass.mass > 0.0 {
                    mass.mass *= m;
                }
                mass.diagonal_inertia = mass.diagonal_inertia.map(|i| i * m * l * l);
                mass.center_of_mass = mass.center_of_mass.map(|c| c * l);
            }
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
}
