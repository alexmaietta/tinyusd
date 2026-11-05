#include <tinyusd/tinyusd.hpp>

#include <array>
#include <cstdio>
#include <optional>
#include <string>
#include <vector>

using Vec2 = std::array<float, 2>;
using Vec3 = std::array<float, 3>;

struct Texture {
    std::string path;
    std::string wrap_s;
    std::string wrap_t;
};

struct Material {
    std::string name;
    Vec3 base_color{0.18f, 0.18f, 0.18f};
    float roughness = 0.5f;
    float metallic = 0.0f;
    std::optional<Texture> base_color_texture;
};

struct GpuMesh {
    std::string name;
    std::vector<Vec3> positions;
    std::vector<Vec3> normals;
    std::vector<Vec2> uvs;
    std::vector<std::uint32_t> indices;
    std::optional<Material> material;
};

static Vec3 transform_point(const tinyusd::Matrix4d& m, const Vec3& p) {
    Vec3 out{};
    for (int c = 0; c < 3; ++c) {
        out[c] = static_cast<float>(p[0] * m[0 * 4 + c] + p[1] * m[1 * 4 + c] + p[2] * m[2 * 4 + c] + m[12 + c]);
    }
    return out;
}

static Vec3 transform_vector(const tinyusd::Matrix4d& m, const Vec3& v) {
    Vec3 out{};
    for (int c = 0; c < 3; ++c) {
        out[c] = static_cast<float>(v[0] * m[0 * 4 + c] + v[1] * m[1 * 4 + c] + v[2] * m[2 * 4 + c]);
    }
    return out;
}

static std::optional<tinyusd::Prim> connected_shader(const tinyusd::Stage& stage, const tinyusd::Prim& prim,
                                                     const std::string& input) {
    auto sources = prim.connections(input);
    if (sources.empty()) return std::nullopt;
    const std::string& source = sources.front();
    return stage.prim(source.substr(0, source.find('.')));
}

static Material read_material(const tinyusd::Stage& stage, const tinyusd::Prim& material) {
    Material out;
    out.name = std::string(material.path());
    auto surface = connected_shader(stage, material, "outputs:surface");
    if (!surface) return out;

    tinyusd::Value v;
    if (auto texture = connected_shader(stage, *surface, "inputs:diffuseColor")) {
        if (texture->get("inputs:file", v)) {
            Texture t;
            t.path = std::string(v.resolved_path().empty() ? v.string() : v.resolved_path());
            if (texture->get("inputs:wrapS", v)) t.wrap_s = std::string(v.string());
            if (texture->get("inputs:wrapT", v)) t.wrap_t = std::string(v.string());
            out.base_color_texture = t;
        }
    } else if (surface->get("inputs:diffuseColor", v)) {
        auto c = v.as_span<float>();
        if (c.size() == 3) out.base_color = {c[0], c[1], c[2]};
    }
    out.roughness = surface->get<float>("inputs:roughness").value_or(out.roughness);
    out.metallic = surface->get<float>("inputs:metallic").value_or(out.metallic);
    return out;
}

template <class T>
static T corner_value(tinyusd::Span<T> data, tinyusd::Interpolation interp, std::size_t face, std::size_t corner,
                      std::int32_t vertex) {
    switch (interp) {
        case tinyusd::Interpolation::Constant: return data[0];
        case tinyusd::Interpolation::Uniform: return data[face];
        case tinyusd::Interpolation::FaceVarying: return data[corner];
        default: return data[static_cast<std::size_t>(vertex)];
    }
}

static GpuMesh import_mesh(const tinyusd::Stage& stage, const tinyusd::Prim& prim) {
    GpuMesh mesh;
    mesh.name = std::string(prim.path());
    const auto world = prim.world_transform();

    tinyusd::Value points, counts, indices, normals, uvs;
    prim.get("points", points);
    prim.get("faceVertexCounts", counts);
    prim.get("faceVertexIndices", indices);
    auto p = points.as_span<Vec3>();
    auto fc = counts.as_span<std::int32_t>();
    auto fi = indices.as_span<std::int32_t>();

    auto normal_info = prim.primvar("normals", normals);
    auto n = normals.as_span<Vec3>();
    auto normal_interp = normal_info ? normal_info->interpolation : tinyusd::Interpolation::Vertex;
    if (!normal_info && prim.get("normals", normals)) {
        n = normals.as_span<Vec3>();
        tinyusd::Value interp;
        if (prim.attribute_metadata("normals", "interpolation", interp) && interp.string() == "faceVarying") {
            normal_interp = tinyusd::Interpolation::FaceVarying;
        }
    }
    auto uv_info = prim.primvar("st", uvs);
    auto uv = uvs.as_span<Vec2>();

    std::size_t corner = 0;
    for (std::size_t face = 0; face < fc.size(); ++face) {
        const auto count = static_cast<std::size_t>(fc[face]);
        const auto base = static_cast<std::uint32_t>(mesh.positions.size());
        for (std::size_t k = 0; k < count; ++k, ++corner) {
            const std::int32_t vertex = fi[corner];
            mesh.positions.push_back(transform_point(world, p[static_cast<std::size_t>(vertex)]));
            if (!n.empty()) mesh.normals.push_back(transform_vector(world, corner_value(n, normal_interp, face, corner, vertex)));
            if (uv_info && !uv.empty()) mesh.uvs.push_back(corner_value(uv, uv_info->interpolation, face, corner, vertex));
        }
        for (std::size_t k = 1; k + 1 < count; ++k) {
            mesh.indices.insert(mesh.indices.end(), {base, base + static_cast<std::uint32_t>(k),
                                                     base + static_cast<std::uint32_t>(k + 1)});
        }
    }

    if (auto material = prim.bound_material("full")) mesh.material = read_material(stage, *material);
    return mesh;
}

int main(int argc, char** argv) {
    if (argc < 2) {
        std::fprintf(stderr, "usage: mesh_import <file.usd>\n");
        return 2;
    }
    try {
        auto stage = tinyusd::Stage::open(argv[1]);
        tinyusd::Value up;
        stage.metadata("upAxis", up);
        std::printf("up axis: %s\n", std::string(up.string()).c_str());

        for (const auto& prim : stage.traverse(tinyusd::Traverse::InstanceProxies)) {
            if (!prim.is_a("Mesh")) continue;
            auto mesh = import_mesh(stage, prim);
            std::printf("mesh %s: %zu corners, %zu triangles, normals %s, uvs %s\n", mesh.name.c_str(),
                        mesh.positions.size(), mesh.indices.size() / 3, mesh.normals.empty() ? "no" : "yes",
                        mesh.uvs.empty() ? "no" : "yes");
            if (!mesh.positions.empty()) {
                const auto& f = mesh.positions.front();
                std::printf("  first corner (world): %.3f %.3f %.3f\n", f[0], f[1], f[2]);
            }
            if (mesh.material) {
                const auto& m = *mesh.material;
                std::printf("  material %s: roughness %.2f, metallic %.2f, ", m.name.c_str(), m.roughness, m.metallic);
                if (m.base_color_texture) {
                    std::printf("base color texture %s (wrap %s/%s)\n", m.base_color_texture->path.c_str(),
                                m.base_color_texture->wrap_s.c_str(), m.base_color_texture->wrap_t.c_str());
                } else {
                    std::printf("base color %.2f %.2f %.2f\n", m.base_color[0], m.base_color[1], m.base_color[2]);
                }
            }
        }
    } catch (const tinyusd::Error& e) {
        std::fprintf(stderr, "error: %s\n", e.what());
        return 1;
    }
    return 0;
}
