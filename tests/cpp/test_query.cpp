#include <tinyusd/tinyusd.hpp>

#include <algorithm>
#include <cmath>
#include <array>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <iterator>
#include <map>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

#define CHECK(cond)                                                                       \
    do {                                                                                  \
        if (!(cond)) {                                                                    \
            std::fprintf(stderr, "%s:%d: CHECK(%s) failed\n", __FILE__, __LINE__, #cond); \
            std::exit(1);                                                                 \
        }                                                                                 \
    } while (0)

struct Vec3f {
    float x, y, z;
};

static bool contains(const std::vector<std::string>& v, const std::string& s) {
    return std::find(v.begin(), v.end(), s) != v.end();
}

static void check_stage(const std::string& file) {
    auto stage = tinyusd::Stage::open(file);

    tinyusd::Value v;
    CHECK(stage.metadata("upAxis", v) && v.type() == tinyusd::ValueType::Token && v.string() == "Z");
    CHECK(stage.metadata("metersPerUnit", v) && v.as<double>() == 0.01);
    CHECK(stage.has_authored_metadata("upAxis"));
    auto range = stage.time_range();
    CHECK(range.start_time_code == 1.0 && range.end_time_code == 10.0 && range.time_codes_per_second == 24.0);
    CHECK(stage.composition_errors().empty());
    CHECK(stage.default_prim() && stage.default_prim()->path() == "/World");

    auto prims = stage.traverse();
    CHECK(prims.size() == 10);
    CHECK(prims.front().path() == "/World" && prims.back().path() == "/World/Payloaded");
    CHECK(stage.traverse(tinyusd::Traverse::All).size() == 13);

    auto mesh = stage.prim("/World/Mesh");
    CHECK(mesh && mesh->type_name() == "Mesh" && mesh->name() == "Mesh");
    CHECK(mesh->is_a("Gprim") && !mesh->is_a("Xform"));
    CHECK(mesh->has_api("SemanticsLabelsAPI:class") && mesh->has_api("PhysicsCollisionAPI"));
    CHECK(contains(mesh->applied_schemas(), "SemanticsLabelsAPI:class"));
    CHECK(contains(mesh->attribute_names(true), "points"));
    CHECK(mesh->parent() && mesh->parent()->path() == "/World");
    CHECK(!stage.prim("/World/Nope"));

    CHECK(mesh->get("points", v) && v.type() == tinyusd::ValueType::Float3 && v.is_array());
    CHECK(v.type_name() == "point3f[]" && v.size() == 3);
    auto pts = v.as_span<Vec3f>();
    CHECK(pts.size() == 3 && pts[1].x == 1.0f && pts[2].y == 1.0f);
    CHECK((v.as_span<std::array<float, 3>>().size() == 3));
    CHECK(v.as_span<float>().size() == 9);
    CHECK(v.as_span<double>().empty());
    CHECK(v.as_span<std::int32_t>().empty());

    auto types = stage.prim("/World/Types");
    CHECK(types->get<std::int32_t>("i") == -3);
    CHECK(types->get<std::uint64_t>("u64") == 18000000000000000000ull);
    CHECK(types->get<double>("d") == 3.25);
    CHECK(types->get<std::int32_t>("baseValue") == 7);
    CHECK(types->get("qf", v) && v.type() == tinyusd::ValueType::Quatf);
    auto q = v.as_span<float>();
    CHECK(q.size() == 4 && q[0] == 0.5f);
    CHECK(types->get("m4", v) && v.as_span<double>().size() == 16 && v.as_span<double>()[12] == 5.0);
    CHECK(types->get("ta", v) && v.strings().size() == 2 && v.string(1) == "b");
    CHECK(types->get("a", v) && v.type() == tinyusd::ValueType::Asset && v.string() == "./part.usda");
    CHECK(v.resolved_path().size() > 0);
    CHECK(types->get<float>("animated", 5.5) == 4.5f);
    CHECK((types->time_samples("animated") == std::vector<double>{1.0, 10.0}));
    CHECK(!types->get("blocked", v) && v.empty());
    CHECK(types->targets("forward", true) == std::vector<std::string>{"/World/Mesh"});
    CHECK(types->attribute_metadata("primvars:displayColor", "interpolation", v) && v.string() == "constant");
    auto info = types->attribute_info("pts");
    CHECK(info && info->type_name == "point3f[]" && info->custom && !info->uniform);
    CHECK(!types->attribute_info("nope"));

    auto mat = stage.prim("/World/Mat");
    CHECK(mat->connections("outputs:surface") == std::vector<std::string>{"/World/Mat/Shader.outputs:surface"});
    auto world = stage.prim("/World");
    CHECK(world->kind() == "assembly" && world->flags().has(tinyusd::PrimFlag::Model));
    CHECK(stage.prim("/_Base")->specifier() == tinyusd::Specifier::Class);

    auto i1 = stage.prim("/World/Instance1");
    CHECK(i1->is_instance());
    auto proto = i1->prototype();
    CHECK(proto && stage.prototypes().size() == 1 && proto->instances().size() == 2);

    bool threw = false;
    try {
        stage.prim("not a path");
    } catch (const tinyusd::Error& e) {
        threw = e.status() == tinyusd::Status::InvalidPath;
    }
    CHECK(threw);
}

class MemoryResolver : public tinyusd::Resolver {
public:
    explicit MemoryResolver(const std::string& dir) {
        for (const char* name : {"query.usda", "part.usda", "textured.usdz"}) {
            std::ifstream f(dir + "/" + name, std::ios::binary);
            files["mem://assets/" + std::string(name)] =
                std::vector<std::uint8_t>(std::istreambuf_iterator<char>(f), std::istreambuf_iterator<char>());
        }
    }

    std::optional<std::string> resolve(std::string_view asset_path, std::optional<std::string_view> anchor) override {
        std::string candidate(asset_path);
        if (candidate.rfind("mem://", 0) != 0) {
            if (!anchor) return std::nullopt;
            std::string a(*anchor);
            if (candidate.rfind("./", 0) == 0) candidate = candidate.substr(2);
            candidate = a.substr(0, a.rfind('/') + 1) + candidate;
        }
        if (!files.count(candidate)) return std::nullopt;
        return candidate;
    }

    bool read(std::string_view resolved_path, std::vector<std::uint8_t>& out) override {
        ++reads;
        auto it = files.find(std::string(resolved_path));
        if (it == files.end()) throw std::runtime_error("no such file: " + std::string(resolved_path));
        out = it->second;
        return true;
    }

    std::map<std::string, std::vector<std::uint8_t>> files;
    int reads = 0;
};

int main(int argc, char** argv) {
    CHECK(argc > 1);
    const std::string data = argv[1];
    CHECK(tinyusd::abi_version() == TUSD_ABI_VERSION);
    CHECK(!tinyusd::version().empty());
    CHECK(tinyusd::has_feature(tinyusd::Feature::Geom));

    check_stage(data + "/query.usda");
    check_stage(data + "/query.usdc");

    auto pkg = tinyusd::Stage::open(data + "/textured.usdz");
    tinyusd::Value file;
    CHECK(pkg.prim("/Root/Texture")->get("inputs:file", file));
    auto bytes = pkg.read_asset(std::string(file.resolved_path()));
    CHECK(std::string(bytes.begin(), bytes.end()) == "tinyusd-texture-bytes");

    auto resolve = tinyusd::Stage::open(data + "/resolve.usda");
    auto own = resolve.prim("/World/Group/Own");
    auto world = own->world_transform();
    CHECK(world[12] == 10.0 && world[13] == 0.0);
    CHECK(own->world_transform(5.0)[13] == 10.0);
    CHECK(resolve.prim("/World/Group/Reset")->local_transform().resets_xform_stack);
    auto names = own->primvar_names();
    CHECK(names.size() == 4 && contains(names, "roughness") && contains(names, "st"));
    tinyusd::Value pv;
    auto st = own->primvar("st", pv);
    CHECK(st && st->interpolation == tinyusd::Interpolation::FaceVarying && st->indexed && pv.size() == 7);
    CHECK(st->type_name == "texCoord2f[]" && st->name == "st");
    auto rough = own->primvar("roughness", pv);
    CHECK(rough && rough->source.path() == "/World" && pv.as<float>() == 0.25f);
    CHECK(!resolve.prim("/World/Group/Inherits")->primvar("st", pv) && pv.empty());
    if (tinyusd::has_feature(tinyusd::Feature::Shade)) {
        CHECK(own->bound_material()->path() == "/World/Looks/Blue");
        CHECK(own->bound_material("preview")->path() == "/World/Looks/Preview");
        CHECK(resolve.prim("/World/ByCollection/Target")->bound_material()->path() == "/World/Looks/Collection");
        CHECK(resolve.prim("/World/Group/Inherits")->bound_material("full")->path() == "/World/Looks/Red");
    }

    auto inst_stage = tinyusd::Stage::open(data + "/instancer.usda");
    auto trees = inst_stage.prim("/World/Trees")->instancer_instances();
    CHECK(trees && trees->prototypes.size() == 2 && trees->prototypes[1].path() == "/World/Trees/Prototypes/Oak");
    CHECK((trees->ids == std::vector<std::int64_t>{10, 12}));
    CHECK((trees->proto_indices == std::vector<std::int32_t>{0, 1}));
    CHECK(trees->transforms[0][13] == 1.0 && trees->transforms[1][12] == 10.0 && trees->transforms[1][5] == 3.0);
    CHECK(inst_stage.prim("/World/Trees")->instancer_instances(std::nullopt, false, false)->ids.size() == 4);
    CHECK(inst_stage.prim("/World/Rocks")->instancer_instances(5.0)->transforms[1][13] == 5.0);
    CHECK(!inst_stage.prim("/World")->instancer_instances());

    if (tinyusd::has_feature(tinyusd::Feature::Skel)) {
        auto skel_stage = tinyusd::Stage::open(data + "/skel.usda");
        auto bindings = skel_stage.skel_bindings();
        CHECK(bindings.size() == 1 && bindings[0].skeleton.path() == "/Character/Skel");
        CHECK(bindings[0].animation && bindings[0].animation->path() == "/Character/Skel/Anim");
        CHECK(bindings[0].skinned.size() == 2 && bindings[0].skinned[1].path() == "/Character/Hand");
        auto pose = bindings[0].skeleton.skeleton_pose(10.0);
        CHECK(pose && (pose->joints == std::vector<std::string>{"root", "root/elbow"}));
        CHECK((pose->parents == std::vector<std::int32_t>{-1, 0}) && pose->skinning.size() == 2);
        CHECK(std::abs(pose->skinning[1][12] - 1.0) < 1e-6);
        auto arm = *skel_stage.prim("/Character/Arm");
        auto skin = arm.skin_binding(5.0);
        CHECK(skin && skin->influences_per_point == 2 && !skin->rigid && skin->skeleton.path() == "/Character/Skel");
        CHECK(skin->joint_indices[0] == 0 && skin->joint_indices[1] == 1);
        CHECK((skin->blend_shapes == std::vector<std::string>{"bulge"}) && skin->blend_shape_weights[0] == 0.5f);
        CHECK(skin->skinning_method == tinyusd::SkinningMethod::ClassicLinear);
        auto shapes = arm.blend_shapes(5.0);
        CHECK(shapes && shapes->point_indices.size() == 1 && shapes->sub_shapes.size() == 3);
        CHECK(shapes->sub_shapes[1].weight == 1.0f && shapes->sub_shapes[1].point_offsets.size() == 2);
        tinyusd::Value skinned;
        CHECK(skel_stage.prim("/Character/Hand")->skinned_points(skinned, 10.0) && skinned.size() == 3);
        CHECK(std::abs(skinned.as_span<float>()[0] + 1.0f) < 1e-5f);
        CHECK(!skel_stage.prim("/Character/Skel")->skin_binding());
    }

    {
        tinyusd::OpenOptions with_schema;
        with_schema.schema_paths.push_back(data + "/schemas/demo");
        auto custom = tinyusd::Stage::open(data + "/custom.usda", with_schema);
        CHECK(custom.prim("/World/Widget")->is_a("Xformable"));
        CHECK(custom.prim("/World/Defaults")->get<float>("demo:size") == 2.5f);
        CHECK(!tinyusd::Stage::open(data + "/custom.usda").prim("/World/Defaults")->get<float>("demo:size"));
        tinyusd::OpenOptions missing;
        missing.schema_paths.push_back(data + "/schemas/nope");
        bool threw = false;
        try {
            tinyusd::Stage::open(data + "/custom.usda", missing);
        } catch (const tinyusd::Error& e) {
            threw = e.status() == tinyusd::Status::SchemaFailed;
        }
        CHECK(threw);
    }

    if (tinyusd::has_feature(tinyusd::Feature::Physics)) {
        auto ps = tinyusd::Stage::open(data + "/physics_full.usda");
        auto phys = ps.physics();
        CHECK(phys.meters_per_unit == 0.01 && phys.kilograms_per_unit == 0.001 && !phys.si_units);
        CHECK(phys.shapes.size() == 8 && phys.rigid_bodies.size() == 6 && phys.joints.size() == 5);
        CHECK(phys.scenes[0].gravity_direction[1] == -1.0f);
        const tinyusd::Joint* elbow = nullptr;
        for (const auto& j : phys.joints)
            if (j.prim.path() == "/World/Robot/Elbow") elbow = &j;
        CHECK(elbow && elbow->kind == tinyusd::JointKind::Revolute && elbow->body1->path() == "/World/Robot/Link");
        CHECK(elbow->limits[0].dof == tinyusd::Dof::RotZ && elbow->drives[0].stiffness == 50.0f);
        const tinyusd::RigidBody* crate = nullptr;
        for (const auto& b : phys.rigid_bodies)
            if (b.prim.path() == "/World/Props/Crate") crate = &b;
        CHECK(crate);
        std::map<TusdPrim, std::pair<tinyusd::Vec3, tinyusd::Quat>> poses;
        for (const auto& sh : phys.shapes) poses[sh.prim.handle()] = {sh.local_pos, sh.local_rot};
        CHECK(ps.mass_colliders(crate->prim).size() == 1);
        auto mass = ps.mass_properties(crate->prim, [&](const tinyusd::Prim& c) -> std::optional<tinyusd::MassInformation> {
            if (c.path() != "/World/Props/Crate") return std::nullopt;
            auto it = poses.find(c.handle());
            if (it == poses.end()) return std::nullopt;
            tinyusd::MassInformation m{10.0, {1, 0, 0, 0, 1, 0, 0, 0, 1}, {0, 0, 0}, {}, {}};
            for (int k = 0; k < 3; ++k) m.local_pos[k] = it->second.first[k];
            for (int k = 0; k < 4; ++k) m.local_rot[k] = it->second.second[k];
            return m;
        });
        CHECK(std::abs(mass.mass - 10.0) < 1e-9);
        auto authored = ps.mass_properties(*ps.prim("/World/Robot/Base"), [](const tinyusd::Prim&) {
            return std::optional<tinyusd::MassInformation>();
        });
        CHECK(authored.mass == 5000.0 && authored.diagonal_inertia[2] == 30.0);
        CHECK(phys.articulations[0].roots[0].path() == "/World/Robot/Base");
        tinyusd::PhysicsOptions si;
        si.si_units = true;
        si.include.push_back(*ps.prim("/World/Props"));
        auto props = ps.physics(si);
        CHECK(props.si_units && props.rigid_bodies.size() == 4 && props.scenes.empty());
        for (const auto& b : props.rigid_bodies)
            if (b.prim.path() == "/World/Props/Crate") CHECK(std::abs(b.position[2] - 0.4f) < 1e-6f);
    }

    auto unloaded = tinyusd::Stage::open(data + "/query.usda", {false});
    auto payloaded = *unloaded.prim("/World/Payloaded");
    CHECK(!payloaded.flags().has(tinyusd::PrimFlag::Loaded));
    CHECK(unloaded.loadable().size() == 1 && unloaded.loadable().front() == payloaded);
    unloaded.load(payloaded);
    CHECK(payloaded.flags().has(tinyusd::PrimFlag::Loaded) && payloaded.type_name() == "Mesh");
    unloaded.unload(payloaded);
    CHECK(!payloaded.flags().has(tinyusd::PrimFlag::Loaded));
    bool foreign = false;
    try {
        unloaded.traverse(*resolve.prim("/World"), tinyusd::Traverse::Default);
    } catch (const tinyusd::Error& e) {
        foreign = e.status() == tinyusd::Status::InvalidArgument;
    }
    CHECK(foreign);

    auto memory = std::make_shared<MemoryResolver>(data);
    {
        tinyusd::OpenOptions options;
        options.resolver = memory;
        auto mem = tinyusd::Stage::open("mem://assets/query.usda", options);
        CHECK(mem.traverse().size() == 10);
        CHECK(mem.prim("/World/Payloaded")->type_name() == "Mesh");
        tinyusd::Value a;
        CHECK(mem.prim("/World/Types")->get("a", a) && a.resolved_path() == "mem://assets/part.usda");
        auto pkg_stage = tinyusd::Stage::open("mem://assets/textured.usdz", options);
        tinyusd::Value tex;
        CHECK(pkg_stage.prim("/Root/Texture")->get("inputs:file", tex));
        CHECK(tex.resolved_path() == "mem://assets/textured.usdz[tex.png]");
        auto tex_bytes = pkg_stage.read_asset(std::string(tex.resolved_path()));
        CHECK(std::string(tex_bytes.begin(), tex_bytes.end()) == "tinyusd-texture-bytes");
        bool threw = false;
        try {
            mem.read_asset("mem://assets/missing.png");
        } catch (const tinyusd::Error& e) {
            threw = e.status() == tinyusd::Status::AssetFailed &&
                    std::string(e.what()).find("no such file") != std::string::npos;
        }
        CHECK(threw);
        CHECK(memory.use_count() > 1);
    }
    CHECK(memory.use_count() == 1 && memory->reads > 0);

    bool threw = false;
    try {
        tinyusd::Stage::open(data + "/missing.usda");
    } catch (const tinyusd::Error& e) {
        threw = e.status() == tinyusd::Status::OpenFailed && std::string(e.what()).find("missing.usda") != std::string::npos;
    }
    CHECK(threw);

    std::puts("ok");
    return 0;
}
