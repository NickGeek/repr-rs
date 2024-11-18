use crate::cache::{Cache, CacheableRead};
use hashbrown::HashMap;
use std::cell::{Ref, RefCell};
use std::ops::Deref;

pub struct Repr<T, I: Fn(&T) -> bool> {
	inner: RefCell<T>,
	invariant: I,
	caches: HashMap<usize, Box<dyn Cache<T>>>
}
impl<T: 'static, I: Fn(&T) -> bool> Repr<T, I> {
	pub fn new(inner: T, invariant: I) -> Self {
		Self {
			inner: RefCell::new(inner),
			invariant,
			caches: Default::default(),
		}
	}
	pub fn borrow(&self) -> ReprView<T> {
		ReprView { inner: self.inner.borrow() }
	}
	pub fn mutate<F: FnOnce(&mut T)>(&mut self, f: F) {
		let mut borrow = self.inner.borrow_mut();
		f(&mut borrow);
		assert!((self.invariant)(borrow.deref()), "Invariant violated");
		for cache in self.caches.values_mut() {
			cache.invalidate();
		}
	}
	pub fn lazy<R: Clone + 'static>(&mut self, read_fn: fn(&T) -> R) -> R {
		let fn_identity = &raw const read_fn as usize;
		let entry = self.caches.entry(fn_identity);

		let cache = entry.or_insert_with(|| Box::new(CacheableRead::<T, R>::new(read_fn)));
		let cache = cache.downcast_mut::<CacheableRead<T, R>>().unwrap();
		cache.read(&*self.inner.borrow())
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
