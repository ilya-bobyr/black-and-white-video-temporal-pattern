//! Grey levels can be represented using multiple different patterns and sequences.
//!
//! This module help define them.

use std::borrow::Cow;

use clap::{ValueEnum, builder::PossibleValue};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Pattern {
    /// Regular (non-random) pattern that uses 2 frames, and 2 by 2 grid in each frame.
    Regular2f2w2h,
}

impl Pattern {
    /// Number of frames this pattern processes as a single group.
    ///
    /// For patterns that do not group frames it should return 1.
    pub(crate) fn frame_group_size(&self) -> u32 {
        match self {
            Pattern::Regular2f2w2h => 2,
        }
    }

    /// Width of a single cell this pattern uses.
    pub(crate) fn width(&self) -> u32 {
        match self {
            Pattern::Regular2f2w2h => 2,
        }
    }

    /// Height of a single cell this pattern uses.
    pub(crate) fn height(&self) -> u32 {
        match self {
            Pattern::Regular2f2w2h => 2,
        }
    }
}

impl ValueEnum for Pattern {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Regular2f2w2h]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            Pattern::Regular2f2w2h => PossibleValue::new("r2f2w2h"),
        })
    }

    fn from_str(input: &str, ignore_case: bool) -> Result<Self, String> {
        let input = if ignore_case {
            Cow::Owned(input.to_ascii_lowercase())
        } else {
            Cow::Borrowed(input)
        };
        match input.as_ref() {
            "r2f2w2h" | "regular-2f2w2h" => Ok(Self::Regular2f2w2h),
            _ => Err(format!("invalid pattern value: {input}")),
        }
    }
}

impl ToString for Pattern {
    fn to_string(&self) -> String {
        match self {
            Pattern::Regular2f2w2h => "r2f2w2h".to_owned(),
        }
    }
}
