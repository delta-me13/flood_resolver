use clap::{Parser, ValueHint};
use std::fs::canonicalize;
use std::path::PathBuf;

mod algorithm;
mod excel;
mod interpolate;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("参数输入错误：{0}")]
    CliValidationError(String),

    #[error(transparent)]
    ExcelIOError(#[from] excel::Error),

    #[error(transparent)]
    ResolutionError(#[from] algorithm::Error),

    #[error(transparent)]
    BuildConfigError(#[from] algorithm::ConfigBuilderError),
}

#[derive(Debug, Parser)]
#[command(name = "调洪演算求解器")]
#[command(about = "基于列表试算法的调洪演算求解器")]
pub struct Args {
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    input_excel: PathBuf,

    /// 输出 Excel 文件路径 (有默认值)
    #[arg(short, long, value_hint = ValueHint::FilePath, default_value = "output_result.xlsx")]
    pub output_excel: PathBuf,

    /// 起调水位 (必填, 单位通常为 m)
    #[arg(short, long)]
    pub initial_water_level: f64,

    /// 死水位 (必填, 单位通常为 m)
    #[arg(short, long)]
    pub dead_water_level: f64,

    /// 迭代计算精度 (例如: 1e-6)
    #[arg(short, long, default_value = "1e-6")]
    pub precision: f64,

    /// 最大迭代次数 (防止不收敛时无限循环)
    #[arg(short, long, default_value = "30")]
    pub max_iterations: usize,

    /// 时间步间距 (单位: h 或 d，视具体模型而定)
    #[arg(short, long, default_value = "0.2")]
    pub time_step: f64,
}

impl Args {
    pub fn validate(&self) -> Result<(), Error> {
        if canonicalize(self.input_excel.clone())
            .map_err(|_| Error::CliValidationError("无法解析文件路径".to_string()))?
            == canonicalize(self.output_excel.clone())
                .map_err(|_| Error::CliValidationError("无法解析文件路径".to_string()))?
        {
            return Err(Error::CliValidationError(
                "输入 Excel 文件路径和输出 Excel 文件路径不能相同".to_string(),
            ));
        }

        if self.initial_water_level < self.dead_water_level {
            return Err(Error::CliValidationError(
                "起调水位必须大于死水位".to_string(),
            ));
        }

        if self.precision <= 0.0 {
            return Err(Error::CliValidationError(
                "迭代计算精度必须大于0".to_string(),
            ));
        }

        if self.max_iterations <= 0 {
            return Err(Error::CliValidationError(
                "最大迭代次数必须大于0".to_string(),
            ));
        }

        if self.time_step <= 0.0 {
            return Err(Error::CliValidationError("时间步间距必须大于0".to_string()));
        }

        Ok(())
    }
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    args.validate()?;

    println!("参数校验通过！准备开始计算...");
    println!("--------------------------------------------------");
    println!("输入文件 : {:?}", args.input_excel);
    println!("输出文件 : {:?}", args.output_excel);
    println!("起调水位 : {} m", args.initial_water_level);
    println!("死水位   : {} m", args.dead_water_level);
    println!("计算精度 : {}", args.precision);
    println!("最大迭代 : {} 次", args.max_iterations);
    println!("时间步长 : {}", args.time_step);
    println!("--------------------------------------------------");

    let excel_data = excel::ExcelData::build(args.input_excel)?;
    let config = algorithm::ConfigBuilder::default()
        .precision(args.precision)
        .sampling_interval(args.time_step)
        .max_iterations(args.max_iterations)
        .dead_level(args.dead_water_level)
        .start_level(args.initial_water_level)
        .build()?;

    let result = algorithm::Algorithm::new(config, excel_data)
        .run_flood_routing()?
        .finish();

    excel::writer::write_to_excel(&result, args.output_excel)?;
    Ok(())
}
