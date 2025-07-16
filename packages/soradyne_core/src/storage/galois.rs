//! Galois Field GF(256) arithmetic operations
//! 
//! This module implements arithmetic in the Galois Field GF(2^8) using the
//! irreducible polynomial x^8 + x^4 + x^3 + x + 1 (0x11b).

/// Galois Field GF(256) implementation
#[derive(Debug, Clone)]
pub struct GF256 {
    /// Precomputed logarithm table for fast multiplication
    log_table: [u8; 256],
    /// Precomputed antilog table for fast multiplication
    antilog_table: [u8; 256],
}

impl GF256 {
    /// Create a new GF(256) instance with precomputed tables
    pub fn new() -> Self {
        let mut gf = Self {
            log_table: [0; 256],
            antilog_table: [0; 256],
        };
        gf.build_tables();
        gf
    }
    
    /// Build logarithm and antilog tables for fast multiplication
    fn build_tables(&mut self) {
        // Use the irreducible polynomial x^8 + x^4 + x^3 + x + 1 = 0x11b
        // But for reduction, we only need the lower 8 bits: 0x1b
        const REDUCTION_POLY: u8 = 0x1b;
        
        // Build antilog table (powers of the primitive element 2)
        let mut value = 1u8;
        for i in 0..255 {
            self.antilog_table[i] = value;
            
            // Only set log_table entry if it hasn't been set yet
            // This prevents overwriting log[1] = 0 when value cycles back to 1
            if self.log_table[value as usize] == 0 && value != 1 {
                self.log_table[value as usize] = i as u8;
            } else if value == 1 && i == 0 {
                // Special case: ensure log[1] = 0 is set correctly on first iteration
                self.log_table[1] = 0;
            }
            
            // Multiply by 2 (the primitive element) in GF(256)
            let high_bit = value & 0x80;
            value <<= 1;
            if high_bit != 0 {
                value ^= REDUCTION_POLY;
            }
        }
        
        // Complete the cycle - antilog[255] should equal antilog[0] = 1
        self.antilog_table[255] = self.antilog_table[0];
        
        // Special case: log(0) is undefined, but we set it to 0 for convenience
        // This won't be used in practice since we check for zero in multiply/divide
        self.log_table[0] = 0;
    }
    
    /// Addition in GF(256) - same as XOR
    pub fn add(&self, a: u8, b: u8) -> u8 {
        a ^ b
    }
    
    /// Subtraction in GF(256) - same as XOR (since -x = x in GF(2^n))
    pub fn subtract(&self, a: u8, b: u8) -> u8 {
        a ^ b
    }
    
    /// Multiplication in GF(256) using log/antilog tables
    pub fn multiply(&self, a: u8, b: u8) -> u8 {
        if a == 0 || b == 0 {
            return 0;
        }
        
        let log_a = self.log_table[a as usize] as u16;
        let log_b = self.log_table[b as usize] as u16;
        let log_result = (log_a + log_b) % 255;
        
        self.antilog_table[log_result as usize]
    }
    
    /// Division in GF(256) - multiplication by multiplicative inverse
    pub fn divide(&self, a: u8, b: u8) -> Result<u8, &'static str> {
        if b == 0 {
            return Err("Division by zero in GF(256)");
        }
        if a == 0 {
            return Ok(0);
        }
        
        let log_a = self.log_table[a as usize] as u16;
        let log_b = self.log_table[b as usize] as u16;
        let log_result = (255 + log_a - log_b) % 255;
        
        Ok(self.antilog_table[log_result as usize])
    }
    
    /// Multiplicative inverse in GF(256)
    pub fn inverse(&self, a: u8) -> Result<u8, &'static str> {
        if a == 0 {
            return Err("Zero has no multiplicative inverse");
        }
        
        let log_a = self.log_table[a as usize] as u16;
        let log_inverse = (255 - log_a) % 255;
        
        Ok(self.antilog_table[log_inverse as usize])
    }
    
    /// Power operation in GF(256)
    pub fn power(&self, base: u8, exponent: u8) -> u8 {
        if base == 0 {
            return if exponent == 0 { 1 } else { 0 };
        }
        if exponent == 0 {
            return 1;
        }
        
        let log_base = self.log_table[base as usize] as u16;
        let log_result = (log_base * exponent as u16) % 255;
        
        self.antilog_table[log_result as usize]
    }
    
    /// Evaluate polynomial at given point
    /// poly[0] + poly[1]*x + poly[2]*x^2 + ... + poly[n]*x^n
    pub fn eval_polynomial(&self, poly: &[u8], x: u8) -> u8 {
        if poly.is_empty() {
            return 0;
        }
        
        let mut result = poly[0];
        let mut x_power = x;
        
        for &coeff in &poly[1..] {
            result = self.add(result, self.multiply(coeff, x_power));
            x_power = self.multiply(x_power, x);
        }
        
        result
    }
    
    /// Lagrange interpolation to find polynomial value at x=0
    /// Given points (x_i, y_i), compute the polynomial value at 0
    pub fn lagrange_interpolate_at_zero(&self, points: &[(u8, u8)]) -> Result<u8, &'static str> {
        if points.is_empty() {
            return Ok(0);
        }
        
        let mut result = 0u8;
        
        for (i, &(xi, yi)) in points.iter().enumerate() {
            // Calculate Lagrange basis polynomial L_i(0)
            let mut numerator = 1u8;
            let mut denominator = 1u8;
            
            for (j, &(xj, _)) in points.iter().enumerate() {
                if i != j {
                    // For L_i(0): numerator *= (0 - xj) = xj (since -xj = xj in GF(2^n))
                    // denominator *= (xi - xj)
                    numerator = self.multiply(numerator, xj);
                    denominator = self.multiply(denominator, self.subtract(xi, xj));
                }
            }
            
            // Calculate yi * L_i(0) = yi * (numerator / denominator)
            let lagrange_coeff = self.divide(numerator, denominator)?;
            let term = self.multiply(yi, lagrange_coeff);
            result = self.add(result, term);
        }
        
        Ok(result)
    }
}

impl Default for GF256 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_gf256_basic_operations() {
        let gf = GF256::new();
        
        // Test addition (XOR)
        assert_eq!(gf.add(0x53, 0xCA), 0x99);
        assert_eq!(gf.add(0xFF, 0xFF), 0x00);
        assert_eq!(gf.add(0x00, 0xFF), 0xFF);
        
        // Test subtraction (same as addition in GF(2^n))
        assert_eq!(gf.subtract(0x53, 0xCA), 0x99);
        assert_eq!(gf.subtract(0xFF, 0xFF), 0x00);
    }
    
    #[test]
    fn test_gf256_multiplication() {
        let gf = GF256::new();
        
        // Test basic multiplication
        assert_eq!(gf.multiply(0, 5), 0);
        assert_eq!(gf.multiply(5, 0), 0);
        assert_eq!(gf.multiply(1, 5), 5);
        assert_eq!(gf.multiply(5, 1), 5);
        
        // Test specific known values
        assert_eq!(gf.multiply(2, 2), 4);
        assert_eq!(gf.multiply(2, 3), 6);
        assert_eq!(gf.multiply(3, 3), 5); // 3*3 = 9 = x^3 + 1, reduced mod x^8+x^4+x^3+x+1
        
        // Test commutativity
        for a in 1..=10 {
            for b in 1..=10 {
                assert_eq!(gf.multiply(a, b), gf.multiply(b, a));
            }
        }
    }
    
    #[test]
    fn test_gf256_inverse() {
        let gf = GF256::new();
        
        // Test that a * a^(-1) = 1 for all non-zero elements
        for a in 1..=255u8 {
            let inv_a = gf.inverse(a).unwrap();
            assert_eq!(gf.multiply(a, inv_a), 1, "Failed for a={}", a);
        }
        
        // Test that 0 has no inverse
        assert!(gf.inverse(0).is_err());
    }
    
    #[test]
    fn test_gf256_division() {
        let gf = GF256::new();
        
        // Test basic division
        assert_eq!(gf.divide(0, 5).unwrap(), 0);
        assert_eq!(gf.divide(5, 1).unwrap(), 5);
        assert_eq!(gf.divide(6, 2).unwrap(), 3);
        
        // Test division by zero
        assert!(gf.divide(5, 0).is_err());
        
        // Test that (a * b) / b = a for all non-zero b
        for a in 1..=10 {
            for b in 1..=10 {
                let product = gf.multiply(a, b);
                let quotient = gf.divide(product, b).unwrap();
                assert_eq!(quotient, a, "Failed for a={}, b={}", a, b);
            }
        }
    }
    
    #[test]
    fn test_gf256_power() {
        let gf = GF256::new();
        
        // Test basic powers
        assert_eq!(gf.power(2, 0), 1);
        assert_eq!(gf.power(2, 1), 2);
        assert_eq!(gf.power(2, 2), 4);
        assert_eq!(gf.power(2, 3), 8);
        
        // Test that 0^0 = 1 and 0^n = 0 for n > 0
        assert_eq!(gf.power(0, 0), 1);
        assert_eq!(gf.power(0, 5), 0);
        
        // Test that a^1 = a
        for a in 1..=10 {
            assert_eq!(gf.power(a, 1), a);
        }
    }
    
    #[test]
    fn test_polynomial_evaluation() {
        let gf = GF256::new();
        
        // Test constant polynomial
        let poly = vec![5];
        assert_eq!(gf.eval_polynomial(&poly, 10), 5);
        
        // Test linear polynomial: 3 + 2x
        let poly = vec![3, 2];
        assert_eq!(gf.eval_polynomial(&poly, 0), 3);
        assert_eq!(gf.eval_polynomial(&poly, 1), gf.add(3, 2)); // 3 + 2*1
        assert_eq!(gf.eval_polynomial(&poly, 5), gf.add(3, gf.multiply(2, 5))); // 3 + 2*5
        
        // Test quadratic polynomial: 1 + 2x + 3x^2
        let poly = vec![1, 2, 3];
        let x = 4;
        let expected = gf.add(gf.add(1, gf.multiply(2, x)), gf.multiply(3, gf.multiply(x, x)));
        assert_eq!(gf.eval_polynomial(&poly, x), expected);
    }
    
    #[test]
    fn test_lagrange_interpolation() {
        let gf = GF256::new();
        
        // Test with simple points that should give a constant polynomial
        let points = vec![(1, 5), (2, 5), (3, 5)];
        let result = gf.lagrange_interpolate_at_zero(&points).unwrap();
        assert_eq!(result, 5);
        
        // Test with points from a linear polynomial f(x) = 3 + 2x
        // f(1) = 5, f(2) = 7^2 = 1 (in GF), f(3) = 3^2^3 = 9^3 = ...
        let points = vec![
            (1, gf.add(3, gf.multiply(2, 1))),
            (2, gf.add(3, gf.multiply(2, 2))),
            (3, gf.add(3, gf.multiply(2, 3))),
        ];
        let result = gf.lagrange_interpolate_at_zero(&points).unwrap();
        assert_eq!(result, 3); // Should recover the constant term
        
        // Test with known polynomial coefficients
        let secret = 42u8;
        let poly = vec![secret, 17, 23]; // f(x) = 42 + 17x + 23x^2
        
        // Generate points
        let points = vec![
            (1, gf.eval_polynomial(&poly, 1)),
            (2, gf.eval_polynomial(&poly, 2)),
            (3, gf.eval_polynomial(&poly, 3)),
        ];
        
        // Interpolate back to get f(0) = secret
        let recovered = gf.lagrange_interpolate_at_zero(&points).unwrap();
        assert_eq!(recovered, secret);
    }
    
    #[test]
    fn test_shamir_secret_sharing_simulation() {
        let gf = GF256::new();
        let secret = 123u8;
        let threshold = 3;
        let total_shares = 5;
        
        // Generate random polynomial coefficients
        let mut poly = vec![secret]; // f(0) = secret
        for _ in 1..threshold {
            poly.push(42); // Use fixed coefficients for reproducible test
        }
        
        // Generate shares
        let mut shares = Vec::new();
        for i in 1..=total_shares {
            let x = i as u8;
            let y = gf.eval_polynomial(&poly, x);
            shares.push((x, y));
        }
        
        // Test reconstruction with exactly threshold shares
        let points = &shares[..threshold];
        let recovered = gf.lagrange_interpolate_at_zero(points).unwrap();
        assert_eq!(recovered, secret);
        
        // Test reconstruction with different subset
        let points = vec![shares[0], shares[2], shares[4]];
        let recovered = gf.lagrange_interpolate_at_zero(&points).unwrap();
        assert_eq!(recovered, secret);
        
        // Test that we can't reconstruct with insufficient shares
        let points = &shares[..threshold-1];
        // This should still work mathematically, but won't give the right answer
        // in a real Shamir scheme due to insufficient constraints
        let _recovered = gf.lagrange_interpolate_at_zero(points).unwrap();
        // We don't assert equality here because it's expected to be wrong
    }
}
