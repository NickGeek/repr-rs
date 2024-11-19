mod repr;
mod cache;

#[cfg(feature = "eager")]
pub use repr::EagerCacheLookup;
// Re-exports
pub use repr::Repr;
pub use repr::ReprView;

#[cfg(test)]
mod tests {
	use crate::repr::Repr;
	use std::cell::RefCell;
	use std::rc::Rc;

	#[derive(Debug, Copy, Clone)]
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
		assert_eq!(repr.borrow().min, 1);
		assert_eq!(repr.borrow().max, 5);
	}

	#[test]
	fn allowed_mutation() {
		let mut repr = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		let a = repr.borrow().min;
		repr.borrow_mut().min = 4;
		assert_eq!(4, repr.borrow().min);
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
		let a = repr.borrow().min;
		{
			repr.borrow_mut().min = 4;
			assert_eq!(4, repr.borrow().min);
			assert_eq!(1, a);
		}
		repr.borrow_mut().max = 10;
	}

	#[test]
	#[should_panic]
	fn banned_mutation() {
		let mut repr = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		repr.borrow_mut().min = 6;
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
		repr.borrow_mut();
	}

	#[test]
	#[should_panic]
	fn banned_mutation_with_msg() {
		let mut repr = Repr::with_msg(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
			"min must always be less than max!".into(),
		);
		repr.borrow_mut().min = 6;
	}

	#[test]
	fn should_read_from_cache() {
		let mut repr = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		fn get_min(mm: &MinMax) -> i32 { mm.min }
		assert_eq!(1, repr.lazy(get_min));
		assert_eq!(1, repr.lazy(get_min));
	}
	#[test]
	fn should_invalidate_cache_on_mutation() {
		let mut repr = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		fn get_min(mm: &MinMax) -> i32 {
			mm.min
		}
		assert_eq!(1, repr.lazy(get_min));
		assert_eq!(1, repr.lazy(get_min));
		repr.borrow_mut().min = 4;
		assert_eq!(4, repr.lazy(get_min));
		assert_eq!(4, repr.lazy(get_min));
	}

	#[test]
	fn should_allow_static_closures_for_cache_reads() {
		let mut repr = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		assert_eq!(1, repr.lazy(|mm| mm.min));
		assert_eq!(1, repr.lazy(|mm| mm.min));
		repr.borrow_mut().min = 4;
		assert_eq!(4, repr.lazy(|mm| mm.min));
		assert_eq!(4, repr.lazy(|mm| mm.min));
	}

	#[cfg(feature = "eager")]
	mod eager {
		use crate::repr::{EagerCacheLookup, Repr};
		use crate::tests::MinMax;

		#[tokio::test(flavor = "multi_thread")]
		async fn should_read_from_cache() {
			let mut repr = Repr::new(
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
			let mut repr = Repr::new(
				MinMax { min: 1, max: 5 },
				|mm| mm.min < mm.max,
			);
			fn get_min(mm: &MinMax) -> i32 {
				mm.min
			}
			assert_eq!(1, repr.eager(get_min).await);
			assert_eq!(1, repr.eager(get_min).await);
			repr.borrow_mut().min = 4;
			assert_eq!(4, repr.eager(get_min).await);
			assert_eq!(4, repr.eager(get_min).await);
		}

		#[tokio::test(flavor = "multi_thread")]
		async fn should_allow_static_closures_for_cache_reads() {
			let mut repr = Repr::new(
				MinMax { min: 1, max: 5 },
				|mm| mm.min < mm.max,
			);
			assert_eq!(1, repr.eager(|mm| mm.min).await);
			assert_eq!(1, repr.eager(|mm| mm.min).await);
			repr.borrow_mut().min = 4;
			assert_eq!(4, repr.eager(|mm| mm.min).await);
			assert_eq!(4, repr.eager(|mm| mm.min).await);
		}

		#[ignore]
		#[tokio::test(flavor = "multi_thread")]
		async fn should_work_with_expensive_computations() {
			let mut repr = Repr::new(
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
			repr.borrow_mut().max = 42;
			assert_eq!(267914296, repr.eager(plain_fib).await);
		}

		#[tokio::test(flavor = "multi_thread")]
		async fn should_be_able_to_unregister_caches() {
			let mut repr = Repr::new(
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
			let mut repr = Repr::new(
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
			repr.borrow_mut().min = 2;
		}
	}
}
