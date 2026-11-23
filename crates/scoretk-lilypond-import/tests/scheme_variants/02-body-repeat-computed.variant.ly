\version "2.24.0"
rep = #(define-music-function (music) (ly:music?) #{ \repeat unfold #(+ 1 1) $music #})
\score { \fixed c' { \rep { c'8 d' } e'2 } \midi { } }
