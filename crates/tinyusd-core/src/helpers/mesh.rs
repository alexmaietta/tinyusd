use openusd::usd::{self, SchemaBase};
use openusd_schemas::geom::{self, SubsetSchema};

use crate::stage::usd_err;
use crate::{Error, Interpolation, Prim, Result, Stage, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeomSubset {
    pub prim: Prim,
    pub family: String,
    pub element_type: String,
    pub indices: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Triangles {
    pub point_indices: Vec<[i32; 3]>,
    pub corner_indices: Vec<[i32; 3]>,
    pub faces: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Normals {
    pub normals: Vec<[f32; 3]>,
    pub interpolation: Interpolation,
    pub authored: bool,
}

impl Stage {
    fn ints(&self, prim: Prim, name: &str, time: Option<f64>) -> Result<Vec<i32>> {
        let mut v = Value::new();
        if !self.get(prim, name, time, &mut v)? {
            return Ok(Vec::new());
        }
        Ok(v.as_slice::<i32>().map(<[i32]>::to_vec).unwrap_or_default())
    }

    pub fn geom_subsets(&mut self, prim: Prim, time: Option<f64>) -> Result<Vec<GeomSubset>> {
        let Some(imageable) = geom::Imageable::from_prim(self.usd_prim(prim)?).map_err(usd_err)? else {
            return Ok(Vec::new());
        };
        let t = time.map(usd::TimeCode::new);
        let subsets = geom::Subset::all(&imageable).map_err(usd_err)?;
        let mut out = Vec::with_capacity(subsets.len());
        for subset in subsets {
            let family = subset
                .family_name()
                .map_err(usd_err)?
                .map(|f| f.to_string())
                .unwrap_or_default();
            let element_type = subset
                .element_type()
                .map_err(usd_err)?
                .unwrap_or_default()
                .as_token()
                .to_owned();
            let indices = subset.indices_at(t).map_err(usd_err)?.unwrap_or_default();
            let path = subset.prim().path().clone();
            out.push(GeomSubset {
                prim: self.intern(&path),
                family,
                element_type,
                indices,
            });
        }
        Ok(out)
    }

    pub fn triangulate(&self, prim: Prim, time: Option<f64>) -> Result<Option<Triangles>> {
        if !self.is_a(prim, "Mesh")? {
            return Ok(None);
        }
        let counts = self.ints(prim, "faceVertexCounts", time)?;
        let indices = self.ints(prim, "faceVertexIndices", time)?;
        let mut holes = self.ints(prim, "holeIndices", time)?;
        holes.sort_unstable();
        let mut orientation = Value::new();
        let left = self.get(prim, "orientation", time, &mut orientation)?
            && orientation.strings().first().is_some_and(|s| s == "leftHanded");
        let total: i64 = counts.iter().map(|&c| i64::from(c.max(0))).sum();
        if total != indices.len() as i64 {
            return Err(Error::InvalidValue(format!(
                "{}: faceVertexCounts sum to {total} but there are {} faceVertexIndices",
                self.path(prim)?,
                indices.len()
            )));
        }
        let mut out = Triangles::default();
        let mut offset = 0i32;
        for (face, &count) in counts.iter().enumerate() {
            let face = face as i32;
            if count >= 3 && holes.binary_search(&face).is_err() {
                for k in 1..count - 1 {
                    let (b, c) = if left {
                        (offset + k + 1, offset + k)
                    } else {
                        (offset + k, offset + k + 1)
                    };
                    let corners = [offset, b, c];
                    out.point_indices.push(corners.map(|i| indices[i as usize]));
                    out.corner_indices.push(corners);
                    out.faces.push(face);
                }
            }
            offset += count.max(0);
        }
        Ok(Some(out))
    }

    pub fn mesh_normals(&mut self, prim: Prim, time: Option<f64>) -> Result<Option<Normals>> {
        let Some(triangles) = self.triangulate(prim, time)? else {
            return Ok(None);
        };
        let mut v = Value::new();
        if let Some(info) = self.primvar(prim, "normals", time, &mut v)?
            && let Some(normals) = v.as_slice::<[f32; 3]>()
        {
            return Ok(Some(Normals {
                normals: normals.to_vec(),
                interpolation: info.interpolation,
                authored: true,
            }));
        }
        if self.get(prim, "normals", time, &mut v)?
            && let Some(normals) = v.as_slice::<[f32; 3]>()
        {
            let normals = normals.to_vec();
            let mut meta = Value::new();
            let interpolation = if self.attribute_metadata(prim, "normals", "interpolation", &mut meta)? {
                match meta.strings().first().map(String::as_str) {
                    Some("constant") => Interpolation::Constant,
                    Some("uniform") => Interpolation::Uniform,
                    Some("varying") => Interpolation::Varying,
                    Some("faceVarying") => Interpolation::FaceVarying,
                    _ => Interpolation::Vertex,
                }
            } else {
                Interpolation::Vertex
            };
            return Ok(Some(Normals {
                normals,
                interpolation,
                authored: true,
            }));
        }
        if !self.get(prim, "points", time, &mut v)? {
            return Ok(Some(Normals {
                normals: Vec::new(),
                interpolation: Interpolation::Vertex,
                authored: false,
            }));
        }
        let points = v.as_slice::<[f32; 3]>().unwrap_or(&[]);
        let mut normals = vec![[0.0f32; 3]; points.len()];
        for tri in &triangles.point_indices {
            if tri.iter().any(|&i| i < 0 || i as usize >= points.len()) {
                return Err(Error::InvalidValue(format!(
                    "{}: a face vertex index is out of range for {} points",
                    self.path(prim)?,
                    points.len()
                )));
            }
            let [a, b, c] = tri.map(|i| points[i as usize]);
            let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            for &i in tri {
                for k in 0..3 {
                    normals[i as usize][k] += n[k];
                }
            }
        }
        for n in &mut normals {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len > 0.0 {
                n.iter_mut().for_each(|c| *c /= len);
            }
        }
        Ok(Some(Normals {
            normals,
            interpolation: Interpolation::Vertex,
            authored: false,
        }))
    }
}
