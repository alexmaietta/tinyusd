use bytemuck::Pod;
use openusd::{gf, sdf, tf, vt};

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum BaseType {
    Empty = 0,
    Bool = 1,
    UChar = 2,
    Int = 3,
    UInt = 4,
    Int64 = 5,
    UInt64 = 6,
    Half = 7,
    Float = 8,
    Double = 9,
    TimeCode = 10,
    String = 11,
    Token = 12,
    Asset = 13,
    Path = 14,
    Int2 = 15,
    Int3 = 16,
    Int4 = 17,
    Half2 = 18,
    Half3 = 19,
    Half4 = 20,
    Float2 = 21,
    Float3 = 22,
    Float4 = 23,
    Double2 = 24,
    Double3 = 25,
    Double4 = 26,
    Quath = 27,
    Quatf = 28,
    Quatd = 29,
    Matrix2d = 30,
    Matrix3d = 31,
    Matrix4d = 32,
    Unsupported = 33,
}

impl BaseType {
    pub fn from_type_name(type_name: &str) -> Option<(Self, bool)> {
        let (name, array) = match type_name.strip_suffix("[]") {
            Some(name) => (name, true),
            None => (type_name, false),
        };
        let name = ["point", "normal", "vector", "color", "texCoord"]
            .iter()
            .find_map(|role| name.strip_prefix(role))
            .unwrap_or(name);
        let base = match name {
            "bool" => Self::Bool,
            "uchar" => Self::UChar,
            "int" => Self::Int,
            "uint" => Self::UInt,
            "int64" => Self::Int64,
            "uint64" => Self::UInt64,
            "half" => Self::Half,
            "float" => Self::Float,
            "double" => Self::Double,
            "timecode" => Self::TimeCode,
            "string" => Self::String,
            "token" => Self::Token,
            "asset" => Self::Asset,
            "int2" => Self::Int2,
            "int3" => Self::Int3,
            "int4" => Self::Int4,
            "2h" | "half2" => Self::Half2,
            "3h" | "half3" => Self::Half3,
            "4h" | "half4" => Self::Half4,
            "2f" | "float2" => Self::Float2,
            "3f" | "float3" => Self::Float3,
            "4f" | "float4" => Self::Float4,
            "2d" | "double2" => Self::Double2,
            "3d" | "double3" => Self::Double3,
            "4d" | "double4" => Self::Double4,
            "quath" => Self::Quath,
            "quatf" => Self::Quatf,
            "quatd" => Self::Quatd,
            "matrix2d" => Self::Matrix2d,
            "matrix3d" => Self::Matrix3d,
            "matrix4d" | "frame4d" => Self::Matrix4d,
            _ => return None,
        };
        Some((base, array))
    }

    pub const fn is_string_like(self) -> bool {
        matches!(
            self,
            BaseType::String | BaseType::Token | BaseType::Asset | BaseType::Path
        )
    }

    pub const fn component_count(self) -> usize {
        match self {
            BaseType::Int2 | BaseType::Half2 | BaseType::Float2 | BaseType::Double2 => 2,
            BaseType::Int3 | BaseType::Half3 | BaseType::Float3 | BaseType::Double3 => 3,
            BaseType::Int4
            | BaseType::Half4
            | BaseType::Float4
            | BaseType::Double4
            | BaseType::Quath
            | BaseType::Quatf
            | BaseType::Quatd
            | BaseType::Matrix2d => 4,
            BaseType::Matrix3d => 9,
            BaseType::Matrix4d => 16,
            BaseType::Empty | BaseType::Unsupported => 0,
            _ => 1,
        }
    }

    pub const fn scalar_size(self) -> usize {
        match self {
            BaseType::Bool | BaseType::UChar => 1,
            BaseType::Half | BaseType::Half2 | BaseType::Half3 | BaseType::Half4 | BaseType::Quath => 2,
            BaseType::Int
            | BaseType::UInt
            | BaseType::Float
            | BaseType::Int2
            | BaseType::Int3
            | BaseType::Int4
            | BaseType::Float2
            | BaseType::Float3
            | BaseType::Float4
            | BaseType::Quatf => 4,
            BaseType::Int64
            | BaseType::UInt64
            | BaseType::Double
            | BaseType::TimeCode
            | BaseType::Double2
            | BaseType::Double3
            | BaseType::Double4
            | BaseType::Quatd
            | BaseType::Matrix2d
            | BaseType::Matrix3d
            | BaseType::Matrix4d => 8,
            _ => 0,
        }
    }

    pub const fn element_size(self) -> usize {
        self.scalar_size() * self.component_count()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Value {
    base: Option<BaseType>,
    array: bool,
    len: usize,
    data: Vec<u64>,
    byte_len: usize,
    strings: Vec<String>,
    resolved: Vec<String>,
    type_name: String,
}

impl Value {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.base = None;
        self.array = false;
        self.len = 0;
        self.byte_len = 0;
        self.data.clear();
        self.strings.clear();
        self.resolved.clear();
        self.type_name.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.base.is_none_or(|b| b == BaseType::Empty)
    }

    pub fn base_type(&self) -> BaseType {
        self.base.unwrap_or(BaseType::Empty)
    }

    pub fn is_array(&self) -> bool {
        self.array
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn bytes(&self) -> &[u8] {
        &bytemuck::cast_slice::<u64, u8>(&self.data)[..self.byte_len]
    }

    pub fn as_slice<T: Pod>(&self) -> Option<&[T]> {
        if self.base_type().is_string_like() || self.byte_len == 0 && self.len > 0 {
            return None;
        }
        bytemuck::try_cast_slice(self.bytes()).ok()
    }

    pub fn as_scalar<T: Pod>(&self) -> Option<T> {
        if self.array {
            return None;
        }
        self.as_slice::<T>().and_then(|s| s.first().copied())
    }

    pub fn strings(&self) -> &[String] {
        &self.strings
    }

    pub fn resolved_paths(&self) -> &[String] {
        &self.resolved
    }

    pub fn from_bytes(base: BaseType, array: bool, bytes: &[u8]) -> Result<Self> {
        let size = base.element_size();
        if size == 0 {
            return Err(Error::InvalidValue(format!(
                "{base:?} values are built with Value::from_strings"
            )));
        }
        if !bytes.len().is_multiple_of(size) {
            return Err(Error::InvalidValue(format!(
                "{} bytes is not a whole number of {base:?} elements of {size} bytes",
                bytes.len()
            )));
        }
        let len = bytes.len() / size;
        if !array && len != 1 {
            return Err(Error::InvalidValue(format!(
                "a scalar {base:?} needs one element, got {len}"
            )));
        }
        let mut value = Self::new();
        value.set_pod(base, array, bytes);
        value.len = len;
        Ok(value)
    }

    pub fn from_slice<T: Pod>(base: BaseType, array: bool, items: &[T]) -> Result<Self> {
        Self::from_bytes(base, array, bytemuck::cast_slice(items))
    }

    pub fn from_strings<S: AsRef<str>>(base: BaseType, array: bool, items: &[S]) -> Result<Self> {
        if !base.is_string_like() {
            return Err(Error::InvalidValue(format!(
                "{base:?} values are built with Value::from_bytes"
            )));
        }
        if base == BaseType::Path && !array {
            return Err(Error::InvalidValue("path values are always arrays".to_owned()));
        }
        if !array && items.len() != 1 {
            return Err(Error::InvalidValue(format!(
                "a scalar {base:?} needs one item, got {}",
                items.len()
            )));
        }
        let mut value = Self::new();
        value.set_strings(base, array, items.iter().map(AsRef::as_ref));
        Ok(value)
    }

    pub fn bool(v: bool) -> Self {
        Self::scalar(BaseType::Bool, &[u8::from(v)])
    }

    pub fn int(v: i32) -> Self {
        Self::scalar(BaseType::Int, &[v])
    }

    pub fn float(v: f32) -> Self {
        Self::scalar(BaseType::Float, &[v])
    }

    pub fn double(v: f64) -> Self {
        Self::scalar(BaseType::Double, &[v])
    }

    pub fn token(v: &str) -> Self {
        let mut value = Self::new();
        value.set_strings(BaseType::Token, false, [v]);
        value
    }

    pub fn string(v: &str) -> Self {
        let mut value = Self::new();
        value.set_strings(BaseType::String, false, [v]);
        value
    }

    pub fn asset(v: &str) -> Self {
        let mut value = Self::new();
        value.set_strings(BaseType::Asset, false, [v]);
        value
    }

    fn scalar<T: Pod>(base: BaseType, item: &[T; 1]) -> Self {
        let mut value = Self::new();
        value.set_pod(base, false, item);
        value
    }

    #[cfg_attr(not(feature = "write"), allow(dead_code))]
    pub(crate) fn to_vt(&self) -> Result<vt::Value> {
        use BaseType as B;
        use vt::Value as V;
        fn pods<T: Pod>(bytes: &[u8]) -> Vec<T> {
            bytemuck::pod_collect_to_vec(bytes)
        }
        let bytes = self.bytes();
        let a = self.array;
        macro_rules! pick {
            ($scalar:ident, $vec:ident, $t:ty) => {{
                let items = pods::<$t>(bytes);
                if a { V::$vec(items) } else { V::$scalar(items[0]) }
            }};
        }
        let strings = &self.strings;
        Ok(match self.base_type() {
            B::Empty | B::Unsupported => {
                return Err(Error::InvalidValue(
                    "an empty or unsupported value cannot be written".to_owned(),
                ));
            }
            B::Bool => {
                let items: Vec<bool> = bytes.iter().map(|&b| b != 0).collect();
                if a { V::BoolVec(items) } else { V::Bool(items[0]) }
            }
            B::UChar => pick!(Uchar, UcharVec, u8),
            B::Int => pick!(Int, IntVec, i32),
            B::UInt => pick!(Uint, UintVec, u32),
            B::Int64 => pick!(Int64, Int64Vec, i64),
            B::UInt64 => pick!(Uint64, Uint64Vec, u64),
            B::Half => pick!(Half, HalfVec, gf::f16),
            B::Float => pick!(Float, FloatVec, f32),
            B::Double => pick!(Double, DoubleVec, f64),
            B::TimeCode => {
                let items: Vec<sdf::TimeCode> = pods::<f64>(bytes).into_iter().map(sdf::TimeCode).collect();
                if a {
                    V::TimeCodeVec(items)
                } else {
                    V::TimeCode(items[0])
                }
            }
            B::Int2 => pick!(Vec2i, Vec2iVec, gf::Vec2i),
            B::Int3 => pick!(Vec3i, Vec3iVec, gf::Vec3i),
            B::Int4 => pick!(Vec4i, Vec4iVec, gf::Vec4i),
            B::Half2 => pick!(Vec2h, Vec2hVec, gf::Vec2h),
            B::Half3 => pick!(Vec3h, Vec3hVec, gf::Vec3h),
            B::Half4 => pick!(Vec4h, Vec4hVec, gf::Vec4h),
            B::Float2 => pick!(Vec2f, Vec2fVec, gf::Vec2f),
            B::Float3 => pick!(Vec3f, Vec3fVec, gf::Vec3f),
            B::Float4 => pick!(Vec4f, Vec4fVec, gf::Vec4f),
            B::Double2 => pick!(Vec2d, Vec2dVec, gf::Vec2d),
            B::Double3 => pick!(Vec3d, Vec3dVec, gf::Vec3d),
            B::Double4 => pick!(Vec4d, Vec4dVec, gf::Vec4d),
            B::Quath => pick!(Quath, QuathVec, gf::Quath),
            B::Quatf => pick!(Quatf, QuatfVec, gf::Quatf),
            B::Quatd => pick!(Quatd, QuatdVec, gf::Quatd),
            B::Matrix2d => pick!(Matrix2d, Matrix2dVec, gf::Mat2d),
            B::Matrix3d => pick!(Matrix3d, Matrix3dVec, gf::Mat3d),
            B::Matrix4d => pick!(Matrix4d, Matrix4dVec, gf::Mat4d),
            B::String => {
                if a {
                    V::StringVec(strings.clone())
                } else {
                    V::String(strings[0].clone())
                }
            }
            B::Token => {
                let items: Vec<tf::Token> = strings.iter().map(|s| tf::Token::from(s.clone())).collect();
                if a {
                    V::TokenVec(items)
                } else {
                    V::Token(items[0].clone())
                }
            }
            B::Asset => {
                let items: Vec<sdf::AssetPath> = strings.iter().map(sdf::AssetPath::new).collect();
                if a {
                    V::AssetPathVec(items)
                } else {
                    V::AssetPath(items[0].clone())
                }
            }
            B::Path => V::PathVec(
                strings
                    .iter()
                    .map(|s| sdf::Path::new(s).map_err(|e| Error::InvalidPath(format!("{s}: {e}"))))
                    .collect::<Result<_>>()?,
            ),
        })
    }

    pub fn set_uchar_array(&mut self, bytes: &[u8]) {
        self.clear();
        self.set_pod(BaseType::UChar, true, bytes);
    }

    pub fn set_float3_array(&mut self, values: &[[f32; 3]]) {
        self.clear();
        self.set_pod(BaseType::Float3, true, values);
    }

    pub fn set_double_array(&mut self, values: &[f64]) {
        self.clear();
        self.set_pod(BaseType::Double, true, values);
    }

    pub(crate) fn set_type_name(&mut self, name: &str) {
        self.type_name.clear();
        self.type_name.push_str(name);
    }

    fn set_pod<T: Pod>(&mut self, base: BaseType, array: bool, items: &[T]) {
        let bytes: &[u8] = bytemuck::cast_slice(items);
        self.base = Some(base);
        self.array = array;
        self.len = items.len();
        self.byte_len = bytes.len();
        self.data.clear();
        self.data.resize(bytes.len().div_ceil(8), 0);
        bytemuck::cast_slice_mut::<u64, u8>(&mut self.data)[..bytes.len()].copy_from_slice(bytes);
    }

    fn set_strings<'a>(&mut self, base: BaseType, array: bool, items: impl IntoIterator<Item = &'a str>) {
        self.base = Some(base);
        self.array = array;
        self.strings.clear();
        self.strings.extend(items.into_iter().map(str::to_owned));
        self.len = self.strings.len();
    }

    fn set_assets<'a>(&mut self, array: bool, items: impl IntoIterator<Item = &'a sdf::AssetPath>) {
        self.base = Some(BaseType::Asset);
        self.array = array;
        self.strings.clear();
        self.resolved.clear();
        for asset in items {
            self.strings.push(asset.as_str().to_owned());
            self.resolved
                .push(simplify_path(asset.resolved_path().unwrap_or_default()));
        }
        self.len = self.strings.len();
    }

    fn set_unsupported(&mut self) {
        self.base = Some(BaseType::Unsupported);
        self.array = false;
        self.len = 0;
    }

    pub(crate) fn assign(&mut self, value: vt::Value) -> bool {
        use vt::Value as V;
        let type_name = std::mem::take(&mut self.type_name);
        self.clear();
        self.type_name = type_name;
        match value {
            V::None | V::ValueBlock => return false,
            V::Bool(v) => self.set_pod(BaseType::Bool, false, &[u8::from(v)]),
            V::BoolVec(v) => self.set_pod(
                BaseType::Bool,
                true,
                &v.iter().map(|&b| u8::from(b)).collect::<Vec<_>>(),
            ),
            V::Uchar(v) => self.set_pod(BaseType::UChar, false, &[v]),
            V::UcharVec(v) => self.set_pod(BaseType::UChar, true, &v),
            V::Int(v) => self.set_pod(BaseType::Int, false, &[v]),
            V::IntVec(v) => self.set_pod(BaseType::Int, true, &v),
            V::Uint(v) => self.set_pod(BaseType::UInt, false, &[v]),
            V::UintVec(v) => self.set_pod(BaseType::UInt, true, &v),
            V::Int64(v) => self.set_pod(BaseType::Int64, false, &[v]),
            V::Int64Vec(v) => self.set_pod(BaseType::Int64, true, &v),
            V::Uint64(v) => self.set_pod(BaseType::UInt64, false, &[v]),
            V::Uint64Vec(v) => self.set_pod(BaseType::UInt64, true, &v),
            V::Half(v) => self.set_pod(BaseType::Half, false, &[v]),
            V::HalfVec(v) => self.set_pod(BaseType::Half, true, &v),
            V::Float(v) => self.set_pod(BaseType::Float, false, &[v]),
            V::FloatVec(v) => self.set_pod(BaseType::Float, true, &v),
            V::Double(v) => self.set_pod(BaseType::Double, false, &[v]),
            V::DoubleVec(v) => self.set_pod(BaseType::Double, true, &v),
            V::TimeCode(v) => self.set_pod(BaseType::TimeCode, false, &[v.value()]),
            V::TimeCodeVec(v) => self.set_pod(
                BaseType::TimeCode,
                true,
                &v.iter().map(|t| t.value()).collect::<Vec<_>>(),
            ),
            V::String(v) => self.set_strings(BaseType::String, false, [v.as_str()]),
            V::StringVec(v) => self.set_strings(BaseType::String, true, v.iter().map(String::as_str)),
            V::Token(v) => self.set_strings(BaseType::Token, false, [v.as_str()]),
            V::TokenVec(v) => self.set_strings(BaseType::Token, true, v.iter().map(|t| t.as_str())),
            V::AssetPath(v) => self.set_assets(false, [&v]),
            V::AssetPathVec(v) => self.set_assets(true, v.iter()),
            V::PathVec(v) => {
                let paths: Vec<String> = v.iter().map(ToString::to_string).collect();
                self.set_strings(BaseType::Path, true, paths.iter().map(String::as_str));
            }
            V::Specifier(v) => self.set_strings(BaseType::Token, false, [specifier_token(v)]),
            V::Permission(v) => self.set_strings(BaseType::Token, false, [permission_token(v)]),
            V::Variability(v) => self.set_strings(BaseType::Token, false, [variability_token(v)]),
            V::Quath(v) => self.set_pod(BaseType::Quath, false, &[v]),
            V::Quatf(v) => self.set_pod(BaseType::Quatf, false, &[v]),
            V::Quatd(v) => self.set_pod(BaseType::Quatd, false, &[v]),
            V::QuathVec(v) => self.set_pod(BaseType::Quath, true, &v),
            V::QuatfVec(v) => self.set_pod(BaseType::Quatf, true, &v),
            V::QuatdVec(v) => self.set_pod(BaseType::Quatd, true, &v),
            V::Vec2h(v) => self.set_pod(BaseType::Half2, false, &[v]),
            V::Vec2f(v) => self.set_pod(BaseType::Float2, false, &[v]),
            V::Vec2d(v) => self.set_pod(BaseType::Double2, false, &[v]),
            V::Vec2i(v) => self.set_pod(BaseType::Int2, false, &[v]),
            V::Vec2hVec(v) => self.set_pod(BaseType::Half2, true, &v),
            V::Vec2fVec(v) => self.set_pod(BaseType::Float2, true, &v),
            V::Vec2dVec(v) => self.set_pod(BaseType::Double2, true, &v),
            V::Vec2iVec(v) => self.set_pod(BaseType::Int2, true, &v),
            V::Vec3h(v) => self.set_pod(BaseType::Half3, false, &[v]),
            V::Vec3f(v) => self.set_pod(BaseType::Float3, false, &[v]),
            V::Vec3d(v) => self.set_pod(BaseType::Double3, false, &[v]),
            V::Vec3i(v) => self.set_pod(BaseType::Int3, false, &[v]),
            V::Vec3hVec(v) => self.set_pod(BaseType::Half3, true, &v),
            V::Vec3fVec(v) => self.set_pod(BaseType::Float3, true, &v),
            V::Vec3dVec(v) => self.set_pod(BaseType::Double3, true, &v),
            V::Vec3iVec(v) => self.set_pod(BaseType::Int3, true, &v),
            V::Vec4h(v) => self.set_pod(BaseType::Half4, false, &[v]),
            V::Vec4f(v) => self.set_pod(BaseType::Float4, false, &[v]),
            V::Vec4d(v) => self.set_pod(BaseType::Double4, false, &[v]),
            V::Vec4i(v) => self.set_pod(BaseType::Int4, false, &[v]),
            V::Vec4hVec(v) => self.set_pod(BaseType::Half4, true, &v),
            V::Vec4fVec(v) => self.set_pod(BaseType::Float4, true, &v),
            V::Vec4dVec(v) => self.set_pod(BaseType::Double4, true, &v),
            V::Vec4iVec(v) => self.set_pod(BaseType::Int4, true, &v),
            V::Matrix2d(v) => self.set_pod(BaseType::Matrix2d, false, &[v]),
            V::Matrix3d(v) => self.set_pod(BaseType::Matrix3d, false, &[v]),
            V::Matrix4d(v) => self.set_pod(BaseType::Matrix4d, false, &[v]),
            V::Matrix2dVec(v) => self.set_pod(BaseType::Matrix2d, true, &v),
            V::Matrix3dVec(v) => self.set_pod(BaseType::Matrix3d, true, &v),
            V::Matrix4dVec(v) => self.set_pod(BaseType::Matrix4d, true, &v),
            _ => self.set_unsupported(),
        }
        true
    }
}

pub(crate) const fn specifier_token(v: sdf::Specifier) -> &'static str {
    match v {
        sdf::Specifier::Def => "def",
        sdf::Specifier::Over => "over",
        sdf::Specifier::Class => "class",
    }
}

const fn permission_token(v: sdf::Permission) -> &'static str {
    match v {
        sdf::Permission::Public => "public",
        sdf::Permission::Private => "private",
    }
}

pub(crate) const fn variability_token(v: sdf::Variability) -> &'static str {
    match v {
        sdf::Variability::Varying => "varying",
        sdf::Variability::Uniform => "uniform",
    }
}

pub(crate) fn simplify_path(path: &str) -> String {
    let Some(rest) = path.strip_prefix(r"\\?\") else {
        return path.to_owned();
    };
    if let Some(unc) = rest.strip_prefix(r"UNC\") {
        return format!(r"\\{unc}");
    }
    let bytes = rest.as_bytes();
    let drive = bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\';
    if drive && rest.len() < 260 {
        rest.to_owned()
    } else {
        path.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::simplify_path;

    #[test]
    fn strips_windows_verbatim_prefixes() {
        assert_eq!(simplify_path(r"\\?\C:\a\part.usda"), r"C:\a\part.usda");
        assert_eq!(simplify_path(r"\\?\C:\a\pkg.usdz[tex.png]"), r"C:\a\pkg.usdz[tex.png]");
        assert_eq!(simplify_path(r"\\?\UNC\server\share\a.usda"), r"\\server\share\a.usda");
        assert_eq!(simplify_path("/home/a/part.usda"), "/home/a/part.usda");
        let long = format!(r"\\?\C:\{}", "x".repeat(300));
        assert_eq!(simplify_path(&long), long);
    }
}

#[cfg(test)]
mod type_name_tests {
    use super::BaseType;

    #[test]
    fn maps_roles_and_arrays() {
        assert_eq!(BaseType::from_type_name("point3f[]"), Some((BaseType::Float3, true)));
        assert_eq!(BaseType::from_type_name("color4d"), Some((BaseType::Double4, false)));
        assert_eq!(BaseType::from_type_name("texCoord2h[]"), Some((BaseType::Half2, true)));
        assert_eq!(BaseType::from_type_name("normal3f"), Some((BaseType::Float3, false)));
        assert_eq!(BaseType::from_type_name("frame4d"), Some((BaseType::Matrix4d, false)));
        assert_eq!(BaseType::from_type_name("token[]"), Some((BaseType::Token, true)));
        assert_eq!(BaseType::from_type_name("opaque"), None);
        assert_eq!(BaseType::from_type_name("dictionary"), None);
    }
}
