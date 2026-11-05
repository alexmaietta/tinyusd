#[cfg(feature = "shade")]
use std::cell::RefCell;
#[cfg(feature = "shade")]
use std::collections::HashMap;

use crate::{Prim, Result, Stage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundMaterial {
    pub material: Prim,
    pub binding: String,
}

#[cfg(feature = "shade")]
#[derive(Default)]
pub(crate) struct MaterialCaches {
    bindings: RefCell<HashMap<String, openusd_schemas::shade::BindingCache>>,
}

impl Stage {
    #[cfg(feature = "shade")]
    pub fn bound_material(&mut self, prim: Prim, purpose: &str) -> Result<Option<BoundMaterial>> {
        use crate::stage::usd_err;
        use openusd::usd::SchemaBase;
        use openusd_schemas::shade::MaterialBindingAPI;
        let api = MaterialBindingAPI::from_prim_unchecked(self.usd_prim(prim)?);
        let resolved = {
            let mut caches = self.caches.materials.bindings.borrow_mut();
            let cache = caches.entry(purpose.to_owned()).or_default();
            api.resolve_binding(purpose, true, cache).map_err(usd_err)?
        };
        let Some(resolved) = resolved else {
            return Ok(None);
        };
        let Some(material) = resolved.material().map_err(usd_err)? else {
            return Ok(None);
        };
        let material_path = material.prim().path().clone();
        let binding = resolved.rel().path().to_string();
        Ok(Some(BoundMaterial {
            material: self.intern(&material_path),
            binding,
        }))
    }

    #[cfg(not(feature = "shade"))]
    pub fn bound_material(&mut self, _prim: Prim, _purpose: &str) -> Result<Option<BoundMaterial>> {
        Err(crate::Error::FeatureDisabled("shade"))
    }
}
