use crate::cache::Cache;
use std::cell::RefCell;

pub(crate) struct CacheableRead<T, R: Clone> {
	read_fn: fn(&T) -> R,
	cache: RefCell<Option<R>>,
}
impl<T, R: Clone> CacheableRead<T, R> {
	pub(crate) fn new(read_fn: fn(&T) -> R) -> Self {
		Self {
			read_fn,
			cache: RefCell::new(None),
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
	fn notify(&mut self, _: &T) {
		self.cache.replace(None);
	}
}
