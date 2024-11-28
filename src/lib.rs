// TODO: when this is stable, we can make `read` and `write` const-fns.
// #![feature(const_mut_refs)]

pub mod repr;
pub mod cache;

#[cfg(feature = "eager")]
pub use cache::eager::EagerCacheLookup;
pub use cache::CacheableRepr;
pub use repr::Repr;

#[cfg(test)]
mod tests {
	use std::borrow::Cow;
	use crate::repr::Repr;
	use crate::CacheableRepr;
	use std::cell::RefCell;
	use std::collections::HashMap;
	use std::rc::Rc;
	use std::sync::{Arc};
	use tokio::sync::RwLock;

	#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
	struct MinMax {
		min: i32,
		max: i32,
	}

	#[test]
	fn reading() {
		let repr = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		assert_eq!(repr.read().min, 1);
		assert_eq!(repr.read().max, 5);
	}

	#[test]
	fn reading_as_ref() {
		let repr = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		fn get_min<M: AsRef<MinMax>>(x: M) -> i32 { x.as_ref().min }
		assert_eq!(get_min(&repr), 1);
		assert_eq!(repr.read().max, 5);
	}

	#[test]
	fn allowed_mutation() {
		let mut repr = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		let a = repr.read().min;
		repr.write().min = 4;
		assert_eq!(4, repr.read().min);
		assert_eq!(1, a);
	}

	#[test]
	#[should_panic]
	fn should_propagate_panic() {
		let mut repr = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| {
				if mm.max <= 5 {
					mm.min < mm.max
				} else {
					panic!("random panic")
				}
			},
		);
		let a = repr.read().min;
		{
			repr.write().min = 4;
			assert_eq!(4, repr.read().min);
			assert_eq!(1, a);
		}
		repr.write().max = 10;
	}

	#[test]
	#[should_panic]
	fn banned_mutation() {
		let mut repr = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		repr.write().min = 6;
	}

	#[test]
	#[should_panic]
	fn should_try_to_detect_non_deterministic_invariants() {
		let value = Rc::new(RefCell::new(true));
		let mut repr = Repr::new(
			value.clone(),
			|_| {
				let res = *value.borrow();
				value.replace(false);
				res
			},
		);
		repr.write();
	}

	#[test]
	#[should_panic]
	fn banned_mutation_with_msg() {
		let mut repr = Repr::with_msg(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
			"min must always be less than max!",
		);
		repr.write().min = 6;
	}
	
	#[test]
	fn should_work_with_borrowed_data() {
		#[derive(Debug)]
		struct Person<'a> {
			name: Cow<'a, str>,
			age: u8,
		}
		impl Person<'_> {
			fn is_valid(&self) -> bool {
				!self.name.is_empty() && self.age < 200
			}
		}
		let bob = String::from("Bob");
		let mut repr = Repr::with_msg(
			Person {
				name: bob.as_str().into(),
				age: 25,
			},
			Person::is_valid,
			"People must have a name and cannot be older than 200",
		);
		{
			let person = repr.read();
			assert_eq!("Bob", person.name);
			assert_eq!(25, person.age);
		}
		{
			let mut person = repr.write();
			person.name = "Alice".into();
			person.age = 30;
		}
		{
			let person = repr.read();
			assert_eq!("Alice", person.name);
			assert_eq!(30, person.age);
		}
	}
	#[test]
	#[should_panic]
	fn should_work_with_borrowed_data_doing_a_bad_mutation() {
		#[derive(Debug)]
		struct Person<'a> {
			name: Cow<'a, str>,
			age: u8,
		}
		impl Person<'_> {
			fn is_valid(&self) -> bool {
				!self.name.is_empty() && self.age < 200
			}
		}
		let bob = String::from("Bob");
		let mut repr = Repr::with_msg(
			Person {
				name: bob.as_str().into(),
				age: 25,
			},
			Person::is_valid,
			"People must have a name and cannot be older than 200",
		);
		{
			let person = repr.read();
			assert_eq!("Bob", person.name);
			assert_eq!(25, person.age);
		}
		{
			let mut person = repr.write();
			person.name = "Alice".into();
			person.age = 200;
		}
		{
			let person = repr.read();
			assert_eq!("Alice", person.name);
			assert_eq!(30, person.age);
		}
	}

	#[test]
	fn should_read_from_cache() {
		let mut repr = CacheableRepr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		fn get_min(mm: &MinMax) -> i32 { mm.min }
		assert_eq!(1, repr.lazy(get_min));
		assert_eq!(1, repr.lazy(get_min));
	}
	#[test]
	fn should_invalidate_cache_on_mutation() {
		let mut repr = CacheableRepr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		fn get_min(mm: &MinMax) -> i32 {
			mm.min
		}
		assert_eq!(1, repr.lazy(get_min));
		assert_eq!(1, repr.lazy(get_min));
		repr.write().min = 4;
		assert_eq!(4, repr.lazy(get_min));
		assert_eq!(4, repr.lazy(get_min));
	}

	#[test]
	fn should_allow_static_closures_for_cache_reads() {
		let mut repr = CacheableRepr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		assert_eq!(1, repr.lazy(|mm| mm.min));
		assert_eq!(1, repr.lazy(|mm| mm.min));
		repr.write().min = 4;
		assert_eq!(4, repr.lazy(|mm| mm.min));
		assert_eq!(4, repr.lazy(|mm| mm.min));
	}

	#[test]
	fn should_hash_by_inner() {
		let mut repr1 = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		let mut repr2 = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		let mut map = HashMap::new();
		let repr1_w: &mut MinMax = &mut repr1.write();
		let repr2_w: &mut MinMax = &mut repr2.write();
		map.insert(&repr1_w, 1);
		assert!(map.contains_key(&repr1_w));
		assert!(map.contains_key(&repr2_w));
		repr2_w.max = 10;
		assert!(map.contains_key(&repr1_w));
		assert!(!map.contains_key(&repr2_w));
	}

	#[test]
	fn should_be_moveable_across_threads() {
		let mut repr = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		fn get_min(mm: &MinMax) -> i32 { mm.min }
		fn get_max(mm: &MinMax) -> i32 { mm.max }

		{
			let mm = repr.as_ref();
			assert_eq!(1, get_min(mm));
			assert_eq!(5, get_max(mm));
		}
		{
			let mut mm = repr.write();
			mm.max = 100;
			mm.min = 50;
		}
		std::thread::spawn(move || {
			let mm = repr.as_ref();
			assert_eq!(50, get_min(mm));
			assert_eq!(100, get_max(mm));
		}).join().unwrap();
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn should_be_moveable_across_tasks() {
		let mut repr = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		fn get_min(mm: &MinMax) -> i32 { mm.min }
		fn get_max(mm: &MinMax) -> i32 { mm.max }

		{
			let mm = repr.as_ref();
			assert_eq!(1, get_min(mm));
			assert_eq!(5, get_max(mm));
		}
		{
			let mut mm = repr.write();
			mm.max = 100;
			mm.min = 50;
		}
		tokio::spawn(async move {
			let mm = repr.as_ref();
			assert_eq!(50, get_min(mm));
			assert_eq!(100, get_max(mm));
		}).await.unwrap();
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn should_be_shareable_across_threads() {
		let mut repr = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		fn get_min(mm: &MinMax) -> i32 { mm.min }
		fn get_max(mm: &MinMax) -> i32 { mm.max }

		{
			let mm = repr.as_ref();
			assert_eq!(1, get_min(mm));
			assert_eq!(5, get_max(mm));
		}
		{
			let mut mm = repr.write();
			mm.max = 100;
			mm.min = 50;
		}
		std::thread::scope(|s| {
			s.spawn(|| {
				let mm = repr.as_ref();
				assert_eq!(50, get_min(mm));
				assert_eq!(100, get_max(mm));
			});
		});
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn should_work_with_shared_ownership_and_rwlock() {
		let repr = Arc::new(RwLock::new(Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		)));
		fn get_min(mm: &MinMax) -> i32 { mm.min }
		fn get_max(mm: &MinMax) -> i32 { mm.max }

		{
			let lock = repr.read().await;
			let mm = lock.read();
			assert_eq!(1, get_min(mm));
			assert_eq!(5, get_max(mm));
		}
		{
			let mut lock = repr.write().await;
			let mut mm = lock.write();
			mm.max = 100;
			mm.min = 50;
		}
		let r = repr.clone();
		tokio::spawn(async move {
			let mut lock = r.write().await;
			let mut mm = lock.write();
			assert_eq!(50, get_min(&mm));
			assert_eq!(100, get_max(&mm));
			mm.min = 10;
			mm.max = 20;
		}).await.unwrap();
		{
			let lock = repr.read().await;
			let mm = lock.read();
			assert_eq!(10, get_min(mm));
			assert_eq!(20, get_max(mm));
		}
	}

	#[cfg(feature = "eager")]
	mod eager {
		use std::sync::atomic::{AtomicU32, Ordering};
		use std::time::Duration;
		use crate::tests::MinMax;
		use crate::{CacheableRepr, EagerCacheLookup};

		#[tokio::test(flavor = "multi_thread")]
		async fn should_read_from_cache() {
			let mut repr = CacheableRepr::new(
				MinMax { min: 1, max: 5 },
				|mm| mm.min < mm.max,
			);
			fn get_min(mm: &MinMax) -> i32 {
				mm.min
			}
			assert_eq!(1, repr.eager(get_min).await);
			assert_eq!(1, repr.eager(get_min).await);
			// repr.borrow_mut()n = 4;
		}
		#[tokio::test(flavor = "multi_thread")]
		async fn should_invalidate_cache_on_mutation() {
			let mut repr = CacheableRepr::new(
				MinMax { min: 1, max: 5 },
				|mm| mm.min < mm.max,
			);
			fn get_min(mm: &MinMax) -> i32 {
				mm.min
			}
			assert_eq!(1, repr.eager(get_min).await);
			assert_eq!(1, repr.eager(get_min).await);
			repr.write().min = 4;
			assert_eq!(4, repr.eager(get_min).await);
			assert_eq!(4, repr.eager(get_min).await);
		}

		#[tokio::test(flavor = "multi_thread")]
		async fn should_allow_static_closures_for_cache_reads() {
			let mut repr = CacheableRepr::new(
				MinMax { min: 1, max: 5 },
				|mm| mm.min < mm.max,
			);
			assert_eq!(1, repr.eager(|mm| mm.min).await);
			assert_eq!(1, repr.eager(|mm| mm.min).await);
			repr.write().min = 4;
			assert_eq!(4, repr.eager(|mm| mm.min).await);
			assert_eq!(4, repr.eager(|mm| mm.min).await);
		}

		#[ignore]
		#[tokio::test(flavor = "multi_thread")]
		async fn should_work_with_expensive_computations() {
			let mut repr = CacheableRepr::new(
				MinMax { min: 1, max: 40 },
				|mm| mm.min < mm.max,
			);

			fn fib(n: u64) -> u64 {
				if n <= 1 {
					return n;
				}
				fib(n - 1) + fib(n - 2)
			}
			fn plain_fib(mm: &MinMax) -> u64 { fib(mm.max as u64) }
			fn plus2(mm: &MinMax) -> u64 { fib((mm.max + 2) as u64) }
			fn plus3(mm: &MinMax) -> u64 { fib((mm.max + 3) as u64) }
			fn plus4(mm: &MinMax) -> u64 { fib((mm.max + 4) as u64) }
			fn plus5(mm: &MinMax) -> u64 { fib((mm.max + 5) as u64) }

			assert_eq!(102334155, repr.eager(plain_fib).await);
			assert_eq!(267914296, repr.eager(plus2).await);
			assert_eq!(433494437, repr.eager(plus3).await);
			assert_eq!(701408733, repr.eager(plus4).await);
			assert_eq!(1134903170, repr.eager(plus5).await);
			repr.write().max = 42;
			assert_eq!(267914296, repr.eager(plain_fib).await);
		}

		#[tokio::test(flavor = "multi_thread")]
		async fn should_be_able_to_unregister_caches() {
			let mut repr = CacheableRepr::new(
				MinMax { min: 1, max: 5 },
				|mm| mm.min < mm.max,
			);
			fn get_min(mm: &MinMax) -> i32 {
				mm.min
			}
			fn get_min2(mm: &MinMax) -> i32 {
				mm.min
			}
			assert_eq!(1, repr.eager(get_min).await);
			assert_eq!(1, repr.eager(get_min2).await);
			assert_eq!(1, repr.eager(get_min).await);
			assert!(repr.unregister(get_min2));
		}

		#[ignore]
		#[tokio::test(flavor = "multi_thread")]
		#[should_panic]
		async fn should_propagate_panic_in_eager_cache() {
			let mut repr = CacheableRepr::new(
				MinMax { min: 1, max: 5 },
				|mm| mm.min < mm.max,
			);
			fn get_min(mm: &MinMax) -> i32 {
				mm.min
			}
			fn get_min2(mm: &MinMax) -> i32 {
				if mm.min == 1 {
					mm.min
				} else {
					panic!("random panic")
				}
			}
			assert_eq!(1, repr.eager(get_min).await);
			assert_eq!(1, repr.eager(get_min2).await);
			assert_eq!(1, repr.eager(get_min).await);
			repr.write().min = 2;
		}

		#[tokio::test(flavor = "multi_thread")]
		async fn counting_eager_cache_hits() {
			#[derive(Debug, Clone)]
			struct Person { name: String }
			let mut repr = CacheableRepr::new(Person { name: "Alice and Bob together at last".into() }, |p| !p.name.is_empty());
			static READ_SPY: AtomicU32 = AtomicU32::new(0);
			fn expensive_read(p: &Person) -> usize {
			  // Just for demonstration purposes.
			  // Do not do side effects in your read functions!
			  READ_SPY.fetch_add(1, Ordering::Relaxed);
			  fib(p.name.len())
			}
			let fib_of_name_len = repr.eager(expensive_read).await;
			assert_eq!(832040, fib_of_name_len);
			// this does not recompute the fibonacci number, it just gets it from the cache!
			let fib_of_name_len2 = repr.eager(expensive_read).await;
			assert_eq!(832040, fib_of_name_len2);
			assert_eq!(1, READ_SPY.load(Ordering::Relaxed));
			repr.write().name = "Alice".into();
			// if we wait a bit we can see that a new value has been computed
			tokio::time::sleep(Duration::from_millis(100)).await;
			assert_eq!(2, READ_SPY.load(Ordering::Relaxed));
			// Now when we fetch it again, we should see the new value without needing to recompute it
			let fib_of_name_len3 = repr.eager(expensive_read).await;
			assert_eq!(5, fib_of_name_len3);
			assert_eq!(2, READ_SPY.load(Ordering::Relaxed));
			fn fib(n: usize) -> usize {
			  if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
			}
		}
	}
}
