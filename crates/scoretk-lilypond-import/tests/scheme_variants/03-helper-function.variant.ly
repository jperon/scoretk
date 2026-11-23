\version "2.24.0"
#(define (twice n) (* n 2))
rep = #(define-music-function (music) (ly:music?) #{ \repeat unfold #(twice 1) $music #})
\score { \fixed c' { \rep { c'8 d' } e'2 } \midi { } }
