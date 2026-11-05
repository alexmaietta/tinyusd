use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::rc::Rc;

use tinyusd_core::{AssetResolver, Error, OpenOptions, PrimFlags, Stage, TraverseFlags, Value};

fn data(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/../../tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[derive(Default)]
struct MemoryResolver {
    files: HashMap<String, Vec<u8>>,
    reads: RefCell<Vec<String>>,
}

impl MemoryResolver {
    fn with_fixtures() -> Self {
        let mut files = HashMap::new();
        for name in ["query.usda", "part.usda", "textured.usdz"] {
            files.insert(format!("mem://assets/{name}"), data(name));
        }
        Self {
            files,
            reads: RefCell::default(),
        }
    }
}

impl AssetResolver for MemoryResolver {
    fn resolve(&self, asset_path: &str, anchor: Option<&str>) -> Option<String> {
        let candidate = if asset_path.starts_with("mem://") {
            asset_path.to_owned()
        } else {
            let dir = anchor?.rsplit_once('/')?.0;
            format!("{dir}/{}", asset_path.trim_start_matches("./"))
        };
        self.files.contains_key(&candidate).then_some(candidate)
    }

    fn read(&self, resolved_path: &str) -> io::Result<Vec<u8>> {
        self.reads.borrow_mut().push(resolved_path.to_owned());
        self.files
            .get(resolved_path)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, resolved_path))
    }
}

fn open(path: &str, resolver: Rc<MemoryResolver>, load_payloads: bool) -> Stage {
    Stage::open(
        path,
        &OpenOptions {
            load_payloads,
            resolver: Some(resolver),
            ..OpenOptions::default()
        },
    )
    .unwrap()
}

#[test]
fn composes_entirely_through_the_host() {
    let host = Rc::new(MemoryResolver::with_fixtures());
    let mut stage = open("mem://assets/query.usda", host.clone(), true);
    let prims = stage.traverse(TraverseFlags::DEFAULT).unwrap();
    assert_eq!(prims.len(), 10);
    let payloaded = stage.prim_at_path("/World/Payloaded").unwrap().unwrap();
    assert_eq!(stage.type_name(payloaded).unwrap(), "Mesh");
    let reads = host.reads.borrow();
    assert!(reads.contains(&"mem://assets/query.usda".to_owned()));
    assert!(reads.contains(&"mem://assets/part.usda".to_owned()));
}

#[test]
fn asset_values_resolve_through_the_host() {
    let host = Rc::new(MemoryResolver::with_fixtures());
    let mut stage = open("mem://assets/query.usda", host, true);
    let types = stage.prim_at_path("/World/Types").unwrap().unwrap();
    let mut v = Value::new();
    assert!(stage.get(types, "aa", None, &mut v).unwrap());
    assert_eq!(v.resolved_paths(), ["mem://assets/part.usda", ""]);
    assert_eq!(stage.read_asset("mem://assets/part.usda").unwrap(), data("part.usda"));
    assert!(matches!(
        stage.read_asset("mem://assets/missing.png"),
        Err(Error::Asset { .. })
    ));
}

#[test]
fn reads_packages_served_by_the_host() {
    let host = Rc::new(MemoryResolver::with_fixtures());
    let mut stage = open("mem://assets/textured.usdz", host.clone(), true);
    let texture = stage.prim_at_path("/Root/Texture").unwrap().unwrap();
    let mut v = Value::new();
    assert!(stage.get(texture, "inputs:file", None, &mut v).unwrap());
    let resolved = v.resolved_paths()[0].clone();
    assert_eq!(resolved, "mem://assets/textured.usdz[tex.png]");
    for _ in 0..3 {
        assert_eq!(stage.read_asset(&resolved).unwrap(), b"tinyusd-texture-bytes");
    }
    assert_eq!(*host.reads.borrow(), ["mem://assets/textured.usdz"]);
}

#[test]
fn missing_root_layer_fails_to_open() {
    let host = Rc::new(MemoryResolver::with_fixtures());
    let result = Stage::open(
        "mem://assets/nope.usda",
        &OpenOptions {
            load_payloads: true,
            resolver: Some(host),
            ..OpenOptions::default()
        },
    );
    assert!(matches!(result, Err(Error::Open { .. })));
}

#[test]
fn payloads_load_and_unload_after_open() {
    let path = format!("{}/../../tests/data/query.usda", env!("CARGO_MANIFEST_DIR"));
    let mut stage = Stage::open(
        &path,
        &OpenOptions {
            load_payloads: false,
            resolver: None,
            ..OpenOptions::default()
        },
    )
    .unwrap();
    let root = stage.pseudo_root();
    let payloaded = stage.prim_at_path("/World/Payloaded").unwrap().unwrap();
    assert_eq!(stage.loadable(root).unwrap(), [payloaded]);
    assert!(!stage.flags(payloaded).unwrap().contains(PrimFlags::LOADED));
    assert_eq!(stage.type_name(payloaded).unwrap(), "");

    stage.load(payloaded, true).unwrap();
    assert!(stage.flags(payloaded).unwrap().contains(PrimFlags::LOADED));
    assert_eq!(stage.type_name(payloaded).unwrap(), "Mesh");
    let mut v = Value::new();
    assert!(stage.get(payloaded, "points", None, &mut v).unwrap());
    assert_eq!(v.len(), 3);
    assert_eq!(stage.world_transform(payloaded, None).unwrap()[15], 1.0);

    stage.unload(payloaded).unwrap();
    assert!(!stage.flags(payloaded).unwrap().contains(PrimFlags::LOADED));
    assert!(!stage.get(payloaded, "points", None, &mut v).unwrap());
}
