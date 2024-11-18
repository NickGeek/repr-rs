mod repr;
mod cache;

#[cfg(test)]
mod tests {
	use crate::repr::Repr;

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
		repr.mutate(|mm| mm.min = 4);
		assert_eq!(4, repr.borrow().min);
		assert_eq!(1, a);
	}

	#[test]
	#[should_panic]
	fn banned_mutation() {
		let mut repr = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		repr.mutate(|mm| mm.min = 6);
	}

	#[test]
	fn should_read_from_cache() {
		let mut repr = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		fn get_min(mm: &MinMax) -> i32 {
			mm.min
		}
		assert_eq!(1, repr.lazy(get_min));
		assert_eq!(1, repr.lazy(get_min));
		// repr.mutate(|mm| mm.min = 4);
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
		repr.mutate(|mm| mm.min = 4);
		assert_eq!(4, repr.lazy(get_min));
	}

	#[test]
	fn should_reject_invalid_cache_lookup_fns() {
		let mut repr = Repr::new(
			MinMax { min: 1, max: 5 },
			|mm| mm.min < mm.max,
		);
		assert_eq!(1, repr.lazy(|mm| mm.min));
		repr.mutate(|mm| mm.min = 4);
		assert_eq!(4, repr.lazy(|mm| mm.min));
	}
}
