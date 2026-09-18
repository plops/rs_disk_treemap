use macroquad::prelude::*;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::thread;
struct Node {
    path: PathBuf,
    size: u64,
    is_dir: bool,
    children: Vec<Node>,
    rect: Rect,
    color: Color,
}
fn report_skip(context: &str, path: &Path, err: std::io::Error) {
    eprintln!("skip [{context}]: {}: {err:?}", path.display())
}
fn scan_tree(path: &Path) -> Node {
    {
        let mut node = Node {
            path: path.to_path_buf(),
            size: 0,
            is_dir: true,
            children: Vec::new(),
            rect: Rect::default(),
            color: Color::new(0.150, 0.170, 0.220, 1.0),
        };
        let s = path.to_string_lossy();
        if s.starts_with("/proc") || s.starts_with("/sys") || s.starts_with("/dev") {
            return node;
        }
        match fs::read_dir(path) {
            Ok(entries) => {
                for entry_res in entries {
                    match entry_res {
                        Ok(entry) => scan_entry(&mut node, entry),
                        Err(e) => report_skip("entry", path, e),
                    }
                }
            }
            Err(e) => report_skip("read_dir", path, e),
        }
        node
    }
}
fn scan_entry(node: &mut Node, entry: std::fs::DirEntry) {
    {
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(e) => {
                report_skip("file_type", &entry.path(), e);
                return;
            }
        };
        if ft.is_symlink() {
            return;
        }
        {
            let entry_path = entry.path();
            if ft.is_dir() {
                {
                    let child = scan_tree(&entry_path);
                    if 0 < child.size {
                        node.size += child.size;
                        node.children.push(child)
                    }
                }
            } else {
                if ft.is_file() {
                    {
                        let size = match entry.metadata() {
                            Ok(m) => m.len(),
                            Err(e) => {
                                report_skip("metadata", &entry_path, e);
                                0
                            }
                        };
                        if 0 < size && size < 1 << 48 {
                            node.children.push(Node {
                                color: color_for_path(&entry_path),
                                path: entry_path,
                                size,
                                is_dir: false,
                                children: Vec::new(),
                                rect: Rect::default(),
                            })
                        }
                    }
                }
            }
        }
    }
}
fn worst(areas: &[f64], row: &[usize], sum: f64, side: f32) -> f64 {
    {
        let (s2, sum2) = ((side * side) as f64, sum * sum);
        row.iter()
            .map(|i: &usize| ((s2 * areas[*i]) / sum2).max(sum2 / (s2 * areas[*i])))
            .fold(0.0, f64::max)
    }
}
fn layout_row(nodes: &mut [Node], areas: &[f64], row: &[usize], sum: f64, r: &mut Rect) {
    {
        let side = r.w.min(r.h);
        let thickness = (sum / side as f64) as f32;
        let is_horiz = r.w < r.h;
        {
            let mut offset = if is_horiz { r.x } else { r.y };
            for i in row {
                {
                    let item_len = ((areas[*i] / sum) * side as f64) as f32;
                    nodes[*i].rect = if is_horiz {
                        Rect::new(offset, r.y, item_len, thickness)
                    } else {
                        Rect::new(r.x, offset, thickness, item_len)
                    };
                    offset += item_len
                }
            }
            if is_horiz {
                {
                    r.y += thickness;
                    r.h -= thickness
                }
            } else {
                {
                    r.x += thickness;
                    r.w -= thickness
                }
            }
        }
    }
}
fn squarify(nodes: &mut [Node], mut rect: Rect) {
    {
        let total: u64 = nodes.iter().map(|n| n.size).sum();
        if total == 0 || rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        nodes.sort_unstable_by_key(|n| std::cmp::Reverse(n.size));
        {
            let area_mult = (rect.w * rect.h) as f64 / total as f64;
            {
                let areas: Vec<f64> = nodes.iter().map(|n| n.size as f64 * area_mult).collect();
                {
                    let mut row = Vec::new();
                    let mut row_sum = 0.0;
                    for i in 0..areas.len() {
                        {
                            let side = rect.w.min(rect.h);
                            let area = areas[i];
                            let mut next_row = row.clone();
                            next_row.push(i);
                            if row.is_empty()
                                || worst(&areas, &next_row, row_sum + area, side)
                                    <= worst(&areas, &row, row_sum, side)
                            {
                                {
                                    row.push(i);
                                    row_sum += area
                                }
                            } else {
                                {
                                    layout_row(nodes, &areas, &row, row_sum, &mut rect);
                                    row = vec![i];
                                    row_sum = area;
                                }
                            }
                        }
                    }
                    if !row.is_empty() {
                        layout_row(nodes, &areas, &row, row_sum, &mut rect)
                    }
                    for node in nodes.iter_mut() {
                        if node.is_dir && 4.0 < node.rect.w && 4.0 < node.rect.h {
                            squarify(&mut node.children, node.rect)
                        }
                    }
                }
            }
        }
    }
}
fn hash_color(path: &Path) -> Color {
    {
        let name = path.file_name().and_then(OsStr::to_str).unwrap_or("");
        let h = name
            .bytes()
            .fold(0, |acc: u32, b: u8| -> u32 { acc.wrapping_add(b as u32) });
        Color::from_rgba(
            ((h * 37) % 160 + 80) as u8,
            ((h * 59) % 160 + 80) as u8,
            ((h * 83) % 160 + 80) as u8,
            255,
        )
    }
}
fn color_for_path(path: &Path) -> Color {
    {
        let ext = path.extension().and_then(OsStr::to_str).unwrap_or("");
        if ext == "rs"
            || ext == "c"
            || ext == "cpp"
            || ext == "py"
            || ext == "js"
            || ext == "ts"
            || ext == "txt"
            || ext == "md"
        {
            Color::new(0.20, 0.750, 0.450, 1.0)
        } else {
            if ext == "png" || ext == "jpg" || ext == "jpeg" || ext == "svg" || ext == "webp" {
                Color::new(0.20, 0.650, 0.950, 1.0)
            } else {
                if ext == "mp4" || ext == "mkv" || ext == "mov" || ext == "mp3" || ext == "flac" {
                    Color::new(0.750, 0.350, 0.850, 1.0)
                } else {
                    if ext == "zip" || ext == "tar" || ext == "gz" || ext == "7z" {
                        Color::new(0.90, 0.30, 0.250, 1.0)
                    } else {
                        hash_color(path)
                    }
                }
            }
        }
    }
}
fn format_bytes(b: u64) -> String {
    {
        let units = ["B", "KB", "MB", "GB", "TB"];
        let mut d = b as f64;
        let mut i: usize = 0;
        while 1024.0 <= d && i < 4 {
            d /= 1024.0;
            i += 1
        }
        format!("{:.1} {}", d, units[i])
    }
}
#[test]
fn test_format_bytes() {
    assert_eq!(format_bytes(0), "0.0 B");
    assert_eq!(format_bytes(100), "100.0 B");
    assert_eq!(format_bytes(2048), "2.0 KB");
    assert_eq!(format_bytes(5242880), "5.0 MB")
}
fn render_tree(node: &Node, mouse: Vec2, hovered: &mut Option<String>) {
    if node.rect.w < 1.0 || node.rect.h < 1.0 {
        return;
    }
    if node.children.is_empty() {
        {
            draw_rectangle(
                node.rect.x,
                node.rect.y,
                node.rect.w,
                node.rect.h,
                node.color,
            );
            draw_rectangle_lines(
                node.rect.x,
                node.rect.y,
                node.rect.w,
                node.rect.h,
                1.0,
                Color::new(0.0, 0.0, 0.0, 0.350),
            )
        }
    } else {
        {
            for child in &node.children {
                render_tree(child, mouse, hovered)
            }
            draw_rectangle_lines(
                node.rect.x,
                node.rect.y,
                node.rect.w,
                node.rect.h,
                1.0,
                Color::new(0.0, 0.0, 0.0, 0.60),
            )
        }
    }
    if hovered.is_none() && node.rect.contains(mouse) {
        *hovered = Some(format!(
            "{} ({})",
            node.path.display(),
            format_bytes(node.size)
        ))
    }
}
#[macroquad::main("Treemap Disk Visualizer MVP")]
async fn main() {
    {
        let raw_target = std::env::args()
            .nth(1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let target = fs::canonicalize(&raw_target).unwrap_or(raw_target);
        let (tx, rx) = (channel::<Node>)();
        eprintln!("target [target.display()]={:?}", target.display());
        thread::spawn(move || {
            let root = scan_tree(&target);
            let _ = tx.send(root);
        });
        {
            let mut root: Option<Node> = None;
            let mut last_size = (0.0, 0.0);
            loop {
                clear_background(Color::new(8.00e-2, 9.00e-2, 0.120, 1.0));
                {
                    let (screen_w, screen_h) = (screen_width(), screen_height());
                    if let Ok(loaded_root) = rx.try_recv() {
                        {
                            root = Some(loaded_root);
                            last_size = (0.0, 0.0);
                        }
                    }
                    if let Some(tree) = &mut root {
                        {
                            {
                                let mut avail_h = screen_h - 36.0;
                                if avail_h < 1.0 {
                                    avail_h = 1.0;
                                }
                                {
                                    let canvas = Rect::new(0.0, 36.0, screen_w, avail_h);
                                    if (screen_w, screen_h) != last_size {
                                        tree.rect = canvas;
                                        squarify(&mut tree.children, canvas);
                                        last_size = (screen_w, screen_h);
                                        eprintln!(
                                            "layout [screen_w]={:?} [screen_h]={:?} [tree.size]={:?}",
                                            screen_w, screen_h, tree.size
                                        )
                                    }
                                    {
                                        let mut hovered: Option<String> = None;
                                        render_tree(
                                            tree,
                                            Vec2::from(mouse_position()),
                                            &mut hovered,
                                        );
                                        draw_rectangle(
                                            0.0,
                                            0.0,
                                            screen_w,
                                            36.0,
                                            Color::new(5.00e-2, 6.00e-2, 8.00e-2, 0.950),
                                        );
                                        {
                                            let header = hovered.unwrap_or_else(|| {
                                                format!(
                                                    "{}: {}",
                                                    tree.path.display(),
                                                    format_bytes(tree.size)
                                                )
                                            });
                                            draw_text(&header, 14.0, 24.0, 16.0, WHITE);
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        draw_text("Scanning filesystem...", 20.0, 40.0, 24.0, LIGHTGRAY);
                    }
                    next_frame().await
                }
            }
        }
    }
}
