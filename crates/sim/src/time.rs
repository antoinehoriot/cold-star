//! Pure conversion of elapsed wall-clock time into whole ticks. The `app`
//! supplies the elapsed time; this keeps the boundary clean and testable.

pub fn ticks_for_elapsed(elapsed_ms: f64, tick_ms: f64, cap_ticks: u64) -> u64 {
    if tick_ms <= 0.0 || elapsed_ms <= 0.0 || !elapsed_ms.is_finite() {
        return 0;
    }
    let n = (elapsed_ms / tick_ms).floor();
    if n <= 0.0 {
        return 0;
    }
    (n as u64).min(cap_ticks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_elapsed_is_zero_ticks() {
        assert_eq!(ticks_for_elapsed(0.0, 1000.0, 100), 0);
    }

    #[test]
    fn exact_multiple() {
        assert_eq!(ticks_for_elapsed(3000.0, 1000.0, 100), 3);
    }

    #[test]
    fn floors_partial() {
        assert_eq!(ticks_for_elapsed(2500.0, 1000.0, 100), 2);
    }

    #[test]
    fn respects_cap() {
        assert_eq!(ticks_for_elapsed(1_000_000.0, 1000.0, 50), 50);
    }

    #[test]
    fn negative_or_bad_inputs_are_zero() {
        assert_eq!(ticks_for_elapsed(-5.0, 1000.0, 100), 0);
        assert_eq!(ticks_for_elapsed(5000.0, 0.0, 100), 0);
    }
}
