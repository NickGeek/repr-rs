#[cfg(feature = "eager")]
use crate::cache::eager;
use crate::cache::{lazy, Cache};
use std::borrow::Cow;
use std::cell::{Ref, RefCell};
use std::collections::BTreeMap;
use std::ops::Deref;

pub struct Repr<T, I: Fn(&T) -> bool> {
	inner: RefCell<T>,
	invariant: I,
	caches: BTreeMap<usize, Box<dyn Cache<T>>>,
	eager_caches: BTreeMap<usize, Box<dyn Cache<T>>>,
	violation_message: Cow<'static, str>,
}
impl<T: 'static, I: Fn(&T) -> bool> Repr<T, I> {
	pub fn new(inner: T, invariant: I) -> Self {
		Self {
			inner: RefCell::new(inner),
			invariant,
			caches: Default::default(),
			eager_caches: Default::default(),
			violation_message: Cow::Borrowed("Invariant violated"),
		}
	}
	pub fn with_msg(inner: T, invariant: I, violation_message: Cow<'static, str>) -> Self {
		Self {
			inner: RefCell::new(inner),
			invariant,
			caches: Default::default(),
			eager_caches: Default::default(),
			violation_message,
		}
	}
	pub fn borrow(&self) -> ReprView<T> {
		ReprView { inner: self.inner.borrow() }
	}
	pub fn mutate<F: FnOnce(&mut T)>(&mut self, f: F) {
		let mut borrow = self.inner.borrow_mut();
		f(&mut borrow);
		let data = borrow.deref();
		assert!((self.invariant)(data), "{}", self.violation_message);
		for cache in self.caches.values_mut().chain(self.eager_caches.values_mut()) {
			cache.notify(data);
		}
	}
	pub fn lazy<R: Clone + 'static>(&mut self, read_fn: fn(&T) -> R) -> R {
		let fn_identity = read_fn as *const fn(&T) -> R as usize;
		let entry = self.caches.entry(fn_identity);

		let cache = entry.or_insert_with(|| Box::new(lazy::CacheableRead::<T, R>::new(read_fn)));
		let cache = cache.downcast_mut::<lazy::CacheableRead<T, R>>().unwrap();
		let data = &*self.inner.borrow();
		cache.read(data)
	}
}

#[cfg(feature = "eager")]
pub trait EagerCacheLookup<T: Clone + Sync + Send + 'static, I: Fn(&T) -> bool> {
	async fn eager<R: Clone + Clone + Sync + Send + 'static>(&mut self, read_fn: fn(&T) -> R) -> R;
	fn unregister<R: Clone + Clone + Sync + Send + 'static>(&mut self, read_fn: fn(&T) -> R) -> bool;
}
#[cfg(feature = "eager")]
impl<T: Clone + Sync + Send + 'static, I: Fn(&T) -> bool> EagerCacheLookup<T, I> for Repr<T, I> {
	#[allow(clippy::await_holding_refcell_ref)] // safe because the &mut self on this fn prevents other borrows
	async fn eager<R: Clone + Sync + Send + 'static>(&mut self, read_fn: fn(&T) -> R) -> R {
		let fn_identity = read_fn as *const fn(&T) -> R as usize;
		let is_empty = self.eager_caches.contains_key(&fn_identity);
		let entry = self.eager_caches.entry(fn_identity);

		let cache = entry.or_insert_with(|| Box::new(eager::CacheableRead::<T, R>::new(read_fn)));
		let cache = cache.downcast_mut::<eager::CacheableRead<T, R>>().unwrap();
		let data = &*self.inner.borrow();
		if is_empty {
			cache.update(data).await.unwrap();
		}
		cache.read(data)
	}
	fn unregister<R: Clone + Clone + Sync + Send + 'static>(&mut self, read_fn: fn(&T) -> R) -> bool {
		let fn_identity = read_fn as *const fn(&T) -> R as usize;
		self.eager_caches.remove(&fn_identity).is_some()
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
