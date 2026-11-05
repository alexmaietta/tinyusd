use std::io;
use std::rc::Rc;

use tinyusd_core::{AssetResolver, OpenOptions, Stage, TraverseFlags, Value};

struct OneFile {
    path: String,
    bytes: Vec<u8>,
}

impl AssetResolver for OneFile {
    fn resolve(&self, asset_path: &str, _anchor: Option<&str>) -> Option<String> {
        (asset_path == self.path).then(|| self.path.clone())
    }

    fn read(&self, resolved_path: &str) -> io::Result<Vec<u8>> {
        if resolved_path == self.path {
            Ok(self.bytes.clone())
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, resolved_path.to_owned()))
        }
    }
}

pub fn exercise(extension: &str, data: &[u8]) {
    let path = format!("mem://fuzz.{extension}");
    let resolver = Rc::new(OneFile {
        path: path.clone(),
        bytes: data.to_vec(),
    });
    let Ok(mut stage) = Stage::open(
        &path,
        &OpenOptions {
            load_payloads: true,
            resolver: Some(resolver),
        },
    ) else {
        return;
    };
    let flags = TraverseFlags::INCLUDE_INACTIVE
        | TraverseFlags::INCLUDE_UNLOADED
        | TraverseFlags::INCLUDE_UNDEFINED
        | TraverseFlags::INCLUDE_ABSTRACT
        | TraverseFlags::INSTANCE_PROXIES;
    let Ok(prims) = stage.traverse(flags) else {
        return;
    };
    let mut value = Value::new();
    for &prim in prims.iter().take(256) {
        let _ = stage.type_name(prim);
        let _ = stage.applied_schemas(prim);
        let _ = stage.flags(prim);
        if let Ok(names) = stage.attribute_names(prim, false) {
            for name in names.iter().take(64) {
                let _ = stage.get(prim, name, None, &mut value);
                let _ = stage.attribute_info(prim, name);
                if let Ok(times) = stage.time_samples(prim, name)
                    && let Some(&t) = times.first()
                {
                    let _ = stage.get(prim, name, Some(t + 0.5), &mut value);
                }
                let _ = stage.connections(prim, name);
            }
        }
        if let Ok(names) = stage.relationship_names(prim, false) {
            for name in names.iter().take(16) {
                let _ = stage.targets(prim, name, true);
            }
        }
        let _ = stage.world_transform(prim, None);
        let _ = stage.local_transform(prim, Some(1.0));
        if let Ok(names) = stage.primvar_names(prim) {
            for name in names.iter().take(16) {
                let _ = stage.primvar(prim, name, None, &mut value);
            }
        }
        let _ = stage.bound_material(prim, "");
    }
    let _ = stage.prototypes();
    let _ = stage.composition_errors();
}
