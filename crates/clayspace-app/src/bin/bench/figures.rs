//! One measured quantity, and the arithmetic for judging it.

use std::time::Duration;

/// One measured quantity.
#[derive(Debug, Clone)]
pub struct Figure {
    /// Milliseconds, megabytes or a count — stated by `unit`.
    pub value: f64,
    pub unit: &'static str,
    /// What it must not exceed, when the specification states one.
    pub budget: Option<f64>,
    /// How much worse a run may be than the baseline before the gate fails.
    ///
    /// Timings on a shared CI runner move around by tens of percent for
    /// reasons that have nothing to do with the change under test, so a gate
    /// that fails on any regression fails constantly and gets ignored.
    pub tolerance: f64,
    /// Below this the figure is too small to have a meaningful ratio.
    ///
    /// Backend discovery measures 0.00 ms; against a baseline of zero, any
    /// value at all is an infinite regression. A ratio needs something to be a
    /// ratio *of*.
    pub noise_floor: f64,
}

impl Figure {
    /// Whether this is worse than the baseline by more than noise.
    pub fn regressed_against(&self, baseline: f64) -> bool {
        if self.value <= self.noise_floor && baseline <= self.noise_floor {
            return false;
        }
        self.value / baseline.max(f64::MIN_POSITIVE) > self.tolerance
    }

    pub fn ms(value: f64, budget: Option<f64>) -> Self {
        // A millisecond: below that the measurement is scheduling noise.
        Self {
            value,
            unit: "ms",
            budget,
            tolerance: 1.5,
            noise_floor: 1.0,
        }
    }

    pub fn count(value: f64) -> Self {
        Self {
            value,
            unit: "",
            budget: None,
            tolerance: 1.25,
            noise_floor: 0.0,
        }
    }

    pub fn mb(value: f64) -> Self {
        Self {
            value,
            unit: "MB",
            budget: None,
            tolerance: 1.25,
            noise_floor: 0.5,
        }
    }

    /// A ratio between two figures taken moments apart on the same machine.
    ///
    /// The kind of number that survives a change of machine, which an absolute
    /// timing does not.
    pub fn ratio(value: f64, budget: Option<f64>, tolerance: f64) -> Self {
        Self {
            value,
            unit: "x",
            budget,
            tolerance,
            noise_floor: 0.0,
        }
    }
}

/// What a figure's samples looked like, beside the one number it reports.
///
/// The gap this closes is the one ClayCore's own v0.78.0 release notes name in
/// their device gate: "`sdf_stroke_bricks` cannot tell a 1.1x from noise. It
/// takes one observation per point and records no spread." This harness took
/// twelve samples of a repeatable measurement and three of a one-shot, reduced
/// each set to one or two numbers, and then dropped the samples on the floor —
/// so a baseline could state that a figure was 19.04 ms and nothing at all
/// about whether 21 ms was a regression or a Tuesday.
///
/// Five numbers rather than a standard deviation, because these are not normal
/// and are not meant to be: a gesture's segments rise across the stroke by
/// construction (see [`Record::figures`]), so a spread here is a *range* that
/// says how far apart the cheapest and the dearest sample were, not a
/// dispersion about a centre. `n` is part of it because a range over three
/// samples and a range over twelve are different claims.
///
/// **This is within-run spread and it is not the whole answer.** The variance
/// that dominates is between runs — a cold graphics card measured 11.9 ms
/// against a boosted one's 7.8 for the same figure, which is why
/// `groups::warmup` exists — and one process cannot sample that. The way to
/// see it is to run the whole harness several times per pin and read the
/// medians against each other. What this buys is the floor under that: a
/// change smaller than the spread of either side was never distinguishable to
/// begin with, and now the file says so.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spread {
    /// How many samples the figure was reduced from.
    pub n: usize,
    pub min: f64,
    pub median: f64,
    pub p95: f64,
    pub max: f64,
}

impl Spread {
    /// Summarises a sample set, or `None` where there was nothing to sample.
    pub fn of(samples: &[f64]) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }
        let mut sorted = samples.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
        Some(Self {
            n: sorted.len(),
            min: sorted[0],
            median: quantile(&sorted, 0.5),
            p95: quantile(&sorted, 0.95),
            max: sorted[sorted.len() - 1],
        })
    }

    /// Whether a value sits inside the range these samples covered.
    ///
    /// The question a comparison wants and could not previously ask: a run
    /// reporting 21 ms against a baseline of 19 has not necessarily got worse
    /// if the baseline's own twelve samples ran from 17 to 23.
    pub fn covers(&self, value: f64) -> bool {
        value >= self.min && value <= self.max
    }

    /// `12x 17.10-23.40`, for the report table.
    pub fn describe(&self) -> String {
        format!("{}x {:.2}-{:.2}", self.n, self.min, self.max)
    }
}

pub fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

pub fn mean(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples.iter().sum::<f64>() / samples.len() as f64
}

pub fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let at = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[at]
}

/// How a figure is taken.
///
/// The one place the cost/noise trade-off is stated. A measurement declares
/// which kind it is and gets its sample count and its tolerance from here,
/// rather than each call site choosing its own and nobody being able to see
/// the whole picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Record {
    /// Dabs onto a document that carries them: samples are cheap, so there are
    /// enough of them for a median and a 95th percentile to mean something.
    Repeatable,
    /// A conversion, a bake, an export. The document has to be rebuilt between
    /// samples — the second conversion of a layer is not the first — so
    /// samples are expensive and few, and the tolerance is widened to match.
    OneShot,
}

impl Record {
    /// How many samples to take.
    pub const fn samples(self) -> usize {
        match self {
            Self::Repeatable => 12,
            Self::OneShot => 3,
        }
    }

    /// How much worse than the baseline a figure may be before the gate fails.
    ///
    /// Three samples of an expensive operation move around more than twelve of
    /// a cheap one. A doubling is still caught at 2.0; a 40 % drift is not,
    /// and that is the price of measuring these at all.
    pub const fn tolerance(self) -> f64 {
        match self {
            Self::Repeatable => 1.5,
            Self::OneShot => 2.0,
        }
    }

    /// And how much worse a *tail* figure may be.
    ///
    /// Wider, because a 95th percentile of twelve samples is the second
    /// largest of them: one sample delayed by anything at all — an allocator,
    /// a driver, another process on the machine — moves it and moves nothing
    /// else. Measured on an unchanged tree, `brush.mesh.camada.p95` came out
    /// between 22.3 ms and 29.0 ms across four runs while its median stayed
    /// inside 19.3 to 21.3. At 1.5 that is a gate which fails on a tree nobody
    /// touched, and a gate that cries wolf is one people learn to pass with
    /// `--no-verify`.
    ///
    /// The median is what catches an operation getting slower. This catches a
    /// tail that has *doubled*, which is a different and rarer claim.
    pub const fn tail_tolerance(self) -> f64 {
        2.0
    }

    /// The figures a set of timings yields, named under `prefix`.
    ///
    /// Two for a repeatable measurement, because the tail is what a sculptor
    /// feels; one for a one-shot, because a 95th percentile of three samples
    /// is the largest of three and says nothing.
    ///
    /// The mean and not the median, which is the opposite of the usual advice
    /// and is right here. A stroke's segments are not repeated measurements of
    /// one quantity: each dab lands on more surface and a longer tape than the
    /// one before, so the samples rise across the gesture — measured,
    /// `brush.sdf.padrao` ran 4.7, 5.6, 5.6, 8.2, 8.2, 8.2, 8.7, 12.5, 14.8,
    /// 15.0, 16.3, 18.9, 21.7 ms. There is a gap in the middle of that, the
    /// median falls in it, and which side it lands on is decided by noise: the
    /// same unchanged code reported 8.68, 11.36 and 8.01 on three consecutive
    /// runs. The mean of those same three sample sets was 11.41, 11.66 and
    /// 10.70.
    ///
    /// A median is the robust statistic when the samples are one quantity
    /// measured repeatedly. These are a gesture's worth of different dabs, and
    /// what a sculptor pays for the gesture is their sum.
    pub fn figures(self, prefix: &str, samples: &[f64]) -> Vec<(String, Figure)> {
        let mut samples = samples.to_vec();
        samples.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
        if std::env::var_os("CLAYSPACE_BENCH_SAMPLES").is_some() {
            let each: Vec<String> = samples.iter().map(|s| format!("{s:.2}")).collect();
            println!("  {prefix}: {}", each.join(" "));
        }
        let at = |q: f64, tolerance: f64| Figure {
            tolerance,
            ..Figure::ms(quantile(&samples, q), None)
        };
        match self {
            Self::Repeatable => vec![
                (
                    format!("{prefix}.mean"),
                    Figure {
                        tolerance: self.tolerance(),
                        ..Figure::ms(mean(&samples), None)
                    },
                ),
                (format!("{prefix}.p95"), at(0.95, self.tail_tolerance())),
            ],
            // Three samples of the same operation on three rebuilt documents
            // *are* one quantity measured repeatedly, so the median is the
            // right statistic and the outlier is what it is there to drop.
            Self::OneShot => vec![(format!("{prefix}.ms"), at(0.5, self.tolerance()))],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tail figure is judged more loosely than the median beside it, and the
    /// gate depends on that: measured across four runs of an unchanged tree, a
    /// mesh brush's p95 moved by 30 % while its median moved by 10 %.
    #[test]
    fn a_tail_is_judged_more_loosely_than_the_figure_beside_it() {
        let figures = Record::Repeatable.figures("brush.mesh.camada", &[10.0; 12]);
        let tolerances: Vec<f64> = figures.iter().map(|(_, f)| f.tolerance).collect();
        assert_eq!(tolerances, vec![1.5, 2.0]);
    }

    /// The sample set that made the case: a median lands in the gap and moves
    /// with it, and the mean does not.
    #[test]
    fn a_gesture_is_reported_by_its_mean_rather_than_its_middle() {
        let measured = vec![
            4.72, 5.60, 5.63, 8.16, 8.17, 8.23, 8.68, 12.47, 14.79, 15.03, 16.28, 18.91, 21.68,
        ];
        let figures = Record::Repeatable.figures("brush.sdf.padrao", &measured);
        assert_eq!(figures[0].0, "brush.sdf.padrao.mean");
        assert!(
            (figures[0].1.value - 11.41).abs() < 0.01,
            "{}",
            figures[0].1.value
        );
        // Which is nowhere near the middle element the median would have taken.
        assert_eq!(quantile(&measured, 0.5), 8.68);
    }

    #[test]
    fn a_one_shot_reports_one_figure_and_no_tail() {
        let figures = Record::OneShot.figures("convert.sdf_to_voxel", &[1.0, 2.0, 3.0]);
        assert_eq!(figures.len(), 1);
        assert_eq!(figures[0].0, "convert.sdf_to_voxel.ms");
        assert_eq!(figures[0].1.value, 2.0);
    }

    /// The claim the whole spread exists to make: a change inside the range
    /// the baseline's own samples covered is not distinguishable from the
    /// noise that baseline was recorded in.
    #[test]
    fn a_spread_says_whether_a_value_is_inside_what_was_already_seen() {
        let spread = Spread::of(&[17.1, 18.0, 19.0, 20.2, 23.4]).expect("five samples");
        assert_eq!(spread.n, 5);
        assert_eq!(spread.min, 17.1);
        assert_eq!(spread.max, 23.4);
        assert_eq!(spread.median, 19.0);
        assert!(spread.covers(21.0));
        assert!(!spread.covers(24.0));
    }

    #[test]
    fn a_measurement_with_no_samples_states_no_spread() {
        assert_eq!(Spread::of(&[]), None);
    }

    /// Unsorted in, sorted out — every caller in the harness hands over the
    /// samples in the order they were taken.
    #[test]
    fn a_spread_does_not_care_what_order_the_samples_arrived_in() {
        let spread = Spread::of(&[9.0, 1.0, 5.0]).expect("three samples");
        assert_eq!((spread.min, spread.median, spread.max), (1.0, 5.0, 9.0));
    }

    #[test]
    fn a_figure_at_the_noise_floor_does_not_regress_against_nothing() {
        let discovery = Figure::ms(0.4, None);
        assert!(!discovery.regressed_against(0.0007));
    }
}
