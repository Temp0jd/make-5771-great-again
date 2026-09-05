use image::{GrayImage, RgbaImage};
use serde::{Deserialize, Serialize};

/// Which template-matching engine to use. `Precise` remains the deserialization
/// default so existing profiles keep their calibrated score semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MatchAlgorithm {
    /// Weighted RGB candidate matching plus ZNCC structural verification.
    /// New profiles use this mode; old profiles remain on `Precise` until the
    /// user explicitly switches and recalibrates their thresholds.
    Hybrid,
    /// RGB three-channel matching: integral-image lower-bound prefilter plus
    /// banded multithreading.
    #[default]
    Precise,
    /// Grayscale: integral-image lower-bound prefilter plus banded
    /// multithreading.
    Fast,
    /// The original grayscale strided pre-scan, single-threaded.
    Classic,
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

/// Significant-pixel analysis of a template (OpenCV mask idea).
///
/// A PNG containing transparent pixels is treated as an explicit mask:
/// alpha-zero pixels are ignored and every visible pixel participates. For
/// ordinary opaque screenshots, pixels close to the template's average color
/// are treated as background so large flat areas cannot earn similarity on
/// their own.
#[derive(Debug, Clone)]
pub struct TemplateWeights {
    /// Row-major pixel indices that count in weighted scoring and structural
    /// verification.
    pub significant_indices: Vec<u32>,
    /// Significant pixels lying on the coarse sample grid (falls back to all
    /// significant pixels when the grid misses them).
    pub significant_samples: Vec<u32>,
    pub significant_count: u64,
    /// Channel sum over significant pixels only.
    #[allow(dead_code, reason = "precomputed for diagnostics; asserted in tests")]
    pub significant_sum: u64,
    /// Channel sum over every pixel (drives the window-sum lower bound).
    pub total_sum: u64,
    pub pixel_count: u64,
    /// True when transparency supplied the mask rather than the automatic
    /// average-color heuristic.
    pub explicit_alpha_mask: bool,
}

impl TemplateWeights {
    /// Max per-channel deviation from the average color below which a pixel
    /// counts as background.
    pub const BACKGROUND_EPSILON: u32 = 12;
    const ALPHA_VISIBLE_THRESHOLD: u8 = 16;

    /// Weighted matching is pointless below this significant-pixel ratio;
    /// callers warn and suggest a tighter crop.
    pub const MIN_SIGNIFICANT_RATIO: f32 = 0.15;

    pub fn analyze(template: &RgbaImage) -> Self {
        let raw = template.as_raw();
        let pixel_count = (template.width() as usize * template.height() as usize).max(1);
        let mut channel_sums = [0_u64; 3];
        for pixel in raw.as_chunks::<4>().0 {
            for (channel, sum) in channel_sums.iter_mut().enumerate() {
                *sum += u64::from(pixel[channel]);
            }
        }
        let mean = channel_sums.map(|sum| sum / pixel_count as u64);
        // Require genuinely transparent pixels before interpreting alpha as a
        // mask; incidental 254-valued metadata must not disable auto masking.
        let explicit_alpha_mask = raw.as_chunks::<4>().0.iter().any(|pixel| pixel[3] == 0);

        let mut significant_indices = Vec::new();
        let mut significant_sum = 0_u64;
        let mut total_sum = 0_u64;
        for (index, pixel) in raw.as_chunks::<4>().0.iter().enumerate() {
            let channel_sum = u64::from(pixel[0]) + u64::from(pixel[1]) + u64::from(pixel[2]);
            total_sum += channel_sum;
            let max_deviation = (0..3)
                .map(|channel| u64::from(pixel[channel]).abs_diff(mean[channel]))
                .max()
                .unwrap_or(0);
            let included = if explicit_alpha_mask {
                pixel[3] >= Self::ALPHA_VISIBLE_THRESHOLD
            } else {
                max_deviation >= u64::from(Self::BACKGROUND_EPSILON)
            };
            if included {
                significant_indices.push(index as u32);
                significant_sum += channel_sum;
            }
        }

        let sample_step = ((pixel_count as f32 / 144.0).sqrt().ceil() as usize).max(2);
        let template_width = template.width() as usize;
        let mut significant_samples: Vec<u32> = significant_indices
            .iter()
            .copied()
            .filter(|index| {
                let x = *index as usize % template_width;
                let y = *index as usize / template_width;
                x.is_multiple_of(sample_step) && y.is_multiple_of(sample_step)
            })
            .collect();
        if significant_samples.is_empty() && !significant_indices.is_empty() {
            // The sample grid misses every significant pixel (tiny detail);
            // score over the significant pixels directly.
            significant_samples = significant_indices.clone();
        }

        let significant_count = significant_indices.len() as u64;
        Self {
            significant_indices,
            significant_samples,
            significant_count,
            significant_sum,
            total_sum,
            pixel_count: pixel_count as u64,
            explicit_alpha_mask,
        }
    }

    pub fn significant_ratio(&self) -> f32 {
        if self.pixel_count == 0 {
            return 0.0;
        }
        self.significant_count as f32 / self.pixel_count as f32
    }

    /// True when so little of the template participates in scoring that the
    /// match is likely unstable; the UI suggests a tighter crop.
    pub fn is_mostly_background(&self) -> bool {
        self.significant_ratio() < Self::MIN_SIGNIFICANT_RATIO
    }
}

/// Zero-mean normalized cross-correlation at one candidate position.
///
/// The SAD matcher remains the cheap candidate generator; this allocation-free
/// structural check rejects look-alike controls that share colors and chrome
/// but have different text or icon shapes. Brightness and contrast shifts are
/// normalized away. `None` means either image geometry is invalid or one side
/// has too little variance for correlation to be meaningful.
pub fn structural_similarity_at(
    frame: &RgbaImage,
    template: &RgbaImage,
    weights: &TemplateWeights,
    x: u32,
    y: u32,
) -> Option<f32> {
    if template.width() == 0
        || template.height() == 0
        || x.saturating_add(template.width()) > frame.width()
        || y.saturating_add(template.height()) > frame.height()
    {
        return None;
    }

    let indices = &weights.significant_indices;
    if indices.len() < 8 {
        return None;
    }

    let frame_raw = frame.as_raw();
    let template_raw = template.as_raw();
    let frame_width = frame.width() as usize;
    let template_width = template.width() as usize;
    let mut sum_frame = 0.0_f64;
    let mut sum_template = 0.0_f64;
    let mut sum_frame_sq = 0.0_f64;
    let mut sum_template_sq = 0.0_f64;
    let mut sum_cross = 0.0_f64;

    for &index in indices {
        let index = index as usize;
        let tx = index % template_width;
        let ty = index / template_width;
        let frame_base = ((y as usize + ty) * frame_width + x as usize + tx) * 4;
        let template_base = index * 4;
        let frame_luma = rgb_luma(&frame_raw[frame_base..frame_base + 3]);
        let template_luma = rgb_luma(&template_raw[template_base..template_base + 3]);
        sum_frame += frame_luma;
        sum_template += template_luma;
        sum_frame_sq += frame_luma * frame_luma;
        sum_template_sq += template_luma * template_luma;
        sum_cross += frame_luma * template_luma;
    }

    let count = indices.len() as f64;
    let covariance = count * sum_cross - sum_frame * sum_template;
    let frame_variance = count * sum_frame_sq - sum_frame * sum_frame;
    let template_variance = count * sum_template_sq - sum_template * sum_template;
    let denominator = (frame_variance * template_variance).sqrt();
    if denominator <= f64::EPSILON {
        return None;
    }
    Some((covariance / denominator).clamp(-1.0, 1.0) as f32)
}

#[inline]
fn rgb_luma(rgb: &[u8]) -> f64 {
    // ITU-R BT.601 integer weights; conversion precision is ample for ZNCC.
    f64::from(77_u32 * u32::from(rgb[0]) + 150_u32 * u32::from(rgb[1]) + 29_u32 * u32::from(rgb[2]))
        / 256.0
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
        MatchAlgorithm::Fast | MatchAlgorithm::Precise | MatchAlgorithm::Hybrid => {
            find_fast(frame, template, region, threshold)
        }
    }
}

/// Finds the closest RGB template using three-channel mean absolute
/// similarity (unweighted `Precise` engine: integral-image lower-bound
/// prefilter plus banded multithreading, deterministic merge). Production
/// code uses the weighted variant; this entry is kept for benchmarks and
/// tests.
#[allow(dead_code, reason = "kept as the unweighted baseline for tests")]
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
    /// Pixels participating in the final score: the full template for
    /// unweighted scans, only significant pixels for weighted ones.
    fn scoring_pixels(&self) -> u64;
    /// Sum of the matched channels over the whole template.
    fn template_sum(&self) -> u64;
    /// Difference budget for the window-sum lower bound. Weighted scans widen
    /// it: their accepted positions can carry arbitrarily large *background*
    /// difference, so the unweighted window bound must look further ahead
    /// (see RgbWeightedScan).
    fn bound_max_diff(&self, threshold: f32) -> u64;
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
    let (candidates, best_score) = find_banded_candidates(scan, region, threshold, max_bands, 1);
    MatchReport {
        matched: candidates.into_iter().next(),
        best_score,
    }
}

/// Returns up to `candidate_limit` lowest-difference positions. Keeping a
/// small bounded list lets structural verification recover when the best SAD
/// look-alike is not the best hybrid match.
fn find_banded_candidates<S: ScanOps>(
    scan: &S,
    region: SearchRegion,
    threshold: f32,
    max_bands: usize,
    candidate_limit: usize,
) -> (Vec<TemplateMatch>, f32) {
    let (template_width, template_height) = scan.template_size();
    let (frame_width, frame_height) = scan.frame_size();
    if template_width == 0
        || template_height == 0
        || region.width < template_width
        || region.height < template_height
        || region.x.saturating_add(region.width) > frame_width
        || region.y.saturating_add(region.height) > frame_height
    {
        return (Vec::new(), 0.0);
    }

    let candidate_limit = candidate_limit.max(1);
    let threshold = threshold.clamp(0.0, 1.0);
    let pixel_count = scan.scoring_pixels();
    let channel_scale = scan.channel_scale();
    let max_diff = ((1.0 - threshold) * pixel_count as f32 * channel_scale) as u64;
    let bound_max_diff = scan.bound_max_diff(threshold);
    let max_x = region.x + region.width - template_width;
    let max_y = region.y + region.height - template_height;

    let position_count = u64::from(max_x - region.x + 1) * u64::from(max_y - region.y + 1);
    let states = if position_count <= DENSE_SCAN_LIMIT {
        // Small areas: dense single-threaded scan, no summed-area table.
        let mut state = FastState::new(candidate_limit);
        for y in region.y..=max_y {
            for x in region.x..=max_x {
                state.consider(scan, x, y, threshold, max_diff, pixel_count);
            }
        }
        vec![state]
    } else {
        let grid_rows = (max_y - region.y) / PRESCAN_STRIDE + 1;
        let bands = max_bands.clamp(1, (grid_rows as usize).min(MAX_FAST_BANDS));
        let fast = FastScan {
            scan,
            threshold,
            max_diff,
            bound_max_diff,
            pixel_count,
            template_sum: scan.template_sum(),
            region,
            max_x,
            max_y,
            candidate_limit,
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
        states
    };

    let mut merged = FastState::new(candidate_limit);
    for state in states {
        merged.best_score = merged.best_score.max(state.best_score);
        for candidate in state.best {
            merged.insert(candidate);
        }
    }
    let matches = merged
        .best
        .into_iter()
        .map(|(difference, y, x)| TemplateMatch {
            x,
            y,
            width: template_width,
            height: template_height,
            score: 1.0 - difference as f32 / (pixel_count as f32 * channel_scale),
        })
        .collect();
    (matches, merged.best_score)
}

/// RGB scan with significant-pixel weighting: only the template's
/// significant pixels (see `TemplateWeights`) participate in coarse and exact
/// scoring, so flat template backgrounds cannot earn similarity.
///
/// Candidate generation keeps the *unweighted* window-sum lower bound, but
/// with the threshold widened by `WEIGHTED_BOUND_MARGIN` (floored at 0.5):
/// an accepted weighted match may differ arbitrarily in background pixels, so
/// its unweighted window difference can exceed the plain budget. Widening the
/// bound recovers those candidates; the margin only affects recall, never
/// acceptance — acceptance is decided by the exact weighted difference. The
/// documented trade-off: a position whose significant pixels match perfectly
/// while its background differs hugely (beyond the widened budget) can still
/// be missed.
struct RgbWeightedScan<'a> {
    scan: RgbScan<'a>,
    weights: &'a TemplateWeights,
}

/// How far the window-sum bound's threshold is lowered for weighted scans.
const WEIGHTED_BOUND_MARGIN: f32 = 0.15;

impl RgbWeightedScan<'_> {
    fn coarse_score(&self, origin_x: u32, origin_y: u32) -> f32 {
        let scan = &self.scan;
        let mut difference = 0_u64;
        let mut samples = 0_u64;
        for &index in &self.weights.significant_samples {
            let tx = index as usize % scan.template_width;
            let ty = index as usize / scan.template_width;
            let frame_base =
                ((origin_y as usize + ty) * scan.frame_width + origin_x as usize + tx) * 4;
            let template_base = index as usize * 4;
            for channel in 0..3 {
                difference += scan.frame[frame_base + channel]
                    .abs_diff(scan.template[template_base + channel])
                    as u64;
            }
            samples += 1;
        }
        1.0 - difference as f32 / (samples.max(1) as f32 * 765.0)
    }

    /// Weighted three-channel difference over significant pixels only,
    /// bailing out as soon as `abort_at` is reached.
    fn diff_at(&self, origin_x: u32, origin_y: u32, abort_at: u64) -> Option<u64> {
        let scan = &self.scan;
        let mut difference = 0_u64;
        for &index in &self.weights.significant_indices {
            let tx = index as usize % scan.template_width;
            let ty = index as usize / scan.template_width;
            let frame_base =
                ((origin_y as usize + ty) * scan.frame_width + origin_x as usize + tx) * 4;
            let template_base = index as usize * 4;
            for channel in 0..3 {
                difference += u64::from(
                    scan.frame[frame_base + channel]
                        .abs_diff(scan.template[template_base + channel]),
                );
            }
            if difference >= abort_at {
                return None;
            }
        }
        Some(difference)
    }
}

impl ScanOps for RgbWeightedScan<'_> {
    fn frame_size(&self) -> (u32, u32) {
        self.scan.frame_size()
    }

    fn template_size(&self) -> (u32, u32) {
        self.scan.template_size()
    }

    fn channel_scale(&self) -> f32 {
        765.0
    }

    fn scoring_pixels(&self) -> u64 {
        self.weights.significant_count
    }

    fn template_sum(&self) -> u64 {
        self.weights.total_sum
    }

    fn bound_max_diff(&self, threshold: f32) -> u64 {
        if self.weights.explicit_alpha_mask {
            // Transparent pixels may contain arbitrary RGB values and are not
            // scored, so an unmasked whole-window sum cannot safely reject a
            // candidate selected by an explicit alpha mask.
            return self.weights.pixel_count * self.channel_scale() as u64;
        }
        let widened = (threshold - WEIGHTED_BOUND_MARGIN).max(0.5);
        ((1.0 - widened) * self.weights.pixel_count as f32 * self.channel_scale()) as u64
    }

    fn coarse_score(&self, origin_x: u32, origin_y: u32) -> f32 {
        RgbWeightedScan::coarse_score(self, origin_x, origin_y)
    }

    fn diff_at(&self, origin_x: u32, origin_y: u32, abort_at: u64) -> Option<u64> {
        RgbWeightedScan::diff_at(self, origin_x, origin_y, abort_at)
    }

    fn build_sum_table(&self, x0: u32, y0: u32, w: usize, h: usize) -> SumTable {
        self.scan.build_sum_table(x0, y0, w, h)
    }
}

/// Minimum accepted score for the hybrid matcher. Very low user thresholds
/// are unsafe with normalized pixel differences because unrelated color data
/// has a surprisingly high baseline.
pub const MIN_HYBRID_THRESHOLD: f32 = 0.78;

pub fn effective_threshold(algorithm: MatchAlgorithm, threshold: f32) -> f32 {
    if algorithm == MatchAlgorithm::Hybrid {
        threshold.max(MIN_HYBRID_THRESHOLD)
    } else {
        threshold
    }
}

const HYBRID_CANDIDATE_MARGIN: f32 = 0.04;
const HYBRID_STRUCTURE_WEIGHT: f32 = 0.65;
const HYBRID_CANDIDATE_LIMIT: usize = 4;
const HYBRID_FAST_PATH_STRUCTURE: f32 = 0.985;

#[cfg(test)]
fn available_scan_threads() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .min(MAX_FAST_BANDS)
}

fn score_hybrid_candidate(
    frame: &RgbaImage,
    template: &RgbaImage,
    weights: &TemplateWeights,
    mut found: TemplateMatch,
) -> (TemplateMatch, Option<f32>) {
    let color_score = found.score;
    let structural_score = structural_similarity_at(frame, template, weights, found.x, found.y);
    found.score = structural_score.map_or(color_score, |score| {
        color_score * (1.0 - HYBRID_STRUCTURE_WEIGHT) + score.max(0.0) * HYBRID_STRUCTURE_WEIGHT
    });
    (found, structural_score)
}

fn best_hybrid_candidate(
    frame: &RgbaImage,
    template: &RgbaImage,
    weights: &TemplateWeights,
    candidates: Vec<TemplateMatch>,
) -> Option<TemplateMatch> {
    candidates
        .into_iter()
        .map(|found| score_hybrid_candidate(frame, template, weights, found).0)
        .max_by(|left, right| {
            left.score.total_cmp(&right.score).then_with(|| {
                // Reverse coordinates so max_by keeps the top-left candidate
                // when scores tie.
                (right.y, right.x).cmp(&(left.y, left.x))
            })
        })
}

/// Thorough lightweight matcher used for verification and template testing.
/// It ranks up to four RGB SAD candidates and verifies their structure without
/// FFT, model runtimes, native libraries or per-candidate allocations.
#[cfg(test)]
pub fn find_template_report_rgb_hybrid(
    frame: &RgbaImage,
    template: &RgbaImage,
    weights: &TemplateWeights,
    region: SearchRegion,
    threshold: f32,
) -> MatchReport {
    find_template_report_rgb_hybrid_with_bands(
        frame,
        template,
        weights,
        region,
        threshold,
        available_scan_threads(),
    )
}

pub fn find_template_report_rgb_hybrid_with_bands(
    frame: &RgbaImage,
    template: &RgbaImage,
    weights: &TemplateWeights,
    region: SearchRegion,
    threshold: f32,
    max_bands: usize,
) -> MatchReport {
    let effective_threshold = effective_threshold(MatchAlgorithm::Hybrid, threshold);
    let candidate_threshold = (effective_threshold - HYBRID_CANDIDATE_MARGIN).max(0.65);
    let max_bands = max_bands.clamp(1, MAX_FAST_BANDS);

    // The thorough path performs one Top-4 scan. The runner invokes it only
    // for small tracked/ROI regions, periodic recovery, visual conditions or
    // exhaustive branch confirmation, avoiding a duplicate full-screen pass.
    let (candidates, scan_best) = find_precise_weighted_candidates_impl(
        frame,
        template,
        weights,
        region,
        candidate_threshold,
        max_bands,
        HYBRID_CANDIDATE_LIMIT,
    );
    let best = best_hybrid_candidate(frame, template, weights, candidates);
    MatchReport {
        matched: best.filter(|found| found.score >= effective_threshold),
        best_score: best.map_or(scan_best, |found| found.score),
    }
}

/// Cheap discovery pass for large regions. It only returns a match when the
/// best RGB candidate also has near-exact structure; uncertain screens are
/// deliberately deferred to the periodic thorough pass.
pub fn find_template_report_rgb_hybrid_fast_with_bands(
    frame: &RgbaImage,
    template: &RgbaImage,
    weights: &TemplateWeights,
    region: SearchRegion,
    threshold: f32,
    max_bands: usize,
) -> MatchReport {
    hybrid_fast_pass(
        frame,
        template,
        weights,
        region,
        effective_threshold(MatchAlgorithm::Hybrid, threshold),
        max_bands.clamp(1, MAX_FAST_BANDS),
    )
}

fn hybrid_fast_pass(
    frame: &RgbaImage,
    template: &RgbaImage,
    weights: &TemplateWeights,
    region: SearchRegion,
    effective_threshold: f32,
    max_bands: usize,
) -> MatchReport {
    let (first_candidates, scan_best) = find_precise_weighted_candidates_impl(
        frame,
        template,
        weights,
        region,
        effective_threshold,
        max_bands,
        1,
    );
    let Some(first) = first_candidates.into_iter().next() else {
        return MatchReport {
            matched: None,
            best_score: scan_best,
        };
    };
    let (first, structure) = score_hybrid_candidate(frame, template, weights, first);
    let trusted = first.score >= effective_threshold
        && structure.is_some_and(|score| score >= HYBRID_FAST_PATH_STRUCTURE);
    MatchReport {
        matched: trusted.then_some(first),
        best_score: first.score,
    }
}

/// Weighted RGB candidate matching: like `find_template_report_rgb`, but
/// template background pixels carry no weight. A fully flat template (no
/// significant pixels) degrades to the unweighted engine.
#[cfg(test)]
pub fn find_template_report_rgb_weighted(
    frame: &RgbaImage,
    template: &RgbaImage,
    weights: &TemplateWeights,
    region: SearchRegion,
    threshold: f32,
) -> MatchReport {
    find_template_report_rgb_weighted_with_bands(
        frame,
        template,
        weights,
        region,
        threshold,
        available_scan_threads(),
    )
}

pub fn find_template_report_rgb_weighted_with_bands(
    frame: &RgbaImage,
    template: &RgbaImage,
    weights: &TemplateWeights,
    region: SearchRegion,
    threshold: f32,
    max_bands: usize,
) -> MatchReport {
    find_precise_weighted_impl(
        frame,
        template,
        weights,
        region,
        threshold,
        max_bands.clamp(1, MAX_FAST_BANDS),
    )
}

fn find_precise_weighted_impl(
    frame: &RgbaImage,
    template: &RgbaImage,
    weights: &TemplateWeights,
    region: SearchRegion,
    threshold: f32,
    max_bands: usize,
) -> MatchReport {
    if weights.significant_count == 0 {
        // Nothing to weight: the template is entirely flat.
        return find_precise_impl(frame, template, region, threshold, max_bands);
    }
    let scan = RgbWeightedScan {
        scan: RgbScan {
            frame: frame.as_raw(),
            frame_width: frame.width() as usize,
            template: template.as_raw(),
            template_width: template.width() as usize,
            template_height: template.height() as usize,
        },
        weights,
    };
    find_banded(&scan, region, threshold, max_bands)
}

fn find_precise_weighted_candidates_impl(
    frame: &RgbaImage,
    template: &RgbaImage,
    weights: &TemplateWeights,
    region: SearchRegion,
    threshold: f32,
    max_bands: usize,
    candidate_limit: usize,
) -> (Vec<TemplateMatch>, f32) {
    let scan = RgbScan {
        frame: frame.as_raw(),
        frame_width: frame.width() as usize,
        template: template.as_raw(),
        template_width: template.width() as usize,
        template_height: template.height() as usize,
    };
    if weights.significant_count == 0 {
        return find_banded_candidates(&scan, region, threshold, max_bands, candidate_limit);
    }
    find_banded_candidates(
        &RgbWeightedScan { scan, weights },
        region,
        threshold,
        max_bands,
        candidate_limit,
    )
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

    fn scoring_pixels(&self) -> u64 {
        (self.template_width * self.template_height) as u64
    }

    fn bound_max_diff(&self, threshold: f32) -> u64 {
        ((1.0 - threshold) * self.scoring_pixels() as f32 * self.channel_scale()) as u64
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

    fn scoring_pixels(&self) -> u64 {
        (self.template_width * self.template_height) as u64
    }

    fn bound_max_diff(&self, threshold: f32) -> u64 {
        ((1.0 - threshold) * self.scoring_pixels() as f32 * self.channel_scale()) as u64
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
    bound_max_diff: u64,
    pixel_count: u64,
    template_sum: u64,
    region: SearchRegion,
    max_x: u32,
    max_y: u32,
    candidate_limit: usize,
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
            <= fast.bound_max_diff
    };

    let mut state = FastState::new(fast.candidate_limit);
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
struct FastState {
    best: Vec<(u64, u32, u32)>,
    best_score: f32,
    candidate_limit: usize,
}

impl FastState {
    fn new(candidate_limit: usize) -> Self {
        Self {
            best: Vec::with_capacity(candidate_limit),
            best_score: 0.0,
            candidate_limit,
        }
    }

    fn insert(&mut self, candidate: (u64, u32, u32)) {
        if let Some(index) = self.best.iter().position(|&(_, y, x)| {
            y.abs_diff(candidate.1) <= PRESCAN_STRIDE && x.abs_diff(candidate.2) <= PRESCAN_STRIDE
        }) {
            if candidate >= self.best[index] {
                return;
            }
            self.best.remove(index);
        }
        let position = self.best.binary_search(&candidate).unwrap_or_else(|at| at);
        self.best.insert(position, candidate);
        self.best.truncate(self.candidate_limit);
    }

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
        let abort_at = if self.best.len() >= self.candidate_limit {
            self.best
                .last()
                .map(|(difference, _, _)| difference.saturating_add(1))
                .unwrap_or(u64::MAX)
                .min(max_diff.saturating_add(1))
        } else {
            max_diff.saturating_add(1)
        };
        let Some(difference) = scan.diff_at(x, y, abort_at) else {
            return;
        };
        let score = 1.0 - difference as f32 / (pixel_count as f32 * scan.channel_scale());
        self.best_score = self.best_score.max(score);
        self.insert((difference, y, x));
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

    /// Naive dense weighted reference: every position gets the weighted
    /// coarse pass over significant samples and promising ones the exact
    /// weighted difference.
    fn naive_find_weighted(
        frame: &RgbaImage,
        template: &RgbaImage,
        weights: &TemplateWeights,
        region: SearchRegion,
        threshold: f32,
    ) -> MatchReport {
        let threshold = threshold.clamp(0.0, 1.0);
        let significant = weights.significant_count.max(1);
        let max_diff = ((1.0 - threshold) * significant as f32 * 765.0) as u64;
        let max_x = region.x + region.width - template.width();
        let max_y = region.y + region.height - template.height();
        let mut best: Option<TemplateMatch> = None;
        let mut best_diff = u64::MAX;
        let mut best_score = 0.0_f32;

        for y in region.y..=max_y {
            for x in region.x..=max_x {
                let coarse = naive_weighted_coarse(frame, template, weights, x, y);
                best_score = best_score.max(coarse);
                if coarse + 0.06 < threshold {
                    continue;
                }
                let abort_at = max_diff.saturating_add(1).min(best_diff);
                let Some(difference) =
                    naive_weighted_diff(frame, template, weights, x, y, abort_at)
                else {
                    continue;
                };
                let score = 1.0 - difference as f32 / (significant as f32 * 765.0);
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

    fn weighted_pixel_diff(
        frame: &RgbaImage,
        template: &RgbaImage,
        index: u32,
        origin_x: u32,
        origin_y: u32,
    ) -> u64 {
        let tx = index % template.width();
        let ty = index / template.width();
        let frame_pixel = frame.get_pixel(origin_x + tx, origin_y + ty);
        let template_pixel = template.get_pixel(tx, ty);
        let mut difference = 0_u64;
        for channel in 0..3 {
            difference += frame_pixel[channel].abs_diff(template_pixel[channel]) as u64;
        }
        difference
    }

    fn naive_weighted_coarse(
        frame: &RgbaImage,
        template: &RgbaImage,
        weights: &TemplateWeights,
        origin_x: u32,
        origin_y: u32,
    ) -> f32 {
        let mut difference = 0_u64;
        let mut samples = 0_u64;
        for &index in &weights.significant_samples {
            difference += weighted_pixel_diff(frame, template, index, origin_x, origin_y);
            samples += 1;
        }
        1.0 - difference as f32 / (samples.max(1) as f32 * 765.0)
    }

    fn naive_weighted_diff(
        frame: &RgbaImage,
        template: &RgbaImage,
        weights: &TemplateWeights,
        origin_x: u32,
        origin_y: u32,
        abort_at: u64,
    ) -> Option<u64> {
        let mut difference = 0_u64;
        for &index in &weights.significant_indices {
            difference += weighted_pixel_diff(frame, template, index, origin_x, origin_y);
            if difference >= abort_at {
                return None;
            }
        }
        Some(difference)
    }

    fn assert_weighted_equivalent(
        frame: &RgbaImage,
        template: &RgbaImage,
        threshold: f32,
        context: &str,
    ) {
        let weights = TemplateWeights::analyze(template);
        let region = SearchRegion::full(frame);
        let weighted =
            find_template_report_rgb_weighted(frame, template, &weights, region, threshold);
        let reference = naive_find_weighted(frame, template, &weights, region, threshold);
        if let Some(expected) = reference.matched {
            let found = weighted
                .matched
                .unwrap_or_else(|| panic!("{context}: reference matched but weighted did not"));
            assert!(
                (found.score - expected.score).abs() <= 0.02,
                "{context}: score {:.3} vs reference {:.3}",
                found.score,
                expected.score
            );
        }
        assert!(
            (weighted.best_score - reference.best_score).abs() <= 0.02,
            "{context}: best score {:.3} vs reference {:.3}",
            weighted.best_score,
            reference.best_score
        );
    }

    #[test]
    fn weighted_matches_reference_on_embedded_noise_template() {
        let (frame, template) = embedded_noise_rgb_case();
        assert_weighted_equivalent(&frame, &template, 0.90, "weighted embedded noise");
    }

    #[test]
    fn weighted_matches_reference_on_pure_noise_without_match() {
        let (frame, template) = pure_noise_rgb_case();
        assert_weighted_equivalent(&frame, &template, 0.90, "weighted pure noise");
    }

    #[test]
    fn weighted_multithread_matches_single_thread() {
        for (name, (frame, template)) in [
            ("embedded noise", embedded_noise_rgb_case()),
            ("pure noise", pure_noise_rgb_case()),
        ] {
            let weights = TemplateWeights::analyze(&template);
            let region = SearchRegion::full(&frame);
            let single = find_precise_weighted_impl(&frame, &template, &weights, region, 0.90, 1);
            let multi = find_precise_weighted_impl(
                &frame,
                &template,
                &weights,
                region,
                0.90,
                MAX_FAST_BANDS,
            );
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
    fn analyze_marks_flat_template_as_background_and_degrades() {
        let flat = RgbaImage::from_pixel(16, 16, Rgba([150, 150, 150, 255]));
        let weights = TemplateWeights::analyze(&flat);
        assert_eq!(weights.significant_count, 0);
        assert!(weights.significant_indices.is_empty());
        assert!(weights.is_mostly_background());
        assert_eq!(weights.total_sum, 16 * 16 * 450);

        // A fully flat template degrades to the unweighted engine exactly.
        let mut rng = XorShift(0x5771_2A01);
        let frame = noise_rgb_image(&mut rng, 64, 48);
        let region = SearchRegion::full(&frame);
        let unweighted = find_precise_impl(&frame, &flat, region, 0.5, 1);
        let weighted = find_precise_weighted_impl(&frame, &flat, &weights, region, 0.5, 1);
        assert_eq!(
            unweighted
                .matched
                .map(|found| (found.x, found.y, found.score.to_bits())),
            weighted
                .matched
                .map(|found| (found.x, found.y, found.score.to_bits()))
        );
        assert_eq!(
            unweighted.best_score.to_bits(),
            weighted.best_score.to_bits()
        );
    }

    #[test]
    fn analyze_counts_significant_pixels() {
        // 90% background (160) + 10% text (60): mean 150, so the background
        // deviates by 10 (< epsilon) and only the text is significant.
        let mut sparse = RgbaImage::from_pixel(10, 10, Rgba([160, 160, 160, 255]));
        for x in 0..10 {
            sparse.put_pixel(x, 0, Rgba([60, 60, 60, 255]));
        }
        let weights = TemplateWeights::analyze(&sparse);
        assert_eq!(weights.significant_count, 10);
        assert_eq!(weights.significant_sum, 10 * 180);
        assert_eq!(weights.total_sum, 90 * 480 + 10 * 180);
        assert!(weights.is_mostly_background());

        // 80% background (100) + 20% slightly-off pixels (114): mean 102,
        // text deviation is exactly epsilon.
        let mut dense = RgbaImage::from_pixel(10, 10, Rgba([100, 100, 100, 255]));
        for x in 0..10 {
            dense.put_pixel(x, 0, Rgba([114, 114, 114, 255]));
            dense.put_pixel(x, 1, Rgba([114, 114, 114, 255]));
        }
        let weights = TemplateWeights::analyze(&dense);
        assert_eq!(weights.significant_count, 20);
        assert!(!weights.is_mostly_background());
    }

    #[test]
    fn alpha_channel_supplies_an_explicit_mask() {
        let mut template = RgbaImage::from_pixel(4, 4, Rgba([80, 90, 100, 0]));
        template.put_pixel(1, 1, Rgba([240, 240, 240, 255]));
        template.put_pixel(2, 2, Rgba([20, 20, 20, 128]));
        let weights = TemplateWeights::analyze(&template);

        assert!(weights.explicit_alpha_mask);
        assert_eq!(weights.significant_indices, vec![5, 10]);
        assert_eq!(weights.significant_count, 2);

        let incidental_alpha = RgbaImage::from_pixel(4, 4, Rgba([80, 90, 100, 254]));
        assert!(!TemplateWeights::analyze(&incidental_alpha).explicit_alpha_mask);
    }

    #[test]
    fn hybrid_rejects_same_chrome_with_different_symbol() {
        let mut template = RgbaImage::from_fn(32, 20, |x, _| {
            let value = 70 + (x * 2) as u8;
            Rgba([value, value + 10, value + 20, 255])
        });
        let mut impostor = template.clone();
        for y in 4..16 {
            for x in [9, 10, 20, 21] {
                template.put_pixel(x, y, Rgba([240, 240, 240, 255]));
            }
        }
        for y in [6, 7, 12, 13] {
            for x in 6..26 {
                impostor.put_pixel(x, y, Rgba([240, 240, 240, 255]));
            }
        }
        let weights = TemplateWeights::analyze(&template);
        let region = SearchRegion::full(&impostor);
        let color_only = find_template_report_rgb_weighted(
            &impostor,
            &template,
            &weights,
            region,
            MIN_HYBRID_THRESHOLD,
        );
        assert!(color_only.matched.is_some());

        let hybrid = find_template_report_rgb_hybrid(
            &impostor,
            &template,
            &weights,
            region,
            MIN_HYBRID_THRESHOLD,
        );
        assert!(hybrid.matched.is_none());
    }

    #[test]
    fn hybrid_checks_multiple_candidates_instead_of_only_best_sad() {
        let mut template = RgbaImage::from_fn(24, 20, |x, _| {
            let value = 70 + (x * 2) as u8;
            Rgba([value, value + 10, value + 20, 255])
        });
        for y in 4..16 {
            for x in [7, 8, 15, 16] {
                template.put_pixel(x, y, Rgba([210, 210, 210, 255]));
            }
        }
        let mut impostor = template.clone();
        for y in 4..16 {
            for x in [7, 8, 15, 16] {
                impostor.put_pixel(x, y, Rgba([100, 110, 120, 255]));
            }
        }
        for y in [6, 7, 12, 13] {
            for x in 5..19 {
                impostor.put_pixel(x, y, Rgba([210, 210, 210, 255]));
            }
        }
        let brighter = RgbaImage::from_fn(template.width(), template.height(), |x, y| {
            let source = template.get_pixel(x, y);
            Rgba([
                source[0].saturating_add(35),
                source[1].saturating_add(35),
                source[2].saturating_add(35),
                255,
            ])
        });
        let mut frame = RgbaImage::from_pixel(72, 20, Rgba([20, 20, 20, 255]));
        image::imageops::replace(&mut frame, &impostor, 0, 0);
        image::imageops::replace(&mut frame, &brighter, 48, 0);
        let weights = TemplateWeights::analyze(&template);
        let region = SearchRegion::full(&frame);

        let color_only =
            find_template_report_rgb_weighted(&frame, &template, &weights, region, 0.78)
                .matched
                .expect("both button candidates should pass the color gate");
        assert_eq!(color_only.x, 0, "the look-alike should win SAD alone");

        let hybrid = find_template_report_rgb_hybrid(&frame, &template, &weights, region, 0.78)
            .matched
            .expect("hybrid matching should recover the structurally correct button");
        assert_eq!(hybrid.x, 48);
    }

    #[test]
    fn thorough_hybrid_recovers_low_variance_template_rejected_by_fast_discovery() {
        let template = RgbaImage::from_pixel(12, 8, Rgba([80, 120, 180, 255]));
        let weights = TemplateWeights::analyze(&template);
        assert!(weights.significant_count < 8);
        let mut frame = RgbaImage::from_pixel(40, 20, Rgba([10, 20, 30, 255]));
        image::imageops::replace(&mut frame, &template, 17, 6);
        let region = SearchRegion::full(&frame);

        let fast = find_template_report_rgb_hybrid_fast_with_bands(
            &frame, &template, &weights, region, 0.90, 2,
        );
        assert!(fast.matched.is_none());

        let thorough = find_template_report_rgb_hybrid_with_bands(
            &frame, &template, &weights, region, 0.90, 2,
        )
        .matched
        .expect("thorough RGB fallback must retain low-variance template compatibility");
        assert_eq!((thorough.x, thorough.y), (17, 6));
    }

    #[test]
    fn structural_similarity_normalizes_brightness_but_rejects_changed_shape() {
        let template = RgbaImage::from_fn(8, 8, |x, y| {
            let value = if x == y || x + y == 7 { 220 } else { 40 };
            Rgba([value, value, value, 255])
        });
        let weights = TemplateWeights::analyze(&template);
        let brighter = RgbaImage::from_fn(8, 8, |x, y| {
            let value = if x == y || x + y == 7 { 250 } else { 90 };
            Rgba([value, value, value, 255])
        });
        let changed = RgbaImage::from_fn(8, 8, |x, _| {
            let value = if x < 4 { 220 } else { 40 };
            Rgba([value, value, value, 255])
        });

        let same_shape = structural_similarity_at(&brighter, &template, &weights, 0, 0)
            .expect("structured templates should have a correlation score");
        let changed_shape = structural_similarity_at(&changed, &template, &weights, 0, 0)
            .expect("structured templates should have a correlation score");
        assert!(same_shape > 0.99);
        assert!(changed_shape < 0.5);
    }

    /// The selling point of weighting: a large flat template background plus
    /// a small text block. Region A is pure background (unweighted matching
    /// accepts it — the classic misclick); region B has a slightly different
    /// background but the exact same text. Weighted matching must pick B.
    #[test]
    fn weighted_prefers_text_over_flat_background() {
        let mut template = RgbaImage::from_pixel(24, 24, Rgba([200, 200, 200, 255]));
        for y in 10..14 {
            for x in 10..14 {
                template.put_pixel(x, y, Rgba([40, 40, 40, 255]));
            }
        }
        let weights = TemplateWeights::analyze(&template);
        assert_eq!(weights.significant_count, 16);

        // Region A at (8, 8): pure template background, no text at all.
        let mut frame = RgbaImage::from_pixel(64, 48, Rgba([120, 120, 120, 255]));
        for y in 0..24 {
            for x in 0..24 {
                frame.put_pixel(8 + x, 8 + y, Rgba([200, 200, 200, 255]));
            }
        }
        // Region B at (32, 8): background slightly off, text identical.
        for y in 0..24 {
            for x in 0..24 {
                frame.put_pixel(32 + x, 8 + y, Rgba([190, 200, 210, 255]));
            }
        }
        for y in 10..14 {
            for x in 10..14 {
                frame.put_pixel(32 + x, 8 + y, Rgba([40, 40, 40, 255]));
            }
        }

        let region = SearchRegion::full(&frame);
        let unweighted = find_template_report_rgb(&frame, &template, region, 0.97);
        let impostor = unweighted
            .matched
            .expect("unweighted matching should fall for the flat region");
        assert_eq!(
            (impostor.x, impostor.y),
            (8, 8),
            "unweighted matching picks the text-free flat region — the misclick"
        );

        let weighted = find_template_report_rgb_weighted(&frame, &template, &weights, region, 0.97);
        let found = weighted
            .matched
            .expect("weighted matching should find the region with the real text");
        assert_eq!((found.x, found.y), (32, 8));
        assert!(found.score > 0.99);
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

            // Weighted variants of both scans.
            let noise_weights = TemplateWeights::analyze(&noise_template);
            let started = std::time::Instant::now();
            let weighted_single = find_precise_weighted_impl(
                &noise_frame,
                &noise_template,
                &noise_weights,
                SearchRegion::full(&noise_frame),
                0.90,
                1,
            );
            let weighted_single_elapsed = started.elapsed();
            let started = std::time::Instant::now();
            let weighted_multi = find_precise_weighted_impl(
                &noise_frame,
                &noise_template,
                &noise_weights,
                SearchRegion::full(&noise_frame),
                0.90,
                MAX_FAST_BANDS,
            );
            let weighted_multi_elapsed = started.elapsed();
            eprintln!(
                "{width}×{height} noise scan: weighted 1-thread {weighted_single_elapsed:?}, \
                 weighted {MAX_FAST_BANDS}-thread {weighted_multi_elapsed:?}"
            );
            assert_eq!(weighted_single.matched, weighted_multi.matched);

            let real_weights = TemplateWeights::analyze(&real_template);
            let started = std::time::Instant::now();
            let weighted_single = find_precise_weighted_impl(
                &real_frame,
                &real_template,
                &real_weights,
                region,
                0.90,
                1,
            );
            let weighted_single_elapsed = started.elapsed();
            let started = std::time::Instant::now();
            let weighted_multi = find_precise_weighted_impl(
                &real_frame,
                &real_template,
                &real_weights,
                region,
                0.90,
                MAX_FAST_BANDS,
            );
            let weighted_multi_elapsed = started.elapsed();
            eprintln!(
                "{width}×{height} realistic scan: weighted 1-thread {weighted_single_elapsed:?}, \
                 weighted {MAX_FAST_BANDS}-thread {weighted_multi_elapsed:?}"
            );
            assert_eq!(weighted_single.matched, weighted_multi.matched);

            let mut matched_frame = real_frame.clone();
            let match_x = width / 2;
            let match_y = height / 2;
            image::imageops::replace(
                &mut matched_frame,
                &real_template,
                i64::from(match_x),
                i64::from(match_y),
            );
            for bands in [2, 4, 8] {
                let started = std::time::Instant::now();
                let hybrid_fast = find_template_report_rgb_hybrid_fast_with_bands(
                    &matched_frame,
                    &real_template,
                    &real_weights,
                    region,
                    0.90,
                    bands,
                );
                let hybrid_fast_elapsed = started.elapsed();
                eprintln!(
                    "{width}×{height} exact-match scan: hybrid-fast {bands}-thread \
                     {hybrid_fast_elapsed:?}"
                );
                assert_eq!(
                    hybrid_fast.matched.map(|found| (found.x, found.y)),
                    Some((match_x, match_y))
                );
            }
            let hybrid_thorough = find_template_report_rgb_hybrid_with_bands(
                &matched_frame,
                &real_template,
                &real_weights,
                region,
                0.90,
                4,
            );
            assert_eq!(
                hybrid_thorough.matched.map(|found| (found.x, found.y)),
                Some((match_x, match_y))
            );

            let started = std::time::Instant::now();
            let hybrid_fast_miss = find_template_report_rgb_hybrid_fast_with_bands(
                &real_frame,
                &real_template,
                &real_weights,
                region,
                0.90,
                4,
            );
            let hybrid_fast_miss_elapsed = started.elapsed();
            let started = std::time::Instant::now();
            let hybrid_thorough_miss = find_template_report_rgb_hybrid_with_bands(
                &real_frame,
                &real_template,
                &real_weights,
                region,
                0.90,
                4,
            );
            let hybrid_thorough_miss_elapsed = started.elapsed();
            eprintln!(
                "{width}×{height} no-match scan: hybrid-fast 4-thread \
                 {hybrid_fast_miss_elapsed:?}, thorough {hybrid_thorough_miss_elapsed:?}"
            );
            assert!(hybrid_fast_miss.matched.is_none());
            assert!(hybrid_thorough_miss.matched.is_none());

            // Mostly-flat template (2% significant pixels), the weighted
            // engine's home turf.
            let mut flat_template = RgbaImage::from_pixel(120, 80, Rgba([200, 200, 200, 255]));
            for y in 34..46 {
                for x in 54..66 {
                    flat_template.put_pixel(x, y, Rgba([40, 40, 40, 255]));
                }
            }
            let flat_weights = TemplateWeights::analyze(&flat_template);
            let started = std::time::Instant::now();
            let weighted_single = find_precise_weighted_impl(
                &real_frame,
                &flat_template,
                &flat_weights,
                region,
                0.90,
                1,
            );
            let weighted_single_elapsed = started.elapsed();
            let started = std::time::Instant::now();
            let weighted_multi = find_precise_weighted_impl(
                &real_frame,
                &flat_template,
                &flat_weights,
                region,
                0.90,
                MAX_FAST_BANDS,
            );
            let weighted_multi_elapsed = started.elapsed();
            eprintln!(
                "{width}×{height} flat-template scan: weighted 1-thread {weighted_single_elapsed:?}, \
                 weighted {MAX_FAST_BANDS}-thread {weighted_multi_elapsed:?} \
                 (significant {}%)",
                (flat_weights.significant_ratio() * 100.0).round() as u32
            );
            assert_eq!(weighted_single.matched, weighted_multi.matched);
        }
    }
}
