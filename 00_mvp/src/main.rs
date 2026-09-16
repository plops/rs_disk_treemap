use macroquad::prelude::*;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::thread;

struct Node {
    name: String,
    size: u64,
    is_dir: bool,
    children: Vec<Node>,
    rect: Rect,
    color: Color,
}

// ----------------------------------------------------------------------------
// 1. Filesystem Traversal (Gracefully ignores symlinks & permission errors)
// ----------------------------------------------------------------------------
fn scan_tree(path: &Path) -> Node {
    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let mut node = Node {
        name,
        size: 0,
        is_dir: true,
        children: Vec::new(),
        rect: Rect::default(),
        color: Color::new(0.15, 0.17, 0.22, 1.0),
    };

    // Skip virtual Linux trees if launched at root
    let s = path.to_string_lossy();
    if s.starts_with("/proc") || s.starts_with("/sys") || s.starts_with("/dev") {
        return node;
    }

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if ft.is_symlink() {
                continue;
            }

            if ft.is_dir() {
                let child = scan_tree(&entry.path());
                if child.size > 0 {
                    node.size += child.size;
                    node.children.push(child);
                }
            } else if ft.is_file() {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                if size > 0 && size < (1 << 48) {
                    node.size += size;
                    let name = entry.file_name().to_string_lossy().into_owned();
                    node.children.push(Node {
                        color: color_for_name(&name),
                        name,
                        size,
                        is_dir: false,
                        children: Vec::new(),
                        rect: Rect::default(),
                    });
                }
            }
        }
    }

    node
}

// ----------------------------------------------------------------------------
// 2. Squarified Treemap Algorithm (Bruls, Huizing, van Wijk)
// ----------------------------------------------------------------------------
fn squarify(nodes: &mut [Node], mut rect: Rect) {
    let total: u64 = nodes.iter().map(|n| n.size).sum();
    if total == 0 || rect.w <= 0.0 || rect.h <= 0.0 {
        return;
    }

    nodes.sort_unstable_by_key(|n| std::cmp::Reverse(n.size));
    let area_mult = (rect.w * rect.h) as f64 / total as f64;
    let areas: Vec<f64> = nodes.iter().map(|n| n.size as f64 * area_mult).collect();

    let worst = |row: &[usize], sum: f64, side: f32| -> f64 {
        let (s2, sum2) = ((side * side) as f64, sum * sum);
        row.iter()
            .map(|&i| (s2 * areas[i] / sum2).max(sum2 / (s2 * areas[i])))
            .fold(0.0, f64::max)
    };

    let mut layout_row = |nodes: &mut [Node], row: &[usize], sum: f64, r: &mut Rect| {
        let side = r.w.min(r.h);
        let thickness = (sum / side as f64) as f32;
        let is_horiz = r.w < r.h;
        let mut offset = if is_horiz { r.x } else { r.y };

        for &i in row {
            let item_len = (areas[i] / sum * side as f64) as f32;
            nodes[i].rect = if is_horiz {
                Rect::new(offset, r.y, item_len, thickness)
            } else {
                Rect::new(r.x, offset, thickness, item_len)
            };
            offset += item_len;
        }

        if is_horiz {
            r.y += thickness;
            r.h -= thickness;
        } else {
            r.x += thickness;
            r.w -= thickness;
        }
    };

    let mut row = Vec::new();
    let mut row_sum = 0.0;

    for i in 0..nodes.len() {
        let side = rect.w.min(rect.h);
        let mut next_row = row.clone();
        next_row.push(i);

        if row.is_empty() || worst(&next_row, row_sum + areas[i], side) <= worst(&row, row_sum, side) {
            row.push(i);
            row_sum += areas[i];
        } else {
            layout_row(nodes, &row, row_sum, &mut rect);
            row = vec![i];
            row_sum = areas[i];
        }
    }
    if !row.is_empty() {
        layout_row(nodes, &row, row_sum, &mut rect);
    }

    // Recurse into directories
    for node in nodes.iter_mut() {
        if node.is_dir && node.rect.w > 4.0 && node.rect.h > 4.0 {
            squarify(&mut node.children, node.rect);
        }
    }
}

// ----------------------------------------------------------------------------
// 3. Rendering & Utilities
// ----------------------------------------------------------------------------
fn render_tree(node: &Node, mouse: Vec2, hovered: &mut Option<String>) {
    if node.rect.w < 1.0 || node.rect.h < 1.0 {
        return;
    }

    if node.children.is_empty() {
        draw_rectangle(node.rect.x, node.rect.y, node.rect.w, node.rect.h, node.color);
        draw_rectangle_lines(node.rect.x, node.rect.y, node.rect.w, node.rect.h, 1.0, Color::new(0., 0., 0., 0.35));
    } else {
        for child in &node.children {
            render_tree(child, mouse, hovered);
        }
        draw_rectangle_lines(node.rect.x, node.rect.y, node.rect.w, node.rect.h, 1.0, Color::new(0., 0., 0., 0.6));
    }

    if node.rect.contains(mouse) {
        *hovered = Some(format!("{} ({})", node.name, format_bytes(node.size)));
    }
}

fn color_for_name(name: &str) -> Color {
    let ext = Path::new(name).extension().and_then(OsStr::to_str).unwrap_or("");
    match ext {
        "rs" | "c" | "cpp" | "py" | "js" | "txt" | "md" => Color::new(0.2, 0.75, 0.45, 1.0),
        "png" | "jpg" | "jpeg" | "svg" | "webp" => Color::new(0.2, 0.65, 0.95, 1.0),
        "mp4" | "mkv" | "mov" | "mp3" | "flac" => Color::new(0.75, 0.35, 0.85, 1.0),
        "zip" | "tar" | "gz" | "7z" => Color::new(0.9, 0.3, 0.25, 1.0),
        _ => {
            let h = name.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32));
            Color::from_rgba((h * 37 % 160 + 80) as u8, (h * 59 % 160 + 80) as u8, (h * 83 % 160 + 80) as u8, 255)
        }
    }
}

fn format_bytes(b: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let (mut d, mut i) = (b as f64, 0);
    while d >= 1024.0 && i < UNITS.len() - 1 {
        d /= 1024.0;
        i += 1;
    }
    format!("{:.1} {}", d, UNITS[i])
}

// ----------------------------------------------------------------------------
// 4. Main Event Loop
// ----------------------------------------------------------------------------
#[macroquad::main("Treemap Disk Visualizer MVP")]
async fn main() {
    let target = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    let (tx, rx): (std::sync::mpsc::Sender<Node>, Receiver<Node>) = channel();

    // Scan in background thread to prevent UI freezing
    thread::spawn(move || {
        let root = scan_tree(&target);
        let _ = tx.send(root);
    });

    let mut root: Option<Node> = None;
    let mut last_size = (0.0, 0.0);

    loop {
        clear_background(Color::new(0.08, 0.09, 0.12, 1.0));
        let (screen_w, screen_h) = (screen_width(), screen_height());

        if let Ok(loaded_root) = rx.try_recv() {
            root = Some(loaded_root);
            last_size = (0.0, 0.0); // Trigger relayout
        }

        if let Some(tree) = &mut root {
            let canvas = Rect::new(0.0, 36.0, screen_w, screen_h - 36.0);

            // Recompute layout only when the window is resized
            if (screen_w, screen_h) != last_size {
                tree.rect = canvas;
                squarify(&mut tree.children, canvas);
                last_size = (screen_w, screen_h);
            }

            let mut hovered = None;
            render_tree(tree, Vec2::from(mouse_position()), &mut hovered);

            // Top Status Bar
            draw_rectangle(0.0, 0.0, screen_w, 36.0, Color::new(0.05, 0.06, 0.08, 0.95));
            let header = hovered.unwrap_or_else(|| format!("Total: {}", format_bytes(tree.size)));
            draw_text(&header, 14.0, 24.0, 18.0, WHITE);
        } else {
            draw_text("Scanning filesystem...", 20.0, 40.0, 24.0, LIGHTGRAY);
        }

        next_frame().await;
    }
}