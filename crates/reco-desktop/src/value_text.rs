//! How a slider's value reads in its field, and what a typed value or an
//! ↑/↓ step asks for. No Makepad types.

use crate::time_ruler::{clock, parse_clock};

/// How a value reads: a number with its decimals and unit ("75°", "0.05",
/// "2.5 s"), or a clock ("1:30").
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reading {
    Number {
        /// Decimals shown; a step of ↑/↓ is one unit of the last.
        digits: usize,
        /// After the number, as shown ("°", " s"); optional when typed.
        unit: &'static str,
        /// Shown instead of a zero ("Off"), and read as one.
        zero: Option<&'static str>,
    },
    /// Minutes and seconds, hours once there are any; a step is a second.
    Clock,
}

impl Reading {
    pub const fn number(digits: usize, unit: &'static str) -> Self {
        Self::Number {
            digits,
            unit,
            zero: None,
        }
    }

    /// A number that reads `word` at zero.
    pub const fn or_word(digits: usize, unit: &'static str, word: &'static str) -> Self {
        Self::Number {
            digits,
            unit,
            zero: Some(word),
        }
    }

    /// The value as its field shows it.
    pub fn text(self, value: f64) -> String {
        match self {
            Self::Number { digits, unit, zero } => {
                let number = format!("{value:.digits$}");
                match zero {
                    Some(word) if number.parse::<f64>() == Ok(0.0) => word.to_string(),
                    _ => format!("{number}{unit}"),
                }
            }
            Self::Clock => clock(value),
        }
    }

    /// One step of ↑/↓.
    pub fn step(self) -> f64 {
        match self {
            Self::Number { digits, .. } => 10f64.powi(-(digits as i32)),
            Self::Clock => 1.0,
        }
    }

    /// A typed value: the number with or without its unit, a decimal comma
    /// read as a point, the zero word as zero; `None` for anything else.
    pub fn read(self, text: &str) -> Option<f64> {
        let text = text.trim().replace(',', ".");
        match self {
            Self::Number { unit, zero, .. } => {
                if zero.is_some_and(|word| text.eq_ignore_ascii_case(word)) {
                    return Some(0.0);
                }
                let number = text.strip_suffix(unit.trim()).unwrap_or(&text).trim();
                number.parse::<f64>().ok().filter(|v| v.is_finite())
            }
            Self::Clock => parse_clock(&text),
        }
    }

    /// What a field's text asks for, given the value it shows: `None` when
    /// the text is still what it shows (a shown 0.05 must not round a
    /// 0.0523 away) or doesn't read.
    pub fn typed(self, text: &str, current: f64) -> Option<f64> {
        if text.trim() == self.text(current) {
            return None;
        }
        self.read(text)
    }

    /// `steps` steps from the value as shown (75.3° shows 75°, and one step
    /// up is 76°; a clock shows whole seconds).
    pub fn stepped(self, current: f64, steps: i32) -> f64 {
        let step = self.step();
        let shown = match self {
            Self::Number { .. } => (current / step).round(),
            Self::Clock => current.floor(),
        };
        (shown + f64::from(steps)) * step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEGREES: Reading = Reading::number(0, "°");
    const BLEND: Reading = Reading::number(2, "");
    const TILT: Reading = Reading::number(1, "°");
    const LOOKAHEAD: Reading = Reading::or_word(1, " s", "Off");

    #[test]
    fn a_number_reads_with_its_decimals_and_unit() {
        assert_eq!(DEGREES.text(75.4), "75°");
        assert_eq!(BLEND.text(0.0523), "0.05");
        assert_eq!(TILT.text(-2.24), "-2.2°");
        assert_eq!(Reading::number(4, "").text(0.0001), "0.0001");
        assert_eq!(LOOKAHEAD.text(2.5), "2.5 s");
    }

    #[test]
    fn a_zero_word_stands_for_a_zero_as_shown() {
        assert_eq!(LOOKAHEAD.text(0.0), "Off");
        assert_eq!(LOOKAHEAD.text(0.04), "Off");
        assert_eq!(LOOKAHEAD.text(0.06), "0.1 s");
    }

    #[test]
    fn a_clock_reads_as_the_time_panel_does() {
        assert_eq!(Reading::Clock.text(90.4), "1:30");
        assert_eq!(Reading::Clock.text(3723.0), "1:02:03");
    }

    #[test]
    fn a_step_is_one_unit_of_the_last_digit_shown() {
        assert_eq!(DEGREES.step(), 1.0);
        assert!((BLEND.step() - 0.01).abs() < 1e-12);
        assert!((Reading::number(3, "").step() - 0.001).abs() < 1e-12);
        assert_eq!(Reading::Clock.step(), 1.0);
    }

    #[test]
    fn the_unit_is_optional_when_typed() {
        assert_eq!(DEGREES.read("80"), Some(80.0));
        assert_eq!(DEGREES.read(" 80° "), Some(80.0));
        assert_eq!(DEGREES.read("80 °"), Some(80.0));
        assert_eq!(LOOKAHEAD.read("1.5"), Some(1.5));
        assert_eq!(LOOKAHEAD.read("1.5 s"), Some(1.5));
        assert_eq!(LOOKAHEAD.read("1.5s"), Some(1.5));
        assert_eq!(TILT.read("-3"), Some(-3.0));
    }

    #[test]
    fn a_decimal_comma_reads_as_a_point() {
        assert_eq!(BLEND.read("0,05"), Some(0.05));
        assert_eq!(LOOKAHEAD.read("1,5 s"), Some(1.5));
    }

    #[test]
    fn the_zero_word_reads_as_zero_in_any_case() {
        assert_eq!(LOOKAHEAD.read("Off"), Some(0.0));
        assert_eq!(LOOKAHEAD.read("off"), Some(0.0));
        assert_eq!(LOOKAHEAD.read("0"), Some(0.0));
        assert_eq!(DEGREES.read("off"), None);
    }

    #[test]
    fn only_finite_numbers_read() {
        for text in ["", "soon", "inf", "NaN", "-inf", "1/2", "80°°", "0.05°"] {
            assert_eq!(BLEND.read(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_clock_takes_times_and_seconds() {
        assert_eq!(Reading::Clock.read("1:30"), Some(90.0));
        assert_eq!(Reading::Clock.read("90"), Some(90.0));
        assert_eq!(Reading::Clock.read("soon"), None);
    }

    #[test]
    fn the_shown_text_asks_for_nothing() {
        assert_eq!(BLEND.typed("0.05", 0.0523), None);
        assert_eq!(BLEND.typed(" 0.05 ", 0.0523), None);
        assert_eq!(Reading::Clock.typed("0:02", 2.6), None);
        assert_eq!(LOOKAHEAD.typed("Off", 0.0), None);
    }

    #[test]
    fn a_time_left_as_it_was_asks_for_nothing() {
        // Whole seconds shown: a shown 1:00 must not cut 0.4 s off the end.
        assert_eq!(Reading::Clock.typed("1:00", 60.4), None);
        assert_eq!(Reading::Clock.typed(" 1:00 ", 60.4), None);
        assert_eq!(Reading::Clock.typed("0:59", 60.4), Some(59.0));
        assert_eq!(Reading::Clock.typed("soon", 60.4), None);
    }

    #[test]
    fn typed_text_asks_for_its_number() {
        assert_eq!(BLEND.typed("0.05", 0.08), Some(0.05));
        assert_eq!(BLEND.typed("0.123", 0.05), Some(0.123));
        assert_eq!(DEGREES.typed("80", 75.0), Some(80.0));
        assert_eq!(BLEND.typed("soon", 0.05), None);
    }

    #[test]
    fn steps_start_from_the_value_as_shown() {
        assert_eq!(DEGREES.stepped(75.3, 1), 76.0);
        assert_eq!(DEGREES.stepped(75.6, -1), 75.0);
        assert_eq!(DEGREES.stepped(75.0, 10), 85.0);
        assert_eq!(BLEND.text(BLEND.stepped(0.05, 1)), "0.06");
        assert_eq!(BLEND.text(BLEND.stepped(0.0523, -10)), "-0.05");
        assert_eq!(Reading::Clock.stepped(2.6, 1), 3.0);
        assert_eq!(LOOKAHEAD.text(LOOKAHEAD.stepped(0.0, 1)), "0.1 s");
    }
}
