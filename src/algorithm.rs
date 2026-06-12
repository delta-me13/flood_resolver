use crate::excel::ExcelData;
use crate::interpolate::Interpolate;
use derive_builder::Builder;
use roots::{SimpleConvergency, find_root_brent};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    CanNotResolveRoot(#[from] roots::SearchError),

    #[error("单时间步试算出错")]
    StepCalculationFailed,

    #[error("无法找到精确解区间")]
    NoExactSolutionRange,
}

/// 一个调洪结果项， 所有库容均为万立方米
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// 时间 (期末时间 t) - 序列基础字段
    pub t: f64,

    // --- 入库侧 ---
    /// 1. 入库流量 (期末 I₂)
    pub i: f64,
    /// 2. 入库平均流量 (I_avg = (I₁ + I₂) / 2)
    pub i_avg: f64,
    /// 3. 时段入库水量/库容 (V_in = I_avg * Δt)
    pub v_in: f64,

    // --- 出库侧 ---
    /// 4. 出库流量 (期末 q₂)
    pub q: f64,
    /// 5. 出库平均流量 (q_avg = (q₁ + q₂) / 2)
    pub q_avg: f64,
    /// 6. 时段出库水量/库容 (V_out = q_avg * Δt)
    pub v_out: f64,

    // --- 水库状态 ---
    /// 7. 库容变化 (ΔV = V_in - V_out)
    pub delta_v: f64,
    /// 8. 总库容 (期末 V₂)
    pub v: f64,
    /// 9. 库水位 (期末 Z₂)
    pub z: f64,
}

impl Step {
    pub fn from_trial(
        prev_step: &Step,
        t_next: f64,
        i_next: f64,
        q_next: f64,
        v_next: f64,
        z_next: f64,
        time_factor: f64,
    ) -> Self {
        // 入库计算
        let i_avg = (prev_step.i + i_next) / 2.0;
        let v_in = i_avg * time_factor / 1e4;

        // 出库计算
        let q_avg = (prev_step.q + q_next) / 2.0;
        let v_out = q_avg * time_factor / 1e4;

        // 水库状态计算
        let delta_v = v_in - v_out;

        Self {
            t: t_next,
            i: i_next,
            i_avg,
            v_in,
            q: q_next,
            q_avg,
            v_out,
            delta_v,
            v: v_next,
            z: z_next,
        }
    }

    /// 构建初始步（t=0时没有时段变化，所以入库与出库水量为0）
    pub fn initial(t: f64, i: f64, q: f64, v: f64, z: f64) -> Self {
        Self {
            t,
            i,
            i_avg: i,
            v_in: 0.0,
            q,
            q_avg: q,
            v_out: 0.0,
            delta_v: 0.0,
            v,
            z,
        }
    }
}

#[derive(Builder, Debug)]
pub struct Config {
    /// 精度与最小误差
    #[builder(setter(into))]
    precision: f64,
    /// 采样间隔
    #[builder(setter(into))]
    sampling_interval: f64,
    /// 最大迭代次数
    #[builder(setter(into))]
    max_iterations: usize,
    /// 死水位
    #[builder(setter(into))]
    dead_level: f64,
    /// 起调水位
    #[builder(setter(into))]
    start_level: f64,
}

/// 具体算法实现
pub struct Algorithm {
    curves: ExcelData,
    config: Config,
    steps: Vec<Step>,
}

impl Algorithm {
    // 方便直接调用，因为采样间隔是按小时计的, 公式按秒计算
    const HOUR_TOSECOND: f64 = 3600.0;

    pub fn new(config: Config, curves: ExcelData) -> Self {
        Self {
            config,
            steps: Vec::new(),
            curves,
        }
    }

    /// 结果依次为 z_next, v_next, q_next
    fn trial_calculation(&self, prev_step: &Step, t_next: f64) -> Result<(f64, f64, f64), Error> {
        let delta_t = t_next - prev_step.t;
        let time_factor = delta_t * Self::HOUR_TOSECOND;

        // 公式变形
        // V2​+q2​​Δt/2=(V1​−q1​​Δt/2)+(I1​+I2)/2*​​Δt
        // 左端函数是 F(Z2)，右端为一个常量C
        // 构建残差函数Fcalc=F(Z2)−C, 求解Z2。
        // 为了方便求解
        // v的单位为104m3。所有参与计算的v都需要换算单位

        let i_avg = (prev_step.i + self.curves.water_comming.get(t_next)) / 2.0;

        // 计算常数项
        let c = (prev_step.v - prev_step.q * time_factor) / 2.0 + i_avg * time_factor;

        // 构建残差函数
        let residual = |z2: f64| {
            let v = self.curves.water_level_with_storage.get(z2) * 1e4;
            let q = self.curves.water_level_with_discharge.get(z2);
            (v + q * time_factor / 2.0) - c
        };

        // 确定z上限
        let v_max_possible = (prev_step.v + i_avg * time_factor) * 1e4;
        let z_max = (self.curves.water_level_with_storage.get_reverse(
            v_max_possible,
            self.curves.water_level_with_storage.domain(),
        )? + 0.1)
            .min(self.curves.water_level_with_storage.range().1);

        // 确定下限
        let z_min = if i_avg > prev_step.q {
            // 涨水段，下限水位为当前水位, 为了防止精度问题减0.1且大于死水位
            (prev_step.z - 0.1).max(self.config.dead_level)
        } else {
            // 退水段，下限水位为死水位
            self.config.dead_level
        };

        // debug用校验边界符号
        debug_assert!(residual(z_max) < 0.0);
        debug_assert!(residual(z_min) > 0.0);

        let mut convergency = SimpleConvergency {
            eps: self.config.precision,
            max_iter: self.config.max_iterations,
        };

        let z_next = find_root_brent(z_min, z_max, residual, &mut convergency)
            .map_err(|_| Error::StepCalculationFailed)?;

        // 这里不换算V单位是方便登记
        Ok((
            z_next,
            self.curves.water_level_with_storage.get(z_next),
            self.curves.water_level_with_discharge.get(z_next),
        ))
    }

    pub fn run_flood_routing(&mut self) -> Result<&mut Self, Error> {
        let init_step = Step::initial(
            0.0,
            self.curves.water_comming.get(0.0),
            self.curves
                .water_level_with_discharge
                .get(self.config.start_level),
            self.curves
                .water_level_with_storage
                .get(self.config.start_level),
            self.config.start_level,
        );

        self.steps.push(init_step.clone());
        let mut prev_step = init_step;

        let (mut t, end_t) = self.curves.water_comming.domain();
        while t < end_t {
            let t_next = (t + self.config.sampling_interval).min(end_t);

            let (z_next, v_next, q_next) = self.trial_calculation(&prev_step, t_next)?;

            let current_step = Step::from_trial(
                &prev_step,
                t_next,
                self.curves.water_comming.get(t_next),
                q_next,
                v_next,
                z_next,
                (t_next - t) * Self::HOUR_TOSECOND,
            );

            self.steps.push(current_step.clone());
            t = t_next; // 更新时间
            prev_step = current_step; // 更新前一步
        }

        // 开始寻找精确的解。查找库容变化编号的时间
        let (left, right) = self
            .steps
            .windows(2)
            .enumerate()
            .find_map(|(idx, pair)| {
                if pair[0].delta_v * pair[1].delta_v < 0.0 {
                    Some((idx, idx + 1))
                } else {
                    None
                }
            })
            .ok_or(Error::NoExactSolutionRange)?;

        let mut convergency = SimpleConvergency {
            eps: self.config.precision,
            max_iter: self.config.max_iterations,
        };
        let t = self.curves.water_comming.domain().0;
        let t_left = left as f64 * self.config.sampling_interval + t;
        let t_right = right as f64 * self.config.sampling_interval + t;

        let exact_t = find_root_brent(
            t_left,
            t_right,
            self.build_residual_mapping(t_left, t_right, self.steps[left].v, self.steps[right].v),
            &mut convergency,
        )?;

        let i_q_next = self.curves.water_comming.get(exact_t);
        let z_next = self
            .curves
            .water_level_with_discharge
            .get_reverse(i_q_next, self.curves.water_level_with_discharge.domain())?;
        let v_next = self.curves.water_level_with_storage.get(z_next);

        let step_exact = Step::from_trial(
            &self.steps[left],
            t_left + exact_t,
            i_q_next,
            i_q_next,
            v_next,
            z_next,
            exact_t * Self::HOUR_TOSECOND,
        );

        self.steps.insert(left + 1, step_exact);

        Ok(self)
    }

    /// 精确解的残差函数，入库流量等于出库流量时有解
    /// 使用库容映射法，求下泄，因为其公式本身不是线性的而库容在这个精确解区间内变化幅度小，近似线性，可不用试算
    fn build_residual_mapping(
        &self,
        t_left: f64,
        t_right: f64,
        v_left: f64,
        v_right: f64,
    ) -> impl FnMut(f64) -> f64 {
        move |t| -> f64 {
            // t时刻入库流量
            let i_t = self.curves.water_comming.get(t);
            let dt_total = t_right - t_left;

            // 某个精度下甚至不用插值可以近似
            let v_t = if dt_total > 1e-9 {
                v_left + (v_right - v_left) * (t - t_left) / dt_total
            } else {
                v_left
            };

            let z_t = self
                .curves
                .water_level_with_storage
                .get_reverse(v_t, self.curves.water_level_with_storage.domain())
                .expect(&format!("Z->V 曲线反查, 非正常错误: v={}", v_t));
            let q_t = self.curves.water_level_with_discharge.get(z_t);

            i_t - q_t
        }
    }

    // pub fn finish<'a>(&'a self) -> &'a [Step] {
    //     &self.steps
    // }
    pub fn finish(&self) -> Vec<Step> {
        self.steps.clone()
    }
}
