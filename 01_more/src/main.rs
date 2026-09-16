use macroquad::prelude::*;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use rayon::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::ffi::OsStr;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

// ============================================================================
// 1. Data Structures (Memory Optimized: No PathBuf per leaf)
// ============================================================================

pub struct FileNode {
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub children: Vec<FileNode>,
    pub color: Color,

    // Layout targets & animated interpolation
    pub target_rect: Rect,
    pub current_rect: Rect,

    // Real-time animation & write tracking
    pub growth_pulse: f32, // 1.0 -> 0.0 visual decay
    pub growth_rate: f64,  // Bytes / sec
    pub last_size: u64,
    pub last_change_time: Instant,
}

impl FileNode {
    pub fn new(name: String, is_dir: bool, size_bytes: u64) -> Self {
        let color = if is_dir {
            Color::new(0.14, 0.16, 0.20, 1.0)
        } else {
            get_color_for_filename(&name)
        };

        Self {
            name,
            is_dir,
            size_bytes,
            children: Vec::new(),
            color,
            target_rect: Rect::default(),
            current_rect: Rect::default(),
            growth_pulse: 0.0,
            growth_rate: 0.0,
            last_size: size_bytes,
            last_change_time: Instant::now(),
        }
    }

    /// Recursively find a child node matching relative path components
    pub fn find_mut(&mut self, components: &[&OsStr]) -> Option<&mut FileNode> {
        if components.is_empty() {
            return Some(self);
        }
        let target = components[0].to_string_lossy();
        for child in &mut self.children {
            if child.name == target {
                return child.find_mut(&components[1..]);
            }
        }
        None
    }

    /// Update file size on inotify writes, compute write rate, bubble size delta upward
    pub fn update_file_size(
        &mut self,
        components: &[&OsStr],
        new_size: u64,
        now: Instant,
    ) -> Option<i64> {
        if components.is_empty() {
            let old_size = self.size_bytes;
            let diff = new_size as i64 - old_size as i64;
            let dt = (now - self.last_change_time).as_secs_f64().max(0.01);

            if diff > 0 {
                self.growth_pulse = 1.0;
                self.growth_rate = (diff as f64) / dt;
            }

            self.size_bytes = new_size;
            self.last_size = old_size;
            self.last_change_time = now;
            return Some(diff);
        }

        let target = components[0].to_string_lossy();
        for child in &mut self.children {
            if child.name == target {
                if let Some(diff) = child.update_file_size(&components[1..], new_size, now) {
                    self.size_bytes = (self.size_bytes as i64 + diff).max(0) as u64;
                    if diff > 0 {
                        self.growth_pulse = (self.growth_pulse + 0.3).min(1.0);
                    }
                    return Some(diff);
                }
                break;
            }
        }
        None
    }
}

// ============================================================================
// 2. Parallel Squarified Treemap Layout (Rayon + Zero-Allocation Hot Loop)
// ============================================================================

fn worst_aspect_ratio(
    row: &[usize],
    extra: Option<usize>,
    areas: &[f64],
    row_sum: f64,
    side: f32,
) -> f64 {
    if row_sum <= 0.0 || side <= 0.0 {
        return f64::INFINITY;
    }
    let s2 = (side * side) as f64;
    let sum2 = row_sum * row_sum;

    row.iter()
        .copied()
        .chain(extra)
        .map(|idx| {
            let a = areas[idx];
            if a <= 0.0 {
                0.0
            } else {
                (s2 * a / sum2).max(sum2 / (s2 * a))
            }
        })
        .fold(0.0, f64::max)
}

fn layout_row(
    children: &mut [FileNode],
    row: &[usize],
    areas: &[f64],
    row_sum: f64,
    rect: &mut Rect,
    is_last: bool,
) {
    if row.is_empty() {
        return;
    }

    if rect.w <= 0.0 || rect.h <= 0.0 || row_sum <= 0.0 {
        for &idx in row {
            children[idx].target_rect = Rect::new(rect.x, rect.y, 0.0, 0.0);
        }
        return;
    }

    if rect.w >= rect.h {
        let mut row_thickness = if is_last {
            rect.w
        } else {
            (row_sum / rect.h as f64) as f32
        };
        row_thickness = row_thickness.clamp(0.0, rect.w);

        let mut current_y = rect.y;
        for (i, &idx) in row.iter().enumerate() {
            let item_h = if i == row.len() - 1 {
                (rect.y + rect.h) - current_y
            } else {
                ((areas[idx] / row_sum) * rect.h as f64) as f32
            }
            .max(0.0);

            children[idx].target_rect = Rect::new(rect.x, current_y, row_thickness, item_h);
            current_y += item_h;
        }

        rect.x += row_thickness;
        rect.w = (rect.w - row_thickness).max(0.0);
    } else {
        let mut row_thickness = if is_last {
            rect.h
        } else {
            (row_sum / rect.w as f64) as f32
        };
        row_thickness = row_thickness.clamp(0.0, rect.h);

        let mut current_x = rect.x;
        for (i, &idx) in row.iter().enumerate() {
            let item_w = if i == row.len() - 1 {
                (rect.x + rect.w) - current_x
            } else {
                ((areas[idx] / row_sum) * rect.w as f64) as f32
            }
            .max(0.0);

            children[idx].target_rect = Rect::new(current_x, rect.y, item_w, row_thickness);
            current_x += item_w;
        }

        rect.y += row_thickness;
        rect.h = (rect.h - row_thickness).max(0.0);
    }
}

fn squarify_children(node: &mut FileNode) {
    if node.children.is_empty() || node.size_bytes == 0 {
        return;
    }

    node.children
        .sort_unstable_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

    let total_bytes: u64 = node.children.iter().map(|c| c.size_bytes).sum();
    if total_bytes == 0 {
        return;
    }

    let total_area = (node.target_rect.w * node.target_rect.h) as f64;
    let areas: Vec<f64> = node
        .children
        .iter()
        .map(|c| (c.size_bytes as f64 / total_bytes as f64) * total_area)
        .collect();

    let mut remaining_rect = node.target_rect;
    let mut row: Vec<usize> = Vec::new();
    let mut row_sum = 0.0;

    for i in 0..node.children.len() {
        if areas[i] <= 0.0 {
            node.children[i].target_rect = Rect::new(remaining_rect.x, remaining_rect.y, 0.0, 0.0);
            continue;
        }

        let side = remaining_rect.w.min(remaining_rect.h);
        let worst_with = worst_aspect_ratio(&row, Some(i), &areas, row_sum + areas[i], side);
        let worst_without = worst_aspect_ratio(&row, None, &areas, row_sum, side);

        if row.is_empty() || worst_with <= worst_without {
            row.push(i);
            row_sum += areas[i];
        } else {
            layout_row(
                &mut node.children,
                &row,
                &areas,
                row_sum,
                &mut remaining_rect,
                false,
            );
            row.clear();
            row.push(i);
            row_sum = areas[i];

            if remaining_rect.w <= 0.0 || remaining_rect.h <= 0.0 {
                for j in (i + 1)..node.children.len() {
                    node.children[j].target_rect =
                        Rect::new(remaining_rect.x, remaining_rect.y, 0.0, 0.0);
                }
                break;
            }
        }
    }

    if !row.is_empty() {
        layout_row(
            &mut node.children,
            &row,
            &areas,
            row_sum,
            &mut remaining_rect,
            true,
        );
    }
}

/// Recursive parallel layout using Rayon + 1px Visual Inset for folder hierarchy
pub fn layout_treemap_parallel(node: &mut FileNode) {
    if node.children.is_empty() || node.size_bytes == 0 {
        return;
    }

    squarify_children(node);

    // Parallel recursive descent with 1px container padding (Visual Nesting)
    let eligible_subdirs: Vec<&mut FileNode> = node
        .children
        .iter_mut()
        .filter(|c| {
            c.is_dir && !c.children.is_empty() && c.target_rect.w >= 6.0 && c.target_rect.h >= 6.0
        })
        .collect();

    eligible_subdirs.into_par_iter().for_each(|child| {
        child.target_rect = Rect::new(
            child.target_rect.x + 1.0,
            child.target_rect.y + 1.0,
            (child.target_rect.w - 2.0).max(1.0),
            (child.target_rect.h - 2.0).max(1.0),
        );
        layout_treemap_parallel(child);
    });
}

// ============================================================================
// 3. Robust Filesystem Scanner
// ============================================================================

pub struct DirectoryBatch {
    pub parent_path: PathBuf,
    pub children: Vec<FileNode>,
}

pub enum ScanEvent {
    Batch(DirectoryBatch),
    Finished,
}

fn is_virtual_or_special_fs(path: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        let s = path.to_string_lossy();
        if s.starts_with("/proc")
            || s.starts_with("/sys")
            || s.starts_with("/dev")
            || s.starts_with("/run")
        {
            return true;
        }
    }
    false
}

fn scan_directory_recursive(
    dir_path: &Path,
    tx: &Sender<ScanEvent>,
    abort: &Arc<AtomicBool>,
    total_files: &Arc<AtomicU64>,
    total_bytes: &Arc<AtomicU64>,
) {
    if abort.load(Ordering::Relaxed) || is_virtual_or_special_fs(dir_path) {
        return;
    }

    let entries = match fs::read_dir(dir_path) {
        Ok(iter) => iter,
        Err(_) => return, // Permission denied or inaccessible
    };

    let mut scanned_children = Vec::new();
    let mut subdirs_to_recurse = Vec::new();

    for entry in entries.flatten() {
        if abort.load(Ordering::Relaxed) {
            return;
        }

        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        // Skip symlinks to prevent circular loops
        if file_type.is_symlink() {
            continue;
        }

        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        if file_type.is_dir() {
            scanned_children.push(FileNode::new(name, true, 0));
            subdirs_to_recurse.push(path);
        } else if file_type.is_file() {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if size > (1 << 48) {
                continue; // Ignore virtual pseudo-files (e.g., /proc/kcore)
            }

            total_files.fetch_add(1, Ordering::Relaxed);
            total_bytes.fetch_add(size, Ordering::Relaxed);

            scanned_children.push(FileNode::new(name, false, size));
        }
    }

    let _ = tx.send(ScanEvent::Batch(DirectoryBatch {
        parent_path: dir_path.to_path_buf(),
        children: scanned_children,
    }));

    for subdir in subdirs_to_recurse {
        scan_directory_recursive(&subdir, tx, abort, total_files, total_bytes);
    }
}

// ============================================================================
// 4. Live Filesystem Watcher (inotify / notify bridge)
// ============================================================================

pub struct FileChangeEvent {
    pub path: PathBuf,
    pub is_removal: bool,
}

fn start_fs_watcher(
    root_path: PathBuf,
    tx: Sender<FileChangeEvent>,
) -> Option<notify::RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        if let Ok(event) = res {
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) => {
                    for path in event.paths {
                        let _ = tx.send(FileChangeEvent {
                            path,
                            is_removal: false,
                        });
                    }
                }
                EventKind::Remove(_) => {
                    for path in event.paths {
                        let _ = tx.send(FileChangeEvent {
                            path,
                            is_removal: true,
                        });
                    }
                }
                _ => {}
            }
        }
    })
    .ok()?;

    let _ = watcher.watch(&root_path, RecursiveMode::Recursive);
    Some(watcher)
}

// ============================================================================
// 5. Rendering, Spring Animation & Non-Overwriting Hover Detection
// ============================================================================

fn update_animations(node: &mut FileNode, dt: f32) {
    let t = 1.0 - (-14.0 * dt).exp(); // Exponential smoothing

    node.current_rect.x += (node.target_rect.x - node.current_rect.x) * t;
    node.current_rect.y += (node.target_rect.y - node.current_rect.y) * t;
    node.current_rect.w += (node.target_rect.w - node.current_rect.w) * t;
    node.current_rect.h += (node.target_rect.h - node.current_rect.h) * t;

    if node.growth_pulse > 0.0 {
        node.growth_pulse = (node.growth_pulse - dt * 1.5).max(0.0);
    }

    for child in &mut node.children {
        update_animations(child, dt);
    }
}

fn draw_cushion_rect(rect: Rect, base_color: Color, pulse: f32) {
    let fill_color = if pulse > 0.0 {
        let pulse_color = Color::new(0.3, 1.0, 0.4, 1.0); // Neon pulse highlight
        Color::new(
            base_color.r * (1.0 - pulse) + pulse_color.r * pulse,
            base_color.g * (1.0 - pulse) + pulse_color.g * pulse,
            base_color.b * (1.0 - pulse) + pulse_color.b * pulse,
            1.0,
        )
    } else {
        base_color
    };

    draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill_color);

    if rect.w >= 4.0 && rect.h >= 4.0 {
        let bevel = (rect.w.min(rect.h) * 0.12).clamp(1.0, 5.0);
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            bevel,
            Color::new(1.0, 1.0, 1.0, 0.22),
        );
        draw_rectangle(
            rect.x,
            rect.y,
            bevel,
            rect.h,
            Color::new(1.0, 1.0, 1.0, 0.22),
        );
        draw_rectangle(
            rect.x,
            rect.y + rect.h - bevel,
            rect.w,
            bevel,
            Color::new(0.0, 0.0, 0.0, 0.35),
        );
        draw_rectangle(
            rect.x + rect.w - bevel,
            rect.y,
            bevel,
            rect.h,
            Color::new(0.0, 0.0, 0.0, 0.35),
        );
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            1.0,
            Color::new(0.05, 0.05, 0.08, 0.5),
        );
    }

    if pulse > 0.05 {
        draw_rectangle_lines(
            rect.x - 1.0,
            rect.y - 1.0,
            rect.w + 2.0,
            rect.h + 2.0,
            2.0,
            Color::new(0.4, 1.0, 0.5, pulse * 0.8),
        );
    }
}

/// Render with LOD culling to avoid draw-call explosion on huge directory trees
fn render_treemap(
    node: &FileNode,
    view_min: Vec2,
    view_max: Vec2,
    camera_pos: Vec2,
    camera_zoom: f32,
) {
    let r = node.current_rect;
    // Frustum Culling
    if r.x > view_max.x || (r.x + r.w) < view_min.x || r.y > view_max.y || (r.y + r.h) < view_min.y
    {
        return;
    }

    let screen_x = (r.x + camera_pos.x) * camera_zoom;
    let screen_y = (r.y + camera_pos.y) * camera_zoom;
    let screen_w = r.w * camera_zoom;
    let screen_h = r.h * camera_zoom;

    // Subpixel Culling
    if screen_w < 1.0 || screen_h < 1.0 {
        return;
    }

    let screen_rect = Rect::new(screen_x, screen_y, screen_w, screen_h);

    if node.children.is_empty() {
        draw_cushion_rect(screen_rect, node.color, node.growth_pulse);
    } else {
        // LOD Threshold: If folder rectangle is < 4x4 screen pixels, draw a flat placeholder and skip traversing children
        if screen_w < 4.0 || screen_h < 4.0 {
            draw_rectangle(
                screen_x,
                screen_y,
                screen_w,
                screen_h,
                Color::new(0.16, 0.18, 0.23, 1.0),
            );
            return;
        }

        for child in &node.children {
            render_treemap(child, view_min, view_max, camera_pos, camera_zoom);
        }
        draw_rectangle_lines(
            screen_x,
            screen_y,
            screen_w,
            screen_h,
            1.0,
            Color::new(0.0, 0.0, 0.0, 0.5),
        );
    }

    // Dynamic Label Rendering
    if screen_w > 65.0 && screen_h > 20.0 {
        let label = if node.growth_rate > 1024.0 {
            format!(
                "{} [+{}/s]",
                node.name,
                format_bytes(node.growth_rate as u64)
            )
        } else {
            node.name.clone()
        };
        draw_text(&label, screen_x + 5.0, screen_y + 14.0, 14.0, BLACK);
        draw_text(&label, screen_x + 4.0, screen_y + 13.0, 14.0, WHITE);
    }
}

/// Dedicated hover detection: Searches children first so leaves always take precedence over parents
fn find_hovered<'a>(
    node: &'a FileNode,
    pt: Vec2,
    path_acc: String,
) -> Option<(&'a FileNode, String)> {
    if !node.current_rect.contains(pt) {
        return None;
    }

    let current_path = if path_acc.is_empty() {
        node.name.clone()
    } else {
        format!("{}/{}", path_acc, node.name)
    };

    // Deepest child match wins
    for child in &node.children {
        if let Some(hit) = find_hovered(child, pt, current_path.clone()) {
            return Some(hit);
        }
    }

    Some((node, current_path))
}

// ============================================================================
// 6. Color Generator & Formatting
// ============================================================================

fn get_color_for_filename(name: &str) -> Color {
    let ext = Path::new(name)
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "mp4" | "mkv" | "avi" | "mov" => Color::new(0.72, 0.23, 0.86, 1.0),
        "mp3" | "flac" | "wav" | "ogg" => Color::new(0.94, 0.80, 0.14, 1.0),
        "png" | "jpg" | "jpeg" | "webp" | "gif" => Color::new(0.12, 0.74, 0.90, 1.0),
        "zip" | "rar" | "7z" | "tar" | "gz" => Color::new(0.92, 0.20, 0.20, 1.0),
        "rs" | "cpp" | "c" | "h" | "py" | "js" | "ts" | "txt" | "md" => {
            Color::new(0.16, 0.82, 0.43, 1.0)
        }
        "exe" | "dll" | "so" | "bin" => Color::new(0.24, 0.39, 0.94, 1.0),
        _ => {
            let mut hasher = DefaultHasher::new();
            name.hash(&mut hasher);
            let h = hasher.finish();
            Color::from_rgba(
                ((h >> 16) % 160 + 80) as u8,
                ((h >> 8) % 160 + 80) as u8,
                (h % 160 + 80) as u8,
                255,
            )
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    const SUFFIXES: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut d = bytes as f64;
    let mut i = 0;
    while d >= 1024.0 && i < 4 {
        d /= 1024.0;
        i += 1;
    }
    format!("{:.2} {}", d, SUFFIXES[i])
}

// ============================================================================
// 7. Main Event Loop
// ============================================================================

fn window_conf() -> Conf {
    Conf {
        window_title: "Treemap Disk Visualizer".to_string(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let raw_target = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    // Canonicalize to guarantee valid absolute paths on all platforms
    let target_dir = fs::canonicalize(&raw_target).unwrap_or(raw_target);
    let root_name = target_dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| target_dir.to_string_lossy().into_owned());

    let mut root_node = FileNode::new(root_name, true, 0);

    // Thread Communication & Atomies
    let total_files = Arc::new(AtomicU64::new(0));
    let total_bytes = Arc::new(AtomicU64::new(0));
    let is_scanning = Arc::new(AtomicBool::new(true));
    let abort_scan = Arc::new(AtomicBool::new(false));

    let (scan_tx, scan_rx): (Sender<ScanEvent>, Receiver<ScanEvent>) = channel();
    let (watch_tx, watch_rx): (Sender<FileChangeEvent>, Receiver<FileChangeEvent>) = channel();

    // Start background scanner thread
    {
        let target_dir = target_dir.clone();
        let abort = abort_scan.clone();
        let files = total_files.clone();
        let bytes = total_bytes.clone();
        let is_scanning = is_scanning.clone();

        thread::spawn(move || {
            scan_directory_recursive(&target_dir, &scan_tx, &abort, &files, &bytes);
            let _ = scan_tx.send(ScanEvent::Finished);
            is_scanning.store(false, Ordering::Release);
        });
    }

    // Start inotify file watcher
    let _watcher = start_fs_watcher(target_dir.clone(), watch_tx);

    let mut camera_pos = Vec2::ZERO;
    let mut camera_zoom = 1.0f32;
    let mut last_mouse = Vec2::ZERO;
    let mut layout_dirty = true;

    loop {
        let dt = get_frame_time().min(0.05);
        let screen_w = screen_width();
        let screen_h = screen_height();
        // Safe canvas computation (prevents negative dimensions when window is minimized)
        let world_canvas = Rect::new(0.0, 44.0, screen_w, (screen_h - 44.0).max(1.0));

        // 1. Drain progressive scan batches from worker thread
        while let Ok(event) = scan_rx.try_recv() {
            match event {
                ScanEvent::Batch(batch) => {
                    if let Ok(rel) = batch.parent_path.strip_prefix(&target_dir) {
                        let components: Vec<&OsStr> = rel.iter().collect();
                        if let Some(parent) = root_node.find_mut(&components) {
                            for mut child in batch.children {
                                child.target_rect = parent.target_rect;
                                child.current_rect = parent.current_rect;
                                parent.children.push(child);
                            }
                            layout_dirty = true;
                        }
                    }
                }
                ScanEvent::Finished => {
                    layout_dirty = true;
                }
            }
        }

        // 2. Drain live inotify file changes
        let now = Instant::now();
        while let Ok(change) = watch_rx.try_recv() {
            if let Ok(rel) = change.path.strip_prefix(&target_dir) {
                let components: Vec<&OsStr> = rel.iter().collect();
                if change.is_removal {
                    if let Some(parent_comps) = components.split_last().map(|(_, p)| p) {
                        if let Some(parent) = root_node.find_mut(parent_comps) {
                            if let Some(file_name) = components.last() {
                                let name = file_name.to_string_lossy();
                                if let Some(pos) =
                                    parent.children.iter().position(|c| c.name == name)
                                {
                                    let removed = parent.children.remove(pos);
                                    total_files.fetch_sub(1, Ordering::Relaxed);
                                    total_bytes.fetch_sub(removed.size_bytes, Ordering::Relaxed);
                                    layout_dirty = true;
                                }
                            }
                        }
                    }
                } else if let Ok(meta) = fs::metadata(&change.path) {
                    if meta.is_file() {
                        let new_size = meta.len();
                        if let Some(diff) = root_node.update_file_size(&components, new_size, now) {
                            if diff != 0 {
                                if diff > 0 {
                                    total_bytes.fetch_add(diff as u64, Ordering::Relaxed);
                                } else {
                                    total_bytes.fetch_sub((-diff) as u64, Ordering::Relaxed);
                                }
                                layout_dirty = true;
                            }
                        }
                    }
                }
            }
        }

        // 3. Parallel Treemap Recalculation (Rayon)
        if layout_dirty {
            root_node.target_rect = world_canvas;

            fn sum_tree_bytes(node: &mut FileNode) -> u64 {
                if !node.is_dir {
                    return node.size_bytes;
                }
                let s: u64 = node.children.iter_mut().map(sum_tree_bytes).sum();
                node.size_bytes = s;
                s
            }
            root_node.size_bytes = sum_tree_bytes(&mut root_node);

            layout_treemap_parallel(&mut root_node);
            layout_dirty = false;
        }

        // 4. Update spring lerps & animations
        update_animations(&mut root_node, dt);

        // 5. Input Handling: Pan, Zoom, Reset
        let mouse_screen = Vec2::from(mouse_position());

        if is_mouse_button_down(MouseButton::Left) || is_mouse_button_down(MouseButton::Middle) {
            let delta = (mouse_screen - last_mouse) / camera_zoom;
            camera_pos += delta;
        }
        last_mouse = mouse_screen;

        let wheel = mouse_wheel().1;
        if wheel.abs() > 0.01 {
            let mouse_world_before = (mouse_screen / camera_zoom) - camera_pos;
            if wheel > 0.0 {
                camera_zoom *= 1.15;
            } else {
                camera_zoom /= 1.15;
            }
            camera_zoom = camera_zoom.clamp(0.05, 100.0);
            let mouse_world_after = (mouse_screen / camera_zoom) - camera_pos;
            camera_pos += mouse_world_after - mouse_world_before;
        }

        if is_key_pressed(KeyCode::Space) {
            camera_pos = Vec2::ZERO;
            camera_zoom = 1.0;
        }

        // 6. Draw Treemap Scene
        clear_background(Color::new(0.08, 0.09, 0.12, 1.0));

        let view_min = -camera_pos;
        let view_max = (Vec2::new(screen_w, screen_h) / camera_zoom) - camera_pos;

        render_treemap(&root_node, view_min, view_max, camera_pos, camera_zoom);

        // 7. Non-Overwriting Hover Detection
        let mouse_world = (mouse_screen / camera_zoom) - camera_pos;
        let hovered_info = find_hovered(&root_node, mouse_world, String::new());

        // 8. Draw Status Bar & HUD
        draw_rectangle(0.0, 0.0, screen_w, 44.0, Color::new(0.06, 0.07, 0.09, 0.95));
        draw_line(
            0.0,
            44.0,
            screen_w,
            44.0,
            1.0,
            Color::new(0.15, 0.17, 0.22, 1.0),
        );

        let scanning = is_scanning.load(Ordering::Relaxed);
        let status_color = if scanning {
            Color::new(0.95, 0.8, 0.1, 1.0)
        } else {
            Color::new(0.2, 0.85, 0.3, 1.0)
        };
        let status_text = if scanning {
            "SCANNING..."
        } else {
            "WATCHING (INOTIFY)"
        };

        draw_text(status_text, 14.0, 18.0, 16.0, status_color);
        draw_text(
            &format!("Files: {}", total_files.load(Ordering::Relaxed)),
            220.0,
            18.0,
            16.0,
            WHITE,
        );
        draw_text(
            &format!(
                "Size: {}",
                format_bytes(total_bytes.load(Ordering::Relaxed))
            ),
            360.0,
            18.0,
            16.0,
            Color::new(0.2, 0.8, 1.0, 1.0),
        );

        if let Some((hovered_node, path_str)) = hovered_info {
            let mut info = format!("{} ({})", path_str, format_bytes(hovered_node.size_bytes));
            if hovered_node.growth_rate > 1024.0 {
                info.push_str(&format!(
                    "  [Active: {}/s]",
                    format_bytes(hovered_node.growth_rate as u64)
                ));
            }
            draw_text(&info, 14.0, 36.0, 14.0, Color::new(0.9, 0.9, 0.9, 1.0));
        } else {
            draw_text(
                "Pan: Left/Middle Drag | Zoom: Wheel | Reset: Space | Target: ",
                14.0,
                36.0,
                14.0,
                GRAY,
            );
            draw_text(&target_dir.to_string_lossy(), 420.0, 36.0, 14.0, LIGHTGRAY);
        }

        next_frame().await;
    }
}
