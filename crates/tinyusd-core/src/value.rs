use bytemuck::Pod;
use openusd::{sdf, vt};

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
            self.resolved.push(asset.resolved_path().unwrap_or_default().to_owned());
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
