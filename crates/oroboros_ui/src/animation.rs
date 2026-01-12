//! Animation system with sharp exponential easing.
//!
//! ARCHITECT'S MANDATE: Animations are SHARP, not soft.
//! Use exponential curves, not linear or ease-in-out.

/// Easing function type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Easing {
    /// Linear interpolation (boring - avoid).
    Linear,
    /// Exponential ease-out (SHARP snap to target).
    #[default]
    ExponentialOut,
    /// Exponential ease-in (accelerating).
    ExponentialIn,
    /// Exponential ease-in-out.
    ExponentialInOut,
    /// Instant (no animation).
    Instant,
}

impl Easing {
    /// Applies the easing function to a t value (0-1).
    #[must_use]
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        
        match self {
            Self::Linear => t,
            Self::ExponentialOut => {
                // Sharp snap: 1 - 2^(-10t)
                if t >= 1.0 {
                    1.0
                } else {
                    1.0 - 2.0_f32.powf(-10.0 * t)
                }
            }
            Self::ExponentialIn => {
                // Accelerating: 2^(10(t-1))
                if t <= 0.0 {
                    0.0
                } else {
                    2.0_f32.powf(10.0 * (t - 1.0))
                }
            }
            Self::ExponentialInOut => {
                if t <= 0.0 {
                    0.0
                } else if t >= 1.0 {
                    1.0
                } else if t < 0.5 {
                    2.0_f32.powf(20.0 * t - 10.0) / 2.0
                } else {
                    (2.0 - 2.0_f32.powf(-20.0 * t + 10.0)) / 2.0
                }
            }
            Self::Instant => 1.0,
        }
    }
}

/// A single animated value.
#[derive(Debug, Clone)]
pub struct Animation {
    /// Current value.
    current: f32,
    /// Target value.
    target: f32,
    /// Animation progress (0-1).
    progress: f32,
    /// Animation duration (seconds).
    duration: f32,
    /// Easing function.
    easing: Easing,
    /// Start value (for interpolation).
    start: f32,
}

impl Animation {
    /// Default animation duration.
    pub const DEFAULT_DURATION: f32 = 0.15; // 150ms - fast but visible
    
    /// Creates a new animation at the given value.
    #[must_use]
    pub fn new(value: f32, easing: Easing) -> Self {
        Self {
            current: value,
            target: value,
            progress: 1.0,
            duration: Self::DEFAULT_DURATION,
            easing,
            start: value,
        }
    }
    
    /// Creates an animation with custom duration.
    #[must_use]
    pub fn with_duration(mut self, duration: f32) -> Self {
        self.duration = duration;
        self
    }
    
    /// Returns the current value.
    #[must_use]
    pub fn value(&self) -> f32 {
        self.current
    }
    
    /// Returns true if the animation is complete.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.progress >= 1.0
    }
    
    /// Sets a new target value, starting animation from current value.
    pub fn set_target(&mut self, target: f32) {
        if (target - self.target).abs() > 0.0001 {
            self.start = self.current;
            self.target = target;
            self.progress = 0.0;
        }
    }
    
    /// Immediately sets the value without animation.
    pub fn set_immediate(&mut self, value: f32) {
        self.current = value;
        self.target = value;
        self.start = value;
        self.progress = 1.0;
    }
    
    /// Updates the animation.
    ///
    /// `dt` is delta time in seconds.
    pub fn update(&mut self, dt: f32) {
        if self.progress >= 1.0 {
            return;
        }
        
        // Advance progress
        if self.duration > 0.0 {
            self.progress += dt / self.duration;
        } else {
            self.progress = 1.0;
        }
        
        self.progress = self.progress.min(1.0);
        
        // Apply easing and interpolate
        let eased = self.easing.apply(self.progress);
        self.current = self.start + (self.target - self.start) * eased;
        
        // Snap to target when complete
        if self.progress >= 1.0 {
            self.current = self.target;
        }
    }
}

impl Default for Animation {
    fn default() -> Self {
        Self::new(0.0, Easing::ExponentialOut)
    }
}

/// Animated 2D vector.
#[derive(Debug, Clone)]
pub struct Animation2D {
    /// X component animation.
    pub x: Animation,
    /// Y component animation.
    pub y: Animation,
}

impl Animation2D {
    /// Creates a new 2D animation.
    #[must_use]
    pub fn new(x: f32, y: f32, easing: Easing) -> Self {
        Self {
            x: Animation::new(x, easing),
            y: Animation::new(y, easing),
        }
    }
    
    /// Returns the current value.
    #[must_use]
    pub fn value(&self) -> (f32, f32) {
        (self.x.value(), self.y.value())
    }
    
    /// Sets a new target.
    pub fn set_target(&mut self, x: f32, y: f32) {
        self.x.set_target(x);
        self.y.set_target(y);
    }
    
    /// Updates the animation.
    pub fn update(&mut self, dt: f32) {
        self.x.update(dt);
        self.y.update(dt);
    }
    
    /// Returns true if both animations are complete.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.x.is_complete() && self.y.is_complete()
    }
}

/// Animated color (RGBA).
#[derive(Debug, Clone)]
pub struct AnimatedColor {
    /// Red component.
    pub r: Animation,
    /// Green component.
    pub g: Animation,
    /// Blue component.
    pub b: Animation,
    /// Alpha component.
    pub a: Animation,
}

impl AnimatedColor {
    /// Creates a new animated color.
    #[must_use]
    pub fn new(r: f32, g: f32, b: f32, a: f32, easing: Easing) -> Self {
        Self {
            r: Animation::new(r, easing),
            g: Animation::new(g, easing),
            b: Animation::new(b, easing),
            a: Animation::new(a, easing),
        }
    }
    
    /// Returns the current color values.
    #[must_use]
    pub fn value(&self) -> (f32, f32, f32, f32) {
        (self.r.value(), self.g.value(), self.b.value(), self.a.value())
    }
    
    /// Sets a new target color.
    pub fn set_target(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.r.set_target(r);
        self.g.set_target(g);
        self.b.set_target(b);
        self.a.set_target(a);
    }
    
    /// Updates the animation.
    pub fn update(&mut self, dt: f32) {
        self.r.update(dt);
        self.g.update(dt);
        self.b.update(dt);
        self.a.update(dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_exponential_out_is_sharp() {
        let easing = Easing::ExponentialOut;
        
        // At t=0.3 (30% through), exponential should be >80% done
        let value = easing.apply(0.3);
        assert!(value > 0.8, "Exponential out should snap quickly: {value}");
    }
    
    #[test]
    fn test_animation_reaches_target() {
        let mut anim = Animation::new(0.0, Easing::ExponentialOut);
        anim.set_target(100.0);
        
        // Run for full duration
        for _ in 0..20 {
            anim.update(0.016); // ~60fps
        }
        
        assert!((anim.value() - 100.0).abs() < 0.01);
        assert!(anim.is_complete());
    }
}
