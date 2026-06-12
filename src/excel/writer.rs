use super::Error;
use crate::algorithm::Step;
use rust_xlsxwriter::{Chart, ChartLegendPosition, ChartType, Format, FormatAlign, Workbook};
use std::path::Path;

/// 将调洪演算结果写入 Excel 并创建图表
pub fn write_to_excel<T: AsRef<Path>>(steps: &[Step], output_path: T) -> Result<(), Error> {
    // 1. 创建工作簿和工作表
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    let title_format = Format::new()
        .set_bold()
        .set_font_size(14)
        .set_align(FormatAlign::Center);

    // 3. 写入标题（可选）
    worksheet.merge_range(0, 0, 0, 9, "调洪演算结果表", &title_format)?;

    // 4. 写入表头（从第2行开始，索引为1）
    let headers = [
        "时间 (h)",
        "入库流量 (m³/s)",
        "入库平均流量 (m³/s)",
        "时段入库水量 (万m³)",
        "出库流量 (m³/s)",
        "出库平均流量 (m³/s)",
        "时段出库水量 (万m³)",
        "库容变化 (万m³)",
        "总库容 (万m³)",
        "库水位 (m)",
    ];

    for (col, header) in headers.iter().enumerate() {
        worksheet.write_string(1, col as u16, header.to_string())?;
    }

    // 5. 写入数据（从第3行开始，索引为2）
    for (row, step) in steps.iter().enumerate() {
        let excel_row = (row + 2) as u32; // 数据从第3行开始

        worksheet.write_number(excel_row, 0, step.t)?;
        worksheet.write_number(excel_row, 1, step.i)?;
        worksheet.write_number(excel_row, 2, step.i_avg)?;
        worksheet.write_number(excel_row, 4, step.q)?;
        worksheet.write_number(excel_row, 5, step.q_avg)?;
        worksheet.write_number(excel_row, 6, step.v_out)?;
        worksheet.write_number(excel_row, 7, step.delta_v)?;
        worksheet.write_number(excel_row, 8, step.v)?;
        worksheet.write_number(excel_row, 9, step.z)?;
    }

    // 6. 调整列宽
    worksheet.set_column_width(0, 18)?; // 时间
    worksheet.set_column_width(1, 15)?; // 入库流量
    worksheet.set_column_width(2, 15)?; // 入库平均
    worksheet.set_column_width(3, 15)?; // 入库水量
    worksheet.set_column_width(4, 15)?; // 出库流量
    worksheet.set_column_width(5, 15)?; // 出库平均
    worksheet.set_column_width(6, 15)?; // 出库水量
    worksheet.set_column_width(7, 15)?; // 库容变化
    worksheet.set_column_width(8, 15)?; // 总库容
    worksheet.set_column_width(9, 12)?; // 库水位

    // 7. 创建图表（来水与泄水过程线）
    let mut chart = Chart::new(ChartType::ScatterSmooth);

    chart
        .add_series()
        .set_name("来水过程线")
        .set_categories((worksheet.name().as_str(), 2, 0, steps.len() as u32, 0))
        .set_values((worksheet.name().as_str(), 2, 1, steps.len() as u32, 1));

    chart
        .add_series()
        .set_name("泄水过程线")
        .set_categories((worksheet.name().as_str(), 2, 0, steps.len() as u32, 0))
        .set_values((worksheet.name().as_str(), 2, 6, steps.len() as u32, 6));

    chart.x_axis().set_name("时间 t(h)");
    chart.y_axis().set_name("流量 Q(m³/s)");

    chart.legend().set_overlay(true);
    chart.legend().set_position(ChartLegendPosition::TopRight);
    chart.title().set_hidden();

    worksheet.insert_chart_with_offset(0, 11, &chart, 5, 5)?;

    workbook.save(output_path)?;

    Ok(())
}
