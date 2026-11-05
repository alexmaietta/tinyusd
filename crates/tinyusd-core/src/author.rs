use std::collections::HashMap;
use std::io::Cursor;

use openusd::usd::{self, StageAuthoringError};
use openusd::{gf, sdf, tf, vt};

use crate::BaseType;
use crate::stage::usd_err;
use crate::value::{Value, simplify_path};
use crate::{Error, OpenOptions, Prim, Result, Stage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum LayerFormat {
    Usda = 0,
    Usdc = 1,
    Usdz = 2,
}

impl LayerFormat {
    pub fn from_path(path: &str) -> Option<Self> {
        let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
        match ext.as_str() {
            "usda" => Some(Self::Usda),
            "usdc" | "usd" => Some(Self::Usdc),
            "usdz" => Some(Self::Usdz),
            _ => None,
        }
    }

    const fn extension(self) -> &'static str {
        match self {
            Self::Usda => "usda",
            Self::Usdc => "usdc",
            Self::Usdz => "usdz",
        }
    }
}

fn write_error(path: &str, message: impl std::fmt::Display) -> Error {
    Error::Write {
        path: path.to_owned(),
        message: message.to_string(),
    }
}

fn layer_format(path: &str) -> Result<LayerFormat> {
    LayerFormat::from_path(path).ok_or_else(|| write_error(path, "the extension must be .usda, .usdc, .usd or .usdz"))
}

fn serialize(data: &dyn sdf::AbstractData, format: LayerFormat, path: &str) -> Result<Vec<u8>> {
    let writer = sdf::LayerRegistry::find_by_extension(format.extension())
        .ok_or_else(|| write_error(path, format!("no writer for .{}", format.extension())))?;
    let mut out = Cursor::new(Vec::new());
    writer.write(data, &mut out).map_err(|e| write_error(path, e))?;
    Ok(out.into_inner())
}

fn author_err(e: StageAuthoringError) -> Error {
    match e {
        StageAuthoringError::ValueType(e) => Error::InvalidValue(e.to_string()),
        other => Error::Usd(other.to_string()),
    }
}

fn parse_path(path: &str) -> Result<sdf::Path> {
    sdf::Path::new(path).map_err(|e| Error::InvalidPath(format!("{path}: {e}")))
}

fn parse_paths(paths: &[&str]) -> Result<Vec<sdf::Path>> {
    paths.iter().map(|p| parse_path(p)).collect()
}

fn type_name(name: &str) -> Result<sdf::ValueTypeName> {
    let parsed = sdf::ValueTypeName::from(name);
    if parsed.kind().is_none() && name != "opaque" {
        return Err(Error::InvalidValue(format!("unknown attribute type '{name}'")));
    }
    Ok(parsed)
}

#[derive(Default)]
pub(crate) struct Batch {
    ops: Vec<Op>,
    attributes: HashMap<sdf::Path, usize>,
    defined: HashMap<sdf::Path, tf::Token>,
}

enum Op {
    Define {
        path: sdf::Path,
        type_name: tf::Token,
    },
    Override {
        path: sdf::Path,
    },
    Attribute {
        path: sdf::Path,
        type_name: sdf::ValueTypeName,
        uniform: bool,
        values: Vec<(vt::Value, Option<f64>)>,
        block: bool,
        metadata: Vec<(&'static str, vt::Value)>,
    },
    Targets {
        path: sdf::Path,
        targets: Vec<sdf::Path>,
    },
}

impl Batch {
    fn attribute(&mut self, path: &sdf::Path, type_name: sdf::ValueTypeName) -> &mut Op {
        let index = *self.attributes.entry(path.clone()).or_insert_with(|| {
            self.ops.push(Op::Attribute {
                path: path.clone(),
                type_name,
                uniform: false,
                values: Vec::new(),
                block: false,
                metadata: Vec::new(),
            });
            self.ops.len() - 1
        });
        &mut self.ops[index]
    }

    fn commit(self, stage: &usd::Stage) -> std::result::Result<(), StageAuthoringError> {
        stage.edit(|edit| {
            for op in self.ops {
                match op {
                    Op::Define { path, type_name } => {
                        edit.define_typed_prim(path, type_name.as_str())?;
                    }
                    Op::Override { path } => {
                        edit.override_prim(path)?;
                    }
                    Op::Attribute {
                        path,
                        type_name,
                        uniform,
                        values,
                        block,
                        metadata,
                    } => {
                        let mut builder = edit.attribute_builder(path, type_name);
                        for (key, value) in metadata {
                            builder = builder.metadata(key, value);
                        }
                        if uniform {
                            builder = builder.variability(sdf::Variability::Uniform);
                        }
                        if block {
                            builder = builder.block();
                        }
                        for (value, time) in values {
                            builder = builder.set_at(value, time.map(usd::TimeCode::new));
                        }
                        builder.build()?;
                    }
                    Op::Targets { path, targets } => {
                        edit.relationship_builder(path).set_targets(targets).build()?;
                    }
                }
            }
            Ok(())
        })
    }
}

impl Stage {
    pub fn create_new(path: &str, options: &OpenOptions) -> Result<Self> {
        let format = layer_format(path)?;
        if format == LayerFormat::Usdz {
            return Err(write_error(
                path,
                "a .usdz package cannot be edited in place; create a .usda or .usdc stage and export it",
            ));
        }
        let empty = sdf::Layer::new_anonymous("tinyusd");
        let bytes = serialize(empty.data(), format, path)?;
        store(options.resolver.as_deref(), path, &bytes)?;
        Self::open(path, options)
    }

    pub fn create_in_memory(options: &OpenOptions) -> Result<Self> {
        let (builder, resolver) = Self::builder(options)?;
        let inner = builder.in_memory("tinyusd.usda").map_err(|e| Error::Open {
            path: "<memory>".to_owned(),
            message: e.to_string(),
        })?;
        Ok(Self::wrap(inner, resolver, options))
    }

    fn store(&self, path: &str, bytes: &[u8]) -> Result<()> {
        store(self.host.as_deref(), path, bytes)
    }

    fn store_layer(&self, layer: &sdf::Layer, path: &str) -> Result<()> {
        if self.host.is_some() {
            let bytes = serialize(layer.data(), layer_format(path)?, path)?;
            self.store(path, &bytes)
        } else {
            layer.export(path).map_err(|e| write_error(path, e))
        }
    }

    fn handle(&self, layer: &str) -> Result<sdf::LayerHandle> {
        if let Some(handle) = self.inner.layer_handle(layer) {
            return Ok(handle);
        }
        self.inner
            .layer_identifiers()
            .iter()
            .find(|id| simplify_path(id) == layer)
            .and_then(|id| self.inner.layer_handle(id))
            .ok_or_else(|| Error::Usd(format!("no layer '{layer}' on the stage")))
    }

    fn not_batched(&self, what: &str) -> Result<()> {
        if self.batch.is_some() {
            return Err(Error::Usd(format!(
                "{what} can't be queued in an edit batch; commit the batch first"
            )));
        }
        Ok(())
    }

    fn stage_metadata_edit(
        &mut self,
        f: impl FnOnce(&usd::Stage) -> std::result::Result<(), StageAuthoringError>,
    ) -> Result<()> {
        self.immediate("setting stage metadata", |s| {
            let _root = s.inner.edit_context(s.inner.edit_target_root())?;
            f(&s.inner)
        })
    }

    fn touched(&mut self) {
        self.caches = crate::helpers::Caches::default();
    }

    fn immediate<T>(
        &mut self,
        what: &str,
        f: impl FnOnce(&Self) -> std::result::Result<T, StageAuthoringError>,
    ) -> Result<T> {
        self.not_batched(what)?;
        let out = f(self).map_err(author_err)?;
        self.touched();
        Ok(out)
    }

    pub fn save(&mut self) -> Result<()> {
        self.not_batched("save")?;
        for id in self.inner.layer_identifiers() {
            let Some(handle) = self.inner.layer_handle(&id) else {
                continue;
            };
            let dirty = handle.read(|l| l.is_dirty() && !l.is_anonymous()).map_err(usd_err)?;
            if !dirty {
                continue;
            }
            let shown = simplify_path(&id);
            if self.host.is_some() {
                let format = layer_format(&id)?;
                let bytes = handle
                    .read(|l| serialize(l.data(), format, &shown))
                    .map_err(usd_err)??;
                self.store(&id, &bytes)?;
            } else {
                handle.save().map_err(|e| write_error(&shown, e))?;
            }
        }
        Ok(())
    }

    pub fn export(&self, path: &str) -> Result<()> {
        self.not_batched("export")?;
        let flat = self.inner.flatten().map_err(usd_err)?;
        self.store_layer(&flat, path)
    }

    pub fn export_layer(&self, layer: &str, path: &str) -> Result<()> {
        self.not_batched("export")?;
        self.handle(layer)?
            .read(|l| self.store_layer(l, path))
            .map_err(usd_err)?
    }

    pub fn export_bytes(&self, layer: Option<&str>, format: LayerFormat) -> Result<Vec<u8>> {
        self.not_batched("export")?;
        match layer {
            None => {
                let flat = self.inner.flatten().map_err(usd_err)?;
                serialize(flat.data(), format, "<flattened>")
            }
            Some(id) => self
                .handle(id)?
                .read(|l| serialize(l.data(), format, id))
                .map_err(usd_err)?,
        }
    }

    pub fn edit_target(&self) -> String {
        simplify_path(self.inner.edit_target().layer_identifier())
    }

    pub fn set_edit_target(&mut self, layer: &str) -> Result<()> {
        self.not_batched("changing the edit target")?;
        let id = self.handle(layer)?.identifier().to_owned();
        let target = self
            .inner
            .edit_target_for_local_layer(&id)
            .ok_or_else(|| Error::Usd(format!("'{layer}' is not in the stage's root layer stack")))?;
        self.inner.set_edit_target(target).map_err(author_err)
    }

    pub fn add_sublayer(&mut self, asset_path: &str, create: bool) -> Result<String> {
        self.not_batched("adding a sublayer")?;
        let (root, absolute) = {
            let root = self.inner.root_layer();
            (root.identifier().to_owned(), root.absolute_path(asset_path))
        };
        if create && self.resolver.resolve(&absolute).is_none() {
            let format = layer_format(&absolute)?;
            let empty = sdf::Layer::new_anonymous("tinyusd");
            let bytes = serialize(empty.data(), format, &absolute)?;
            self.store(&absolute, &bytes)?;
        }
        let resolved = self
            .resolver
            .resolve(&absolute)
            .map(|p| p.to_string_lossy().into_owned())
            .ok_or_else(|| Error::Usd(format!("sublayer '{asset_path}' was not found")))?;
        let authored = asset_path.to_owned();
        self.inner
            .edit_layer(&root, |edit| {
                edit.pseudo_root_mut()?
                    .insert_sublayer(0, authored, sdf::LayerOffset::default());
                Ok(())
            })
            .map_err(author_err)?;
        self.touched();
        let id = self.handle(&simplify_path(&resolved))?.identifier().to_owned();
        Ok(simplify_path(&id))
    }

    pub fn begin_edit(&mut self) -> Result<()> {
        if self.batch.is_some() {
            return Err(Error::Usd("an edit batch is already open".to_owned()));
        }
        self.batch = Some(Batch::default());
        Ok(())
    }

    pub fn commit_edit(&mut self) -> Result<()> {
        let batch = self
            .batch
            .take()
            .ok_or_else(|| Error::Usd("no edit batch is open".to_owned()))?;
        let result = batch.commit(&self.inner).map_err(author_err);
        self.touched();
        result
    }

    pub fn discard_edit(&mut self) {
        self.batch = None;
    }

    pub fn in_edit(&self) -> bool {
        self.batch.is_some()
    }

    pub fn edit<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.begin_edit()?;
        match f(self) {
            Ok(value) => {
                self.commit_edit()?;
                Ok(value)
            }
            Err(e) => {
                self.discard_edit();
                Err(e)
            }
        }
    }

    pub fn define_prim(&mut self, path: &str, type_name: &str) -> Result<Prim> {
        let sdf_path = parse_path(path)?;
        let type_name = tf::Token::from(type_name.to_owned());
        if let Some(batch) = &mut self.batch {
            batch.defined.insert(sdf_path.clone(), type_name.clone());
            batch.ops.push(Op::Define {
                path: sdf_path.clone(),
                type_name,
            });
        } else {
            self.inner
                .define_typed_prim(sdf_path.clone(), type_name.as_str())
                .map_err(author_err)?;
            self.touched();
        }
        Ok(self.intern(&sdf_path))
    }

    pub fn override_prim(&mut self, path: &str) -> Result<Prim> {
        let sdf_path = parse_path(path)?;
        if let Some(batch) = &mut self.batch {
            batch.ops.push(Op::Override { path: sdf_path.clone() });
        } else {
            self.inner.override_prim(sdf_path.clone()).map_err(author_err)?;
            self.touched();
        }
        Ok(self.intern(&sdf_path))
    }

    pub fn remove_prim(&mut self, prim: Prim) -> Result<bool> {
        let path = self.sdf_path(prim)?.clone();
        self.immediate("removing a prim", |s| s.inner.remove_prim(path))
    }

    pub fn set_type_name(&mut self, prim: Prim, type_name: &str) -> Result<()> {
        let p = self.usd_prim(prim)?;
        self.immediate("setting a type name", |_| p.set_type_name(type_name).map(drop))
    }

    pub fn set_active(&mut self, prim: Prim, active: bool) -> Result<()> {
        let p = self.usd_prim(prim)?;
        self.immediate("setting active", |_| p.set_active(active).map(drop))
    }

    pub fn set_kind(&mut self, prim: Prim, kind: &str) -> Result<()> {
        let p = self.usd_prim(prim)?;
        self.immediate("setting a kind", |_| p.set_kind(kind).map(drop))
    }

    pub fn set_instanceable(&mut self, prim: Prim, instanceable: bool) -> Result<()> {
        let p = self.usd_prim(prim)?;
        self.immediate("setting instanceable", |_| p.set_instanceable(instanceable).map(drop))
    }

    pub fn apply_api(&mut self, prim: Prim, schema: &str) -> Result<()> {
        let p = self.usd_prim(prim)?;
        self.immediate("applying an API schema", |_| p.apply_api(schema).map(drop))
    }

    pub fn remove_api(&mut self, prim: Prim, schema: &str) -> Result<()> {
        self.not_batched("removing an API schema")?;
        let path = self.sdf_path(prim)?.clone();
        let target = self.inner.edit_target();
        let spec_path = target
            .map_to_spec_path(&path)
            .ok_or_else(|| Error::Usd(format!("{path} is outside the edit target")))?;
        let local = self
            .inner
            .layer(target.layer_identifier())
            .and_then(|layer| layer.prim(spec_path).ok().flatten().and_then(|spec| spec.api_schemas()));
        let name = tf::Token::from(schema.to_owned());
        let mut op = local.unwrap_or_default();
        for ty in [
            sdf::ListOpType::Explicit,
            sdf::ListOpType::Prepended,
            sdf::ListOpType::Appended,
            sdf::ListOpType::Added,
        ] {
            op.remove_item(ty, &name);
        }
        if !op.is_explicit() && !op.deleted_items().contains(&name) {
            op.push(sdf::ListOpType::Deleted, name);
        }
        let p = self.usd_prim(prim)?;
        self.immediate("removing an API schema", |_| {
            p.set_metadata("apiSchemas", vt::Value::TokenListOp(op)).map(drop)
        })
    }

    pub fn rename_prim(&mut self, prim: Prim, new_name: &str) -> Result<()> {
        let parent = self.parent(prim)?.unwrap_or(Prim::PSEUDO_ROOT);
        self.move_prim(prim, parent, new_name)
    }

    pub fn reparent_prim(&mut self, prim: Prim, new_parent: Prim) -> Result<()> {
        let name = self.name(prim)?.to_owned();
        self.move_prim(prim, new_parent, &name)
    }

    pub fn move_prim(&mut self, prim: Prim, new_parent: Prim, new_name: &str) -> Result<()> {
        self.not_batched("moving a prim")?;
        let old = self.sdf_path(prim)?.clone();
        if old == sdf::Path::abs_root() {
            return Err(Error::InvalidPrim);
        }
        let new = self
            .sdf_path(new_parent)?
            .append_path(new_name)
            .map_err(|e| Error::InvalidPath(format!("{new_name}: {e}")))?;
        if new == old {
            return Ok(());
        }
        let mut editor = usd::NamespaceEditor::new(&self.inner);
        editor.move_prim(old.clone(), new.clone()).map_err(usd_err)?;
        editor.apply().map_err(usd_err)?;
        self.retarget_prims(&old, &new);
        self.touched();
        Ok(())
    }

    pub fn edit_variant(&mut self, prim: Prim, variant_set: &str, variant: &str) -> Result<()> {
        self.not_batched("changing the edit target")?;
        let layer = self.inner.edit_target().layer_identifier().to_owned();
        let plain = self
            .inner
            .edit_target_for_local_layer(&layer)
            .ok_or_else(|| Error::Usd(format!("'{layer}' is not in the stage's root layer stack")))?;
        self.inner.set_edit_target(plain).map_err(author_err)?;
        let p = self.usd_prim(prim)?;
        let set = p.variant_sets().get(variant_set);
        if !set.names().map_err(usd_err)?.iter().any(|v| v == variant) {
            set.add_variant(variant, usd::ListPosition::BackOfPrepend)
                .map_err(author_err)?;
        }
        set.set_selection(variant).map_err(author_err)?;
        let target_path = p
            .path()
            .append_variant_selection(variant_set, variant)
            .map_err(|e| Error::InvalidPath(e.to_string()))?;
        let target = usd::EditTarget::for_local_direct_variant(layer, target_path).map_err(usd_err)?;
        self.inner.set_edit_target(target).map_err(author_err)?;
        self.touched();
        Ok(())
    }

    pub fn variant_sets(&self, prim: Prim) -> Result<Vec<(String, Vec<String>)>> {
        let sets = self.usd_prim(prim)?.variant_sets();
        sets.names()
            .map_err(usd_err)?
            .into_iter()
            .map(|name| {
                let variants = sets.get(name.as_str()).names().map_err(usd_err)?;
                Ok((name, variants))
            })
            .collect()
    }

    pub fn add_reference(&mut self, prim: Prim, asset_path: &str, prim_path: &str) -> Result<()> {
        let p = self.usd_prim(prim)?;
        let reference = sdf::Reference {
            asset_path: asset_path.to_owned(),
            prim_path: optional_path(prim_path)?,
            ..Default::default()
        };
        self.immediate("adding a reference", |_| p.references().add(reference))
    }

    pub fn add_payload(&mut self, prim: Prim, asset_path: &str, prim_path: &str) -> Result<()> {
        let p = self.usd_prim(prim)?;
        let payload = sdf::Payload {
            asset_path: asset_path.to_owned(),
            prim_path: optional_path(prim_path)?,
            ..Default::default()
        };
        self.immediate("adding a payload", |_| p.payloads().add(payload))
    }

    pub fn add_inherit(&mut self, prim: Prim, path: &str) -> Result<()> {
        let p = self.usd_prim(prim)?;
        let path = parse_path(path)?;
        self.immediate("adding an inherit", |_| p.inherits().add(path))
    }

    pub fn add_specialize(&mut self, prim: Prim, path: &str) -> Result<()> {
        let p = self.usd_prim(prim)?;
        let path = parse_path(path)?;
        self.immediate("adding a specialize", |_| p.specializes().add(path))
    }

    pub fn set_variant_selection(&mut self, prim: Prim, variant_set: &str, variant: &str) -> Result<()> {
        let p = self.usd_prim(prim)?;
        self.immediate("selecting a variant", |_| {
            p.variant_sets().set_selection(variant_set, variant)
        })
    }

    pub fn create_attribute(&mut self, prim: Prim, name: &str, type_name_str: &str, uniform: bool) -> Result<()> {
        let declared = type_name(type_name_str)?;
        let path = self.sdf_path(prim)?.append_property(name).map_err(usd_err)?;
        if let Some(batch) = &mut self.batch {
            if let Op::Attribute {
                type_name, uniform: u, ..
            } = batch.attribute(&path, declared.clone())
            {
                *type_name = declared;
                *u = uniform;
            }
            return Ok(());
        }
        let p = self.usd_prim(prim)?;
        let variability = if uniform {
            sdf::Variability::Uniform
        } else {
            sdf::Variability::Varying
        };
        self.immediate("creating an attribute", |_| {
            p.attribute_builder(name, declared)
                .variability(variability)
                .build()
                .map(drop)
        })
    }

    pub fn set(&mut self, prim: Prim, name: &str, value: &Value, time: Option<f64>) -> Result<()> {
        let vt_value = value.to_vt()?;
        if self.batch.is_some() {
            let path = self.sdf_path(prim)?.append_property(name).map_err(usd_err)?;
            let declared = self.batch_declared_type(prim, &path, name)?;
            if let Some(batch) = self.batch.as_mut()
                && let Op::Attribute { values, block, .. } = batch.attribute(&path, declared)
            {
                *block = false;
                match values
                    .iter_mut()
                    .rev()
                    .find(|(_, t)| t.map(f64::to_bits) == time.map(f64::to_bits))
                {
                    Some(slot) => slot.0 = vt_value,
                    None => values.push((vt_value, time)),
                }
            }
            return Ok(());
        }
        let attr = self.usd_prim(prim)?.attribute(name);
        if !attr.is_defined().map_err(usd_err)? {
            return Err(self.undeclared(prim, name));
        }
        self.immediate("setting a value", |_| {
            attr.set_at(vt_value, time.map(usd::TimeCode::new)).map(drop)
        })
    }

    fn undeclared(&self, prim: Prim, name: &str) -> Error {
        let path = self.path(prim).unwrap_or("?");
        Error::InvalidValue(format!(
            "attribute '{name}' on {path} is not declared; create it with its type first"
        ))
    }

    fn batch_declared_type(&self, prim: Prim, path: &sdf::Path, name: &str) -> Result<sdf::ValueTypeName> {
        let batch = self.batch.as_ref().expect("batch is open");
        if let Some(&index) = batch.attributes.get(path)
            && let Op::Attribute { type_name, .. } = &batch.ops[index]
        {
            return Ok(type_name.clone());
        }
        if let Some(type_name) = self.usd_prim(prim)?.attribute(name).type_name().map_err(usd_err)? {
            return Ok(type_name);
        }
        let declared = batch.defined.get(&path.prim_path()).and_then(|t| {
            let definition = self.inner.schema_registry().concrete_prim_definition(t)?;
            definition.property(&tf::Token::from(name.to_owned()))?.type_name()
        });
        declared.ok_or_else(|| self.undeclared(prim, name))
    }

    pub fn declared_type(&self, prim: Prim, name: &str) -> Result<Option<String>> {
        if self.batch.is_none() {
            return Ok(self.attribute_info(prim, name)?.map(|info| info.type_name));
        }
        let path = self.sdf_path(prim)?.append_property(name).map_err(usd_err)?;
        match self.batch_declared_type(prim, &path, name) {
            Ok(type_name) => Ok(Some(type_name.to_string())),
            Err(Error::InvalidValue(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn block(&mut self, prim: Prim, name: &str) -> Result<()> {
        if self.batch.is_some() {
            let path = self.sdf_path(prim)?.append_property(name).map_err(usd_err)?;
            let declared = self.batch_declared_type(prim, &path, name)?;
            if let Some(batch) = self.batch.as_mut()
                && let Op::Attribute { values, block, .. } = batch.attribute(&path, declared)
            {
                values.clear();
                *block = true;
            }
            return Ok(());
        }
        let attr = self.usd_prim(prim)?.attribute(name);
        self.immediate("blocking a value", |_| attr.block().map(drop))
    }

    pub fn clear(&mut self, prim: Prim, name: &str) -> Result<()> {
        let attr = self.usd_prim(prim)?.attribute(name);
        self.immediate("clearing a value", |_| attr.clear().map(drop))
    }

    pub fn set_connections(&mut self, prim: Prim, name: &str, targets: &[&str]) -> Result<()> {
        let attr = self.usd_prim(prim)?.attribute(name);
        if !attr.is_defined().map_err(usd_err)? {
            return Err(self.undeclared(prim, name));
        }
        let targets = parse_paths(targets)?;
        self.immediate("setting connections", |_| attr.set_connections(targets).map(drop))
    }

    pub fn set_targets(&mut self, prim: Prim, name: &str, targets: &[&str]) -> Result<()> {
        let targets = parse_paths(targets)?;
        if self.batch.is_some() {
            let path = self.sdf_path(prim)?.append_property(name).map_err(usd_err)?;
            if let Some(batch) = self.batch.as_mut() {
                batch.ops.push(Op::Targets { path, targets });
            }
            return Ok(());
        }
        let p = self.usd_prim(prim)?;
        self.immediate("setting targets", |_| {
            p.relationship_builder(name).set_targets(targets).build().map(drop)
        })
    }

    pub fn add_target(&mut self, prim: Prim, name: &str, target: &str) -> Result<()> {
        let target = parse_path(target)?;
        let p = self.usd_prim(prim)?;
        self.immediate("adding a target", |_| {
            p.create_relationship(name)?.add_target(target).map(drop)
        })
    }

    pub fn set_default_prim(&mut self, prim: Prim) -> Result<()> {
        let name = self.name(prim)?.to_owned();
        self.stage_metadata_edit(|s| s.set_default_prim(name))
    }

    pub fn set_up_axis(&mut self, axis: &str) -> Result<()> {
        let value = vt::Value::Token(tf::Token::from(axis.to_owned()));
        self.stage_metadata_edit(|s| s.set_stage_metadata("upAxis", value))
    }

    pub fn set_meters_per_unit(&mut self, meters: f64) -> Result<()> {
        self.stage_metadata_edit(|s| s.set_stage_metadata("metersPerUnit", vt::Value::Double(meters)))
    }

    pub fn set_kilograms_per_unit(&mut self, kilograms: f64) -> Result<()> {
        self.stage_metadata_edit(|s| s.set_stage_metadata("kilogramsPerUnit", vt::Value::Double(kilograms)))
    }

    pub fn set_time_code_range(&mut self, start: f64, end: f64) -> Result<()> {
        self.stage_metadata_edit(|s| {
            s.set_start_time_code(start)?;
            s.set_end_time_code(end)
        })
    }

    pub fn set_time_codes_per_second(&mut self, rate: f64) -> Result<()> {
        self.stage_metadata_edit(|s| s.set_time_codes_per_second(rate))
    }

    pub fn set_frames_per_second(&mut self, rate: f64) -> Result<()> {
        self.stage_metadata_edit(|s| s.set_frames_per_second(rate))
    }

    pub fn set_documentation(&mut self, text: &str) -> Result<()> {
        let value = vt::Value::String(text.to_owned());
        self.stage_metadata_edit(|s| s.set_stage_metadata("documentation", value))
    }
}

impl Stage {
    fn set_attribute_metadata(&mut self, prim: Prim, name: &str, key: &'static str, value: vt::Value) -> Result<()> {
        if self.batch.is_some() {
            let path = self.sdf_path(prim)?.append_property(name).map_err(usd_err)?;
            let declared = self.batch_declared_type(prim, &path, name)?;
            if let Some(batch) = self.batch.as_mut()
                && let Op::Attribute { metadata, .. } = batch.attribute(&path, declared)
            {
                metadata.retain(|(k, _)| *k != key);
                metadata.push((key, value));
            }
            return Ok(());
        }
        let attr = self.usd_prim(prim)?.attribute(name);
        self.immediate("setting attribute metadata", |_| {
            attr.set_metadata(key, value).map(drop)
        })
    }

    fn ensure_attribute(&mut self, prim: Prim, name: &str, type_name: &str) -> Result<String> {
        match self.declared_type(prim, name)? {
            Some(declared) => Ok(declared),
            None => {
                self.create_attribute(prim, name, type_name, false)?;
                Ok(type_name.to_owned())
            }
        }
    }

    fn set_op_order(&mut self, prim: Prim, ops: &[&str]) -> Result<()> {
        if self.batch.is_none() {
            let mut current = Value::new();
            if self.get(prim, "xformOpOrder", None, &mut current)? && current.strings() == ops {
                return Ok(());
            }
        }
        let order = Value::from_strings(BaseType::Token, true, ops)?;
        self.set(prim, "xformOpOrder", &order, None)
    }

    pub fn set_local_transform(&mut self, prim: Prim, matrix: &[f64; 16], time: Option<f64>) -> Result<()> {
        self.ensure_attribute(prim, "xformOp:transform", "matrix4d")?;
        let value = Value::from_slice(BaseType::Matrix4d, false, &[*matrix])?;
        self.set(prim, "xformOp:transform", &value, time)?;
        self.set_op_order(prim, &["xformOp:transform"])
    }

    pub fn set_world_transform(&mut self, prim: Prim, matrix: &[f64; 16], time: Option<f64>) -> Result<()> {
        let parent = self.parent(prim)?.unwrap_or(Prim::PSEUDO_ROOT);
        let local = if parent == Prim::PSEUDO_ROOT {
            *matrix
        } else {
            let parent_world = gf::Mat4d(self.world_transform(parent, time)?);
            let inverse = parent_world.inverse().ok_or_else(|| {
                Error::InvalidValue(format!(
                    "the parent of {} has a singular transform",
                    self.path(prim).unwrap_or("?")
                ))
            })?;
            (gf::Mat4d(*matrix) * inverse).0
        };
        self.set_local_transform(prim, &local, time)
    }

    pub fn set_trs(
        &mut self,
        prim: Prim,
        translate: [f64; 3],
        orient: [f64; 4],
        scale: [f64; 3],
        time: Option<f64>,
    ) -> Result<()> {
        let vec3 = |declared: &str, v: [f64; 3], what: &str| -> Result<Value> {
            match declared {
                "double3" => Value::from_slice(BaseType::Double3, false, &[v]),
                "float3" => Value::from_slice(BaseType::Float3, false, &[v.map(|x| x as f32)]),
                other => Err(Error::InvalidValue(format!(
                    "xformOp:{what} is declared {other}, not double3 or float3"
                ))),
            }
        };
        let declared = self.ensure_attribute(prim, "xformOp:translate", "double3")?;
        self.set(
            prim,
            "xformOp:translate",
            &vec3(&declared, translate, "translate")?,
            time,
        )?;
        let declared = self.ensure_attribute(prim, "xformOp:orient", "quatd")?;
        let q = match declared.as_str() {
            "quatd" => Value::from_slice(BaseType::Quatd, false, &[orient])?,
            "quatf" => Value::from_slice(BaseType::Quatf, false, &[orient.map(|x| x as f32)])?,
            other => {
                return Err(Error::InvalidValue(format!(
                    "xformOp:orient is declared {other}, not quatd or quatf"
                )));
            }
        };
        self.set(prim, "xformOp:orient", &q, time)?;
        let declared = self.ensure_attribute(prim, "xformOp:scale", "double3")?;
        self.set(prim, "xformOp:scale", &vec3(&declared, scale, "scale")?, time)?;
        self.set_op_order(prim, &["xformOp:translate", "xformOp:orient", "xformOp:scale"])
    }

    pub fn set_mesh(
        &mut self,
        prim: Prim,
        points: &[[f32; 3]],
        face_vertex_counts: &[i32],
        face_vertex_indices: &[i32],
        time: Option<f64>,
    ) -> Result<()> {
        let total: i64 = face_vertex_counts.iter().map(|&c| i64::from(c)).sum();
        if face_vertex_counts.iter().any(|&c| c < 3) || total != face_vertex_indices.len() as i64 {
            return Err(Error::InvalidValue(format!(
                "face vertex counts (each at least 3) sum to {total}, but there are {} indices",
                face_vertex_indices.len()
            )));
        }
        if let Some(bad) = face_vertex_indices
            .iter()
            .find(|&&i| i < 0 || i as usize >= points.len())
        {
            return Err(Error::InvalidValue(format!(
                "face vertex index {bad} is out of range for {} points",
                points.len()
            )));
        }
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for p in points {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        if points.is_empty() {
            lo = [0.0; 3];
            hi = [0.0; 3];
        }
        self.set(
            prim,
            "points",
            &Value::from_slice(BaseType::Float3, true, points)?,
            time,
        )?;
        self.set(
            prim,
            "extent",
            &Value::from_slice(BaseType::Float3, true, &[lo, hi])?,
            time,
        )?;
        let counts = Value::from_slice(BaseType::Int, true, face_vertex_counts)?;
        self.set(prim, "faceVertexCounts", &counts, time)?;
        let indices = Value::from_slice(BaseType::Int, true, face_vertex_indices)?;
        self.set(prim, "faceVertexIndices", &indices, time)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_primvar(
        &mut self,
        prim: Prim,
        name: &str,
        type_name: &str,
        value: &Value,
        interpolation: crate::Interpolation,
        indices: Option<&[i32]>,
        element_size: i32,
        time: Option<f64>,
    ) -> Result<()> {
        let attr = if name.starts_with("primvars:") {
            name.to_owned()
        } else {
            format!("primvars:{name}")
        };
        self.ensure_attribute(prim, &attr, type_name)?;
        self.set(prim, &attr, value, time)?;
        let token = match interpolation {
            crate::Interpolation::Constant => "constant",
            crate::Interpolation::Uniform => "uniform",
            crate::Interpolation::Varying => "varying",
            crate::Interpolation::Vertex => "vertex",
            crate::Interpolation::FaceVarying => "faceVarying",
        };
        self.set_attribute_metadata(prim, &attr, "interpolation", vt::Value::Token(tf::Token::from(token)))?;
        if element_size > 0 {
            self.set_attribute_metadata(prim, &attr, "elementSize", vt::Value::Int(element_size))?;
        }
        if let Some(indices) = indices {
            let index_attr = format!("{attr}:indices");
            self.ensure_attribute(prim, &index_attr, "int[]")?;
            self.set(
                prim,
                &index_attr,
                &Value::from_slice(BaseType::Int, true, indices)?,
                time,
            )?;
        }
        Ok(())
    }

    pub fn bind_material(&mut self, prim: Prim, material: Prim, purpose: &str) -> Result<()> {
        let target = self.path(material)?.to_owned();
        self.apply_api(prim, "MaterialBindingAPI")?;
        let rel = if purpose.is_empty() {
            "material:binding".to_owned()
        } else {
            format!("material:binding:{purpose}")
        };
        self.set_targets(prim, &rel, &[&target])
    }

    pub fn export_package(&self, path: &str) -> Result<()> {
        self.not_batched("export")?;
        if layer_format(path)? != LayerFormat::Usdz {
            return Err(write_error(path, "a package must be written to a .usdz path"));
        }
        let root = self.inner.root_layer().identifier().to_owned();
        if self.host.is_none() {
            openusd::usd_utils::create_usdz_package(&self.inner, root.as_str(), path, None)
                .map_err(|e| write_error(path, e))?;
            return Ok(());
        }
        let dir = std::env::temp_dir().join(format!("tinyusd-package-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| write_error(path, e))?;
        let temp = dir.join("package.usdz");
        let result = openusd::usd_utils::create_usdz_package(&self.inner, root.as_str(), &temp, None)
            .map_err(|e| write_error(path, e))
            .and_then(|_| std::fs::read(&temp).map_err(|e| write_error(path, e)));
        let _ = std::fs::remove_dir_all(&dir);
        self.store(path, &result?)
    }
}

fn optional_path(path: &str) -> Result<sdf::Path> {
    if path.is_empty() {
        Ok(sdf::Path::default())
    } else {
        parse_path(path)
    }
}

fn store(host: Option<&dyn crate::AssetResolver>, path: &str, bytes: &[u8]) -> Result<()> {
    match host {
        Some(host) => host.write(path, bytes).map_err(|e| write_error(path, e)),
        None => write_file(path, bytes),
    }
}

fn write_file(path: &str, bytes: &[u8]) -> Result<()> {
    let target = std::path::Path::new(path);
    let mut temp = target.as_os_str().to_owned();
    temp.push(".tinyusd-tmp");
    let temp = std::path::PathBuf::from(temp);
    std::fs::write(&temp, bytes)
        .and_then(|()| std::fs::rename(&temp, target))
        .map_err(|e| {
            let _ = std::fs::remove_file(&temp);
            write_error(path, e)
        })
}
