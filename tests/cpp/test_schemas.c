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

int main(int argc, char** argv) {
    char usd[4096], schema[8192], defs_path[8192], plugin[8192];
    const char* search[1];
    TusdSchemaLibrary* library = NULL;
    TusdShaderDefs* defs = NULL;
    TusdSchemaLibraryView info;
    TusdSchemaClassView cls;
    TusdSchemaPropertyView prop;
    TusdShaderNodeView node;
    TusdShaderPortView port;
    TusdValue* value = tusd_value_new();
    TusdStringList* keys = tusd_string_list_new();
    bool found = false;
    size_t index = 0, i;

    CHECK(argc > 2);
    snprintf(usd, sizeof usd, "%s/openusd-26.11.1", argv[1]);
    snprintf(schema, sizeof schema, "%s/usdGeom/schema.usda", usd);
    snprintf(defs_path, sizeof defs_path, "%s/usdShaders/shaderDefs.usda", usd);
    snprintf(plugin, sizeof plugin, "%s/c_usdGeom", argv[2]);
    search[0] = usd;

    CHECK(tusd_has_feature(TUSD_FEATURE_SCHEMAS));
    CHECK(tusd_schema_library_read(schema, search, 1, &library) == TUSD_STATUS_OK);
    CHECK(tusd_schema_library_info(library, &info) == TUSD_STATUS_OK);
    CHECK(strcmp(info.name, "usdGeom") == 0 && strcmp(info.prefix, "UsdGeom") == 0);
    CHECK(info.class_count > 0 && info.token_count > 0 && strstr(info.generated_schema, "class Mesh") != NULL);
    CHECK(tusd_dictionary_get(info.global_data, "libraryName", value, &found) == TUSD_STATUS_OK && found);
    CHECK(strcmp(tusd_value_string(value, 0), "usdGeom") == 0);

    CHECK(tusd_schema_library_find_class(library, "Mesh", &found, &index) == TUSD_STATUS_OK && found);
    CHECK(tusd_schema_library_class(library, index, &cls) == TUSD_STATUS_OK);
    CHECK(cls.kind == TUSD_SCHEMA_KIND_CONCRETE_TYPED && strcmp(cls.bases[0], "PointBased") == 0);
    found = false;
    for (i = 0; i < cls.property_count; ++i) {
        CHECK(tusd_schema_library_property(library, index, i, &prop) == TUSD_STATUS_OK);
        if (strcmp(prop.name, "subdivisionScheme") == 0) {
            found = true;
            CHECK(strcmp(prop.declared_by, "Mesh") == 0 && prop.uniform && !prop.relationship);
            CHECK(prop.allowed_token_count == 4 && strcmp(prop.allowed_tokens[0], "catmullClark") == 0);
            CHECK(prop.fallback != NULL && strcmp(tusd_value_string(prop.fallback, 0), "catmullClark") == 0);
        }
        if (strcmp(prop.name, "primvars:displayColor") == 0) {
            CHECK(strcmp(prop.declared_by, "Gprim") == 0 && prop.fallback == NULL);
            CHECK(tusd_dictionary_keys(prop.custom_data, "", keys) == TUSD_STATUS_OK);
            CHECK(tusd_string_list_len(keys) == 1 && strcmp(tusd_string_list_get(keys, 0), "apiName") == 0);
        }
    }
    CHECK(found);
    CHECK(tusd_schema_library_property(library, index, cls.property_count, &prop) == TUSD_STATUS_INVALID_ARGUMENT);
    CHECK(tusd_schema_library_find_class(library, "BackPlateAPI", &found, &index) == TUSD_STATUS_OK && found);
    CHECK(tusd_schema_library_class(library, index, &cls) == TUSD_STATUS_OK);
    CHECK(cls.kind == TUSD_SCHEMA_KIND_MULTIPLE_APPLY_API && strcmp(cls.property_namespace_prefix, "backPlate") == 0);
    CHECK(tusd_schema_library_write_plugin(library, plugin) == TUSD_STATUS_OK);
    tusd_schema_library_free(library);

    CHECK(tusd_shader_defs_read(defs_path, search, 1, &defs) == TUSD_STATUS_OK);
    CHECK(tusd_shader_defs_node_count(defs) > 0);
    CHECK(tusd_shader_defs_node(defs, 0, &node) == TUSD_STATUS_OK);
    CHECK(strcmp(node.id, "UsdPreviewSurface") == 0 && node.input_count > 0 && node.output_count > 0);
    CHECK(tusd_shader_defs_input(defs, 0, 0, &port) == TUSD_STATUS_OK && port.type_name[0] != '\0');
    CHECK(tusd_shader_defs_output(defs, 0, 0, &port) == TUSD_STATUS_OK);
    tusd_shader_defs_free(defs);
    defs = NULL;
    CHECK(tusd_shader_defs_read("missing.usda", NULL, 0, &defs) == TUSD_STATUS_SCHEMA_FAILED && defs == NULL);

    tusd_string_list_free(keys);
    tusd_value_free(value);
    puts("ok");
    return 0;
}
