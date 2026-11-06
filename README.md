[![pipeline](https://img.shields.io/github/actions/workflow/status/alexmaietta/tinyusd/ci.yml?branch=main&label=pipeline)](https://github.com/alexmaietta/tinyusd/actions/workflows/ci.yml)
[![coverage](https://img.shields.io/endpoint?url=https://alexmaietta.github.io/tinyusd/coverage/badge.json)](https://alexmaietta.github.io/tinyusd/coverage/)
[![release](https://img.shields.io/github/v/release/alexmaietta/tinyusd)](https://github.com/alexmaietta/tinyusd/releases)
[![pypi](https://img.shields.io/pypi/v/tinyusd)](https://pypi.org/project/tinyusd/)
[![license](https://img.shields.io/badge/license-BSD_3--Clause-blue)](LICENSE)

# tinyusd

A small, dependency-free OpenUSD reader and writer for C, C++ and Python, written in Rust on top of the [`openusd`](https://github.com/mxpv/openusd) crate.

tinyusd opens `.usda`, `.usdc` and `.usdz` files, composes them the same way OpenUSD does (sublayers, references, payloads, variants, inherits, specializes, instancing), and lets you query the resulting prims, attributes, relationships, connections, metadata and assets. Your importer reads USD data directly and builds whatever structures your engine or simulator needs.

tinyusd is specifically designed for C and C++ projects such as rendering or physics engines to read and write USD data without the need for a full OpenUSD dependency. For Python, it is probably best to still use `usd-core`.

tinyusd also provides some general helpers to apply USD's rules where they are easy to get wrong by hand. On the read side they follow OpenUSD's own APIs to compute world transforms, flattened primvars, bound materials, point-instancer instances, skinning, bounds and the UsdPhysics scene description, and return plain USD data rather than a format of their own. On the write side they author the same conventions back, so meshes, primvars, material bindings, transforms, skeletons and physics get the schemas, relationships and metadata USD expects. They sit on top of the query and write APIs, so you can always drop down to raw prims and attributes instead. See [Resolution helpers](#resolution-helpers), [Physics](#physics) and [Writing](#writing).

> **Status:** 0.2.0, early. The query API, the resolution helpers (transforms, primvars, materials, point instancers, skeletons and skinning), host file systems and payload loading are complete and checked against OpenUSD on our fixtures and on Pixar's Kitchen Set. The [write API](#writing) is new. Files it writes are checked against OpenUSD too. See [Roadmap](#roadmap).

## At a glance

| Property | Value |
|---|---|
| Library size | about 5.0 MB shared library, 2.3 MB gzipped (Linux x86_64, every feature). 4.5 MB without the write API |
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

CI builds and tests every row, shared and static. On Windows the static release archive links against the dynamic C runtime (`/MD`), so link it into projects that use `/MD` too, which is CMake's default. A source build follows `CMAKE_MSVC_RUNTIME_LIBRARY`, so setting it to `MultiThreaded` gives a static library for `/MT` projects.

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
| Unsupported | dictionaries, list edits and other metadata-only types are reported as `Unsupported` instead of failing. Values inside a dictionary are read by key path, as below |

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
| `visible(prim, time)` | whether the prim is drawn | `visibility` inherited from imageable ancestors, so an `invisible` ancestor hides everything under it, and animated visibility |
| `purpose(prim)` | `default`, `render`, `proxy` or `guide` | `purpose` inherited from ancestors |
| `world_bound`, `local_bound`, `untransformed_bound(prim, time, purposes)` | a box in its own frame with the matrix that places it, and the axis-aligned box after that matrix | every boundable type's extent, `extent` and computed extents, children, point instancers, invisible descendants left out, and only the purposes asked for (`default` alone when none are given) |
| `camera(prim, time)` | transform, projection, apertures, focal length, clipping range and planes, f-stop, focus distance, fields of view, the linear exposure scale, and view and projection matrices | the camera's world transform and UsdGeom's lens units, in tenths of a scene unit |
| `light(prim, time)` | the light's type, color, intensity, exposure and normalize flag, its color temperature, `radiance`, and the shape inputs its type has (radius, width, height, length, angle, texture, cone angle, cone softness, focus) | `radiance` is color × blackbody tint × intensity × 2^exposure. Normalizing by area is left to the renderer, which knows the light's size on screen |
| `geom_subsets(prim, time)` | each GeomSubset with its family, element type and indices | |
| `triangulate(prim, time)` | point indices, corner indices (into face-varying data) and the source face of each triangle | fan triangulation, `holeIndices`, and `leftHanded` meshes rewound so every triangle is counterclockwise |
| `normals(prim, time)` | normals with their interpolation, and whether they were authored | `primvars:normals` before `normals`, and smooth area-weighted vertex normals when neither is authored |

Metadata dictionaries such as `customData`, `assetInfo` and `customLayerData` are read by key path. `metadata("customData:pipeline:version")` returns the value at `pipeline`, then `version`, and `metadata_keys("customData")` lists a dictionary's keys. The same works for attribute and stage metadata.

`bound_material` needs the `shade` feature, `light` needs `lux` and the skeleton helpers need `skel`; without them, they return `TUSD_STATUS_FEATURE_DISABLED`. The helpers cache their results on the stage (ancestor transforms, bounds, material bindings, UsdSkel bindings), so calling them for every prim in a traversal is cheap.

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

## Physics

`physics(options)` runs UsdPhysics parsing over a stage, the same algorithm and the same descriptors as upstream's `LoadUsdPhysicsFromRange`. Everything comes back as plain data, with prims as handles.

| Part | What you get |
|---|---|
| Scenes | gravity direction and magnitude. Unauthored gravity defaults to 9.81 m/s² in stage units, pointing down the stage's up axis |
| Collision groups | filtered groups, the invert flag, and groups merged through `mergeGroup` |
| Materials | static and dynamic friction, restitution and density |
| Shapes | the owning body, the pose in that body's frame, materials, filtered collisions and groups, and the geometry with world scale already folded into its sizes (capsules and cylinders take scale along and across their axis separately). Meshes report their approximation and scale |
| Rigid bodies | world position and rotation with scale reported separately, velocities, enabled, kinematic and asleep flags, and filtered pairs |
| Joints | the kind (fixed, revolute, prismatic, spherical, distance, D6, custom), the authored targets and the bodies they resolve to, both local frames, break force and torque, and limits and drives listed per degree of freedom |
| Articulations | roots, joints and bodies |

### Mass properties

`mass_properties(body, mass_info)` mirrors upstream's `UsdPhysicsRigidBodyAPI::ComputeMassProperties`. tinyusd applies UsdPhysics's rules. Authored mass, inertia, center of mass and principal axes on the body come first, then each collider's `MassAPI` mass or density, the body's density, the bound physics material's density and finally 1000 kg/m³. The colliders are then summed into one mass, principal inertia, center of mass and principal axes. The geometry comes from you. `mass_info` is called once for each collider of the body (`mass_colliders(body)` lists them) and returns its volume, its inertia tensor at unit density, its center of mass in its own frame, and its pose in the body's frame, which the parsed shape's `local_pos` and `local_rot` give. Return nothing for a collider you can't describe, and it counts as a unit mass, as upstream does. The callback may query the stage freely. Results are in stage units.

```cpp
auto mass = stage.mass_properties(body.prim, [&](const tinyusd::Prim& collider) -> std::optional<tinyusd::MassInformation> {
    const auto& shape = shapes_by_prim.at(collider.handle());
    auto geometry = chrono_volume_and_inertia(shape);
    return tinyusd::MassInformation{geometry.volume, geometry.inertia, geometry.center_of_mass,
                                    to_double(shape.local_pos), to_double(shape.local_rot)};
});
```

In C, call `tusd_prim_mass_colliders`, fill one `TusdMassInformation` for each collider (a negative volume means "can't describe"), and pass the arrays to `tusd_prim_mass_properties`. In Python, `stage.mass_properties(body, info)` calls `info(collider)`, which returns a dict with `volume`, `inertia` and optionally `center_of_mass`, `local_pos` and `local_rot`, or `None`.

Values are in stage units by default, like upstream. Set `si_units` to convert lengths, positions, velocities, densities, gravity, break forces, linear limits and drive gains to meters and kilograms using `metersPerUnit` and `kilogramsPerUnit`. Angles stay in degrees, as UsdPhysics defines them.

```cpp
tinyusd::PhysicsOptions options;
options.si_units = true;
auto physics = stage.physics(options);
for (const auto& body : physics.rigid_bodies) {
    sim.add_body(body.prim.path(), body.position, body.rotation, body.kinematic);
}
for (const auto& joint : physics.joints) {
    sim.add_joint(joint.kind, joint.body0, joint.body1, joint.local_pos0, joint.local_rot0, joint.limits);
}
```

In Python, `stage.physics(si_units=True)` returns the same data as dicts and lists.

One known difference from upstream. Upstream's parser reports `invertFilteredGroups` as true for a collision group that does not author it, which contradicts the schema, where inverting is opt-in. tinyusd reports false unless it is authored. Groups that author the flag behave the same in both.

## Writing

tinyusd also creates stages, edits stages it opened, and saves them, so a renderer can export a scene and a simulator can record its results as USD. Writing is the `write` feature, on by default, and it adds about 450 KB to the library.

```cpp
auto stage = tinyusd::Stage::create_new("scene.usda");
stage.set_up_axis("Z");
stage.set_meters_per_unit(1.0);
auto world = stage.define_prim("/World", "Xform");
stage.set_default_prim(world);

auto mesh = stage.define_prim("/World/Ground", "Mesh");
mesh.set_mesh(points, face_vertex_counts, face_vertex_indices);  // also writes extent
mesh.set_primvar("st", "texCoord2f[]", uvs, tinyusd::Interpolation::FaceVarying);
mesh.bind_material(stage.define_prim("/World/Looks/Concrete", "Material"));

auto body = stage.define_prim("/World/Crate", "Cube");
body.apply_api("PhysicsRigidBodyAPI");
body.apply_api("PhysicsMassAPI");
body.set("physics:mass", 12.0f);
body.set_trs({0, 0, 2}, {1, 0, 0, 0}, {1, 1, 1});

stage.save();
```

A simulator recording its results over an asset, in Python. The source file is never written.

```python
stage = tinyusd.Stage.create_new("run_042.usda")
stage.add_sublayer("./robot.usda")
stage.set_up_axis("Z")  # stage metadata is read from the root layer only
stage.set_meters_per_unit(1.0)
stage.set_time_range(0, frames - 1, time_codes_per_second=60)

with stage.edit():
    for frame in range(frames):
        sim.step()
        for prim, body in bodies:
            prim.set_trs(body.position, body.orientation_wxyz, time=frame)
stage.save()
```

| Call | What it does |
|---|---|
| `create_new(path)`, `create_in_memory()` | A new stage whose root layer is an empty `.usda`, `.usdc` or `.usd` file, or a layer held only in memory |
| `save()` | Writes every layer with unsaved edits back where it came from. Through your own resolver, a layer whose bytes haven't changed since the last save isn't written again |
| `export(path)`, `export_layer(layer, path)`, `export_bytes(format, layer)` | Writes the flattened stage, or one layer as it stands, to a file or to bytes. The extension picks `.usda`, `.usdc` or `.usdz`. A flattened stage keeps its instancing, as OpenUSD's flatten does. Each prototype is written once, as a `class` prim named `Flattened_Prototype_<n>`, and each instance refers to it |
| `export_package(path)` | A `.usdz` holding the stage and every layer and asset it reaches, textures included, with their paths rewritten |
| `define_prim(path, type)`, `override_prim(path)`, `remove_prim(prim)` | `def` a prim (and missing ancestors), author an `over`, or remove the edit target's spec |
| `set(name, value, time)`, `create_attribute(name, type)`, `block`, `clear` | Values, at the default time or as time samples |
| `set_targets`, `set_connections` | Relationship targets and attribute connections |
| `apply_api`, `remove_api`, `set_type_name`, `set_active`, `set_kind`, `set_instanceable` | Prim schemas and metadata. Multiple-apply schemas take their instance after a colon (`PhysicsDriveAPI:angular`). `remove_api` also hides the schema where a weaker layer applies it |
| `rename_prim`, `reparent_prim`, `move_prim` | Move a prim in every layer of the layer stack that holds it, fixing up references and relationship targets. Prim handles follow the move |
| `add_target` | One more relationship target, keeping the others |
| `add_reference`, `add_payload`, `add_inherit`, `add_specialize`, `set_variant_selection` | Composition arcs. References and payloads take a layer offset and scale (`add_reference_with_offset` in Rust and C, `offset` and `scale` arguments in C++ and Python) to retime the animation they bring in, so stage time `t` reads the asset at `(t - offset) / scale` |
| `remove_reference`, `remove_payload`, `remove_inherit`, `remove_specialize` | Take an arc out, deleting it where a weaker layer adds it. A reference or payload is matched by asset and prim path, whatever its layer offset |
| `remove_property`, `remove_target`, `remove_connection` | Remove the edit target's spec for a property, or one relationship target or attribute connection |
| `set_metadata(key, value)`, `clear_metadata(key)`, `set_property_metadata(name, key, value)`, `clear_property_metadata(name, key)` | Prim and property metadata. Prims take `active`, `comment`, `displayName`, `documentation`, `hidden`, `instanceable` and `kind`. Attributes and relationships take `colorSpace`, `comment`, `displayGroup`, `displayName`, `documentation`, `elementSize`, `hidden` and `interpolation`. Both take `customData` and `assetInfo` entries one at a time as `customData:name` or `customData:outer:inner`, of any value type. Clearing a key or an entry removes the edit target's opinion only |
| `set_up_axis`, `set_meters_per_unit`, `set_kilograms_per_unit`, `set_time_range`, `set_default_prim`, `set_documentation` | Stage metadata, always written to the root layer |
| `edit_target`, `set_edit_target(layer)`, `add_sublayer(path, create)` | Which layer edits go to |
| `edit_variant(prim, set, variant)`, `variant_sets(prim)` | Send later edits inside a variant, declaring and selecting it. `set_edit_target` leaves it |
| `remove_variant(prim, set, variant)` | Delete a variant and everything authored inside it from the edit target, along with that layer's selection of it |

**Values are strictly typed.** A value must match the attribute's declared type exactly, so a `point3f[]` takes a `Float3` array and a `token` takes a `Token`. Nothing is converted, not even `double` to `float`, and a mismatch fails with `TUSD_STATUS_INVALID_VALUE` (`InvalidValueError` in Python). Attributes the prim's schemas declare (`points`, `physics:mass`, `radius`) can be set straight away. Your own need `create_attribute(name, type_name)` first, with a type such as `float`, `double3[]` or `matrix4d`. In C++, `set` infers the type from `float`, `double`, `std::int32_t`, `bool` and `std::array` of those, and `Value::token`, `Value::string` and `Value::asset` build text. In Python, the declared type picks the encoding. NumPy arrays must already have its dtype (`float32` for `point3f[]`), while plain Python numbers and lists are converted within their kind, so an `int` can become a `float` but a `float` never becomes an `int`.

**Batches.** Every edit is written straight away by default. Between `begin_edit` and `commit_edit` (an `EditBatch` in C++, `with stage.edit():` in Python) every edit is queued and written when the batch commits. On error nothing is written. Prims, properties and values are written as one transaction. Schemas, arcs, removals, metadata and stage metadata are run in order between those transactions, and a commit that has any is undone as a whole when one of them fails. Values can be set on the properties a schema applied in the same batch declares. Reads inside a batch see the stage as it was before it, and calls that report what they removed report `false` there, since nothing is removed until the commit. Saving, exporting, changing the edit target, adding sublayers, moving prims and the preview material helpers need the batch committed first. Batching pays off for time samples. One batch held across a whole recording writes each attribute's samples together, which is several times faster than writing them one at a time.

A batch per frame is no faster than single edits. To measure on your own machine, run `cargo run --profile dev-release -p tinyusd-core --example write_bench -- 1000 100`, which writes 1000 bodies' translations over 100 frames each way.

**Overlays.** A layer is always stronger than its sublayers, so to write over an asset without touching it, create a new stage and add the asset as a sublayer, as above. Edits to the new root then win. USD reads stage metadata such as `upAxis`, `metersPerUnit` and `defaultPrim` from the root layer only, so set them on the new root too. `set_edit_target` sends edits into one of the root layer's sublayers instead.

**Write helpers** author USD's conventions for you, the inverse of the resolution helpers.

| Helper | Writes |
|---|---|
| `set_local_transform(matrix, time)` | One `xformOp:transform` and the matching `xformOpOrder` |
| `set_world_transform(matrix, time)` | A local transform that places the prim at `matrix` under its parent |
| `set_trs(translate, orient, scale, time)` | `xformOp:translate`, `xformOp:orient` (`w, x, y, z`) and `xformOp:scale`, as physics engines keep poses. New ops are `double3` and `quatd`; ops the prim already declares as `float3` or `quatf` keep their type |
| `set_mesh(points, counts, indices, time)` | Points, topology and `extent`, after checking that the indices fit |
| `set_primvar(name, type, value, interpolation, indices, element_size, time)` | `primvars:<name>` with its interpolation, and for indexed primvars its `:indices` |
| `bind_material(material, purpose)` | `MaterialBindingAPI` and the `material:binding` relationship |
| `define_preview_material(path)` | A `Material` whose surface output is connected to a `UsdPreviewSurface` shader named `PreviewSurface` under it |
| `set_preview_input(input, value)` | One `UsdPreviewSurface` input on such a material, such as `diffuseColor` (a `color3f`) or `roughness` (a `float`), declared with the type the shader gives it |
| `set_preview_texture(input, asset_path)` | A `UsdUVTexture` reading the image into that input through a shared `st` primvar reader. Colors are read as sRGB and other inputs as raw data, and normal maps are remapped to -1..1. Returns the texture shader |
| `set_instances(prototypes, proto_indices, transforms, ids, time)` | A `PointInstancer`'s prototypes, `protoIndices`, `positions`, `orientationsf`, `scales` and, where given, `ids`, from the instance transforms `instancer_instances` reads without the prototypes' own. Each transform must be translate, rotate and scale, with no shear. Rust and C take the `Instances` the reader returns |
| `define_skeleton(path, joints, bind_transforms, rest_transforms)` | A `Skeleton` with its joint paths, such as `hips/spine`, each after its parent, and their bind and rest transforms |
| `set_skel_animation(joints, local_transforms, time)` | A `SkelAnimation`'s joints and, at `time`, their local transforms as translations, rotations and scales |
| `bind_skeleton(skeleton, animation)` | `SkelBindingAPI` and `skel:skeleton` on a mesh or `SkelRoot`, and `skel:animationSource` on the skeleton where an animation is given |
| `set_skinning(joint_indices, joint_weights, influences_per_point, geom_bind_transform, joints)` | The joint influence primvars, per point or, for one set, rigid, with the geom bind transform and an optional joint order |
| `define_blend_shape(path, offsets, normal_offsets, point_indices)`, `set_blend_shape_inbetween(name, weight, offsets, normal_offsets)` | A `BlendShape` with its offsets, optional normal offsets and the points they move, every point where none are given, and its inbetween shapes at their weights |
| `set_blend_shapes(names, targets)`, `set_blend_shape_weights(names, weights, time)` | The named blend shapes a mesh binds, and an animation's weights for them |
| `set_value_clips(set, asset_paths, prim_path, active, times, manifest)`, `export_clip_manifest(set, path)` | Value clips playing the prim's attributes from other layers, and the manifest naming the attributes they animate. Clips give values only to attributes the scene declares |

The helpers' caches are reset after every edit, so reads always see what was written. Splines aren't written, as `openusd` doesn't support them yet.

**Physics** is written from the same structures `physics()` returns, so a simulator can read a scene, change it and write it back, or build one from scratch. `write_physics(physics, options)` writes everything, converting SI values back to stage units, and `write_physics_scene`, `write_collision_group`, `write_physics_material`, `write_shape`, `write_rigid_body`, `write_joint`, `write_articulation` and `write_body_mass` write one item each, in stage units. Prims of the schema's types (scenes, collision groups, joints) are defined with that type, and API schemas are applied where missing. Each joint kind's limits and drives are written the way UsdPhysics declares them, with a disabled limit written as an unbounded one, and joint frames are written against the prims the joint was read through, carried back into their frames, or against `body0` and `body1` for a new joint. A collider's materials are written back to the face subsets they were read from, then to the collider itself. Collider geometry and poses are the gprims' own attributes and are left alone. A rigid body's world pose is written as its transform only with `poses` on, into the translate, `orient` or `rotateXYZ` and scale ops the body already has where they can hold it, and as one matrix otherwise. Writing back what `physics()` read gives the same `physics()` result, which the tests check on the fixtures. Start new items from `RigidBody::new(prim)`, `Joint::new(prim, kind)` and the like in Rust, from the structures' defaults in C++, or leave keys out of the dicts in Python. Each of these takes UsdPhysics's fallbacks.

```python
stage.write_physics({
    "rigid_bodies": [{"prim": arm, "position": [0, 0, 1.5]}, {"prim": base, "kinematic": True}],
    "joints": [{"prim": hinge, "kind": "Revolute", "axis": "Y", "body0": base, "body1": arm,
                "limits": [{"dof": "RotY", "lower": -45, "upper": 90}],
                "drives": [{"dof": "RotY", "stiffness": 50, "damping": 5}]}],
}, poses=True)
```

In C, fill the same view structs the read functions return and pass them to `tusd_stage_write_rigid_body` and the others, or collect them in a `TusdPhysics` with `tusd_physics_add_rigid_body` and friends and write it with `tusd_stage_write_physics`. An enum field outside its range is reported as `TUSD_STATUS_INVALID_ARGUMENT`.

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

To save or export a stage opened through a resolver, give it a third job, **write(resolved_path, bytes)**, which stores a file. Every save and export then goes through it instead of the disk. `write` is optional (a `Resolver` method in C++, a method on the object in Python, and a callback in C) and without it saving fails with a write error.

In C, fill a `TusdResolverCallbacks` with `resolve`, `read` and the optional `write` and `release`, and point `TusdOpenOptions.resolver` at it. Callbacks hand results back with `tusd_output_set_string`, `tusd_output_set_bytes` and `tusd_output_set_error`. tinyusd copies the struct during `tusd_stage_open` and calls `release(user_data)` exactly once: when the stage is freed, or straight away if opening fails. Callbacks run on the thread that is using the stage and must not call back into that stage. A C++ resolver that throws is treated as a failure, and the exception message becomes the error.

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

With a schema registered, `is_a` follows its bases (a typed schema derived from `Xformable` is an `Xformable`), `has_api` knows its API schemas, `get` returns its fallback values where nothing is authored, and `attribute_names` lists its properties. `attribute_info(...).has_authored_value` still tells authored values from fallbacks. Without it, the prims still load and authored values read normally, but the type is unknown to `is_a` and there are no fallbacks. Applied API schemas are still reported by `applied_schemas` and `has_api` from their authored names, whereas upstream's `GetAppliedSchemas` leaves out schemas it doesn't know. In C, set `schema_paths` and `schema_path_count` in `TusdOpenOptions`; a schema that can't be read fails the open with `TUSD_STATUS_SCHEMA_FAILED`.

## Payloads

Open with `load_payloads` off to compose a stage without its payloads, then load just what you need. `loadable(root)` lists the prims that have payloads, and `load(prim, with_descendants)` and `unload(prim)` change what's loaded. Prim handles stay valid across loads and unloads, and the helpers' caches are reset automatically.

## C ABI contract

- **Stages** are opened with `tusd_stage_open` and released with `tusd_stage_free`.
- **Prims** are `uint32_t` handles. They stay valid until their stage is freed. `tusd_stage_pseudo_root()` is the root `/`.
- **Strings returned directly** by `tusd_prim_path`, `tusd_prim_name`, `tusd_prim_type_name`, `tusd_prim_kind`, `tusd_stage_edit_target`, `tusd_stage_add_sublayer`, and in `TusdAttributeInfo`, `TusdPrimvarInfo` and the binding from `tusd_prim_bound_material`, are owned by the stage and stay valid until it is freed. The functions that return a string directly return `NULL` on error.
- **Matrices** are written to a caller-provided array of 16 doubles.
- **Values and lists** (`TusdValue`, `TusdPrimList`, `TusdStringList`) are created and freed by the caller and can be reused. Fill a `TusdValue` to write with `tusd_value_set_data` or `tusd_value_set_strings`. Their contents stay valid until the next call that fills them, or until they are freed.
- **Time** is passed as `const double*`. `NULL` means the default time.
- **Errors.** Fallible calls return a `TusdStatus`. Read the message with `tusd_last_error()`; it is per thread and stays valid until that thread's next tinyusd call. Null or invalid arguments are reported before any other error.
- **Threads.** Use each stage from one thread at a time. Separate stages can be used on separate threads.
- **Prim handles** belong to the stage that produced them. Passing one to a different stage is undefined in C; the C++ and Python wrappers reject it.
- **Panics.** Internal panics never cross the boundary; they are returned as `TUSD_STATUS_PANIC`.
- **Versioning.** `tusd_abi_version()` must equal `TUSD_ABI_VERSION`. `tusd_has_feature()` reports which schema families, and whether the write API (`TUSD_FEATURE_WRITE`), were compiled in. Without it, the write functions return `TUSD_STATUS_FEATURE_DISABLED`.
- **Options.** Call `tusd_open_options_init` before setting fields. `struct_size` lets fields be added without breaking older callers.

## CMake

```cmake
include(FetchContent)
set(TINYUSD_USE_PREBUILT ON)
FetchContent_Declare(tinyusd GIT_REPOSITORY https://github.com/alexmaietta/tinyusd.git GIT_TAG v0.2.0)
FetchContent_MakeAvailable(tinyusd)
target_link_libraries(my_app PRIVATE tinyusd::tinyusd)
```

To build from source with cargo, leave `TINYUSD_USE_PREBUILT` off (the default). CMake fetches [Corrosion](https://github.com/corrosion-rs/corrosion) and drives cargo for you. An installed copy works with `find_package(tinyusd 0.2 CONFIG REQUIRED)`.

| Option | Default | Meaning |
|---|---|---|
| `TINYUSD_USE_PREBUILT` | `OFF` | Download a release archive instead of building with cargo |
| `TINYUSD_STATIC` | `OFF` | Link a static library instead of a shared one |
| `TINYUSD_FEATURES` | everything | Schema families and the write API to compile in: `shade;lux;skel;physics;vol;media;semantics;write` |
| `TINYUSD_RELEASE_URL` | GitHub releases | Base URL for prebuilt archives (for mirrors) |
| `TINYUSD_PREBUILT_URL` | empty | Exact archive URL; overrides `TINYUSD_RELEASE_URL` |
| `TINYUSD_PREBUILT_SHA256` | empty | Expected archive hash |
| `TINYUSD_BUILD_TESTS` | top-level only | Build the C and C++ tests |
| `TINYUSD_BUILD_EXAMPLES` | top-level only | Build the example importers |
| `TINYUSD_INSTALL` | top-level only | Generate install rules and a CMake package |

## Packages

Recipes for vcpkg, Conan and conda are in [packaging/](packaging/), and the [packages](.github/workflows/packages.yml) workflow builds each one on Linux, macOS and Windows and runs the C and C++ test suites against the installed package. [packaging/test](packaging/test/) does that for any install, given `-DTINYUSD_TESTS_DIR=<repo>/tests`. They aren't in the vcpkg registry, ConanCenter or conda-forge yet, so use them from a checkout.

| Manager | Builds from | Use it |
|---|---|---|
| vcpkg | the release archives, so no Rust toolchain is needed | `vcpkg install tinyusd --overlay-ports=packaging/vcpkg/ports` |
| Conan 2 | source, with cargo from `PATH` | `conan create packaging/conan`, then `requires = "tinyusd/0.2.0"` |
| conda | source, with conda-forge's Rust compiler | `rattler-build build --recipe packaging/conda/recipe.yaml -c conda-forge` builds `libtinyusd` |

All three provide the same CMake package, `find_package(tinyusd CONFIG REQUIRED)` with the target `tinyusd::tinyusd`. The vcpkg port follows the triplet's linkage and supports every platform in the table above, except Windows triplets with a static CRT (`x64-windows-static`), because the static archive uses `/MD`. Use `x64-windows-static-md` instead. The Conan recipe has a `shared` option (off by default), a `write` option and a `with_<family>` option for each schema family, and follows `compiler.runtime`. The conda package is the shared library.

The port's hashes are filled in for each release by `packaging/update.py`, which the release workflow runs to attach `tinyusd-<version>-vcpkg-port.tar.gz` to the release. Run `packaging/update.py <version>` after a release to point the port in the repository at it.

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
packaging/            vcpkg port, Conan recipe, conda recipe and the test they share
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

The fuzz targets in [fuzz/](fuzz/) open arbitrary bytes as a `.usda`, `.usdc` or `.usdz` file through an in-memory resolver, then traverse everything and read every attribute, relationship, transform, primvar and material binding. `edit_sequence` reads its bytes as a sequence of edits instead (defining, removing, moving and overriding prims, typed and mistyped values, time samples, batches, schemas, references with layer offsets, variants, metadata, removals and the write helpers), then checks that the flattened stage exports and reads back as `.usda` and `.usdc` with the same prims. They need a nightly toolchain and `cargo install cargo-fuzz`. `cargo test` in `fuzz/` also runs `edit_sequence` over a few hundred generated inputs on stable.

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

To check the write API, `tests/oracle/write_scenes.py` writes scenes with every format, write helper, physics item, variant and move through the Python package. `compare.py` then checks that OpenUSD reads them the way tinyusd does, and `tests/oracle/validate.py` runs every OpenUSD validator (`UsdValidation`) over them. CI runs all three.

```sh
python tests/oracle/write_scenes.py target/oracle-write
python tests/oracle/compare.py --helpers target/oracle-write/*.usd?
python tests/oracle/compare.py --physics target/oracle-write/physics_*.usda
python tests/oracle/validate.py target/oracle-write/written.usda target/oracle-write/physics_written.usda
```

`--helpers` also compares every helper for every prim, at the default time or at `--time`. That covers world and local transforms, primvars (flattened, with inheritance), bound materials for the all-purpose, `full` and `preview` purposes, point-instancer instances, skeleton poses, skin bindings, skinned points, visibility, purpose, world bounds and cameras.

## Roadmap

1. Run the fuzzers continuously (OSS-Fuzz or a CI schedule), compare against a larger corpus (ALab, AOUSD conformance assets) and publish performance numbers.
2. Switch `openusd` from a pinned git revision to crates.io once a release includes the APIs tinyusd uses from its `main` branch. The 0.8.0 release lacks primvar inheritance, cached material binding resolution, prim and attribute metadata reads, filtered traversal and a few others. The git checkout is about 530 MB; the published crate is about 1.4 MB.

## License

tinyusd is licensed under the BSD 3-Clause License; see [LICENSE](LICENSE).

The libraries and Python wheels include third-party Rust crates. Each one's license files are in [licenses/](licenses/), copied unchanged, with [licenses/INDEX.txt](licenses/INDEX.txt) listing every crate, its version, its license and where it came from. That includes `openusd` (MIT), which carries OpenUSD schema files and generated code under the Tomorrow Open Source Technology License 1.0 (`licenses/openusd/OpenUSD-LICENSE`). Installed packages put the same files under `share/tinyusd/`, and wheels under `dist-info/licenses/`.
