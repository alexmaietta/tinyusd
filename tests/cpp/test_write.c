#include <tinyusd/tinyusd.h>

#include <stddef.h>
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

static char stored[1 << 16];
static size_t stored_size = 0;
static int writes = 0;

static bool mem_resolve(void* user_data, const char* asset_path, const char* anchor, TusdOutput* out) {
    (void)user_data;
    (void)anchor;
    if (strcmp(asset_path, "mem://scene.usda") != 0 || stored_size == 0) return false;
    tusd_output_set_string(out, asset_path);
    return true;
}

static bool mem_read(void* user_data, const char* resolved_path, TusdOutput* out) {
    (void)user_data;
    (void)resolved_path;
    tusd_output_set_bytes(out, (const uint8_t*)stored, stored_size);
    return true;
}

static bool mem_write(void* user_data, const char* resolved_path, const uint8_t* data, size_t size, TusdOutput* out) {
    (void)user_data;
    (void)resolved_path;
    if (size > sizeof(stored)) {
        tusd_output_set_error(out, "too big");
        return false;
    }
    memcpy(stored, data, size);
    stored_size = size;
    writes++;
    return true;
}

int main(int argc, char** argv) {
    static const float points[4][3] = {{0, 0, 0}, {1, 0, 0}, {1, 1, 0}, {0, 1, 0}};
    static const int32_t counts[1] = {4};
    static const double doubles[4][3] = {{0}};
    const char* scheme = "none";
    const char* body0 = "/World/Mesh";
    TusdOpenOptions options;
    TusdResolverCallbacks callbacks;
    TusdStage* stage = NULL;
    TusdStage* reopened = NULL;
    TusdPrim mesh = 0, joint = 0, found_prim = 0;
    TusdValue* value = tusd_value_new();
    TusdStringList* targets = tusd_string_list_new();
    bool has = false, found = false;
    double frame;
    const char* text;
    (void)argc;
    (void)argv;

    CHECK(tusd_has_feature(TUSD_FEATURE_WRITE));
    CHECK(tusd_stage_create_in_memory(NULL, &stage) == TUSD_STATUS_OK);
    CHECK(tusd_stage_define_prim(stage, "/World/Mesh", "Mesh", &mesh) == TUSD_STATUS_OK);
    CHECK(tusd_stage_set_up_axis(stage, "Z") == TUSD_STATUS_OK);

    CHECK(tusd_value_set_data(value, TUSD_VALUE_TYPE_FLOAT3, true, points, 4) == TUSD_STATUS_OK);
    CHECK(tusd_attribute_set(stage, mesh, "points", NULL, value) == TUSD_STATUS_OK);
    CHECK(tusd_value_set_data(value, TUSD_VALUE_TYPE_INT, true, counts, 1) == TUSD_STATUS_OK);
    CHECK(tusd_attribute_set(stage, mesh, "faceVertexCounts", NULL, value) == TUSD_STATUS_OK);
    CHECK(tusd_value_set_strings(value, TUSD_VALUE_TYPE_TOKEN, false, &scheme, 1) == TUSD_STATUS_OK);
    CHECK(tusd_attribute_set(stage, mesh, "subdivisionScheme", NULL, value) == TUSD_STATUS_OK);

    CHECK(tusd_value_set_data(value, TUSD_VALUE_TYPE_DOUBLE3, true, doubles, 4) == TUSD_STATUS_OK);
    CHECK(tusd_attribute_set(stage, mesh, "points", NULL, value) == TUSD_STATUS_INVALID_VALUE);
    CHECK(tusd_value_set_data(value, 99, true, doubles, 4) == TUSD_STATUS_INVALID_ARGUMENT);
    CHECK(tusd_value_set_data(value, TUSD_VALUE_TYPE_FLOAT, false, points, 2) == TUSD_STATUS_INVALID_VALUE);

    CHECK(tusd_attribute_create(stage, mesh, "my:temp", "float", false) == TUSD_STATUS_OK);
    CHECK(tusd_stage_begin_edit(stage) == TUSD_STATUS_OK);
    for (frame = 0; frame < 10; frame += 1) {
        float f = (float)frame;
        CHECK(tusd_value_set_data(value, TUSD_VALUE_TYPE_FLOAT, false, &f, 1) == TUSD_STATUS_OK);
        CHECK(tusd_attribute_set(stage, mesh, "my:temp", &frame, value) == TUSD_STATUS_OK);
    }
    CHECK(tusd_prim_apply_api(stage, mesh, "PhysicsCollisionAPI") == TUSD_STATUS_OK);
    CHECK(tusd_stage_save(stage) == TUSD_STATUS_USD_ERROR);
    CHECK(tusd_stage_commit_edit(stage) == TUSD_STATUS_OK);
    frame = 7;
    CHECK(tusd_attribute_get(stage, mesh, "my:temp", &frame, value, &has) == TUSD_STATUS_OK && has);
    CHECK(*(const float*)tusd_value_data(value) == 7.0f);

    CHECK(tusd_prim_apply_api(stage, mesh, "PhysicsCollisionAPI") == TUSD_STATUS_OK);
    CHECK(tusd_stage_define_prim(stage, "/World/Joint", "PhysicsFixedJoint", &joint) == TUSD_STATUS_OK);
    CHECK(tusd_relationship_set_targets(stage, joint, "physics:body0", &body0, 1) == TUSD_STATUS_OK);
    CHECK(tusd_relationship_targets(stage, joint, "physics:body0", false, targets) == TUSD_STATUS_OK);
    CHECK(tusd_string_list_len(targets) == 1 && strcmp(tusd_string_list_get(targets, 0), body0) == 0);

    {
        TusdPrim material = 0, texture = 0;
        bool removed = false;
        float rough = 0.5f;
        CHECK(tusd_value_set_data(value, TUSD_VALUE_TYPE_BOOL, false, &(bool){true}, 1) == TUSD_STATUS_OK);
        CHECK(tusd_prim_set_metadata(stage, mesh, "hidden", value) == TUSD_STATUS_OK);
        CHECK(tusd_prim_set_metadata(stage, mesh, "customData:flag", value) == TUSD_STATUS_OK);
        CHECK(tusd_prim_set_metadata(stage, mesh, "displayName", value) == TUSD_STATUS_INVALID_VALUE);
        CHECK(tusd_prim_clear_metadata(stage, mesh, "customData:flag") == TUSD_STATUS_OK);
        CHECK(tusd_prim_metadata(stage, mesh, "customData:flag", value, &has) == TUSD_STATUS_OK && !has);
        CHECK(tusd_value_set_data(value, TUSD_VALUE_TYPE_BOOL, false, &(bool){true}, 1) == TUSD_STATUS_OK);
        CHECK(tusd_property_set_metadata(stage, mesh, "points", "hidden", value) == TUSD_STATUS_OK);
        CHECK(tusd_property_clear_metadata(stage, mesh, "points", "hidden") == TUSD_STATUS_OK);
        CHECK(tusd_prim_remove_property(stage, mesh, "my:temp", &removed) == TUSD_STATUS_OK && removed);
        CHECK(tusd_relationship_remove_target(stage, joint, "physics:body0", body0) == TUSD_STATUS_OK);
        CHECK(tusd_relationship_targets(stage, joint, "physics:body0", false, targets) == TUSD_STATUS_OK);
        CHECK(tusd_string_list_len(targets) == 0);
        CHECK(tusd_prim_add_reference_with_offset(stage, joint, "./other.usda", NULL, 10.0, 0.0) ==
              TUSD_STATUS_INVALID_VALUE);
        CHECK(tusd_stage_define_preview_material(stage, "/World/Looks/Mat", &material) == TUSD_STATUS_OK);
        CHECK(tusd_value_set_data(value, TUSD_VALUE_TYPE_FLOAT, false, &rough, 1) == TUSD_STATUS_OK);
        CHECK(tusd_material_set_preview_input(stage, material, "roughness", value) == TUSD_STATUS_OK);
        CHECK(tusd_material_set_preview_texture(stage, material, "roughness", "./rough.png", &texture) ==
              TUSD_STATUS_OK);
        CHECK(tusd_material_set_preview_texture(stage, mesh, "roughness", "./rough.png", NULL) ==
              TUSD_STATUS_INVALID_VALUE);
        CHECK(tusd_prim_remove_variant(stage, mesh, "look", "red", &removed) == TUSD_STATUS_OK && !removed);
        {
            TusdPrim field = 0, skel = 0, proto = mesh;
            int32_t indices[2] = {0, 0};
            double transforms[32] = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
                                     1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 4, 5, 6, 1};
            const char* joints[2] = {"root", "root/tip"};
            TusdInstancesView view;
            memset(&view, 0, sizeof(view));
            view.prototypes = &proto;
            view.prototype_count = 1;
            view.proto_indices = indices;
            view.transforms = transforms;
            view.count = 2;
            CHECK(tusd_stage_define_prim(stage, "/World/Field", NULL, &field) == TUSD_STATUS_OK);
            CHECK(tusd_prim_set_instances(stage, field, &view, NULL) == TUSD_STATUS_OK);
            indices[1] = 3;
            CHECK(tusd_prim_set_instances(stage, field, &view, NULL) == TUSD_STATUS_INVALID_VALUE);
            CHECK(tusd_stage_define_skeleton(stage, "/World/Skel", joints, 2, transforms, NULL, &skel) == TUSD_STATUS_OK);
            CHECK(tusd_prim_bind_skeleton(stage, mesh, skel, TUSD_NO_PRIM) == TUSD_STATUS_OK);
            {
                TusdPrim shape = 0;
                float offsets[3] = {0, 0, 1};
                float weight = 1.0f;
                int32_t point = 0;
                const char* name = "lift";
                CHECK(tusd_stage_define_blend_shape(stage, "/World/Mesh/Lift", offsets, 1, NULL, &point, &shape) ==
                      TUSD_STATUS_OK);
                CHECK(tusd_prim_set_blend_shape_inbetween(stage, shape, "half", 0.5f, offsets, 1, NULL) ==
                      TUSD_STATUS_OK);
                CHECK(tusd_prim_set_blend_shape_inbetween(stage, shape, "full", 1.0f, offsets, 1, NULL) ==
                      TUSD_STATUS_INVALID_VALUE);
                CHECK(tusd_prim_set_blend_shapes(stage, mesh, &name, &shape, 1) == TUSD_STATUS_OK);
                CHECK(tusd_prim_set_blend_shape_weights(stage, mesh, &name, &weight, 1, NULL) ==
                      TUSD_STATUS_INVALID_VALUE);
            }
        }
    }
    CHECK(tusd_stage_export_bytes(stage, NULL, TUSD_LAYER_FORMAT_USDA, value) == TUSD_STATUS_OK);
    CHECK(tusd_value_type(value) == TUSD_VALUE_TYPE_UCHAR);
    CHECK(strstr((const char*)tusd_value_data(value), "def Mesh \"Mesh\"") != NULL);
    CHECK(tusd_stage_save(stage) == TUSD_STATUS_OK);
    text = tusd_stage_edit_target(stage);
    CHECK(text != NULL && text[0] != '\0');
    tusd_stage_free(stage);

    memset(&callbacks, 0, sizeof(callbacks));
    callbacks.struct_size = (uint32_t)offsetof(TusdResolverCallbacks, write);
    callbacks.resolve = mem_resolve;
    callbacks.read = mem_read;
    callbacks.write = mem_write;
    tusd_open_options_init(&options);
    options.resolver = &callbacks;
    CHECK(tusd_stage_create_new("mem://scene.usda", &options, &stage) == TUSD_STATUS_WRITE_FAILED);
    CHECK(writes == 0);

    callbacks.struct_size = sizeof(callbacks);
    CHECK(tusd_stage_create_new("mem://scene.usda", &options, &stage) == TUSD_STATUS_OK);
    CHECK(tusd_stage_define_prim(stage, "/Hello", "Sphere", &mesh) == TUSD_STATUS_OK);
    CHECK(tusd_stage_save(stage) == TUSD_STATUS_OK);
    CHECK(writes == 2);
    tusd_stage_free(stage);
    CHECK(tusd_stage_open("mem://scene.usda", &options, &reopened) == TUSD_STATUS_OK);
    CHECK(tusd_stage_prim_at_path(reopened, "/Hello", &found_prim, &found) == TUSD_STATUS_OK && found);
    tusd_stage_free(reopened);

    tusd_string_list_free(targets);
    tusd_value_free(value);
    puts("ok");
    return 0;
}
