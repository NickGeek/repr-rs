// pub(crate) struct Cache {
// 	// inner: HashMap<usize, >
// }

use downcast_rs::{impl_downcast, Downcast};
use std::cell::RefCell;
use std::marker::PhantomData;
// #[derive(Clone)]
// pub(crate) struct CacheableRead<T, R, F: Fn(&T) -> R> {
// 	read_fn: F,
// 	cache: Option<R>,
// 	_arg: PhantomData<T>,
// }

pub(crate) trait Cache<T>: Downcast {
	fn invalidate(&mut self);
}
impl_downcast!(Cache<T>);

pub(crate) struct CacheableRead<T, R: Clone> {
	read_fn: fn(&T) -> R,
	cache: RefCell<Option<R>>,
	_arg: PhantomData<T>,
}
impl<T, R: Clone> CacheableRead<T, R> {
	pub(crate) fn new(read_fn: fn(&T) -> R) -> Self {
		Self {
			read_fn,
			cache: RefCell::new(None),
			_arg: PhantomData,
		}
	}
	pub(crate) fn read(&self, arg: &T) -> R {
		if let Some(cached) = self.cache.borrow().as_ref() {
			return cached.clone();
		}
		let result = (self.read_fn)(arg);
		self.cache.replace(Some(result.clone()));
		result
	}
}
impl<T: 'static, R: Clone + 'static> Cache<T> for CacheableRead<T, R> {
	fn invalidate(&mut self) {
		self.cache.replace(None);
		// self.cache.store(&mut None, Ordering::Relaxed);
	}
}
