#include <tinyusd/tinyusd.hpp>

#include <array>
#include <cstdio>
#include <optional>
#include <string>
#include <vector>

using Vec3 = std::array<float, 3>;

struct Shape {
    std::string kind;
    std::array<double, 3> dims{};
};

struct Collider {
    std::string path;
    std::string body;
    Shape shape;
    std::array<double, 3> position{};
};

struct Body {
    std::string path;
    float mass = 0.0f;
    bool kinematic = false;
    std::array<double, 3> position{};
};

struct Joint {
    std::string path;
    std::string kind;
    std::string body0;
    std::string body1;
    std::string axis;
    std::optional<std::array<float, 2>> limits;
};

static std::array<double, 3> translation(const tinyusd::Matrix4d& m) { return {m[12], m[13], m[14]}; }

static double scalar(const tinyusd::Prim& prim, const char* name, double fallback) {
    tinyusd::Value v;
    if (!prim.get(name, v)) return fallback;
    if (auto d = v.as<double>()) return *d;
    if (auto f = v.as<float>()) return *f;
    return fallback;
}

static std::string token(const tinyusd::Prim& prim, const char* name, const char* fallback = "") {
    tinyusd::Value v;
    return prim.get(name, v) ? std::string(v.string()) : std::string(fallback);
}

static std::string owning_body(const tinyusd::Prim& prim) {
    for (std::optional<tinyusd::Prim> p = prim; p; p = p->parent()) {
        if (p->has_api("PhysicsRigidBodyAPI")) return std::string(p->path());
    }
    return "<static>";
}

static Shape shape_of(const tinyusd::Prim& prim) {
    const std::string type(prim.type_name());
    if (type == "Cube") return {"box", {scalar(prim, "size", 2.0), scalar(prim, "size", 2.0), scalar(prim, "size", 2.0)}};
    if (type == "Sphere") return {"sphere", {scalar(prim, "radius", 1.0), 0, 0}};
    if (type == "Capsule") return {"capsule-" + token(prim, "axis", "Z"), {scalar(prim, "radius", 0.5), scalar(prim, "height", 1.0), 0}};
    if (type == "Cylinder") return {"cylinder-" + token(prim, "axis", "Z"), {scalar(prim, "radius", 1.0), scalar(prim, "height", 2.0), 0}};
    if (type == "Mesh") return {"mesh-" + token(prim, "physics:approximation", "none"), {}};
    return {type, {}};
}

int main(int argc, char** argv) {
    if (argc < 2) {
        std::fprintf(stderr, "usage: physics_import <file.usd>\n");
        return 2;
    }
    try {
        auto stage = tinyusd::Stage::open(argv[1]);
        tinyusd::Value v;
        const double meters = stage.metadata("metersPerUnit", v) ? v.as<double>().value_or(0.01) : 0.01;
        const double kilograms = stage.metadata("kilogramsPerUnit", v) ? v.as<double>().value_or(1.0) : 1.0;
        std::printf("units: %.3g m, %.3g kg\n", meters, kilograms);

        std::vector<Body> bodies;
        std::vector<Collider> colliders;
        std::vector<Joint> joints;

        for (const auto& prim : stage.traverse(tinyusd::Traverse::InstanceProxies)) {
            if (prim.is_a("PhysicsScene")) {
                Vec3 dir{0, 0, 0};
                if (prim.get("physics:gravityDirection", v) && v.as_span<float>().size() == 3) {
                    auto d = v.as_span<float>();
                    dir = {d[0], d[1], d[2]};
                }
                std::printf("scene %s: gravity %.2f along %.0f %.0f %.0f\n", std::string(prim.path()).c_str(),
                            scalar(prim, "physics:gravityMagnitude", 9.81 / meters), dir[0], dir[1], dir[2]);
            }
            if (prim.has_api("PhysicsRigidBodyAPI")) {
                Body b;
                b.path = std::string(prim.path());
                b.mass = static_cast<float>(scalar(prim, "physics:mass", 0.0));
                b.kinematic = prim.get<bool>("physics:kinematicEnabled").value_or(false);
                b.position = translation(prim.world_transform());
                bodies.push_back(b);
            }
            if (prim.has_api("PhysicsCollisionAPI")) {
                colliders.push_back({std::string(prim.path()), owning_body(prim), shape_of(prim),
                                     translation(prim.world_transform())});
            }
            if (prim.is_a("PhysicsJoint")) {
                Joint j;
                j.path = std::string(prim.path());
                j.kind = std::string(prim.type_name());
                auto b0 = prim.targets("physics:body0");
                auto b1 = prim.targets("physics:body1");
                j.body0 = b0.empty() ? "<world>" : b0.front();
                j.body1 = b1.empty() ? "<world>" : b1.front();
                j.axis = token(prim, "physics:axis");
                auto lo = prim.get<float>("physics:lowerLimit");
                auto hi = prim.get<float>("physics:upperLimit");
                if (lo && hi) j.limits = std::array<float, 2>{*lo, *hi};
                joints.push_back(j);
            }
        }

        for (const auto& b : bodies) {
            std::printf("body %s: mass %.2f kg%s at %.2f %.2f %.2f\n", b.path.c_str(), b.mass * kilograms,
                        b.kinematic ? " (kinematic)" : "", b.position[0], b.position[1], b.position[2]);
        }
        for (const auto& c : colliders) {
            std::printf("collider %s: %s [%.2f %.2f %.2f] on %s\n", c.path.c_str(), c.shape.kind.c_str(),
                        c.shape.dims[0], c.shape.dims[1], c.shape.dims[2], c.body.c_str());
        }
        for (const auto& j : joints) {
            std::printf("joint %s: %s %s -> %s axis %s", j.path.c_str(), j.kind.c_str(), j.body0.c_str(),
                        j.body1.c_str(), j.axis.c_str());
            if (j.limits) std::printf(" limits [%.1f, %.1f]", (*j.limits)[0], (*j.limits)[1]);
            std::printf("\n");
        }
    } catch (const tinyusd::Error& e) {
        std::fprintf(stderr, "error: %s\n", e.what());
        return 1;
    }
    return 0;
}
