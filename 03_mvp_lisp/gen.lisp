;; gen.lisp --- loader for the transpiled treemap MVP.
;;
;; Run from the repo root:
;;   sbcl --load 03_mvp_lisp/gen.lisp --quit
;; Writes:
;;   03_mvp_lisp/src/main.rs   (committed generated code)
;;
;; Pattern: cl-rust-generator/examples/21_mandelbrot/gen00.lisp
;; Language reference: cl-rust-generator/SUPPORTED_FORMS.md
(eval-when (:compile-toplevel :execute :load-toplevel)
  (ql:register-local-projects)
  (ql:quickload "cl-rust-generator"))
(in-package :cl-rust-generator)
(setf *rustfmt-arguments* '("--edition" "2024"))
(defparameter *project-dir*
  (make-pathname
   :name nil
   :type nil
   :version nil
   :defaults (merge-pathnames #P"./" *load-pathname*)))
(defparameter *code-file*
  (merge-pathnames #P"src/main.rs" *project-dir*))
;; lprint: C++ model (gen-cpp-freestanding-example.lisp) in Rust shape.
;; Every variable shows up as [label]={:?} in exactly one eprintln! call.
;; A var is an expression, or (:as "label" expression) for a custom label.
(defun lprint (&key (msg "") (vars nil))
  (let ((entries (mapcar (lambda (v)
                           (if (and (consp v) (eq (car v) :as))
			       (list (second v) (third v))
			       (list (emit-rs :code v) v)))
                         vars)))
    `(eprintln!
      (string
       ,(format
         nil
         "~a~{[~a]={:?}~^ ~}"
         (if (string= msg "")
             ""
             (format nil "~a " msg))
         (mapcar #'first entries)))
      ,@(mapcar #'second entries))))


;; (ext-is ext (e1 e2 ...)) => (or (== ext "e1") (== ext "e2") ...)
(defun ext-is (ext-var exts)
  `(or
    ,@(loop for e in exts
            collect `(== ,ext-var (string ,e)))))

(let ((*omit-redundant-parens* t))
  (write-source
   *code-file*
   `(do0
     (use
      (macroquad prelude *)
      (std ffi OsStr)
      (std fs)
      (std path (curly Path PathBuf))
      (std sync mpsc channel)
      (std thread))
     (defstruct0 Node
         (path PathBuf)
       (size u64)
       (is_dir bool)
       (children "Vec<Node>")
       (rect Rect)
       (color Color))
     ;; report-skip-fn: exactly one Rust helper fn for all scan error sites.
     (defun report_skip (&context &path err)
       (declare
	(type str &context)
	(type Path &path)
	(type "std::io::Error" err)
	)
       (eprintln!
	(string "skip [{context}]: {}: {err:?}")
	(dot path (display))
	)
       )
     
     (defun scan_tree (&path)
       (declare (type Path &path) (values Node))
       (let ((node (make-instance Node
				  :path (dot path (to_path_buf)) :size 0 :is_dir true
				  :children (Vec--new) :rect (Rect--default)
				  :color (Color--new 0.15s0 0.17s0 0.22s0 1.0s0))
		   )
             (s (dot path (to_string_lossy)))
             )
	 (declare (mutable node))
	 (when (or (dot s (starts_with (string "/proc")))
		   (dot s (starts_with (string "/sys")))
		   (dot s (starts_with (string "/dev"))))
	   (return node))
	 (case (fs--read_dir path)
	   ((Ok entries)
	    (for (entry_res entries)
		 (case entry_res
		   ((Ok entry) (scan_entry (ref-mut node) entry))
		   ((Err e) (report_skip (string "entry") path e)))))
	   ((Err e) (report_skip (string "read_dir") path e)))
	 node)
       )

     (defun scan_entry (&node entry)
       (declare (type Node &node) (type "std::fs::DirEntry" entry) (mutable &node))
       (let ((ft (case (dot entry (file_type))
		   ((Ok ft) ft)
		   ((Err e)
                    (progn
                      (report_skip (string "file_type") (ref (dot entry (path))) e)
                      (return))))))
	 (when (dot ft (is_symlink)) (return))
	 (let ((entry_path (dot entry (path))))
	   (if (dot ft (is_dir))
               (let ((child (scan_tree (ref entry_path))))
		 (when (< 0 (dot child size))
		   (incf (dot node size) (dot child size))
		   (dot node children (push child))))
               (when (dot ft (is_file))
		 (let ((size (case (dot entry (metadata))
                               ((Ok m) (dot m (len)))
                               ((Err e)
				(progn
				  (report_skip (string "metadata") (ref entry_path) e)
				  0)))))
		   (when (and (< 0 size) (< size (<< 1 48)))
                     (dot node children
			  (push (make-instance Node
					       :color (color_for_path (ref entry_path))
					       :path entry_path size :is_dir false
					       :children (Vec--new) :rect (Rect--default)))))))))))


     (defun emit-worst ()
       `(defun worst (areas row sum side)
	  (declare (type "&[f64]" areas) (type "&[usize]" row)
		   (type f64 sum) (type f32 side) (values f64))
	  (let (((tuple s2 sum2) (tuple (coerce (* side side) f64) (* sum sum))))
	    (dot row (iter)
                 (map (lambda (i)
                        (declare (type "&usize" i))
                        (dot (/ (* s2 (aref areas (deref i))) sum2)
                             (max (/ sum2 (* s2 (aref areas (deref i))))))))
                 (fold 0.0 (scope f64 max)))))
       )
     (defun layout_row (nodes areas row sum r)
       (declare (type "&mut [Node]" nodes) (type "&[f64]" areas)
		(type "&[usize]" row) (type f64 sum) (type "&mut Rect" r))
       (let ((side (dot (dot r w) (min (dot r h))))
             (thickness (coerce (/ sum (coerce side f64)) f32))
             (is_horiz (< (dot r w) (dot r h))))
	 (let ((offset (if is_horiz (dot r x) (dot r y))))
	   (declare (mutable offset))
	   (for (i row)
		(let ((item_len (coerce (* (/ (aref areas (deref i)) sum) (coerce side f64)) f32)))
		  (= (dot (aref nodes (deref i)) rect)
                     (if is_horiz
			 (Rect--new offset (dot r y) item_len thickness)
			 (Rect--new (dot r x) offset thickness item_len)))
		  (incf offset item_len)))
	   (if is_horiz
               (progn (incf (dot r y) thickness) (decf (dot r h) thickness))
               (progn (incf (dot r x) thickness) (decf (dot r w) thickness))))))


     (defun squarify (&nodes rect)
       (declare (type "[Node]" &nodes) (type Rect rect) (mutable &nodes rect))
       (let ((total (dot nodes (iter) (map (lambda (n) (dot n size))) (sum))))
	 (declare (type u64 total))
	 (when (or (== total 0) (<= (dot rect w) 0.0) (<= (dot rect h) 0.0))
	   (return))
	 (dot nodes (sort_unstable_by_key
                     (lambda (n) (std--cmp--Reverse (dot n size)))))
	 (let ((area_mult (/ (coerce (* (dot rect w) (dot rect h)) f64)
                             (coerce total f64))))
	   (let ((areas (dot nodes (iter)
                             (map (lambda (n)
				    (* (coerce (dot n size) f64) area_mult)))
                             (collect))))
             (declare (type "Vec<f64>" areas))
             (let ((row (Vec--new)) (row_sum 0.0))
               (declare (mutable row row_sum))
               (for (i (range 0 (dot areas (len))))
		    (let ((side (dot rect w (min (dot rect h))))
			  (area (aref areas i))
			  (next_row (dot row (clone))))
                      (declare (mutable next_row))
                      (dot next_row (push i))
                      (if (or (dot row (is_empty))
                              (<= (worst (ref areas) (ref next_row) (+ row_sum area) side)
				  (worst (ref areas) (ref row) row_sum side)))
			  (progn (dot row (push i)) (incf row_sum area))
			  (progn
			    (layout_row nodes (ref areas) (ref row) row_sum (ref-mut rect))
			    (setf row (vec! i)) (setf row_sum area)))))
               (when (not (dot row (is_empty)))
		 (layout_row nodes (ref areas) (ref row) row_sum (ref-mut rect)))
               (for (node (dot nodes (iter_mut)))
		    (when (and (dot node is_dir)
                               (< 4.0 (dot (dot node rect) w))
                               (< 4.0 (dot (dot node rect) h)))
                      (squarify (ref-mut (dot node children)) (dot node rect)))))))))

     (defun emit-hash-color ()
       `(defun hash_color (&path)
	  (declare (type Path &path) (values Color))
	  (let ((name (dot path (file_name)
                           (and_then (scope OsStr to_str))
                           (unwrap_or (string ""))))
		(h (dot name (bytes)
			(fold 0 (lambda (acc b)
				  (declare (type u32 acc) (type u8 b) (values u32))
				  (dot acc (wrapping_add (coerce b u32))))))))
	    (Color--from_rgba (coerce (+ (% (* h 37) 160) 80) u8)
                              (coerce (+ (% (* h 59) 160) 80) u8)
                              (coerce (+ (% (* h 83) 160) 80) u8)
                              255)))
       )
     (defun color_for_path (&path)
       (declare (type Path &path) (values Color))
       (let ((ext (dot path (extension)
                       (and_then (scope OsStr to_str))
                       (unwrap_or (string "")))))
	 (if ,(ext-is 'ext '("rs" "c" "cpp" "py" "js" "ts" "txt" "md"))
             (Color--new 0.2s0 0.75s0 0.45s0 1.0s0)
             (if ,(ext-is 'ext '("png" "jpg" "jpeg" "svg" "webp"))
		 (Color--new 0.2s0 0.65s0 0.95s0 1.0s0)
		 (if ,(ext-is 'ext '("mp4" "mkv" "mov" "mp3" "flac"))
                     (Color--new 0.75s0 0.35s0 0.85s0 1.0s0)
                     (if ,(ext-is 'ext '("zip" "tar" "gz" "7z"))
			 (Color--new 0.9s0 0.3s0 0.25s0 1.0s0)
			 (hash_color path)))))))

     (defun format_bytes (b)
       (declare (type u64 b) (values String))
       (let ((units (bracket (string "B") (string "KB") (string "MB")
                             (string "GB") (string "TB")))
             (d (coerce b f64))
             (i 0))
	 (declare (type usize i) (mutable d i))
	 (while (and (<= 1024.0 d) (< i 4))
		;; HATCH: (/= d 1024.0) emits d/=(1024.0); rustfmt keeps the
		;; redundant parens (unused_parens). Proposed upstream in walkthrough.
		"d /= 1024.0;"
		(incf i))
	 (format! (string "{:.1} {}") d (aref units i))))

     "#[test]"
     (defun test_format_bytes ()
       ,@(loop for (bytes want) in '((0 "0.0 B") (100 "100.0 B")
                                     (2048 "2.0 KB") (5242880 "5.0 MB"))
               collect `(assert_eq! (format_bytes ,bytes) (string ,want))))


     (defun render_tree (node mouse hovered)
       (declare (type &Node node) (type Vec2 mouse)
		(type "&mut Option<String>" hovered))
       (when (or (< (dot node rect w) 1.0) (< (dot node rect h) 1.0))
	 (return))
       (if (dot node children (is_empty))
	   (progn
             (draw_rectangle (dot node rect x) (dot node rect y)
                             (dot node rect w) (dot node rect h) (dot node color))
             (draw_rectangle_lines (dot node rect x) (dot node rect y)
				   (dot node rect w) (dot node rect h) 1.0
				   (Color--new 0.0s0 0.0s0 0.0s0 0.35s0)))
	   (progn
             (for (child (ref (dot node children)))
		  (render_tree child mouse hovered))
             (draw_rectangle_lines (dot node rect x) (dot node rect y)
				   (dot node rect w) (dot node rect h) 1.0
				   (Color--new 0.0s0 0.0s0 0.0s0 0.6s0))))
       (when (and (dot hovered (is_none))
		  (dot (dot node rect) (contains mouse)))
	 (= (deref hovered)
	    (Some (format! (string "{} ({})")
			   (dot node path (display))
			   (format_bytes (dot node size)))))))


     

     (attr "macroquad::main(\"Treemap Disk Visualizer MVP\")"
	   (defun-async main ()
	     (let ((raw_target (dot (std--env--args) (nth 1)
				    (map (scope PathBuf from))
				    (unwrap_or_else
                                     (lambda ()
                                       (PathBuf--from (string "."))))))
		   (target (dot (fs--canonicalize (ref raw_target))
				(unwrap_or raw_target)))
		   ((tuple tx rx) ((scope channel (angle Node)))))
               ,(lprint :msg "target" :vars '((dot target (display))))
               (thread--spawn "move || { let root = scan_tree(&target); let _ = tx.send(root); }")
               (let ((root None) (last_size (tuple 0.0 0.0)))
		 (declare (type "Option<Node>" root) (mutable root last_size))
		 (loop
		   (clear_background (Color--new 0.08s0 0.09s0 0.12s0 1.0s0))
		   (let (((tuple screen_w screen_h)
			   (tuple (screen_width) (screen_height))))
		     (if-let ((Ok loaded_root) (dot rx (try_recv)))
		       (progn
			 (= root (Some loaded_root))
			 (stmt (= last_size (tuple 0.0 0.0)))))
		     (if-let ((Some tree) (ref-mut root))
		       (progn
			 (let ((avail_h (- screen_h 36.0)))
			   (declare (mutable avail_h))
			   (when (< avail_h 1.0) (stmt (= avail_h 1.0)))
			   (let ((canvas (Rect--new 0.0 36.0 screen_w avail_h)))
			     (when (!= (tuple screen_w screen_h) last_size)
			       (= (dot tree rect) canvas)
			       (squarify (ref-mut (dot tree children)) canvas)
			       (= last_size (tuple screen_w screen_h))
			       ,(lprint :msg "layout"
					:vars '(screen_w screen_h (dot tree size))))
			     (let ((hovered None))
			       (declare (type "Option<String>" hovered) (mutable hovered))
			       (render_tree tree (Vec2--from (mouse_position)) (ref-mut hovered))
			       (draw_rectangle 0.0 0.0 screen_w 36.0
					       (Color--new 0.05s0 0.06s0 0.08s0 0.95s0))
			       (let ((header (dot hovered
						  (unwrap_or_else
						   (lambda ()
						     (format! (string "{}: {}")
							      (dot tree path (display))
							      (format_bytes (dot tree size))))))))
				 (stmt (draw_text (ref header) 14.0 24.0 16.0 WHITE)))))))
		       (stmt (draw_text (string "Scanning filesystem...")
					20.0 40.0 24.0 LIGHTGRAY)))
		     (await (next_frame))))))))


     )
   )
  )
