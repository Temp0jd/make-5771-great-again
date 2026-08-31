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
    let mut best: Option<TemplateMatch> = None;
    let mut best_diff = u64::MAX;
    let mut best_score = 0.0_f32;

    for y in region.y..=max_y {
        for x in region.x..=max_x {
            let coarse = coarse_score(frame, template, x, y);
            best_score = best_score.max(coarse);
            if coarse + 0.06 < threshold {
                continue;
            }
            let abort_at = max_diff.saturating_add(1).min(best_diff);
            let Some(difference) = diff_at(frame, template, x, y, abort_at) else {
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

fn coarse_score(frame: &GrayImage, template: &GrayImage, origin_x: u32, origin_y: u32) -> f32 {
    // Keep the cheap rejection pass near a fixed sample budget even when the
    // selected template is large. This is the main guard against high CPU use.
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

/// Returns the summed absolute pixel difference at `origin`, bailing out as
/// soon as the accumulated difference reaches `abort_at` (meaning the position
/// can no longer qualify or beat the current best match).
fn diff_at(
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
}
