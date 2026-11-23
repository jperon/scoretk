\version "2.24.0"
\score { \fixed c' { \once \override Stem.thickness = #(if #t 2 3) c'4 d'4 e'2 } \midi { } }
