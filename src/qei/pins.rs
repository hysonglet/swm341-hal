//! QEI pad functions of the SWM341 pin matrix.
//!
//! The QEI signals are all on port D (datasheet V1.33 table 5-4 and the
//! `PORTD_SEL0` field descriptions), each at function code `1`:
//!
//! | pad  | code  | function   | signal                |
//! |------|-------|------------|-----------------------|
//! | PD4  | 0001  | `QEA_IN`   | encoder phase A input |
//! | PD3  | 0001  | `QEB_IN`   | encoder phase B input |
//! | PD2  | 0001  | `INDEX_IN` | index (Z) phase input |
//! | PD7  | 0001  | `UPDN_OUT` | count direction output|
//!
//! `scripts/gen_pin_func.py` does not cover QEI (the vendor `SWM341_port.h`
//! token set stops short of it), so this table is transcribed by hand from the
//! datasheet. Code `0` (generic GPIO) is omitted.
//!
//! [`crate::qei::Qei::new`] takes the A/B/index pads and muxes them for you, so
//! reaching for these variants directly is only needed for `QeiUpdn` (PD7, the
//! optional direction output, which the constructor does not claim). In that
//! case apply a variant with [`crate::gpio::set_function`] after acquiring the
//! pad with [`crate::gpio::Flex::new`], which also enables the input buffer.

/// Generate the per-pin `Pin{n}Func` enums for one bank module.
///
/// Each entry is `PinType => Enum { Variant = func_code, ... }`; the macro
/// writes the `#[repr(u8)]` enum plus its `PinFunc` / `PinFuncOf` impls.
macro_rules! pad_funcs {
    ( $( $pin_ty:ident => $enum:ident { $( $var:ident = $val:literal ),* $(,)? } ),* $(,)? ) => {
        $(
            #[derive(Clone, Copy, PartialEq, Eq, Debug)]
            #[repr(u8)]
            pub enum $enum {
                $( $var = $val ),*
            }
            impl crate::gpio::PinFunc for $enum {
                #[inline]
                fn value(self) -> u8 { self as u8 }
            }
            impl crate::gpio::PinFuncOf<crate::gpio::$pin_ty> for $enum {}
        )*
    };
}

pub mod d {
    pad_funcs! {
        PD2 => Pin2Func { QeiIndex = 1 },
        PD3 => Pin3Func { QeiB = 1 },
        PD4 => Pin4Func { QeiA = 1 },
        PD7 => Pin7Func { QeiUpdn = 1 },
    }
}
