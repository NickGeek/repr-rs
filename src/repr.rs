use std::cell::{Ref, UnsafeCell};
use std::fmt::{Debug, Display};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

/// Wraps a value and ensures that an invariant is maintained while allowing that value to be
/// mutated. The invariant is checked after every mutation.
/// See [`crate::CacheableRepr`] for a version of this struct that supports caching.
pub struct Repr<T: Debug, I: Fn(&T) -> bool> {
	pub(crate) inner: UnsafeCell<T>,
	invariant: I,
	violation_message: &'static str,
}
impl<T: Debug, I: Fn(&T) -> bool> Repr<T, I> {
	/// Creates a new representation invariant with the given value and invariant function.
	/// ```rust
	/// use repr_rs::Repr;
	/// struct MinMax { min: i32, max: i32 }
	/// Repr::new(
	///   MinMax { min: 1, max: 5 },
	///   |mm| mm.min < mm.max,
	/// );
	/// ```
	pub const fn new(inner: T, invariant: I) -> Self {
		Self {
			inner: UnsafeCell::new(inner),
			invariant,
			violation_message: "Invariant violated",
		}
	}
	/// Creates a new representation invariant with the given value, invariant function, and violation message.
	/// ```rust
	/// use repr_rs::Repr;
	/// struct MinMax { min: i32, max: i32 }
	/// Repr::with_msg(
	///   MinMax { min: 1, max: 5 },
	///   |mm| mm.min < mm.max,
	///   "min must be less than max",
	/// );
	/// ```
	pub const fn with_msg(inner: T, invariant: I, violation_message: &'static str) -> Self {
		Self {
			inner: UnsafeCell::new(inner),
			invariant,
			violation_message,
		}
	}
	/// Borrows a read-only view of the value in the representation invariant.
	/// ```rust
	/// use repr_rs::Repr;
	/// struct MinMax { min: i32, max: i32 }
	/// let repr = Repr::new(MinMax { min: 1, max: 5 }, |mm| mm.min < mm.max);
	/// let view = repr.read();
	/// assert_eq!(1, view.min);
	/// assert_eq!(5, view.max);
	/// ```
	#[inline]
	pub fn read(&self) -> &T {
		// Safety: borrowing rules ensure that T is valid, and because this is an immutable borrow
		// of the Repr, no mutable borrows can take place.
		unsafe { &*self.inner.get() }
	}
	/// Borrows a mutable view of the value in the representation invariant.
	/// ```rust
	/// use repr_rs::Repr;
	/// struct MinMax { min: i32, max: i32 }
	/// let mut repr = Repr::new(MinMax { min: 1, max: 5 }, |mm| mm.min < mm.max);
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
	/// use repr_rs::Repr;
	/// struct MinMax { min: i32, max: i32 }
	/// let mut repr = Repr::new(MinMax { min: 1, max: 5 }, |mm| mm.min < mm.max);
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
	pub(crate) fn check(&mut self) {
		let data = self.inner.get_mut();
		assert!((self.invariant)(data), "{}\nState was: {:?}", self.violation_message, data);
		// In debug mode
		for _ in 0..10 {
			debug_assert!((self.invariant)(data), "Invariants should be deterministic! The invariant function for this Repr is not deterministic.");
		}
	}
}

impl<T: Debug + Clone, I: Fn(&T) -> bool + Clone> Clone for Repr<T, I> {
	fn clone(&self) -> Self {
		let inner = self.read().clone();
		Self::with_msg(inner, self.invariant.clone(), self.violation_message)
	}
}
impl<T: Debug + Hash, I: Fn(&T) -> bool> Hash for Repr<T, I> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.read().hash(state);
	}
}
impl<T: Debug + PartialEq, I: Fn(&T) -> bool> PartialEq for Repr<T, I> {
	fn eq(&self, other: &Self) -> bool {
		self.read() == other.read()
	}
}
impl<T: Debug + Eq, I: Fn(&T) -> bool> Eq for Repr<T, I> {}

impl<T: Debug, I: Fn(&T) -> bool> Debug for Repr<T, I> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Repr({:?})", self.read())
	}
}
impl <T: Debug + Display, I: Fn(&T) -> bool> Display for Repr<T, I> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.read())
	}
}

#[repr(transparent)]
pub struct ReprView<'a, T> {
	inner: Ref<'a, T>,
}
impl<'a, T> Deref for ReprView<'a, T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

#[repr(transparent)]
pub struct ReprMutator<'a, T: Debug, I: Fn(&T) -> bool> {
	// inner: &'a mut T,
	repr: &'a mut Repr<T, I>,
}
impl<'a, T: Debug, I: Fn(&T) -> bool> Deref for ReprMutator<'a, T, I> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		// Safety: borrowing rules ensure that T is valid, and because ReprMutate mutably borrows
		// the Repr, no mutable borrows of the inner can take place if we borrow it as imm here.
		unsafe { &*self.repr.inner.get() }
	}
}
impl<'a, T: Debug, I: Fn(&T) -> bool> DerefMut for ReprMutator<'a, T, I> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.repr.inner.get_mut()
	}
}
impl<T: Debug, I: Fn(&T) -> bool> Drop for ReprMutator<'_, T, I> {
	fn drop(&mut self) {
		self.repr.check();
	}
}
