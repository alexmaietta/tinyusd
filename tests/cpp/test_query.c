#include <tinyusd/tinyusd.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHECK(cond)                                                                                     \
    do {                                                                                                \
        if (!(cond)) {                                                                                  \
            fprintf(stderr, "%s:%d: CHECK(%s) failed: %s\n", __FILE__, __LINE__, #cond, tusd_last_error()); \
            exit(1);                                                                                    \
        }                                                                                               \
    } while (0)

static int releases = 0;

static bool deny_resolve(void* user_data, const char* asset_path, const char* anchor, TusdOutput* out) {
    (void)user_data;
    (void)asset_path;
    (void)anchor;
    (void)out;
    return false;
}

static bool deny_read(void* user_data, const char* resolved_path, TusdOutput* out) {
    (void)user_data;
    (void)resolved_path;
    tusd_output_set_error(out, "denied");
    return false;
}

static void count_release(void* user_data) {
    ++*(int*)user_data;
}

int main(int argc, char** argv) {
    char path[4096];
    TusdOpenOptions options;
    TusdStage* stage = NULL;
    TusdPrim prim = 0;
    bool found = false;
    bool has = false;
    double t = 5.5;
    size_t i, mesh_count = 0;
    TusdValue* value = tusd_value_new();
    TusdPrimList* prims = tusd_prim_list_new();
    TusdStringList* strings = tusd_string_list_new();

    CHECK(argc > 1);
    snprintf(path, sizeof path, "%s/query.usda", argv[1]);
    tusd_open_options_init(&options);
    CHECK(tusd_stage_open(path, &options, &stage) == TUSD_STATUS_OK);

    CHECK(tusd_stage_traverse(stage, tusd_stage_pseudo_root(), TUSD_TRAVERSE_FLAGS_DEFAULT, prims) == TUSD_STATUS_OK);
    CHECK(tusd_prim_list_len(prims) == 10);
    for (i = 0; i < tusd_prim_list_len(prims); ++i) {
        bool is_mesh = false;
        TusdPrim p = tusd_prim_list_data(prims)[i];
        CHECK(tusd_prim_is_a(stage, p, "Mesh", &is_mesh) == TUSD_STATUS_OK);
        if (is_mesh) {
            ++mesh_count;
            CHECK(tusd_attribute_get(stage, p, "points", NULL, value, &has) == TUSD_STATUS_OK && has);
            CHECK(tusd_value_type(value) == TUSD_VALUE_TYPE_FLOAT3);
            CHECK(tusd_value_data_size(value) == tusd_value_len(value) * 3 * sizeof(float));
            CHECK(tusd_prim_attribute_names(stage, p, true, strings) == TUSD_STATUS_OK);
            CHECK(tusd_string_list_len(strings) > 0);
        }
    }
    CHECK(mesh_count == 4);

    CHECK(tusd_stage_prim_at_path(stage, "/World/Types", &prim, &found) == TUSD_STATUS_OK && found);
    CHECK(strcmp(tusd_prim_path(stage, prim), "/World/Types") == 0);
    CHECK(strcmp(tusd_prim_type_name(stage, prim), "Scope") == 0);
    CHECK(tusd_attribute_get(stage, prim, "animated", &t, value, &has) == TUSD_STATUS_OK && has);
    CHECK(*(const float*)tusd_value_data(value) == 4.5f);
    CHECK(tusd_attribute_get(stage, prim, "ta", NULL, value, &has) == TUSD_STATUS_OK && has);
    CHECK(tusd_value_len(value) == 2 && strcmp(tusd_value_string(value, 0), "a") == 0);
    CHECK(tusd_value_string(value, 2) == NULL);
    CHECK(tusd_relationship_targets(stage, prim, "target", false, strings) == TUSD_STATUS_OK);
    CHECK(strcmp(tusd_string_list_get(strings, 0), "/World/Mesh") == 0);
    CHECK(tusd_attribute_get(stage, prim, "blocked", NULL, value, &has) == TUSD_STATUS_OK && !has);
    CHECK(tusd_value_type(value) == TUSD_VALUE_TYPE_EMPTY);

    CHECK(tusd_stage_prim_at_path(stage, "/World/Nope", &prim, &found) == TUSD_STATUS_OK && !found);
    CHECK(tusd_stage_prim_at_path(stage, "not a path", &prim, &found) == TUSD_STATUS_INVALID_PATH);
    CHECK(strstr(tusd_last_error(), "not a path") != NULL);
    CHECK(tusd_prim_path(stage, 99999) == NULL);
    CHECK(tusd_prim_flags(stage, 99999, NULL) == TUSD_STATUS_INVALID_ARGUMENT);
    tusd_stage_free(stage);

    snprintf(path, sizeof path, "%s/resolve.usda", argv[1]);
    CHECK(tusd_stage_open(path, &options, &stage) == TUSD_STATUS_OK);
    CHECK(tusd_stage_prim_at_path(stage, "/World/Group/Own", &prim, &found) == TUSD_STATUS_OK && found);
    {
        double m[16];
        bool resets = true;
        TusdPrimvarInfo info;
        TusdPrim material = 0;
        const char* binding = NULL;
        CHECK(tusd_prim_world_transform(stage, prim, NULL, m) == TUSD_STATUS_OK && m[12] == 10.0);
        CHECK(tusd_prim_local_transform(stage, prim, &t, m, &resets) == TUSD_STATUS_OK && !resets && m[13] == 5.5);
        CHECK(tusd_prim_world_transform(stage, prim, NULL, NULL) == TUSD_STATUS_INVALID_ARGUMENT);
        CHECK(tusd_prim_primvar(stage, prim, "st", NULL, value, &info, &found) == TUSD_STATUS_OK && found);
        CHECK(info.interpolation == TUSD_INTERPOLATION_FACE_VARYING && info.indexed && tusd_value_len(value) == 7);
        CHECK(strcmp(tusd_prim_path(stage, info.source), "/World/Group/Own") == 0);
        if (tusd_has_feature(TUSD_FEATURE_SHADE)) {
            CHECK(tusd_prim_bound_material(stage, prim, NULL, &material, &binding, &found) == TUSD_STATUS_OK && found);
            CHECK(strcmp(tusd_prim_path(stage, material), "/World/Looks/Blue") == 0);
            CHECK(strcmp(binding, "/World/Group/Own.material:binding") == 0);
        }
    }
    tusd_stage_free(stage);

    snprintf(path, sizeof path, "%s/instancer.usda", argv[1]);
    CHECK(tusd_stage_open(path, &options, &stage) == TUSD_STATUS_OK);
    CHECK(tusd_stage_prim_at_path(stage, "/World/Trees", &prim, &found) == TUSD_STATUS_OK && found);
    {
        TusdInstances* instances = tusd_instances_new();
        TusdInstancesView view;
        CHECK(tusd_prim_instancer_instances(stage, prim, NULL, true, true, instances, &found) == TUSD_STATUS_OK && found);
        CHECK(tusd_instances_view(instances, &view) == TUSD_STATUS_OK);
        CHECK(view.count == 2 && view.prototype_count == 2 && view.ids[1] == 12 && view.transforms[16 + 12] == 10.0);
        tusd_instances_free(instances);
    }
    tusd_stage_free(stage);

    {
        TusdResolverCallbacks callbacks;
        memset(&callbacks, 0, sizeof callbacks);
        callbacks.struct_size = sizeof callbacks;
        callbacks.user_data = &releases;
        callbacks.resolve = deny_resolve;
        callbacks.read = deny_read;
        callbacks.release = count_release;
        tusd_open_options_init(&options);
        options.resolver = &callbacks;
        CHECK(tusd_stage_open("anything.usda", &options, &stage) == TUSD_STATUS_OPEN_FAILED && stage == NULL);
        CHECK(releases == 1);
        callbacks.read = NULL;
        CHECK(tusd_stage_open("anything.usda", &options, &stage) == TUSD_STATUS_INVALID_ARGUMENT);
        CHECK(releases == 2);
        tusd_open_options_init(&options);
    }

    CHECK(tusd_stage_open(NULL, NULL, &stage) == TUSD_STATUS_INVALID_ARGUMENT);
    options.struct_size = 1;
    CHECK(tusd_stage_open(path, &options, &stage) == TUSD_STATUS_INVALID_ARGUMENT && stage == NULL);

    {
        TusdBound bound;
        TusdCameraInfo camera;
        TusdLightInfo light;
        TusdPurpose purpose;
        TusdInterpolation interpolation;
        TusdValue* corners = tusd_value_new();
        TusdValue* faces = tusd_value_new();
        bool visible = true, authored = true;
        double ten = 10.0;
        snprintf(path, sizeof path, "%s/render.usda", argv[1]);
        CHECK(tusd_stage_open(path, NULL, &stage) == TUSD_STATUS_OK);
        CHECK(tusd_stage_prim_at_path(stage, "/World/Props/Animated/Ball", &prim, &found) == TUSD_STATUS_OK && found);
        CHECK(tusd_prim_visible(stage, prim, NULL, &visible) == TUSD_STATUS_OK && visible);
        CHECK(tusd_prim_visible(stage, prim, &ten, &visible) == TUSD_STATUS_OK && !visible);
        CHECK(tusd_stage_prim_at_path(stage, "/World/Props/Proxy", &prim, &found) == TUSD_STATUS_OK && found);
        CHECK(tusd_prim_purpose(stage, prim, &purpose) == TUSD_STATUS_OK && purpose == TUSD_PURPOSE_PROXY);
        CHECK(tusd_stage_prim_at_path(stage, "/World/Props", &prim, &found) == TUSD_STATUS_OK && found);
        CHECK(tusd_prim_bound(stage, prim, TUSD_BOUND_SPACE_WORLD, NULL, 0, &bound, &found) == TUSD_STATUS_OK && found);
        CHECK(bound.aligned_min[0] == 9.0 && bound.aligned_max[2] == 3.0);
        CHECK(tusd_prim_bound(stage, prim, TUSD_BOUND_SPACE_WORLD, NULL, (1u << TUSD_PURPOSE_DEFAULT) | (1u << TUSD_PURPOSE_PROXY),
                              &bound, &found) == TUSD_STATUS_OK && bound.aligned_min[0] == 6.0);
        CHECK(tusd_stage_prim_at_path(stage, "/World/Shot", &prim, &found) == TUSD_STATUS_OK && found);
        CHECK(tusd_prim_camera(stage, prim, NULL, &camera, &found) == TUSD_STATUS_OK && found);
        CHECK(camera.focal_length == 35.0f && camera.projection == TUSD_PROJECTION_PERSPECTIVE && camera.far == 500.0f);
        CHECK(tusd_stage_prim_at_path(stage, "/World/Key", &prim, &found) == TUSD_STATUS_OK && found);
        CHECK(tusd_prim_light(stage, prim, NULL, &light, &found) == TUSD_STATUS_OK);
        if (tusd_has_feature(TUSD_FEATURE_LUX)) {
            CHECK(found && light.radiance[0] == 40.0f && light.radius == 0.5f && light.width != light.width);
            CHECK(strcmp(light.type_name, "SphereLight") == 0 && light.texture == NULL);
        }
        CHECK(tusd_stage_prim_at_path(stage, "/World/Grid", &prim, &found) == TUSD_STATUS_OK && found);
        CHECK(tusd_prim_geom_subsets(stage, prim, NULL, prims) == TUSD_STATUS_OK && tusd_prim_list_len(prims) == 2);
        CHECK(tusd_prim_triangulate(stage, prim, NULL, value, corners, faces, &found) == TUSD_STATUS_OK && found);
        CHECK(tusd_value_type(value) == TUSD_VALUE_TYPE_INT3 && tusd_value_len(value) == 6);
        CHECK(((const int32_t*)tusd_value_data(faces))[2] == 2);
        CHECK(tusd_prim_normals(stage, prim, NULL, value, &interpolation, &authored, &found) == TUSD_STATUS_OK && found);
        CHECK(!authored && interpolation == TUSD_INTERPOLATION_VERTEX && ((const float*)tusd_value_data(value))[2] == 1.0f);
        CHECK(tusd_stage_prim_at_path(stage, "/World", &prim, &found) == TUSD_STATUS_OK && found);
        CHECK(tusd_prim_metadata(stage, prim, "customData:pipeline:version", value, &has) == TUSD_STATUS_OK && has);
        CHECK(*(const int32_t*)tusd_value_data(value) == 3);
        CHECK(tusd_prim_metadata_keys(stage, prim, "customData", strings) == TUSD_STATUS_OK);
        CHECK(tusd_string_list_len(strings) == 2 && strcmp(tusd_string_list_get(strings, 1), "pipeline") == 0);
        CHECK(tusd_stage_metadata_keys(stage, "customLayerData", strings) == TUSD_STATUS_OK && tusd_string_list_len(strings) == 2);
        tusd_value_free(faces);
        tusd_value_free(corners);
        tusd_stage_free(stage);
    }

    tusd_string_list_free(strings);
    tusd_prim_list_free(prims);
    tusd_value_free(value);
    puts("ok");
    return 0;
}
