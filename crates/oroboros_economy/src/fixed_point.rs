//! # Fixed-Point Arithmetic
//!
//! **CRITICAL: NO FLOATING POINT IN FINANCIAL CALCULATIONS**
//!
//! This module provides fixed-point decimal numbers for all economic calculations.
//!
//! ## Precision Levels
//!
//! - `FixedPoint` (legacy): u64 with 6 decimals - for internal game economy
//! - `FixedPoint18`: u128 with 18 decimals - for blockchain/ETH compatibility
//!
//! ## Why 18 Decimals?
//!
//! Ethereum uses 18 decimals for ETH and most ERC-20 tokens.
//! Using the same precision prevents dust accumulation and rounding errors
//! when converting between game currency and blockchain assets.
//!
//! ## Why Fixed-Point?
//!
//! - Deterministic: Same calculation = same result on all hardware
//! - No rounding errors: 0.1 + 0.2 == 0.3 (unlike IEEE 754 floats)
//! - Auditable: Financial transactions must be reproducible

use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};

use crate::error::{EconomyError, EconomyResult};

/// Number of decimal places for legacy FixedPoint (internal economy).
const DECIMAL_PLACES_6: u32 = 6;

/// Number of decimal places for blockchain-compatible FixedPoint18.
const DECIMAL_PLACES_18: u32 = 18;

/// The multiplier for 6 decimal places.
const MULTIPLIER_6: u64 = 10u64.pow(DECIMAL_PLACES_6);

/// The multiplier for 18 decimal places.
const MULTIPLIER_18: u128 = 10u128.pow(DECIMAL_PLACES_18);

// =============================================================================
// FixedPoint18 - Blockchain-Compatible (18 decimals)
// =============================================================================

/// Fixed-point decimal number with 18 decimal places.
///
/// **Use this for all blockchain-related calculations.**
///
/// Internally stores value * 10^18 as a u128.
///
/// # Range
///
/// - Minimum: 0.000000000000000000
/// - Maximum: 340,282,366,920,938.463463374607431768211455
///
/// # Compatibility
///
/// This matches Ethereum's native precision (wei = 10^-18 ETH).
///
/// # Example
///
/// ```rust,ignore
/// let eth_price = FixedPoint18::from_whole(100);  // 100.0 ETH
/// let wei_amount = eth_price.to_wei();            // 100 * 10^18 wei
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct FixedPoint18(u128);

impl FixedPoint18 {
    /// Zero value.
    pub const ZERO: Self = Self(0);

    /// One unit (1.000...000 with 18 zeros).
    pub const ONE: Self = Self(MULTIPLIER_18);

    /// Maximum representable value.
    pub const MAX: Self = Self(u128::MAX);

    /// Creates from a whole number.
    #[inline]
    #[must_use]
    pub const fn from_whole(whole: u128) -> Self {
        Self(whole * MULTIPLIER_18)
    }

    /// Creates from raw wei value (no conversion).
    #[inline]
    #[must_use]
    pub const fn from_wei(wei: u128) -> Self {
        Self(wei)
    }

    /// Returns the raw wei value.
    #[inline]
    #[must_use]
    pub const fn to_wei(self) -> u128 {
        self.0
    }

    /// Returns the whole number part.
    #[inline]
    #[must_use]
    pub const fn whole(self) -> u128 {
        self.0 / MULTIPLIER_18
    }

    /// Returns the decimal part (0 to 10^18 - 1).
    #[inline]
    #[must_use]
    pub const fn decimal(self) -> u128 {
        self.0 % MULTIPLIER_18
    }

    /// Checked addition.
    #[inline]
    #[must_use]
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        match self.0.checked_add(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Checked subtraction.
    #[inline]
    #[must_use]
    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        match self.0.checked_sub(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Checked multiplication by integer.
    #[inline]
    #[must_use]
    pub const fn checked_mul_int(self, rhs: u128) -> Option<Self> {
        match self.0.checked_mul(rhs) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Checked division by integer.
    #[inline]
    #[must_use]
    pub const fn checked_div_int(self, rhs: u128) -> Option<Self> {
        if rhs == 0 {
            None
        } else {
            Some(Self(self.0 / rhs))
        }
    }

    /// Multiplies by basis points (10000 = 100%).
    #[inline]
    #[must_use]
    pub const fn mul_percent_bp(self, percent_basis_points: u32) -> Self {
        // For u128, we can safely multiply first then divide
        Self((self.0 * percent_basis_points as u128) / 10000)
    }

    /// Saturating addition.
    #[inline]
    #[must_use]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    /// Saturating subtraction.
    #[inline]
    #[must_use]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    /// Safe addition with error.
    #[inline]
    pub fn safe_add(self, rhs: Self) -> EconomyResult<Self> {
        self.checked_add(rhs).ok_or(EconomyError::ArithmeticOverflow)
    }

    /// Safe subtraction with error.
    #[inline]
    pub fn safe_sub(self, rhs: Self) -> EconomyResult<Self> {
        self.checked_sub(rhs).ok_or(EconomyError::ArithmeticOverflow)
    }

    /// Converts from legacy FixedPoint (6 decimals) to FixedPoint18.
    #[inline]
    #[must_use]
    pub const fn from_fixed_point(fp: FixedPoint) -> Self {
        // 6 decimals to 18 decimals = multiply by 10^12
        Self(fp.0 as u128 * 1_000_000_000_000)
    }

    /// Converts to legacy FixedPoint (6 decimals), truncating precision.
    ///
    /// **WARNING**: This loses 12 decimal places of precision.
    #[inline]
    #[must_use]
    pub const fn to_fixed_point(self) -> Option<FixedPoint> {
        // 18 decimals to 6 decimals = divide by 10^12
        let result = self.0 / 1_000_000_000_000;
        if result > u64::MAX as u128 {
            None
        } else {
            Some(FixedPoint(result as u64))
        }
    }

    /// Returns true if zero.
    #[inline]
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

impl Add for FixedPoint18 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.wrapping_add(rhs.0))
    }
}

impl Sub for FixedPoint18 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.wrapping_sub(rhs.0))
    }
}

impl Mul<u128> for FixedPoint18 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: u128) -> Self::Output {
        Self(self.0.wrapping_mul(rhs))
    }
}

impl Div<u128> for FixedPoint18 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: u128) -> Self::Output {
        Self(self.0 / rhs)
    }
}

impl fmt::Debug for FixedPoint18 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FixedPoint18({}.{:018})", self.whole(), self.decimal())
    }
}

impl fmt::Display for FixedPoint18 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:018}", self.whole(), self.decimal())
    }
}

// =============================================================================
// FixedPoint - Legacy (6 decimals) - For Internal Economy
// =============================================================================

/// Fixed-point decimal number with 6 decimal places.
///
/// **Use this for internal game economy calculations.**
/// For blockchain interactions, use `FixedPoint18`.
///
/// Internally stores value * 1,000,000 as a u64.
///
/// # Range
///
/// - Minimum: 0.000000
/// - Maximum: 18,446,744,073,709.551615
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct FixedPoint(u64);

/// Alias for the legacy constant.
const MULTIPLIER: u64 = MULTIPLIER_6;

impl FixedPoint {
    /// Zero value.
    pub const ZERO: Self = Self(0);

    /// One unit (1.000000).
    pub const ONE: Self = Self(MULTIPLIER);

    /// Maximum representable value.
    pub const MAX: Self = Self(u64::MAX);

    /// Creates a fixed-point number from a whole number.
    ///
    /// # Arguments
    ///
    /// * `whole` - The whole number part
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let ten = FixedPoint::from_whole(10); // 10.000000
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_whole(whole: u64) -> Self {
        Self(whole * MULTIPLIER)
    }

    /// Creates a fixed-point number from parts.
    ///
    /// # Arguments
    ///
    /// * `whole` - The whole number part
    /// * `decimal` - The decimal part (0-999999)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let value = FixedPoint::from_parts(3, 141592); // 3.141592
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_parts(whole: u64, decimal: u32) -> Self {
        Self(whole * MULTIPLIER + (decimal as u64 % MULTIPLIER))
    }

    /// Creates a fixed-point number from raw internal value.
    ///
    /// # Arguments
    ///
    /// * `raw` - The raw internal representation
    #[inline]
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the raw internal value.
    #[inline]
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Returns the whole number part.
    #[inline]
    #[must_use]
    pub const fn whole(self) -> u64 {
        self.0 / MULTIPLIER
    }

    /// Returns the decimal part (0-999999).
    #[inline]
    #[must_use]
    pub const fn decimal(self) -> u32 {
        (self.0 % MULTIPLIER) as u32
    }

    /// Checked addition. Returns `None` on overflow.
    #[inline]
    #[must_use]
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        match self.0.checked_add(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Checked subtraction. Returns `None` on underflow.
    #[inline]
    #[must_use]
    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        match self.0.checked_sub(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Checked multiplication by an integer.
    #[inline]
    #[must_use]
    pub const fn checked_mul_int(self, rhs: u64) -> Option<Self> {
        match self.0.checked_mul(rhs) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Checked division by an integer.
    #[inline]
    #[must_use]
    pub const fn checked_div_int(self, rhs: u64) -> Option<Self> {
        if rhs == 0 {
            None
        } else {
            Some(Self(self.0 / rhs))
        }
    }

    /// Multiplies by a percentage (0-10000 representing 0.00% to 100.00%).
    ///
    /// # Arguments
    ///
    /// * `percent_basis_points` - Percentage in basis points (100 = 1%)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let value = FixedPoint::from_whole(100);
    /// let five_percent = value.mul_percent_bp(500); // 5.000000
    /// ```
    #[inline]
    #[must_use]
    pub const fn mul_percent_bp(self, percent_basis_points: u32) -> Self {
        // Use u128 to avoid overflow during calculation
        let result = (self.0 as u128 * percent_basis_points as u128) / 10000;
        Self(result as u64)
    }

    /// Saturating addition.
    #[inline]
    #[must_use]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    /// Saturating subtraction.
    #[inline]
    #[must_use]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    /// Returns true if this value is zero.
    #[inline]
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Safe addition with error on overflow.
    ///
    /// # Errors
    ///
    /// Returns `EconomyError::ArithmeticOverflow` if the addition would overflow.
    #[inline]
    pub fn safe_add(self, rhs: Self) -> EconomyResult<Self> {
        self.checked_add(rhs).ok_or(EconomyError::ArithmeticOverflow)
    }

    /// Safe subtraction with error on underflow.
    ///
    /// # Errors
    ///
    /// Returns `EconomyError::ArithmeticOverflow` if the subtraction would underflow.
    #[inline]
    pub fn safe_sub(self, rhs: Self) -> EconomyResult<Self> {
        self.checked_sub(rhs).ok_or(EconomyError::ArithmeticOverflow)
    }
}

impl Add for FixedPoint {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.wrapping_add(rhs.0))
    }
}

impl AddAssign for FixedPoint {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.0 = self.0.wrapping_add(rhs.0);
    }
}

impl Sub for FixedPoint {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.wrapping_sub(rhs.0))
    }
}

impl SubAssign for FixedPoint {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 = self.0.wrapping_sub(rhs.0);
    }
}

impl Mul<u64> for FixedPoint {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: u64) -> Self::Output {
        Self(self.0.wrapping_mul(rhs))
    }
}

impl Div<u64> for FixedPoint {
    type Output = Self;

    #[inline]
    fn div(self, rhs: u64) -> Self::Output {
        Self(self.0 / rhs)
    }
}

impl fmt::Debug for FixedPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FixedPoint({}.{:06})", self.whole(), self.decimal())
    }
}

impl fmt::Display for FixedPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:06}", self.whole(), self.decimal())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_whole() {
        let value = FixedPoint::from_whole(100);
        assert_eq!(value.whole(), 100);
        assert_eq!(value.decimal(), 0);
    }

    #[test]
    fn test_from_parts() {
        let value = FixedPoint::from_parts(3, 141592);
        assert_eq!(value.whole(), 3);
        assert_eq!(value.decimal(), 141592);
    }

    #[test]
    fn test_addition() {
        let a = FixedPoint::from_parts(1, 500000); // 1.5
        let b = FixedPoint::from_parts(2, 300000); // 2.3
        let result = a + b;
        assert_eq!(result.whole(), 3);
        assert_eq!(result.decimal(), 800000);
    }

    #[test]
    fn test_subtraction() {
        let a = FixedPoint::from_parts(5, 0);
        let b = FixedPoint::from_parts(2, 500000);
        let result = a - b;
        assert_eq!(result.whole(), 2);
        assert_eq!(result.decimal(), 500000);
    }

    #[test]
    fn test_percent() {
        let value = FixedPoint::from_whole(1000);
        let five_percent = value.mul_percent_bp(500); // 5%
        assert_eq!(five_percent.whole(), 50);
    }

    #[test]
    fn test_checked_add_overflow() {
        let max = FixedPoint::MAX;
        assert!(max.checked_add(FixedPoint::ONE).is_none());
    }

    #[test]
    fn test_checked_sub_underflow() {
        let zero = FixedPoint::ZERO;
        assert!(zero.checked_sub(FixedPoint::ONE).is_none());
    }

    #[test]
    fn test_display() {
        let value = FixedPoint::from_parts(42, 123456);
        assert_eq!(format!("{value}"), "42.123456");
    }
}
