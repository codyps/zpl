//! C layouts use integer flags, never Rust bool/Option discriminants.
use crate::{Failure, Result};

trait Value<T>: Sized {
    fn from_native(value: T) -> Self;
    fn native(self) -> Result<T>;
}
macro_rules! scalar {
    ($($ty:ty),*) => {$(impl Value<$ty> for $ty {
        fn from_native(value: $ty) -> Self { value }
        fn native(self) -> Result<$ty> { Ok(self) }
    })*};
}
scalar!(u8, u32, u64, usize);
impl Value<f64> for f64 {
    fn from_native(value: f64) -> Self {
        value
    }
    fn native(self) -> Result<f64> {
        if self.is_finite() && self >= 0.0 {
            Ok(self)
        } else {
            Err(Failure::argument("limits must be finite and nonnegative"))
        }
    }
}
impl Value<bool> for u8 {
    fn from_native(value: bool) -> Self {
        u8::from(value)
    }
    fn native(self) -> Result<bool> {
        match self {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Failure::argument("flags must be 0 or 1")),
        }
    }
}
macro_rules! optional {
    ($name:ident, $ty:ty) => {
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct $name {
            pub present: u8,
            pub value: $ty,
        }
        impl Value<Option<$ty>> for $name {
            fn from_native(value: Option<$ty>) -> Self {
                Self {
                    present: u8::from(value.is_some()),
                    value: value.unwrap_or_default(),
                }
            }
            fn native(self) -> Result<Option<$ty>> {
                match self.present {
                    0 => Ok(None),
                    1 => Ok(Some(self.value)),
                    _ => Err(Failure::argument("presence flags must be 0 or 1")),
                }
            }
        }
    };
}
optional!(ZplOptionalU32, u32);
optional!(ZplOptionalU16, u16);
optional!(ZplOptionalFileId, [u16; 3]);

macro_rules! configuration {
    ($name:ident, $native:ty, {$($field:ident: $ty:ty),* $(,)?}) => {
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct $name { $(pub $field: $ty,)* }
        impl From<$native> for $name {
            fn from(value: $native) -> Self {
                Self { $($field: Value::from_native(value.$field),)* }
            }
        }
        impl $name {
            pub(crate) fn native(self) -> Result<$native> {
                type Native = $native;
                Ok(Native { $($field: self.$field.native()?,)* })
            }
        }
    }
}
include!("config_fields.rs");

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ZplOptions {
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
    pub compatibility: ZplCompatibility,
}
impl From<zpl::Options> for ZplOptions {
    fn from(value: zpl::Options) -> Self {
        Self {
            width: value.width,
            height: value.height,
            dpi: value.dpi,
            compatibility: value.compatibility.into(),
        }
    }
}
impl ZplOptions {
    pub(crate) fn native(self) -> Result<zpl::Options> {
        Ok(zpl::Options {
            width: self.width,
            height: self.height,
            dpi: self.dpi,
            compatibility: self.compatibility.native()?,
        })
    }
}
