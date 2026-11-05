use std::cell::RefCell;

use openusd::usd;
use openusd_schemas::geom;

use crate::stage::usd_err;
use crate::{Prim, Result, Stage};

#[derive(Default)]
pub(crate) struct XformCaches {
    cache: RefCell<Option<(Option<u64>, geom::XformCache)>>,
}

impl Stage {
    fn with_xforms<R>(
        &self,
        time: Option<f64>,
        f: impl FnOnce(&mut geom::XformCache) -> std::result::Result<R, openusd_schemas::SchemaError>,
    ) -> Result<R> {
        let mut slot = self.caches.xforms.cache.borrow_mut();
        let key = time.map(f64::to_bits);
        if slot.as_ref().is_none_or(|(k, _)| *k != key) {
            *slot = Some((key, geom::XformCache::new(time.map(usd::TimeCode::new))));
        }
        let (_, cache) = slot.as_mut().expect("cache initialized");
        f(cache).map_err(usd_err)
    }

    pub fn world_transform(&self, prim: Prim, time: Option<f64>) -> Result<[f64; 16]> {
        let p = self.usd_prim(prim)?;
        self.with_xforms(time, |c| c.local_to_world_transform(&p)).map(|m| m.0)
    }

    pub fn local_transform(&self, prim: Prim, time: Option<f64>) -> Result<([f64; 16], bool)> {
        let p = self.usd_prim(prim)?;
        self.with_xforms(time, |c| c.local_transform(&p))
            .map(|(m, resets)| (m.0, resets))
    }
}
