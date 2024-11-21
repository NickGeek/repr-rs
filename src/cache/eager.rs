use std::fmt::Debug;
use crate::cache::{Cache, CacheableRepr};
use std::future::Future;
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
	
	pub(crate) fn update(&self, value: &T) -> JoinHandle<()> {
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
	fn notify(&self, value: &T) {
		self.update(value);
	}
}

#[cfg(feature = "eager")]
pub trait EagerCacheLookup<T: Clone + Sync + Send + 'static, I: Fn(&T) -> bool> {
	fn eager<R: Clone + Clone + Sync + Send + 'static>(&mut self, read_fn: fn(&T) -> R) -> impl Future<Output=R>;
	fn unregister<R: Clone + Clone + Sync + Send + 'static>(&mut self, read_fn: fn(&T) -> R) -> bool;
}
#[cfg(feature = "eager")]
impl<T: Debug + Clone + Sync + Send + 'static, I: Fn(&T) -> bool> EagerCacheLookup<T, I> for CacheableRepr<T, I> {
	/// Borrows a read-only view of the value in the representation invariant and caches the
	/// result of the read function. The cache is keyed by the read function's address, so in general
	/// you should use function references instead of closures. It is a bug to perform any side effects
	/// in the read function (i.e. reading from a file). This cache is updated eagerly, so whenever
	/// the value is mutated, all eager caches will be updated in parallel. See [`Repr::lazy`] for
	/// a lazy version of this function.
	#[allow(clippy::await_holding_refcell_ref)] // safe because the &mut self on this fn prevents other borrows
	async fn eager<R: Clone + Sync + Send + 'static>(&mut self, read_fn: fn(&T) -> R) -> R {
		let fn_identity = read_fn as *const fn(&T) -> R as usize;
		let is_empty = self.eager_caches.contains_key(&fn_identity);
		let entry = self.eager_caches.entry(fn_identity);

		let cache = entry.or_insert_with(|| Box::new(CacheableRead::<T, R>::new(read_fn)));
		let cache = cache.downcast_mut::<CacheableRead<T, R>>().unwrap();
		let data = self.inner.inner.get_mut();
		if is_empty {
			cache.update(data).await.unwrap();
		}
		cache.read(data)
	}
	/// Unregisters an eager cache. Returns true if the cache was found and removed.
	fn unregister<R: Clone + Clone + Sync + Send + 'static>(&mut self, read_fn: fn(&T) -> R) -> bool {
		let fn_identity = read_fn as *const fn(&T) -> R as usize;
		self.eager_caches.remove(&fn_identity).is_some()
	}
}
