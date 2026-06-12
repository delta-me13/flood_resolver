use enterpolation::bspline::{BSpline, BSplineError, BorderBuffer};
use enterpolation::{ConstSpace, Sorted};

pub type Curve = BSpline<BorderBuffer<Sorted<Vec<f64>>>, Vec<f64>, ConstSpace<f64, 2>>;

pub fn build_curve(x: Vec<f64>, y: Vec<f64>) -> Result<Curve, BSplineError> {
    Ok(BSpline::builder()
        .clamped()
        .elements(y)
        .knots(x)
        .constant::<2>()
        .build()?)
}

pub trait Interpolate {
    fn get(&self, x: f64) -> f64;
    fn get_reverse(&self, y: f64, domain: (f64, f64)) -> Result<f64, roots::SearchError>;
}
