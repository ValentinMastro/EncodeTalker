use crate::app::VmafGraphData;
use ratatui::{
    prelude::*,
    widgets::{
        canvas::{Canvas, Points},
        Block, Borders, Clear, Paragraph,
    },
};

/// Downsampler les données de frames pour correspondre à la largeur effective du terminal
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn downsample(frames: &[(f64, f64)], target_points: usize) -> Vec<(f64, f64)> {
    if frames.len() <= target_points || target_points == 0 {
        return frames.to_vec();
    }

    let bucket_size = frames.len() as f64 / target_points as f64;
    let mut result = Vec::with_capacity(target_points);

    for i in 0..target_points {
        let start = (i as f64 * bucket_size) as usize;
        let end = (((i + 1) as f64 * bucket_size) as usize).min(frames.len());
        if start < end {
            let avg_frame = frames[start].0 + (frames[end - 1].0 - frames[start].0) / 2.0;
            let avg_vmaf: f64 =
                frames[start..end].iter().map(|(_, v)| v).sum::<f64>() / (end - start) as f64;
            result.push((avg_frame, avg_vmaf));
        }
    }
    result
}

/// Rendre le graphe VMAF en overlay plein écran
#[allow(clippy::too_many_lines)]
pub fn render_vmaf_graph(frame: &mut Frame, area: Rect, data: &VmafGraphData) {
    let dialog_area = fullscreen_rect(area);

    frame.render_widget(Clear, dialog_area);

    // Layout : stats + graphe + footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Barre de stats
            Constraint::Min(10),   // Zone graphe
            Constraint::Length(1), // Footer
        ])
        .split(dialog_area);

    // Barre de stats
    let stats_text = format!(
        " {} | Frames: {} | Moyenne: {:.2} | Min: {:.2} | Max: {:.2}{}",
        data.filename,
        data.total_frames,
        data.mean,
        data.min,
        data.max,
        data.harmonic_mean
            .map(|h| format!(" | Harmonique: {h:.2}"))
            .unwrap_or_default(),
    );
    let stats = Paragraph::new(stats_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" VMAF par frame "),
        )
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(stats, chunks[0]);

    // Résolution effective Braille : 2 points par cellule en largeur
    let graph_area = chunks[1];
    let effective_width = (graph_area.width.saturating_sub(2) as usize) * 2;

    // Downsampler les données
    let downsampled = downsample(&data.frames, effective_width);

    let x_max = data.frames.last().map_or(1.0, |(x, _)| *x).max(1.0);

    // Trouver la frame avec le score VMAF minimum
    let min_frame = data
        .frames
        .iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .copied()
        .unwrap_or((0.0, 0.0));

    // Laisser de la marge en bas pour le label min
    let y_range = 100.0 - data.min.floor().min(80.0);
    let y_min = data.min.floor().min(80.0) - y_range * 0.08;

    // Grouper les points par bande de couleur
    let mut red_points = Vec::new();
    let mut orange_points = Vec::new();
    let mut yellow_points = Vec::new();
    let mut lime_points = Vec::new();
    let mut green_points = Vec::new();

    for &(x, y) in &downsampled {
        let target = if y < 80.0 {
            &mut red_points
        } else if y < 85.0 {
            &mut orange_points
        } else if y < 90.0 {
            &mut yellow_points
        } else if y < 95.0 {
            &mut lime_points
        } else {
            &mut green_points
        };
        target.push((x, y));
    }

    let y_base = data.min.floor().min(80.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let min_label = format!("▲ {:.2} (f.{})", min_frame.1, min_frame.0 as u64);
    let min_label_color = if min_frame.1 < 80.0 {
        Color::Red
    } else if min_frame.1 < 85.0 {
        Color::Rgb(255, 128, 0)
    } else if min_frame.1 < 90.0 {
        Color::Yellow
    } else if min_frame.1 < 95.0 {
        Color::Rgb(128, 255, 0)
    } else {
        Color::Green
    };
    let canvas = Canvas::default()
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Score VMAF ({y_base:.0}-100) | {} frames ",
            data.total_frames
        )))
        .x_bounds([0.0, x_max])
        .y_bounds([y_min, 100.0])
        .marker(Marker::Braille)
        .paint(move |ctx| {
            // Points de données par bande de couleur
            if !red_points.is_empty() {
                ctx.draw(&Points {
                    coords: &red_points,
                    color: Color::Red,
                });
            }
            if !orange_points.is_empty() {
                ctx.draw(&Points {
                    coords: &orange_points,
                    color: Color::Rgb(255, 128, 0),
                });
            }
            if !yellow_points.is_empty() {
                ctx.draw(&Points {
                    coords: &yellow_points,
                    color: Color::Yellow,
                });
            }
            if !lime_points.is_empty() {
                ctx.draw(&Points {
                    coords: &lime_points,
                    color: Color::Rgb(128, 255, 0),
                });
            }
            if !green_points.is_empty() {
                ctx.draw(&Points {
                    coords: &green_points,
                    color: Color::Green,
                });
            }

            // Labels Y-axis
            ctx.print(0.0, 100.0, "100".fg(Color::White));
            ctx.print(0.0, 90.0, " 90".fg(Color::Yellow));
            if y_base < 80.0 {
                ctx.print(0.0, 80.0, " 80".fg(Color::Red));
            }

            // Label frame min (en dessous du point, flèche vers le haut)
            let label_y = min_frame.1 - (100.0 - y_min) * 0.05;
            ctx.print(min_frame.0, label_y, min_label.clone().fg(min_label_color));
        });

    frame.render_widget(canvas, graph_area);

    // Footer
    let footer = Paragraph::new(" ESC: Retour ")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}

/// Créer un rectangle quasi plein écran avec une petite marge
fn fullscreen_rect(area: Rect) -> Rect {
    let margin_x = (area.width / 20).max(1);
    let margin_y = (area.height / 20).max(1);
    Rect {
        x: area.x + margin_x,
        y: area.y + margin_y,
        width: area.width.saturating_sub(margin_x * 2),
        height: area.height.saturating_sub(margin_y * 2),
    }
}
