#include <tinyusd/tinyusd.hpp>

#include <cstdio>
#include <string>

int main() {
    if (tusd_abi_version() != TUSD_ABI_VERSION) {
        std::fprintf(stderr, "ABI mismatch: header %u, library %u\n", TUSD_ABI_VERSION, tusd_abi_version());
        return 1;
    }
    if (!tusd_has_feature(TUSD_FEATURE_WRITE)) {
        std::puts("tinyusd without the write API, ABI checked");
        return 0;
    }
    auto stage = tinyusd::Stage::create_in_memory();
    stage.define_prim("/World", "Xform");
    auto bytes = stage.export_bytes(tinyusd::LayerFormat::Usda);
    std::string text(bytes.begin(), bytes.end());
    if (text.find("def Xform \"World\"") == std::string::npos) {
        std::fprintf(stderr, "unexpected export:\n%s\n", text.c_str());
        return 1;
    }
    std::puts("tinyusd package works");
    return 0;
}
