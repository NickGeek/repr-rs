use std::cell::{Ref, RefCell};
use std::ops::Deref;

pub struct Repr<T, I: Fn(&T) -> bool> {
	inner: RefCell<T>,
	invariant: I,
}
impl<T, I: Fn(&T) -> bool> Repr<T, I> {
	pub fn new(inner: T, invariant: I) -> Self {
		Self {
			inner: RefCell::new(inner),
			invariant,
		}
	}
	pub fn borrow(&self) -> ReprView<T> {
		ReprView { inner: self.inner.borrow() }
	}
	pub fn mutate<F: FnOnce(&mut T)>(&mut self, f: F) {
		let mut borrow = self.inner.borrow_mut();
		f(&mut borrow);
		assert!((self.invariant)(borrow.deref()), "Invariant violated");
	}
}

pub struct ReprView<'a, T> {
	inner: Ref<'a, T>,
}
impl<'a, T> Deref for ReprView<'a, T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}
