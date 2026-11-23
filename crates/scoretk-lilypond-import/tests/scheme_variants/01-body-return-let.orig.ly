\version "2.24.0"
twice = #(define-music-function (music) (ly:music?) music)
\score { \fixed c' { \twice { c'4 d' } e'4 f'2 } \midi { } }
