//! Register bitfield types and macros
//!
//! To conveniently access and manipulate fields of a register, this
//! library provides types and macros to describe and access bitfields
//! of a register. This can be especially useful in conjuction with
//! the APIs defined in [`interfaces`](crate::interfaces), which make
//! use of these types and hence allow to access and manipulate
//! bitfields of proper registers directly.
//!
//! A specific section (bitfield) in a register is described by the
//! [`Field`] type, consisting of an unshifted bitmask over the base
//! register [`IntLike`](crate::IntLike) type, and a shift
//! parameter. It is further associated with a specific
//! [`RegisterLongName`], which can prevent its use with incompatible
//! registers.
//!
//! A value of a section of a register is described by the
//! [`FieldValue`] type. It stores the information of the respective
//! section in the register, as well as the associated value. A
//! [`FieldValue`] can be created from a [`Field`] through the
//! [`val`](Field::val) method.
//!
//! ## `register_bitfields` macro
//!
//! For defining register layouts with an associated
//! [`RegisterLongName`](crate::RegisterLongName), along with
//! [`Field`]s and matching [`FieldValue`]s, a convenient macro-based
//! interface can be used.
//!
//! The following example demonstrates how two registers can be
//! defined, over a `u32` base type:
//!
//! ```rust
//! # use tock_registers::register_bitfields;
//! # use tock_registers::registers::InMemoryRegister;
//! # use tock_registers::interfaces::{Readable, ReadWriteable};
//! register_bitfields![u32,
//!     Uart [
//!         ENABLE OFFSET(0) NUMBITS(4) [
//!             ON = 8,
//!             OFF = 0
//!         ]
//!     ],
//!     Psel [
//!         PIN OFFSET(0) NUMBITS(6),
//!         CONNECT OFFSET(31) NUMBITS(1)
//!     ],
//! ];
//!
//! // In this scope, `Uart` is a module, representing the register and
//! // its fields. `Uart::Register` is a `RegisterLongName` type
//! // identifying this register. `Uart::ENABLE` is a field covering the
//! // first 4 bits of this register. `Uart::ENABLE::ON` is a
//! // `FieldValue` over that field, with the associated value 8.
//! // We can now use the types like so:
//! let reg: InMemoryRegister<u32, Uart::Register> = InMemoryRegister::new(0);
//! assert!(reg.read(Uart::ENABLE) == 0x00000000);
//! reg.modify(Uart::ENABLE::ON);
//! assert!(reg.get() == 0x00000008);
//! ```

// The register interface uses `+` in a way that is fine for bitfields, but
// looks unusual (and perhaps problematic) to a linter. We just ignore those
// lints for this file.
#![allow(clippy::suspicious_op_assign_impl)]
#![allow(clippy::suspicious_arithmetic_impl)]

use core::marker::PhantomData;
use core::ops::{Add, AddAssign};

use crate::{IntLike, RegisterLongName};

/// Specific section of a register.
///
/// For the Field, the mask is unshifted, ie. the LSB should always be set.
pub struct Field<T: IntLike, R: RegisterLongName> {
    pub mask: T,
    pub shift: usize,
    associated_register: PhantomData<R>,
}

impl<T: IntLike, R: RegisterLongName> Field<T, R> {
    pub const fn new(mask: T, shift: usize) -> Field<T, R> {
        Field {
            mask: mask,
            shift: shift,
            associated_register: PhantomData,
        }
    }

    #[inline]
    pub fn read(self, val: T) -> T {
        (val & (self.mask << self.shift)) >> self.shift
    }

    #[inline]
    /// Check if one or more bits in a field are set
    pub fn is_set(self, val: T) -> bool {
        val & (self.mask << self.shift) != T::zero()
    }

    #[inline]
    /// Read value of the field as an enum member
    pub fn read_as_enum<E: TryFromValue<T, EnumType = E>>(self, val: T) -> Option<E> {
        E::try_from(self.read(val))
    }
}

// #[derive(Copy, Clone)] won't work here because it will use
// incorrect bounds, as a result of using a PhantomData over the
// generic R. The PhantomData<R> implements Copy regardless of whether
// R does, but the #[derive(Copy, Clone)] generates
//
//    #[automatically_derived]
//    #[allow(unused_qualifications)]
//    impl<T: ::core::marker::Copy + IntLike,
//         R: ::core::marker::Copy + RegisterLongName>
//            ::core::marker::Copy for Field<T, R> {}
//
// , so Field will only implement Copy if R: Copy.
//
// Manually implementing Clone and Copy works around this issue.
//
// Relevant Rust issue: https://github.com/rust-lang/rust/issues/26925
impl<T: IntLike, R: RegisterLongName> Clone for Field<T, R> {
    fn clone(&self) -> Self {
        Field {
            mask: self.mask,
            shift: self.shift,
            associated_register: self.associated_register,
        }
    }
}
impl<T: IntLike, R: RegisterLongName> Copy for Field<T, R> {}

macro_rules! Field_impl_for {
    ($type:ty) => {
        impl<R: RegisterLongName> Field<$type, R> {
            pub fn val(&self, value: $type) -> FieldValue<$type, R> {
                FieldValue::<$type, R>::new(self.mask, self.shift, value)
            }
        }
    };
}

Field_impl_for!(u8);
Field_impl_for!(u16);
Field_impl_for!(u32);
Field_impl_for!(u64);
Field_impl_for!(u128);
Field_impl_for!(usize);

/// Values for the specific register fields.
///
/// For the FieldValue, the masks and values are shifted into their actual
/// location in the register.
#[derive(Copy, Clone)]
pub struct FieldValue<T: IntLike, R: RegisterLongName> {
    mask: T,
    pub value: T,
    associated_register: PhantomData<R>,
}

macro_rules! FieldValue_impl_for {
    ($type:ty) => {
        // Necessary to split the implementation of new() out because the bitwise
        // math isn't treated as const when the type is generic.
        // Tracking issue: https://github.com/rust-lang/rfcs/pull/2632
        impl<R: RegisterLongName> FieldValue<$type, R> {
            pub const fn new(mask: $type, shift: usize, value: $type) -> Self {
                FieldValue {
                    mask: mask << shift,
                    value: (value & mask) << shift,
                    associated_register: PhantomData,
                }
            }
        }

        // Necessary to split the implementation of From<> out because of the orphan rule
        // for foreign trait implementation (see [E0210](https://doc.rust-lang.org/error-index.html#E0210)).
        impl<R: RegisterLongName> From<FieldValue<$type, R>> for $type {
            fn from(val: FieldValue<$type, R>) -> $type {
                val.value
            }
        }
    };
}

FieldValue_impl_for!(u8);
FieldValue_impl_for!(u16);
FieldValue_impl_for!(u32);
FieldValue_impl_for!(u64);
FieldValue_impl_for!(u128);
FieldValue_impl_for!(usize);

impl<T: IntLike, R: RegisterLongName> FieldValue<T, R> {
    /// Get the raw bitmask represented by this FieldValue.
    #[inline]
    pub fn mask(&self) -> T {
        self.mask as T
    }

    #[inline]
    pub fn read(&self, field: Field<T, R>) -> T {
        field.read(self.value)
    }

    /// Modify fields in a register value
    #[inline]
    pub fn modify(self, val: T) -> T {
        (val & !self.mask) | self.value
    }

    /// Check if any specified parts of a field match
    #[inline]
    pub fn matches_any(&self, val: T) -> bool {
        val & self.mask != T::zero()
    }

    /// Check if all specified parts of a field match
    #[inline]
    pub fn matches_all(&self, val: T) -> bool {
        val & self.mask == self.value
    }
}

// Combine two fields with the addition operator
impl<T: IntLike, R: RegisterLongName> Add for FieldValue<T, R> {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        FieldValue {
            mask: self.mask | rhs.mask,
            value: self.value | rhs.value,
            associated_register: PhantomData,
        }
    }
}

// Combine two fields with the += operator
impl<T: IntLike, R: RegisterLongName> AddAssign for FieldValue<T, R> {
    #[inline]
    fn add_assign(&mut self, rhs: FieldValue<T, R>) {
        self.mask |= rhs.mask;
        self.value |= rhs.value;
    }
}

/// Conversion of raw register value into enumerated values member.
/// Implemented inside register_bitfields! macro for each bit field.
pub trait TryFromValue<V> {
    type EnumType;

    fn try_from(v: V) -> Option<Self::EnumType>;
}

/// Helper macro for computing bitmask of variable number of bits
#[macro_export]
macro_rules! bitmask {
    ($numbits:expr) => {
        (1 << ($numbits - 1)) + ((1 << ($numbits - 1)) - 1)
    };
}

/// Helper macro for defining register fields.
#[macro_export]
macro_rules! register_bitmasks {
    {
        // BITFIELD_NAME OFFSET(x)
        $(#[$outer:meta])*
        $valtype:ident, $reg_desc:ident, [
            $( $(#[$inner:meta])* $field:ident OFFSET($offset:expr)),+ $(,)?
        ]
    } => {
        $(#[$outer])*
        $( $crate::register_bitmasks!($valtype, $reg_desc, $(#[$inner])* $field, $offset, 1, []); )*
    };
    {
        // BITFIELD_NAME OFFSET
        // All fields are 1 bit
        $(#[$outer:meta])*
        $valtype:ident, $reg_desc:ident, [
            $( $(#[$inner:meta])* $field:ident $offset:expr ),+ $(,)?
        ]
    } => {
        $(#[$outer])*
        $( $crate::register_bitmasks!($valtype, $reg_desc, $(#[$inner])* $field, $offset, 1, []); )*
    };

    {
        // BITFIELD_NAME OFFSET(x) NUMBITS(y)
        $(#[$outer:meta])*
        $valtype:ident, $reg_desc:ident, [
            $( $(#[$inner:meta])* $field:ident OFFSET($offset:expr) NUMBITS($numbits:expr) ),+ $(,)?
        ]
    } => {
        $(#[$outer])*
        $( $crate::register_bitmasks!($valtype, $reg_desc, $(#[$inner])* $field, $offset, $numbits, []); )*
    };

    {
        // BITFIELD_NAME OFFSET(x) NUMBITS(y) []
        $(#[$outer:meta])*
        $valtype:ident, $reg_desc:ident, [
            $( $(#[$inner:meta])* $field:ident OFFSET($offset:expr) NUMBITS($numbits:expr)
               $values:tt ),+ $(,)?
        ]
    } => {
        $(#[$outer])*
        $( $crate::register_bitmasks!($valtype, $reg_desc, $(#[$inner])* $field, $offset, $numbits,
                              $values); )*
    };
    {
        $valtype:ident, $reg_desc:ident, $(#[$outer:meta])* $field:ident,
                    $offset:expr, $numbits:expr,
                    [$( $(#[$inner:meta])* $valname:ident = $value:expr ),+ $(,)?]
    } => { // this match arm is duplicated below with an allowance for 0 elements in the valname -> value array,
        // to seperately support the case of zero-variant enums not supporting non-default
        // representations.
        #[allow(non_upper_case_globals)]
        #[allow(unused)]
        pub const $field: Field<$valtype, $reg_desc> =
            Field::<$valtype, $reg_desc>::new($crate::bitmask!($numbits), $offset);

        #[allow(non_snake_case)]
        #[allow(unused)]
        $(#[$outer])*
        pub mod $field {
            #[allow(unused_imports)]
            use $crate::fields::{TryFromValue, FieldValue};
            use super::$reg_desc;

            $(
            #[allow(non_upper_case_globals)]
            #[allow(unused)]
            $(#[$inner])*
            pub const $valname: FieldValue<$valtype, $reg_desc> =
                FieldValue::<$valtype, $reg_desc>::new($crate::bitmask!($numbits),
                    $offset, $value);
            )*

            #[allow(non_upper_case_globals)]
            #[allow(unused)]
            pub const SET: FieldValue<$valtype, $reg_desc> =
                FieldValue::<$valtype, $reg_desc>::new($crate::bitmask!($numbits),
                    $offset, $crate::bitmask!($numbits));

            #[allow(non_upper_case_globals)]
            #[allow(unused)]
            pub const CLEAR: FieldValue<$valtype, $reg_desc> =
                FieldValue::<$valtype, $reg_desc>::new($crate::bitmask!($numbits),
                    $offset, 0);

            #[allow(dead_code)]
            #[allow(non_camel_case_types)]
            #[repr($valtype)] // so that values larger than isize::MAX can be stored
            $(#[$outer])*
            pub enum Value {
                $(
                    $(#[$inner])*
                    $valname = $value,
                )*
            }

            impl TryFromValue<$valtype> for Value {
                type EnumType = Value;

                fn try_from(v: $valtype) -> Option<Self::EnumType> {
                    match v {
                        $(
                            $(#[$inner])*
                            x if x == Value::$valname as $valtype => Some(Value::$valname),
                        )*

                        _ => Option::None
                    }
                }
            }
        }
    };
    {
        $valtype:ident, $reg_desc:ident, $(#[$outer:meta])* $field:ident,
                    $offset:expr, $numbits:expr,
                    []
    } => { //same pattern as previous match arm, for 0 elements in array. Removes code associated with array.
        #[allow(non_upper_case_globals)]
        #[allow(unused)]
        pub const $field: Field<$valtype, $reg_desc> =
            Field::<$valtype, $reg_desc>::new($crate::bitmask!($numbits), $offset);

        #[allow(non_snake_case)]
        #[allow(unused)]
        $(#[$outer])*
        pub mod $field {
            #[allow(unused_imports)]
            use $crate::fields::{FieldValue, TryFromValue};
            use super::$reg_desc;

            #[allow(non_upper_case_globals)]
            #[allow(unused)]
            pub const SET: FieldValue<$valtype, $reg_desc> =
                FieldValue::<$valtype, $reg_desc>::new($crate::bitmask!($numbits),
                    $offset, $crate::bitmask!($numbits));

            #[allow(non_upper_case_globals)]
            #[allow(unused)]
            pub const CLEAR: FieldValue<$valtype, $reg_desc> =
                FieldValue::<$valtype, $reg_desc>::new($crate::bitmask!($numbits),
                    $offset, 0);

            #[allow(dead_code)]
            #[allow(non_camel_case_types)]
            $(#[$outer])*
            pub enum Value {}

            impl TryFromValue<$valtype> for Value {
                type EnumType = Value;

                fn try_from(_v: $valtype) -> Option<Self::EnumType> {
                    Option::None
                }
            }
        }
    };
}

/// Define register types and fields.
#[macro_export]
macro_rules! register_bitfields {
    {
        $valtype:ident, $( $(#[$inner:meta])* $vis:vis $reg:ident $fields:tt ),* $(,)?
    } => {
        $(
            #[allow(non_snake_case)]
            $(#[$inner])*
            $vis mod $reg {
                // Visibility note: This is left always `pub` as it is not
                // meaningful to restrict access to the `Register` element of
                // the register module if the module itself is in scope
                //
                // (if you can access $reg, you can access $reg::Register)
                #[derive(Clone, Copy)]
                pub struct Register;
                impl $crate::RegisterLongName for Register {}

                use $crate::fields::Field;

                $crate::register_bitmasks!( $valtype, Register, $fields );
            }
        )*
    }
}