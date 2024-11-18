mod repr;

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
		repr.mutate(|mm| mm.min = 4);
		assert_eq!(repr.borrow().min, 4);
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
}
