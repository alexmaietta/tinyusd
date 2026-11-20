#include <tinyusd/tinyusd.hpp>

#include <algorithm>
#include <array>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <vector>

#define CHECK(cond)                                                                       \
    do {                                                                                  \
        if (!(cond)) {                                                                    \
            std::fprintf(stderr, "%s:%d: CHECK(%s) failed\n", __FILE__, __LINE__, #cond); \
            std::exit(1);                                                                 \
        }                                                                                 \
    } while (0)

using Vec2f = std::array<float, 2>;

static const std::vector<std::string> kFamilies = {"usd",      "usdGeom",   "usdShade",  "usdVol",
                                                   "usdLux",   "usdRender", "usdPhysics"};

static bool same_value(const tinyusd::Value& a, const tinyusd::Value& b) {
    if (a.type() != b.type() || a.is_array() != b.is_array() || a.size() != b.size()) return false;
    const auto x = a.bytes();
    const auto y = b.bytes();
    return a.strings() == b.strings() && x.size() == y.size() && std::equal(x.begin(), x.end(), y.begin());
}

static void same_metadata(const tinyusd::Prim& a, const tinyusd::Prim& b, const std::string& key) {
    tinyusd::Value x, y;
    const bool fx = a.metadata(key, x);
    const bool fy = b.metadata(key, y);
    if (fx != fy || (fx && !same_value(x, y))) {
        std::fprintf(stderr, "%s differs on %s\n", std::string(a.path()).c_str(), key.c_str());
        std::exit(1);
    }
}

static void same_attribute(const tinyusd::Prim& a, const tinyusd::Prim& b, const std::string& name) {
    auto ia = a.attribute_info(name);
    auto ib = b.attribute_info(name);
    CHECK(ia && ib);
    CHECK(ia->type_name == ib->type_name && ia->uniform == ib->uniform);
    tinyusd::Value x, y;
    const bool fx = a.get(name, x);
    const bool fy = b.get(name, y);
    if (fx != fy || (fx && !same_value(x, y))) {
        std::fprintf(stderr, "%s.%s fallback differs\n", std::string(a.path()).c_str(), name.c_str());
        std::exit(1);
    }
    tinyusd::Value bx, by;
    const bool hx = a.attribute_metadata(name, "customData:userDocBrief", bx);
    const bool hy = b.attribute_metadata(name, "customData:userDocBrief", by);
    CHECK(hx == hy && (!hx || bx.string() == by.string()));
}

static void compare_schematics(const std::string& ours, const std::string& theirs) {
    auto a = tinyusd::Stage::open(ours);
    auto b = tinyusd::Stage::open(theirs);
    const auto flags = tinyusd::Traverse::All;
    auto pa = a.traverse(flags);
    auto pb = b.traverse(flags);
    CHECK(pa.size() == pb.size());
    for (const auto& prim : pb) {
        auto mine = a.prim(std::string(prim.path()));
        CHECK(mine.has_value());
        CHECK(mine->type_name() == prim.type_name());
        CHECK(mine->specifier() == prim.specifier());
        same_metadata(*mine, prim, "apiSchemas");
        same_metadata(*mine, prim, "customData:userDocBrief");
        same_metadata(*mine, prim, "customData:apiSchemaOverridePropertyNames");
        auto names = prim.attribute_names(true);
        CHECK(mine->attribute_names(true) == names);
        CHECK(mine->relationship_names(true) == prim.relationship_names(true));
        for (const auto& name : names) same_attribute(*mine, prim, name);
    }
}

int main(int argc, char** argv) {
    CHECK(argc > 2);
    const std::string usd = std::string(argv[1]) + "/openusd-26.11.1";
    const std::string out = argv[2];
    CHECK(tinyusd::has_feature(tinyusd::Feature::Schemas));

    for (const auto& family : kFamilies) {
        auto library = tinyusd::read_schema_library(usd + "/" + family + "/schema.usda", {usd});
        CHECK(library.name == family);
        CHECK(!library.classes.empty());
        auto name = library.global_data.get("libraryName");
        CHECK(name && name->string() == family);
        const std::string dir = out + "/cpp_" + family;
        library.write_plugin(dir);
        compare_schematics(dir + "/generatedSchema.usda", usd + "/" + family + "/generatedSchema.usda");
    }

    auto geom = tinyusd::read_schema_library(usd + "/usdGeom/schema.usda", {usd});
    CHECK(geom.prefix == "UsdGeom");
    const auto* mesh = geom.find_class("Mesh");
    CHECK(mesh && mesh->kind == tinyusd::SchemaKind::ConcreteTyped && mesh->cpp_name == "UsdGeomMesh");
    CHECK(mesh->bases.front() == "PointBased" && mesh->bases.back() == "Typed");
    const auto* scheme = mesh->find_property("subdivisionScheme");
    CHECK(scheme && scheme->declared_by == "Mesh" && scheme->uniform && scheme->type_name == "token");
    CHECK(scheme->fallback && scheme->fallback->string() == "catmullClark");
    CHECK(scheme->allowed_tokens.size() == 4 && scheme->allowed_tokens[1] == "loop");
    const auto* points = mesh->find_property("points");
    CHECK(points && points->declared_by == "PointBased" && !points->fallback);
    const auto* color = mesh->find_property("primvars:displayColor");
    CHECK(color && color->custom_data.get("apiName")->string() == "displayColor");
    CHECK(mesh->find_property("proxyPrim")->relationship);
    const auto* plate = geom.find_class("BackPlateAPI");
    CHECK(plate && plate->kind == tinyusd::SchemaKind::MultipleApplyApi);
    CHECK(plate->property_namespace_prefix == std::optional<std::string>("backPlate"));
    const auto& tweak = plate->find_property("scale:tweak")->fallback;
    CHECK(tweak && tweak->as<Vec2f>() == (Vec2f{1, 1}));
    CHECK(geom.find_class("VisibilityAPI")->can_only_apply_to == std::vector<std::string>{"Imageable"});
    CHECK(std::any_of(geom.tokens.begin(), geom.tokens.end(), [](const auto& t) { return t.name == "interpolation"; }));
    CHECK(!geom.global_data.keys("libraryTokens").empty());
    CHECK(geom.global_data.is_dictionary("libraryTokens"));

    auto shade = tinyusd::read_schema_library(usd + "/usdShade/schema.usda", {usd});
    const auto* material = shade.find_class("Material");
    CHECK(material->custom_data.is_dictionary("extraPlugInfo"));
    CHECK(shade.find_class("Shader")->api_schemas == std::vector<std::string>{"NodeDefAPI"});

    auto nodes = tinyusd::read_shader_defs(usd + "/usdShaders/shaderDefs.usda", {usd});
    auto surface = std::find_if(nodes.begin(), nodes.end(), [](const auto& n) { return n.id == "UsdPreviewSurface"; });
    CHECK(surface != nodes.end());
    auto roughness = std::find_if(surface->inputs.begin(), surface->inputs.end(),
                                  [](const auto& p) { return p.name == "roughness"; });
    CHECK(roughness != surface->inputs.end() && roughness->fallback->as<float>() == 0.5f);
    auto texture = std::find_if(nodes.begin(), nodes.end(), [](const auto& n) { return n.id == "UsdUVTexture"; });
    CHECK(texture->sdr_metadata.get("role")->string() == "texture");

    const std::string written = out + "/plugin_usdGeom";
    geom.write_plugin(written);
    auto plugin = tinyusd::read_schema_plugin(written);
    CHECK(plugin.name == "usdGeom");
    CHECK(plugin.find_class("Mesh") && plugin.find_class("VisibilityAPI"));
    const auto* plugin_sphere = plugin.find_class("Sphere");
    CHECK(plugin_sphere && plugin_sphere->kind == tinyusd::SchemaKind::ConcreteTyped);
    CHECK(plugin_sphere->bases.front() == "Gprim");
    CHECK(plugin_sphere->find_property("radius")->fallback->as<double>() == 1.0);
    CHECK(plugin_sphere->find_property("extent")->declared_by == "Sphere");
    CHECK(plugin_sphere->find_property("visibility")->declared_by == "Imageable");
    CHECK(plugin.find_class("BackPlateAPI")->property_namespace_prefix == "backPlate");

    tinyusd::OpenOptions builtin;
    auto before = tinyusd::Stage::open(std::string(argv[1]) + "/backplate.usda", builtin);
    CHECK(!before.prim("/Camera")->get<Vec2f>("backPlate:plate:scale:tweak"));
    tinyusd::OpenOptions newer;
    newer.schema_paths = {usd + "/usdGeom"};
    auto after = tinyusd::Stage::open(std::string(argv[1]) + "/backplate.usda", newer);
    CHECK(after.prim("/Camera")->get<Vec2f>("backPlate:plate:scale:tweak") ==
          (Vec2f{1, 1}));

    std::puts("ok");
    return 0;
}
