#include <tinyusd/tinyusd.hpp>

#include <array>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <iterator>
#include <map>
#include <memory>
#include <string>
#include <vector>

#define CHECK(cond)                                                                       \
    do {                                                                                  \
        if (!(cond)) {                                                                    \
            std::fprintf(stderr, "%s:%d: CHECK(%s) failed\n", __FILE__, __LINE__, #cond); \
            std::exit(1);                                                                 \
        }                                                                                 \
    } while (0)

template <class F>
static tinyusd::Status status_of(F&& f) {
    try {
        f();
    } catch (const tinyusd::Error& e) {
        return e.status();
    }
    return tinyusd::Status::Ok;
}

static std::string read_file(const std::string& path) {
    std::ifstream in(path, std::ios::binary);
    return std::string(std::istreambuf_iterator<char>(in), {});
}

using Vec3 = std::array<float, 3>;
static const std::vector<Vec3> kPoints = {{0, 0, 0}, {1, 0, 0}, {1, 1, 0}, {0, 1, 0}};

static void build(const tinyusd::Stage& stage) {
    auto world = stage.define_prim("/World", "Xform");
    stage.set_default_prim(world);
    stage.set_up_axis("Z");
    stage.set_meters_per_unit(0.01);
    auto mesh = stage.define_prim("/World/Quad", "Mesh");
    mesh.set("points", kPoints);
    mesh.set("faceVertexCounts", std::vector<std::int32_t>{4});
    mesh.set("faceVertexIndices", std::vector<std::int32_t>{0, 1, 2, 3});
    mesh.set("subdivisionScheme", tinyusd::Value::token("none"));
}

static void check(const tinyusd::Stage& stage) {
    auto mesh = stage.prim("/World/Quad");
    CHECK(mesh && mesh->is_a("Mesh"));
    tinyusd::Value v;
    CHECK(mesh->get("points", v) && v.type_name() == "point3f[]" && v.size() == 4);
    CHECK(v.as_span<Vec3>()[2][1] == 1.0f);
    CHECK(stage.default_prim()->path() == "/World");
    CHECK(stage.metadata("metersPerUnit", v) && v.as<double>() == 0.01);
}

class MemoryResolver : public tinyusd::Resolver {
public:
    std::map<std::string, std::vector<std::uint8_t>> files;

    std::optional<std::string> resolve(std::string_view path, std::optional<std::string_view>) override {
        auto key = std::string(path);
        if (files.count(key)) return key;
        return std::nullopt;
    }
    bool read(std::string_view path, std::vector<std::uint8_t>& out) override {
        auto it = files.find(std::string(path));
        if (it == files.end()) return false;
        out = it->second;
        return true;
    }
    bool write(std::string_view path, const std::uint8_t* data, std::size_t size) override {
        files[std::string(path)] = std::vector<std::uint8_t>(data, data + size);
        return true;
    }
};

int main(int argc, char** argv) {
    CHECK(argc > 2);
    const std::string data = argv[1];
    const std::string out = argv[2];
    CHECK(tinyusd::has_feature(tinyusd::Feature::Write));

    {
        auto stage = tinyusd::Stage::create_new(out + "/cpp_scene.usda");
        build(stage);
        stage.save();
    }
    check(tinyusd::Stage::open(out + "/cpp_scene.usda"));

    auto stage = tinyusd::Stage::create_in_memory();
    build(stage);
    auto mesh = *stage.prim("/World/Quad");
    CHECK(status_of([&] { mesh.set("points", std::vector<std::array<double, 3>>(4)); }) ==
          tinyusd::Status::InvalidValue);
    CHECK(status_of([&] { mesh.set("subdivisionScheme", tinyusd::Value::string("none")); }) ==
          tinyusd::Status::InvalidValue);
    CHECK(status_of([&] { mesh.set("my:weight", 1.0f); }) == tinyusd::Status::InvalidValue);
    CHECK(status_of([&] { tinyusd::Value().set(tinyusd::ValueType::Float3, kPoints.data()->data(), 4); }) ==
          tinyusd::Status::InvalidValue);
    mesh.create_attribute("my:weight", "float");
    mesh.set("my:weight", 2.5f);
    CHECK(mesh.get<float>("my:weight") == 2.5f);

    {
        tinyusd::EditBatch batch(stage);
        for (int frame = 0; frame < 24; ++frame) {
            mesh.set("my:weight", static_cast<float>(frame), static_cast<double>(frame));
        }
        CHECK(mesh.time_samples("my:weight").empty());
        batch.commit();
    }
    CHECK(mesh.time_samples("my:weight").size() == 24 && mesh.get<float>("my:weight", 12.0) == 12.0f);
    {
        tinyusd::EditBatch discarded(stage);
        stage.define_prim("/World/Never", "Xform");
    }
    CHECK(!stage.prim("/World/Never"));

    auto body = stage.define_prim("/World/Body", "Cube");
    body.apply_api("PhysicsRigidBodyAPI");
    body.apply_api("PhysicsMassAPI");
    body.set("physics:mass", 3.0f);
    body.create_attribute("xformOp:orient", "quatf");
    body.set("xformOp:orient", tinyusd::Value().set_scalar(tinyusd::ValueType::Quatf, std::array<float, 4>{1, 0, 0, 0}));
    auto joint = stage.define_prim("/World/Joint", "PhysicsRevoluteJoint");
    joint.set_targets("physics:body0", {"/World/Body"});
    CHECK(joint.targets("physics:body0") == std::vector<std::string>{"/World/Body"});
    auto physics = stage.physics();
    CHECK(physics.rigid_bodies.size() == 1 && physics.joints.size() == 1);

    stage.export_to(out + "/cpp_flat.usdc");
    check(tinyusd::Stage::open(out + "/cpp_flat.usdc"));
    auto text = stage.export_bytes(tinyusd::LayerFormat::Usda);
    CHECK(std::string(text.begin(), text.end()).find("def Mesh \"Quad\"") != std::string::npos);

    const std::string source = data + "/physics.usda";
    const std::string before = read_file(source);
    {
        auto overlay = tinyusd::Stage::create_new(out + "/cpp_overlay.usda");
        overlay.add_sublayer(source);
        auto base = overlay.prim("/World/Base");
        CHECK(base);
        tinyusd::EditBatch batch(overlay);
        for (int frame = 0; frame < 5; ++frame) {
            base->set("xformOp:translate", std::array<double, 3>{0, 0, double(frame)}, double(frame));
        }
        batch.commit();
        overlay.save();
    }
    CHECK(read_file(source) == before);
    auto reopened = tinyusd::Stage::open(out + "/cpp_overlay.usda");
    CHECK(reopened.prim("/World/Base")->world_transform(3.0)[14] == 3.0);

    auto fs = std::make_shared<MemoryResolver>();
    tinyusd::OpenOptions options;
    options.resolver = fs;
    {
        auto mem = tinyusd::Stage::create_new("mem://scene.usda", options);
        build(mem);
        mem.save();
    }
    CHECK(fs->files.count("mem://scene.usda"));
    check(tinyusd::Stage::open("mem://scene.usda", options));

    auto helpers = tinyusd::Stage::create_in_memory();
    auto parent = helpers.define_prim("/P", "Xform");
    parent.set_trs({1, 2, 3}, {1, 0, 0, 0}, {1, 1, 1});
    auto child = helpers.define_prim("/P/C", "Mesh");
    tinyusd::Matrix4d target{1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 5, 5, 5, 1};
    child.set_world_transform(target);
    CHECK(child.world_transform() == target);
    child.set_mesh(kPoints, std::vector<std::int32_t>{4}, std::vector<std::int32_t>{0, 1, 2, 3});
    CHECK(status_of([&] { child.set_mesh(kPoints, std::vector<std::int32_t>{3}, std::vector<std::int32_t>{0, 1, 2, 3}); }) ==
          tinyusd::Status::InvalidValue);
    tinyusd::Value uv;
    uv.set(std::vector<std::array<float, 2>>{{0, 0}, {1, 1}});
    std::vector<std::int32_t> uv_indices{0, 1, 1, 0};
    child.set_primvar("st", "texCoord2f[]", uv, tinyusd::Interpolation::FaceVarying, tinyusd::Span<std::int32_t>(uv_indices));
    auto st = child.primvar("st", uv);
    CHECK(st && st->indexed && uv.size() == 4 && st->interpolation == tinyusd::Interpolation::FaceVarying);
    auto look = helpers.define_prim("/Looks/Red", "Material");
    child.bind_material(look);
    CHECK(child.bound_material() && child.bound_material()->path() == "/Looks/Red");
    helpers.export_to(out + "/cpp_helpers.usda");
    std::ofstream(out + "/tex.png") << "png";
    auto packaged = tinyusd::Stage::create_new(out + "/cpp_pkg.usda");
    auto tex = packaged.define_prim("/Tex", "Shader");
    tex.create_attribute("inputs:file", "asset");
    tex.set("inputs:file", tinyusd::Value::asset("./tex.png"));
    packaged.export_package(out + "/cpp_pkg.usdz");
    tinyusd::Value file;
    CHECK(tinyusd::Stage::open(out + "/cpp_pkg.usdz").prim("/Tex")->get("inputs:file", file));
    CHECK(std::string(file.resolved_path()).find(".usdz[") != std::string::npos);

    {
        auto overlay = tinyusd::Stage::create_new(out + "/cpp_physics.usda");
        overlay.add_sublayer(data + "/physics_full.usda");
        overlay.set_meters_per_unit(0.01);
        overlay.set_kilograms_per_unit(0.001);
        overlay.set_up_axis("Y");
        tinyusd::PhysicsOptions si;
        si.si_units = true;
        auto before = overlay.physics(si);
        overlay.write_physics(before, true);
        auto after = overlay.physics(si);
        CHECK(after.joints.size() == before.joints.size() && after.shapes.size() == before.shapes.size());
        for (std::size_t i = 0; i < before.joints.size(); ++i) {
            CHECK(after.joints[i].limits.size() == before.joints[i].limits.size());
            CHECK(std::fabs(after.joints[i].local_pos1[1] - before.joints[i].local_pos1[1]) < 1e-4);
        }

        auto scratch = tinyusd::Stage::create_in_memory();
        auto a = scratch.define_prim("/A", "Cube");
        auto b = scratch.define_prim("/B", "Cube");
        tinyusd::RigidBody body;
        body.prim = b;
        body.position = {0, 0, 3};
        scratch.write_rigid_body(body, true);
        tinyusd::Joint hinge;
        hinge.prim = scratch.define_prim("/Hinge");
        hinge.kind = tinyusd::JointKind::Revolute;
        hinge.body0 = a;
        hinge.body1 = b;
        tinyusd::JointLimit limit;
        limit.dof = tinyusd::Dof::RotX;
        limit.lower = -30;
        limit.upper = 30;
        hinge.limits.push_back(limit);
        scratch.write_joint(hinge);
        tinyusd::BodyMass mass;
        mass.mass = 4;
        scratch.write_body_mass(b, mass);
        auto parsed = scratch.physics();
        CHECK(parsed.rigid_bodies.size() == 1 && parsed.rigid_bodies[0].position[2] == 3.0f);
        CHECK(parsed.joints.size() == 1 && parsed.joints[0].kind == tinyusd::JointKind::Revolute);
        CHECK(parsed.joints[0].limits[0].enabled && parsed.joints[0].limits[0].upper == 30.0f);
        CHECK(b.get<float>("physics:mass") == 4.0f);
        b.remove_api("PhysicsMassAPI");
        CHECK(!b.has_api("PhysicsMassAPI"));

        b.rename("Body");
        CHECK(b.path() == "/Body" && parsed.joints[0].body1->path() == "/Body");
        CHECK(scratch.prim("/Hinge")->targets("physics:body1") == std::vector<std::string>{"/Body"});
        scratch.move_prim(a, b);
        CHECK(a.path() == "/Body/A");

        auto asset = scratch.define_prim("/Asset", "Xform");
        asset.edit_variant("look", "red");
        scratch.define_prim("/Asset/Paint", "Sphere");
        asset.edit_variant("look", "blue");
        scratch.define_prim("/Asset/Paint", "Cube");
        scratch.set_edit_target(scratch.edit_target());
        CHECK(asset.variant_sets() == std::vector<std::string>{"look"});
        CHECK((asset.variants("look") == std::vector<std::string>{"blue", "red"}));
        asset.set_variant_selection("look", "red");
        CHECK(scratch.prim("/Asset/Paint")->is_a("Sphere"));
    }

    std::puts("ok");
    return 0;
}
