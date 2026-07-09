//! Lightweight framework-agnostic geometry types (f32-based).
//!
//! `core` does not depend on GPUI; the gpui layer converts these to
//! `Point<Pixels>` / `Size<Pixels>` / `Bounds<Pixels>` at the boundary.

pub mod edge_path;
pub mod hit_test;
pub mod port_calc;
pub mod routing;

use serde::{Deserialize, Serialize};

/// A 2D point in logical (document) coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct PointF {
    pub x: f32,
    pub y: f32,
}

impl PointF {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }

    pub fn distance_to(self, other: Self) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        (dx * dx + dy * dy).sqrt()
    }
}

impl std::ops::Add for PointF {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl std::ops::Sub for PointF {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl std::ops::Mul<f32> for PointF {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Self {
            x: self.x * s,
            y: self.y * s,
        }
    }
}

/// A 2D size in logical coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct SizeF {
    pub w: f32,
    pub h: f32,
}

impl SizeF {
    pub const fn new(w: f32, h: f32) -> Self {
        Self { w, h }
    }

    pub const fn zero() -> Self {
        Self { w: 0.0, h: 0.0 }
    }
}

/// An axis-aligned rectangle in logical coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct RectF {
    pub origin: PointF,
    pub size: SizeF,
}

impl RectF {
    pub const fn new(origin: PointF, size: SizeF) -> Self {
        Self { origin, size }
    }

    pub fn from_center(center: PointF, size: SizeF) -> Self {
        Self {
            origin: PointF::new(center.x - size.w * 0.5, center.y - size.h * 0.5),
            size,
        }
    }

    pub fn left(self) -> f32 {
        self.origin.x
    }

    pub fn right(self) -> f32 {
        self.origin.x + self.size.w
    }

    pub fn top(self) -> f32 {
        self.origin.y
    }

    pub fn bottom(self) -> f32 {
        self.origin.y + self.size.h
    }

    pub fn center(self) -> PointF {
        PointF::new(
            self.origin.x + self.size.w * 0.5,
            self.origin.y + self.size.h * 0.5,
        )
    }

    pub fn contains(self, p: PointF) -> bool {
        p.x >= self.left()
            && p.x <= self.right()
            && p.y >= self.top()
            && p.y <= self.bottom()
    }

    /// Expand the rectangle outward by `amount` on every side.
    pub fn expand(self, amount: f32) -> Self {
        Self {
            origin: PointF::new(self.origin.x - amount, self.origin.y - amount),
            size: SizeF::new(self.size.w + amount * 2.0, self.size.h + amount * 2.0),
        }
    }

    /// 两个轴对齐矩形是否重叠（边相切不算重叠）。
    pub fn intersects(self, other: Self) -> bool {
        self.left() < other.right()
            && self.right() > other.left()
            && self.top() < other.bottom()
            && self.bottom() > other.top()
    }

    /// Compute the smallest rectangle containing both `self` and `other`.
    pub fn union(self, other: Self) -> Self {
        let left = self.left().min(other.left());
        let top = self.top().min(other.top());
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Self::new(
            PointF::new(left, top),
            SizeF::new(right - left, bottom - top),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains_and_center() {
        let r = RectF::new(PointF::new(10.0, 20.0), SizeF::new(100.0, 50.0));
        assert!(r.contains(PointF::new(60.0, 45.0)));
        assert!(!r.contains(PointF::new(5.0, 45.0)));
        assert_eq!(r.center(), PointF::new(60.0, 45.0));
        assert_eq!(r.left(), 10.0);
        assert_eq!(r.right(), 110.0);
        assert_eq!(r.top(), 20.0);
        assert_eq!(r.bottom(), 70.0);
    }

    #[test]
    fn point_lerp_and_distance() {
        let a = PointF::new(0.0, 0.0);
        let b = PointF::new(10.0, 0.0);
        assert_eq!(a.lerp(b, 0.5), PointF::new(5.0, 0.0));
        assert!((a.distance_to(b) - 10.0).abs() < 1e-6);
    }

    #[test]
    fn rect_expand() {
        let r = RectF::new(PointF::new(10.0, 10.0), SizeF::new(20.0, 20.0));
        let e = r.expand(5.0);
        assert_eq!(e.left(), 5.0);
        assert_eq!(e.right(), 35.0);
        assert_eq!(e.size, SizeF::new(30.0, 30.0));
    }

    #[test]
    fn rect_intersects() {
        let a = RectF::new(PointF::new(10.0, 10.0), SizeF::new(20.0, 20.0));

        // 重叠：b 与 a 部分相交
        let overlap = RectF::new(PointF::new(20.0, 20.0), SizeF::new(20.0, 20.0));
        assert!(a.intersects(overlap));
        assert!(overlap.intersects(a)); // 对称

        // 相切：b 紧贴 a 右边（边相切不算重叠）
        let touching = RectF::new(PointF::new(30.0, 10.0), SizeF::new(20.0, 20.0));
        assert!(!a.intersects(touching));

        // 包含：b 完全在 a 内部
        let inner = RectF::new(PointF::new(15.0, 15.0), SizeF::new(5.0, 5.0));
        assert!(a.intersects(inner));
        assert!(inner.intersects(a));

        // 分离：b 远离 a
        let separate = RectF::new(PointF::new(100.0, 100.0), SizeF::new(20.0, 20.0));
        assert!(!a.intersects(separate));
    }

    #[test]
    fn rect_union() {
        let a = RectF::new(PointF::new(10.0, 10.0), SizeF::new(20.0, 20.0));
        let b = RectF::new(PointF::new(30.0, 30.0), SizeF::new(20.0, 20.0));
        let u = a.union(b);
        assert_eq!(u.left(), 10.0);
        assert_eq!(u.top(), 10.0);
        assert_eq!(u.right(), 50.0);
        assert_eq!(u.bottom(), 50.0);

        // Union with self is identity
        let s = a.union(a);
        assert_eq!(s, a);

        // Union with contained rect is the outer rect
        let inner = RectF::new(PointF::new(15.0, 15.0), SizeF::new(5.0, 5.0));
        assert_eq!(a.union(inner), a);
    }
}
