use std::collections::HashMap;
use std::rc::Rc;

use openusd::ar;
use openusd::usd::{self, PrimStatus};
use openusd::{sdf, vt};

use crate::host::{AssetResolver, HostResolver, PackageCache};
use crate::value::{Value, variability_token};
use crate::{Error, Result};

pub(crate) fn usd_err(e: impl std::fmt::Display) -> Error {
    Error::Usd(e.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct Prim(u32);

impl Prim {
    pub const PSEUDO_ROOT: Prim = Prim(0);

    pub const fn index(self) -> u32 {
        self.0
    }

    pub const fn from_index(index: u32) -> Self {
        Self(index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TraverseFlags(u32);

impl TraverseFlags {
    pub const DEFAULT: Self = Self(0);
    pub const INCLUDE_INACTIVE: Self = Self(1 << 0);
    pub const INCLUDE_UNLOADED: Self = Self(1 << 1);
    pub const INCLUDE_UNDEFINED: Self = Self(1 << 2);
    pub const INCLUDE_ABSTRACT: Self = Self(1 << 3);
    pub const INSTANCE_PROXIES: Self = Self(1 << 4);
    pub const ALL: Self = Self(0b1_1111);

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn from_bits_truncate(bits: u32) -> Self {
        Self(bits & Self::ALL.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    fn predicate(self) -> usd::PrimPredicate {
        let mut required = PrimStatus::ACTIVE | PrimStatus::LOADED | PrimStatus::DEFINED;
        let mut rejected = PrimStatus::ABSTRACT;
        if self.contains(Self::INCLUDE_INACTIVE) {
            required.remove(PrimStatus::ACTIVE);
        }
        if self.contains(Self::INCLUDE_UNLOADED) {
            required.remove(PrimStatus::LOADED);
        }
        if self.contains(Self::INCLUDE_UNDEFINED) {
            required.remove(PrimStatus::DEFINED);
        }
        if self.contains(Self::INCLUDE_ABSTRACT) {
            rejected.remove(PrimStatus::ABSTRACT);
        }
        usd::PrimPredicate::new(required, rejected).with_instance_proxies(self.contains(Self::INSTANCE_PROXIES))
    }
}

impl std::ops::BitOr for TraverseFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PrimFlags(u32);

impl PrimFlags {
    pub const ACTIVE: Self = Self(1 << 0);
    pub const LOADED: Self = Self(1 << 1);
    pub const DEFINED: Self = Self(1 << 2);
    pub const ABSTRACT: Self = Self(1 << 3);
    pub const MODEL: Self = Self(1 << 4);
    pub const INSTANCEABLE: Self = Self(1 << 5);
    pub const INSTANCE: Self = Self(1 << 6);
    pub const INSTANCE_PROXY: Self = Self(1 << 7);
    pub const PROTOTYPE: Self = Self(1 << 8);
    pub const IN_PROTOTYPE: Self = Self(1 << 9);

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum Specifier {
    #[default]
    Def = 0,
    Over = 1,
    Class = 2,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AttributeInfo {
    pub type_name: String,
    pub uniform: bool,
    pub custom: bool,
    pub has_value: bool,
    pub has_authored_value: bool,
    pub might_be_time_varying: bool,
}

#[derive(Clone)]
pub struct OpenOptions {
    pub load_payloads: bool,
    pub resolver: Option<Rc<dyn AssetResolver>>,
    pub schema_paths: Vec<std::path::PathBuf>,
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self {
            load_payloads: true,
            resolver: None,
            schema_paths: Vec::new(),
        }
    }
}

impl std::fmt::Debug for OpenOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenOptions")
            .field("load_payloads", &self.load_payloads)
            .field("resolver", &self.resolver.is_some())
            .field("schema_paths", &self.schema_paths)
            .finish()
    }
}

pub struct Stage {
    pub(crate) inner: usd::Stage,
    prims: Vec<(sdf::Path, String)>,
    lookup: HashMap<sdf::Path, u32>,
    resolver: Box<dyn ar::Resolver>,
    pub(crate) caches: crate::helpers::Caches,
}

impl Stage {
    pub fn open(path: &str, options: &OpenOptions) -> Result<Self> {
        let load = if options.load_payloads {
            usd::InitialLoadSet::LoadAll
        } else {
            usd::InitialLoadSet::LoadNone
        };
        let registry = crate::schemas::registry(&options.schema_paths)?;
        let builder = usd::Stage::builder().schema_registry(registry).load(load);
        let (builder, resolver): (_, Box<dyn ar::Resolver>) = match &options.resolver {
            Some(host) => {
                let packages = PackageCache::default();
                (
                    builder.resolver(HostResolver::new(host.clone(), packages.clone())),
                    Box::new(HostResolver::new(host.clone(), packages)),
                )
            }
            None => (builder, Box::new(ar::DefaultResolver::new())),
        };
        let inner = builder.open(path).map_err(|e| Error::Open {
            path: path.to_owned(),
            message: e.to_string(),
        })?;
        let root = sdf::Path::abs_root();
        let mut lookup = HashMap::new();
        lookup.insert(root.clone(), 0);
        Ok(Self {
            inner,
            prims: vec![(root, "/".to_owned())],
            lookup,
            resolver,
            caches: crate::helpers::Caches::default(),
        })
    }

    pub fn pseudo_root(&self) -> Prim {
        Prim::PSEUDO_ROOT
    }

    pub(crate) fn intern(&mut self, path: &sdf::Path) -> Prim {
        if let Some(&id) = self.lookup.get(path) {
            return Prim(id);
        }
        let id = u32::try_from(self.prims.len()).expect("prim table overflow");
        self.prims.push((path.clone(), path.to_string()));
        self.lookup.insert(path.clone(), id);
        Prim(id)
    }

    pub(crate) fn sdf_path(&self, prim: Prim) -> Result<&sdf::Path> {
        self.prims
            .get(prim.0 as usize)
            .map(|(p, _)| p)
            .ok_or(Error::InvalidPrim)
    }

    pub(crate) fn usd_prim(&self, prim: Prim) -> Result<usd::Prim> {
        let path = self.sdf_path(prim)?.clone();
        self.inner.prim(path).map_err(usd_err)
    }

    pub fn prim_at_path(&mut self, path: &str) -> Result<Option<Prim>> {
        let sdf_path = sdf::Path::new(path).map_err(|e| Error::InvalidPath(format!("{path}: {e}")))?;
        if let Some(&id) = self.lookup.get(&sdf_path) {
            return Ok(Some(Prim(id)));
        }
        let prim = self.inner.prim(sdf_path.clone()).map_err(usd_err)?;
        if !prim.is_valid().map_err(usd_err)? {
            return Ok(None);
        }
        Ok(Some(self.intern(&sdf_path)))
    }

    pub fn path(&self, prim: Prim) -> Result<&str> {
        self.prims
            .get(prim.0 as usize)
            .map(|(_, s)| s.as_str())
            .ok_or(Error::InvalidPrim)
    }

    pub fn name(&self, prim: Prim) -> Result<&str> {
        Ok(self.sdf_path(prim)?.name().unwrap_or(""))
    }

    pub fn parent(&mut self, prim: Prim) -> Result<Option<Prim>> {
        let parent = self.sdf_path(prim)?.parent();
        Ok(parent.map(|p| self.intern(&p)))
    }

    pub fn default_prim(&mut self) -> Result<Option<Prim>> {
        match self.inner.default_prim() {
            Some(name) => self.prim_at_path(&format!("/{}", name.as_str())),
            None => Ok(None),
        }
    }

    pub fn traverse(&mut self, flags: TraverseFlags) -> Result<Vec<Prim>> {
        let mut paths = Vec::new();
        self.inner
            .traverse(flags.predicate(), |p| paths.push(p.clone()))
            .map_err(usd_err)?;
        Ok(paths.iter().map(|p| self.intern(p)).collect())
    }

    pub fn traverse_from(&mut self, root: Prim, flags: TraverseFlags) -> Result<Vec<Prim>> {
        if root == Prim::PSEUDO_ROOT {
            return self.traverse(flags);
        }
        let range = self
            .usd_prim(root)?
            .filtered_subtree(flags.predicate())
            .map_err(usd_err)?;
        let mut paths = Vec::new();
        for prim in range {
            paths.push(prim.map_err(usd_err)?.path().clone());
        }
        Ok(paths.iter().map(|p| self.intern(p)).collect())
    }

    pub fn children(&mut self, prim: Prim, flags: TraverseFlags) -> Result<Vec<Prim>> {
        let children = self
            .usd_prim(prim)?
            .filtered_children(flags.predicate())
            .map_err(usd_err)?;
        Ok(children.iter().map(|c| self.intern(c.path())).collect())
    }

    pub fn prototypes(&mut self) -> Result<Vec<Prim>> {
        let paths = self.inner.prototypes().map_err(usd_err)?;
        Ok(paths.iter().map(|p| self.intern(p)).collect())
    }

    pub fn prototype_of(&mut self, prim: Prim) -> Result<Option<Prim>> {
        let proto = self.usd_prim(prim)?.prototype().map_err(usd_err)?;
        Ok(proto.map(|p| self.intern(&p)))
    }

    pub fn instances_of(&mut self, prototype: Prim) -> Result<Vec<Prim>> {
        let paths = self.usd_prim(prototype)?.instances().map_err(usd_err)?;
        Ok(paths.iter().map(|p| self.intern(p)).collect())
    }

    pub fn type_name(&self, prim: Prim) -> Result<String> {
        let name = self.usd_prim(prim)?.type_name().map_err(usd_err)?;
        Ok(name.map(|t| t.as_str().to_owned()).unwrap_or_default())
    }

    pub fn specifier(&self, prim: Prim) -> Result<Specifier> {
        Ok(match self.usd_prim(prim)?.specifier().map_err(usd_err)? {
            Some(sdf::Specifier::Over) => Specifier::Over,
            Some(sdf::Specifier::Class) => Specifier::Class,
            _ => Specifier::Def,
        })
    }

    pub fn kind(&self, prim: Prim) -> Result<String> {
        let kind = self.usd_prim(prim)?.kind().map_err(usd_err)?;
        Ok(kind.map(|t| t.as_str().to_owned()).unwrap_or_default())
    }

    pub fn is_a(&self, prim: Prim, schema: &str) -> Result<bool> {
        self.usd_prim(prim)?.is_a(schema).map_err(usd_err)
    }

    pub fn has_api(&self, prim: Prim, schema: &str) -> Result<bool> {
        let (name, instance) = match schema.split_once(':') {
            Some((name, instance)) => (name, Some(instance)),
            None => (schema, None),
        };
        let usd_prim = self.usd_prim(prim)?;
        match instance {
            None => Ok(usd_prim.has_api_schema(name).map_err(usd_err)?
                || !usd_prim.api_schema_instance_names(name).map_err(usd_err)?.is_empty()),
            Some(instance) => Ok(usd_prim
                .api_schema_instance_names(name)
                .map_err(usd_err)?
                .iter()
                .any(|i| i.as_str() == instance)),
        }
    }

    pub fn applied_schemas(&self, prim: Prim) -> Result<Vec<String>> {
        let schemas = self.usd_prim(prim)?.api_schemas().map_err(usd_err)?;
        Ok(schemas.iter().map(|t| t.as_str().to_owned()).collect())
    }

    pub fn flags(&self, prim: Prim) -> Result<PrimFlags> {
        let p = self.usd_prim(prim)?;
        let mut bits = 0;
        let checks: [(PrimFlags, openusd::Result<bool>); 10] = [
            (PrimFlags::ACTIVE, p.is_active()),
            (PrimFlags::LOADED, p.is_loaded()),
            (PrimFlags::DEFINED, p.is_defined()),
            (PrimFlags::ABSTRACT, p.is_abstract()),
            (PrimFlags::MODEL, p.is_model()),
            (PrimFlags::INSTANCEABLE, p.is_instanceable()),
            (PrimFlags::INSTANCE, p.is_instance()),
            (PrimFlags::INSTANCE_PROXY, p.is_instance_proxy()),
            (PrimFlags::PROTOTYPE, p.is_prototype()),
            (PrimFlags::IN_PROTOTYPE, p.is_in_prototype()),
        ];
        for (flag, check) in checks {
            if check.map_err(usd_err)? {
                bits |= flag.0;
            }
        }
        Ok(PrimFlags(bits))
    }

    pub fn attribute_names(&self, prim: Prim, authored_only: bool) -> Result<Vec<String>> {
        let p = self.usd_prim(prim)?;
        let attrs = if authored_only {
            p.authored_attributes()
        } else {
            p.attributes()
        }
        .map_err(usd_err)?;
        Ok(attrs.iter().map(|a| a.name().to_owned()).collect())
    }

    pub fn relationship_names(&self, prim: Prim, authored_only: bool) -> Result<Vec<String>> {
        let p = self.usd_prim(prim)?;
        let rels = if authored_only {
            p.authored_relationships()
        } else {
            p.relationships()
        }
        .map_err(usd_err)?;
        Ok(rels.iter().map(|r| r.name().to_owned()).collect())
    }

    fn attribute(&self, prim: Prim, name: &str) -> Result<usd::Attribute> {
        Ok(self.usd_prim(prim)?.attribute(name))
    }

    pub fn attribute_info(&self, prim: Prim, name: &str) -> Result<Option<AttributeInfo>> {
        let attr = self.attribute(prim, name)?;
        if !attr.is_defined().map_err(usd_err)? {
            return Ok(None);
        }
        let variability = attr.variability().map_err(usd_err)?.unwrap_or_default();
        Ok(Some(AttributeInfo {
            type_name: attr
                .type_name()
                .map_err(usd_err)?
                .map(|t| t.as_str().to_owned())
                .unwrap_or_default(),
            uniform: variability_token(variability) == "uniform",
            custom: attr.is_custom().map_err(usd_err)?,
            has_value: attr.has_value().map_err(usd_err)?,
            has_authored_value: attr.has_authored_value().map_err(usd_err)?,
            might_be_time_varying: attr.might_be_time_varying().map_err(usd_err)?,
        }))
    }

    pub fn get(&self, prim: Prim, name: &str, time: Option<f64>, out: &mut Value) -> Result<bool> {
        let attr = self.attribute(prim, name)?;
        let value = attr
            .get_at::<vt::Value>(time.map(usd::TimeCode::new))
            .map_err(usd_err)?;
        let type_name = attr.type_name().map_err(usd_err)?;
        out.set_type_name(type_name.as_ref().map_or("", |t| t.as_str()));
        match value {
            Some(v) => Ok(out.assign(v)),
            None => {
                out.assign(vt::Value::None);
                Ok(false)
            }
        }
    }

    pub fn time_samples(&self, prim: Prim, name: &str) -> Result<Vec<f64>> {
        self.attribute(prim, name)?.time_sample_times().map_err(usd_err)
    }

    pub fn connections(&self, prim: Prim, name: &str) -> Result<Vec<String>> {
        let paths = self.attribute(prim, name)?.connections().map_err(usd_err)?;
        Ok(paths.iter().map(ToString::to_string).collect())
    }

    pub fn targets(&self, prim: Prim, name: &str, forwarded: bool) -> Result<Vec<String>> {
        let rel = self.usd_prim(prim)?.relationship(name);
        let paths = if forwarded {
            rel.forwarded_targets()
        } else {
            rel.targets()
        }
        .map_err(usd_err)?;
        Ok(paths.iter().map(ToString::to_string).collect())
    }

    pub fn prim_metadata(&self, prim: Prim, key: &str, out: &mut Value) -> Result<bool> {
        let value = self.usd_prim(prim)?.metadata::<vt::Value>(key).map_err(usd_err)?;
        Ok(assign_metadata(out, value))
    }

    pub fn attribute_metadata(&self, prim: Prim, name: &str, key: &str, out: &mut Value) -> Result<bool> {
        let value = self
            .attribute(prim, name)?
            .metadata::<vt::Value>(key)
            .map_err(usd_err)?;
        Ok(assign_metadata(out, value))
    }

    pub fn stage_metadata(&self, key: &str, out: &mut Value) -> Result<bool> {
        let value = self.inner.stage_metadata(key).map_err(usd_err)?;
        Ok(assign_metadata(out, value))
    }

    pub fn has_authored_stage_metadata(&self, key: &str) -> Result<bool> {
        self.inner.has_authored_stage_metadata(key).map_err(usd_err)
    }

    pub fn start_time_code(&self) -> f64 {
        self.inner.start_time_code()
    }

    pub fn end_time_code(&self) -> f64 {
        self.inner.end_time_code()
    }

    pub fn time_codes_per_second(&self) -> f64 {
        self.inner.time_codes_per_second()
    }

    pub fn frames_per_second(&self) -> f64 {
        self.inner.frames_per_second()
    }

    pub fn layer_identifiers(&self) -> Vec<String> {
        self.inner
            .layer_identifiers()
            .iter()
            .map(|l| crate::value::simplify_path(l))
            .collect()
    }

    pub fn composition_errors(&self) -> Vec<String> {
        self.inner
            .composition_errors()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    pub fn load(&mut self, prim: Prim, with_descendants: bool) -> Result<()> {
        let policy = if with_descendants {
            usd::LoadPolicy::WithDescendants
        } else {
            usd::LoadPolicy::WithoutDescendants
        };
        let path = self.sdf_path(prim)?.clone();
        self.inner.load(path, policy).map_err(usd_err)?;
        self.caches = crate::helpers::Caches::default();
        Ok(())
    }

    pub fn unload(&mut self, prim: Prim) -> Result<()> {
        let path = self.sdf_path(prim)?.clone();
        self.inner.unload(path).map_err(usd_err)?;
        self.caches = crate::helpers::Caches::default();
        Ok(())
    }

    pub fn loadable(&mut self, root: Prim) -> Result<Vec<Prim>> {
        let path = self.sdf_path(root)?.clone();
        let paths = self.inner.find_loadable(path).map_err(usd_err)?;
        Ok(paths.iter().map(|p| self.intern(p)).collect())
    }

    pub fn read_asset(&self, resolved_path: &str) -> Result<Vec<u8>> {
        let fail = |e: std::io::Error| Error::Asset {
            path: resolved_path.to_owned(),
            message: e.to_string(),
        };
        let mut asset = self
            .resolver
            .open_asset(&ar::ResolvedPath::new(resolved_path))
            .map_err(fail)?;
        asset.read_all().map_err(fail)
    }

    pub fn resolve_asset(&self, asset_path: &str) -> Option<String> {
        self.resolver
            .resolve(asset_path)
            .map(|p| crate::value::simplify_path(&p.to_string_lossy()))
    }
}

fn assign_metadata(out: &mut Value, value: Option<vt::Value>) -> bool {
    out.set_type_name("");
    out.assign(value.unwrap_or(vt::Value::None))
}
