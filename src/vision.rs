use image::GrayImage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl SearchRegion {
    pub fn full(image: &GrayImage) -> Self {
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

/// Finds the closest grayscale template using mean absolute pixel similarity.
///
/// The workflow editor encourages a tight search region, so this deliberately
/// favors a compact pure-Rust implementation over a large native CV runtime.
///
/// Speed: pixel access goes through the raw image buffers, and large search
/// areas are pre-scanned on a stride-4 grid; only grid points whose coarse
/// score is within 0.08 of the threshold get a full-resolution refinement of
/// their ±3 neighborhood. Small areas are scanned densely, like before.
pub fn find_template_report(
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

#[cfg(test)]
mod tests {
    use image::{GrayImage, Luma};

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
        let found = find_template_report(&frame, &template, SearchRegion::full(&frame), 0.99)
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
            find_template_report(&frame, &template, region, 0.99)
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
            find_template_report(&frame, &template, region, 0.8)
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

        let found = find_template_report(&frame, &template, SearchRegion::full(&frame), 0.5)
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
        let report = find_template_report(&frame, &template, SearchRegion::full(&frame), 0.99);

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
        context: &str,
    ) {
        let region = SearchRegion::full(frame);
        let optimized = find_template_report(frame, template, region, threshold);
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

    #[test]
    fn matches_reference_on_embedded_noise_template() {
        let mut rng = XorShift(0x5771_A001);
        let mut frame = noise_image(&mut rng, 512, 384);
        let template = noise_image(&mut rng, 96, 72);
        // Stride-aligned position: the pre-scan grid sees the exact match.
        embed(&mut frame, &template, 128, 96);

        assert_reports_equivalent(&frame, &template, 0.90, "embedded noise template");
    }

    #[test]
    fn matches_reference_on_unaligned_smooth_template() {
        let mut rng = XorShift(0x5771_B002);
        let frame = smooth_image(&mut rng, 512, 384);
        // Template cut from an off-grid position of the same smooth frame.
        let template = image::imageops::crop_imm(&frame, 157, 83, 96, 72).to_image();

        assert_reports_equivalent(&frame, &template, 0.90, "unaligned smooth template");
    }

    #[test]
    fn matches_reference_on_degraded_unaligned_template() {
        let mut rng = XorShift(0x5771_C003);
        let frame = smooth_image(&mut rng, 512, 384);
        let mut template = image::imageops::crop_imm(&frame, 157, 83, 96, 72).to_image();
        // Degrade the template slightly, like a real capture with artifacts.
        for pixel in template.pixels_mut() {
            let delta = (rng.next() % 13) as i32 - 6;
            pixel[0] = pixel[0].saturating_add_signed(delta as i8);
        }

        assert_reports_equivalent(&frame, &template, 0.90, "degraded template");
    }

    #[test]
    fn matches_reference_on_pure_noise_without_match() {
        let mut rng = XorShift(0x5771_D004);
        let frame = noise_image(&mut rng, 512, 384);
        let template = noise_image(&mut rng, 96, 72);

        assert_reports_equivalent(&frame, &template, 0.90, "pure noise");
    }

    #[test]
    #[ignore = "performance comparison, run with cargo test --release -- --ignored"]
    fn perf_full_hd_scan() {
        let mut rng = XorShift(0x5771_E005);
        let frame = noise_image(&mut rng, 1920, 1080);
        let template = noise_image(&mut rng, 120, 80);
        let region = SearchRegion::full(&frame);

        let started = std::time::Instant::now();
        let optimized = find_template_report(&frame, &template, region, 0.90);
        let optimized_elapsed = started.elapsed();

        let started = std::time::Instant::now();
        let reference = naive_find_template_report(&frame, &template, region, 0.90);
        let reference_elapsed = started.elapsed();

        eprintln!(
            "1920×1080 full scan: optimized {optimized_elapsed:?}, naive {reference_elapsed:?}, speedup {:.1}×",
            reference_elapsed.as_secs_f64() / optimized_elapsed.as_secs_f64()
        );
        assert_eq!(optimized.matched.is_some(), reference.matched.is_some());
    }
}
