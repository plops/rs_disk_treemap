;; helpers.lisp --- tiny generator helpers (all side-effect free).
(in-package :cl-rust-generator)
;; lprint: C++ model (gen-cpp-freestanding-example.lisp) in Rust shape.
;; Every variable shows up as [label]={:?} in exactly one eprintln! call.
;; A var is an expression, or (:as "label" expression) for a custom label.
(defun lprint (&key (msg "") (vars nil))
  (let ((entries (mapcar (lambda (v)
                           (if (and (consp v) (eq (car v) :as))
                               (list (second v) (third v))
                               (list (emit-rs :code v) v))
                           )
                         vars
                         )
                 )
        )
    `(eprintln!
      (string
       ,(format
         nil
         "~a~{[~a]={:?}~^ ~}"
         (if (string= msg "")
             ""
             (format nil "~a " msg)
             )
         (mapcar #'first entries)
         )
       )
      ,@(mapcar #'second entries)
      )
    )
  )
;; (ext-is ext (e1 e2 ...)) => (or (== ext "e1") (== ext "e2") ...)
(defun ext-is (ext-var exts)
  `(or
    ,@(loop for e in exts
            collect `(== ,ext-var (string ,e))
            )
    )
  )
;; report-skip-fn: exactly one Rust helper fn for all scan error sites.
(defun report-skip-fn ()
  `(defun report_skip (&context &path err)
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
  )
;; Shared `use` block for the whole generated crate.
(defun emit-uses ()
  `(use
    (macroquad prelude *)
    (notify (curly Event EventKind RecommendedWatcher RecursiveMode Watcher))
    (std collections (curly HashSet hash_map--DefaultHasher))
    (std ffi OsStr)
    (std fs)
    (std hash (curly Hash Hasher))
    (std io (curly self Write))
    (std path (curly Path PathBuf))
    (std sync Arc)
    (std sync atomic (curly AtomicBool AtomicUsize Ordering))
    (std sync mpsc (curly Receiver Sender channel))
    (std thread)
    (std time Instant)
    )
  )
