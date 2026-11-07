//! The vis pane: what the speaker is playing, drawn from the samples it hands the run screen
//! (`Speaker::listen`), never from the game. Each chrome dresses it (`ChromeView::vis`); the
//! maths (spectrum, level) is core's `scope`. With nothing playing it lies flat and still.

use egui::{Sense, Stroke, Ui, pos2};

use crate::theme::Palette;

/// An oscilloscope trace of `samples` across the pane, in the text colour on a dim centre line:
/// the classic dressing.
pub fn trace(ui: &mut Ui, p: &Palette, samples: &[f32]) {
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let painter = ui.painter_at(rect);
    let mid = rect.center().y.round();
    painter.line_segment(
        [pos2(rect.left(), mid), pos2(rect.right(), mid)],
        Stroke::new(1.0, p.faint),
    );
    let points = trace_points(samples, rect.width() as usize);
    if points.iter().all(|v| *v == 0.0) {
        return;
    }
    let half = rect.height() / 2.0 - 2.0;
    let line: Vec<egui::Pos2> = points
        .iter()
        .enumerate()
        .map(|(i, v)| pos2(rect.left() + i as f32, mid - v.clamp(-1.0, 1.0) * half))
        .collect();
    painter.add(egui::Shape::line(line, Stroke::new(1.0, p.fg)));
}

/// `samples` cut to `width` points, one per pixel column (each the column's loudest swing, so
/// a narrow pane never hides a peak). Scaled so a run tune fills the pane without clipping.
pub fn trace_points(samples: &[f32], width: usize) -> Vec<f32> {
    if width == 0 || samples.is_empty() {
        return vec![0.0; width];
    }
    (0..width)
        .map(|x| {
            let a = x * samples.len() / width;
            let b = ((x + 1) * samples.len() / width)
                .max(a + 1)
                .min(samples.len());
            let s = &samples[a..b];
            let hi = s.iter().copied().fold(0.0f32, f32::max);
            let lo = s.iter().copied().fold(0.0f32, f32::min);
            (if hi >= -lo { hi } else { lo }) * TRACE_GAIN
        })
        .collect()
}

/// How much a trace is magnified: run music peaks around 0.3.
pub const TRACE_GAIN: f32 = 3.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trace_keeps_its_peaks_and_lies_flat_in_silence() {
        assert_eq!(trace_points(&[], 4), [0.0; 4]);
        assert_eq!(trace_points(&[0.0; 100], 4), [0.0; 4]);
        let mut s = vec![0.0; 100];
        s[10] = 0.1;
        s[60] = -0.2;
        let t = trace_points(&s, 4);
        assert!((t[0] - 0.3).abs() < 1e-6, "{t:?}");
        assert!((t[2] + 0.6).abs() < 1e-6, "{t:?}");
        assert_eq!(trace_points(&s, 400).len(), 400, "wider than the samples");
    }
}
