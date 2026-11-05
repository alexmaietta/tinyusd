#ifndef TINYUSD_HPP
#define TINYUSD_HPP

#include <tinyusd/tinyusd.h>

#include <array>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>
#include <vector>

namespace tinyusd {

template <class T>
class Span {
public:
    constexpr Span() noexcept = default;
    constexpr Span(const T* data, std::size_t size) noexcept : data_(data), size_(size) {}

    constexpr const T* data() const noexcept { return data_; }
    constexpr std::size_t size() const noexcept { return size_; }
    constexpr bool empty() const noexcept { return size_ == 0; }
    constexpr const T* begin() const noexcept { return data_; }
    constexpr const T* end() const noexcept { return data_ + size_; }
    constexpr const T& operator[](std::size_t i) const noexcept { return data_[i]; }

private:
    const T* data_ = nullptr;
    std::size_t size_ = 0;
};

enum class Status : int {
    Ok = TUSD_STATUS_OK,
    InvalidArgument = TUSD_STATUS_INVALID_ARGUMENT,
    OpenFailed = TUSD_STATUS_OPEN_FAILED,
    InvalidPath = TUSD_STATUS_INVALID_PATH,
    InvalidPrim = TUSD_STATUS_INVALID_PRIM,
    AssetFailed = TUSD_STATUS_ASSET_FAILED,
    UsdError = TUSD_STATUS_USD_ERROR,
    Panic = TUSD_STATUS_PANIC,
    FeatureDisabled = TUSD_STATUS_FEATURE_DISABLED,
    SchemaFailed = TUSD_STATUS_SCHEMA_FAILED,
};

enum class Feature : std::uint32_t {
    Geom = TUSD_FEATURE_GEOM,
    Shade = TUSD_FEATURE_SHADE,
    Lux = TUSD_FEATURE_LUX,
    Skel = TUSD_FEATURE_SKEL,
    Physics = TUSD_FEATURE_PHYSICS,
    Vol = TUSD_FEATURE_VOL,
    Media = TUSD_FEATURE_MEDIA,
    Semantics = TUSD_FEATURE_SEMANTICS,
};

enum class Interpolation : int {
    Constant = TUSD_INTERPOLATION_CONSTANT,
    Uniform = TUSD_INTERPOLATION_UNIFORM,
    Varying = TUSD_INTERPOLATION_VARYING,
    Vertex = TUSD_INTERPOLATION_VERTEX,
    FaceVarying = TUSD_INTERPOLATION_FACE_VARYING,
};

enum class Specifier : int { Def = TUSD_SPECIFIER_DEF, Over = TUSD_SPECIFIER_OVER, Class = TUSD_SPECIFIER_CLASS };

enum class ValueType : int {
    Empty = TUSD_VALUE_TYPE_EMPTY,
    Bool = TUSD_VALUE_TYPE_BOOL,
    UChar = TUSD_VALUE_TYPE_UCHAR,
    Int = TUSD_VALUE_TYPE_INT,
    UInt = TUSD_VALUE_TYPE_UINT,
    Int64 = TUSD_VALUE_TYPE_INT64,
    UInt64 = TUSD_VALUE_TYPE_UINT64,
    Half = TUSD_VALUE_TYPE_HALF,
    Float = TUSD_VALUE_TYPE_FLOAT,
    Double = TUSD_VALUE_TYPE_DOUBLE,
    TimeCode = TUSD_VALUE_TYPE_TIME_CODE,
    String = TUSD_VALUE_TYPE_STRING,
    Token = TUSD_VALUE_TYPE_TOKEN,
    Asset = TUSD_VALUE_TYPE_ASSET,
    Path = TUSD_VALUE_TYPE_PATH,
    Int2 = TUSD_VALUE_TYPE_INT2,
    Int3 = TUSD_VALUE_TYPE_INT3,
    Int4 = TUSD_VALUE_TYPE_INT4,
    Half2 = TUSD_VALUE_TYPE_HALF2,
    Half3 = TUSD_VALUE_TYPE_HALF3,
    Half4 = TUSD_VALUE_TYPE_HALF4,
    Float2 = TUSD_VALUE_TYPE_FLOAT2,
    Float3 = TUSD_VALUE_TYPE_FLOAT3,
    Float4 = TUSD_VALUE_TYPE_FLOAT4,
    Double2 = TUSD_VALUE_TYPE_DOUBLE2,
    Double3 = TUSD_VALUE_TYPE_DOUBLE3,
    Double4 = TUSD_VALUE_TYPE_DOUBLE4,
    Quath = TUSD_VALUE_TYPE_QUATH,
    Quatf = TUSD_VALUE_TYPE_QUATF,
    Quatd = TUSD_VALUE_TYPE_QUATD,
    Matrix2d = TUSD_VALUE_TYPE_MATRIX2D,
    Matrix3d = TUSD_VALUE_TYPE_MATRIX3D,
    Matrix4d = TUSD_VALUE_TYPE_MATRIX4D,
    Unsupported = TUSD_VALUE_TYPE_UNSUPPORTED,
};

enum class Traverse : std::uint32_t {
    Default = TUSD_TRAVERSE_FLAGS_DEFAULT,
    IncludeInactive = TUSD_TRAVERSE_FLAGS_INCLUDE_INACTIVE,
    IncludeUnloaded = TUSD_TRAVERSE_FLAGS_INCLUDE_UNLOADED,
    IncludeUndefined = TUSD_TRAVERSE_FLAGS_INCLUDE_UNDEFINED,
    IncludeAbstract = TUSD_TRAVERSE_FLAGS_INCLUDE_ABSTRACT,
    InstanceProxies = TUSD_TRAVERSE_FLAGS_INSTANCE_PROXIES,
    All = TUSD_TRAVERSE_FLAGS_INCLUDE_INACTIVE | TUSD_TRAVERSE_FLAGS_INCLUDE_UNLOADED |
          TUSD_TRAVERSE_FLAGS_INCLUDE_UNDEFINED | TUSD_TRAVERSE_FLAGS_INCLUDE_ABSTRACT,
};

constexpr Traverse operator|(Traverse a, Traverse b) noexcept {
    return static_cast<Traverse>(static_cast<std::uint32_t>(a) | static_cast<std::uint32_t>(b));
}

enum class PrimFlag : std::uint32_t {
    Active = TUSD_PRIM_FLAGS_ACTIVE,
    Loaded = TUSD_PRIM_FLAGS_LOADED,
    Defined = TUSD_PRIM_FLAGS_DEFINED,
    Abstract = TUSD_PRIM_FLAGS_ABSTRACT,
    Model = TUSD_PRIM_FLAGS_MODEL,
    Instanceable = TUSD_PRIM_FLAGS_INSTANCEABLE,
    Instance = TUSD_PRIM_FLAGS_INSTANCE,
    InstanceProxy = TUSD_PRIM_FLAGS_INSTANCE_PROXY,
    Prototype = TUSD_PRIM_FLAGS_PROTOTYPE,
    InPrototype = TUSD_PRIM_FLAGS_IN_PROTOTYPE,
};

class PrimFlags {
public:
    constexpr explicit PrimFlags(std::uint32_t bits = 0) noexcept : bits_(bits) {}
    constexpr bool has(PrimFlag f) const noexcept { return (bits_ & static_cast<std::uint32_t>(f)) != 0; }
    constexpr std::uint32_t bits() const noexcept { return bits_; }

private:
    std::uint32_t bits_;
};

class Error : public std::runtime_error {
public:
    Error(Status status, const std::string& message) : std::runtime_error(message), status_(status) {}
    Status status() const noexcept { return status_; }

private:
    Status status_;
};

inline std::uint32_t abi_version() noexcept { return tusd_abi_version(); }
inline std::string_view version() noexcept { return tusd_version_string(); }
inline bool has_feature(Feature f) noexcept { return tusd_has_feature(static_cast<std::uint32_t>(f)); }

namespace detail {
inline void check(TusdStatus status) {
    if (status != TUSD_STATUS_OK) {
        throw Error(static_cast<Status>(status), tusd_last_error());
    }
}

inline std::string_view checked(const char* s) {
    if (!s) {
        throw Error(static_cast<Status>(TUSD_STATUS_INVALID_PRIM), tusd_last_error());
    }
    return s;
}

struct ValueDeleter {
    void operator()(TusdValue* v) const noexcept { tusd_value_free(v); }
};
struct StageDeleter {
    void operator()(TusdStage* s) const noexcept { tusd_stage_free(s); }
};
struct PrimListDeleter {
    void operator()(TusdPrimList* l) const noexcept { tusd_prim_list_free(l); }
};
struct StringListDeleter {
    void operator()(TusdStringList* l) const noexcept { tusd_string_list_free(l); }
};

template <class T>
constexpr int scalar_kind() noexcept {
    if constexpr (std::is_same_v<T, std::uint8_t> || std::is_same_v<T, bool>) return 1;
    else if constexpr (std::is_same_v<T, std::int32_t>) return 2;
    else if constexpr (std::is_same_v<T, std::uint32_t>) return 3;
    else if constexpr (std::is_same_v<T, std::int64_t>) return 4;
    else if constexpr (std::is_same_v<T, std::uint64_t>) return 5;
    else if constexpr (std::is_same_v<T, std::uint16_t>) return 6;
    else if constexpr (std::is_same_v<T, float>) return 7;
    else if constexpr (std::is_same_v<T, double>) return 8;
    else return 0;
}

constexpr int scalar_kind_of(ValueType t) noexcept {
    switch (t) {
        case ValueType::Bool:
        case ValueType::UChar: return 1;
        case ValueType::Int:
        case ValueType::Int2:
        case ValueType::Int3:
        case ValueType::Int4: return 2;
        case ValueType::UInt: return 3;
        case ValueType::Int64: return 4;
        case ValueType::UInt64: return 5;
        case ValueType::Half:
        case ValueType::Half2:
        case ValueType::Half3:
        case ValueType::Half4:
        case ValueType::Quath: return 6;
        case ValueType::Float:
        case ValueType::Float2:
        case ValueType::Float3:
        case ValueType::Float4:
        case ValueType::Quatf: return 7;
        case ValueType::Double:
        case ValueType::TimeCode:
        case ValueType::Double2:
        case ValueType::Double3:
        case ValueType::Double4:
        case ValueType::Quatd:
        case ValueType::Matrix2d:
        case ValueType::Matrix3d:
        case ValueType::Matrix4d: return 8;
        default: return 0;
    }
}
}

class Value {
public:
    Value() : value_(tusd_value_new()) {}

    ValueType type() const noexcept { return static_cast<ValueType>(tusd_value_type(value_.get())); }
    bool empty() const noexcept { return type() == ValueType::Empty; }
    bool is_array() const noexcept { return tusd_value_is_array(value_.get()); }
    std::size_t size() const noexcept { return tusd_value_len(value_.get()); }
    std::string_view type_name() const noexcept { return tusd_value_type_name(value_.get()); }
    Span<std::uint8_t> bytes() const noexcept {
        return {static_cast<const std::uint8_t*>(tusd_value_data(value_.get())), tusd_value_data_size(value_.get())};
    }

    template <class T>
    Span<T> as_span() const noexcept {
        static_assert(std::is_trivially_copyable_v<T>, "T must be trivially copyable");
        using Scalar = std::remove_cv_t<std::remove_all_extents_t<T>>;
        constexpr int kind = detail::scalar_kind<Scalar>();
        if constexpr (kind != 0) {
            if (detail::scalar_kind_of(type()) != kind) return {};
        }
        const auto b = bytes();
        if (b.empty() || b.size() % sizeof(T) != 0) return {};
        return {reinterpret_cast<const T*>(b.data()), b.size() / sizeof(T)};
    }

    template <class T>
    std::optional<T> as() const noexcept {
        if (is_array()) return std::nullopt;
        auto s = as_span<T>();
        if (s.empty()) return std::nullopt;
        return s[0];
    }

    std::string_view string(std::size_t i = 0) const noexcept {
        const char* s = tusd_value_string(value_.get(), i);
        return s ? std::string_view(s) : std::string_view();
    }
    std::vector<std::string_view> strings() const {
        std::vector<std::string_view> out;
        for (std::size_t i = 0;; ++i) {
            const char* s = tusd_value_string(value_.get(), i);
            if (!s) break;
            out.emplace_back(s);
        }
        return out;
    }
    std::string_view resolved_path(std::size_t i = 0) const noexcept {
        const char* s = tusd_value_resolved_path(value_.get(), i);
        return s ? std::string_view(s) : std::string_view();
    }

    TusdValue* raw() noexcept { return value_.get(); }
    const TusdValue* raw() const noexcept { return value_.get(); }

private:
    std::unique_ptr<TusdValue, detail::ValueDeleter> value_;
};

class Resolver {
public:
    virtual ~Resolver() = default;
    virtual std::optional<std::string> resolve(std::string_view asset_path,
                                               std::optional<std::string_view> anchor) = 0;
    virtual bool read(std::string_view resolved_path, std::vector<std::uint8_t>& out) = 0;
};

struct OpenOptions {
    bool load_payloads = true;
    std::shared_ptr<Resolver> resolver;
    std::vector<std::string> schema_paths;
};

namespace detail {
inline bool resolve_trampoline(void* user_data, const char* asset_path, const char* anchor, TusdOutput* out) {
    try {
        auto& resolver = *static_cast<std::shared_ptr<Resolver>*>(user_data);
        std::optional<std::string_view> a;
        if (anchor) a = anchor;
        auto resolved = resolver->resolve(asset_path, a);
        if (!resolved) return false;
        tusd_output_set_bytes(out, resolved->data(), resolved->size());
        return true;
    } catch (const std::exception& e) {
        tusd_output_set_error(out, e.what());
    } catch (...) {
    }
    return false;
}

inline bool read_trampoline(void* user_data, const char* resolved_path, TusdOutput* out) {
    try {
        auto& resolver = *static_cast<std::shared_ptr<Resolver>*>(user_data);
        std::vector<std::uint8_t> bytes;
        if (!resolver->read(resolved_path, bytes)) return false;
        tusd_output_set_bytes(out, bytes.data(), bytes.size());
        return true;
    } catch (const std::exception& e) {
        tusd_output_set_error(out, e.what());
    } catch (...) {
    }
    return false;
}

inline void release_trampoline(void* user_data) { delete static_cast<std::shared_ptr<Resolver>*>(user_data); }
}

struct TimeRange {
    double start_time_code;
    double end_time_code;
    double time_codes_per_second;
    double frames_per_second;
};

struct AttributeInfo {
    std::string_view type_name;
    bool uniform;
    bool custom;
    bool has_value;
    bool has_authored_value;
    bool might_be_time_varying;
};

class Stage;
struct PrimvarInfo;
struct LocalTransform;
struct Instances;
struct SkeletonPose;
struct SkinBinding;
struct BlendShapes;
struct SkelBinding;

using Matrix4d = std::array<double, 16>;

class Prim {
public:
    Prim() = default;

    TusdPrim handle() const noexcept { return handle_; }
    bool operator==(const Prim& o) const noexcept { return stage_ == o.stage_ && handle_ == o.handle_; }
    bool operator!=(const Prim& o) const noexcept { return !(*this == o); }

    std::string_view path() const { return detail::checked(tusd_prim_path(stage_, handle_)); }
    std::string_view name() const { return detail::checked(tusd_prim_name(stage_, handle_)); }
    std::string_view type_name() const { return detail::checked(tusd_prim_type_name(stage_, handle_)); }
    std::string_view kind() const { return detail::checked(tusd_prim_kind(stage_, handle_)); }

    Specifier specifier() const {
        TusdSpecifier s;
        detail::check(tusd_prim_specifier(stage_, handle_, &s));
        return static_cast<Specifier>(s);
    }
    PrimFlags flags() const {
        std::uint32_t bits = 0;
        detail::check(tusd_prim_flags(stage_, handle_, &bits));
        return PrimFlags(bits);
    }
    bool is_active() const { return flags().has(PrimFlag::Active); }
    bool is_instance() const { return flags().has(PrimFlag::Instance); }

    bool is_a(const std::string& schema) const {
        bool out = false;
        detail::check(tusd_prim_is_a(stage_, handle_, schema.c_str(), &out));
        return out;
    }
    bool has_api(const std::string& schema) const {
        bool out = false;
        detail::check(tusd_prim_has_api(stage_, handle_, schema.c_str(), &out));
        return out;
    }

    inline std::optional<Prim> parent() const;
    inline std::vector<Prim> children(Traverse flags = Traverse::Default) const;
    inline std::optional<Prim> prototype() const;
    inline std::vector<Prim> instances() const;
    inline std::vector<std::string> applied_schemas() const;
    inline std::vector<std::string> attribute_names(bool authored_only = false) const;
    inline std::vector<std::string> relationship_names(bool authored_only = false) const;
    inline std::vector<std::string> targets(const std::string& relationship, bool forwarded = false) const;
    inline std::vector<std::string> connections(const std::string& attribute) const;
    inline std::vector<double> time_samples(const std::string& attribute) const;

    bool get(const std::string& attribute, Value& out, std::optional<double> time = std::nullopt) const {
        bool has = false;
        const double t = time.value_or(0.0);
        detail::check(tusd_attribute_get(stage_, handle_, attribute.c_str(), time ? &t : nullptr, out.raw(), &has));
        return has;
    }

    template <class T>
    std::optional<T> get(const std::string& attribute, std::optional<double> time = std::nullopt) const {
        Value v;
        if (!get(attribute, v, time)) return std::nullopt;
        return v.as<T>();
    }

    std::optional<AttributeInfo> attribute_info(const std::string& attribute) const {
        TusdAttributeInfo info;
        bool found = false;
        detail::check(tusd_attribute_info(stage_, handle_, attribute.c_str(), &info, &found));
        if (!found) return std::nullopt;
        return AttributeInfo{info.type_name, info.uniform, info.custom, info.has_value, info.has_authored_value,
                             info.might_be_time_varying};
    }

    Matrix4d world_transform(std::optional<double> time = std::nullopt) const {
        Matrix4d m{};
        const double t = time.value_or(0.0);
        detail::check(tusd_prim_world_transform(stage_, handle_, time ? &t : nullptr, m.data()));
        return m;
    }

    inline LocalTransform local_transform(std::optional<double> time = std::nullopt) const;
    inline std::vector<std::string> primvar_names() const;
    inline std::optional<PrimvarInfo> primvar(const std::string& name, Value& out,
                                              std::optional<double> time = std::nullopt) const;
    inline std::optional<Prim> bound_material(const std::string& purpose = "") const;
    inline std::optional<Instances> instancer_instances(std::optional<double> time = std::nullopt,
                                                        bool include_prototype_transform = true,
                                                        bool apply_visibility = true) const;
    inline std::optional<SkeletonPose> skeleton_pose(std::optional<double> time = std::nullopt) const;
    inline std::optional<SkinBinding> skin_binding(std::optional<double> time = std::nullopt) const;
    inline std::optional<BlendShapes> blend_shapes(std::optional<double> time = std::nullopt) const;
    inline bool skinned_points(Value& out, std::optional<double> time = std::nullopt) const;

    bool metadata(const std::string& key, Value& out) const {
        bool found = false;
        detail::check(tusd_prim_metadata(stage_, handle_, key.c_str(), out.raw(), &found));
        return found;
    }

    bool attribute_metadata(const std::string& attribute, const std::string& key, Value& out) const {
        bool found = false;
        detail::check(tusd_attribute_metadata(stage_, handle_, attribute.c_str(), key.c_str(), out.raw(), &found));
        return found;
    }

private:
    friend class Stage;
    Prim(TusdStage* stage, TusdPrim handle) : stage_(stage), handle_(handle) {}

    static std::vector<Prim> from_list(TusdStage* stage, const TusdPrimList* list) {
        std::vector<Prim> out;
        const TusdPrim* data = tusd_prim_list_data(list);
        const std::size_t n = tusd_prim_list_len(list);
        out.reserve(n);
        for (std::size_t i = 0; i < n; ++i) out.push_back(Prim(stage, data[i]));
        return out;
    }

    static std::vector<std::string> from_list(const TusdStringList* list) {
        std::vector<std::string> out;
        const std::size_t n = tusd_string_list_len(list);
        out.reserve(n);
        for (std::size_t i = 0; i < n; ++i) out.emplace_back(tusd_string_list_get(list, i));
        return out;
    }

    template <class F>
    std::vector<Prim> prims(F&& f) const {
        std::unique_ptr<TusdPrimList, detail::PrimListDeleter> list(tusd_prim_list_new());
        detail::check(f(list.get()));
        return from_list(stage_, list.get());
    }

    template <class F>
    std::vector<std::string> strings(F&& f) const {
        std::unique_ptr<TusdStringList, detail::StringListDeleter> list(tusd_string_list_new());
        detail::check(f(list.get()));
        return from_list(list.get());
    }

    template <class F>
    std::optional<Prim> optional_prim(F&& f) const {
        TusdPrim p = 0;
        bool found = false;
        detail::check(f(&p, &found));
        if (!found) return std::nullopt;
        return Prim(stage_, p);
    }

    TusdStage* stage_ = nullptr;
    TusdPrim handle_ = 0;
};

struct LocalTransform {
    Matrix4d matrix;
    bool resets_xform_stack;
};

struct PrimvarInfo {
    std::string_view name;
    std::string_view type_name;
    Interpolation interpolation;
    int element_size;
    bool indexed;
    Prim source;
};

inline LocalTransform Prim::local_transform(std::optional<double> time) const {
    LocalTransform out{};
    const double t = time.value_or(0.0);
    detail::check(
        tusd_prim_local_transform(stage_, handle_, time ? &t : nullptr, out.matrix.data(), &out.resets_xform_stack));
    return out;
}

inline std::vector<std::string> Prim::primvar_names() const {
    return strings([&](TusdStringList* l) { return tusd_prim_primvar_names(stage_, handle_, l); });
}

inline std::optional<PrimvarInfo> Prim::primvar(const std::string& name, Value& out,
                                                std::optional<double> time) const {
    TusdPrimvarInfo info;
    bool found = false;
    const double t = time.value_or(0.0);
    detail::check(tusd_prim_primvar(stage_, handle_, name.c_str(), time ? &t : nullptr, out.raw(), &info, &found));
    if (!found) return std::nullopt;
    return PrimvarInfo{info.name,         info.type_name, static_cast<Interpolation>(info.interpolation),
                       info.element_size, info.indexed,   Prim(stage_, info.source)};
}

inline std::optional<Prim> Prim::bound_material(const std::string& purpose) const {
    return optional_prim([&](TusdPrim* p, bool* f) {
        return tusd_prim_bound_material(stage_, handle_, purpose.c_str(), p, nullptr, f);
    });
}

struct Instances {
    std::vector<Prim> prototypes;
    std::vector<std::int32_t> proto_indices;
    std::vector<std::int64_t> ids;
    std::vector<Matrix4d> transforms;
};

struct SkelBinding {
    Prim skeleton;
    std::optional<Prim> animation;
    std::vector<Prim> skinned;
};

struct SkeletonPose {
    std::vector<std::string> joints;
    std::vector<std::int32_t> parents;
    std::vector<Matrix4d> bind;
    std::vector<Matrix4d> rest_local;
    std::vector<Matrix4d> local;
    std::vector<Matrix4d> skel;
    std::vector<Matrix4d> skinning;
};

enum class SkinningMethod : int {
    ClassicLinear = TUSD_SKINNING_METHOD_CLASSIC_LINEAR,
    DualQuaternion = TUSD_SKINNING_METHOD_DUAL_QUATERNION,
};

struct SkinBinding {
    Prim skeleton;
    std::vector<std::int32_t> joint_indices;
    std::vector<float> joint_weights;
    std::uint32_t influences_per_point;
    bool rigid;
    Matrix4d geom_bind_transform;
    SkinningMethod skinning_method;
    std::vector<std::string> blend_shapes;
    std::vector<float> blend_shape_weights;
};

struct SubShape {
    std::uint32_t blend_shape;
    float weight;
    std::vector<std::array<float, 3>> point_offsets;
    std::vector<std::array<float, 3>> normal_offsets;
};

struct BlendShapes {
    std::vector<std::vector<std::int32_t>> point_indices;
    std::vector<SubShape> sub_shapes;
};

namespace detail {
struct InstancesDeleter {
    void operator()(TusdInstances* p) const noexcept { tusd_instances_free(p); }
};
struct SkeletonPoseDeleter {
    void operator()(TusdSkeletonPose* p) const noexcept { tusd_skeleton_pose_free(p); }
};
struct SkinBindingDeleter {
    void operator()(TusdSkinBinding* p) const noexcept { tusd_skin_binding_free(p); }
};
struct BlendShapesDeleter {
    void operator()(TusdBlendShapes* p) const noexcept { tusd_blend_shapes_free(p); }
};
struct SkelBindingsDeleter {
    void operator()(TusdSkelBindings* p) const noexcept { tusd_skel_bindings_free(p); }
};

inline std::vector<Matrix4d> matrices(const double* data, std::size_t count) {
    std::vector<Matrix4d> out(count);
    for (std::size_t i = 0; i < count; ++i) {
        for (std::size_t k = 0; k < 16; ++k) out[i][k] = data[i * 16 + k];
    }
    return out;
}

inline std::vector<std::array<float, 3>> vec3s(const float* data, std::size_t count) {
    std::vector<std::array<float, 3>> out(count);
    for (std::size_t i = 0; i < count; ++i) out[i] = {data[i * 3], data[i * 3 + 1], data[i * 3 + 2]};
    return out;
}
}

inline std::optional<Instances> Prim::instancer_instances(std::optional<double> time, bool include_prototype_transform,
                                                          bool apply_visibility) const {
    std::unique_ptr<TusdInstances, detail::InstancesDeleter> raw(tusd_instances_new());
    bool found = false;
    const double t = time.value_or(0.0);
    detail::check(tusd_prim_instancer_instances(stage_, handle_, time ? &t : nullptr, include_prototype_transform,
                                                apply_visibility, raw.get(), &found));
    if (!found) return std::nullopt;
    TusdInstancesView v;
    detail::check(tusd_instances_view(raw.get(), &v));
    Instances out;
    for (std::size_t i = 0; i < v.prototype_count; ++i) out.prototypes.push_back(Prim(stage_, v.prototypes[i]));
    out.proto_indices.assign(v.proto_indices, v.proto_indices + v.count);
    out.ids.assign(v.ids, v.ids + v.count);
    out.transforms = detail::matrices(v.transforms, v.count);
    return out;
}

inline std::optional<SkeletonPose> Prim::skeleton_pose(std::optional<double> time) const {
    std::unique_ptr<TusdSkeletonPose, detail::SkeletonPoseDeleter> raw(tusd_skeleton_pose_new());
    bool found = false;
    const double t = time.value_or(0.0);
    detail::check(tusd_prim_skeleton_pose(stage_, handle_, time ? &t : nullptr, raw.get(), &found));
    if (!found) return std::nullopt;
    TusdSkeletonPoseView v;
    detail::check(tusd_skeleton_pose_view(raw.get(), &v));
    SkeletonPose out;
    out.joints.assign(v.joints, v.joints + v.joint_count);
    out.parents.assign(v.parents, v.parents + v.joint_count);
    out.bind = detail::matrices(v.bind, v.bind_count);
    out.rest_local = detail::matrices(v.rest_local, v.rest_local_count);
    out.local = detail::matrices(v.local, v.local_count);
    out.skel = detail::matrices(v.skel, v.skel_count);
    out.skinning = detail::matrices(v.skinning, v.skinning_count);
    return out;
}

inline std::optional<SkinBinding> Prim::skin_binding(std::optional<double> time) const {
    std::unique_ptr<TusdSkinBinding, detail::SkinBindingDeleter> raw(tusd_skin_binding_new());
    bool found = false;
    const double t = time.value_or(0.0);
    detail::check(tusd_prim_skin_binding(stage_, handle_, time ? &t : nullptr, raw.get(), &found));
    if (!found) return std::nullopt;
    TusdSkinBindingView v;
    detail::check(tusd_skin_binding_view(raw.get(), &v));
    SkinBinding out{Prim(stage_, v.skeleton), {}, {}, v.influences_per_point, v.rigid, {},
                    static_cast<SkinningMethod>(v.skinning_method), {}, {}};
    out.joint_indices.assign(v.joint_indices, v.joint_indices + v.influence_count);
    out.joint_weights.assign(v.joint_weights, v.joint_weights + v.influence_count);
    for (std::size_t k = 0; k < 16; ++k) out.geom_bind_transform[k] = v.geom_bind_transform[k];
    out.blend_shapes.assign(v.blend_shapes, v.blend_shapes + v.blend_shape_count);
    out.blend_shape_weights.assign(v.blend_shape_weights, v.blend_shape_weights + v.blend_shape_count);
    return out;
}

inline std::optional<BlendShapes> Prim::blend_shapes(std::optional<double> time) const {
    std::unique_ptr<TusdBlendShapes, detail::BlendShapesDeleter> raw(tusd_blend_shapes_new());
    bool found = false;
    const double t = time.value_or(0.0);
    detail::check(tusd_prim_blend_shapes(stage_, handle_, time ? &t : nullptr, raw.get(), &found));
    if (!found) return std::nullopt;
    BlendShapes out;
    for (std::size_t i = 0; i < tusd_blend_shapes_count(raw.get()); ++i) {
        std::size_t n = 0;
        const std::int32_t* indices = tusd_blend_shapes_point_indices(raw.get(), i, &n);
        out.point_indices.emplace_back(indices, indices + n);
    }
    for (std::size_t i = 0; i < tusd_blend_shapes_sub_shape_count(raw.get()); ++i) {
        TusdSubShapeView v;
        detail::check(tusd_blend_shapes_sub_shape(raw.get(), i, &v));
        out.sub_shapes.push_back({v.blend_shape, v.weight, detail::vec3s(v.point_offsets, v.point_offset_count),
                                  detail::vec3s(v.normal_offsets, v.normal_offset_count)});
    }
    return out;
}

inline bool Prim::skinned_points(Value& out, std::optional<double> time) const {
    bool found = false;
    const double t = time.value_or(0.0);
    detail::check(tusd_prim_skinned_points(stage_, handle_, time ? &t : nullptr, out.raw(), &found));
    return found;
}

inline std::optional<Prim> Prim::parent() const {
    return optional_prim([&](TusdPrim* p, bool* f) { return tusd_prim_parent(stage_, handle_, p, f); });
}
inline std::vector<Prim> Prim::children(Traverse flags) const {
    return prims([&](TusdPrimList* l) {
        return tusd_prim_children(stage_, handle_, static_cast<std::uint32_t>(flags), l);
    });
}
inline std::optional<Prim> Prim::prototype() const {
    return optional_prim([&](TusdPrim* p, bool* f) { return tusd_prim_prototype(stage_, handle_, p, f); });
}
inline std::vector<Prim> Prim::instances() const {
    return prims([&](TusdPrimList* l) { return tusd_prim_instances(stage_, handle_, l); });
}
inline std::vector<std::string> Prim::applied_schemas() const {
    return strings([&](TusdStringList* l) { return tusd_prim_applied_schemas(stage_, handle_, l); });
}
inline std::vector<std::string> Prim::attribute_names(bool authored_only) const {
    return strings([&](TusdStringList* l) { return tusd_prim_attribute_names(stage_, handle_, authored_only, l); });
}
inline std::vector<std::string> Prim::relationship_names(bool authored_only) const {
    return strings(
        [&](TusdStringList* l) { return tusd_prim_relationship_names(stage_, handle_, authored_only, l); });
}
inline std::vector<std::string> Prim::targets(const std::string& relationship, bool forwarded) const {
    return strings([&](TusdStringList* l) {
        return tusd_relationship_targets(stage_, handle_, relationship.c_str(), forwarded, l);
    });
}
inline std::vector<std::string> Prim::connections(const std::string& attribute) const {
    return strings(
        [&](TusdStringList* l) { return tusd_attribute_connections(stage_, handle_, attribute.c_str(), l); });
}
inline std::vector<double> Prim::time_samples(const std::string& attribute) const {
    Value v;
    detail::check(tusd_attribute_time_samples(stage_, handle_, attribute.c_str(), v.raw()));
    auto s = v.as_span<double>();
    return std::vector<double>(s.begin(), s.end());
}

using Quat = std::array<float, 4>;
using Vec3 = std::array<float, 3>;

enum class Axis : int { X = TUSD_AXIS_X, Y = TUSD_AXIS_Y, Z = TUSD_AXIS_Z };

enum class ShapeKind : int {
    Sphere = TUSD_SHAPE_KIND_SPHERE,
    Cube = TUSD_SHAPE_KIND_CUBE,
    Capsule = TUSD_SHAPE_KIND_CAPSULE,
    Capsule1 = TUSD_SHAPE_KIND_CAPSULE1,
    Cylinder = TUSD_SHAPE_KIND_CYLINDER,
    Cylinder1 = TUSD_SHAPE_KIND_CYLINDER1,
    Cone = TUSD_SHAPE_KIND_CONE,
    Plane = TUSD_SHAPE_KIND_PLANE,
    Mesh = TUSD_SHAPE_KIND_MESH,
    SpherePoints = TUSD_SHAPE_KIND_SPHERE_POINTS,
    Custom = TUSD_SHAPE_KIND_CUSTOM,
};

enum class JointKind : int {
    Fixed = TUSD_JOINT_KIND_FIXED,
    Revolute = TUSD_JOINT_KIND_REVOLUTE,
    Prismatic = TUSD_JOINT_KIND_PRISMATIC,
    Spherical = TUSD_JOINT_KIND_SPHERICAL,
    Distance = TUSD_JOINT_KIND_DISTANCE,
    D6 = TUSD_JOINT_KIND_D6,
    Custom = TUSD_JOINT_KIND_CUSTOM,
};

enum class Dof : int {
    Distance = TUSD_DOF_DISTANCE,
    TransX = TUSD_DOF_TRANS_X,
    TransY = TUSD_DOF_TRANS_Y,
    TransZ = TUSD_DOF_TRANS_Z,
    RotX = TUSD_DOF_ROT_X,
    RotY = TUSD_DOF_ROT_Y,
    RotZ = TUSD_DOF_ROT_Z,
};

struct PhysicsOptions {
    std::vector<Prim> include;
    std::vector<Prim> exclude;
    bool si_units = false;
};

struct PhysicsScene {
    Prim prim;
    Vec3 gravity_direction;
    float gravity_magnitude;
};

struct CollisionGroup {
    Prim prim;
    bool invert_filtered_groups;
    std::vector<Prim> filtered_groups;
    std::string merge_group_name;
    std::vector<Prim> merged_groups;
};

struct PhysicsMaterial {
    Prim prim;
    float static_friction;
    float dynamic_friction;
    float restitution;
    float density;
};

struct Shape {
    Prim prim;
    std::optional<Prim> rigid_body;
    Vec3 local_pos;
    Quat local_rot;
    Vec3 local_scale;
    std::vector<Prim> materials;
    std::vector<Prim> simulation_owners;
    std::vector<Prim> filtered_collisions;
    std::vector<Prim> collision_groups;
    bool collision_enabled;
    ShapeKind kind;
    float radius;
    float top_radius;
    float bottom_radius;
    float half_height;
    Vec3 half_extents;
    Axis axis;
    std::string mesh_approximation;
    Vec3 mesh_scale;
    bool double_sided;
    std::vector<std::array<float, 4>> sphere_points;
    std::string custom_type;
};

struct BodyMass {
    double mass;
    std::array<double, 3> diagonal_inertia;
    std::array<double, 3> center_of_mass;
    std::array<double, 4> principal_axes;
};

struct RigidBody {
    Prim prim;
    std::vector<Prim> collisions;
    std::vector<Prim> filtered_collisions;
    std::vector<Prim> simulation_owners;
    Vec3 position;
    Quat rotation;
    Vec3 scale;
    bool enabled;
    bool kinematic;
    bool starts_asleep;
    Vec3 linear_velocity;
    Vec3 angular_velocity;
};

struct MassInformation {
    double volume;
    std::array<double, 9> inertia;
    std::array<double, 3> center_of_mass;
    std::array<double, 3> local_pos;
    std::array<double, 4> local_rot;
};

using MassInfoFn = std::function<std::optional<MassInformation>(const Prim& collider)>;

struct JointLimit {
    Dof dof;
    bool enabled;
    float lower;
    float upper;
};

struct JointDrive {
    Dof dof;
    float target_position;
    float target_velocity;
    float force_limit;
    float stiffness;
    float damping;
    bool acceleration;
};

struct Joint {
    Prim prim;
    JointKind kind;
    std::string type_name;
    std::optional<Prim> rel0, rel1, body0, body1;
    Vec3 local_pos0;
    Quat local_rot0;
    Vec3 local_pos1;
    Quat local_rot1;
    bool enabled;
    bool collision_enabled;
    float break_force;
    float break_torque;
    bool exclude_from_articulation;
    Axis axis;
    std::vector<JointLimit> limits;
    std::vector<JointDrive> drives;
    bool min_distance_enabled;
    bool max_distance_enabled;
};

struct Articulation {
    Prim prim;
    std::vector<Prim> roots;
    std::vector<Prim> filtered_collisions;
    std::vector<Prim> joints;
    std::vector<std::optional<Prim>> bodies;
};

struct Physics {
    double meters_per_unit;
    double kilograms_per_unit;
    bool si_units;
    std::vector<PhysicsScene> scenes;
    std::vector<CollisionGroup> collision_groups;
    std::vector<PhysicsMaterial> materials;
    std::vector<Shape> shapes;
    std::vector<RigidBody> rigid_bodies;
    std::vector<Joint> joints;
    std::vector<Articulation> articulations;
    std::vector<std::string> diagnostics;
};

namespace detail {
struct PhysicsDeleter {
    void operator()(TusdPhysics* p) const noexcept { tusd_physics_free(p); }
};
}

class Stage {
public:
    static Stage open(const std::string& path, const OpenOptions& options = {}) {
        TusdOpenOptions raw;
        tusd_open_options_init(&raw);
        raw.load_payloads = options.load_payloads;
        TusdResolverCallbacks callbacks{};
        if (options.resolver) {
            callbacks.struct_size = sizeof(TusdResolverCallbacks);
            callbacks.user_data = new std::shared_ptr<Resolver>(options.resolver);
            callbacks.resolve = &detail::resolve_trampoline;
            callbacks.read = &detail::read_trampoline;
            callbacks.release = &detail::release_trampoline;
            raw.resolver = &callbacks;
        }
        std::vector<const char*> schema_paths;
        for (const auto& p : options.schema_paths) schema_paths.push_back(p.c_str());
        raw.schema_paths = schema_paths.data();
        raw.schema_path_count = schema_paths.size();
        TusdStage* stage = nullptr;
        detail::check(tusd_stage_open(path.c_str(), &raw, &stage));
        return Stage(stage);
    }

    Prim pseudo_root() const noexcept { return Prim(stage_.get(), tusd_stage_pseudo_root()); }

    std::optional<Prim> prim(const std::string& path) const {
        return pseudo_root().optional_prim(
            [&](TusdPrim* p, bool* f) { return tusd_stage_prim_at_path(stage_.get(), path.c_str(), p, f); });
    }

    std::optional<Prim> default_prim() const {
        return pseudo_root().optional_prim(
            [&](TusdPrim* p, bool* f) { return tusd_stage_default_prim(stage_.get(), p, f); });
    }

    std::vector<Prim> traverse(Traverse flags = Traverse::Default) const { return traverse(pseudo_root(), flags); }

    std::vector<Prim> traverse(const Prim& root, Traverse flags) const {
        own(root);
        return pseudo_root().prims([&](TusdPrimList* l) {
            return tusd_stage_traverse(stage_.get(), root.handle(), static_cast<std::uint32_t>(flags), l);
        });
    }

    inline std::vector<SkelBinding> skel_bindings() const { return skel_bindings(pseudo_root()); }

    std::vector<SkelBinding> skel_bindings(const Prim& root) const {
        own(root);
        std::unique_ptr<TusdSkelBindings, detail::SkelBindingsDeleter> raw(tusd_skel_bindings_new());
        detail::check(tusd_stage_skel_bindings(stage_.get(), root.handle(), raw.get()));
        std::vector<SkelBinding> out;
        for (std::size_t i = 0; i < tusd_skel_bindings_len(raw.get()); ++i) {
            TusdSkelBindingView v;
            detail::check(tusd_skel_bindings_get(raw.get(), i, &v));
            SkelBinding b{Prim(stage_.get(), v.skeleton), std::nullopt, {}};
            if (v.has_animation) b.animation = Prim(stage_.get(), v.animation);
            for (std::size_t k = 0; k < v.skinned_count; ++k) b.skinned.push_back(Prim(stage_.get(), v.skinned[k]));
            out.push_back(std::move(b));
        }
        return out;
    }

    Physics physics(const PhysicsOptions& options = {}) const {
        TusdPhysicsOptions raw;
        tusd_physics_options_init(&raw);
        std::vector<TusdPrim> include, exclude;
        for (const auto& p : options.include) {
            own(p);
            include.push_back(p.handle());
        }
        for (const auto& p : options.exclude) {
            own(p);
            exclude.push_back(p.handle());
        }
        raw.include = include.data();
        raw.include_count = include.size();
        raw.exclude = exclude.data();
        raw.exclude_count = exclude.size();
        raw.si_units = options.si_units;
        std::unique_ptr<TusdPhysics, detail::PhysicsDeleter> r(tusd_physics_new());
        detail::check(tusd_stage_physics(stage_.get(), &raw, r.get()));

        TusdStage* s = stage_.get();
        auto prim = [s](TusdPrim h) { return Prim(s, h); };
        auto opt = [s](TusdPrim h) -> std::optional<Prim> {
            if (h == TUSD_NO_PRIM) return std::nullopt;
            return Prim(s, h);
        };
        auto list = [&](const TusdPrim* p, std::size_t n) {
            std::vector<Prim> out;
            for (std::size_t i = 0; i < n; ++i) out.push_back(prim(p[i]));
            return out;
        };
        auto str = [](const char* c) { return c ? std::string(c) : std::string(); };
        auto v3 = [](const float* v) { return Vec3{v[0], v[1], v[2]}; };
        auto q4 = [](const float* v) { return Quat{v[0], v[1], v[2], v[3]}; };

        Physics out{};
        detail::check(tusd_physics_units(r.get(), &out.meters_per_unit, &out.kilograms_per_unit, &out.si_units));
        for (std::size_t i = 0; i < tusd_physics_count(r.get(), TUSD_PHYSICS_KIND_SCENE); ++i) {
            TusdPhysicsSceneView v;
            detail::check(tusd_physics_scene(r.get(), i, &v));
            out.scenes.push_back({prim(v.prim), v3(v.gravity_direction), v.gravity_magnitude});
        }
        for (std::size_t i = 0; i < tusd_physics_count(r.get(), TUSD_PHYSICS_KIND_COLLISION_GROUP); ++i) {
            TusdCollisionGroupView v;
            detail::check(tusd_physics_collision_group(r.get(), i, &v));
            out.collision_groups.push_back({prim(v.prim), v.invert_filtered_groups,
                                            list(v.filtered_groups, v.filtered_group_count), str(v.merge_group_name),
                                            list(v.merged_groups, v.merged_group_count)});
        }
        for (std::size_t i = 0; i < tusd_physics_count(r.get(), TUSD_PHYSICS_KIND_MATERIAL); ++i) {
            TusdPhysicsMaterialView v;
            detail::check(tusd_physics_material(r.get(), i, &v));
            out.materials.push_back({prim(v.prim), v.static_friction, v.dynamic_friction, v.restitution, v.density});
        }
        for (std::size_t i = 0; i < tusd_physics_count(r.get(), TUSD_PHYSICS_KIND_SHAPE); ++i) {
            TusdShapeView v;
            detail::check(tusd_physics_shape(r.get(), i, &v));
            Shape sh{prim(v.prim), opt(v.rigid_body), v3(v.local_pos), q4(v.local_rot), v3(v.local_scale),
                     list(v.materials, v.material_count), list(v.simulation_owners, v.simulation_owner_count),
                     list(v.filtered_collisions, v.filtered_collision_count),
                     list(v.collision_groups, v.collision_group_count), v.collision_enabled,
                     static_cast<ShapeKind>(v.kind), v.radius, v.top_radius, v.bottom_radius, v.half_height,
                     v3(v.half_extents), static_cast<Axis>(v.axis), str(v.mesh_approximation), v3(v.mesh_scale),
                     v.double_sided, {}, str(v.custom_type)};
            for (std::size_t k = 0; k < v.sphere_point_count; ++k) {
                const float* sp = v.sphere_points + 4 * k;
                sh.sphere_points.push_back({sp[0], sp[1], sp[2], sp[3]});
            }
            out.shapes.push_back(std::move(sh));
        }
        for (std::size_t i = 0; i < tusd_physics_count(r.get(), TUSD_PHYSICS_KIND_RIGID_BODY); ++i) {
            TusdRigidBodyView v;
            detail::check(tusd_physics_rigid_body(r.get(), i, &v));
            RigidBody b{prim(v.prim), list(v.collisions, v.collision_count),
                        list(v.filtered_collisions, v.filtered_collision_count),
                        list(v.simulation_owners, v.simulation_owner_count), v3(v.position), q4(v.rotation),
                        v3(v.scale), v.enabled, v.kinematic, v.starts_asleep, v3(v.linear_velocity),
                        v3(v.angular_velocity)};
            out.rigid_bodies.push_back(std::move(b));
        }
        for (std::size_t i = 0; i < tusd_physics_count(r.get(), TUSD_PHYSICS_KIND_JOINT); ++i) {
            TusdJointView v;
            detail::check(tusd_physics_joint(r.get(), i, &v));
            Joint j{prim(v.prim), static_cast<JointKind>(v.kind), str(v.type_name), opt(v.rel0), opt(v.rel1),
                    opt(v.body0), opt(v.body1), v3(v.local_pos0), q4(v.local_rot0), v3(v.local_pos1),
                    q4(v.local_rot1), v.enabled, v.collision_enabled, v.break_force, v.break_torque,
                    v.exclude_from_articulation, static_cast<Axis>(v.axis), {}, {}, v.min_distance_enabled,
                    v.max_distance_enabled};
            for (std::size_t k = 0; k < v.limit_count; ++k) {
                const auto& l = v.limits[k];
                j.limits.push_back({static_cast<Dof>(l.dof), l.enabled, l.lower, l.upper});
            }
            for (std::size_t k = 0; k < v.drive_count; ++k) {
                const auto& d = v.drives[k];
                j.drives.push_back({static_cast<Dof>(d.dof), d.target_position, d.target_velocity, d.force_limit,
                                    d.stiffness, d.damping, d.acceleration});
            }
            out.joints.push_back(std::move(j));
        }
        for (std::size_t i = 0; i < tusd_physics_count(r.get(), TUSD_PHYSICS_KIND_ARTICULATION); ++i) {
            TusdArticulationView v;
            detail::check(tusd_physics_articulation(r.get(), i, &v));
            Articulation a{prim(v.prim), list(v.roots, v.root_count),
                           list(v.filtered_collisions, v.filtered_collision_count), list(v.joints, v.joint_count), {}};
            for (std::size_t k = 0; k < v.body_count; ++k) a.bodies.push_back(opt(v.bodies[k]));
            out.articulations.push_back(std::move(a));
        }
        for (std::size_t i = 0; i < tusd_physics_count(r.get(), TUSD_PHYSICS_KIND_DIAGNOSTIC); ++i) {
            out.diagnostics.push_back(str(tusd_physics_diagnostic(r.get(), i)));
        }
        return out;
    }

    std::vector<Prim> mass_colliders(const Prim& body) const {
        own(body);
        return pseudo_root().prims(
            [&](TusdPrimList* l) { return tusd_prim_mass_colliders(stage_.get(), body.handle(), l); });
    }

    BodyMass mass_properties(const Prim& body, const MassInfoFn& mass_info) const {
        std::vector<TusdPrim> colliders;
        std::vector<TusdMassInformation> infos;
        for (const auto& collider : mass_colliders(body)) {
            auto info = mass_info(collider);
            if (!info) continue;
            TusdMassInformation raw{};
            raw.volume = info->volume;
            for (int k = 0; k < 9; ++k) raw.inertia[k] = info->inertia[k];
            for (int k = 0; k < 3; ++k) raw.center_of_mass[k] = info->center_of_mass[k];
            for (int k = 0; k < 3; ++k) raw.local_pos[k] = info->local_pos[k];
            for (int k = 0; k < 4; ++k) raw.local_rot[k] = info->local_rot[k];
            colliders.push_back(collider.handle());
            infos.push_back(raw);
        }
        TusdBodyMass m;
        detail::check(tusd_prim_mass_properties(stage_.get(), body.handle(), colliders.data(), infos.data(),
                                                colliders.size(), &m));
        return BodyMass{m.mass,
                        {m.diagonal_inertia[0], m.diagonal_inertia[1], m.diagonal_inertia[2]},
                        {m.center_of_mass[0], m.center_of_mass[1], m.center_of_mass[2]},
                        {m.principal_axes[0], m.principal_axes[1], m.principal_axes[2], m.principal_axes[3]}};
    }

    std::vector<Prim> prototypes() const {
        return pseudo_root().prims([&](TusdPrimList* l) { return tusd_stage_prototypes(stage_.get(), l); });
    }

    bool metadata(const std::string& key, Value& out) const {
        bool found = false;
        detail::check(tusd_stage_metadata(stage_.get(), key.c_str(), out.raw(), &found));
        return found;
    }

    bool has_authored_metadata(const std::string& key) const {
        bool out = false;
        detail::check(tusd_stage_has_authored_metadata(stage_.get(), key.c_str(), &out));
        return out;
    }

    TimeRange time_range() const {
        TusdTimeRange r;
        detail::check(tusd_stage_time_range(stage_.get(), &r));
        return {r.start_time_code, r.end_time_code, r.time_codes_per_second, r.frames_per_second};
    }

    std::vector<std::string> layer_identifiers() const {
        return pseudo_root().strings([&](TusdStringList* l) { return tusd_stage_layer_identifiers(stage_.get(), l); });
    }

    std::vector<std::string> composition_errors() const {
        return pseudo_root().strings(
            [&](TusdStringList* l) { return tusd_stage_composition_errors(stage_.get(), l); });
    }

    void load(const Prim& prim, bool with_descendants = true) {
        own(prim);
        detail::check(tusd_stage_load(stage_.get(), prim.handle(), with_descendants));
    }

    void unload(const Prim& prim) {
        own(prim);
        detail::check(tusd_stage_unload(stage_.get(), prim.handle()));
    }

    std::vector<Prim> loadable() const { return loadable(pseudo_root()); }

    std::vector<Prim> loadable(const Prim& root) const {
        own(root);
        return pseudo_root().prims(
            [&](TusdPrimList* l) { return tusd_stage_loadable_prims(stage_.get(), root.handle(), l); });
    }

    std::vector<std::uint8_t> read_asset(const std::string& resolved_path) const {
        Value v;
        detail::check(tusd_stage_read_asset(stage_.get(), resolved_path.c_str(), v.raw()));
        auto b = v.bytes();
        return std::vector<std::uint8_t>(b.begin(), b.end());
    }

    TusdStage* raw() const noexcept { return stage_.get(); }

private:
    explicit Stage(TusdStage* stage) : stage_(stage) {}
    void own(const Prim& prim) const {
        if (prim.stage_ != stage_.get()) {
            throw Error(Status::InvalidArgument, "prim belongs to a different stage");
        }
    }
    std::unique_ptr<TusdStage, detail::StageDeleter> stage_;
};

}

#endif
