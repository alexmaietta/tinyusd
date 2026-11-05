#ifndef TINYUSD_H
#define TINYUSD_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define TUSD_ABI_VERSION 7

typedef enum TusdStatus {
  TUSD_STATUS_OK = 0,
  TUSD_STATUS_INVALID_ARGUMENT = 1,
  TUSD_STATUS_OPEN_FAILED = 2,
  TUSD_STATUS_INVALID_PATH = 3,
  TUSD_STATUS_INVALID_PRIM = 4,
  TUSD_STATUS_ASSET_FAILED = 5,
  TUSD_STATUS_USD_ERROR = 6,
  TUSD_STATUS_PANIC = 7,
  TUSD_STATUS_FEATURE_DISABLED = 8,
  TUSD_STATUS_SCHEMA_FAILED = 9,
} TusdStatus;

typedef enum TusdSpecifier {
  TUSD_SPECIFIER_DEF = 0,
  TUSD_SPECIFIER_OVER = 1,
  TUSD_SPECIFIER_CLASS = 2,
} TusdSpecifier;

typedef enum TusdInterpolation {
  TUSD_INTERPOLATION_CONSTANT = 0,
  TUSD_INTERPOLATION_UNIFORM = 1,
  TUSD_INTERPOLATION_VARYING = 2,
  TUSD_INTERPOLATION_VERTEX = 3,
  TUSD_INTERPOLATION_FACE_VARYING = 4,
} TusdInterpolation;

typedef enum TusdValueType {
  TUSD_VALUE_TYPE_EMPTY = 0,
  TUSD_VALUE_TYPE_BOOL = 1,
  TUSD_VALUE_TYPE_UCHAR = 2,
  TUSD_VALUE_TYPE_INT = 3,
  TUSD_VALUE_TYPE_UINT = 4,
  TUSD_VALUE_TYPE_INT64 = 5,
  TUSD_VALUE_TYPE_UINT64 = 6,
  TUSD_VALUE_TYPE_HALF = 7,
  TUSD_VALUE_TYPE_FLOAT = 8,
  TUSD_VALUE_TYPE_DOUBLE = 9,
  TUSD_VALUE_TYPE_TIME_CODE = 10,
  TUSD_VALUE_TYPE_STRING = 11,
  TUSD_VALUE_TYPE_TOKEN = 12,
  TUSD_VALUE_TYPE_ASSET = 13,
  TUSD_VALUE_TYPE_PATH = 14,
  TUSD_VALUE_TYPE_INT2 = 15,
  TUSD_VALUE_TYPE_INT3 = 16,
  TUSD_VALUE_TYPE_INT4 = 17,
  TUSD_VALUE_TYPE_HALF2 = 18,
  TUSD_VALUE_TYPE_HALF3 = 19,
  TUSD_VALUE_TYPE_HALF4 = 20,
  TUSD_VALUE_TYPE_FLOAT2 = 21,
  TUSD_VALUE_TYPE_FLOAT3 = 22,
  TUSD_VALUE_TYPE_FLOAT4 = 23,
  TUSD_VALUE_TYPE_DOUBLE2 = 24,
  TUSD_VALUE_TYPE_DOUBLE3 = 25,
  TUSD_VALUE_TYPE_DOUBLE4 = 26,
  TUSD_VALUE_TYPE_QUATH = 27,
  TUSD_VALUE_TYPE_QUATF = 28,
  TUSD_VALUE_TYPE_QUATD = 29,
  TUSD_VALUE_TYPE_MATRIX2D = 30,
  TUSD_VALUE_TYPE_MATRIX3D = 31,
  TUSD_VALUE_TYPE_MATRIX4D = 32,
  TUSD_VALUE_TYPE_UNSUPPORTED = 33,
} TusdValueType;

typedef enum TusdSkinningMethod {
  TUSD_SKINNING_METHOD_CLASSIC_LINEAR = 0,
  TUSD_SKINNING_METHOD_DUAL_QUATERNION = 1,
} TusdSkinningMethod;

typedef enum TusdPhysicsKind {
  TUSD_PHYSICS_KIND_SCENE = 0,
  TUSD_PHYSICS_KIND_COLLISION_GROUP = 1,
  TUSD_PHYSICS_KIND_MATERIAL = 2,
  TUSD_PHYSICS_KIND_SHAPE = 3,
  TUSD_PHYSICS_KIND_RIGID_BODY = 4,
  TUSD_PHYSICS_KIND_JOINT = 5,
  TUSD_PHYSICS_KIND_ARTICULATION = 6,
  TUSD_PHYSICS_KIND_DIAGNOSTIC = 7,
} TusdPhysicsKind;

typedef enum TusdShapeKind {
  TUSD_SHAPE_KIND_SPHERE = 0,
  TUSD_SHAPE_KIND_CUBE = 1,
  TUSD_SHAPE_KIND_CAPSULE = 2,
  TUSD_SHAPE_KIND_CAPSULE1 = 3,
  TUSD_SHAPE_KIND_CYLINDER = 4,
  TUSD_SHAPE_KIND_CYLINDER1 = 5,
  TUSD_SHAPE_KIND_CONE = 6,
  TUSD_SHAPE_KIND_PLANE = 7,
  TUSD_SHAPE_KIND_MESH = 8,
  TUSD_SHAPE_KIND_SPHERE_POINTS = 9,
  TUSD_SHAPE_KIND_CUSTOM = 10,
} TusdShapeKind;

typedef enum TusdAxis {
  TUSD_AXIS_X = 0,
  TUSD_AXIS_Y = 1,
  TUSD_AXIS_Z = 2,
} TusdAxis;

typedef enum TusdJointKind {
  TUSD_JOINT_KIND_FIXED = 0,
  TUSD_JOINT_KIND_REVOLUTE = 1,
  TUSD_JOINT_KIND_PRISMATIC = 2,
  TUSD_JOINT_KIND_SPHERICAL = 3,
  TUSD_JOINT_KIND_DISTANCE = 4,
  TUSD_JOINT_KIND_D6 = 5,
  TUSD_JOINT_KIND_CUSTOM = 6,
} TusdJointKind;

typedef enum TusdDof {
  TUSD_DOF_DISTANCE = 0,
  TUSD_DOF_TRANS_X = 1,
  TUSD_DOF_TRANS_Y = 2,
  TUSD_DOF_TRANS_Z = 3,
  TUSD_DOF_ROT_X = 4,
  TUSD_DOF_ROT_Y = 5,
  TUSD_DOF_ROT_Z = 6,
} TusdDof;

typedef enum TusdFeature {
  TUSD_FEATURE_GEOM = (1 << 0),
  TUSD_FEATURE_SHADE = (1 << 1),
  TUSD_FEATURE_LUX = (1 << 2),
  TUSD_FEATURE_SKEL = (1 << 3),
  TUSD_FEATURE_PHYSICS = (1 << 4),
  TUSD_FEATURE_VOL = (1 << 5),
  TUSD_FEATURE_MEDIA = (1 << 6),
  TUSD_FEATURE_SEMANTICS = (1 << 7),
} TusdFeature;

typedef enum TusdTraverseFlags {
  TUSD_TRAVERSE_FLAGS_DEFAULT = 0,
  TUSD_TRAVERSE_FLAGS_INCLUDE_INACTIVE = (1 << 0),
  TUSD_TRAVERSE_FLAGS_INCLUDE_UNLOADED = (1 << 1),
  TUSD_TRAVERSE_FLAGS_INCLUDE_UNDEFINED = (1 << 2),
  TUSD_TRAVERSE_FLAGS_INCLUDE_ABSTRACT = (1 << 3),
  TUSD_TRAVERSE_FLAGS_INSTANCE_PROXIES = (1 << 4),
} TusdTraverseFlags;

typedef enum TusdPrimFlags {
  TUSD_PRIM_FLAGS_ACTIVE = (1 << 0),
  TUSD_PRIM_FLAGS_LOADED = (1 << 1),
  TUSD_PRIM_FLAGS_DEFINED = (1 << 2),
  TUSD_PRIM_FLAGS_ABSTRACT = (1 << 3),
  TUSD_PRIM_FLAGS_MODEL = (1 << 4),
  TUSD_PRIM_FLAGS_INSTANCEABLE = (1 << 5),
  TUSD_PRIM_FLAGS_INSTANCE = (1 << 6),
  TUSD_PRIM_FLAGS_INSTANCE_PROXY = (1 << 7),
  TUSD_PRIM_FLAGS_PROTOTYPE = (1 << 8),
  TUSD_PRIM_FLAGS_IN_PROTOTYPE = (1 << 9),
} TusdPrimFlags;

typedef struct TusdBlendShapes TusdBlendShapes;

typedef struct TusdInstances TusdInstances;

typedef struct TusdOutput TusdOutput;

typedef struct TusdPhysics TusdPhysics;

typedef struct TusdPrimList TusdPrimList;

typedef struct TusdSkelBindings TusdSkelBindings;

typedef struct TusdSkeletonPose TusdSkeletonPose;

typedef struct TusdSkinBinding TusdSkinBinding;

typedef struct TusdStage TusdStage;

typedef struct TusdStringList TusdStringList;

typedef struct TusdValue TusdValue;

typedef struct TusdResolverCallbacks {
  uint32_t struct_size;
  void *user_data;
  bool (*resolve)(void *user_data,
                  const char *asset_path,
                  const char *anchor,
                  struct TusdOutput *out);
  bool (*read)(void *user_data, const char *resolved_path, struct TusdOutput *out);
  void (*release)(void *user_data);
} TusdResolverCallbacks;

typedef struct TusdOpenOptions {
  uint32_t struct_size;
  bool load_payloads;
  const struct TusdResolverCallbacks *resolver;
  const char *const *schema_paths;
  size_t schema_path_count;
} TusdOpenOptions;

typedef uint32_t TusdPrim;

typedef struct TusdTimeRange {
  double start_time_code;
  double end_time_code;
  double time_codes_per_second;
  double frames_per_second;
} TusdTimeRange;

typedef struct TusdAttributeInfo {
  const char *type_name;
  bool uniform;
  bool custom;
  bool has_value;
  bool has_authored_value;
  bool might_be_time_varying;
} TusdAttributeInfo;

typedef struct TusdPrimvarInfo {
  const char *name;
  const char *type_name;
  enum TusdInterpolation interpolation;
  int32_t element_size;
  bool indexed;
  TusdPrim source;
} TusdPrimvarInfo;

typedef struct TusdInstancesView {
  const TusdPrim *prototypes;
  size_t prototype_count;
  const int32_t *proto_indices;
  const int64_t *ids;
  const double *transforms;
  size_t count;
} TusdInstancesView;

typedef struct TusdSkelBindingView {
  TusdPrim skeleton;
  bool has_animation;
  TusdPrim animation;
  const TusdPrim *skinned;
  size_t skinned_count;
} TusdSkelBindingView;

typedef struct TusdSkeletonPoseView {
  size_t joint_count;
  const char *const *joints;
  const int32_t *parents;
  const double *bind;
  size_t bind_count;
  const double *rest_local;
  size_t rest_local_count;
  const double *local;
  size_t local_count;
  const double *skel;
  size_t skel_count;
  const double *skinning;
  size_t skinning_count;
} TusdSkeletonPoseView;

typedef struct TusdSkinBindingView {
  TusdPrim skeleton;
  const int32_t *joint_indices;
  const float *joint_weights;
  size_t influence_count;
  uint32_t influences_per_point;
  bool rigid;
  const double *geom_bind_transform;
  enum TusdSkinningMethod skinning_method;
  const char *const *blend_shapes;
  const float *blend_shape_weights;
  size_t blend_shape_count;
} TusdSkinBindingView;

typedef struct TusdSubShapeView {
  uint32_t blend_shape;
  float weight;
  const float *point_offsets;
  size_t point_offset_count;
  const float *normal_offsets;
  size_t normal_offset_count;
} TusdSubShapeView;

typedef struct TusdPhysicsOptions {
  uint32_t struct_size;
  const TusdPrim *include;
  size_t include_count;
  const TusdPrim *exclude;
  size_t exclude_count;
  bool compute_mass;
  bool si_units;
} TusdPhysicsOptions;

typedef struct TusdPhysicsSceneView {
  TusdPrim prim;
  float gravity_direction[3];
  float gravity_magnitude;
} TusdPhysicsSceneView;

typedef struct TusdCollisionGroupView {
  TusdPrim prim;
  bool invert_filtered_groups;
  const TusdPrim *filtered_groups;
  size_t filtered_group_count;
  const char *merge_group_name;
  const TusdPrim *merged_groups;
  size_t merged_group_count;
} TusdCollisionGroupView;

typedef struct TusdPhysicsMaterialView {
  TusdPrim prim;
  float static_friction;
  float dynamic_friction;
  float restitution;
  float density;
} TusdPhysicsMaterialView;

typedef struct TusdShapeView {
  TusdPrim prim;
  TusdPrim rigid_body;
  float local_pos[3];
  float local_rot[4];
  float local_scale[3];
  const TusdPrim *materials;
  size_t material_count;
  const TusdPrim *simulation_owners;
  size_t simulation_owner_count;
  const TusdPrim *filtered_collisions;
  size_t filtered_collision_count;
  const TusdPrim *collision_groups;
  size_t collision_group_count;
  bool collision_enabled;
  enum TusdShapeKind kind;
  float radius;
  float top_radius;
  float bottom_radius;
  float half_height;
  float half_extents[3];
  enum TusdAxis axis;
  const char *mesh_approximation;
  float mesh_scale[3];
  bool double_sided;
  const float *sphere_points;
  size_t sphere_point_count;
  const char *custom_type;
} TusdShapeView;

typedef struct TusdRigidBodyView {
  TusdPrim prim;
  const TusdPrim *collisions;
  size_t collision_count;
  const TusdPrim *filtered_collisions;
  size_t filtered_collision_count;
  const TusdPrim *simulation_owners;
  size_t simulation_owner_count;
  float position[3];
  float rotation[4];
  float scale[3];
  bool enabled;
  bool kinematic;
  bool starts_asleep;
  float linear_velocity[3];
  float angular_velocity[3];
  bool has_mass;
  double mass;
  double diagonal_inertia[3];
  double center_of_mass[3];
  double principal_axes[4];
} TusdRigidBodyView;

typedef struct TusdJointLimit {
  enum TusdDof dof;
  bool enabled;
  float lower;
  float upper;
} TusdJointLimit;

typedef struct TusdJointDrive {
  enum TusdDof dof;
  float target_position;
  float target_velocity;
  float force_limit;
  float stiffness;
  float damping;
  bool acceleration;
} TusdJointDrive;

typedef struct TusdJointView {
  TusdPrim prim;
  enum TusdJointKind kind;
  const char *type_name;
  TusdPrim rel0;
  TusdPrim rel1;
  TusdPrim body0;
  TusdPrim body1;
  float local_pos0[3];
  float local_rot0[4];
  float local_pos1[3];
  float local_rot1[4];
  bool enabled;
  bool collision_enabled;
  float break_force;
  float break_torque;
  bool exclude_from_articulation;
  enum TusdAxis axis;
  const struct TusdJointLimit *limits;
  size_t limit_count;
  const struct TusdJointDrive *drives;
  size_t drive_count;
  bool min_distance_enabled;
  bool max_distance_enabled;
} TusdJointView;

typedef struct TusdArticulationView {
  TusdPrim prim;
  const TusdPrim *roots;
  size_t root_count;
  const TusdPrim *filtered_collisions;
  size_t filtered_collision_count;
  const TusdPrim *joints;
  size_t joint_count;
  const TusdPrim *bodies;
  size_t body_count;
} TusdArticulationView;

#define TUSD_NO_PRIM UINT32_MAX

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

uint32_t tusd_abi_version(void);

const char *tusd_version_string(void);

uint32_t tusd_features(void);

bool tusd_has_feature(uint32_t feature);

const char *tusd_last_error(void);

void tusd_open_options_init(struct TusdOpenOptions *options);

enum TusdStatus tusd_stage_open(const char *path,
                                const struct TusdOpenOptions *options,
                                struct TusdStage **out_stage);

void tusd_output_set_bytes(struct TusdOutput *out, const void *data, size_t size);

void tusd_output_set_string(struct TusdOutput *out, const char *value);

void tusd_output_set_error(struct TusdOutput *out, const char *message);

enum TusdStatus tusd_stage_load(struct TusdStage *stage, TusdPrim p, bool with_descendants);

enum TusdStatus tusd_stage_unload(struct TusdStage *stage, TusdPrim p);

enum TusdStatus tusd_stage_loadable_prims(struct TusdStage *stage,
                                          TusdPrim root,
                                          struct TusdPrimList *out);

void tusd_stage_free(struct TusdStage *stage);

TusdPrim tusd_stage_pseudo_root(void);

enum TusdStatus tusd_stage_prim_at_path(struct TusdStage *stage,
                                        const char *path,
                                        TusdPrim *out_prim,
                                        bool *out_found);

enum TusdStatus tusd_stage_default_prim(struct TusdStage *stage,
                                        TusdPrim *out_prim,
                                        bool *out_found);

enum TusdStatus tusd_stage_traverse(struct TusdStage *stage,
                                    TusdPrim root,
                                    uint32_t flags_bits,
                                    struct TusdPrimList *out);

enum TusdStatus tusd_stage_prototypes(struct TusdStage *stage, struct TusdPrimList *out);

enum TusdStatus tusd_stage_metadata(struct TusdStage *stage,
                                    const char *key,
                                    struct TusdValue *out_value,
                                    bool *out_found);

enum TusdStatus tusd_stage_has_authored_metadata(struct TusdStage *stage,
                                                 const char *key,
                                                 bool *out);

enum TusdStatus tusd_stage_time_range(struct TusdStage *stage, struct TusdTimeRange *out);

enum TusdStatus tusd_stage_layer_identifiers(struct TusdStage *stage, struct TusdStringList *out);

enum TusdStatus tusd_stage_composition_errors(struct TusdStage *stage, struct TusdStringList *out);

enum TusdStatus tusd_stage_read_asset(struct TusdStage *stage,
                                      const char *resolved_path,
                                      struct TusdValue *out_bytes);

const char *tusd_prim_path(struct TusdStage *stage, TusdPrim p);

const char *tusd_prim_name(struct TusdStage *stage, TusdPrim p);

const char *tusd_prim_type_name(struct TusdStage *stage, TusdPrim p);

const char *tusd_prim_kind(struct TusdStage *stage, TusdPrim p);

enum TusdStatus tusd_prim_specifier(struct TusdStage *stage, TusdPrim p, enum TusdSpecifier *out);

enum TusdStatus tusd_prim_flags(struct TusdStage *stage, TusdPrim p, uint32_t *out);

enum TusdStatus tusd_prim_parent(struct TusdStage *stage,
                                 TusdPrim p,
                                 TusdPrim *out_prim,
                                 bool *out_found);

enum TusdStatus tusd_prim_children(struct TusdStage *stage,
                                   TusdPrim p,
                                   uint32_t flags_bits,
                                   struct TusdPrimList *out);

enum TusdStatus tusd_prim_prototype(struct TusdStage *stage,
                                    TusdPrim p,
                                    TusdPrim *out_prim,
                                    bool *out_found);

enum TusdStatus tusd_prim_instances(struct TusdStage *stage, TusdPrim p, struct TusdPrimList *out);

enum TusdStatus tusd_prim_is_a(struct TusdStage *stage, TusdPrim p, const char *schema, bool *out);

enum TusdStatus tusd_prim_has_api(struct TusdStage *stage,
                                  TusdPrim p,
                                  const char *schema,
                                  bool *out);

enum TusdStatus tusd_prim_applied_schemas(struct TusdStage *stage,
                                          TusdPrim p,
                                          struct TusdStringList *out);

enum TusdStatus tusd_prim_attribute_names(struct TusdStage *stage,
                                          TusdPrim p,
                                          bool authored_only,
                                          struct TusdStringList *out);

enum TusdStatus tusd_prim_relationship_names(struct TusdStage *stage,
                                             TusdPrim p,
                                             bool authored_only,
                                             struct TusdStringList *out);

enum TusdStatus tusd_prim_metadata(struct TusdStage *stage,
                                   TusdPrim p,
                                   const char *key,
                                   struct TusdValue *out_value,
                                   bool *out_found);

enum TusdStatus tusd_attribute_get(struct TusdStage *stage,
                                   TusdPrim p,
                                   const char *name,
                                   const double *time,
                                   struct TusdValue *out_value,
                                   bool *out_has_value);

enum TusdStatus tusd_attribute_info(struct TusdStage *stage,
                                    TusdPrim p,
                                    const char *name,
                                    struct TusdAttributeInfo *out_info,
                                    bool *out_found);

enum TusdStatus tusd_attribute_time_samples(struct TusdStage *stage,
                                            TusdPrim p,
                                            const char *name,
                                            struct TusdValue *out_times);

enum TusdStatus tusd_attribute_connections(struct TusdStage *stage,
                                           TusdPrim p,
                                           const char *name,
                                           struct TusdStringList *out);

enum TusdStatus tusd_attribute_metadata(struct TusdStage *stage,
                                        TusdPrim p,
                                        const char *name,
                                        const char *key,
                                        struct TusdValue *out_value,
                                        bool *out_found);

enum TusdStatus tusd_relationship_targets(struct TusdStage *stage,
                                          TusdPrim p,
                                          const char *name,
                                          bool forwarded,
                                          struct TusdStringList *out);

enum TusdStatus tusd_prim_world_transform(struct TusdStage *stage,
                                          TusdPrim p,
                                          const double *time,
                                          double *out_matrix);

enum TusdStatus tusd_prim_local_transform(struct TusdStage *stage,
                                          TusdPrim p,
                                          const double *time,
                                          double *out_matrix,
                                          bool *out_resets_xform_stack);

enum TusdStatus tusd_prim_primvar_names(struct TusdStage *stage,
                                        TusdPrim p,
                                        struct TusdStringList *out);

enum TusdStatus tusd_prim_primvar(struct TusdStage *stage,
                                  TusdPrim p,
                                  const char *name,
                                  const double *time,
                                  struct TusdValue *out_value,
                                  struct TusdPrimvarInfo *out_info,
                                  bool *out_found);

enum TusdStatus tusd_prim_bound_material(struct TusdStage *stage,
                                         TusdPrim p,
                                         const char *purpose,
                                         TusdPrim *out_material,
                                         const char **out_binding,
                                         bool *out_found);

struct TusdValue *tusd_value_new(void);

void tusd_value_free(struct TusdValue *value);

enum TusdValueType tusd_value_type(const struct TusdValue *value);

bool tusd_value_is_array(const struct TusdValue *value);

size_t tusd_value_len(const struct TusdValue *value);

const void *tusd_value_data(const struct TusdValue *value);

size_t tusd_value_data_size(const struct TusdValue *value);

const char *tusd_value_type_name(const struct TusdValue *value);

const char *tusd_value_string(const struct TusdValue *value, size_t index);

const char *tusd_value_resolved_path(const struct TusdValue *value, size_t index);

struct TusdPrimList *tusd_prim_list_new(void);

void tusd_prim_list_free(struct TusdPrimList *list);

size_t tusd_prim_list_len(const struct TusdPrimList *list);

const TusdPrim *tusd_prim_list_data(const struct TusdPrimList *list);

struct TusdStringList *tusd_string_list_new(void);

void tusd_string_list_free(struct TusdStringList *list);

size_t tusd_string_list_len(const struct TusdStringList *list);

const char *tusd_string_list_get(const struct TusdStringList *list, size_t index);

struct TusdInstances *tusd_instances_new(void);

void tusd_instances_free(struct TusdInstances *instances);

enum TusdStatus tusd_prim_instancer_instances(struct TusdStage *stage,
                                              TusdPrim p,
                                              const double *time,
                                              bool include_prototype_transform,
                                              bool apply_visibility,
                                              struct TusdInstances *out,
                                              bool *out_found);

enum TusdStatus tusd_instances_view(const struct TusdInstances *instances,
                                    struct TusdInstancesView *out);

struct TusdSkelBindings *tusd_skel_bindings_new(void);

void tusd_skel_bindings_free(struct TusdSkelBindings *bindings);

enum TusdStatus tusd_stage_skel_bindings(struct TusdStage *stage,
                                         TusdPrim root,
                                         struct TusdSkelBindings *out);

size_t tusd_skel_bindings_len(const struct TusdSkelBindings *bindings);

enum TusdStatus tusd_skel_bindings_get(const struct TusdSkelBindings *bindings,
                                       size_t index,
                                       struct TusdSkelBindingView *out);

struct TusdSkeletonPose *tusd_skeleton_pose_new(void);

void tusd_skeleton_pose_free(struct TusdSkeletonPose *pose);

enum TusdStatus tusd_prim_skeleton_pose(struct TusdStage *stage,
                                        TusdPrim p,
                                        const double *time,
                                        struct TusdSkeletonPose *out,
                                        bool *out_found);

enum TusdStatus tusd_skeleton_pose_view(const struct TusdSkeletonPose *pose,
                                        struct TusdSkeletonPoseView *out);

struct TusdSkinBinding *tusd_skin_binding_new(void);

void tusd_skin_binding_free(struct TusdSkinBinding *binding);

enum TusdStatus tusd_prim_skin_binding(struct TusdStage *stage,
                                       TusdPrim p,
                                       const double *time,
                                       struct TusdSkinBinding *out,
                                       bool *out_found);

enum TusdStatus tusd_skin_binding_view(const struct TusdSkinBinding *binding,
                                       struct TusdSkinBindingView *out);

struct TusdBlendShapes *tusd_blend_shapes_new(void);

void tusd_blend_shapes_free(struct TusdBlendShapes *shapes);

enum TusdStatus tusd_prim_blend_shapes(struct TusdStage *stage,
                                       TusdPrim p,
                                       const double *time,
                                       struct TusdBlendShapes *out,
                                       bool *out_found);

size_t tusd_blend_shapes_count(const struct TusdBlendShapes *shapes);

const int32_t *tusd_blend_shapes_point_indices(const struct TusdBlendShapes *shapes,
                                               size_t blend_shape,
                                               size_t *out_count);

size_t tusd_blend_shapes_sub_shape_count(const struct TusdBlendShapes *shapes);

enum TusdStatus tusd_blend_shapes_sub_shape(const struct TusdBlendShapes *shapes,
                                            size_t index,
                                            struct TusdSubShapeView *out);

enum TusdStatus tusd_prim_skinned_points(struct TusdStage *stage,
                                         TusdPrim p,
                                         const double *time,
                                         struct TusdValue *out_points,
                                         bool *out_found);

struct TusdPhysics *tusd_physics_new(void);

void tusd_physics_free(struct TusdPhysics *physics);

void tusd_physics_options_init(struct TusdPhysicsOptions *options);

enum TusdStatus tusd_stage_physics(struct TusdStage *stage,
                                   const struct TusdPhysicsOptions *options,
                                   struct TusdPhysics *out);

enum TusdStatus tusd_physics_units(const struct TusdPhysics *physics,
                                   double *out_meters_per_unit,
                                   double *out_kilograms_per_unit,
                                   bool *out_si_units);

size_t tusd_physics_count(const struct TusdPhysics *physics, enum TusdPhysicsKind kind);

enum TusdStatus tusd_physics_scene(const struct TusdPhysics *physics,
                                   size_t index,
                                   struct TusdPhysicsSceneView *out);

enum TusdStatus tusd_physics_collision_group(const struct TusdPhysics *physics,
                                             size_t index,
                                             struct TusdCollisionGroupView *out);

enum TusdStatus tusd_physics_material(const struct TusdPhysics *physics,
                                      size_t index,
                                      struct TusdPhysicsMaterialView *out);

enum TusdStatus tusd_physics_shape(const struct TusdPhysics *physics,
                                   size_t index,
                                   struct TusdShapeView *out);

enum TusdStatus tusd_physics_rigid_body(const struct TusdPhysics *physics,
                                        size_t index,
                                        struct TusdRigidBodyView *out);

enum TusdStatus tusd_physics_joint(const struct TusdPhysics *physics,
                                   size_t index,
                                   struct TusdJointView *out);

enum TusdStatus tusd_physics_articulation(const struct TusdPhysics *physics,
                                          size_t index,
                                          struct TusdArticulationView *out);

const char *tusd_physics_diagnostic(const struct TusdPhysics *physics, size_t index);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* TINYUSD_H */
