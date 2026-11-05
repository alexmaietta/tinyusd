use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::rc::Rc;
use std::sync::Arc;

use openusd::{ar, usdz};

pub trait AssetResolver {
    fn resolve(&self, asset_path: &str, anchor: Option<&str>) -> Option<String>;
    fn read(&self, resolved_path: &str) -> io::Result<Vec<u8>>;
}

pub(crate) type PackageCache = Rc<RefCell<HashMap<String, Arc<[u8]>>>>;

pub(crate) struct HostResolver {
    host: Rc<dyn AssetResolver>,
    packages: PackageCache,
    identity: String,
}

impl HostResolver {
    pub(crate) fn new(host: Rc<dyn AssetResolver>, packages: PackageCache) -> Self {
        let identity = format!("tinyusd-host-{:p}", Rc::as_ptr(&host).cast::<()>());
        Self {
            host,
            packages,
            identity,
        }
    }

    fn package_bytes(&self, path: &str) -> io::Result<Arc<[u8]>> {
        if let Some(bytes) = self.packages.borrow().get(path) {
            return Ok(bytes.clone());
        }
        let bytes: Arc<[u8]> = self.host.read(path)?.into();
        self.packages.borrow_mut().insert(path.to_owned(), bytes.clone());
        Ok(bytes)
    }
}

fn is_package(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("usdz"))
}

fn is_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains(':')
        && !ar::is_package_relative_path(path)
}

fn join_relative(dir: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    for segment in rel.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn outer_package(path: &str) -> Cow<'_, str> {
    match ar::split_package_relative_path_outer(path) {
        Some((package, _)) => Cow::Owned(package),
        None => Cow::Borrowed(path),
    }
}

impl ar::Resolver for HostResolver {
    fn create_identifier(&self, asset_path: &str, anchor_path: Option<&ar::ResolvedPath>) -> String {
        if asset_path.is_empty() {
            return String::new();
        }
        let anchor = anchor_path.map(|a| a.to_string_lossy().into_owned());
        if let Some(anchor) = anchor.as_deref()
            && ar::is_package_relative_path(anchor)
            && is_relative(asset_path)
            && let Some((package, inner)) = ar::split_package_relative_path_inner(anchor)
        {
            let dir = inner.rsplit_once('/').map_or("", |(d, _)| d);
            return ar::join_package_relative_path(&package, &join_relative(dir, asset_path));
        }
        if let Some((package, inner)) = ar::split_package_relative_path_outer(asset_path) {
            let package = self.create_identifier(&package, anchor_path);
            return ar::join_package_relative_path(&package, &inner);
        }
        let host_anchor = anchor.as_deref().map(outer_package);
        self.host
            .resolve(asset_path, host_anchor.as_deref())
            .unwrap_or_else(|| asset_path.to_owned())
    }

    fn resolve(&self, asset_path: &str) -> Option<ar::ResolvedPath> {
        if let Some((package, inner)) = ar::split_package_relative_path_outer(asset_path) {
            let package = self.host.resolve(&package, None)?;
            return Some(ar::ResolvedPath::new(ar::join_package_relative_path(&package, &inner)));
        }
        self.host.resolve(asset_path, None).map(ar::ResolvedPath::new)
    }

    fn resolve_for_new_asset(&self, _asset_path: &str) -> Option<ar::ResolvedPath> {
        None
    }

    fn open_asset(&self, resolved_path: &ar::ResolvedPath) -> io::Result<Box<dyn ar::Asset>> {
        let path = resolved_path.to_string_lossy();
        if let Some((package, inner)) = ar::split_package_relative_path_outer(&path) {
            let entry = usdz::package_entry(self, &package, &inner).map_err(|e| io::Error::other(e.to_string()))?;
            return Ok(Box::new(io::Cursor::new(entry)));
        }
        if is_package(&path) {
            return Ok(Box::new(io::Cursor::new(ar::AssetBuffer::from(
                self.package_bytes(&path)?,
            ))));
        }
        Ok(Box::new(io::Cursor::new(self.host.read(&path)?)))
    }

    fn modification_timestamp(
        &self,
        _asset_path: &str,
        _resolved_path: &ar::ResolvedPath,
    ) -> Option<std::time::SystemTime> {
        None
    }

    fn identity(&self) -> String {
        self.identity.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::join_relative;

    #[test]
    fn joins_relative_paths() {
        assert_eq!(join_relative("a/b", "c.usda"), "a/b/c.usda");
        assert_eq!(join_relative("a/b", "./c.usda"), "a/b/c.usda");
        assert_eq!(join_relative("a/b", "../c.usda"), "a/c.usda");
        assert_eq!(join_relative("", "tex/x.png"), "tex/x.png");
    }
}
