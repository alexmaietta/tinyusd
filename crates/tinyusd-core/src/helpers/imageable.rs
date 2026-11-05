use std::cell::RefCell;

use openusd::{gf, usd};
use openusd_schemas::geom::{self, ImageableExt};

use crate::stage::usd_err;
use crate::{Prim, Result, Stage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum Purpose {
    #[default]
    Default = 0,
    Render = 1,
    Proxy = 2,
    Guide = 3,
}

impl Purpose {
    fn to_geom(self) -> geom::Purpose {
        match self {
            Purpose::Default => geom::Purpose::Default,
            Purpose::Render => geom::Purpose::Render,
            Purpose::Proxy => geom::Purpose::Proxy,
            Purpose::Guide => geom::Purpose::Guide,
        }
    }

    fn from_geom(p: geom::Purpose) -> Self {
        match p {
            geom::Purpose::Default => Purpose::Default,
            geom::Purpose::Render => Purpose::Render,
            geom::Purpose::Proxy => Purpose::Proxy,
            geom::Purpose::Guide => Purpose::Guide,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bound {
    pub min: [f64; 3],
    pub max: [f64; 3],
    pub matrix: [f64; 16],
    pub aligned_min: [f64; 3],
    pub aligned_max: [f64; 3],
}

fn v3(v: gf::Vec3d) -> [f64; 3] {
    [v.x, v.y, v.z]
}

fn bound(b: gf::BBox3d) -> Option<Bound> {
    let range = b.range();
    if range.is_empty() {
        return None;
    }
    let aligned = b.aligned_range();
    Some(Bound {
        min: v3(range.min()),
        max: v3(range.max()),
        matrix: b.matrix().0,
        aligned_min: v3(aligned.min()),
        aligned_max: v3(aligned.max()),
    })
}

type BoundKey = (Option<u64>, Vec<Purpose>);

#[derive(Default)]
pub(crate) struct BoundCaches {
    cache: RefCell<Option<(BoundKey, geom::BBoxCache)>>,
}

#[derive(Clone, Copy)]
enum Space {
    World,
    Local,
    Untransformed,
}

impl Stage {
    fn imageable(&self, prim: Prim) -> Result<Option<geom::Imageable>> {
        let mut p = self.usd_prim(prim)?;
        loop {
            if let Some(imageable) = geom::Imageable::from_prim(p.clone()).map_err(usd_err)? {
                return Ok(Some(imageable));
            }
            match p.path().parent() {
                Some(parent) if parent != openusd::sdf::Path::abs_root() => {
                    p = self.inner.prim(parent).map_err(usd_err)?;
                }
                _ => return Ok(None),
            }
        }
    }

    pub fn visible(&self, prim: Prim, time: Option<f64>) -> Result<bool> {
        let Some(imageable) = self.imageable(prim)? else {
            return Ok(true);
        };
        let visibility = imageable
            .compute_visibility(time.map(usd::TimeCode::new))
            .map_err(usd_err)?;
        Ok(visibility != geom::Visibility::Invisible)
    }

    pub fn purpose(&self, prim: Prim) -> Result<Purpose> {
        match self.imageable(prim)? {
            Some(imageable) => Ok(Purpose::from_geom(imageable.compute_purpose().map_err(usd_err)?)),
            None => Ok(Purpose::Default),
        }
    }

    fn bound_in(&self, prim: Prim, time: Option<f64>, purposes: &[Purpose], space: Space) -> Result<Option<Bound>> {
        let p = self.usd_prim(prim)?;
        let mut purposes = if purposes.is_empty() {
            vec![Purpose::Default]
        } else {
            purposes.to_vec()
        };
        purposes.sort_by_key(|p| *p as u32);
        purposes.dedup();
        let key = (time.map(f64::to_bits), purposes);
        let mut slot = self.caches.bounds.cache.borrow_mut();
        if slot.as_ref().is_none_or(|(k, _)| *k != key) {
            let cache = geom::BBoxCache::new(time.map(usd::TimeCode::new), key.1.iter().map(|p| p.to_geom()));
            *slot = Some((key, cache));
        }
        let (_, cache) = slot.as_mut().expect("cache initialized");
        let b = match space {
            Space::World => cache.world_bound(&p),
            Space::Local => cache.local_bound(&p),
            Space::Untransformed => cache.untransformed_bound(&p),
        }
        .map_err(usd_err)?;
        Ok(bound(b))
    }

    pub fn world_bound(&self, prim: Prim, time: Option<f64>, purposes: &[Purpose]) -> Result<Option<Bound>> {
        self.bound_in(prim, time, purposes, Space::World)
    }

    pub fn local_bound(&self, prim: Prim, time: Option<f64>, purposes: &[Purpose]) -> Result<Option<Bound>> {
        self.bound_in(prim, time, purposes, Space::Local)
    }

    pub fn untransformed_bound(&self, prim: Prim, time: Option<f64>, purposes: &[Purpose]) -> Result<Option<Bound>> {
        self.bound_in(prim, time, purposes, Space::Untransformed)
    }
}
