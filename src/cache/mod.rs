pub(crate) mod lazy;
#[cfg(feature = "eager")]
pub(crate) mod eager;

use downcast_rs::{impl_downcast, Downcast};

pub(crate) trait Cache<T>: Downcast {
	fn notify(&self, _value: &T);
}
impl_downcast!(Cache<T>);
