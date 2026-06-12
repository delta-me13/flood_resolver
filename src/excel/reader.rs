use super::{CurveData, CurveExt, Error};
use calamine::{Data, DataType, Range, Reader, Sheets, open_workbook_auto};

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub trait Paser: CurveExt {
    #[inline]
    fn load_curve(workbook: &mut Sheets<BufReader<File>>) -> Result<Range<Data>, Error> {
        Ok(workbook.worksheet_range(Self::SHEET_NAME)?)
    }
    fn paser_data(range: Range<Data>) -> Result<CurveData, Error> {
        let mut x = Vec::with_capacity(range.height());
        let mut y = Vec::with_capacity(range.height());

        if range.width() != 2 || range.height() < 2 {
            return Err(Error::ShapeErr(
                range.width().saturating_add(1),
                range.height().saturating_add(1),
            ));
        }

        for row in 1..=range.height() {
            x.push(
                range
                    .get_value((row as u32, 0))
                    .ok_or(Error::EmptyCellErr(row + 1, 1))?
                    .as_f64()
                    .ok_or(Error::EmptyCellErr(row + 1, 1))?,
            );
            y.push(
                range
                    .get_value((row as u32, 1))
                    .ok_or(Error::EmptyCellErr(row + 1, 2))?
                    .as_f64()
                    .ok_or(Error::EmptyCellErr(row + 1, 2))?,
            );
        }

        Ok(CurveData {
            domain: (x[0], x[x.len() - 1]),
            x,
            y,
        })
    }
}

#[inline]
pub fn open<T: AsRef<Path>>(path: T) -> Result<Sheets<BufReader<File>>, Error> {
    Ok(open_workbook_auto(path)?)
}
