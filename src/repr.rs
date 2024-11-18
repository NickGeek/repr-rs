#[cfg(feature = "eager")]
use crate::cache::eager;
use crate::cache::{lazy, Cache};
use std::borrow::Cow;
use std::cell::{Ref, RefCell};
use std::collections::BTreeMap;
#[cfg(feature = "eager")]
use std::future::Future;
use std::ops::Deref;

/// Wraps a value and ensures that an invariant is maintained while allowing that value to be
/// mutated. The invariant is checked after every mutation.
/// 
/// Additionally, this struct allows for cacheable reads of the value. This is useful when the
/// read function is expensive. By default, the caching is lazy, so after a value is read once that
/// same read function will fetch the cached value unless the value has been mutated.
/// 
/// With the feature `eager` enabled, the [EagerCacheLookup] trait is implemented for this struct
/// and can be used to cache values eagerly. Whenever the value is mutated, all eager caches
/// will be updated in parallel.
pub struct Repr<T, I: Fn(&T) -> bool> {
	inner: RefCell<T>,
	invariant: I,
	caches: BTreeMap<usize, Box<dyn Cache<T>>>,
	eager_caches: BTreeMap<usize, Box<dyn Cache<T>>>,
	violation_message: Cow<'static, str>,
}
impl<T: 'static, I: Fn(&T) -> bool> Repr<T, I> {
	/// Creates a new Repr with the given value and invariant function.
	/// ```rust
	/// use repr_rs::Repr;
	/// struct MinMax { min: i32, max: i32 }
	/// Repr::new(
	///   MinMax { min: 1, max: 5 }, 
	///   |mm| mm.min < mm.max,
	/// );
	/// ```
	pub fn new(inner: T, invariant: I) -> Self {
		Self {
			inner: RefCell::new(inner),
			invariant,
			caches: Default::default(),
			eager_caches: Default::default(),
			violation_message: Cow::Borrowed("Invariant violated"),
		}
	}
	/// Creates a new Repr with the given value, invariant function, and violation message.
	/// ```rust
	/// use repr_rs::Repr;
	/// struct MinMax { min: i32, max: i32 }
	/// Repr::with_msg(
	///   MinMax { min: 1, max: 5 }, 
	///   |mm| mm.min < mm.max,
	///   "min must be less than max".into(),
	/// );
	/// ```
	pub fn with_msg(inner: T, invariant: I, violation_message: Cow<'static, str>) -> Self {
		Self {
			inner: RefCell::new(inner),
			invariant,
			caches: Default::default(),
			eager_caches: Default::default(),
			violation_message,
		}
	}
	/// Borrows a read-only view of the value in the representation invariant.
	/// ```rust
	/// use repr_rs::Repr;
	/// struct MinMax { min: i32, max: i32 }
	/// let repr = Repr::new(MinMax { min: 1, max: 5 }, |mm| mm.min < mm.max);
	/// let view = repr.borrow();
	/// assert_eq!(1, view.min);
	/// assert_eq!(5, view.max);
	/// ```
	pub fn borrow(&self) -> ReprView<T> {
		ReprView { inner: self.inner.borrow() }
	}
	/// Mutates the value in the representation invariant. A mutable view of the representation
	/// invariant is provided as an argument to the mutator closure.
	/// ```rust
	/// use repr_rs::Repr;
	/// struct MinMax { min: i32, max: i32 }
	/// let mut repr = Repr::new(MinMax { min: 1, max: 5 }, |mm| mm.min < mm.max);
	/// {
	///   let view = repr.borrow();
	///   assert_eq!(1, view.min);
	///   assert_eq!(5, view.max);
	/// }
	/// repr.mutate(|mm| mm.min = 4);
	/// let view = repr.borrow();
	/// assert_eq!(4, view.min);
	/// assert_eq!(5, view.max);
	/// ```
	/// 
	/// Rust's borrowing rules prevent the read-only view being held while a mutation occurs. For
	/// example, this won't compile:
	/// ```compile_fail
	/// use repr_rs::Repr;
	/// struct MinMax { min: i32, max: i32 }
	/// let mut repr = Repr::new(MinMax { min: 1, max: 5 }, |mm| mm.min < mm.max);
	/// let view = repr.borrow();
	/// assert_eq!(1, view.min);
	/// assert_eq!(5, view.max);
	/// repr.mutate(|mm| mm.min = 4);
	/// // error[E0502]: cannot borrow `repr` as mutable because it is also borrowed as immutable
	/// assert_eq!(4, view.min);
	/// assert_eq!(5, view.max);
	/// ```
	pub fn mutate<F: FnOnce(&mut T)>(&mut self, f: F) {
		let mut borrow = self.inner.borrow_mut();
		f(&mut borrow);
		let data = borrow.deref();
		assert!((self.invariant)(data), "{}", self.violation_message);
		// In debug mode
		for _ in 0..10 {
			debug_assert!((self.invariant)(data), "Invariants should be deterministic! The invariant function for this Repr is not deterministic.");
		}
		for cache in self.caches.values_mut().chain(self.eager_caches.values_mut()) {
			cache.notify(data);
		}
	}
	/// Borrows a read-only view of the value in the representation invariant and caches the
	/// result of the read function. The cache is keyed by the read function's address, so in general
	/// you should use function references instead of closures. It is a bug to perform any side-effects
	/// in the read function (i.e. reading from a file).
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
	fn eager<R: Clone + Clone + Sync + Send + 'static>(&mut self, read_fn: fn(&T) -> R) -> impl Future<Output=R>;
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
