use crate::Mii;

const EXPRESSION_COUNT: usize = 70;

/// Facial expression rendered on a Mii.
///
/// Variants through [`Frustrated`](Self::Frustrated) use the names exposed by
/// FFL. Later variants use descriptive AFL/Miitomo names where those are known.
/// Neutral `Afl` names are retained where no reliable internal name is known.
#[derive(
	Clone, Copy, Debug, Default, Eq, num_enum::IntoPrimitive, num_enum::TryFromPrimitive, PartialEq,
)]
#[repr(u8)]
pub enum Expression {
	#[default]
	Normal = 0,
	Smile = 1,
	Anger = 2,
	Sorrow = 3,
	Surprise = 4,
	Blink = 5,
	OpenMouth = 6,
	SmileOpenMouth = 7,
	AngerOpenMouth = 8,
	SorrowOpenMouth = 9,
	SurpriseOpenMouth = 10,
	BlinkOpenMouth = 11,
	WinkLeft = 12,
	WinkRight = 13,
	WinkLeftOpenMouth = 14,
	WinkRightOpenMouth = 15,
	LikeWinkLeft = 16,
	LikeWinkRight = 17,
	Frustrated = 18,
	Bored = 19,
	BoredOpenMouth = 20,
	SighMouthStraight = 21,
	Sigh = 22,
	DisgustedMouthStraight = 23,
	Disgusted = 24,
	Love = 25,
	LoveOpenMouth = 26,
	DeterminedMouthStraight = 27,
	Determined = 28,
	CryMouthStraight = 29,
	Cry = 30,
	BigSmileMouthStraight = 31,
	BigSmile = 32,
	Cheeky = 33,
	CheekyDuplicate = 34,
	/// AFL expression 35, whose internal name is unknown.
	Afl35 = 35,
	/// AFL expression 36, whose internal name is unknown.
	Afl36 = 36,
	Smug = 37,
	SmugOpenMouth = 38,
	Resolve = 39,
	ResolveOpenMouth = 40,
	Unbelievable = 41,
	UnbelievableDuplicate = 42,
	Cunning = 43,
	CunningDuplicate = 44,
	Raspberry = 45,
	RaspberryDuplicate = 46,
	Innocent = 47,
	InnocentDuplicate = 48,
	Cat = 49,
	CatDuplicate = 50,
	Dog = 51,
	DogDuplicate = 52,
	Tasty = 53,
	TastyDuplicate = 54,
	MoneyMouthStraight = 55,
	Money = 56,
	ConfusedMouthStraight = 57,
	Confused = 58,
	CheerfulMouthStraight = 59,
	Cheerful = 60,
	/// AFL expression 61, which produces a blank expression mask.
	Afl61 = 61,
	/// AFL expression 62, which produces a duplicate blank expression mask.
	Afl62 = 62,
	GrumbleMouthStraight = 63,
	Grumble = 64,
	MovedMouthStraight = 65,
	Moved = 66,
	SingingMouthSmall = 67,
	Singing = 68,
	Stunned = 69,
}

#[derive(Clone, Copy)]
struct Parts {
	eye_right: u8,
	eye_left: u8,
	mouth: u8,
	eyebrow: u8,
}

#[derive(Clone, Copy)]
struct Correction {
	eye_type: i8,
	mouth_type: i8,
	eye_rotate: i8,
	eyebrow_rotate: i8,
	eyebrow_y: i8,
}

const PARTS: [Parts; EXPRESSION_COUNT] = [
	parts(0, 0, 0, 0),
	parts(1, 1, 0, 0),
	parts(0, 0, 1, 0),
	parts(2, 2, 2, 0),
	parts(3, 3, 0, 0),
	parts(4, 4, 0, 0),
	parts(0, 0, 3, 0),
	parts(1, 1, 3, 0),
	parts(0, 0, 3, 0),
	parts(2, 2, 3, 0),
	parts(3, 3, 3, 0),
	parts(4, 4, 3, 0),
	parts(5, 0, 0, 0),
	parts(0, 5, 0, 0),
	parts(5, 0, 3, 0),
	parts(0, 5, 3, 0),
	parts(5, 0, 5, 0),
	parts(0, 5, 5, 0),
	parts(5, 5, 2, 0),
	parts(13, 13, 6, 0),
	parts(13, 13, 3, 0),
	parts(4, 4, 6, 0),
	parts(4, 4, 19, 0),
	parts(8, 8, 6, 0),
	parts(8, 8, 15, 0),
	parts(11, 11, 6, 0),
	parts(11, 11, 12, 0),
	parts(0, 0, 6, 0),
	parts(0, 0, 11, 0),
	parts(5, 5, 6, 0),
	parts(5, 5, 19, 0),
	parts(9, 9, 6, 0),
	parts(9, 9, 12, 0),
	parts(8, 24, 18, 0),
	parts(8, 24, 18, 0),
	parts(14, 14, 13, 24),
	parts(14, 14, 14, 24),
	parts(13, 13, 9, 0),
	parts(13, 13, 3, 0),
	parts(18, 18, 16, 27),
	parts(18, 18, 17, 27),
	parts(12, 12, 20, 0),
	parts(12, 12, 20, 0),
	parts(15, 15, 8, 0),
	parts(15, 15, 8, 0),
	parts(8, 8, 21, 0),
	parts(8, 8, 21, 0),
	parts(24, 23, 10, 0),
	parts(24, 23, 10, 0),
	parts(16, 16, 22, 25),
	parts(16, 16, 22, 25),
	parts(17, 17, 23, 26),
	parts(17, 17, 23, 26),
	parts(0, 0, 24, 0),
	parts(0, 0, 24, 0),
	parts(22, 21, 6, 0),
	parts(22, 21, 12, 0),
	parts(20, 19, 6, 0),
	parts(20, 19, 19, 0),
	parts(7, 7, 6, 0),
	parts(7, 7, 12, 0),
	parts(0, 0, 0, 0),
	parts(0, 0, 0, 0),
	parts(0, 0, 6, 13),
	parts(0, 0, 20, 13),
	parts(26, 25, 6, 0),
	parts(26, 25, 12, 0),
	parts(28, 28, 25, 0),
	parts(28, 28, 25, 0),
	parts(27, 27, 7, 0),
];

const CORRECTIONS: [Correction; EXPRESSION_COUNT] = [
	correction(-1, -1, 0, 0, 0),
	correction(60, -1, 0, 0, 0),
	correction(-1, 10, 2, 2, 0),
	correction(-1, 12, -2, -2, 0),
	correction(61, -1, 0, 0, -2),
	correction(26, -1, 0, 0, 0),
	correction(-1, 36, 0, 0, 0),
	correction(60, 36, 0, 0, 0),
	correction(-1, 36, 2, 2, 0),
	correction(-1, 36, -2, -2, 0),
	correction(61, 36, 0, 0, -2),
	correction(26, 36, 0, 0, 0),
	correction(47, -1, 0, 0, 0),
	correction(47, -1, 0, 0, 0),
	correction(47, 36, 0, 0, 0),
	correction(47, 36, 0, 0, 0),
	correction(47, -1, 0, 0, 0),
	correction(47, -1, 0, 0, 0),
	correction(47, 12, 0, 0, 0),
	correction(64, 23, 0, 0, 0),
	correction(64, 36, 0, 0, 0),
	correction(26, 23, -2, -2, -2),
	correction(26, 45, -2, -2, -2),
	correction(47, 23, 2, 2, 0),
	correction(47, 41, 2, 2, 0),
	correction(62, 23, 0, 0, -1),
	correction(62, 38, 0, 0, -1),
	correction(-1, 23, 2, 2, 0),
	correction(-1, 37, 2, 2, 0),
	correction(47, 23, 0, -2, -3),
	correction(47, 45, 0, -2, -3),
	correction(60, 23, 0, 0, 0),
	correction(60, 38, 0, 0, 0),
	correction(75, 44, 0, 0, 0),
	correction(75, 44, 0, 0, 0),
	correction(65, 39, 0, 1, 0),
	correction(65, 40, 0, 1, 0),
	correction(64, 15, 0, 2, 0),
	correction(64, 36, 0, 2, 0),
	correction(69, 42, 0, 0, 2),
	correction(69, 43, 0, 0, 2),
	correction(63, 46, 0, 2, 0),
	correction(63, 46, 0, 2, 0),
	correction(66, 14, 0, 2, 1),
	correction(66, 14, 0, 2, 1),
	correction(47, 47, 1, 0, 0),
	correction(47, 47, 1, 0, 0),
	correction(74, 32, 0, 0, 0),
	correction(74, 32, 0, 0, 0),
	correction(67, 48, 0, 0, -1),
	correction(67, 48, 0, 0, -1),
	correction(68, 49, 0, 0, 0),
	correction(68, 49, 0, 0, 0),
	correction(-1, 50, 0, 0, 0),
	correction(-1, 50, 0, 0, 0),
	correction(72, 23, 0, -2, -3),
	correction(72, 38, 0, -2, -3),
	correction(70, 23, 0, -2, -2),
	correction(70, 45, 0, -2, -2),
	correction(23, 23, 0, -2, 0),
	correction(23, 38, 0, -2, 0),
	correction(-1, -1, 0, 0, 0),
	correction(-1, -1, 0, 0, 0),
	correction(-1, 23, 0, -1, -2),
	correction(-1, 46, 0, -1, -2),
	correction(76, 23, 0, -2, -3),
	correction(76, 38, 0, -2, -3),
	correction(79, 51, 0, 0, 0),
	correction(79, 51, 0, 0, 0),
	correction(78, 2, 0, 0, 0),
];

const fn parts(eye_right: u8, eye_left: u8, mouth: u8, eyebrow: u8) -> Parts {
	Parts {
		eye_right,
		eye_left,
		mouth,
		eyebrow,
	}
}

const fn correction(
	eye_type: i8,
	mouth_type: i8,
	eye_rotate: i8,
	eyebrow_rotate: i8,
	eyebrow_y: i8,
) -> Correction {
	Correction {
		eye_type,
		mouth_type,
		eye_rotate,
		eyebrow_rotate,
		eyebrow_y,
	}
}

pub struct Resolved {
	pub(super) mii: Mii,
	pub(super) eye_right: u8,
	pub(super) eye_left: u8,
	pub(super) eyebrow: u8,
	pub(super) mouth: u8,
}

pub fn resolve(mut mii: Mii, expression: Expression) -> Resolved {
	let original = mii;
	let index = usize::from(u8::from(expression));
	let parts = PARTS[index];
	let correction = CORRECTIONS[index];

	apply_expression_dimensions(&mut mii, expression);
	if correction.mouth_type >= 0 {
		mii.mouth_type = u8::try_from(correction.mouth_type).expect("non-negative mouth type");
	}

	let mut eye_rotate = correction.eye_rotate;
	let corrected_eye = u8::try_from(correction.eye_type).ok();
	if corrected_eye.is_some_and(|eye| eye != mii.eye_type) {
		let expression_offset =
			i8::try_from(eye_rotate_offset(mii.eye_type)).expect("eye offset fits i8");
		let source_offset = i8::try_from(eye_rotate_offset(
			corrected_eye.expect("checked corrected eye"),
		))
		.expect("eye offset fits i8");
		eye_rotate = if correction.eye_type < 62 {
			eye_rotate + expression_offset - source_offset
		} else {
			eye_rotate - expression_offset + source_offset
		};
	}
	if eye_rotate != 0 {
		mii.eye_rotate = add_clamped(mii.eye_rotate, eye_rotate, 7);
	}

	if mii.mouth_type == 48 {
		mii.mouth_scale = 8;
	}
	if expression == Expression::SingingMouthSmall {
		mii.mouth_aspect = if mii.mouth_aspect < 3 {
			0
		} else {
			(mii.mouth_aspect - 3).min(6)
		};
	}
	mii.eyebrow_y = add_signed(mii.eyebrow_y, correction.eyebrow_y);
	if correction.eyebrow_rotate != 0 {
		mii.eyebrow_rotate = add_clamped(mii.eyebrow_rotate, correction.eyebrow_rotate, 11);
	}

	Resolved {
		mii,
		eye_right: eye_index(parts.eye_right, original.eye_type),
		eye_left: eye_index(parts.eye_left, original.eye_type),
		eyebrow: if parts.eyebrow == 0 {
			original.eyebrow_type
		} else {
			parts.eyebrow
		},
		mouth: mouth_index(parts.mouth, original.mouth_type),
	}
}

const fn apply_expression_dimensions(mii: &mut Mii, expression: Expression) {
	match expression as u8 {
		19 | 20 => {
			mii.eye_scale = 4;
			mii.eye_aspect = 3;
			mii.mouth_aspect = 3;
			mii.mouth_scale = 4;
		},
		45 | 46 | 53 | 54 => {
			mii.mouth_aspect = 3;
			mii.mouth_scale = 4;
		},
		25 | 26 | 37 | 38 | 55..=58 => {
			mii.eye_scale = 4;
			mii.eye_aspect = 3;
			mii.eye_rotate = 4;
		},
		33 | 34 => {
			mii.eye_rotate = 4;
			mii.mouth_aspect = 3;
			mii.mouth_scale = 4;
		},
		35 | 36 => {
			mii.eye_rotate = 4;
			mii.eye_aspect = 3;
			mii.eye_scale = 4;
			mii.mouth_scale = 4;
			mii.mouth_aspect = 3;
		},
		39 | 40 => {
			mii.eye_rotate = 4;
			mii.eye_aspect = 3;
			mii.eye_scale = 4;
			mii.eyebrow_rotate = 6;
		},
		43 | 44 | 47 | 48 => mii.eye_rotate = 4,
		49..=52 => {
			mii.eye_rotate = 4;
			mii.eye_aspect = 3;
			mii.eye_scale = 4;
			mii.eyebrow_scale = 4;
			mii.eyebrow_aspect = 3;
			mii.eyebrow_rotate = 6;
			mii.mouth_aspect = 3;
			mii.mouth_scale = 4;
			mii.eye_x = 2;
			mii.eye_y = 12;
			mii.eyebrow_x = 2;
			mii.eyebrow_y = 10;
			mii.mouth_y = 13;
			mii.mustache_type = 0;
		},
		67 | 68 => mii.mouth_aspect = 3,
		69 => mii.eye_aspect = 3,
		_ => {},
	}
}

const fn eye_index(kind: u8, original: u8) -> u8 {
	match kind {
		0 | 2 => original,
		1 | 9 => 60,
		3 | 10 => 61,
		4 => 26,
		5 | 8 => 47,
		6 => 18,
		7 => 23,
		11 => 62,
		12..=28 => 51 + kind,
		_ => 0,
	}
}

const fn mouth_index(kind: u8, original: u8) -> u8 {
	match kind {
		0 => original,
		1 => 10,
		2 => 12,
		3 => 36,
		4 | 5 => 19,
		6 => 23,
		7 => 2,
		8 => 14,
		9 => 15,
		10 => 32,
		11..=25 => 26 + kind,
		_ => 0,
	}
}

pub fn eye_rotate_offset(kind: u8) -> u8 {
	const ROTATE: [u8; 80] = [
		3, 4, 4, 4, 3, 4, 4, 4, 3, 4, 4, 4, 4, 3, 3, 4, 4, 4, 3, 3, 4, 3, 4, 3, 3, 4, 3, 4, 4, 3,
		4, 4, 4, 3, 3, 3, 4, 4, 3, 3, 3, 4, 4, 3, 3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 3, 4, 4, 3,
		4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
	];
	32 - ROTATE[usize::from(kind)]
}

fn add_clamped(value: u8, offset: i8, maximum: i16) -> u8 {
	u8::try_from((i16::from(value) + i16::from(offset)).clamp(0, maximum))
		.expect("clamped expression value fits u8")
}

fn add_signed(value: u8, offset: i8) -> u8 {
	u8::try_from(i16::from(value) + i16::from(offset)).expect("corrected expression value fits u8")
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::MiiData;

	const TEST_MII: &str = "0800600308040402020c0301060406020a0000000008000804000a2a003e7f04000214031304160d04000a040109";

	fn fixture() -> Mii {
		Mii::from_data(&MiiData::decode(TEST_MII).expect("fixture should decode"))
			.expect("fixture should validate")
	}

	#[test]
	fn resolves_standard_expression_parts() {
		let normal = resolve(fixture(), Expression::Normal);
		assert_eq!(normal.eye_right, 2);
		assert_eq!(normal.eye_left, 2);
		assert_eq!(normal.mouth, 22);

		let wink = resolve(fixture(), Expression::WinkLeft);
		assert_eq!(wink.eye_right, 47);
		assert_eq!(wink.eye_left, 2);

		let surprised = resolve(fixture(), Expression::Surprise);
		assert_eq!(surprised.eye_right, 61);
		assert_eq!(surprised.mii.eyebrow_y, fixture().eyebrow_y - 2);
	}

	#[test]
	fn resolves_extended_expression_adjustments() {
		let animal = resolve(fixture(), Expression::Cat);
		assert_eq!(animal.eye_right, 67);
		assert_eq!(animal.mouth, 48);
		assert_eq!(animal.mii.mustache_type, 0);
		assert_eq!(animal.mii.mouth_scale, 8);

		let singing = resolve(fixture(), Expression::SingingMouthSmall);
		assert_eq!(singing.eye_right, 79);
		assert_eq!(singing.mouth, 51);
		assert_eq!(singing.mii.mouth_aspect, 0);
	}
}
