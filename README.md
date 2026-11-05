# tinyusd

A small, dependency-free OpenUSD reader for C, C++ and Python, written in Rust on top of the [`openusd`](https://github.com/mxpv/openusd) crate.

tinyusd opens `.usda`, `.usdc` and `.usdz` files, composes them the same way OpenUSD does (sublayers, references, payloads, variants, inherits, specializes, instancing), and lets you query the resulting prims, attributes, relationships, connections, metadata and assets. It does not invent its own scene format. Your importer reads USD data directly and builds whatever structures your engine or simulator needs.

> **Status:** 0.1.0, early. The query API, the resolution helpers (transforms, primvars, materials, point instancers, skeletons and skinning), host file systems and payload loading are complete and checked against OpenUSD on our fixtures and on Pixar's Kitchen Set. See [Roadmap](#roadmap).

## At a glance

| | |
|---|---|
| Library size | about 3.9 MB shared library, 1.8 MB gzipped (Linux x86_64, all schema families) |
| Runtime dependencies | libc only. No TBB, Boost, Python or OpenUSD |
| Exported symbols | only the `tusd_*` C API |
| Source build | about 65 s from a clean cache on 18 cores. Needs Rust 1.96+ |
| Prebuilt | download a release archive; no Rust toolchain needed |
| Languages | C99 or C++11 (`tinyusd.h`), C++17 (`tinyusd.hpp`), Python 3.9+ (`pip install tinyusd`) |

## Platforms

| OS | Architectures | Toolchain | Release archives and wheels |
|---|---|---|---|
| Linux | x86_64, aarch64 | GCC or Clang, glibc | `linux-x86_64`, `linux-aarch64`; manylinux wheels |
| macOS | arm64, x86_64 | Apple Clang | `macos-aarch64`, `macos-x86_64` |
| Windows | x64, arm64 | MSVC (MinGW builds from source) | `windows-x86_64`, `windows-aarch64` |

CI builds and tests every row, shared and static. On Windows the static library links against the dynamic C runtime (`/MD`), so link it into projects that use `/MD` too, which is CMake's default.

## C++

```cpp
#include <tinyusd/tinyusd.hpp>

auto stage = tinyusd::Stage::open("robot.usd");

tinyusd::Value up;
stage.metadata("upAxis", up);

tinyusd::Value points, indices, uvs;
for (auto prim : stage.traverse()) {
    if (prim.is_a("Mesh")) {
        prim.get("points", points);
        prim.get("faceVertexIndices", indices);
        auto p = points.as_span<std::array<float, 3>>();
        auto i = indices.as_span<std::int32_t>();
        engine.add_mesh(prim.path(), p.data(), p.size(), i.data(), i.size(), prim.world_transform());
        if (auto st = prim.primvar("st", uvs)) {
            engine.set_uvs(prim.path(), uvs.as_span<std::array<float, 2>>(), st->interpolation);
        }
        if (auto material = prim.bound_material("full")) {
            engine.bind_material(prim.path(), material->path());
        }
    }
    if (prim.has_api("PhysicsRigidBodyAPI")) {
        sim.add_body(prim.path(), prim.get<float>("physics:mass").value_or(0.0f));
    }
    for (const auto& material : prim.targets("material:binding")) {
        engine.bind_material(prim.path(), material);
    }
}
```

`Value` is reusable: pass the same one to every `get` in a loop and nothing is allocated after the first call. `as_span<T>()` returns an empty span if `T` doesn't match the value's scalar type or size. Errors throw `tinyusd::Error`, which carries a `tinyusd::Status`.

## C

```c
#include <tinyusd/tinyusd.h>

TusdOpenOptions options;
tusd_open_options_init(&options);

TusdStage* stage = NULL;
if (tusd_stage_open("robot.usd", &options, &stage) != TUSD_STATUS_OK) {
    fprintf(stderr, "%s\n", tusd_last_error());
    return 1;
}

TusdPrimList* prims = tusd_prim_list_new();
TusdValue* value = tusd_value_new();
bool has = false, is_mesh = false;

tusd_stage_traverse(stage, tusd_stage_pseudo_root(), TUSD_TRAVERSE_FLAGS_DEFAULT, prims);
for (size_t i = 0; i < tusd_prim_list_len(prims); ++i) {
    TusdPrim prim = tusd_prim_list_data(prims)[i];
    tusd_prim_is_a(stage, prim, "Mesh", &is_mesh);
    if (is_mesh && tusd_attribute_get(stage, prim, "points", NULL, value, &has) == TUSD_STATUS_OK && has) {
        engine_add_points(tusd_prim_path(stage, prim), tusd_value_data(value), tusd_value_len(value));
    }
}

tusd_value_free(value);
tusd_prim_list_free(prims);
tusd_stage_free(stage);
```

## Python

```python
import tinyusd

stage = tinyusd.Stage.open("robot.usd", load_payloads=True)
for prim in stage.traverse():
    if prim.is_a("Mesh"):
        points = prim.get("points")
        indices = prim.get("faceVertexIndices")
        uvs = prim.primvar("st")
        dataset.add(prim.path, points, indices, prim.world_transform(), uvs=uvs.value if uvs else None)
```

`points` comes back as an `(N, 3) float32` NumPy array and `indices` as an `int32` array. A `Stage` must be used from the thread that opened it.

## Values

Every attribute and metadata value has one of the types below, either as a single value or as an array.

| Category | Types |
|---|---|
| Scalars | `bool`, `uchar`, `int`, `uint`, `int64`, `uint64`, `half`, `float`, `double`, `timecode` |
| Vectors | `int2/3/4`, `half2/3/4`, `float2/3/4`, `double2/3/4`. Role types such as `point3f`, `color3f`, `normal3f` and `texCoord2f` use their base type; the role is in the type name |
| Quaternions | `quath`, `quatf`, `quatd` |
| Matrices | `matrix2d`, `matrix3d`, `matrix4d` |
| Text | `string`, `token`, `asset`, and path lists from metadata |
| Unsupported | dictionaries, list edits and other metadata-only types are reported as `Unsupported` instead of failing |

Layout rules:

- **Quaternions are stored real part first, `(w, x, y, z)`**, the same order USD files use. Engines that store `(x, y, z, w)` need to reorder.
- **Matrices are row-major**, and transforms keep their translation in the last row (elements 12–14 of a 4×4), as in USD.
- **Halves** are raw IEEE 754 binary16 bits (`uint16_t`) in C and C++, and `float16` in NumPy.
- **Bools** are one byte each (`0` or `1`) in C and C++.
- **Asset values** carry both the path as authored and the resolved path. The resolved path is empty when the asset can't be found, and for files inside a `.usdz` it has the form `package.usdz[inner/file.png]`. Pass it to `read_asset` to get the bytes.

## Traversal

Traversal follows OpenUSD's default rule: prims must be active, loaded and defined, and abstract prims (classes) are skipped. Flags widen it:

| Flag | Includes |
|---|---|
| `INCLUDE_INACTIVE` | inactive prims (their children are never composed, as in OpenUSD) |
| `INCLUDE_UNLOADED` | prims under unloaded payloads. Inactive prims also count as unloaded, so combine this with `INCLUDE_INACTIVE` to see them |
| `INCLUDE_UNDEFINED` | `over`-only prims |
| `INCLUDE_ABSTRACT` | `class` prims |
| `INSTANCE_PROXIES` | descend into instances, visiting the prims of their prototype under each instance's path |

`Traverse::All` in C++ sets the first four.

## Resolution helpers

The query API returns data exactly as authored. The helpers apply USD's rules where getting them right by hand is easy to get wrong. Each returns plain USD data (matrices, index and weight arrays, prims), not a converted representation.

| Helper | Returns | Handles |
|---|---|---|
| `world_transform(prim, time)` | 4×4 row-major matrix | every xformOp type, `!invert!`, `!resetXformStack!`, animated ops, ancestors |
| `local_transform(prim, time)` | 4×4 matrix and whether the prim resets the transform stack | the prim's own ops only |
| `primvar_names(prim)` | names of primvars with values, including inherited ones | |
| `primvar(prim, name, time)` | the value with indices applied, plus interpolation, `elementSize`, whether it was indexed, and the prim it came from | indexed primvars, and `constant` primvars inherited from ancestors. `name` can be `st` or `primvars:st` |
| `bound_material(prim, purpose)` | the material prim (C also returns the winning binding relationship) | direct and inherited bindings, purposes (`""` for all-purpose, `"full"`, `"preview"`), collection bindings, `bindingStrength`, GeomSubset bindings |
| `instancer_instances(prim, time)` | the prototypes, plus a prototype index, id and 4×4 transform for each instance, in the instancer's local space | scale, then orientation, then translation, plus the prototype's own transform; `invisibleIds` and `inactiveIds`; missing `ids`; positions that only have time samples |
| `skel_bindings(root)` | each skeleton under the root, its animation, and the prims it deforms | `skel:skeleton` inheritance under `SkelRoot`, `skel:animationSource`, instance proxies |
| `skeleton_pose(skeleton, time)` | joint paths, parent indices, and the bind, rest, local, skeleton-space and skinning matrices for every joint | applying the animation, remapping its joint order onto the skeleton's, composing down the hierarchy |
| `skin_binding(prim, time)` | joint indices and weights (indices in the skeleton's joint order), influences per point, whether the binding is rigid, `geomBindTransform`, skinning method, and blend-shape names with their weights at `time` | a prim's own `skel:joints` order, blend-shape weights remapped from the animation |
| `blend_shapes(prim, time)` | point indices per blend shape, and every sub-shape (rest, in-betweens, primary) with its offsets and its weight at `time` | in-between interpolation: a renderer adds each sub-shape's offsets multiplied by its weight |
| `skinned_points(prim, time)` | the prim's points with blend shapes and skinning applied, in skeleton space | linear and dual-quaternion skinning, rigid bindings; for engines without GPU skinning or for physics collision meshes |

`bound_material` needs the `shade` feature and the skeleton helpers need `skel`; without them, they return `TUSD_STATUS_FEATURE_DISABLED`. The helpers cache their results on the stage (ancestor transforms, material bindings, UsdSkel bindings), so calling them for every prim in a traversal is cheap.

A renderer driving a skinned character:

```cpp
auto bindings = stage.skel_bindings();
for (const auto& binding : bindings) {
    auto rest = binding.skeleton.skeleton_pose();
    engine.create_skeleton(binding.skeleton.path(), rest->joints, rest->parents, rest->bind);
    for (const auto& prim : binding.skinned) {
        if (auto skin = prim.skin_binding()) {
            engine.attach_skin(prim.path(), binding.skeleton.path(), skin->joint_indices, skin->joint_weights,
                               skin->influences_per_point, skin->geom_bind_transform);
        }
    }
}

for (const auto& binding : bindings) {
    engine.set_joint_matrices(binding.skeleton.path(), binding.skeleton.skeleton_pose(frame)->skinning);
}
```

Scattered geometry from a point instancer, in Python:

```python
inst = prim.instancer_instances(time=frame)
world = prim.world_transform(frame)
for proto, xform in zip(inst.proto_indices, inst.transforms):
    renderer.add_instance(inst.prototypes[proto].path, xform @ world)
```

## Loading from your own file system

By default tinyusd reads files from disk. Engines that keep assets in packed archives, virtual file systems or over the network can supply a resolver instead. It has two jobs:

- **resolve(asset_path, anchor)** turns an asset path into a resolved path, or reports that it doesn't exist. `anchor` is the resolved path of the layer that authored the reference (absent for the root layer), so relative paths like `./textures/wood.png` can be anchored. Resolving an already resolved path must return it unchanged.
- **read(resolved_path)** returns the file's bytes.

All layers, references, payloads, asset-valued attributes and `read_asset` calls go through it. Paths inside `.usdz` packages (`pkg.usdz[inner/file.png]`) are handled by tinyusd: your resolver only ever sees the package itself, and tinyusd reads the entries out of the bytes you return. Each package is read once per stage and kept in memory until the stage is freed.

```cpp
class PakResolver : public tinyusd::Resolver {
public:
    std::optional<std::string> resolve(std::string_view path, std::optional<std::string_view> anchor) override {
        return pak.lookup(path, anchor);
    }
    bool read(std::string_view resolved, std::vector<std::uint8_t>& out) override {
        return pak.read(resolved, out);
    }
};

tinyusd::OpenOptions options;
options.resolver = std::make_shared<PakResolver>();
auto stage = tinyusd::Stage::open("pak://levels/warehouse.usd", options);
```

```python
class PakResolver:
    def resolve(self, asset_path, anchor):
        return pak.lookup(asset_path, anchor)

    def read(self, resolved_path):
        return pak.read(resolved_path)

stage = tinyusd.Stage.open("pak://levels/warehouse.usd", resolver=PakResolver())
```

In C, fill a `TusdResolverCallbacks` with `resolve`, `read` and an optional `release`, and point `TusdOpenOptions.resolver` at it. Callbacks hand results back with `tusd_output_set_string`, `tusd_output_set_bytes` and `tusd_output_set_error`. tinyusd copies the struct during `tusd_stage_open` and calls `release(user_data)` exactly once: when the stage is freed, or straight away if opening fails. Callbacks run on the thread that is using the stage and must not call back into that stage. A C++ resolver that throws is treated as a failure, and the exception message becomes the error.

## Custom schemas

tinyusd knows the standard schema families (UsdGeom, UsdShade, UsdLux, UsdSkel, UsdPhysics, UsdVol, UsdMedia, UsdSemantics and the rest). For your own codeless schemas, pass the folder that holds their `plugInfo.json` and `generatedSchema.usda`, the two files `usdGenSchema` writes, when you open the stage. Upstream's `@PLUG_INFO_ROOT@`-style placeholders may be left in `plugInfo.json`.

```cpp
tinyusd::OpenOptions options;
options.schema_paths.push_back("/path/to/chrono.schema/resources");
auto stage = tinyusd::Stage::open("robot.usd", options);
```

```python
stage = tinyusd.Stage.open("robot.usd", schemas=["/path/to/chrono.schema/resources"])
```

With a schema registered, `is_a` follows its bases (a typed schema derived from `Xformable` is an `Xformable`), `has_api` knows its API schemas, `get` returns its fallback values where nothing is authored, and `attribute_names` lists its properties. `attribute_info(...).has_authored_value` still tells authored values from fallbacks. Without it, the prims still load and authored values read normally, but the type is unknown to `is_a` and there are no fallbacks. In C, set `schema_paths` and `schema_path_count` in `TusdOpenOptions`; a schema that can't be read fails the open with `TUSD_STATUS_SCHEMA_FAILED`.

## Payloads

Open with `load_payloads` off to compose a stage without its payloads, then load just what you need. `loadable(root)` lists the prims that have payloads, and `load(prim, with_descendants)` and `unload(prim)` change what's loaded. Prim handles stay valid across loads and unloads, and the helpers' caches are reset automatically.

## C ABI contract

- **Stages** are opened with `tusd_stage_open` and released with `tusd_stage_free`.
- **Prims** are `uint32_t` handles. They stay valid until their stage is freed. `tusd_stage_pseudo_root()` is the root `/`.
- **Strings returned directly** by `tusd_prim_path`, `tusd_prim_name`, `tusd_prim_type_name`, `tusd_prim_kind`, and in `TusdAttributeInfo`, `TusdPrimvarInfo` and the binding from `tusd_prim_bound_material`, are owned by the stage and stay valid until it is freed. The functions that return a string directly return `NULL` on error.
- **Matrices** are written to a caller-provided array of 16 doubles.
- **Values and lists** (`TusdValue`, `TusdPrimList`, `TusdStringList`) are created and freed by the caller and can be reused. Their contents stay valid until the next call that fills them, or until they are freed.
- **Time** is passed as `const double*`. `NULL` means the default time.
- **Errors.** Fallible calls return a `TusdStatus`. Read the message with `tusd_last_error()`; it is per thread and stays valid until that thread's next tinyusd call. Null or invalid arguments are reported before any other error.
- **Threads.** Use each stage from one thread at a time. Separate stages can be used on separate threads.
- **Prim handles** belong to the stage that produced them. Passing one to a different stage is undefined in C; the C++ and Python wrappers reject it.
- **Panics.** Internal panics never cross the boundary; they are returned as `TUSD_STATUS_PANIC`.
- **Versioning.** `tusd_abi_version()` must equal `TUSD_ABI_VERSION`. `tusd_has_feature()` reports which schema families were compiled in.
- **Options.** Call `tusd_open_options_init` before setting fields. `struct_size` lets fields be added without breaking older callers.

## CMake

```cmake
include(FetchContent)
set(TINYUSD_USE_PREBUILT ON)
FetchContent_Declare(tinyusd GIT_REPOSITORY https://github.com/alexmaietta/tinyusd.git GIT_TAG v0.1.0)
FetchContent_MakeAvailable(tinyusd)
target_link_libraries(my_app PRIVATE tinyusd::tinyusd)
```

To build from source with cargo, leave `TINYUSD_USE_PREBUILT` off (the default). CMake fetches [Corrosion](https://github.com/corrosion-rs/corrosion) and drives cargo for you. An installed copy works with `find_package(tinyusd 0.1 CONFIG REQUIRED)`.

| Option | Default | Meaning |
|---|---|---|
| `TINYUSD_USE_PREBUILT` | `OFF` | Download a release archive instead of building with cargo |
| `TINYUSD_STATIC` | `OFF` | Link a static library instead of a shared one |
| `TINYUSD_FEATURES` | all families | Schema families to compile in: `shade;lux;skel;physics;vol;media;semantics` |
| `TINYUSD_RELEASE_URL` | GitHub releases | Base URL for prebuilt archives (for mirrors) |
| `TINYUSD_PREBUILT_URL` | empty | Exact archive URL; overrides `TINYUSD_RELEASE_URL` |
| `TINYUSD_PREBUILT_SHA256` | empty | Expected archive hash |
| `TINYUSD_BUILD_TESTS` | top-level only | Build the C and C++ tests |
| `TINYUSD_BUILD_EXAMPLES` | top-level only | Build the example importers |
| `TINYUSD_INSTALL` | top-level only | Generate install rules and a CMake package |

## Examples

[examples/](examples/) shows downstream importers built on the query API. They're sample code to copy and adapt, not part of the library.

| Example | What it builds |
|---|---|
| [mesh_import.cpp](examples/cpp/mesh_import.cpp) | GPU-style meshes: world-space triangles with normals and UVs expanded to corners, plus the bound UsdPreviewSurface material, following texture connections |
| [physics_import.cpp](examples/cpp/physics_import.cpp) | A simulator-style description: scene gravity, rigid bodies with mass and pose, colliders with their shape and owning body, and joints with their bodies, axis and limits |
| [dataset.py](examples/python/dataset.py) | A training dataset: world-space points, triangles and inherited semantic labels for every mesh, saved to `.npz` |

The C++ examples build with the project (`TINYUSD_BUILD_EXAMPLES`, on by default at top level) and run as part of `ctest`.

## Repository layout

```
crates/tinyusd-core   Rust query API on top of openusd and openusd-schemas
crates/tinyusd-capi   C ABI; builds libtinyusd (cdylib and staticlib)
crates/tinyusd-py     Python bindings (PyO3 and maturin)
include/tinyusd       tinyusd.h (generated by cbindgen) and tinyusd.hpp (hand-written C++17 wrapper)
cmake/                Corrosion source build, prebuilt download, package config
tests/data            shared USD fixtures (usda, plus usdc and usdz generated by tests/oracle/make_fixtures.py)
tests/oracle          comparison against OpenUSD (usd-core) and fixture generation
tests/cpp             C and C++ tests run through ctest
tests/consumer        find_package consumer used to test installs
examples/             downstream importer examples (C++ and Python)
fuzz/                 cargo-fuzz targets for the USDA, USDC and USDZ readers
licenses/             license files of every third-party crate shipped in the library and wheels (generated)
pyproject.toml        Python package definition (maturin)
```

## Development

```sh
cargo test -p tinyusd-core
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release && cmake --build build -j && ctest --test-dir build
cargo test -p tinyusd-capi
pip install .[test] && pytest
```

Two files are generated and checked by `cargo test -p tinyusd-capi`: the C header `include/tinyusd/tinyusd.h` (from the C ABI crate, using cbindgen) and the `licenses/` directory (from `cargo metadata`). The test fails if either is out of date. After changing the C ABI or dependencies, regenerate both with:

```sh
TINYUSD_BLESS=1 cargo test -p tinyusd-capi
```

### Fuzzing

The fuzz targets in [fuzz/](fuzz/) open arbitrary bytes as a `.usda`, `.usdc` or `.usdz` file through an in-memory resolver, then traverse everything and read every attribute, relationship, transform, primvar and material binding. They need a nightly toolchain and `cargo install cargo-fuzz`.

```sh
cd fuzz
cargo +nightly fuzz run -O open_usdc corpus/open_usdc -- -fork=8 -max_total_time=3600
```

### Checking against OpenUSD

`tests/oracle/compare.py` dumps a whole composed stage through tinyusd (the `dump` example) and through the reference implementation (`pip install usd-core`), then diffs them: every prim, its type, specifier, kind, applied schemas, every authored attribute's type, default value, time samples and connections, every relationship's targets, and authored stage metadata.

```sh
cargo build -p tinyusd-core --example dump
pip install usd-core
python tests/oracle/compare.py path/to/*.usd
python tests/oracle/compare.py --instance-proxies path/to/*.usd
python tests/oracle/compare.py --helpers --time 2.5 path/to/*.usd
```

`--helpers` also compares every helper for every prim: world and local transforms, primvars (flattened, with inheritance), bound materials for the all-purpose, `full` and `preview` purposes, point-instancer instances, skeleton poses, skin bindings and skinned points, at the default time or at `--time`.

## Roadmap

1. Run the fuzzers continuously (OSS-Fuzz or a CI schedule), compare against a larger corpus (ALab, AOUSD conformance assets) and publish performance numbers.
2. Switch `openusd` from a pinned git revision to crates.io once a release includes the APIs tinyusd uses from its `main` branch. The 0.8.0 release lacks primvar inheritance, cached material binding resolution, prim and attribute metadata reads, filtered traversal and a few others. The git checkout is about 530 MB; the published crate is about 1.4 MB.

## License

tinyusd is licensed under the BSD 3-Clause License; see [LICENSE](LICENSE).

The libraries and Python wheels include third-party Rust crates. Each one's license files are in [licenses/](licenses/), copied unchanged, with [licenses/INDEX.txt](licenses/INDEX.txt) listing every crate, its version, its license and where it came from. That includes `openusd` (MIT), which carries OpenUSD schema files and generated code under the Tomorrow Open Source Technology License 1.0 (`licenses/openusd/OpenUSD-LICENSE`). Installed packages put the same files under `share/tinyusd/`, and wheels under `dist-info/licenses/`.
