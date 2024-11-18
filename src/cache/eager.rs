use crate::cache::Cache;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::task::{spawn_blocking, JoinHandle};

pub(crate) struct CacheableRead<T, R: Clone + Sync + Send> {
	read_fn: fn(&T) -> R,
	cache: Arc<RwLock<Option<R>>>,
}
impl<T: Clone + Sync + Send + 'static, R: Clone + Sync + Send + 'static> CacheableRead<T, R> {
	pub(crate) fn new(read_fn: fn(&T) -> R) -> Self {
		Self {
			read_fn,
			cache: Default::default(),
		}
	}
	pub(crate) fn read(&self, arg: &T) -> R {
		let res = self.cache.read().unwrap();
		if let Some(cached) = res.as_ref() {
			return cached.clone();
		}
		(self.read_fn)(arg)
	}
	
	pub(crate) fn update(&mut self, value: &T) -> JoinHandle<()> {
		let mut writer = self.cache.write().unwrap();
		*writer = None;
		let cell = self.cache.clone();
		let read_fn = self.read_fn;
		let value = value.clone();
		spawn_blocking(move || {
			let value = value;
			let mut writer = cell.write().unwrap();
			*writer = Some(read_fn(&value));
		})
	}
}
impl<T: 'static + Sync + Send + Clone, R: Clone + 'static + Send + Sync> Cache<T> for CacheableRead<T, R> {
	fn notify(&mut self, value: &T) {
		self.update(value);
	}
}
