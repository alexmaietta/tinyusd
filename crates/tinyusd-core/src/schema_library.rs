use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use openusd::sdf::{self, AbstractData};
use openusd::usd::Object;
use openusd::{ar, tf, usd, vt};
use openusd_build::model;
use serde_json::{Map, Value as Json};

use crate::{Error, Result, Value};

const MAX_BRIEF: usize = 500;
const USER_DOC_BRIEF: &str = "userDocBrief";

fn schema_error(path: &Path, message: impl std::fmt::Display) -> Error {
    Error::Schema {
        path: path.display().to_string(),
        message: message.to_string(),
    }
}

#[derive(Debug, Clone, Default)]
pub struct Dictionary(vt::Dictionary);

impl Dictionary {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn at(&self, path: &str) -> Option<&vt::Dictionary> {
        if path.is_empty() {
            return Some(&self.0);
        }
        self.0.get_at(path)?.try_as_dictionary_ref()
    }

    pub fn keys(&self, path: &str) -> Vec<String> {
        let mut keys: Vec<String> = self.at(path).map(|d| d.keys().cloned().collect()).unwrap_or_default();
        keys.sort();
        keys
    }

    pub fn is_dictionary(&self, path: &str) -> bool {
        self.at(path).is_some()
    }

    pub fn get(&self, path: &str, out: &mut Value) -> bool {
        let found = self.0.get_at(path).cloned();
        let type_name = match &found {
            Some(v) if v.try_as_dictionary_ref().is_some() => "dictionary",
            _ => "",
        };
        out.set_type_name(type_name);
        out.assign(found.unwrap_or(vt::Value::None))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SchemaKind {
    AbstractBase = 0,
    AbstractTyped = 1,
    ConcreteTyped = 2,
    NonAppliedApi = 3,
    SingleApplyApi = 4,
    MultipleApplyApi = 5,
}

impl SchemaKind {
    fn from_usd(kind: usd::SchemaKind) -> Self {
        match kind {
            usd::SchemaKind::AbstractTyped => Self::AbstractTyped,
            usd::SchemaKind::ConcreteTyped => Self::ConcreteTyped,
            usd::SchemaKind::NonAppliedApi => Self::NonAppliedApi,
            usd::SchemaKind::SingleApplyApi => Self::SingleApplyApi,
            usd::SchemaKind::MultipleApplyApi => Self::MultipleApplyApi,
            _ => Self::AbstractBase,
        }
    }

    pub const fn token(self) -> &'static str {
        match self {
            Self::AbstractBase => "abstractBase",
            Self::AbstractTyped => "abstractTyped",
            Self::ConcreteTyped => "concreteTyped",
            Self::NonAppliedApi => "nonAppliedAPI",
            Self::SingleApplyApi => "singleApplyAPI",
            Self::MultipleApplyApi => "multipleApplyAPI",
        }
    }

    pub const fn is_applied_api(self) -> bool {
        matches!(self, Self::SingleApplyApi | Self::MultipleApplyApi)
    }
}

#[derive(Debug, Clone)]
pub struct SchemaProperty {
    pub name: String,
    pub declared_by: String,
    pub relationship: bool,
    pub type_name: String,
    pub uniform: bool,
    pub fallback: Option<Value>,
    pub allowed_tokens: Vec<String>,
    pub display_group: Option<String>,
    pub display_name: Option<String>,
    pub documentation: Option<String>,
    pub custom_data: Dictionary,
}

#[derive(Debug, Clone)]
pub struct SchemaClass {
    pub name: String,
    pub cpp_name: String,
    pub kind: SchemaKind,
    pub bases: Vec<String>,
    pub can_only_apply_to: Vec<String>,
    pub auto_apply_to: Vec<String>,
    pub property_namespace_prefix: Option<String>,
    pub allowed_instance_names: Vec<String>,
    pub api_schemas: Vec<String>,
    pub documentation: Option<String>,
    pub custom_data: Dictionary,
    pub properties: Vec<SchemaProperty>,
}

impl SchemaClass {
    pub fn find_property(&self, name: &str) -> Option<&SchemaProperty> {
        self.properties.iter().find(|p| p.name == name)
    }
}

#[derive(Debug, Clone)]
pub struct LibraryToken {
    pub name: String,
    pub value: String,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SchemaLibrary {
    pub name: String,
    pub prefix: String,
    pub skip_code_generation: bool,
    pub global_data: Dictionary,
    pub tokens: Vec<LibraryToken>,
    pub classes: Vec<SchemaClass>,
    pub layers: Vec<String>,
    generated_schema: String,
    plug_info: String,
}

fn strings(tokens: &[tf::Token]) -> Vec<String> {
    tokens.iter().map(|t| t.as_str().to_owned()).collect()
}

fn text_field(fields: &BTreeMap<String, vt::Value>, key: &str) -> Option<String> {
    fields.get(key).and_then(vt::Value::as_str).map(str::to_owned)
}

fn library_prefix(library: &model::Library) -> String {
    if let Some(prefix) = library.global.get("libraryPrefix").and_then(vt::Value::as_str) {
        return prefix.to_owned();
    }
    let mut chars = library.name.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn property(class: &model::Class, property: &model::Property) -> SchemaProperty {
    let fallback = property.fallback().map(|v| {
        let mut out = Value::new();
        out.set_type_name(property.shape.type_name.as_ref().map_or("", tf::Token::as_str));
        out.assign(v.clone());
        out
    });
    let site = property.sites.first();
    SchemaProperty {
        name: property.name.as_str().to_owned(),
        declared_by: site.map_or_else(|| class.identifier.as_str().to_owned(), |s| s.class.as_str().to_owned()),
        relationship: property.shape.spec_type == sdf::SpecType::Relationship,
        type_name: property
            .shape
            .type_name
            .as_ref()
            .map_or_else(String::new, |t| t.as_str().to_owned()),
        uniform: property.shape.variability == sdf::Variability::Uniform,
        fallback,
        allowed_tokens: strings(&property.allowed_tokens()),
        display_group: text_field(&property.fields, sdf::FieldKey::DisplayGroup.as_str()),
        display_name: text_field(&property.fields, sdf::FieldKey::DisplayName.as_str()),
        documentation: property.documentation().map(str::to_owned),
        custom_data: Dictionary(site.map(|s| s.custom_data.clone()).unwrap_or_default()),
    }
}

fn class(class: &model::Class) -> SchemaClass {
    SchemaClass {
        name: class.identifier.as_str().to_owned(),
        cpp_name: class.upstream_name.clone(),
        kind: SchemaKind::from_usd(class.kind),
        bases: class.bases.iter().map(|b| b.identifier.as_str().to_owned()).collect(),
        can_only_apply_to: strings(&class.metadata.can_only_apply_to),
        auto_apply_to: strings(&class.metadata.auto_apply_to),
        property_namespace_prefix: class
            .metadata
            .property_namespace_prefix
            .as_ref()
            .map(|p| p.as_str().to_owned()),
        allowed_instance_names: strings(&class.metadata.allowed_instance_names),
        api_schemas: strings(&class.applied_api_schemas),
        documentation: class.documentation.clone(),
        custom_data: Dictionary(class.custom_data.clone()),
        properties: class.properties.iter().map(|p| property(class, p)).collect(),
    }
}

impl SchemaLibrary {
    pub fn read(schema: impl AsRef<Path>, search_paths: &[PathBuf]) -> Result<Self> {
        let schema = schema.as_ref();
        let builder = search_paths
            .iter()
            .fold(openusd_build::configure(), |b, p| b.search_path(p));
        let output = builder
            .build_library(schema, openusd_build::Views::Skip)
            .map_err(|e| schema_error(schema, e))?;
        let library = output.library();
        let generated_schema = generated_schema(&output, schema, search_paths).map_err(|e| schema_error(schema, e))?;
        Ok(Self {
            name: library.name.clone(),
            prefix: library_prefix(library),
            skip_code_generation: library.skip_code_generation,
            global_data: Dictionary(library.global.clone()),
            tokens: library
                .declared_tokens
                .iter()
                .map(|t| LibraryToken {
                    name: t.id.clone(),
                    value: t.value.clone(),
                    documentation: t.documentation.clone(),
                })
                .collect(),
            classes: library.classes.iter().map(class).collect(),
            layers: library
                .source_layers
                .iter()
                .map(|p| crate::value::simplify_path(&p.to_string_lossy()))
                .collect(),
            plug_info: plug_info(library),
            generated_schema,
        })
    }

    /// Reads a published schema plugin, the `plugInfo.json` and `generatedSchema.usda` pair that
    /// OpenUSD loads at run time, into the same shape [`read`](Self::read) gives a `schema.usda`.
    ///
    /// Kinds, bases, apply targets, namespace prefixes and allowed instance names come from
    /// `plugInfo.json` through the schema registry, and classes and properties come from the
    /// class specs in `generatedSchema.usda`, in their order there. Multiple-apply properties
    /// lose their `prefix:__INSTANCE_NAME__:` template the way `schema.usda` declares them.
    /// Typed classes in a generated schema carry every inherited property, so a property is
    /// declared by the nearest base in the plugin, or concrete built-in base, that defines it the
    /// same way, and by the class itself otherwise.
    pub fn read_plugin(plugin: impl AsRef<Path>) -> Result<Self> {
        let plugin = plugin.as_ref();
        let info_path = crate::schemas::plug_info_path(plugin);
        let json = crate::schemas::read_plug_info(&info_path)?;
        let dir = info_path.parent().unwrap_or(Path::new("."));
        let entry = json["Plugins"]
            .as_array()
            .and_then(|plugins| {
                plugins
                    .iter()
                    .find(|p| p["Info"]["Types"].as_object().is_some_and(|t| !t.is_empty()))
            })
            .ok_or_else(|| schema_error(&info_path, "no plugin declares schema types"))?;
        let field = |key: &str| match entry[key].as_str() {
            Some(v) if !(v.starts_with('@') && v.ends_with('@')) => v.to_owned(),
            _ => ".".to_owned(),
        };
        let schematics = dir
            .join(field("Root"))
            .join(field("ResourcePath"))
            .join("generatedSchema.usda");
        let name = entry["Name"].as_str().map(str::to_owned).unwrap_or_else(|| {
            dir.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "custom".to_owned())
        });
        let mut cpp_names = BTreeMap::new();
        for (cpp, info) in entry["Info"]["Types"].as_object().into_iter().flatten() {
            let id = info["schemaIdentifier"]
                .as_str()
                .or_else(|| info["alias"]["UsdSchemaBase"].as_str())
                .unwrap_or(cpp.as_str());
            cpp_names.insert(id.to_owned(), cpp.clone());
        }
        let registry = crate::schemas::registry(&[plugin.to_path_buf()])?;
        let layer = sdf::Layer::open(schematics.to_string_lossy()).map_err(|e| schema_error(&schematics, e))?;
        let root = layer.pseudo_root().and_then(|r| r.prim_children()).unwrap_or_default();
        let mut classes: Vec<SchemaClass> = Vec::new();
        for identifier in root.iter().filter(|id| cpp_names.contains_key(id.as_str())) {
            let info = registry
                .schema_info(identifier)
                .ok_or_else(|| schema_error(&info_path, format!("{identifier} isn't registered")))?;
            let path = sdf::Path::abs_root()
                .append_path(identifier.as_str())
                .map_err(|e| schema_error(&schematics, e))?;
            let spec = layer
                .prim(&path)
                .map_err(|e| schema_error(&schematics, e))?
                .ok_or_else(|| schema_error(&schematics, format!("no class spec for {identifier}")))?;
            let prefix = info
                .property_namespace_prefix()
                .map(|p| p.as_str().to_owned())
                .or_else(|| {
                    (info.kind() == usd::SchemaKind::MultipleApplyApi)
                        .then(|| spec.property_children().unwrap_or_default())
                        .into_iter()
                        .flatten()
                        .find_map(|name| {
                            name.as_str()
                                .split_once(":__INSTANCE_NAME__")
                                .map(|(prefix, _)| prefix.to_owned())
                        })
                });
            let template = prefix.as_ref().map(|p| format!("{p}:__INSTANCE_NAME__"));
            let bases: Vec<String> = registry
                .ancestors(identifier)
                .into_iter()
                .skip(1)
                .filter(|b| b.as_str() != "SchemaBase")
                .map(|b| b.as_str().to_owned())
                .collect();
            let mut properties = Vec::new();
            for property_name in spec.property_children().unwrap_or_default() {
                let property_path = path
                    .append_property(property_name.clone())
                    .map_err(|e| schema_error(&schematics, e))?;
                let data = layer.data();
                let get = |key: sdf::FieldKey| data.field(&property_path, key.as_str()).ok().map(|v| v.into_owned());
                let type_name = get(sdf::FieldKey::TypeName)
                    .and_then(|v| {
                        v.as_str()
                            .map(str::to_owned)
                            .or_else(|| tf::Token::try_from(v).ok().map(|t| t.as_str().to_owned()))
                    })
                    .unwrap_or_default();
                let fallback = get(sdf::FieldKey::Default).map(|v| {
                    let mut out = Value::new();
                    out.set_type_name(&type_name);
                    out.assign(v);
                    out
                });
                let text = |key: sdf::FieldKey| get(key).and_then(|v| v.as_str().map(str::to_owned));
                let mut name = property_name.as_str().to_owned();
                if let Some(template) = &template {
                    if let Some(base) = name.strip_prefix(&format!("{template}:")) {
                        name = base.to_owned();
                    } else if name == *template {
                        name = "__INSTANCE_NAME__".to_owned();
                    }
                }
                properties.push(SchemaProperty {
                    name,
                    declared_by: identifier.as_str().to_owned(),
                    relationship: data.spec_type(&property_path) == Some(sdf::SpecType::Relationship),
                    type_name,
                    uniform: get(sdf::FieldKey::Variability)
                        .and_then(|v| sdf::Variability::try_from(v).ok())
                        .is_some_and(|v| v == sdf::Variability::Uniform),
                    fallback,
                    allowed_tokens: strings(&model::allowed_tokens(get(sdf::FieldKey::AllowedTokens).as_ref())),
                    display_group: text(sdf::FieldKey::DisplayGroup),
                    display_name: text(sdf::FieldKey::DisplayName),
                    documentation: text(sdf::FieldKey::Documentation),
                    custom_data: Dictionary(
                        get(sdf::FieldKey::CustomData)
                            .and_then(|v| v.try_as_dictionary_ref().cloned())
                            .unwrap_or_default(),
                    ),
                });
            }
            let api_schemas = spec
                .api_schemas()
                .map(|op| {
                    let explicit = op.explicit_items();
                    if explicit.is_empty() {
                        strings(&[op.prepended_items(), op.appended_items()].concat())
                    } else {
                        strings(explicit)
                    }
                })
                .unwrap_or_default();
            let class_field = |key: sdf::FieldKey| spec.field(key.as_str()).ok().flatten();
            classes.push(SchemaClass {
                name: identifier.as_str().to_owned(),
                cpp_name: cpp_names[identifier.as_str()].clone(),
                kind: SchemaKind::from_usd(info.kind()),
                bases,
                can_only_apply_to: strings(info.can_only_apply_to(None)),
                auto_apply_to: strings(info.auto_apply_to()),
                property_namespace_prefix: prefix,
                allowed_instance_names: strings(info.allowed_instance_names()),
                api_schemas,
                documentation: class_field(sdf::FieldKey::Documentation).and_then(|v| v.as_str().map(str::to_owned)),
                custom_data: Dictionary(
                    class_field(sdf::FieldKey::CustomData)
                        .and_then(|v| v.try_as_dictionary_ref().cloned())
                        .unwrap_or_default(),
                ),
                properties,
            });
        }
        attribute_declarers(&mut classes, &registry);
        let read = |path: &Path| std::fs::read_to_string(path).map_err(|e| schema_error(path, e));
        Ok(Self {
            prefix: {
                let mut chars = name.chars();
                chars
                    .next()
                    .map(|c| c.to_uppercase().chain(chars).collect())
                    .unwrap_or_default()
            },
            name,
            skip_code_generation: true,
            global_data: Dictionary::default(),
            tokens: Vec::new(),
            classes,
            layers: vec![crate::value::simplify_path(&schematics.to_string_lossy())],
            generated_schema: read(&schematics)?,
            plug_info: read(&info_path)?,
        })
    }

    pub fn find_class(&self, name: &str) -> Option<&SchemaClass> {
        self.classes.iter().find(|c| c.name == name)
    }

    pub fn generated_schema(&self) -> &str {
        &self.generated_schema
    }

    pub fn plug_info(&self) -> &str {
        &self.plug_info
    }

    pub fn write_plugin(&self, dir: impl AsRef<Path>) -> Result<()> {
        let dir = dir.as_ref();
        let fail = |path: &Path, e: std::io::Error| Error::Write {
            path: path.display().to_string(),
            message: e.to_string(),
        };
        std::fs::create_dir_all(dir).map_err(|e| fail(dir, e))?;
        for (name, text) in [
            ("generatedSchema.usda", &self.generated_schema),
            ("plugInfo.json", &self.plug_info),
        ] {
            let path = dir.join(name);
            std::fs::write(&path, text).map_err(|e| fail(&path, e))?;
        }
        Ok(())
    }
}

fn same_value(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.base_type() == b.base_type()
                && a.is_array() == b.is_array()
                && a.len() == b.len()
                && a.bytes() == b.bytes()
                && a.strings() == b.strings()
        }
        _ => false,
    }
}

fn same_property(a: &SchemaProperty, b: &SchemaProperty) -> bool {
    a.type_name == b.type_name
        && a.relationship == b.relationship
        && a.uniform == b.uniform
        && a.allowed_tokens == b.allowed_tokens
        && same_value(a.fallback.as_ref(), b.fallback.as_ref())
}

fn attribute_declarers(classes: &mut [SchemaClass], registry: &usd::SchemaRegistry) {
    let snapshot: Vec<SchemaClass> = classes.to_vec();
    let find = |name: &str| snapshot.iter().position(|c| c.name == name);
    let mut declared: Vec<Vec<Option<String>>> = snapshot.iter().map(|c| vec![None; c.properties.len()]).collect();
    fn resolve(
        index: usize,
        property: usize,
        snapshot: &[SchemaClass],
        declared: &mut Vec<Vec<Option<String>>>,
        registry: &usd::SchemaRegistry,
        find: &dyn Fn(&str) -> Option<usize>,
    ) -> String {
        if let Some(done) = &declared[index][property] {
            return done.clone();
        }
        let class = &snapshot[index];
        let own = &class.properties[property];
        let mut owner = class.name.clone();
        if !class.kind.is_applied_api() {
            for base in &class.bases {
                if let Some(base_index) = find(base) {
                    if let Some(at) = snapshot[base_index].properties.iter().position(|p| p.name == own.name) {
                        if same_property(&snapshot[base_index].properties[at], own) {
                            owner = resolve(base_index, at, snapshot, declared, registry, find);
                        }
                        break;
                    }
                } else if let Some(definition) = registry.concrete_prim_definition(&tf::Token::from(base.as_str()))
                    && let Some(inherited) = definition.property(&tf::Token::from(own.name.as_str()))
                {
                    let fallback = inherited.fallback().map(|v| {
                        let mut out = Value::new();
                        out.set_type_name(&own.type_name);
                        out.assign(v);
                        out
                    });
                    if same_value(own.fallback.as_ref(), fallback.as_ref()) {
                        owner = base.clone();
                    }
                    break;
                }
            }
        }
        declared[index][property] = Some(owner.clone());
        owner
    }
    for (index, class) in classes.iter_mut().enumerate() {
        for (property, entry) in class.properties.iter_mut().enumerate() {
            entry.declared_by = resolve(index, property, &snapshot, &mut declared, registry, &find);
        }
    }
}

fn first_sentence(doc: &str) -> String {
    let end = [". ", ".\n", ".\r"]
        .iter()
        .filter_map(|s| doc.find(s))
        .min()
        .unwrap_or(doc.len());
    let sentence = doc[..end].trim();
    if sentence.chars().count() > MAX_BRIEF {
        let cut: String = sentence.chars().take(MAX_BRIEF).collect();
        format!("{cut}...")
    } else if !sentence.is_empty() && !sentence.ends_with('.') {
        format!("{sentence}.")
    } else {
        sentence.to_owned()
    }
}

fn strip_doxygen(text: &str) -> String {
    let mut text = text.replace("\\em ", "").replace("\\li", "-");
    for tag in ["ref", "section"] {
        text = strip_tag(&text, tag);
    }
    text
}

fn strip_tag(text: &str, tag: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(slash) = rest.find('\\') {
        let run = rest[slash..].len() - rest[slash..].trim_start_matches('\\').len();
        let after = &rest[slash + run..];
        let removed = after.strip_prefix(tag).and_then(|a| a.strip_prefix(' ')).and_then(|a| {
            let word = a.find(char::is_whitespace)?;
            (word > 0 && a[word..].starts_with(' ')).then(|| &a[word + 1..])
        });
        match removed {
            Some(tail) => {
                out.push_str(&rest[..slash]);
                rest = tail;
            }
            None => {
                out.push_str(&rest[..slash + run]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn authored_brief(fields: &BTreeMap<String, vt::Value>) -> Option<String> {
    fields
        .get(sdf::FieldKey::CustomData.as_str())?
        .try_as_dictionary_ref()?
        .get(USER_DOC_BRIEF)?
        .as_str()
        .map(|b| b.trim().to_owned())
}

fn user_doc_stage(schema: &Path, search_paths: &[PathBuf]) -> std::result::Result<Option<usd::Stage>, String> {
    let Some(path) = schema.parent().map(|d| d.join("userDoc").join("schemaUserDoc.usda")) else {
        return Ok(None);
    };
    if !path.is_file() {
        return Ok(None);
    }
    let registry = usd::SchemaRegistryBuilder::empty().build().map_err(|e| e.to_string())?;
    usd::Stage::builder()
        .resolver(ar::DefaultResolver::with_search_paths(search_paths.to_vec()))
        .schema_registry(registry)
        .open(path.to_string_lossy().as_ref())
        .map(Some)
        .map_err(|e| e.to_string())
}

struct Briefs<'a> {
    user_doc: Option<&'a usd::Stage>,
    class: &'a model::Class,
    path: sdf::Path,
}

impl Briefs<'_> {
    fn user_doc(&self, property: Option<&model::Property>) -> Option<String> {
        let stage = self.user_doc?;
        let prim = stage.prim(&self.path).ok()?;
        let value = match property {
            None => prim.metadata_by_key(sdf::FieldKey::CustomData.as_str(), USER_DOC_BRIEF),
            Some(p) if p.shape.spec_type == sdf::SpecType::Relationship => prim
                .relationship(p.name.clone())
                .metadata_by_key(sdf::FieldKey::CustomData.as_str(), USER_DOC_BRIEF),
            Some(p) => prim
                .attribute(p.name.clone())
                .metadata_by_key(sdf::FieldKey::CustomData.as_str(), USER_DOC_BRIEF),
        };
        value.ok().flatten()?.as_str().map(|b| b.trim().to_owned())
    }

    fn class(&self) -> Option<String> {
        self.user_doc(None)
            .or_else(|| authored_brief(&self.class.fields))
            .or_else(|| {
                self.class
                    .documentation
                    .as_deref()
                    .filter(|d| !d.is_empty())
                    .map(first_sentence)
            })
    }

    fn property(&self, property: &model::Property) -> Option<String> {
        let property = match self.class.kind == usd::SchemaKind::MultipleApplyApi {
            true => {
                let base = property.schematics_name.as_str().rsplit(':').next().unwrap_or_default();
                self.class.properties.iter().find(|p| p.name.as_str() == base)?
            }
            false => property,
        };
        self.user_doc(Some(property))
            .or_else(|| authored_brief(&property.fields))
            .or_else(|| property.documentation().filter(|d| !d.is_empty()).map(first_sentence))
    }
}

fn set_brief(data: &mut sdf::Data, path: &sdf::Path, brief: Option<String>) {
    let Some(brief) = brief else {
        return;
    };
    let key = sdf::FieldKey::CustomData.as_str();
    let mut custom = data
        .try_field(path, key)
        .ok()
        .flatten()
        .and_then(|v| v.into_owned().try_as_dictionary())
        .unwrap_or_default();
    custom.insert(USER_DOC_BRIEF.to_owned(), vt::Value::String(strip_doxygen(&brief)));
    data.set_field(path, key, vt::Value::Dictionary(custom));
}

fn generated_schema(
    output: &openusd_build::Output,
    schema: &Path,
    search_paths: &[PathBuf],
) -> std::result::Result<String, String> {
    let mut data = output.with_family(|f| f.to_data()).map_err(|e| e.to_string())?;
    unpublish(&mut data, output.library())?;
    let user_doc = user_doc_stage(schema, search_paths)?;
    for class in output
        .library()
        .classes
        .iter()
        .filter(|c| published(output.library(), c))
    {
        let briefs = Briefs {
            user_doc: user_doc.as_ref(),
            class,
            path: sdf::Path::abs_root()
                .append_path(class.identifier.as_str())
                .map_err(|e| e.to_string())?,
        };
        set_brief(&mut data, &briefs.path, briefs.class());
        for property in &class.properties {
            let at = briefs
                .path
                .append_property(property.schematics_name.as_str())
                .map_err(|e| e.to_string())?;
            set_brief(&mut data, &at, briefs.property(property));
        }
    }
    data.set_field(
        &sdf::Path::abs_root(),
        sdf::FieldKey::Comment.as_str(),
        vt::Value::String("WARNING: THIS FILE IS GENERATED BY usdGenSchema.  DO NOT EDIT.".to_owned()),
    );
    sdf::Layer::from_shared("generatedSchema.usda", Arc::new(data))
        .export_to_string()
        .map_err(|e| e.to_string())
}

fn json(value: &vt::Value) -> Json {
    use vt::Value as V;
    match value {
        V::Bool(b) => Json::Bool(*b),
        V::Int(v) => Json::from(*v),
        V::Uint(v) => Json::from(*v),
        V::Int64(v) => Json::from(*v),
        V::Uint64(v) => Json::from(*v),
        V::Float(v) => Json::from(*v),
        V::Double(v) => Json::from(*v),
        V::Dictionary(d) | V::UnregisteredDictionary(d) => {
            Json::Object(d.iter().map(|(k, v)| (k.clone(), json(v))).collect())
        }
        V::StringVec(v) => Json::from(v.clone()),
        V::TokenVec(v) => Json::from(strings(v)),
        V::BoolVec(v) => Json::from(v.clone()),
        V::IntVec(v) => Json::from(v.clone()),
        V::DoubleVec(v) => Json::from(v.clone()),
        other => other.as_str().map_or(Json::Null, Json::from),
    }
}

fn cpp_base(library: &model::Library, class: &model::Class) -> String {
    let Some(base) = class.bases.first() else {
        return "UsdSchemaBase".to_owned();
    };
    library
        .find(&base.identifier)
        .map(|b| b.upstream_name.clone())
        .unwrap_or_else(|| format!("Usd{}", base.class_name))
}

fn type_entry(library: &model::Library, class: &model::Class) -> Json {
    let mut entry = match class.custom_data.get("extraPlugInfo").map(json) {
        Some(Json::Object(extra)) => extra,
        _ => Map::new(),
    };
    entry.insert("bases".to_owned(), Json::from(vec![cpp_base(library, class)]));
    entry.insert("autoGenerated".to_owned(), Json::Bool(true));
    entry.insert(
        "schemaKind".to_owned(),
        Json::from(SchemaKind::from_usd(class.kind).token()),
    );
    let kind = SchemaKind::from_usd(class.kind);
    if kind.is_applied_api() {
        let meta = &class.metadata;
        for (key, list) in [
            ("apiSchemaAutoApplyTo", &meta.auto_apply_to),
            ("apiSchemaCanOnlyApplyTo", &meta.can_only_apply_to),
            ("apiSchemaAllowedInstanceNames", &meta.allowed_instance_names),
        ] {
            if !list.is_empty() {
                entry.insert(key.to_owned(), Json::from(strings(list)));
            }
        }
        if !meta.instance_restrictions.is_empty() {
            let instances: Map<String, Json> = meta
                .instance_restrictions
                .iter()
                .map(|(name, allowed)| {
                    let mut inner = Map::new();
                    inner.insert("apiSchemaCanOnlyApplyTo".to_owned(), Json::from(strings(allowed)));
                    (name.as_str().to_owned(), Json::Object(inner))
                })
                .collect();
            entry.insert("apiSchemaInstances".to_owned(), Json::Object(instances));
        }
    }
    if class.identifier.as_str() != class.upstream_name {
        let mut alias = Map::new();
        alias.insert("UsdSchemaBase".to_owned(), Json::from(class.identifier.as_str()));
        entry.insert("alias".to_owned(), Json::Object(alias));
    }
    entry.insert("schemaIdentifier".to_owned(), Json::from(class.identifier.as_str()));
    Json::Object(entry)
}

fn plugin_omits(library: &model::Library) -> Vec<String> {
    let Some(value) = library.global.get("pluginOmits") else {
        return Vec::new();
    };
    Vec::<String>::try_from(value.clone())
        .or_else(|_| {
            Vec::<tf::Token>::try_from(value.clone())
                .map(|tokens| tokens.iter().map(|token| token.as_str().to_owned()).collect())
        })
        .unwrap_or_default()
}

fn published(library: &model::Library, class: &model::Class) -> bool {
    class.library == library.name
        && !plugin_omits(library)
            .iter()
            .any(|name| name == class.identifier.as_str())
}

fn unpublish(data: &mut sdf::Data, library: &model::Library) -> std::result::Result<(), String> {
    let root = sdf::Path::abs_root();
    let mut dropped = Vec::new();
    for class in library.classes.iter().filter(|c| !published(library, c)) {
        let path = root.append_path(class.identifier.as_str()).map_err(|e| e.to_string())?;
        let specs: Vec<sdf::Path> = data
            .iter()
            .map(|(p, _)| p.clone())
            .filter(|p| p.has_prefix(&path))
            .collect();
        for spec in specs {
            data.erase_spec(&spec);
        }
        dropped.push(class.identifier.clone());
    }
    let key = sdf::ChildrenKey::PrimChildren.as_str();
    if let Ok(children) = data.field(&root, key).map(|v| v.into_owned())
        && let Ok(names) = Vec::<tf::Token>::try_from(children)
    {
        let kept: Vec<tf::Token> = names.into_iter().filter(|n| !dropped.contains(n)).collect();
        data.set_field(&root, key, vt::Value::from(kept));
    }
    Ok(())
}

fn plug_info(library: &model::Library) -> String {
    let types: Map<String, Json> = library
        .classes
        .iter()
        .filter(|c| published(library, c))
        .map(|c| (c.upstream_name.clone(), type_entry(library, c)))
        .collect();
    let mut info = Map::new();
    info.insert("Types".to_owned(), Json::Object(types));
    let mut plugin = Map::new();
    plugin.insert("Info".to_owned(), Json::Object(info));
    plugin.insert("Name".to_owned(), Json::from(library.name.as_str()));
    plugin.insert("ResourcePath".to_owned(), Json::from("."));
    plugin.insert("Root".to_owned(), Json::from("."));
    plugin.insert("Type".to_owned(), Json::from("resource"));
    let mut root = Map::new();
    root.insert("Plugins".to_owned(), Json::from(vec![Json::Object(plugin)]));
    let body = serde_json::to_string_pretty(&Json::Object(root)).unwrap_or_default();
    format!(
        "# Portions of this file auto-generated by usdGenSchema.\n# Edits will survive regeneration except for comments and\n# changes to types with autoGenerated=true.\n{body}\n"
    )
}

#[derive(Debug, Clone)]
pub struct ShaderPort {
    pub name: String,
    pub type_name: String,
    pub fallback: Option<Value>,
    pub allowed_tokens: Vec<String>,
    pub interface_only: bool,
    pub documentation: Option<String>,
    pub sdr_metadata: Dictionary,
}

#[derive(Debug, Clone)]
pub struct ShaderNode {
    pub name: String,
    pub id: String,
    pub documentation: Option<String>,
    pub sdr_metadata: Dictionary,
    pub inputs: Vec<ShaderPort>,
    pub outputs: Vec<ShaderPort>,
}

fn port(port: &openusd_build::shader_defs::Port) -> ShaderPort {
    let type_name = port.type_name.as_str().to_owned();
    ShaderPort {
        fallback: port.fallback.clone().map(|v| {
            let mut out = Value::new();
            out.set_type_name(&type_name);
            out.assign(v);
            out
        }),
        name: port.base_name.clone(),
        type_name,
        allowed_tokens: strings(&port.allowed_tokens),
        interface_only: port.interface_only,
        documentation: port.documentation.clone(),
        sdr_metadata: Dictionary(port.sdr_metadata.clone()),
    }
}

pub fn read_shader_defs(path: impl AsRef<Path>, search_paths: &[PathBuf]) -> Result<Vec<ShaderNode>> {
    let path = path.as_ref();
    let builder = search_paths
        .iter()
        .fold(openusd_build::configure(), |b, p| b.search_path(p));
    let library = builder.read_shader_defs(path).map_err(|e| schema_error(path, e))?;
    Ok(library
        .nodes
        .iter()
        .map(|node| ShaderNode {
            name: node.prim_name.clone(),
            id: node.id.as_str().to_owned(),
            documentation: node.documentation.clone(),
            sdr_metadata: Dictionary(node.sdr_metadata.clone()),
            inputs: node.inputs.iter().map(port).collect(),
            outputs: node.outputs.iter().map(port).collect(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::first_sentence;

    #[test]
    fn first_sentence_ends_at_any_line_ending() {
        assert_eq!(first_sentence("One. Two."), "One.");
        assert_eq!(first_sentence("One.\nTwo."), "One.");
        assert_eq!(first_sentence("One.\r\n\r\nTwo."), "One.");
        assert_eq!(first_sentence("No stop"), "No stop.");
    }
}
