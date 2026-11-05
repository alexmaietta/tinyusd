use std::path::PathBuf;

use tinyusd_core::{BaseType, Error, OpenOptions, Stage, Value};

fn data(name: &str) -> PathBuf {
    PathBuf::from(format!("{}/../../tests/data/{name}", env!("CARGO_MANIFEST_DIR")))
}

fn open(schema_paths: Vec<PathBuf>) -> Stage {
    let options = OpenOptions {
        schema_paths,
        ..OpenOptions::default()
    };
    Stage::open(data("custom.usda").to_str().unwrap(), &options).unwrap()
}

fn get(stage: &mut Stage, path: &str, name: &str) -> Option<Value> {
    let p = stage.prim_at_path(path).unwrap().unwrap();
    let mut v = Value::new();
    stage.get(p, name, None, &mut v).unwrap().then_some(v)
}

#[test]
fn registered_schemas_supply_types_and_fallbacks() {
    let mut stage = open(vec![data("schemas/demo")]);
    let widget = stage.prim_at_path("/World/Widget").unwrap().unwrap();
    let defaults = stage.prim_at_path("/World/Defaults").unwrap().unwrap();
    assert!(stage.is_a(widget, "DemoWidget").unwrap());
    assert!(stage.is_a(widget, "Xformable").unwrap());
    assert!(stage.has_api(widget, "DemoTagAPI").unwrap());
    assert!(stage.has_api(widget, "DemoSlotAPI:left").unwrap());
    assert!(!stage.has_api(defaults, "DemoTagAPI").unwrap());

    assert_eq!(
        get(&mut stage, "/World/Widget", "demo:size")
            .unwrap()
            .as_scalar::<f32>(),
        Some(4.0)
    );
    assert_eq!(
        get(&mut stage, "/World/Defaults", "demo:size")
            .unwrap()
            .as_scalar::<f32>(),
        Some(2.5)
    );
    let mode = get(&mut stage, "/World/Defaults", "demo:mode").unwrap();
    assert_eq!(
        (mode.base_type(), mode.strings()),
        (BaseType::Token, &["fast".to_owned()][..])
    );
    assert_eq!(
        get(&mut stage, "/World/Widget", "demo:tag").unwrap().as_scalar::<i32>(),
        Some(7)
    );
    assert!(get(&mut stage, "/World/Defaults", "demo:tag").is_none());
    assert_eq!(
        get(&mut stage, "/World/Widget", "slot:left:weight")
            .unwrap()
            .as_scalar::<f32>(),
        Some(0.5)
    );
    assert_eq!(
        get(&mut stage, "/World/Defaults", "slot:right:weight")
            .unwrap()
            .as_scalar::<f32>(),
        Some(1.0)
    );

    let info = stage.attribute_info(defaults, "demo:size").unwrap().unwrap();
    assert!(info.has_value && !info.has_authored_value);
    let mut names = stage.attribute_names(defaults, false).unwrap();
    names.sort();
    assert_eq!(
        names,
        [
            "demo:mode",
            "demo:size",
            "purpose",
            "slot:right:weight",
            "visibility",
            "xformOpOrder"
        ]
    );

    let again = open(vec![data("schemas/demo/plugInfo.json")]);
    drop(again);
}

#[test]
fn unregistered_schemas_have_no_types_or_fallbacks() {
    let mut stage = open(Vec::new());
    let widget = stage.prim_at_path("/World/Widget").unwrap().unwrap();
    assert_eq!(stage.type_name(widget).unwrap(), "DemoWidget");
    assert!(!stage.is_a(widget, "Xformable").unwrap());
    assert!(get(&mut stage, "/World/Defaults", "demo:size").is_none());
    assert_eq!(
        get(&mut stage, "/World/Widget", "demo:size")
            .unwrap()
            .as_scalar::<f32>(),
        Some(4.0)
    );
}

#[test]
fn missing_schema_files_fail_to_open() {
    let options = OpenOptions {
        schema_paths: vec![data("schemas/nope")],
        ..OpenOptions::default()
    };
    let result = Stage::open(data("custom.usda").to_str().unwrap(), &options);
    assert!(matches!(result, Err(Error::Schema { .. })), "{:?}", result.err());
}
