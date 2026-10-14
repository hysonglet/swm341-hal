//! `embedded-hal` trait implementations for the GPIO driver.
//!
//! Version 1.0 lives at the top level; the 0.2.7 interfaces that the wider
//! ecosystem still asks for are collected in [`v027`].

use super::{Input, Output, Pin};
use crate::mode::Mode;

impl<'d, P: Pin> embedded_hal::digital::ErrorType for Output<'d, P> {
    type Error = core::convert::Infallible;
}

impl<'d, P: Pin> embedded_hal::digital::OutputPin for Output<'d, P> {
    #[inline]
    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.inner.set_low();
        Ok(())
    }

    #[inline]
    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.inner.set_high();
        Ok(())
    }
}

impl<'d, P: Pin> embedded_hal::digital::StatefulOutputPin for Output<'d, P> {
    #[inline]
    fn toggle(&mut self) -> Result<(), Self::Error> {
        self.inner.toggle();
        Ok(())
    }

    #[inline]
    fn is_set_high(&mut self) -> Result<bool, Self::Error> {
        Ok(self.inner.is_high())
    }

    #[inline]
    fn is_set_low(&mut self) -> Result<bool, Self::Error> {
        Ok(self.inner.is_low())
    }
}

impl<'d, P: Pin, M: Mode> embedded_hal::digital::ErrorType for Input<'d, P, M> {
    type Error = core::convert::Infallible;
}

impl<'d, P: Pin, M: Mode> embedded_hal::digital::InputPin for Input<'d, P, M> {
    #[inline]
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(self.inner.is_high())
    }

    #[inline]
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(self.inner.is_low())
    }
}

#[cfg(feature = "embassy")]
impl<'d, P: Pin> embedded_hal_async::digital::Wait for Input<'d, P, crate::mode::Async> {
    async fn wait_for_high(&mut self) -> Result<(), Self::Error> {
        self.wait_for_high().await;
        Ok(())
    }

    async fn wait_for_low(&mut self) -> Result<(), Self::Error> {
        self.wait_for_low().await;
        Ok(())
    }

    async fn wait_for_rising_edge(&mut self) -> Result<(), Self::Error> {
        self.wait_for_rising_edge().await;
        Ok(())
    }

    async fn wait_for_falling_edge(&mut self) -> Result<(), Self::Error> {
        self.wait_for_falling_edge().await;
        Ok(())
    }

    async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {
        self.wait_for_any_edge().await;
        Ok(())
    }
}

/// `embedded-hal` 0.2.7 compatibility layer.
pub mod v027 {
    use super::*;

    impl<'d, P: Pin> embedded_hal_027::digital::v2::OutputPin for Output<'d, P> {
        type Error = core::convert::Infallible;

        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.set_low();
            Ok(())
        }

        fn set_high(&mut self) -> Result<(), Self::Error> {
            self.set_high();
            Ok(())
        }
    }

    impl<'d, P: Pin> embedded_hal_027::digital::v2::ToggleableOutputPin for Output<'d, P> {
        type Error = core::convert::Infallible;

        fn toggle(&mut self) -> Result<(), Self::Error> {
            self.toggle();
            Ok(())
        }
    }

    impl<'d, P: Pin> embedded_hal_027::digital::v2::StatefulOutputPin for Output<'d, P> {
        fn is_set_low(&self) -> Result<bool, Self::Error> {
            Ok(self.inner.is_low())
        }

        fn is_set_high(&self) -> Result<bool, Self::Error> {
            Ok(self.inner.is_high())
        }
    }

    impl<'d, P: Pin, M: Mode> embedded_hal_027::digital::v2::InputPin for Input<'d, P, M> {
        type Error = core::convert::Infallible;

        fn is_low(&self) -> Result<bool, Self::Error> {
            Ok(self.inner.is_low())
        }

        fn is_high(&self) -> Result<bool, Self::Error> {
            Ok(self.inner.is_high())
        }
    }
}
