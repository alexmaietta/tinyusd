#![cfg(feature = "schemas")]

use std::collections::BTreeMap;
use std::path::PathBuf;

use openusd::sdf;
use openusd::vt;
use tinyusd_core::{BaseType, OpenOptions, SchemaKind, SchemaLibrary, Stage, Value, read_shader_defs};

const FAMILIES: [&str; 7] = [
    "usd",
    "usdGeom",
    "usdShade",
    "usdVol",
    "usdLux",
    "usdRender",
    "usdPhysics",
];

fn usd_dir() -> PathBuf {
    PathBuf::from(format!(
        "{}/../../tests/data/openusd-26.11.1",
        env!("CARGO_MANIFEST_DIR")
    ))
}

fn read(family: &str) -> SchemaLibrary {
    let dir = usd_dir();
    SchemaLibrary::read(dir.join(family).join("schema.usda"), &[dir]).unwrap()
}

fn names(items: &[String]) -> Vec<&str> {
    items.iter().map(String::as_str).collect()
}

#[test]
fn reads_every_library() {
    for family in FAMILIES {
        let library = read(family);
        assert_eq!(library.name, family);
        assert!(!library.classes.is_empty(), "{family}");
        assert!(library.layers.iter().any(|l| l.ends_with("schema.usda")), "{family}");
        let mut value = Value::new();
        assert!(library.global_data.get("libraryName", &mut value));
        assert_eq!(value.strings(), [family]);
    }
}

#[test]
fn classes_kinds_and_bases() {
    let geom = read("usdGeom");
    assert_eq!(geom.prefix, "UsdGeom");
    let mesh = geom.find_class("Mesh").unwrap();
    assert_eq!(mesh.kind, SchemaKind::ConcreteTyped);
    assert_eq!(mesh.cpp_name, "UsdGeomMesh");
    assert_eq!(
        names(&mesh.bases),
        ["PointBased", "Gprim", "Boundable", "Xformable", "Imageable", "Typed"]
    );
    assert_eq!(geom.find_class("Gprim").unwrap().kind, SchemaKind::AbstractTyped);
    assert_eq!(geom.find_class("PrimvarsAPI").unwrap().kind, SchemaKind::NonAppliedApi);

    let visibility = geom.find_class("VisibilityAPI").unwrap();
    assert_eq!(visibility.kind, SchemaKind::SingleApplyApi);
    assert_eq!(names(&visibility.can_only_apply_to), ["Imageable"]);

    let plate = geom.find_class("BackPlateAPI").unwrap();
    assert_eq!(plate.kind, SchemaKind::MultipleApplyApi);
    assert_eq!(plate.property_namespace_prefix.as_deref(), Some("backPlate"));
    assert_eq!(names(&plate.bases), ["APISchemaBase"]);
    let mut value = Value::new();
    assert!(plate.custom_data.get("apiSchemaType", &mut value));
    assert_eq!(value.strings(), ["multipleApply"]);

    let core = read("usd");
    assert_eq!(core.find_class("Typed").unwrap().kind, SchemaKind::AbstractBase);
    let collection = core.find_class("CollectionAPI").unwrap();
    assert_eq!(collection.kind, SchemaKind::MultipleApplyApi);
    assert_eq!(collection.property_namespace_prefix.as_deref(), Some("collection"));

    let vol = read("usdVol");
    let api = vol
        .classes
        .iter()
        .find(|c| c.can_only_apply_to == ["ParticleField"])
        .unwrap();
    assert!(api.kind.is_applied_api());

    let shade = read("usdShade");
    let material = shade.find_class("Material").unwrap();
    assert_eq!(names(&material.bases), ["NodeGraph", "Typed"]);
    assert!(material.custom_data.is_dictionary("extraPlugInfo"));
    assert!(
        material
            .custom_data
            .get("extraPlugInfo:providesUsdShadeConnectableAPIBehavior", &mut value)
    );
    assert_eq!(value.as_scalar::<u8>(), Some(1));
    assert_eq!(names(&shade.find_class("Shader").unwrap().api_schemas), ["NodeDefAPI"]);
}

#[test]
fn properties_declared_and_inherited() {
    let geom = read("usdGeom");
    let mesh = geom.find_class("Mesh").unwrap();

    let scheme = mesh.find_property("subdivisionScheme").unwrap();
    assert_eq!(scheme.declared_by, "Mesh");
    assert!(scheme.uniform && !scheme.relationship);
    assert_eq!(scheme.type_name, "token");
    assert_eq!(
        names(&scheme.allowed_tokens),
        ["catmullClark", "loop", "bilinear", "none"]
    );
    assert_eq!(scheme.fallback.as_ref().unwrap().strings(), ["catmullClark"]);

    let points = mesh.find_property("points").unwrap();
    assert_eq!(points.declared_by, "PointBased");
    assert_eq!(points.type_name, "point3f[]");
    assert!(!points.uniform);

    let sided = mesh.find_property("doubleSided").unwrap();
    assert_eq!(sided.declared_by, "Gprim");
    assert_eq!(sided.fallback.as_ref().unwrap().as_scalar::<u8>(), Some(0));

    let color = mesh.find_property("primvars:displayColor").unwrap();
    assert_eq!(color.declared_by, "Gprim");
    assert!(color.fallback.is_none());
    let mut value = Value::new();
    assert!(color.custom_data.get("apiName", &mut value));
    assert_eq!(value.strings(), ["displayColor"]);

    let visibility = mesh.find_property("visibility").unwrap();
    assert_eq!(visibility.declared_by, "Imageable");
    assert_eq!(names(&visibility.allowed_tokens), ["inherited", "invisible"]);
    assert!(mesh.find_property("proxyPrim").unwrap().relationship);

    let plate = geom.find_class("BackPlateAPI").unwrap();
    let translate = plate.find_property("translate:tweak").unwrap();
    let fallback = translate.fallback.as_ref().unwrap();
    assert_eq!(fallback.base_type(), BaseType::Float3);
    assert_eq!(fallback.as_scalar::<[f32; 3]>(), Some([0.0; 3]));
    let image = plate.find_property("image").unwrap();
    assert_eq!(image.type_name, "asset");
    assert_eq!(image.fallback.as_ref().unwrap().strings(), [""]);

    let lux = read("usdLux");
    let light = lux.find_class("LightAPI").unwrap();
    let intensity = light.find_property("inputs:intensity").unwrap();
    assert_eq!(intensity.display_group.as_deref(), Some("Basic"));
    assert_eq!(intensity.display_name.as_deref(), Some("Intensity"));
    assert_eq!(intensity.fallback.as_ref().unwrap().as_scalar::<f32>(), Some(1.0));
    assert!(
        intensity
            .documentation
            .as_deref()
            .unwrap()
            .starts_with("Scales the brightness")
    );
    let dome = lux.find_class("DomeLight").unwrap();
    assert!(dome.api_schemas.iter().any(|s| s == "LightAPI"));
    assert!(dome.find_property("inputs:intensity").is_none());
    assert_eq!(
        dome.find_property("inputs:texture:file").unwrap().declared_by,
        "DomeLight"
    );
    assert_eq!(dome.find_property("xformOpOrder").unwrap().declared_by, "Xformable");
}

#[test]
fn library_tokens() {
    let geom = read("usdGeom");
    let interpolation = geom.tokens.iter().find(|t| t.name == "interpolation").unwrap();
    assert_eq!(interpolation.value, "interpolation");
    assert!(interpolation.documentation.as_deref().unwrap().contains("Primvar"));
    assert_eq!(geom.global_data.keys("libraryTokens").len(), geom.tokens.len());
    assert!(!geom.skip_code_generation);
}

#[test]
fn reads_shader_definitions() {
    let dir = usd_dir();
    let nodes = read_shader_defs(dir.join("usdShaders/shaderDefs.usda"), &[dir]).unwrap();
    assert!(nodes.len() >= 13);
    let surface = nodes.iter().find(|n| n.id == "UsdPreviewSurface").unwrap();
    assert_eq!(surface.name, "UsdPreviewSurface");
    let roughness = surface.inputs.iter().find(|p| p.name == "roughness").unwrap();
    assert_eq!(roughness.type_name, "float");
    assert_eq!(roughness.fallback.as_ref().unwrap().as_scalar::<f32>(), Some(0.5));
    assert!(surface.outputs.iter().any(|p| p.name == "surface"));

    let texture = nodes.iter().find(|n| n.id == "UsdUVTexture").unwrap();
    let mut value = Value::new();
    assert!(texture.sdr_metadata.get("role", &mut value));
    assert_eq!(value.strings(), ["texture"]);
    let wrap = texture.inputs.iter().find(|p| p.name == "wrapS").unwrap();
    assert!(wrap.allowed_tokens.iter().any(|t| t == "repeat"));
    let file = texture.inputs.iter().find(|p| p.name == "file").unwrap();
    assert_eq!(file.type_name, "asset");
    for node in &nodes {
        assert!(!node.id.is_empty());
        for port in node.inputs.iter().chain(&node.outputs) {
            assert!(!port.type_name.is_empty(), "{}.{}", node.name, port.name);
        }
    }
}

type Specs = BTreeMap<String, BTreeMap<String, vt::Value>>;

fn specs(layer: &sdf::Layer) -> Specs {
    const APART: [&str; 3] = [
        sdf::FieldKey::Comment.as_str(),
        sdf::ChildrenKey::PrimChildren.as_str(),
        sdf::ChildrenKey::PropertyChildren.as_str(),
    ];
    let data = layer.data();
    data.spec_paths()
        .into_iter()
        .map(|path| {
            let fields = data
                .list_fields(&path)
                .unwrap_or_default()
                .into_iter()
                .filter(|f| !APART.contains(&f.as_str()))
                .filter_map(|f| Some((f.clone(), data.try_field(&path, &f).ok().flatten()?.into_owned())))
                .collect();
            (path.to_string(), fields)
        })
        .collect()
}

#[test]
fn written_plugins_match_usd() {
    let out = tempfile::tempdir().unwrap();
    for family in FAMILIES {
        let library = read(family);
        let dir = out.path().join(family);
        library.write_plugin(&dir).unwrap();

        let ours = specs(&sdf::Layer::open(dir.join("generatedSchema.usda").to_string_lossy()).unwrap());
        let theirs =
            specs(&sdf::Layer::open(usd_dir().join(family).join("generatedSchema.usda").to_string_lossy()).unwrap());
        assert_eq!(
            ours.keys().collect::<Vec<_>>(),
            theirs.keys().collect::<Vec<_>>(),
            "{family} specs"
        );
        for (path, fields) in &theirs {
            assert_eq!(&ours[path], fields, "{family} {path}");
        }

        let read_json = |path: PathBuf| -> serde_json::Value {
            let text = std::fs::read_to_string(path).unwrap();
            let body: String = text
                .lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n");
            serde_json::from_str(&body).unwrap()
        };
        let ours = read_json(dir.join("plugInfo.json"));
        let theirs = read_json(usd_dir().join(family).join("plugInfo.json"));
        assert_eq!(ours["Plugins"][0]["Name"], theirs["Plugins"][0]["Name"]);
        let generated = |json: &serde_json::Value| -> serde_json::Map<String, serde_json::Value> {
            json["Plugins"][0]["Info"]["Types"]
                .as_object()
                .unwrap()
                .iter()
                .filter(|(_, entry)| entry["autoGenerated"] == true)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        };
        assert_eq!(generated(&ours), generated(&theirs), "{family} plugInfo types");
    }
}

#[test]
fn schema_paths_replace_builtin_families() {
    let scene = PathBuf::from(format!(
        "{}/../../tests/data/backplate.usda",
        env!("CARGO_MANIFEST_DIR")
    ));
    let tweak = |schema_paths: Vec<PathBuf>| {
        let options = OpenOptions {
            schema_paths,
            ..OpenOptions::default()
        };
        let mut stage = Stage::open(scene.to_str().unwrap(), &options).unwrap();
        let camera = stage.prim_at_path("/Camera").unwrap().unwrap();
        assert!(stage.is_a(camera, "Camera").unwrap());
        let mut value = Value::new();
        stage
            .get(camera, "backPlate:plate:scale:tweak", None, &mut value)
            .unwrap()
            .then(|| value.as_scalar::<[f32; 2]>().unwrap())
    };
    assert_eq!(tweak(Vec::new()), None);
    assert_eq!(tweak(vec![usd_dir().join("usdGeom")]), Some([1.0, 1.0]));
    let all = FAMILIES.iter().map(|f| usd_dir().join(f)).collect();
    assert_eq!(tweak(all), Some([1.0, 1.0]));
}
