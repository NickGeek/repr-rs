pub(crate) mod lazy;
#[cfg(feature = "eager")]
pub(crate) mod eager;

use crate::Repr;
use downcast_rs::{impl_downcast, Downcast};
use std::collections::BTreeMap;
use std::fmt::{Debug, Display};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

pub(crate) trait Cache<T>: Downcast {
	fn notify(&self, _value: &T);
}
impl_downcast!(Cache<T>);

/// Wraps a value and ensures that an invariant is maintained while allowing that value to be
/// mutated. The invariant is checked after every mutation.
/// Additionally, this struct allows for cacheable reads of the value. This is useful when the
/// read function is expensive. By default, the caching is lazy, so after a value is read once that
/// same read function will fetch the cached value unless the value has been mutated.
///
/// With the feature `eager` enabled, the [`crate::EagerCacheLookup`] trait is implemented for this struct
/// and can be used to cache values eagerly. Whenever the value is mutated, all eager caches
/// will be updated in parallel.
/// 
/// This struct requires that the value has a `'static` lifetime. If you need to store a value
/// with a non-static lifetime consider using [`Repr`].
pub struct CacheableRepr<T: Debug + 'static, I: Fn(&T) -> bool> {
	inner: Repr<T, I>,
	caches: BTreeMap<usize, Box<dyn Cache<T>>>,
	eager_caches: BTreeMap<usize, Box<dyn Cache<T>>>,
}
impl<T: Debug + 'static, I: Fn(&T) -> bool> CacheableRepr<T, I> {
	/// Creates a new representation invariant with the given value and invariant function.
	/// ```rust
	/// use repr_rs::CacheableRepr;
	/// struct MinMax { min: i32, max: i32 }
	/// CacheableRepr::new(
	///   MinMax { min: 1, max: 5 },
	///   |mm| mm.min < mm.max,
	/// );
	/// ```
	pub const fn new(inner: T, invariant: I) -> Self {
		let repr = Repr::new(inner, invariant);
		Self {
			caches: BTreeMap::new(),
			eager_caches: BTreeMap::new(),
			inner: repr,
		}
	}
	/// Creates a new representation invariant with the given value, invariant function, and violation message.
	/// ```rust
	/// use repr_rs::CacheableRepr;
	/// struct MinMax { min: i32, max: i32 }
	/// CacheableRepr::with_msg(
	///   MinMax { min: 1, max: 5 },
	///   |mm| mm.min < mm.max,
	///   "min must be less than max",
	/// );
	/// ```
	pub const fn with_msg(inner: T, invariant: I, violation_message: &'static str) -> Self {
		let repr = Repr::with_msg(inner, invariant, violation_message);
		Self {
			caches: BTreeMap::new(),
			eager_caches: BTreeMap::new(),
			inner: repr,
		}
	}
	/// Borrows a read-only view of the value in the representation invariant.
	/// ```rust
	/// use repr_rs::CacheableRepr;
	/// struct MinMax { min: i32, max: i32 }
	/// let repr = CacheableRepr::new(MinMax { min: 1, max: 5 }, |mm| mm.min < mm.max);
	/// let view = repr.read();
	/// assert_eq!(1, view.min);
	/// assert_eq!(5, view.max);
	/// ```
	#[inline]
	pub fn read(&self) -> &T {
		// Safety: borrowing rules ensure that T is valid, and because this is an immutable borrow
		// of the Repr, no mutable borrows can take place.
		self.inner.read()
	}
	/// Borrows a mutable view of the value in the representation invariant.
	/// ```rust
	/// use repr_rs::CacheableRepr;
	/// struct MinMax { min: i32, max: i32 }
	/// let mut repr = CacheableRepr::new(MinMax { min: 1, max: 5 }, |mm| mm.min < mm.max);
	/// {
	///   let view = repr.read();
	///   assert_eq!(1, view.min);
	///   assert_eq!(5, view.max);
	/// }
	/// repr.write().min = 4;
	/// let view = repr.read();
	/// assert_eq!(4, view.min);
	/// assert_eq!(5, view.max);
	/// ```
	///
	/// Rust's borrowing rules prevent the read-only view being held while a mutation occurs. For
	/// example, this won't compile:
	/// ```compile_fail
	/// use repr_rs::CacheableRepr;
	/// struct MinMax { min: i32, max: i32 }
	/// let mut repr = CacheableRepr::new(MinMax { min: 1, max: 5 }, |mm| mm.min < mm.max);
	/// let view = repr.borrow();
	/// assert_eq!(1, view.min);
	/// assert_eq!(5, view.max);
	/// // error[E0502]: cannot borrow `repr` as mutable because it is also borrowed as immutable
	/// repr.borrow_mut().min = 4;
	/// assert_eq!(4, view.min);
	/// assert_eq!(5, view.max);
	/// ```
	#[inline]
	pub fn write(&mut self) -> ReprMutator<T, I> {
		// Can be `const` when `const_mut_refs` is stabilised.
		ReprMutator {
			repr: self,
		}
	}
	/// Consumes the representation invariant and returns the inner value.
	/// ```rust
	/// use repr_rs::Repr;
	/// struct MinMax { min: i32, max: i32 }
	/// let repr = Repr::new(MinMax { min: 1, max: 5 }, |mm| mm.min < mm.max);
	/// let inner = repr.into_inner();
	/// assert_eq!(1, inner.min);
	/// ```
	#[inline]
	pub fn into_inner(self) -> T {
		self.inner.into_inner()
	}
	/// Borrows a read-only view of the value in the representation invariant and caches the
	/// result of the read function. The cache is keyed by the read function's address, so in general
	/// you should use function references instead of closures. It is a bug to perform any side effects
	/// in the read function (i.e. reading from a file).
	/// ```rust
	/// use std::sync::atomic::{AtomicU32, Ordering};
	/// use repr_rs::CacheableRepr;
	/// struct Person { name: String }
	/// let mut repr = CacheableRepr::new(Person { name: "Alice and Bob together at last".into() }, |p| !p.name.is_empty());
	/// static READ_SPY: AtomicU32 = AtomicU32::new(0);
	/// fn expensive_read(p: &Person) -> usize {
	///   // Just for demonstration purposes.
	///   // Do not do side effects in your read functions!
	///   READ_SPY.fetch_add(1, Ordering::Relaxed);
	///   fib(p.name.len())
	/// }
	/// let fib_of_name_len = repr.lazy(expensive_read);
	/// assert_eq!(832040, fib_of_name_len);
	/// // this does not recompute the fibonacci number, it just gets it from the cache!
	/// let fib_of_name_len2 = repr.lazy(expensive_read);
	/// assert_eq!(832040, fib_of_name_len2);
	/// repr.write().name = "Alice".into();
	/// // this recomputes the fibonacci number because the name has changed
	/// let fib_of_name_len3 = repr.lazy(expensive_read);
	/// assert_eq!(5, fib_of_name_len3);
	/// assert_eq!(2, READ_SPY.load(Ordering::Relaxed));
	/// # fn fib(n: usize) -> usize {
	/// #   if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
	/// # }
	pub fn lazy<R: Clone + 'static>(&mut self, read_fn: fn(&T) -> R) -> R {
		let fn_identity = read_fn as *const fn(&T) -> R as usize;
		let entry = self.caches.entry(fn_identity);

		let cache = entry.or_insert_with(|| Box::new(lazy::CacheableRead::<T, R>::new(read_fn)));
		let cache = cache.downcast_mut::<lazy::CacheableRead<T, R>>().unwrap();
		let data = self.inner.inner.get_mut();
		cache.read(data)
	}

	fn check(&mut self) {
		self.inner.check();
		let data = self.inner.inner.get_mut();
		for cache in self.caches.values().chain(self.eager_caches.values()) {
			cache.notify(data);
		}
	}
}
impl<T: Debug + 'static, I: Fn(&T) -> bool> From<Repr<T, I>> for CacheableRepr<T, I> {
	fn from(value: Repr<T, I>) -> Self {
		Self {
			caches: BTreeMap::new(),
			eager_caches: BTreeMap::new(),
			inner: value,
		}
	}
}
impl<T: Debug + 'static, I: Fn(&T) -> bool> From<CacheableRepr<T, I>> for Repr<T, I> {
	fn from(value: CacheableRepr<T, I>) -> Self {
		value.inner
	}
}
impl<T: Debug + Clone, I: Fn(&T) -> bool + Clone> Clone for CacheableRepr<T, I> {
	fn clone(&self) -> Self {
		let clone = self.inner.clone();
		Self::from(clone)
	}
}
impl<T: Debug + Hash, I: Fn(&T) -> bool> Hash for CacheableRepr<T, I> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.inner.hash(state);
	}
}
impl<T: Debug + PartialEq, I: Fn(&T) -> bool> PartialEq for CacheableRepr<T, I> {
	fn eq(&self, other: &Self) -> bool {
		self.inner.eq(&other.inner)
	}
}
impl<T: Debug + Eq, I: Fn(&T) -> bool> Eq for CacheableRepr<T, I> {}

impl<T: Debug, I: Fn(&T) -> bool> Debug for CacheableRepr<T, I> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Repr({:?})", self.read())
	}
}
impl <T: Debug + Display, I: Fn(&T) -> bool> Display for CacheableRepr<T, I> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.read())
	}
}

#[repr(transparent)]
pub struct ReprMutator<'a, T: Debug + 'static, I: Fn(&T) -> bool> {
	// inner: &'a mut T,
	repr: &'a mut CacheableRepr<T, I>,
}
impl<'a, T: Debug, I: Fn(&T) -> bool> Deref for ReprMutator<'a, T, I> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		// Safety: borrowing rules ensure that T is valid, and because ReprMutate mutably borrows
		// the Repr, no mutable borrows of the inner can take place if we borrow it as imm here.
		unsafe { &*self.repr.inner.inner.get() }
	}
}
impl<'a, T: Debug, I: Fn(&T) -> bool> DerefMut for ReprMutator<'a, T, I> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.repr.inner.inner.get_mut()
	}
}
impl<T: Debug, I: Fn(&T) -> bool> Drop for ReprMutator<'_, T, I> {
	fn drop(&mut self) {
		self.repr.check();
	}
}
