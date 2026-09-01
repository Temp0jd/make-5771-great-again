use image::{GrayImage, RgbaImage};
use serde::{Deserialize, Serialize};

/// Which template-matching engine to use. The default is `Precise`; `Classic`
/// keeps the previous implementation unchanged for compatibility checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MatchAlgorithm {
    /// RGB three-channel matching: integral-image lower-bound prefilter plus
    /// banded multithreading; strongest against misclicks.
    #[default]
    Precise,
    /// Grayscale: integral-image lower-bound prefilter plus banded
    /// multithreading.
    Fast,
    /// The original grayscale strided pre-scan, single-threaded.
    Classic,
}

impl MatchAlgorithm {
    pub const ALL: [Self; 3] = [Self::Precise, Self::Fast, Self::Classic];

    pub fn label(self) -> &'static str {
        match self {
            Self::Precise => "精准（彩色，推荐）",
            Self::Fast => "极速（灰度，最快）",
            Self::Classic => "经典（旧算法）",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl SearchRegion {
    pub fn full(image: &impl image::GenericImageView) -> Self {
        Self {
            x: 0,
            y: 0,
            width: image.width(),
            height: image.height(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemplateMatch {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub score: f32,
}

impl TemplateMatch {
    pub fn center(self) -> (u32, u32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }
}

/// Diagnostic result of a template search: the threshold-passing match (if
/// any) plus the highest similarity seen anywhere, for timeout diagnostics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchReport {
    pub matched: Option<TemplateMatch>,
    /// Best similarity seen in the region even below the threshold; positions
    /// rejected by the coarse pass contribute their sampled estimate.
    pub best_score: f32,
}

/// Finds the closest grayscale template using mean absolute pixel similarity,
/// dispatching on the configured algorithm. `Precise` needs RGB input; the
/// grayscale entry falls back to `Fast` for it.
pub fn find_template_report(
    frame: &GrayImage,
    template: &GrayImage,
    region: SearchRegion,
    threshold: f32,
    algorithm: MatchAlgorithm,
) -> MatchReport {
    match algorithm {
        MatchAlgorithm::Classic => find_classic(frame, template, region, threshold),
        MatchAlgorithm::Fast | MatchAlgorithm::Precise => {
            find_fast(frame, template, region, threshold)
        }
    }
}

/// Finds the closest RGB template using three-channel mean absolute
/// similarity (the `Precise` engine: integral-image lower-bound prefilter
/// plus banded multithreading, deterministic merge).
pub fn find_template_report_rgb(
    frame: &RgbaImage,
    template: &RgbaImage,
    region: SearchRegion,
    threshold: f32,
) -> MatchReport {
    let bands = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .min(MAX_FAST_BANDS);
    find_precise_impl(frame, template, region, threshold, bands)
}

/// The original implementation: pixel access goes through the raw image
/// buffers, and large search areas are pre-scanned on a stride-4 grid; only
/// grid points whose coarse score is within 0.08 of the threshold get a
/// full-resolution refinement of their ±3 neighborhood. Small areas are
/// scanned densely. Single-threaded; kept unchanged for compatibility.
fn find_classic(
    frame: &GrayImage,
    template: &GrayImage,
    region: SearchRegion,
    threshold: f32,
) -> MatchReport {
    let empty = MatchReport {
        matched: None,
        best_score: 0.0,
    };
    if template.width() == 0
        || template.height() == 0
        || region.width < template.width()
        || region.height < template.height()
        || region.x.saturating_add(region.width) > frame.width()
        || region.y.saturating_add(region.height) > frame.height()
    {
        return empty;
    }

    let threshold = threshold.clamp(0.0, 1.0);
    let pixel_count = template.width() as u64 * template.height() as u64;
    // A candidate only qualifies when its total pixel difference stays within
    // this budget; anything worse than the best match so far is pointless too.
    let max_diff = ((1.0 - threshold) * pixel_count as f32 * 255.0) as u64;
    let max_x = region.x + region.width - template.width();
    let max_y = region.y + region.height - template.height();
    let scan = Scan {
        frame: frame.as_raw(),
        frame_width: frame.width() as usize,
        template: template.as_raw(),
        template_width: template.width() as usize,
        template_height: template.height() as usize,
    };
    let mut state = ScanState::default();

    let position_count = u64::from(max_x - region.x + 1) * u64::from(max_y - region.y + 1);
    if position_count <= DENSE_SCAN_LIMIT {
        for y in region.y..=max_y {
            for x in region.x..=max_x {
                state.consider(&scan, x, y, threshold, max_diff, pixel_count);
            }
        }
    } else {
        for grid_y in (region.y..=max_y).step_by(PRESCAN_STRIDE as usize) {
            for grid_x in (region.x..=max_x).step_by(PRESCAN_STRIDE as usize) {
                let coarse = scan.coarse_score(grid_x, grid_y);
                state.best_score = state.best_score.max(coarse);
                if coarse + 0.08 < threshold {
                    continue;
                }
                let from_x = grid_x.saturating_sub(PRESCAN_STRIDE - 1).max(region.x);
                let from_y = grid_y.saturating_sub(PRESCAN_STRIDE - 1).max(region.y);
                let to_x = (grid_x + PRESCAN_STRIDE - 1).min(max_x);
                let to_y = (grid_y + PRESCAN_STRIDE - 1).min(max_y);
                for y in from_y..=to_y {
                    for x in from_x..=to_x {
                        state.consider(&scan, x, y, threshold, max_diff, pixel_count);
                    }
                }
            }
        }
    }

    MatchReport {
        matched: state.best,
        best_score: state.best_score,
    }
}

/// Search areas up to this many candidate positions are scanned exhaustively;
/// beyond it the stride pre-scan kicks in.
const DENSE_SCAN_LIMIT: u64 = 65_536;

/// Grid spacing of the coarse pre-scan over large search areas.
const PRESCAN_STRIDE: u32 = 4;

/// Raw-buffer view of the frame and template (row-major, stride = width).
struct Scan<'a> {
    frame: &'a [u8],
    frame_width: usize,
    template: &'a [u8],
    template_width: usize,
    template_height: usize,
}

impl Scan<'_> {
    #[inline]
    fn frame_at(&self, x: u32, y: u32) -> u8 {
        self.frame[y as usize * self.frame_width + x as usize]
    }

    #[inline]
    fn template_at(&self, x: u32, y: u32) -> u8 {
        self.template[y as usize * self.template_width + x as usize]
    }

    fn coarse_score(&self, origin_x: u32, origin_y: u32) -> f32 {
        // Keep the cheap rejection pass near a fixed sample budget even when
        // the selected template is large. This is the main guard against high
        // CPU use.
        let pixel_count = self.template_width * self.template_height;
        let sample_step = ((pixel_count as f32 / 144.0).sqrt().ceil() as usize).max(2);
        let mut difference = 0_u64;
        let mut samples = 0_u64;

        for ty in (0..self.template_height).step_by(sample_step) {
            for tx in (0..self.template_width).step_by(sample_step) {
                difference += self
                    .frame_at(origin_x + tx as u32, origin_y + ty as u32)
                    .abs_diff(self.template_at(tx as u32, ty as u32))
                    as u64;
                samples += 1;
            }
        }

        1.0 - difference as f32 / (samples.max(1) as f32 * 255.0)
    }

    /// Returns the summed absolute pixel difference at `origin`, bailing out
    /// as soon as the accumulated difference reaches `abort_at` (meaning the
    /// position can no longer qualify or beat the current best match).
    fn diff_at(&self, origin_x: u32, origin_y: u32, abort_at: u64) -> Option<u64> {
        let mut difference = 0_u64;
        for ty in 0..self.template_height as u32 {
            for tx in 0..self.template_width as u32 {
                difference += self
                    .frame_at(origin_x + tx, origin_y + ty)
                    .abs_diff(self.template_at(tx, ty)) as u64;
            }
            if difference >= abort_at {
                return None;
            }
        }
        Some(difference)
    }
}

struct ScanState {
    best: Option<TemplateMatch>,
    best_diff: u64,
    best_score: f32,
}

impl Default for ScanState {
    fn default() -> Self {
        Self {
            best: None,
            best_diff: u64::MAX,
            best_score: 0.0,
        }
    }
}

impl ScanState {
    /// Full-resolution evaluation of one candidate position: coarse rejection,
    /// then the exact difference when the position looks promising.
    fn consider(
        &mut self,
        scan: &Scan<'_>,
        x: u32,
        y: u32,
        threshold: f32,
        max_diff: u64,
        pixel_count: u64,
    ) {
        let coarse = scan.coarse_score(x, y);
        self.best_score = self.best_score.max(coarse);
        if coarse + 0.06 < threshold {
            return;
        }
        let abort_at = max_diff.saturating_add(1).min(self.best_diff);
        let Some(difference) = scan.diff_at(x, y, abort_at) else {
            return;
        };
        let score = 1.0 - difference as f32 / (pixel_count as f32 * 255.0);
        self.best_score = self.best_score.max(score);
        self.best_diff = difference;
        self.best = Some(TemplateMatch {
            x,
            y,
            width: scan.template_width as u32,
            height: scan.template_height as u32,
            score,
        });
    }
}

/// Cap on the number of bands (threads) used by the fast path.
const MAX_FAST_BANDS: usize = 8;

/// Fast engine: same coarse + exact-difference scoring as the classic one,
/// but each candidate position first goes through an exact lower-bound filter
/// and large areas are processed in parallel bands.
///
/// The lower bound is exact: for a window W and template T,
/// `sum|a-b| >= |sum(W) - sum(T)|` (triangle inequality), so any position
/// with `|window_sum - template_sum| > max_diff` has a true difference above
/// the qualifying budget and can never become a match — zero false negatives.
/// Window sums are O(1) via a per-band u64 summed-area table.
///
/// Determinism: a band-local winner is the lexicographically smallest
/// (diff, y, x); merging bands with the same rule is order-independent, so
/// the multi-threaded result equals the single-threaded one exactly.
fn find_fast(
    frame: &GrayImage,
    template: &GrayImage,
    region: SearchRegion,
    threshold: f32,
) -> MatchReport {
    let bands = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .min(MAX_FAST_BANDS);
    find_fast_impl(frame, template, region, threshold, bands)
}

fn find_fast_impl(
    frame: &GrayImage,
    template: &GrayImage,
    region: SearchRegion,
    threshold: f32,
    max_bands: usize,
) -> MatchReport {
    let scan = Scan {
        frame: frame.as_raw(),
        frame_width: frame.width() as usize,
        template: template.as_raw(),
        template_width: template.width() as usize,
        template_height: template.height() as usize,
    };
    find_banded(&scan, region, threshold, max_bands)
}

fn find_precise_impl(
    frame: &RgbaImage,
    template: &RgbaImage,
    region: SearchRegion,
    threshold: f32,
    max_bands: usize,
) -> MatchReport {
    let scan = RgbScan {
        frame: frame.as_raw(),
        frame_width: frame.width() as usize,
        template: template.as_raw(),
        template_width: template.width() as usize,
        template_height: template.height() as usize,
    };
    find_banded(&scan, region, threshold, max_bands)
}

/// What the banded engines need from a concrete scan (grayscale or RGB).
trait ScanOps: Sync {
    fn frame_size(&self) -> (u32, u32);
    fn template_size(&self) -> (u32, u32);
    /// Maximum absolute pixel difference per pixel: 255 gray, 765 RGB.
    fn channel_scale(&self) -> f32;
    /// Sum of the matched channels over the whole template.
    fn template_sum(&self) -> u64;
    fn coarse_score(&self, origin_x: u32, origin_y: u32) -> f32;
    fn diff_at(&self, origin_x: u32, origin_y: u32, abort_at: u64) -> Option<u64>;
    /// Window-sum table over frame pixels starting at (x0, y0), w × h pixels;
    /// a pixel's value is the sum of its matched channels.
    fn build_sum_table(&self, x0: u32, y0: u32, w: usize, h: usize) -> SumTable;
}

/// Window-sum table for the lower-bound prefilter.
enum SumTable {
    /// Grayscale windows fit u32; wrapping arithmetic keeps them exact.
    U32 { data: Vec<u32>, stride: usize },
    /// RGB channel-sum windows get u64 headroom.
    U64 { data: Vec<u64>, stride: usize },
}

impl SumTable {
    /// Sum of the w × h window at frame position (x, y); (x0, y0) is the
    /// table origin in frame coordinates.
    fn window_sum(&self, x: u32, y: u32, w: u32, h: u32, x0: u32, y0: u32) -> u64 {
        let row = (y - y0) as usize;
        let col = (x - x0) as usize;
        let bottom = row + h as usize;
        let right = col + w as usize;
        match self {
            Self::U32 { data, stride } => u64::from(
                data[bottom * stride + right]
                    .wrapping_add(data[row * stride + col])
                    .wrapping_sub(data[row * stride + right])
                    .wrapping_sub(data[bottom * stride + col]),
            ),
            Self::U64 { data, stride } => {
                data[bottom * stride + right] + data[row * stride + col]
                    - data[row * stride + right]
                    - data[bottom * stride + col]
            }
        }
    }
}

/// Shared banded engine behind `Fast` (grayscale) and `Precise` (RGB).
fn find_banded<S: ScanOps>(
    scan: &S,
    region: SearchRegion,
    threshold: f32,
    max_bands: usize,
) -> MatchReport {
    let empty = MatchReport {
        matched: None,
        best_score: 0.0,
    };
    let (template_width, template_height) = scan.template_size();
    let (frame_width, frame_height) = scan.frame_size();
    if template_width == 0
        || template_height == 0
        || region.width < template_width
        || region.height < template_height
        || region.x.saturating_add(region.width) > frame_width
        || region.y.saturating_add(region.height) > frame_height
    {
        return empty;
    }

    let threshold = threshold.clamp(0.0, 1.0);
    let pixel_count = u64::from(template_width) * u64::from(template_height);
    let channel_scale = scan.channel_scale();
    let max_diff = ((1.0 - threshold) * pixel_count as f32 * channel_scale) as u64;
    let max_x = region.x + region.width - template_width;
    let max_y = region.y + region.height - template_height;

    let position_count = u64::from(max_x - region.x + 1) * u64::from(max_y - region.y + 1);
    if position_count <= DENSE_SCAN_LIMIT {
        // Small areas: dense single-threaded scan, no summed-area table.
        let mut state = FastState::default();
        for y in region.y..=max_y {
            for x in region.x..=max_x {
                state.consider(scan, x, y, threshold, max_diff, pixel_count);
            }
        }
        return state.report(template_width, template_height, pixel_count, channel_scale);
    }

    let grid_rows = (max_y - region.y) / PRESCAN_STRIDE + 1;
    let bands = max_bands.clamp(1, (grid_rows as usize).min(MAX_FAST_BANDS));
    let fast = FastScan {
        scan,
        threshold,
        max_diff,
        pixel_count,
        template_sum: scan.template_sum(),
        region,
        max_x,
        max_y,
    };
    let band_range = |band: usize| -> (u32, u32) {
        let k0 = grid_rows * band as u32 / bands as u32;
        let k1 = grid_rows * (band as u32 + 1) / bands as u32;
        (k0, k1)
    };
    let mut states = Vec::with_capacity(bands);
    if bands == 1 {
        let (k0, k1) = band_range(0);
        states.push(scan_band(&fast, k0, k1));
    } else {
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(bands);
            for band in 0..bands {
                let (k0, k1) = band_range(band);
                let fast = &fast;
                handles.push(scope.spawn(move || scan_band(fast, k0, k1)));
            }
            for handle in handles {
                if let Ok(state) = handle.join() {
                    states.push(state);
                }
            }
        });
    }

    let mut best: Option<(u64, u32, u32)> = None;
    let mut best_score = 0.0_f32;
    for state in states {
        best_score = best_score.max(state.best_score);
        if let Some(candidate) = state.best
            && best.is_none_or(|current| candidate < current)
        {
            best = Some(candidate);
        }
    }
    MatchReport {
        matched: best.map(|(difference, y, x)| TemplateMatch {
            x,
            y,
            width: template_width,
            height: template_height,
            score: 1.0 - difference as f32 / (pixel_count as f32 * channel_scale),
        }),
        best_score,
    }
}

/// Raw-buffer view of RGBA frame and template (row-major, stride = 4·width);
/// only the R/G/B channels participate, alpha is ignored.
struct RgbScan<'a> {
    frame: &'a [u8],
    frame_width: usize,
    template: &'a [u8],
    template_width: usize,
    template_height: usize,
}

impl RgbScan<'_> {
    fn coarse_score(&self, origin_x: u32, origin_y: u32) -> f32 {
        // Same fixed sample budget as the grayscale coarse pass.
        let pixel_count = self.template_width * self.template_height;
        let sample_step = ((pixel_count as f32 / 144.0).sqrt().ceil() as usize).max(2);
        let mut difference = 0_u64;
        let mut samples = 0_u64;

        for ty in (0..self.template_height).step_by(sample_step) {
            for tx in (0..self.template_width).step_by(sample_step) {
                let frame_base =
                    ((origin_y as usize + ty) * self.frame_width + origin_x as usize + tx) * 4;
                let template_base = (ty * self.template_width + tx) * 4;
                for channel in 0..3 {
                    difference += self.frame[frame_base + channel]
                        .abs_diff(self.template[template_base + channel])
                        as u64;
                }
                samples += 1;
            }
        }

        1.0 - difference as f32 / (samples.max(1) as f32 * 765.0)
    }

    /// Three-channel summed absolute difference at `origin`, bailing out as
    /// soon as the accumulated difference reaches `abort_at`.
    fn diff_at(&self, origin_x: u32, origin_y: u32, abort_at: u64) -> Option<u64> {
        let mut difference = 0_u64;
        let mut frame_row = (origin_y as usize * self.frame_width + origin_x as usize) * 4;
        let mut template_row = 0_usize;
        for _ in 0..self.template_height {
            let frame_pixels = &self.frame[frame_row..frame_row + self.template_width * 4];
            let template_pixels =
                &self.template[template_row..template_row + self.template_width * 4];
            for px in 0..self.template_width {
                let i = px * 4;
                difference += u64::from(frame_pixels[i].abs_diff(template_pixels[i]))
                    + u64::from(frame_pixels[i + 1].abs_diff(template_pixels[i + 1]))
                    + u64::from(frame_pixels[i + 2].abs_diff(template_pixels[i + 2]));
            }
            if difference >= abort_at {
                return None;
            }
            frame_row += self.frame_width * 4;
            template_row += self.template_width * 4;
        }
        Some(difference)
    }
}

impl ScanOps for RgbScan<'_> {
    fn frame_size(&self) -> (u32, u32) {
        (
            self.frame_width as u32,
            (self.frame.len() / (self.frame_width * 4)) as u32,
        )
    }

    fn template_size(&self) -> (u32, u32) {
        (self.template_width as u32, self.template_height as u32)
    }

    fn channel_scale(&self) -> f32 {
        765.0
    }

    fn template_sum(&self) -> u64 {
        self.template
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| u64::from(pixel[0]) + u64::from(pixel[1]) + u64::from(pixel[2]))
            .sum()
    }

    fn coarse_score(&self, origin_x: u32, origin_y: u32) -> f32 {
        RgbScan::coarse_score(self, origin_x, origin_y)
    }

    fn diff_at(&self, origin_x: u32, origin_y: u32, abort_at: u64) -> Option<u64> {
        RgbScan::diff_at(self, origin_x, origin_y, abort_at)
    }

    fn build_sum_table(&self, x0: u32, y0: u32, w: usize, h: usize) -> SumTable {
        SumTable::U64 {
            data: build_sat_rgb(self.frame, self.frame_width, x0, y0, w, h),
            stride: w + 1,
        }
    }
}

impl ScanOps for Scan<'_> {
    fn frame_size(&self) -> (u32, u32) {
        (
            self.frame_width as u32,
            (self.frame.len() / self.frame_width) as u32,
        )
    }

    fn template_size(&self) -> (u32, u32) {
        (self.template_width as u32, self.template_height as u32)
    }

    fn channel_scale(&self) -> f32 {
        255.0
    }

    fn template_sum(&self) -> u64 {
        self.template.iter().map(|&value| u64::from(value)).sum()
    }

    fn coarse_score(&self, origin_x: u32, origin_y: u32) -> f32 {
        Scan::coarse_score(self, origin_x, origin_y)
    }

    fn diff_at(&self, origin_x: u32, origin_y: u32, abort_at: u64) -> Option<u64> {
        Scan::diff_at(self, origin_x, origin_y, abort_at)
    }

    fn build_sum_table(&self, x0: u32, y0: u32, w: usize, h: usize) -> SumTable {
        SumTable::U32 {
            data: build_sat(self.frame, self.frame_width, x0, y0, w, h),
            stride: w + 1,
        }
    }
}

/// Shared read-only inputs for one banded scan, passed to every band.
struct FastScan<'a, S: ScanOps> {
    scan: &'a S,
    threshold: f32,
    max_diff: u64,
    pixel_count: u64,
    template_sum: u64,
    region: SearchRegion,
    max_x: u32,
    max_y: u32,
}

/// Processes pre-scan grid rows `[k0, k1)` plus their ±3 refinement
/// neighborhoods. Neighborhoods may overlap neighboring bands; duplicates
/// merge deterministically at the end.
fn scan_band<S: ScanOps>(fast: &FastScan<'_, S>, k0: u32, k1: u32) -> FastState {
    let scan = fast.scan;
    let (template_width, template_height) = scan.template_size();
    let grid_y0 = fast.region.y + k0 * PRESCAN_STRIDE;
    let grid_y1 = fast.region.y + (k1 - 1) * PRESCAN_STRIDE;
    // Frame rows the band's evaluated positions can touch: the refinement
    // neighborhoods reach 3 rows beyond the band's grid points.
    let pos_y0 = grid_y0
        .saturating_sub(PRESCAN_STRIDE - 1)
        .max(fast.region.y);
    let pos_y1 = (grid_y1 + PRESCAN_STRIDE - 1).min(fast.max_y);
    let sat = scan.build_sum_table(
        fast.region.x,
        pos_y0,
        fast.region.width as usize,
        (pos_y1 - pos_y0) as usize + template_height as usize,
    );
    // Exact lower bound: sum|a-b| >= |window_sum - template_sum|, so positions
    // past the budget can never qualify. Returns true when the position may
    // still be a match.
    let sum_bound_passes = |x: u32, y: u32| -> bool {
        sat.window_sum(x, y, template_width, template_height, fast.region.x, pos_y0)
            .abs_diff(fast.template_sum)
            <= fast.max_diff
    };

    let mut state = FastState::default();
    let mut grid_y = grid_y0;
    while grid_y <= grid_y1 {
        let mut grid_x = fast.region.x;
        while grid_x <= fast.max_x {
            if sum_bound_passes(grid_x, grid_y) {
                let coarse = scan.coarse_score(grid_x, grid_y);
                state.best_score = state.best_score.max(coarse);
                if coarse + 0.08 >= fast.threshold {
                    let from_x = grid_x.saturating_sub(PRESCAN_STRIDE - 1).max(fast.region.x);
                    let from_y = grid_y.saturating_sub(PRESCAN_STRIDE - 1).max(fast.region.y);
                    let to_x = (grid_x + PRESCAN_STRIDE - 1).min(fast.max_x);
                    let to_y = (grid_y + PRESCAN_STRIDE - 1).min(fast.max_y);
                    for y in from_y..=to_y {
                        for x in from_x..=to_x {
                            if sum_bound_passes(x, y) {
                                state.consider(
                                    scan,
                                    x,
                                    y,
                                    fast.threshold,
                                    fast.max_diff,
                                    fast.pixel_count,
                                );
                            }
                        }
                    }
                }
            }
            grid_x += PRESCAN_STRIDE;
        }
        grid_y += PRESCAN_STRIDE;
    }
    state
}

/// Builds a u32 summed-area table over the frame rectangle starting at
/// (x0, y0), sized w × h pixels, with a zero top row and left column. Entries
/// wrap on overflow; window sums read back with wrapping arithmetic stay
/// exact because any single template window fits in u32.
fn build_sat(frame: &[u8], frame_width: usize, x0: u32, y0: u32, w: usize, h: usize) -> Vec<u32> {
    let stride = w + 1;
    let mut sat = vec![0_u32; stride * (h + 1)];
    for j in 0..h {
        let frame_row = &frame[(y0 as usize + j) * frame_width + x0 as usize..][..w];
        let (above, current) = sat.split_at_mut((j + 1) * stride);
        let above_row = &above[j * stride..(j + 1) * stride];
        let current_row = &mut current[..stride];
        let mut row_sum = 0_u32;
        for (i, &pixel) in frame_row.iter().enumerate() {
            row_sum = row_sum.wrapping_add(u32::from(pixel));
            current_row[i + 1] = above_row[i + 1].wrapping_add(row_sum);
        }
    }
    sat
}

/// Builds a u64 summed-area table over per-pixel RGB channel sums (RGBA
/// input, alpha ignored).
fn build_sat_rgb(
    frame: &[u8],
    frame_width: usize,
    x0: u32,
    y0: u32,
    w: usize,
    h: usize,
) -> Vec<u64> {
    let stride = w + 1;
    let mut sat = vec![0_u64; stride * (h + 1)];
    for j in 0..h {
        let row_start = ((y0 as usize + j) * frame_width + x0 as usize) * 4;
        let frame_row = &frame[row_start..row_start + w * 4];
        let (above, current) = sat.split_at_mut((j + 1) * stride);
        let above_row = &above[j * stride..(j + 1) * stride];
        let current_row = &mut current[..stride];
        let mut row_sum = 0_u64;
        for i in 0..w {
            let base = i * 4;
            row_sum += u64::from(frame_row[base])
                + u64::from(frame_row[base + 1])
                + u64::from(frame_row[base + 2]);
            current_row[i + 1] = above_row[i + 1] + row_sum;
        }
    }
    sat
}

/// Band-local fast-scan result. `best` is ordered lexicographically by
/// (difference, y, x), so band results merge deterministically regardless of
/// evaluation order.
#[derive(Default)]
struct FastState {
    best: Option<(u64, u32, u32)>,
    best_score: f32,
}

impl FastState {
    /// Full-resolution evaluation of one candidate position: coarse rejection,
    /// then the exact difference when the position looks promising.
    fn consider<S: ScanOps>(
        &mut self,
        scan: &S,
        x: u32,
        y: u32,
        threshold: f32,
        max_diff: u64,
        pixel_count: u64,
    ) {
        let coarse = scan.coarse_score(x, y);
        self.best_score = self.best_score.max(coarse);
        if coarse + 0.06 < threshold {
            return;
        }
        // Ties on difference must run to completion so the (y, x) tie-break
        // can pick the winner; abort only when the position can no longer tie
        // or win.
        let abort_at = self
            .best
            .map(|(difference, _, _)| difference.saturating_add(1))
            .unwrap_or(u64::MAX)
            .min(max_diff.saturating_add(1));
        let Some(difference) = scan.diff_at(x, y, abort_at) else {
            return;
        };
        let score = 1.0 - difference as f32 / (pixel_count as f32 * scan.channel_scale());
        self.best_score = self.best_score.max(score);
        let candidate = (difference, y, x);
        if self.best.is_none_or(|current| candidate < current) {
            self.best = Some(candidate);
        }
    }

    fn report(
        self,
        template_width: u32,
        template_height: u32,
        pixel_count: u64,
        channel_scale: f32,
    ) -> MatchReport {
        MatchReport {
            matched: self.best.map(|(difference, y, x)| TemplateMatch {
                x,
                y,
                width: template_width,
                height: template_height,
                score: 1.0 - difference as f32 / (pixel_count as f32 * channel_scale),
            }),
            best_score: self.best_score,
        }
    }
}

#[cfg(test)]
mod tests {
    use image::{GrayImage, Luma, Rgba, RgbaImage};

    use super::*;

    fn synthetic_frame() -> (GrayImage, GrayImage) {
        let mut frame = GrayImage::from_pixel(12, 10, Luma([15]));
        let template = GrayImage::from_fn(3, 2, |x, y| Luma([80 + (x * 30 + y * 10) as u8]));
        for y in 0..template.height() {
            for x in 0..template.width() {
                frame.put_pixel(6 + x, 4 + y, *template.get_pixel(x, y));
            }
        }
        (frame, template)
    }

    #[test]
    fn finds_exact_template_and_center() {
        let (frame, template) = synthetic_frame();
        let found = find_template_report(
            &frame,
            &template,
            SearchRegion::full(&frame),
            0.99,
            MatchAlgorithm::Classic,
        )
        .matched
        .expect("template should match");

        assert_eq!((found.x, found.y), (6, 4));
        assert_eq!(found.center(), (7, 5));
        assert_eq!(found.score, 1.0);
    }

    #[test]
    fn respects_search_region() {
        let (frame, template) = synthetic_frame();
        let region = SearchRegion {
            x: 0,
            y: 0,
            width: 5,
            height: 5,
        };

        assert!(
            find_template_report(&frame, &template, region, 0.99, MatchAlgorithm::Classic)
                .matched
                .is_none()
        );
    }

    #[test]
    fn rejects_template_larger_than_region() {
        let frame = GrayImage::new(4, 4);
        let template = GrayImage::new(3, 3);
        let region = SearchRegion {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        };

        assert!(
            find_template_report(&frame, &template, region, 0.8, MatchAlgorithm::Classic)
                .matched
                .is_none()
        );
    }

    #[test]
    fn prefers_later_exact_match_over_earlier_noisy_match() {
        let mut frame = GrayImage::from_pixel(20, 10, Luma([15]));
        let template = GrayImage::from_fn(3, 2, |x, y| Luma([80 + (x * 30 + y * 10) as u8]));
        for y in 0..template.height() {
            for x in 0..template.width() {
                let noisy = template.get_pixel(x, y)[0].saturating_sub(30);
                frame.put_pixel(2 + x, 3 + y, Luma([noisy]));
                frame.put_pixel(10 + x, 5 + y, *template.get_pixel(x, y));
            }
        }

        let found = find_template_report(
            &frame,
            &template,
            SearchRegion::full(&frame),
            0.5,
            MatchAlgorithm::Classic,
        )
        .matched
        .expect("exact match should be selected");
        assert_eq!((found.x, found.y), (10, 5));
        assert_eq!(found.score, 1.0);
    }

    #[test]
    fn report_exposes_best_score_below_threshold() {
        let (mut frame, template) = synthetic_frame();
        // Degrade the only match so it lands below a strict threshold.
        frame.put_pixel(6, 4, Luma([15]));
        let report = find_template_report(
            &frame,
            &template,
            SearchRegion::full(&frame),
            0.99,
            MatchAlgorithm::Classic,
        );

        assert!(report.matched.is_none());
        assert!(
            report.best_score > 0.9,
            "best score was {}",
            report.best_score
        );
    }

    /// Deterministic xorshift64 PRNG for reproducible test images.
    struct XorShift(u64);

    impl XorShift {
        fn next(&mut self) -> u64 {
            let mut value = self.0;
            value ^= value << 13;
            value ^= value >> 7;
            value ^= value << 17;
            self.0 = value;
            value
        }

        fn next_u8(&mut self) -> u8 {
            (self.next() >> 32) as u8
        }
    }

    fn noise_image(rng: &mut XorShift, width: u32, height: u32) -> GrayImage {
        GrayImage::from_fn(width, height, |_, _| Luma([rng.next_u8()]))
    }

    /// Smooth random content (upscaled low-resolution noise), where matches
    /// stay findable a few pixels away from the pre-scan grid.
    fn smooth_image(rng: &mut XorShift, width: u32, height: u32) -> GrayImage {
        let coarse = noise_image(rng, width / 10, height / 10);
        image::imageops::resize(
            &coarse,
            width,
            height,
            image::imageops::FilterType::CatmullRom,
        )
    }

    fn embed(frame: &mut GrayImage, template: &GrayImage, at_x: u32, at_y: u32) {
        for y in 0..template.height() {
            for x in 0..template.width() {
                frame.put_pixel(at_x + x, at_y + y, *template.get_pixel(x, y));
            }
        }
    }

    /// The original exhaustive implementation, kept as the equivalence
    /// reference: every position gets the coarse pass and promising ones the
    /// exact difference.
    fn naive_find_template_report(
        frame: &GrayImage,
        template: &GrayImage,
        region: SearchRegion,
        threshold: f32,
    ) -> MatchReport {
        let threshold = threshold.clamp(0.0, 1.0);
        let pixel_count = template.width() as u64 * template.height() as u64;
        let max_diff = ((1.0 - threshold) * pixel_count as f32 * 255.0) as u64;
        let max_x = region.x + region.width - template.width();
        let max_y = region.y + region.height - template.height();
        let mut best: Option<TemplateMatch> = None;
        let mut best_diff = u64::MAX;
        let mut best_score = 0.0_f32;

        for y in region.y..=max_y {
            for x in region.x..=max_x {
                let coarse = naive_coarse_score(frame, template, x, y);
                best_score = best_score.max(coarse);
                if coarse + 0.06 < threshold {
                    continue;
                }
                let abort_at = max_diff.saturating_add(1).min(best_diff);
                let Some(difference) = naive_diff_at(frame, template, x, y, abort_at) else {
                    continue;
                };
                let score = 1.0 - difference as f32 / (pixel_count as f32 * 255.0);
                best_score = best_score.max(score);
                best_diff = difference;
                best = Some(TemplateMatch {
                    x,
                    y,
                    width: template.width(),
                    height: template.height(),
                    score,
                });
            }
        }

        MatchReport {
            matched: best,
            best_score,
        }
    }

    fn naive_coarse_score(
        frame: &GrayImage,
        template: &GrayImage,
        origin_x: u32,
        origin_y: u32,
    ) -> f32 {
        let pixel_count = template.width() as usize * template.height() as usize;
        let sample_step = ((pixel_count as f32 / 144.0).sqrt().ceil() as usize).max(2);
        let mut difference = 0_u64;
        let mut samples = 0_u64;

        for ty in (0..template.height()).step_by(sample_step) {
            for tx in (0..template.width()).step_by(sample_step) {
                let frame_value = frame.get_pixel(origin_x + tx, origin_y + ty)[0];
                let template_value = template.get_pixel(tx, ty)[0];
                difference += frame_value.abs_diff(template_value) as u64;
                samples += 1;
            }
        }

        1.0 - difference as f32 / (samples.max(1) as f32 * 255.0)
    }

    fn naive_diff_at(
        frame: &GrayImage,
        template: &GrayImage,
        origin_x: u32,
        origin_y: u32,
        abort_at: u64,
    ) -> Option<u64> {
        let mut difference = 0_u64;
        for ty in 0..template.height() {
            for tx in 0..template.width() {
                let frame_value = frame.get_pixel(origin_x + tx, origin_y + ty)[0];
                let template_value = template.get_pixel(tx, ty)[0];
                difference += frame_value.abs_diff(template_value) as u64;
            }
            if difference >= abort_at {
                return None;
            }
        }
        Some(difference)
    }

    fn assert_reports_equivalent(
        frame: &GrayImage,
        template: &GrayImage,
        threshold: f32,
        algorithm: MatchAlgorithm,
        context: &str,
    ) {
        let region = SearchRegion::full(frame);
        let optimized = find_template_report(frame, template, region, threshold, algorithm);
        let reference = naive_find_template_report(frame, template, region, threshold);
        if let Some(expected) = reference.matched {
            let found = optimized
                .matched
                .unwrap_or_else(|| panic!("{context}: reference matched but optimized did not"));
            assert!(
                (found.score - expected.score).abs() <= 0.02,
                "{context}: score {:.3} vs reference {:.3}",
                found.score,
                expected.score
            );
        }
        assert!(
            (optimized.best_score - reference.best_score).abs() <= 0.02,
            "{context}: best score {:.3} vs reference {:.3}",
            optimized.best_score,
            reference.best_score
        );
    }

    fn embedded_noise_case() -> (GrayImage, GrayImage) {
        let mut rng = XorShift(0x5771_A001);
        let mut frame = noise_image(&mut rng, 512, 384);
        let template = noise_image(&mut rng, 96, 72);
        // Stride-aligned position: the pre-scan grid sees the exact match.
        embed(&mut frame, &template, 128, 96);
        (frame, template)
    }

    fn unaligned_smooth_case() -> (GrayImage, GrayImage) {
        let mut rng = XorShift(0x5771_B002);
        let frame = smooth_image(&mut rng, 512, 384);
        // Template cut from an off-grid position of the same smooth frame.
        let template = image::imageops::crop_imm(&frame, 157, 83, 96, 72).to_image();
        (frame, template)
    }

    fn degraded_unaligned_case() -> (GrayImage, GrayImage) {
        let mut rng = XorShift(0x5771_C003);
        let frame = smooth_image(&mut rng, 512, 384);
        let mut template = image::imageops::crop_imm(&frame, 157, 83, 96, 72).to_image();
        // Degrade the template slightly, like a real capture with artifacts.
        for pixel in template.pixels_mut() {
            let delta = (rng.next() % 13) as i32 - 6;
            pixel[0] = pixel[0].saturating_add_signed(delta as i8);
        }
        (frame, template)
    }

    fn pure_noise_case() -> (GrayImage, GrayImage) {
        let mut rng = XorShift(0x5771_D004);
        let frame = noise_image(&mut rng, 512, 384);
        let template = noise_image(&mut rng, 96, 72);
        (frame, template)
    }

    fn noise_rgb_image(rng: &mut XorShift, width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |_, _| {
            Rgba([rng.next_u8(), rng.next_u8(), rng.next_u8(), 255])
        })
    }

    fn smooth_rgb_image(rng: &mut XorShift, width: u32, height: u32) -> RgbaImage {
        let coarse = noise_rgb_image(rng, width / 10, height / 10);
        image::imageops::resize(
            &coarse,
            width,
            height,
            image::imageops::FilterType::CatmullRom,
        )
    }

    fn embed_rgb(frame: &mut RgbaImage, template: &RgbaImage, at_x: u32, at_y: u32) {
        for y in 0..template.height() {
            for x in 0..template.width() {
                frame.put_pixel(at_x + x, at_y + y, *template.get_pixel(x, y));
            }
        }
    }

    /// The naive exhaustive RGB reference: every position gets the
    /// three-channel coarse pass and promising ones the exact difference.
    fn naive_find_template_report_rgb(
        frame: &RgbaImage,
        template: &RgbaImage,
        region: SearchRegion,
        threshold: f32,
    ) -> MatchReport {
        let threshold = threshold.clamp(0.0, 1.0);
        let pixel_count = u64::from(template.width()) * u64::from(template.height());
        let max_diff = ((1.0 - threshold) * pixel_count as f32 * 765.0) as u64;
        let max_x = region.x + region.width - template.width();
        let max_y = region.y + region.height - template.height();
        let mut best: Option<TemplateMatch> = None;
        let mut best_diff = u64::MAX;
        let mut best_score = 0.0_f32;

        for y in region.y..=max_y {
            for x in region.x..=max_x {
                let coarse = naive_coarse_score_rgb(frame, template, x, y);
                best_score = best_score.max(coarse);
                if coarse + 0.06 < threshold {
                    continue;
                }
                let abort_at = max_diff.saturating_add(1).min(best_diff);
                let Some(difference) = naive_diff_at_rgb(frame, template, x, y, abort_at) else {
                    continue;
                };
                let score = 1.0 - difference as f32 / (pixel_count as f32 * 765.0);
                best_score = best_score.max(score);
                best_diff = difference;
                best = Some(TemplateMatch {
                    x,
                    y,
                    width: template.width(),
                    height: template.height(),
                    score,
                });
            }
        }

        MatchReport {
            matched: best,
            best_score,
        }
    }

    fn naive_coarse_score_rgb(
        frame: &RgbaImage,
        template: &RgbaImage,
        origin_x: u32,
        origin_y: u32,
    ) -> f32 {
        let pixel_count = template.width() as usize * template.height() as usize;
        let sample_step = ((pixel_count as f32 / 144.0).sqrt().ceil() as usize).max(2);
        let mut difference = 0_u64;
        let mut samples = 0_u64;

        for ty in (0..template.height()).step_by(sample_step) {
            for tx in (0..template.width()).step_by(sample_step) {
                let frame_pixel = frame.get_pixel(origin_x + tx, origin_y + ty);
                let template_pixel = template.get_pixel(tx, ty);
                for channel in 0..3 {
                    difference += frame_pixel[channel].abs_diff(template_pixel[channel]) as u64;
                }
                samples += 1;
            }
        }

        1.0 - difference as f32 / (samples.max(1) as f32 * 765.0)
    }

    fn naive_diff_at_rgb(
        frame: &RgbaImage,
        template: &RgbaImage,
        origin_x: u32,
        origin_y: u32,
        abort_at: u64,
    ) -> Option<u64> {
        let mut difference = 0_u64;
        for ty in 0..template.height() {
            for tx in 0..template.width() {
                let frame_pixel = frame.get_pixel(origin_x + tx, origin_y + ty);
                let template_pixel = template.get_pixel(tx, ty);
                for channel in 0..3 {
                    difference += frame_pixel[channel].abs_diff(template_pixel[channel]) as u64;
                }
            }
            if difference >= abort_at {
                return None;
            }
        }
        Some(difference)
    }

    fn assert_rgb_equivalent(
        frame: &RgbaImage,
        template: &RgbaImage,
        threshold: f32,
        context: &str,
    ) {
        let region = SearchRegion::full(frame);
        let precise = find_template_report_rgb(frame, template, region, threshold);
        let reference = naive_find_template_report_rgb(frame, template, region, threshold);
        if let Some(expected) = reference.matched {
            let found = precise
                .matched
                .unwrap_or_else(|| panic!("{context}: reference matched but precise did not"));
            assert!(
                (found.score - expected.score).abs() <= 0.02,
                "{context}: score {:.3} vs reference {:.3}",
                found.score,
                expected.score
            );
        }
        assert!(
            (precise.best_score - reference.best_score).abs() <= 0.02,
            "{context}: best score {:.3} vs reference {:.3}",
            precise.best_score,
            reference.best_score
        );
    }

    fn embedded_noise_rgb_case() -> (RgbaImage, RgbaImage) {
        let mut rng = XorShift(0x5771_1A01);
        let mut frame = noise_rgb_image(&mut rng, 512, 384);
        let template = noise_rgb_image(&mut rng, 96, 72);
        // Stride-aligned position: the pre-scan grid sees the exact match.
        embed_rgb(&mut frame, &template, 128, 96);
        (frame, template)
    }

    fn unaligned_smooth_rgb_case() -> (RgbaImage, RgbaImage) {
        let mut rng = XorShift(0x5771_1B02);
        let frame = smooth_rgb_image(&mut rng, 512, 384);
        let template = image::imageops::crop_imm(&frame, 157, 83, 96, 72).to_image();
        (frame, template)
    }

    fn pure_noise_rgb_case() -> (RgbaImage, RgbaImage) {
        let mut rng = XorShift(0x5771_1D04);
        let frame = noise_rgb_image(&mut rng, 512, 384);
        let template = noise_rgb_image(&mut rng, 96, 72);
        (frame, template)
    }

    #[test]
    fn precise_matches_reference_on_embedded_noise_template() {
        let (frame, template) = embedded_noise_rgb_case();
        assert_rgb_equivalent(&frame, &template, 0.90, "precise embedded noise");
    }

    #[test]
    fn precise_matches_reference_on_unaligned_smooth_template() {
        let (frame, template) = unaligned_smooth_rgb_case();
        assert_rgb_equivalent(&frame, &template, 0.90, "precise unaligned smooth");
    }

    #[test]
    fn precise_matches_reference_on_pure_noise_without_match() {
        let (frame, template) = pure_noise_rgb_case();
        assert_rgb_equivalent(&frame, &template, 0.90, "precise pure noise");
    }

    #[test]
    fn precise_multithread_matches_single_thread() {
        for (name, (frame, template)) in [
            ("embedded noise", embedded_noise_rgb_case()),
            ("unaligned smooth", unaligned_smooth_rgb_case()),
            ("pure noise", pure_noise_rgb_case()),
        ] {
            let region = SearchRegion::full(&frame);
            let single = find_precise_impl(&frame, &template, region, 0.90, 1);
            let multi = find_precise_impl(&frame, &template, region, 0.90, MAX_FAST_BANDS);
            assert_eq!(
                single
                    .matched
                    .map(|found| (found.x, found.y, found.score.to_bits())),
                multi
                    .matched
                    .map(|found| (found.x, found.y, found.score.to_bits())),
                "{name}: multithreaded result differs from single-threaded"
            );
            assert_eq!(
                single.best_score.to_bits(),
                multi.best_score.to_bits(),
                "{name}: multithreaded best score differs"
            );
        }
    }

    #[test]
    fn rgb_sum_bound_never_rejects_qualifying_positions() {
        let mut rng = XorShift(0x5771_1F06);
        let frame = noise_rgb_image(&mut rng, 64, 48);
        let template = noise_rgb_image(&mut rng, 16, 12);
        let threshold = 0.9_f32;
        let pixel_count = u64::from(template.width()) * u64::from(template.height());
        let max_diff = ((1.0 - threshold) * pixel_count as f32 * 765.0) as u64;
        let max_x = frame.width() - template.width();
        let max_y = frame.height() - template.height();
        let sat = build_sat_rgb(
            frame.as_raw(),
            frame.width() as usize,
            0,
            0,
            frame.width() as usize,
            frame.height() as usize,
        );
        let stride = frame.width() as usize + 1;
        let template_sum: u64 = template
            .as_raw()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| u64::from(pixel[0]) + u64::from(pixel[1]) + u64::from(pixel[2]))
            .sum();
        for y in 0..=max_y {
            for x in 0..=max_x {
                let bottom = (y + template.height()) as usize;
                let right = (x + template.width()) as usize;
                let window_sum = sat[bottom * stride + right]
                    + sat[y as usize * stride + x as usize]
                    - sat[y as usize * stride + right]
                    - sat[bottom * stride + x as usize];
                if window_sum.abs_diff(template_sum) > max_diff {
                    let difference = naive_diff_at_rgb(&frame, &template, x, y, u64::MAX)
                        .expect("full diff should complete");
                    assert!(
                        difference > max_diff,
                        "sum bound rejected ({x}, {y}) but true diff {difference} <= {max_diff}"
                    );
                } else {
                    // Also verify the SAT window sum against a direct sum.
                    let mut direct = 0_u64;
                    for ty in 0..template.height() {
                        for tx in 0..template.width() {
                            let pixel = frame.get_pixel(x + tx, y + ty);
                            direct +=
                                u64::from(pixel[0]) + u64::from(pixel[1]) + u64::from(pixel[2]);
                        }
                    }
                    assert_eq!(window_sum, direct, "SAT window sum wrong at ({x}, {y})");
                }
            }
        }
    }

    /// The selling point of the precise engine: two regions with identical
    /// grayscale but different colors. Grayscale matching cannot tell them
    /// apart and clicks the first one; RGB matching rejects the impostor.
    #[test]
    fn precise_distinguishes_same_luma_colors() {
        let luma = |pixel: Rgba<u8>| {
            image::imageops::grayscale(&RgbaImage::from_pixel(1, 1, pixel)).get_pixel(0, 0)[0]
        };
        let color_a = Rgba([200, 40, 80, 255]);
        let target_luma = luma(color_a);
        let color_b = (0..=255)
            .map(|r| Rgba([r, 30, 220, 255]))
            .find(|&candidate| luma(candidate) == target_luma)
            .expect("a same-luma but different color should exist");
        assert_ne!(color_a, color_b);

        let mut frame = RgbaImage::from_pixel(64, 48, Rgba([30, 30, 30, 255]));
        let template = RgbaImage::from_pixel(16, 16, color_a);
        // Impostor at (8, 8) — scanned first, same luma; true target at (40, 24).
        for y in 0..16 {
            for x in 0..16 {
                frame.put_pixel(8 + x, 8 + y, color_b);
                frame.put_pixel(40 + x, 24 + y, color_a);
            }
        }

        let region = SearchRegion::full(&frame);
        let gray_frame = image::imageops::grayscale(&frame);
        let gray_template = image::imageops::grayscale(&template);
        let gray = find_template_report(
            &gray_frame,
            &gray_template,
            SearchRegion::full(&gray_frame),
            0.99,
            MatchAlgorithm::Fast,
        );
        let impostor = gray.matched.expect("grayscale should match the impostor");
        assert_eq!(
            (impostor.x, impostor.y),
            (8, 8),
            "grayscale cannot tell the two regions apart and clicks the impostor"
        );

        let precise = find_template_report_rgb(&frame, &template, region, 0.99);
        let found = precise.matched.expect("precise should find the true color");
        assert_eq!((found.x, found.y), (40, 24));
        assert!(found.score > 0.99);
    }

    #[test]
    fn matches_reference_on_embedded_noise_template() {
        let (frame, template) = embedded_noise_case();
        assert_reports_equivalent(
            &frame,
            &template,
            0.90,
            MatchAlgorithm::Classic,
            "embedded noise template",
        );
    }

    #[test]
    fn matches_reference_on_unaligned_smooth_template() {
        let (frame, template) = unaligned_smooth_case();
        assert_reports_equivalent(
            &frame,
            &template,
            0.90,
            MatchAlgorithm::Classic,
            "unaligned smooth template",
        );
    }

    #[test]
    fn matches_reference_on_degraded_unaligned_template() {
        let (frame, template) = degraded_unaligned_case();
        assert_reports_equivalent(
            &frame,
            &template,
            0.90,
            MatchAlgorithm::Classic,
            "degraded template",
        );
    }

    #[test]
    fn matches_reference_on_pure_noise_without_match() {
        let (frame, template) = pure_noise_case();
        assert_reports_equivalent(
            &frame,
            &template,
            0.90,
            MatchAlgorithm::Classic,
            "pure noise",
        );
    }

    #[test]
    fn fast_matches_reference_on_embedded_noise_template() {
        let (frame, template) = embedded_noise_case();
        assert_reports_equivalent(
            &frame,
            &template,
            0.90,
            MatchAlgorithm::Fast,
            "fast embedded noise template",
        );
    }

    #[test]
    fn fast_matches_reference_on_unaligned_smooth_template() {
        let (frame, template) = unaligned_smooth_case();
        assert_reports_equivalent(
            &frame,
            &template,
            0.90,
            MatchAlgorithm::Fast,
            "fast unaligned smooth template",
        );
    }

    #[test]
    fn fast_matches_reference_on_degraded_unaligned_template() {
        let (frame, template) = degraded_unaligned_case();
        assert_reports_equivalent(
            &frame,
            &template,
            0.90,
            MatchAlgorithm::Fast,
            "fast degraded template",
        );
    }

    #[test]
    fn fast_matches_reference_on_pure_noise_without_match() {
        let (frame, template) = pure_noise_case();
        assert_reports_equivalent(
            &frame,
            &template,
            0.90,
            MatchAlgorithm::Fast,
            "fast pure noise",
        );
    }

    #[test]
    fn sum_bound_never_rejects_qualifying_positions() {
        let mut rng = XorShift(0x5771_F006);
        let frame = noise_image(&mut rng, 64, 48);
        let template = noise_image(&mut rng, 16, 12);
        let threshold = 0.9_f32;
        let pixel_count = u64::from(template.width()) * u64::from(template.height());
        let max_diff = ((1.0 - threshold) * pixel_count as f32 * 255.0) as u64;
        let max_x = frame.width() - template.width();
        let max_y = frame.height() - template.height();
        let sat = build_sat(
            frame.as_raw(),
            frame.width() as usize,
            0,
            0,
            frame.width() as usize,
            frame.height() as usize,
        );
        let stride = frame.width() as usize + 1;
        let template_sum: u64 = template
            .as_raw()
            .iter()
            .map(|&value| u64::from(value))
            .sum();
        for y in 0..=max_y {
            for x in 0..=max_x {
                let bottom = (y + template.height()) as usize;
                let right = (x + template.width()) as usize;
                let row = y as usize;
                let col = x as usize;
                let window_sum = u64::from(
                    sat[bottom * stride + right]
                        .wrapping_add(sat[row * stride + col])
                        .wrapping_sub(sat[row * stride + right])
                        .wrapping_sub(sat[bottom * stride + col]),
                );
                if window_sum.abs_diff(template_sum) > max_diff {
                    let difference = naive_diff_at(&frame, &template, x, y, u64::MAX)
                        .expect("full diff should complete");
                    assert!(
                        difference > max_diff,
                        "sum bound rejected ({x}, {y}) but true diff {difference} <= {max_diff}"
                    );
                } else {
                    // Also verify the SAT window sum against a direct sum.
                    let mut direct = 0_u64;
                    for ty in 0..template.height() {
                        for tx in 0..template.width() {
                            direct += u64::from(frame.get_pixel(x + tx, y + ty)[0]);
                        }
                    }
                    assert_eq!(window_sum, direct, "SAT window sum wrong at ({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn fast_multithread_matches_single_thread() {
        for (name, (frame, template)) in [
            ("embedded noise", embedded_noise_case()),
            ("unaligned smooth", unaligned_smooth_case()),
            ("pure noise", pure_noise_case()),
        ] {
            let region = SearchRegion::full(&frame);
            let single = find_fast_impl(&frame, &template, region, 0.90, 1);
            let multi = find_fast_impl(&frame, &template, region, 0.90, MAX_FAST_BANDS);
            assert_eq!(
                single
                    .matched
                    .map(|found| (found.x, found.y, found.score.to_bits())),
                multi
                    .matched
                    .map(|found| (found.x, found.y, found.score.to_bits())),
                "{name}: multithreaded result differs from single-threaded"
            );
            assert_eq!(
                single.best_score.to_bits(),
                multi.best_score.to_bits(),
                "{name}: multithreaded best score differs"
            );
        }
    }

    #[test]
    #[ignore = "performance comparison, run with cargo test --release -- --ignored"]
    fn perf_full_hd_scan() {
        for (width, height) in [(1920, 1080), (2560, 1440)] {
            let mut rng = XorShift(0x5771_E005);
            let frame = noise_image(&mut rng, width, height);
            let template = noise_image(&mut rng, 120, 80);
            let region = SearchRegion::full(&frame);

            // Warm up allocations/page faults outside the timed sections.
            let _ = find_fast_impl(&frame, &template, region, 0.90, MAX_FAST_BANDS);

            let started = std::time::Instant::now();
            let classic =
                find_template_report(&frame, &template, region, 0.90, MatchAlgorithm::Classic);
            let classic_elapsed = started.elapsed();

            let started = std::time::Instant::now();
            let fast_single = find_fast_impl(&frame, &template, region, 0.90, 1);
            let fast_single_elapsed = started.elapsed();

            let started = std::time::Instant::now();
            let fast_multi = find_fast_impl(&frame, &template, region, 0.90, MAX_FAST_BANDS);
            let fast_multi_elapsed = started.elapsed();

            eprintln!(
                "{width}×{height} full scan: classic {classic_elapsed:?}, \
                 fast 1-thread {fast_single_elapsed:?}, fast {MAX_FAST_BANDS}-thread {fast_multi_elapsed:?}"
            );
            assert_eq!(classic.matched.is_some(), fast_single.matched.is_some());
            assert_eq!(fast_single.matched, fast_multi.matched);
        }

        // Realistic scenario: a bright UI template on a darker game frame,
        // where the sum-bound prefilter rejects almost everything.
        for (width, height) in [(1920, 1080), (2560, 1440)] {
            let mut frame_rng = XorShift(0x5771_E006);
            let mut frame = smooth_image(&mut frame_rng, width, height);
            for pixel in frame.pixels_mut() {
                pixel[0] = ((u16::from(pixel[0]) * 2) / 5) as u8;
            }
            let mut template_rng = XorShift(0x5771_E007);
            let template_source = smooth_image(&mut template_rng, 240, 160);
            let template = image::imageops::crop_imm(&template_source, 60, 40, 120, 80).to_image();
            let region = SearchRegion::full(&frame);

            let _ = find_fast_impl(&frame, &template, region, 0.90, MAX_FAST_BANDS);

            let started = std::time::Instant::now();
            let classic =
                find_template_report(&frame, &template, region, 0.90, MatchAlgorithm::Classic);
            let classic_elapsed = started.elapsed();

            let started = std::time::Instant::now();
            let fast_single = find_fast_impl(&frame, &template, region, 0.90, 1);
            let fast_single_elapsed = started.elapsed();

            let started = std::time::Instant::now();
            let fast_multi = find_fast_impl(&frame, &template, region, 0.90, MAX_FAST_BANDS);
            let fast_multi_elapsed = started.elapsed();

            eprintln!(
                "{width}×{height} realistic scan: classic {classic_elapsed:?}, \
                 fast 1-thread {fast_single_elapsed:?}, fast {MAX_FAST_BANDS}-thread {fast_multi_elapsed:?}"
            );
            assert_eq!(classic.matched.is_some(), fast_single.matched.is_some());
            assert_eq!(fast_single.matched, fast_multi.matched);
        }

        // Precise (RGB) on noise and realistic content, single vs multi thread.
        for (width, height) in [(1920, 1080), (2560, 1440)] {
            let mut rng = XorShift(0x5771_E008);
            let noise_frame = noise_rgb_image(&mut rng, width, height);
            let noise_template = noise_rgb_image(&mut rng, 120, 80);
            let region = SearchRegion::full(&noise_frame);

            let _ = find_precise_impl(&noise_frame, &noise_template, region, 0.90, MAX_FAST_BANDS);

            let started = std::time::Instant::now();
            let precise_single = find_precise_impl(&noise_frame, &noise_template, region, 0.90, 1);
            let precise_single_elapsed = started.elapsed();

            let started = std::time::Instant::now();
            let precise_multi =
                find_precise_impl(&noise_frame, &noise_template, region, 0.90, MAX_FAST_BANDS);
            let precise_multi_elapsed = started.elapsed();

            eprintln!(
                "{width}×{height} noise scan: precise 1-thread {precise_single_elapsed:?}, \
                 precise {MAX_FAST_BANDS}-thread {precise_multi_elapsed:?}"
            );
            assert_eq!(precise_single.matched, precise_multi.matched);

            // Realistic: bright RGB template on a darker smooth frame.
            let mut frame_rng = XorShift(0x5771_E009);
            let mut real_frame = smooth_rgb_image(&mut frame_rng, width, height);
            for pixel in real_frame.pixels_mut() {
                for channel in 0..3 {
                    pixel[channel] = ((u16::from(pixel[channel]) * 2) / 5) as u8;
                }
            }
            let mut template_rng = XorShift(0x5771_E00A);
            let template_source = smooth_rgb_image(&mut template_rng, 240, 160);
            let real_template =
                image::imageops::crop_imm(&template_source, 60, 40, 120, 80).to_image();
            let region = SearchRegion::full(&real_frame);

            let started = std::time::Instant::now();
            let precise_single = find_precise_impl(&real_frame, &real_template, region, 0.90, 1);
            let precise_single_elapsed = started.elapsed();

            let started = std::time::Instant::now();
            let precise_multi =
                find_precise_impl(&real_frame, &real_template, region, 0.90, MAX_FAST_BANDS);
            let precise_multi_elapsed = started.elapsed();

            eprintln!(
                "{width}×{height} realistic scan: precise 1-thread {precise_single_elapsed:?}, \
                 precise {MAX_FAST_BANDS}-thread {precise_multi_elapsed:?}"
            );
            assert_eq!(precise_single.matched, precise_multi.matched);
        }
    }
}
