;;; object_pre_org_9_7_11.el --- which objects org reads after which character  -*- lexical-binding: t -*-

;; Writes `object_pre_org_9_7_11.txt'. Run from its directory:
;;   emacs -Q --batch -l object_pre_org_9_7_11.el
;;
;; Each row is a paragraph LINE, then the objects org-element reads in it,
;; separated by \x1f. An object is its type and its text without post-blank,
;; separated by \x1e. LINE puts each character C (not a newline) in front of
;; an emphasis, an inline source block, an inline call, an underscore that
;; starts an underline or a subscript and a superscript, and puts objects right after other
;; objects.

(require 'org)
(require 'org-element)

(defconst object-pre-types
  '(bold italic underline verbatim code strike-through inline-src-block
    inline-babel-call link subscript superscript line-break entity))

(defun object-pre-objects (line)
  (with-temp-buffer
    (org-mode)
    (insert line)
    (mapconcat
     #'identity
     (org-element-map (org-element-parse-buffer) object-pre-types
       (lambda (o)
         (format "%s\x1e%s" (org-element-type o)
                 (buffer-substring-no-properties
                  (org-element-begin o)
                  (- (org-element-end o) (org-element-post-blank o))))))
     "\x1f")))

(defconst object-pre-chars
  (append (number-sequence 9 9) (number-sequence 11 12) (number-sequence 32 126)
          '(#x80 #x9f #xa0 #xa5 #xaa #xad #xb2 #xb7 #xbc #xe9 #x24f #x2b0 #x300
            #x3b1 #x430 #x483 #x5d0 #x627 #x661 #x966 #x1e00 #x2000 #x2005 #x200b
            #x200c #x2019 #x201c #x2028 #x202f #x205f #x3000 #x3042 #x4e2d
            #xa700 #xac00 #xfb00 #xff08 #xff21)))

(defconst object-pre-contexts
  '("*/a/*" "*a*/b/" "/a/*b*" "[[x][/a/]]" "[[x][a/b/]]" "[[x]]/a/" "[[x]] /a/"
    "*a*'/b/'" "*a*-/b/" "~a~'/b/" "_a_/b/" "*a*src_x{y}" "=a=call_f()"
    "src_x{y}src_x{z}" "[[x]]src_x{y}" "*a*_b" "*a* _b" "[[x]]^b" "[[x]] ^b"
    "x =a=\\\\" "x \\alpha\\\\" "x \\alpha \\\\" "x\\\\\\\\"))

(with-temp-file "object_pre_org_9_7_11.txt"
  (dolist (c object-pre-chars)
    (dolist (template '("a%s*b* z" "a%ssrc_x{y} z" "a%scall_f() z"
                        "a%s_b_ z" "a%s_*b* z" "a%s_{b} z" "a%s_b z" "a%s^b z"))
      (let ((line (format template (string c))))
        (insert line "\x1f" (object-pre-objects line) "\n"))))
  (dolist (line object-pre-contexts)
    (insert line "\x1f" (object-pre-objects line) "\n")))
