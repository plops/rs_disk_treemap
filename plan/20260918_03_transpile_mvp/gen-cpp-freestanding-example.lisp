(eval-when (:compile-toplevel :execute :load-toplevel)
  (ql:quickload "cl-cpp-generator2"))

(defpackage #:my-cpp-project
  (:use #:cl #:cl-cpp-generator2))
(in-package #:my-cpp-project)

(progn
  ;; ---------------------------------------------------------------- logging
  ;; A var is either an expression (the emitted code doubles as its label)
  ;; or (:as "label" expression).  One std::print call, so every value
  ;; appears exactly once in the generated line.  An optional format spec
  ;; (e.g. ":.3f") can be attached with (:as "label" expr "spec").
  ;; Weg A: route every log line through the injected ls logger.  A single
  ;; LS_LOG_INFO macro call (base/Log.h) forwards to the pmf logging provider
  ;; that SET_PMF_LOGGER wired up, so app output and the internal component
  ;; logs share one console + file sink.  The format string still uses the
  ;; fmt/std::format "{}" syntax the macro expects.
  (defun lprint (&key (msg "") (vars nil))
    (let ((entries (mapcar (lambda (v)
			     (if (and (consp v) (eq (car v) :as))
				 (list (second v) (third v) (fourth v))
				 (list (emit-c :code v) v nil)))
			   vars)))
      `(LS_LOG_INFO (string ,(format nil "~a~{ ~a='{~a}'~}"
				     msg
				     (loop for e in entries
					   append (list (first e)
							(or (third e) "")))))
		    ,@(mapcar #'second entries))))

  (write-source
   "output.cpp"
   `(do0
     (include 
	      "popl.hpp"
	      )
     (include<> chrono cstdint iostream optional thread
		array fstream iterator string vector
		condition_variable mutex utility)

     (defun main (argc argv)
       (declare (type int argc)
		(type char** argv)
		(values int))




       (let ((op (popl--OptionParser (string "Allowed options")))
	     (helpOpt (op.add<popl--Switch> (string "h") (string "help")
					    (string "produce help message"))))
	 (op.parse argc argv)
	 (when (helpOpt->is_set)
	   (return 0)))
       (return 0)))
   :format t
   :tidy nil
   :omit-parens t))
