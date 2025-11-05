/// Multiplies two f64 numbers and returns their product
///
/// # Arguments
/// * `a` - The first number
/// * `b` - The second number
///
/// # Returns
/// The product of `a` and `b`
///
/// # Examples
/// ```
/// let result = multiply(2.5, 4.0);
/// assert_eq!(result, 10.0);
/// ```
pub fn multiply(a: f64, b: f64) -> f64 {
    a * b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiply_positive_numbers() {
        assert_eq!(multiply(2.0, 3.0), 6.0);
    }

    #[test]
    fn test_multiply_negative_numbers() {
        assert_eq!(multiply(-2.0, -3.0), 6.0);
    }

    #[test]
    fn test_multiply_mixed_numbers() {
        assert_eq!(multiply(2.0, -3.0), -6.0);
    }

    #[test]
    fn test_multiply_zero() {
        assert_eq!(multiply(0.0, 5.0), 0.0);
        assert_eq!(multiply(5.0, 0.0), 0.0);
    }

    #[test]
    fn test_multiply_fractional_numbers() {
        assert_eq!(multiply(2.5, 4.0), 10.0);
        assert_eq!(multiply(0.5, 0.5), 0.25);
    }

    #[test]
    fn test_multiply_one() {
        assert_eq!(multiply(5.5, 1.0), 5.5);
        assert_eq!(multiply(1.0, 5.5), 5.5);
    }
}
