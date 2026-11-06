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

type Deferred = Box<dyn FnOnce(&mut Stage) -> Result<()>>;

/// Inside a batch, queue the call to run again when the batch commits, and
/// return `$ret` from the method. The listed `&str` and `&Value` arguments
/// are copied for the queued call.
macro_rules! defer {
    ($self:ident, $ret:expr, [$($owned:ident),*], |$s:ident| $call:expr) => {
        if $self.batch.is_some() {
            $(let $owned = $owned.to_owned();)*
            $self.defer_edit(Box::new(move |$s: &mut Stage| $call.map(drop)));
            return Ok($ret);
        }
    };
}

#[derive(Default)]
pub(crate) struct Batch {
    ops: Vec<Op>,
    /// The queued op later values of an attribute, or targets of a
    /// relationship, merge into. An edit run at commit time ends the run of
    /// ops it can merge across.
    attributes: HashMap<sdf::Path, usize>,
    /// The types of the attributes the batch declares.
    declared: HashMap<sdf::Path, sdf::ValueTypeName>,
    defined: HashMap<sdf::Path, tf::Token>,
    applied: HashMap<sdf::Path, Vec<tf::Token>>,
    deferred: bool,
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
    /// An edit openusd can't queue, run through the stage's own method when
    /// the batch commits, after everything queued before it.
    Deferred(Deferred),
}

/// What a commit that runs deferred edits can fail with, so an openusd
/// error can roll the group back.
enum CommitError {
    Usd(StageAuthoringError),
    Edit(Error),
}

impl From<StageAuthoringError> for CommitError {
    fn from(e: StageAuthoringError) -> Self {
        Self::Usd(e)
    }
}

impl Batch {
    fn attribute(&mut self, path: &sdf::Path, type_name: sdf::ValueTypeName) -> &mut Op {
        self.declared.entry(path.clone()).or_insert_with(|| type_name.clone());
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

    fn defer(&mut self, op: Deferred) {
        self.ops.push(Op::Deferred(op));
        self.attributes.clear();
        self.deferred = true;
    }

    fn commit_queued(ops: Vec<Op>, stage: &usd::Stage) -> std::result::Result<(), StageAuthoringError> {
        if ops.is_empty() {
            return Ok(());
        }
        stage.edit(|edit| {
            for op in ops {
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
                    Op::Deferred(_) => unreachable!("deferred edits run between queued transactions"),
                }
            }
            Ok(())
        })
    }

    /// Writes the batch. Queued edits between deferred ones are written as
    /// one transaction each, and the deferred edits run in order between
    /// them. Where any are deferred, the whole commit is one undo group, so
    /// a failure anywhere reverts what the earlier transactions wrote.
    fn commit(self, stage: &mut Stage) -> Result<()> {
        if !self.deferred {
            return Self::commit_queued(self.ops, &stage.inner).map_err(author_err);
        }
        let undo = usd::UndoStage::from(stage.inner.clone());
        let result = undo.group(|_| -> std::result::Result<(), CommitError> {
            let mut queued = Vec::new();
            for op in self.ops {
                match op {
                    Op::Deferred(edit) => {
                        Self::commit_queued(std::mem::take(&mut queued), &stage.inner)?;
                        edit(stage).map_err(CommitError::Edit)?;
                    }
                    op => queued.push(op),
                }
            }
            Ok(Self::commit_queued(queued, &stage.inner)?)
        });
        drop(undo.into_inner());
        result.map_err(|e| match e {
            CommitError::Usd(e) => author_err(e),
            CommitError::Edit(e) => e,
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

    fn defer_edit(&mut self, edit: Deferred) {
        if let Some(batch) = self.batch.as_mut() {
            batch.defer(edit);
        }
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
                // A layer saved through a host stays dirty, as openusd only
                // marks layers it wrote itself clean, so unchanged bytes are
                // what show there is nothing new to write.
                let digest = digest(&bytes);
                if self.stored.get(&id) == Some(&digest) {
                    continue;
                }
                self.store(&id, &bytes)?;
                self.stored.insert(id, digest);
            } else {
                handle.save().map_err(|e| write_error(&shown, e))?;
            }
        }
        Ok(())
    }

    pub fn export(&self, path: &str) -> Result<()> {
        self.not_batched("export")?;
        let flat = self.flatten()?;
        self.store_layer(&flat, path)
    }

    /// The stage flattened into one layer, with its instancing kept: each
    /// prototype is written once, as a `class` prim named
    /// `Flattened_Prototype_<n>` at the root, and each instance keeps its own
    /// properties and refers to it, as OpenUSD's own flatten does.
    fn flatten(&self) -> Result<sdf::Layer> {
        let mut flat = self.inner.flatten().map_err(usd_err)?;
        let prototypes = self.inner.prototypes().map_err(usd_err)?;
        if prototypes.is_empty() {
            return Ok(flat);
        }
        let mut instances = Vec::with_capacity(prototypes.len());
        for proto in &prototypes {
            let paths = self
                .inner
                .prim(proto.clone())
                .map_err(usd_err)?
                .instances()
                .map_err(usd_err)?;
            instances.push(paths);
        }
        let taken: std::collections::HashSet<String> = match flat
            .data()
            .field(&sdf::Path::abs_root(), "primChildren")
            .ok()
            .map(|v| v.into_owned())
        {
            Some(vt::Value::TokenVec(children)) => children.iter().map(|t| t.as_str().to_owned()).collect(),
            _ => Default::default(),
        };
        let mut next = 1;
        let mut names = Vec::with_capacity(prototypes.len());
        for _ in &prototypes {
            while taken.contains(&format!("Flattened_Prototype_{next}")) {
                next += 1;
            }
            names.push(
                sdf::Path::abs_root()
                    .append_path(format!("Flattened_Prototype_{next}"))
                    .map_err(usd_err)?,
            );
            next += 1;
        }
        flat.edit(|edit| {
            keep_instances(edit.data_mut(), &prototypes, &instances, &names);
            Ok(())
        })
        .map_err(usd_err)?;
        Ok(flat)
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
                let flat = self.flatten()?;
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
        let result = batch.commit(self);
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
        defer!(self, false, [], |s| s.remove_prim(prim));
        let path = self.sdf_path(prim)?.clone();
        self.immediate("removing a prim", |s| s.inner.remove_prim(path))
    }

    pub fn set_type_name(&mut self, prim: Prim, type_name: &str) -> Result<()> {
        if self.batch.is_some() {
            let path = self.sdf_path(prim)?.clone();
            if let Some(batch) = self.batch.as_mut() {
                batch.defined.insert(path, tf::Token::from(type_name.to_owned()));
            }
        }
        defer!(self, (), [type_name], |s| s.set_type_name(prim, &type_name));
        let p = self.usd_prim(prim)?;
        self.immediate("setting a type name", |_| p.set_type_name(type_name).map(drop))
    }

    pub fn set_active(&mut self, prim: Prim, active: bool) -> Result<()> {
        defer!(self, (), [], |s| s.set_active(prim, active));
        let p = self.usd_prim(prim)?;
        self.immediate("setting active", |_| p.set_active(active).map(drop))
    }

    pub fn set_kind(&mut self, prim: Prim, kind: &str) -> Result<()> {
        defer!(self, (), [kind], |s| s.set_kind(prim, &kind));
        let p = self.usd_prim(prim)?;
        self.immediate("setting a kind", |_| p.set_kind(kind).map(drop))
    }

    pub fn set_instanceable(&mut self, prim: Prim, instanceable: bool) -> Result<()> {
        defer!(self, (), [], |s| s.set_instanceable(prim, instanceable));
        let p = self.usd_prim(prim)?;
        self.immediate("setting instanceable", |_| p.set_instanceable(instanceable).map(drop))
    }

    pub fn apply_api(&mut self, prim: Prim, schema: &str) -> Result<()> {
        if self.batch.is_some() {
            let path = self.sdf_path(prim)?.clone();
            if let Some(batch) = self.batch.as_mut() {
                batch
                    .applied
                    .entry(path)
                    .or_default()
                    .push(tf::Token::from(schema.to_owned()));
            }
        }
        defer!(self, (), [schema], |s| s.apply_api(prim, &schema));
        let p = self.usd_prim(prim)?;
        self.immediate("applying an API schema", |_| p.apply_api(schema).map(drop))
    }

    pub fn remove_api(&mut self, prim: Prim, schema: &str) -> Result<()> {
        defer!(self, (), [schema], |s| s.remove_api(prim, &schema));
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
        self.add_reference_with_offset(prim, asset_path, prim_path, 0.0, 1.0)
    }

    pub fn add_payload(&mut self, prim: Prim, asset_path: &str, prim_path: &str) -> Result<()> {
        self.add_payload_with_offset(prim, asset_path, prim_path, 0.0, 1.0)
    }

    pub fn add_inherit(&mut self, prim: Prim, path: &str) -> Result<()> {
        defer!(self, (), [path], |s| s.add_inherit(prim, &path));
        let p = self.usd_prim(prim)?;
        let path = parse_path(path)?;
        self.immediate("adding an inherit", |_| p.inherits().add(path))
    }

    pub fn add_specialize(&mut self, prim: Prim, path: &str) -> Result<()> {
        defer!(self, (), [path], |s| s.add_specialize(prim, &path));
        let p = self.usd_prim(prim)?;
        let path = parse_path(path)?;
        self.immediate("adding a specialize", |_| p.specializes().add(path))
    }

    pub fn set_variant_selection(&mut self, prim: Prim, variant_set: &str, variant: &str) -> Result<()> {
        defer!(self, (), [variant_set, variant], |s| s.set_variant_selection(
            prim,
            &variant_set,
            &variant
        ));
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
                *type_name = declared.clone();
                *u = uniform;
            }
            batch.declared.insert(path, declared);
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
        if let Some(type_name) = batch.declared.get(path) {
            return Ok(type_name.clone());
        }
        if let Some(type_name) = self.usd_prim(prim)?.attribute(name).type_name().map_err(usd_err)? {
            return Ok(type_name);
        }
        // What the prim's schemas declare once the batch's own definitions
        // and applied schemas are in.
        let prim_path = path.prim_path();
        let queued_type = batch.defined.get(&prim_path);
        let queued_apis = batch.applied.get(&prim_path);
        let declared = if queued_type.is_some() || queued_apis.is_some() {
            let type_name = match queued_type {
                Some(t) => t.clone(),
                None => tf::Token::from(self.type_name(prim)?),
            };
            let mut applied: Vec<tf::Token> = self.applied_schemas(prim)?.into_iter().map(tf::Token::from).collect();
            applied.extend(queued_apis.into_iter().flatten().cloned());
            let definition = self
                .inner
                .schema_registry()
                .build_composed_prim_definition(&type_name, &applied);
            definition
                .property(&tf::Token::from(name.to_owned()))
                .and_then(|p| p.type_name())
        } else {
            None
        };
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
        defer!(self, (), [name], |s| s.clear(prim, &name));
        let attr = self.usd_prim(prim)?.attribute(name);
        self.immediate("clearing a value", |_| attr.clear().map(drop))
    }

    pub fn set_connections(&mut self, prim: Prim, name: &str, targets: &[&str]) -> Result<()> {
        if self.batch.is_some() {
            parse_paths(targets)?;
            let name = name.to_owned();
            let targets: Vec<String> = targets.iter().map(|t| (*t).to_owned()).collect();
            self.defer_edit(Box::new(move |s: &mut Stage| {
                let targets: Vec<&str> = targets.iter().map(String::as_str).collect();
                s.set_connections(prim, &name, &targets)
            }));
            return Ok(());
        }
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
                match batch.attributes.get(&path).map(|&i| &mut batch.ops[i]) {
                    Some(Op::Targets { targets: queued, .. }) => *queued = targets,
                    _ => {
                        batch.ops.push(Op::Targets {
                            path: path.clone(),
                            targets,
                        });
                        batch.attributes.insert(path, batch.ops.len() - 1);
                    }
                }
            }
            return Ok(());
        }
        let p = self.usd_prim(prim)?;
        self.immediate("setting targets", |_| {
            p.relationship_builder(name).set_targets(targets).build().map(drop)
        })
    }

    pub fn add_target(&mut self, prim: Prim, name: &str, target: &str) -> Result<()> {
        defer!(self, (), [name, target], |s| s.add_target(prim, &name, &target));
        let target = parse_path(target)?;
        let p = self.usd_prim(prim)?;
        self.immediate("adding a target", |_| {
            p.create_relationship(name)?.add_target(target).map(drop)
        })
    }

    pub fn set_default_prim(&mut self, prim: Prim) -> Result<()> {
        defer!(self, (), [], |s| s.set_default_prim(prim));
        let name = self.name(prim)?.to_owned();
        self.stage_metadata_edit(|s| s.set_default_prim(name))
    }

    pub fn set_up_axis(&mut self, axis: &str) -> Result<()> {
        defer!(self, (), [axis], |s| s.set_up_axis(&axis));
        let value = vt::Value::Token(tf::Token::from(axis.to_owned()));
        self.stage_metadata_edit(|s| s.set_stage_metadata("upAxis", value))
    }

    pub fn set_meters_per_unit(&mut self, meters: f64) -> Result<()> {
        defer!(self, (), [], |s| s.set_meters_per_unit(meters));
        self.stage_metadata_edit(|s| s.set_stage_metadata("metersPerUnit", vt::Value::Double(meters)))
    }

    pub fn set_kilograms_per_unit(&mut self, kilograms: f64) -> Result<()> {
        defer!(self, (), [], |s| s.set_kilograms_per_unit(kilograms));
        self.stage_metadata_edit(|s| s.set_stage_metadata("kilogramsPerUnit", vt::Value::Double(kilograms)))
    }

    pub fn set_time_code_range(&mut self, start: f64, end: f64) -> Result<()> {
        defer!(self, (), [], |s| s.set_time_code_range(start, end));
        self.stage_metadata_edit(|s| {
            s.set_start_time_code(start)?;
            s.set_end_time_code(end)
        })
    }

    pub fn set_time_codes_per_second(&mut self, rate: f64) -> Result<()> {
        defer!(self, (), [], |s| s.set_time_codes_per_second(rate));
        self.stage_metadata_edit(|s| s.set_time_codes_per_second(rate))
    }

    pub fn set_frames_per_second(&mut self, rate: f64) -> Result<()> {
        defer!(self, (), [], |s| s.set_frames_per_second(rate));
        self.stage_metadata_edit(|s| s.set_frames_per_second(rate))
    }

    pub fn set_documentation(&mut self, text: &str) -> Result<()> {
        defer!(self, (), [text], |s| s.set_documentation(&text));
        let value = vt::Value::String(text.to_owned());
        self.stage_metadata_edit(|s| s.set_stage_metadata("documentation", value))
    }
}

impl Stage {
    pub(crate) fn set_attribute_metadata(
        &mut self,
        prim: Prim,
        name: &str,
        key: &'static str,
        value: vt::Value,
    ) -> Result<()> {
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

    fn local_from_world(&mut self, prim: Prim, matrix: &[f64; 16], time: Option<f64>) -> Result<[f64; 16]> {
        let parent = self.parent(prim)?.unwrap_or(Prim::PSEUDO_ROOT);
        if parent == Prim::PSEUDO_ROOT {
            return Ok(*matrix);
        }
        let parent_world = gf::Mat4d(self.world_transform(parent, time)?);
        let inverse = parent_world.inverse().ok_or_else(|| {
            Error::InvalidValue(format!(
                "the parent of {} has a singular transform",
                self.path(prim).unwrap_or("?")
            ))
        })?;
        Ok((gf::Mat4d(*matrix) * inverse).0)
    }

    pub fn set_world_transform(&mut self, prim: Prim, matrix: &[f64; 16], time: Option<f64>) -> Result<()> {
        let local = self.local_from_world(prim, matrix, time)?;
        self.set_local_transform(prim, &local, time)
    }

    /// Places `prim` at the world `matrix` through the transform ops it
    /// already has where they can hold the pose exactly: a translate, an
    /// `orient` or `rotateXYZ` and a scale, in that order, each optional,
    /// keep their precision and get new values. A prim with no ops gets
    /// translate, orient and scale. Any other stack, or a pose with shear,
    /// is written as one `xformOp:transform`.
    #[cfg_attr(not(feature = "physics"), allow(dead_code))]
    pub(crate) fn set_world_pose(&mut self, prim: Prim, matrix: &[f64; 16], time: Option<f64>) -> Result<()> {
        let local = self.local_from_world(prim, matrix, time)?;
        let mut order = Value::new();
        let ops: Vec<String> = if self.get(prim, "xformOpOrder", None, &mut order)? {
            order.strings().to_vec()
        } else {
            Vec::new()
        };
        let Some(pose) = Pose::from_matrix(&local) else {
            return self.set_local_transform(prim, &local, time);
        };
        let plan = match pose_plan(&ops, &pose) {
            Some(plan) => plan,
            None if ops.len() == 1 && ops[0] == "xformOp:transform" => {
                return self.set_local_transform(prim, &local, time);
            }
            None if ops.is_empty() || is_trs_stack(&ops) => {
                let rotation = if ops.iter().any(|o| o == "xformOp:rotateXYZ") && pose.euler_xyz().is_some() {
                    "xformOp:rotateXYZ"
                } else {
                    "xformOp:orient"
                };
                vec!["xformOp:translate", rotation, "xformOp:scale"]
            }
            None => return self.set_local_transform(prim, &local, time),
        };
        let mut written = Vec::with_capacity(plan.len());
        for op in &plan {
            let ok = match *op {
                "xformOp:translate" => self.set_vec3_op(prim, op, pose.translate, time)?,
                "xformOp:scale" => self.set_vec3_op(prim, op, pose.scale, time)?,
                "xformOp:rotateXYZ" => match pose.euler_xyz() {
                    Some(angles) => self.set_vec3_op(prim, op, angles, time)?,
                    None => false,
                },
                _ => self.set_quat_op(prim, op, pose.orient, time)?,
            };
            if !ok {
                return self.set_local_transform(prim, &local, time);
            }
            written.push(*op);
        }
        self.set_op_order(prim, &written)
    }

    /// Writes a three-component op in the type it's declared with, `double3`
    /// where it's new. Returns false for a type it can't hold.
    #[cfg_attr(not(feature = "physics"), allow(dead_code))]
    fn set_vec3_op(&mut self, prim: Prim, name: &str, v: [f64; 3], time: Option<f64>) -> Result<bool> {
        let value = match self.declared_type(prim, name)?.as_deref() {
            None => {
                self.create_attribute(prim, name, "double3", false)?;
                Value::from_slice(BaseType::Double3, false, &[v])?
            }
            Some("double3") => Value::from_slice(BaseType::Double3, false, &[v])?,
            Some("float3") => Value::from_slice(BaseType::Float3, false, &[v.map(|x| x as f32)])?,
            Some(_) => return Ok(false),
        };
        self.set(prim, name, &value, time)?;
        Ok(true)
    }

    #[cfg_attr(not(feature = "physics"), allow(dead_code))]
    fn set_quat_op(&mut self, prim: Prim, name: &str, q: [f64; 4], time: Option<f64>) -> Result<bool> {
        let value = match self.declared_type(prim, name)?.as_deref() {
            None => {
                self.create_attribute(prim, name, "quatd", false)?;
                Value::from_slice(BaseType::Quatd, false, &[q])?
            }
            Some("quatd") => Value::from_slice(BaseType::Quatd, false, &[q])?,
            Some("quatf") => Value::from_slice(BaseType::Quatf, false, &[q.map(|x| x as f32)])?,
            Some(_) => return Ok(false),
        };
        self.set(prim, name, &value, time)?;
        Ok(true)
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

/// A transform as translate, rotation (`w, x, y, z`) and scale, which compose
/// as scale, then rotation, then translation.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Pose {
    pub translate: [f64; 3],
    pub orient: [f64; 4],
    pub scale: [f64; 3],
}

const POSE_EPSILON: f64 = 1e-9;

fn rotation_rows(q: [f64; 4]) -> [[f64; 3]; 3] {
    let [w, x, y, z] = q;
    [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y + w * z),
            2.0 * (x * z - w * y),
        ],
        [
            2.0 * (x * y - w * z),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z + w * x),
        ],
        [
            2.0 * (x * z + w * y),
            2.0 * (y * z - w * x),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ]
}

#[cfg_attr(not(feature = "physics"), allow(dead_code))]
fn euler_xyz_rows(degrees: [f64; 3]) -> [[f64; 3]; 3] {
    let [a, b, g] = degrees.map(f64::to_radians);
    let (sa, ca, sb, cb, sg, cg) = (a.sin(), a.cos(), b.sin(), b.cos(), g.sin(), g.cos());
    [
        [cb * cg, cb * sg, -sb],
        [sa * sb * cg - ca * sg, sa * sb * sg + ca * cg, sa * cb],
        [ca * sb * cg + sa * sg, ca * sb * sg - sa * cg, ca * cb],
    ]
}

#[cfg_attr(not(feature = "physics"), allow(dead_code))]
fn rows_close(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3], tolerance: f64) -> bool {
    a.iter()
        .flatten()
        .zip(b.iter().flatten())
        .all(|(x, y)| (x - y).abs() <= tolerance)
}

impl Pose {
    pub(crate) fn matrix(&self) -> [f64; 16] {
        let rows = rotation_rows(self.orient);
        let mut m = [0.0; 16];
        for (i, row) in rows.iter().enumerate() {
            for (k, v) in row.iter().enumerate() {
                m[i * 4 + k] = v * self.scale[i];
            }
        }
        m[12..15].copy_from_slice(&self.translate);
        m[15] = 1.0;
        m
    }

    /// The pose `m` holds, or `None` where it has shear or a zero scale,
    /// which translate, rotate and scale can't express.
    pub(crate) fn from_matrix(m: &[f64; 16]) -> Option<Self> {
        let mut rows = [[0.0; 3]; 3];
        let mut scale = [0.0; 3];
        for i in 0..3 {
            let row = [m[i * 4], m[i * 4 + 1], m[i * 4 + 2]];
            scale[i] = row.iter().map(|v| v * v).sum::<f64>().sqrt();
            if scale[i] <= POSE_EPSILON {
                return None;
            }
            rows[i] = row.map(|v| v / scale[i]);
        }
        let det = gf::Mat3d([
            rows[0][0], rows[0][1], rows[0][2], rows[1][0], rows[1][1], rows[1][2], rows[2][0], rows[2][1], rows[2][2],
        ])
        .determinant();
        if det < 0.0 {
            scale[0] = -scale[0];
            rows[0] = rows[0].map(|v| -v);
        }
        let mut rot = [0.0; 16];
        for i in 0..3 {
            rot[i * 4..i * 4 + 3].copy_from_slice(&rows[i]);
        }
        rot[15] = 1.0;
        let q = gf::Mat4d(rot).extract_rotation_quat();
        let len = (q.w * q.w + q.x * q.x + q.y * q.y + q.z * q.z).sqrt();
        let pose = Self {
            translate: [m[12], m[13], m[14]],
            orient: [q.w / len, q.x / len, q.y / len, q.z / len],
            scale,
        };
        let back = pose.matrix();
        let size = m.iter().fold(1.0f64, |a, v| a.max(v.abs()));
        back.iter()
            .zip(m)
            .all(|(a, b)| (a - b).abs() <= 1e-9 * size)
            .then_some(pose)
    }

    /// The rotation as `rotateXYZ` angles in degrees, or `None` where they
    /// don't reproduce it, as at gimbal lock.
    #[cfg_attr(not(feature = "physics"), allow(dead_code))]
    pub(crate) fn euler_xyz(&self) -> Option<[f64; 3]> {
        let r = rotation_rows(self.orient);
        let b = (-r[0][2]).clamp(-1.0, 1.0).asin();
        let g = r[0][1].atan2(r[0][0]);
        let a = r[1][2].atan2(r[2][2]);
        let angles = [a, b, g].map(f64::to_degrees);
        rows_close(&euler_xyz_rows(angles), &r, 1e-9).then_some(angles)
    }

    #[cfg_attr(not(feature = "physics"), allow(dead_code))]
    fn is_identity_rotation(&self) -> bool {
        rows_close(&rotation_rows(self.orient), &rotation_rows([1.0, 0.0, 0.0, 0.0]), 1e-12)
    }
}

#[cfg_attr(not(feature = "physics"), allow(dead_code))]
const TRS_SLOTS: [&[&str]; 3] = [
    &["xformOp:translate"],
    &["xformOp:orient", "xformOp:rotateXYZ"],
    &["xformOp:scale"],
];

/// Whether `ops` is a translate, one rotation and a scale, each optional, in
/// that order.
#[cfg_attr(not(feature = "physics"), allow(dead_code))]
fn is_trs_stack(ops: &[String]) -> bool {
    let mut slot = 0;
    for op in ops {
        let Some(found) = (slot..3).find(|&i| TRS_SLOTS[i].contains(&op.as_str())) else {
            return false;
        };
        slot = found + 1;
    }
    true
}

/// The ops of `ops` to write `pose` into, when the stack holds it without
/// adding any: the ops it lacks must be identity in the pose.
#[cfg_attr(not(feature = "physics"), allow(dead_code))]
fn pose_plan<'a>(ops: &'a [String], pose: &Pose) -> Option<Vec<&'a str>> {
    if ops.is_empty() || !is_trs_stack(ops) {
        return None;
    }
    let has = |slot: usize| ops.iter().any(|o| TRS_SLOTS[slot].contains(&o.as_str()));
    let tiny = |v: [f64; 3], base: f64| v.iter().all(|x| (x - base).abs() <= 1e-12);
    if !has(0) && !tiny(pose.translate, 0.0) {
        return None;
    }
    if !has(1) && !pose.is_identity_rotation() {
        return None;
    }
    if !has(2) && !tiny(pose.scale, 1.0) {
        return None;
    }
    if ops.iter().any(|o| o == "xformOp:rotateXYZ") && pose.euler_xyz().is_none() {
        return None;
    }
    Some(ops.iter().map(String::as_str).collect())
}

/// Where a stage path stands in the flattened layer: a path inside a
/// prototype moves to that prototype's copy.
fn flattened_location(path: &sdf::Path, prototypes: &[sdf::Path], copies: &[sdf::Path]) -> sdf::Path {
    prototypes
        .iter()
        .zip(copies)
        .find_map(|(proto, copy)| path.replace_prefix(proto, copy))
        .unwrap_or_else(|| path.clone())
}

fn remap_value(value: vt::Value, from: &sdf::Path, to: &sdf::Path) -> vt::Value {
    let path = |p: &mut sdf::Path| {
        if let Some(moved) = p.replace_prefix(from, to) {
            *p = moved;
        }
    };
    match value {
        vt::Value::PathListOp(mut op) => {
            op.update(path);
            vt::Value::PathListOp(op)
        }
        vt::Value::PathVec(mut paths) => {
            paths.iter_mut().for_each(path);
            vt::Value::PathVec(paths)
        }
        vt::Value::ReferenceListOp(mut op) => {
            op.update(|r| {
                if r.asset_path.is_empty() {
                    path(&mut r.prim_path);
                }
            });
            vt::Value::ReferenceListOp(op)
        }
        vt::Value::PayloadListOp(mut op) => {
            op.update(|r| {
                if r.asset_path.is_empty() {
                    path(&mut r.prim_path);
                }
            });
            vt::Value::PayloadListOp(op)
        }
        other => other,
    }
}

/// Turns the expanded instances of a flattened layer back into instances.
/// Each prototype's contents are copied from one of its instances to its
/// copy path, in an order that has every nested instance's contents written
/// before they're needed, and then each instance loses its descendants and
/// gains `instanceable` and an internal reference to the copy.
fn keep_instances(
    data: &mut dyn sdf::AbstractData,
    prototypes: &[sdf::Path],
    instances: &[Vec<sdf::Path>],
    copies: &[sdf::Path],
) {
    let mut copied = vec![false; prototypes.len()];
    let mut progress = true;
    while progress {
        progress = false;
        for i in 0..prototypes.len() {
            if copied[i] {
                continue;
            }
            let source = instances[i]
                .iter()
                .map(|p| flattened_location(p, prototypes, copies))
                .find(|p| data.spec_type(p).is_some());
            let Some(source) = source else { continue };
            let copy = &copies[i];
            let subtree: Vec<sdf::Path> = data
                .spec_paths()
                .into_iter()
                .filter(|p| *p == source || p.has_prefix(&source))
                .collect();
            for from in subtree {
                let Some(to) = from.replace_prefix(&source, copy) else {
                    continue;
                };
                let Some(kind) = data.spec_type(&from) else { continue };
                let fields = data.list_fields(&from).unwrap_or_default();
                let values: Vec<(String, vt::Value)> = fields
                    .into_iter()
                    .filter_map(|f| data.field(&from, &f).ok().map(|v| (f, v.into_owned())))
                    .collect();
                data.create_spec(to.clone(), kind);
                for (field, value) in values {
                    data.set_field(&to, &field, remap_value(value, &source, copy));
                }
            }
            for field in ["instanceable", "references", "payload", "inheritPaths", "specializes"] {
                data.erase_field(copy, field);
            }
            data.set_field(copy, "specifier", vt::Value::Specifier(sdf::Specifier::Class));
            let root = sdf::Path::abs_root();
            let mut children = match data.field(&root, "primChildren").ok().map(|v| v.into_owned()) {
                Some(vt::Value::TokenVec(children)) => children,
                _ => Vec::new(),
            };
            children.push(tf::Token::from(copy.name().unwrap_or_default().to_owned()));
            data.set_field(&root, "primChildren", vt::Value::TokenVec(children));
            copied[i] = true;
            progress = true;
        }
    }
    for (i, paths) in instances.iter().enumerate() {
        if !copied[i] {
            continue;
        }
        for instance in paths {
            let at = flattened_location(instance, prototypes, copies);
            if data.spec_type(&at).is_none() {
                continue;
            }
            for p in data.spec_paths() {
                let own_property = p.is_property() && p.prim_path() == at;
                if p != at && p.has_prefix(&at) && !own_property {
                    data.erase_spec(&p);
                }
            }
            data.erase_field(&at, "primChildren");
            data.set_field(&at, "instanceable", vt::Value::Bool(true));
            let reference = sdf::Reference {
                prim_path: copies[i].clone(),
                ..Default::default()
            };
            data.set_field(
                &at,
                "references",
                vt::Value::ReferenceListOp(sdf::ReferenceListOp::prepended([reference])),
            );
        }
    }
}

fn digest(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
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

const DICTIONARY_FIELDS: [&str; 2] = ["customData", "assetInfo"];

const PRIM_FIELDS: [(&str, &str); 7] = [
    ("active", "bool"),
    ("comment", "string"),
    ("displayName", "string"),
    ("documentation", "string"),
    ("hidden", "bool"),
    ("instanceable", "bool"),
    ("kind", "token"),
];

const PROPERTY_FIELDS: [(&str, &str); 8] = [
    ("colorSpace", "token"),
    ("comment", "string"),
    ("displayGroup", "string"),
    ("displayName", "string"),
    ("documentation", "string"),
    ("elementSize", "int"),
    ("hidden", "bool"),
    ("interpolation", "token"),
];

struct MetadataField<'a> {
    key: &'static str,
    entry: Option<&'a str>,
    type_name: Option<&'static str>,
}

fn metadata_field(key: &str, property: bool) -> Result<MetadataField<'_>> {
    let (field, entry) = match key.split_once(':') {
        Some((field, entry)) => (field, Some(entry)),
        None => (key, None),
    };
    if let Some(&dict) = DICTIONARY_FIELDS.iter().find(|&&d| d == field) {
        if entry.is_some_and(|e| e.is_empty() || e.split(':').any(str::is_empty)) {
            return Err(Error::InvalidValue(format!("'{key}' has an empty dictionary key")));
        }
        return Ok(MetadataField {
            key: dict,
            entry,
            type_name: None,
        });
    }
    let fields: &[(&'static str, &'static str)] = if property { &PROPERTY_FIELDS } else { &PRIM_FIELDS };
    match fields.iter().find(|(k, _)| *k == key) {
        Some(&(key, type_name)) => Ok(MetadataField {
            key,
            entry: None,
            type_name: Some(type_name),
        }),
        None => {
            let names: Vec<&str> = fields.iter().map(|(k, _)| *k).chain(DICTIONARY_FIELDS).collect();
            Err(Error::InvalidValue(format!(
                "'{key}' is not writable {} metadata; the writable keys are {} (dictionary entries as customData:name)",
                if property { "property" } else { "prim" },
                names.join(", ")
            )))
        }
    }
}

impl MetadataField<'_> {
    fn value(&self, value: &Value) -> Result<vt::Value> {
        match self.type_name {
            None if self.entry.is_none() => Err(Error::InvalidValue(format!(
                "set one entry of {} at a time, as {}:name",
                self.key, self.key
            ))),
            None if value.is_empty() => Err(Error::InvalidValue(format!("{} entries need a value", self.key))),
            None => value.to_vt(),
            Some(type_name) => {
                let expected = BaseType::from_type_name(type_name).map(|(base, _)| base);
                if value.is_array() || Some(value.base_type()) != expected {
                    return Err(Error::InvalidValue(format!(
                        "{} takes a {type_name}, not {}",
                        self.key,
                        if value.is_empty() {
                            "an empty value".to_owned()
                        } else {
                            format!("a {}", value.type_name())
                        }
                    )));
                }
                value.to_vt()
            }
        }
    }
}

fn with_entry(local: Option<vt::Value>, entry: &str, value: Option<vt::Value>) -> Option<vt::Value> {
    let mut dict = match local {
        Some(vt::Value::Dictionary(d) | vt::Value::UnregisteredDictionary(d)) => d,
        _ => vt::Dictionary::new(),
    };
    match value {
        Some(value) => dict.set_at(entry, value),
        None => {
            dict.erase_at(entry);
        }
    }
    (!dict.is_empty()).then_some(vt::Value::Dictionary(dict))
}

enum Property {
    Attribute(usd::Attribute),
    Relationship(usd::Relationship),
}

impl Stage {
    /// The type a writable metadata key takes, such as `bool` for `hidden`, or
    /// `None` for a dictionary entry (`customData:name`), which takes any
    /// value, and for a key that can't be written.
    pub fn metadata_type(key: &str, property: bool) -> Option<&'static str> {
        metadata_field(key, property).ok().and_then(|f| f.type_name)
    }

    fn property(&self, prim: Prim, name: &str) -> Result<Property> {
        let p = self.usd_prim(prim)?;
        let attr = p.attribute(name);
        if attr.is_defined().map_err(usd_err)? {
            return Ok(Property::Attribute(attr));
        }
        let rel = p.relationship(name);
        if rel.is_defined().map_err(usd_err)? {
            return Ok(Property::Relationship(rel));
        }
        let path = self.path(prim).unwrap_or("?");
        Err(Error::InvalidValue(format!("{path} has no property '{name}'")))
    }

    pub fn set_metadata(&mut self, prim: Prim, key: &str, value: &Value) -> Result<()> {
        let field = metadata_field(key, false)?;
        let checked = field.value(value)?;
        defer!(self, (), [key, value], |s| s.set_metadata(prim, &key, &value));
        let value = checked;
        let p = self.usd_prim(prim)?;
        self.immediate("setting prim metadata", |_| match field.entry {
            None => p.set_metadata(field.key, value).map(drop),
            Some(entry) => p
                .update_metadata(field.key, |local| with_entry(local, entry, Some(value)))
                .map(drop),
        })
    }

    pub fn clear_metadata(&mut self, prim: Prim, key: &str) -> Result<()> {
        let field = metadata_field(key, false)?;
        defer!(self, (), [key], |s| s.clear_metadata(prim, &key));
        let p = self.usd_prim(prim)?;
        self.immediate("clearing prim metadata", |_| match field.entry {
            None => p.clear_metadata(field.key).map(drop),
            Some(entry) => p
                .update_metadata(field.key, |local| with_entry(local, entry, None))
                .map(drop),
        })
    }

    pub fn set_property_metadata(&mut self, prim: Prim, name: &str, key: &str, value: &Value) -> Result<()> {
        let field = metadata_field(key, true)?;
        let value_in = value;
        let value = field.value(value)?;
        if self.batch.is_some() && field.entry.is_none() {
            return self.set_attribute_metadata(prim, name, field.key, value);
        }
        if self.batch.is_some() {
            let (name, key, value) = (name.to_owned(), key.to_owned(), value_in.to_owned());
            self.defer_edit(Box::new(move |s: &mut Stage| {
                s.set_property_metadata(prim, &name, &key, &value)
            }));
            return Ok(());
        }
        let property = self.property(prim, name)?;
        self.immediate("setting property metadata", |_| match (property, field.entry) {
            (Property::Attribute(a), None) => a.set_metadata(field.key, value).map(drop),
            (Property::Attribute(a), Some(entry)) => a
                .update_metadata(field.key, |local| with_entry(local, entry, Some(value)))
                .map(drop),
            (Property::Relationship(r), None) => r.set_metadata(field.key, value).map(drop),
            (Property::Relationship(r), Some(entry)) => r
                .update_metadata(field.key, |local| with_entry(local, entry, Some(value)))
                .map(drop),
        })
    }

    pub fn clear_property_metadata(&mut self, prim: Prim, name: &str, key: &str) -> Result<()> {
        metadata_field(key, true)?;
        defer!(self, (), [name, key], |s| s.clear_property_metadata(prim, &name, &key));
        let field = metadata_field(key, true)?;
        let property = self.property(prim, name)?;
        self.immediate("clearing property metadata", |_| match (property, field.entry) {
            (Property::Attribute(a), None) => a.clear_metadata(field.key).map(drop),
            (Property::Attribute(a), Some(entry)) => a
                .update_metadata(field.key, |local| with_entry(local, entry, None))
                .map(drop),
            (Property::Relationship(r), None) => r.clear_metadata(field.key).map(drop),
            (Property::Relationship(r), Some(entry)) => r
                .update_metadata(field.key, |local| with_entry(local, entry, None))
                .map(drop),
        })
    }

    pub fn remove_property(&mut self, prim: Prim, name: &str) -> Result<bool> {
        defer!(self, false, [name], |s| s.remove_property(prim, &name));
        let path = self.sdf_path(prim)?.append_property(name).map_err(usd_err)?;
        self.immediate("removing a property", |s| s.inner.remove_property(path))
    }

    pub fn remove_target(&mut self, prim: Prim, name: &str, target: &str) -> Result<bool> {
        defer!(self, false, [name, target], |s| s.remove_target(prim, &name, &target));
        let target = parse_path(target)?;
        let rel = self.usd_prim(prim)?.relationship(name);
        self.immediate("removing a target", |_| rel.remove_target(target))
    }

    pub fn remove_connection(&mut self, prim: Prim, name: &str, source: &str) -> Result<bool> {
        defer!(self, false, [name, source], |s| s
            .remove_connection(prim, &name, &source));
        let source = parse_path(source)?;
        let attr = self.usd_prim(prim)?.attribute(name);
        self.immediate("removing a connection", |_| attr.remove_connection(source))
    }

    /// Every arc the root layer stack authors on `prim` in `field` that
    /// `matches` picks, whatever its layer offset.
    fn authored_arcs<T>(&self, prim: Prim, field: sdf::FieldKey, matches: impl Fn(&T) -> bool) -> Result<Vec<T>>
    where
        T: Clone + PartialEq + std::hash::Hash,
        sdf::ListOp<T>: TryFrom<vt::Value>,
    {
        let path = self.sdf_path(prim)?;
        let mut found: Vec<T> = Vec::new();
        for id in self.inner.layer_identifiers() {
            if self.inner.edit_target_for_local_layer(&id).is_none() {
                continue;
            }
            let Some(layer) = self.inner.layer(&id) else {
                continue;
            };
            let Some(op) = layer
                .prim(path.clone())
                .ok()
                .flatten()
                .and_then(|spec| spec.get::<sdf::ListOp<T>>(field.as_str()))
            else {
                continue;
            };
            for item in op.iter() {
                if matches(item) && !found.contains(item) {
                    found.push(item.clone());
                }
            }
        }
        Ok(found)
    }

    pub fn remove_reference(&mut self, prim: Prim, asset_path: &str, prim_path: &str) -> Result<()> {
        defer!(self, (), [asset_path, prim_path], |s| s.remove_reference(
            prim,
            &asset_path,
            &prim_path
        ));
        let target = optional_path(prim_path)?;
        let mut items = self.authored_arcs(prim, sdf::FieldKey::References, |r: &sdf::Reference| {
            r.asset_path == asset_path && r.prim_path == target
        })?;
        if items.is_empty() {
            items.push(sdf::Reference {
                asset_path: asset_path.to_owned(),
                prim_path: target,
                ..Default::default()
            });
        }
        let p = self.usd_prim(prim)?;
        self.immediate("removing a reference", |_| {
            items.into_iter().try_for_each(|r| p.references().remove(r))
        })
    }

    pub fn remove_payload(&mut self, prim: Prim, asset_path: &str, prim_path: &str) -> Result<()> {
        defer!(self, (), [asset_path, prim_path], |s| s.remove_payload(
            prim,
            &asset_path,
            &prim_path
        ));
        let target = optional_path(prim_path)?;
        let mut items = self.authored_arcs(prim, sdf::FieldKey::Payload, |r: &sdf::Payload| {
            r.asset_path == asset_path && r.prim_path == target
        })?;
        if items.is_empty() {
            items.push(sdf::Payload {
                asset_path: asset_path.to_owned(),
                prim_path: target,
                ..Default::default()
            });
        }
        let p = self.usd_prim(prim)?;
        self.immediate("removing a payload", |_| {
            items.into_iter().try_for_each(|r| p.payloads().remove(r))
        })
    }

    pub fn remove_inherit(&mut self, prim: Prim, path: &str) -> Result<()> {
        defer!(self, (), [path], |s| s.remove_inherit(prim, &path));
        let p = self.usd_prim(prim)?;
        let path = parse_path(path)?;
        self.immediate("removing an inherit", |_| p.inherits().remove(path))
    }

    pub fn remove_specialize(&mut self, prim: Prim, path: &str) -> Result<()> {
        defer!(self, (), [path], |s| s.remove_specialize(prim, &path));
        let p = self.usd_prim(prim)?;
        let path = parse_path(path)?;
        self.immediate("removing a specialize", |_| p.specializes().remove(path))
    }

    /// Deletes one variant, with everything authored inside it, from the edit
    /// target's layer, and that layer's selection of it. A weaker layer that
    /// also defines it keeps its copy.
    pub fn remove_variant(&mut self, prim: Prim, variant_set: &str, variant: &str) -> Result<bool> {
        defer!(self, false, [variant_set, variant], |s| s.remove_variant(
            prim,
            &variant_set,
            &variant
        ));
        let path = self.sdf_path(prim)?.clone();
        let target = self.inner.edit_target();
        let layer = target.layer_identifier().to_owned();
        let spec_path = target
            .map_to_spec_path(&path)
            .ok_or_else(|| Error::Usd(format!("{path} is outside the edit target")))?;
        let bad = |e: sdf::PathParseError| Error::InvalidPath(e.to_string());
        let set_path = spec_path.append_variant_selection(variant_set, "").map_err(bad)?;
        let variant_path = spec_path.append_variant_selection(variant_set, variant).map_err(bad)?;
        let mut removed = false;
        self.inner
            .edit_layer(&layer, |edit| {
                let data = edit.data_mut();
                if data.spec_type(&variant_path) != Some(sdf::SpecType::Variant) {
                    return Ok(());
                }
                for descendant in data.spec_paths() {
                    if descendant != variant_path && descendant.has_prefix(&variant_path) {
                        data.erase_spec(&descendant);
                    }
                }
                data.erase_spec(&variant_path);
                let key = sdf::ChildrenKey::VariantChildren.as_str();
                if let Ok(Some(children)) = data.try_field(&set_path, key)
                    && let vt::Value::TokenVec(names) = children.into_owned()
                {
                    let kept: Vec<tf::Token> = names.into_iter().filter(|n| n.as_str() != variant).collect();
                    data.set_field(&set_path, key, vt::Value::TokenVec(kept));
                }
                let key = sdf::FieldKey::VariantSelection.as_str();
                if let Ok(Some(field)) = data.try_field(&spec_path, key)
                    && let vt::Value::VariantSelectionMap(mut selections) = field.into_owned()
                    && selections.get(variant_set).is_some_and(|s| s == variant)
                {
                    selections.remove(variant_set);
                    if selections.is_empty() {
                        data.erase_field(&spec_path, key);
                    } else {
                        data.set_field(&spec_path, key, vt::Value::VariantSelectionMap(selections));
                    }
                }
                removed = true;
                Ok(())
            })
            .map_err(author_err)?;
        self.touched();
        Ok(removed)
    }

    pub fn add_reference_with_offset(
        &mut self,
        prim: Prim,
        asset_path: &str,
        prim_path: &str,
        offset: f64,
        scale: f64,
    ) -> Result<()> {
        layer_offset(offset, scale)?;
        optional_path(prim_path)?;
        defer!(self, (), [asset_path, prim_path], |s| s.add_reference_with_offset(
            prim,
            &asset_path,
            &prim_path,
            offset,
            scale
        ));
        let p = self.usd_prim(prim)?;
        let reference = sdf::Reference {
            asset_path: asset_path.to_owned(),
            prim_path: optional_path(prim_path)?,
            layer_offset: layer_offset(offset, scale)?,
            ..Default::default()
        };
        self.immediate("adding a reference", |_| p.references().add(reference))
    }

    pub fn add_payload_with_offset(
        &mut self,
        prim: Prim,
        asset_path: &str,
        prim_path: &str,
        offset: f64,
        scale: f64,
    ) -> Result<()> {
        layer_offset(offset, scale)?;
        optional_path(prim_path)?;
        defer!(self, (), [asset_path, prim_path], |s| s.add_payload_with_offset(
            prim,
            &asset_path,
            &prim_path,
            offset,
            scale
        ));
        let p = self.usd_prim(prim)?;
        let payload = sdf::Payload {
            asset_path: asset_path.to_owned(),
            prim_path: optional_path(prim_path)?,
            layer_offset: layer_offset(offset, scale)?,
        };
        self.immediate("adding a payload", |_| p.payloads().add(payload))
    }
}

fn layer_offset(offset: f64, scale: f64) -> Result<sdf::LayerOffset> {
    if !offset.is_finite() || !scale.is_finite() || scale == 0.0 {
        return Err(Error::InvalidValue(format!(
            "a layer offset needs a finite offset and a finite, nonzero scale, not {offset} and {scale}"
        )));
    }
    Ok(sdf::LayerOffset { offset, scale })
}

const PREVIEW_INPUTS: [(&str, &str); 14] = [
    ("clearcoat", "float"),
    ("clearcoatRoughness", "float"),
    ("diffuseColor", "color3f"),
    ("displacement", "float"),
    ("emissiveColor", "color3f"),
    ("ior", "float"),
    ("metallic", "float"),
    ("normal", "normal3f"),
    ("occlusion", "float"),
    ("opacity", "float"),
    ("opacityThreshold", "float"),
    ("roughness", "float"),
    ("specularColor", "color3f"),
    ("useSpecularWorkflow", "int"),
];

fn preview_input(input: &str) -> Result<(&'static str, &'static str)> {
    let input = input.strip_prefix("inputs:").unwrap_or(input);
    PREVIEW_INPUTS
        .iter()
        .find(|(name, _)| *name == input)
        .copied()
        .ok_or_else(|| {
            let names: Vec<&str> = PREVIEW_INPUTS.iter().map(|(n, _)| *n).collect();
            Error::InvalidValue(format!(
                "'{input}' is not a UsdPreviewSurface input; the inputs are {}",
                names.join(", ")
            ))
        })
}

impl Stage {
    /// The type of a UsdPreviewSurface input, such as `color3f` for
    /// `diffuseColor`.
    pub fn preview_input_type(input: &str) -> Option<&'static str> {
        preview_input(input).ok().map(|(_, t)| t)
    }

    fn define_shader(&mut self, path: &str, id: &str) -> Result<Prim> {
        let shader = self.define_prim(path, "Shader")?;
        if self.declared_type(shader, "info:id")?.is_none() {
            self.create_attribute(shader, "info:id", "token", true)?;
        }
        self.set(shader, "info:id", &Value::token(id), None)?;
        Ok(shader)
    }

    fn connect(&mut self, prim: Prim, name: &str, type_name: &str, source: Prim, output: &str) -> Result<()> {
        self.ensure_attribute(prim, name, type_name)?;
        let source = format!("{}.{output}", self.path(source)?);
        self.set_connections(prim, name, &[&source])
    }

    fn output(&mut self, prim: Prim, name: &str, type_name: &str) -> Result<()> {
        self.ensure_attribute(prim, name, type_name).map(drop)
    }

    /// A Material at `path` whose surface is a UsdPreviewSurface shader named
    /// `PreviewSurface` under it. The shader's inputs keep their defaults
    /// until `set_preview_input` or `set_preview_texture` writes them.
    pub fn define_preview_material(&mut self, path: &str) -> Result<Prim> {
        self.not_batched("defining a preview material")?;
        let material = self.define_prim(path, "Material")?;
        let shader_path = format!("{}/PreviewSurface", self.path(material)?);
        let shader = self.define_shader(&shader_path, "UsdPreviewSurface")?;
        self.output(shader, "outputs:surface", "token")?;
        self.connect(material, "outputs:surface", "token", shader, "outputs:surface")?;
        Ok(material)
    }

    fn preview_surface(&mut self, material: Prim) -> Result<Prim> {
        let sources = self.connections(material, "outputs:surface")?;
        let shader = match sources.first().and_then(|s| s.rsplit_once('.')) {
            Some((prim_path, _)) => self.prim_at_path(prim_path)?,
            None => None,
        };
        let shader = shader.filter(|&s| {
            let mut id = Value::new();
            self.get(s, "info:id", None, &mut id).unwrap_or(false) && id.strings() == ["UsdPreviewSurface"]
        });
        shader.ok_or_else(|| {
            Error::InvalidValue(format!(
                "{}'s surface is not a UsdPreviewSurface; make it with define_preview_material",
                self.path(material).unwrap_or("?")
            ))
        })
    }

    pub fn set_preview_input(&mut self, material: Prim, input: &str, value: &Value) -> Result<()> {
        let (name, type_name) = preview_input(input)?;
        let shader = self.preview_surface(material)?;
        let attr = format!("inputs:{name}");
        self.ensure_attribute(shader, &attr, type_name)?;
        self.set(shader, &attr, value, None)
    }

    /// Feeds a UsdPreviewSurface input from an image. A UsdUVTexture shader
    /// named after the input reads `asset_path` with the `st` primvar, through
    /// one UsdPrimvarReader_float2 the material's textures share. Normal maps
    /// are read as raw data and remapped from 0..1 to -1..1. Returns the
    /// texture shader so more of its inputs can be set.
    pub fn set_preview_texture(&mut self, material: Prim, input: &str, asset_path: &str) -> Result<Prim> {
        self.not_batched("setting a preview texture")?;
        let (name, type_name) = preview_input(input)?;
        let surface = self.preview_surface(material)?;
        let base = self.path(material)?.to_owned();
        let reader_path = format!("{base}/TexCoordReader");
        let reader = match self.prim_at_path(&reader_path)? {
            Some(reader) if self.is_a(reader, "Shader")? => reader,
            _ => {
                let reader = self.define_shader(&reader_path, "UsdPrimvarReader_float2")?;
                self.ensure_attribute(reader, "inputs:varname", "string")?;
                self.set(reader, "inputs:varname", &Value::string("st"), None)?;
                self.output(reader, "outputs:result", "float2")?;
                reader
            }
        };
        let mut texture_name = format!("{name}Texture");
        if let Some(first) = texture_name.get_mut(..1) {
            first.make_ascii_uppercase();
        }
        let texture = self.define_shader(&format!("{base}/{texture_name}"), "UsdUVTexture")?;
        self.ensure_attribute(texture, "inputs:file", "asset")?;
        self.set(texture, "inputs:file", &Value::asset(asset_path), None)?;
        self.connect(texture, "inputs:st", "float2", reader, "outputs:result")?;
        let color = matches!(name, "diffuseColor" | "emissiveColor" | "specularColor");
        let (output, output_type) = if type_name == "float" {
            ("outputs:r", "float")
        } else {
            ("outputs:rgb", "float3")
        };
        self.output(texture, output, output_type)?;
        self.ensure_attribute(texture, "inputs:sourceColorSpace", "token")?;
        let space = if color { "sRGB" } else { "raw" };
        self.set(texture, "inputs:sourceColorSpace", &Value::token(space), None)?;
        if name == "normal" {
            for (attr, v) in [
                ("inputs:scale", [2.0f32, 2.0, 2.0, 1.0]),
                ("inputs:bias", [-1.0, -1.0, -1.0, 0.0]),
            ] {
                self.ensure_attribute(texture, attr, "float4")?;
                self.set(texture, attr, &Value::from_slice(BaseType::Float4, false, &[v])?, None)?;
            }
        }
        self.connect(surface, &format!("inputs:{name}"), type_name, texture, output)?;
        Ok(texture)
    }
}

impl Stage {
    /// Plays `prim`'s attributes from clip layers: `asset_paths` are the
    /// clips, `prim_path` is where each holds the animated prim, `active`
    /// pairs a stage time with the clip used from then on, and `times` pairs a
    /// stage time with the clip time read there. `manifest_asset_path` names
    /// the layer declaring which attributes the clips animate, written by
    /// `export_clip_manifest`. Clip set names other than `default` play as a
    /// separate set.
    #[allow(clippy::too_many_arguments)]
    pub fn set_value_clips(
        &mut self,
        prim: Prim,
        clip_set: &str,
        asset_paths: &[&str],
        prim_path: &str,
        active: &[[f64; 2]],
        times: &[[f64; 2]],
        manifest_asset_path: &str,
    ) -> Result<()> {
        if clip_set.is_empty() || clip_set.contains(':') {
            return Err(Error::InvalidValue(format!("'{clip_set}' is not a clip set name")));
        }
        let clip_prim = parse_path(prim_path)?;
        if !clip_prim.is_abs() || !clip_prim.is_prim() || clip_prim.is_abs_root() {
            return Err(Error::InvalidPath(format!("{prim_path} is not an absolute prim path")));
        }
        if let Some(bad) = active
            .iter()
            .find(|[_, index]| index.fract() != 0.0 || *index < 0.0 || *index as usize >= asset_paths.len())
        {
            return Err(Error::InvalidValue(format!(
                "active clip {} is not one of the {} clips",
                bad[1],
                asset_paths.len()
            )));
        }
        if self.batch.is_some() {
            let (clip_set, prim_path, manifest) = (
                clip_set.to_owned(),
                prim_path.to_owned(),
                manifest_asset_path.to_owned(),
            );
            let assets: Vec<String> = asset_paths.iter().map(|a| (*a).to_owned()).collect();
            let (active, times) = (active.to_vec(), times.to_vec());
            self.defer_edit(Box::new(move |s: &mut Stage| {
                let assets: Vec<&str> = assets.iter().map(String::as_str).collect();
                s.set_value_clips(prim, &clip_set, &assets, &prim_path, &active, &times, &manifest)
            }));
            return Ok(());
        }
        let clips = usd::ClipsAPI::from_prim_unchecked(self.usd_prim(prim)?);
        let pairs = |v: &[[f64; 2]]| v.iter().map(|&[a, b]| gf::Vec2d { x: a, y: b }).collect::<Vec<_>>();
        let assets: Vec<String> = asset_paths.iter().map(|a| (*a).to_owned()).collect();
        self.immediate("setting value clips", |_| {
            clips.set_clip_asset_paths(clip_set, assets)?;
            clips.set_clip_prim_path(clip_set, prim_path)?;
            clips.set_clip_active(clip_set, pairs(active))?;
            if !times.is_empty() {
                clips.set_clip_times(clip_set, pairs(times))?;
            }
            if !manifest_asset_path.is_empty() {
                clips.set_clip_manifest_asset_path(clip_set, manifest_asset_path)?;
            }
            Ok(())
        })
    }

    /// Writes the manifest for `prim`'s clip set to `path`: a layer declaring
    /// every attribute its clips animate, with blocks where a clip lacks one,
    /// so stage times between clips don't fall back to the prim's own values.
    pub fn export_clip_manifest(&self, prim: Prim, clip_set: &str, path: &str) -> Result<()> {
        self.not_batched("export")?;
        let clips = usd::ClipsAPI::from_prim_unchecked(self.usd_prim(prim)?);
        let layer = clips
            .generate_clip_manifest(clip_set, true)
            .map_err(usd_err)?
            .ok_or_else(|| {
                Error::InvalidValue(format!(
                    "{} has no clip set '{clip_set}'",
                    self.path(prim).unwrap_or("?")
                ))
            })?;
        self.store_layer(&layer, path)
    }
}
