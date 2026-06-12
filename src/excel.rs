pub mod reader;
pub mod writer;
use super::interpolate::{Curve, Interpolate, build_curve};
use crate::excel::reader::Paser;
use enterpolation::{Signal, bspline::BSplineError};
use roots::{SearchError, SimpleConvergency, find_root_brent};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ReadIOErr(#[from] calamine::Error),

    #[error(transparent)]
    WriteIOErr(#[from] rust_xlsxwriter::XlsxError),

    #[error("表格形状不符合要求期望2列且有表头, 实际为{0}行*{1}列(含表头)")]
    ShapeErr(usize, usize),

    #[error("位于{0}行{1}列的单元格没有有效内容")]
    EmptyCellErr(usize, usize),

    #[error("无法为相关数据：{0}构建插值曲线")]
    BuildCurveErr(String, BSplineError),
}

pub struct CurveData {
    x: Vec<f64>,
    y: Vec<f64>,
    /// 定义域
    domain: (f64, f64),
}

pub trait CurveExt {
    /// 曲线名称
    const NAME: &'static str;
    /// 工作表名称
    const SHEET_NAME: &'static str;
}

pub struct ExcelData {
    pub water_comming: InflowHydrograph,
    pub water_level_with_storage: WaterLevelWithStorge,
    pub water_level_with_discharge: WaterLevelWithDischarge,
}

macro_rules! define_curve {
    ($name: ident, $curve_name: literal, $sheet_name: literal) => {
        pub struct $name {
            inner: Curve,
            /// 定义域
            domain: (f64, f64),
        }
        impl $name {
            pub fn build(data: CurveData) -> Result<Self, Error> {
                Ok(Self {
                    domain: data.domain,
                    inner: build_curve(data.x, data.y)
                        .map_err(|e| Error::BuildCurveErr(Self::NAME.to_string(), e))?,
                })
            }

            /// 定义域
            pub fn domain(&self) -> (f64, f64) {
                self.domain
            }

            /// 值域
            #[allow(unused)]
            pub fn range(&self) -> (f64, f64) {
                use enterpolation::Curve;
                self.inner.domain().into()
            }
        }

        impl reader::Paser for $name {}

        impl CurveExt for $name {
            const NAME: &'static str = $curve_name;
            const SHEET_NAME: &'static str = $sheet_name;
        }

        impl Interpolate for $name {
            fn get(&self, x: f64) -> f64 {
                self.inner.eval(x)
            }
            fn get_reverse(&self, y: f64, domain: (f64, f64)) -> Result<f64, SearchError> {
                let mut convergency = SimpleConvergency {
                    eps: 1e-6,
                    max_iter: 30,
                };
                Ok(find_root_brent(
                    domain.0,
                    domain.1,
                    |v| self.get(v) - y,
                    &mut convergency,
                )?)
            }
        }
    };
}

define_curve!(InflowHydrograph, "洪水过程线", "来水过程");
define_curve!(WaterLevelWithStorge, "库容特性曲线", "水位-库容关系");
define_curve!(WaterLevelWithDischarge, "溢洪道特性曲线", "水位-泄流关系");

impl ExcelData {
    pub fn build<T: AsRef<std::path::Path>>(excel_path: T) -> Result<Self, Error> {
        let mut workbook = reader::open(excel_path)?;

        Ok(Self {
            water_comming: InflowHydrograph::build(InflowHydrograph::paser_data(
                InflowHydrograph::load_curve(&mut workbook)?,
            )?)?,
            water_level_with_storage: WaterLevelWithStorge::build(
                WaterLevelWithStorge::paser_data(WaterLevelWithStorge::load_curve(&mut workbook)?)?,
            )?,
            water_level_with_discharge: WaterLevelWithDischarge::build(
                WaterLevelWithDischarge::paser_data(WaterLevelWithDischarge::load_curve(
                    &mut workbook,
                )?)?,
            )?,
        })
    }
}
