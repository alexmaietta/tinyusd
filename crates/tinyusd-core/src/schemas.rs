use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use openusd::sdf;
use openusd::tf;
use openusd::usd::{FamilySource, SchemaRegistry};
use serde_json::Value as Json;

use crate::{Error, Result};

const CPP_PREFIXES: [&str; 12] = [
    "UsdGeom",
    "UsdShade",
    "UsdLux",
    "UsdSkel",
    "UsdVol",
    "UsdMedia",
    "UsdRender",
    "UsdUI",
    "UsdProc",
    "UsdHydra",
    "UsdRi",
    "Usd",
];

fn schema_error(path: &Path, message: impl std::fmt::Display) -> Error {
    Error::Schema {
        path: path.display().to_string(),
        message: message.to_string(),
    }
}

struct Family {
    name: String,
    manifest: String,
    schematics: PathBuf,
}

fn plug_info_path(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join("plugInfo.json")
    } else {
        path.to_path_buf()
    }
}

fn read_plug_info(path: &Path) -> Result<Json> {
    let text = std::fs::read_to_string(path).map_err(|e| schema_error(path, e))?;
    let json: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    serde_json::from_str(&json).map_err(|e| schema_error(path, e))
}

fn token_list(values: &[String]) -> String {
    let quoted: Vec<String> = values.iter().map(|v| format!("{v:?}")).collect();
    format!("[{}]", quoted.join(", "))
}

fn strings(value: Option<&Json>) -> Vec<String> {
    value
        .and_then(Json::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

fn base_identifier(cpp: &str, local: &HashMap<String, String>, known: &SchemaRegistry) -> String {
    if let Some(id) = local.get(cpp) {
        return id.clone();
    }
    for prefix in CPP_PREFIXES {
        if let Some(candidate) = cpp.strip_prefix(prefix)
            && !candidate.is_empty()
            && known.schema_info(&tf::Token::from(candidate)).is_some()
        {
            return candidate.to_owned();
        }
    }
    cpp.strip_prefix("Usd").unwrap_or(cpp).to_owned()
}

fn families_from(path: &Path, known: &SchemaRegistry) -> Result<Vec<Family>> {
    let plug_info = plug_info_path(path);
    let json = read_plug_info(&plug_info)?;
    let dir = plug_info.parent().unwrap_or(Path::new("."));
    let mut out = Vec::new();
    for plugin in json["Plugins"]
        .as_array()
        .ok_or_else(|| schema_error(&plug_info, "no \"Plugins\" list"))?
    {
        let types = plugin["Info"]["Types"].as_object();
        let Some(types) = types.filter(|t| !t.is_empty()) else {
            continue;
        };
        let field = |key: &str| match plugin[key].as_str() {
            Some(v) if !(v.starts_with('@') && v.ends_with('@')) => v.to_owned(),
            _ => ".".to_owned(),
        };
        let schematics = dir
            .join(field("Root"))
            .join(field("ResourcePath"))
            .join("generatedSchema.usda");
        let name = plugin["Name"].as_str().map(str::to_owned).unwrap_or_else(|| {
            dir.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "custom".to_owned())
        });
        let mut local = HashMap::new();
        for (cpp, info) in types {
            let id = info["schemaIdentifier"]
                .as_str()
                .or_else(|| info["alias"]["UsdSchemaBase"].as_str())
                .unwrap_or(cpp.as_str());
            local.insert(cpp.clone(), id.to_owned());
        }
        let mut manifest = String::from("#usda 1.0\n\n");
        for (cpp, info) in types {
            let Some(kind) = info["schemaKind"].as_str() else {
                continue;
            };
            let bases: Vec<String> = strings(info.get("bases"))
                .iter()
                .map(|b| base_identifier(b, &local, known))
                .collect();
            let _ = writeln!(manifest, "def {:?}\n{{", local[cpp]);
            let _ = writeln!(manifest, "    uniform token schemaKind = {kind:?}");
            let _ = writeln!(manifest, "    uniform token[] bases = {}", token_list(&bases));
            for (key, attr) in [
                ("apiSchemaCanOnlyApplyTo", "apiSchemaCanOnlyApplyTo"),
                ("apiSchemaAutoApplyTo", "apiSchemaAutoApplyTo"),
                ("apiSchemaAllowedInstanceNames", "allowedInstanceNames"),
            ] {
                let values = strings(info.get(key));
                if !values.is_empty() {
                    let _ = writeln!(manifest, "    uniform token[] {attr} = {}", token_list(&values));
                }
            }
            if let Some(prefix) = info["propertyNamespacePrefix"].as_str() {
                let _ = writeln!(manifest, "    uniform token propertyNamespacePrefix = {prefix:?}");
            }
            manifest.push_str("}\n\n");
        }
        out.push(Family {
            name,
            manifest,
            schematics,
        });
    }
    Ok(out)
}

fn build(paths: &[PathBuf]) -> Result<Arc<SchemaRegistry>> {
    let known = openusd_schemas::schema_registry();
    let mut builder = SchemaRegistry::builder();
    for family in openusd_schemas::ALL {
        builder = builder.register(family);
    }
    for path in paths {
        for family in families_from(path, &known)? {
            let manifest =
                sdf::Layer::from_bytes(format!("{}.manifest.usda", family.name), family.manifest.into_bytes())
                    .map_err(|e| schema_error(path, e))?;
            let schematics = sdf::Layer::open(family.schematics.to_string_lossy())
                .map_err(|e| schema_error(&family.schematics, e))?;
            builder = builder.family(FamilySource {
                name: &family.name,
                manifest: &manifest,
                schematics: &schematics,
            });
        }
    }
    builder
        .build()
        .map_err(|e| schema_error(paths.first().map_or(Path::new(""), PathBuf::as_path), e))
}

pub(crate) fn registry(paths: &[PathBuf]) -> Result<Arc<SchemaRegistry>> {
    if paths.is_empty() {
        return Ok(openusd_schemas::schema_registry());
    }
    static CACHE: OnceLock<Mutex<HashMap<Vec<PathBuf>, Arc<SchemaRegistry>>>> = OnceLock::new();
    let key: Vec<PathBuf> = paths
        .iter()
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
        .collect();
    let cache = CACHE.get_or_init(Mutex::default);
    if let Some(found) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return Ok(found);
    }
    let built = build(&key)?;
    if let Ok(mut c) = cache.lock() {
        c.insert(key, built.clone());
    }
    Ok(built)
}
