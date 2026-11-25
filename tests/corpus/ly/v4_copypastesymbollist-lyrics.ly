\version "2.24.0"
\language "english"

\paper {
  paper-width = 210.058\mm
  paper-height = 296.926\mm
  left-margin = 15.000\mm
  right-margin = 15.000\mm
  top-margin = 15.000\mm
  bottom-margin = 15.000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  c'2 d'4 e'4 |
  f'2 e'4 d'4 |
  c'8 d'8 e'8 f'8 g'8 f'8 e'8 d'8 |
  c'1 |
  c'4 d'4 e'4 f'4 |
  g'8 f'8 e'8 d'8 c'8 d'8 e'8 f'8 |
  e'2 d'2 |
  c'1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
      \new Lyrics \lyricsto "staffOneVoiceOne" {
        Nel mez -- zo del cam \skip 1
        min __ \skip 1 \skip 1 di no -- stra
        vi \skip 1 ta. \skip 1 \skip 1 \skip 1
        \skip 1 \skip 1 \skip 1 \skip 1 \skip 1 \skip 1
        \skip 1 \skip 1 \skip 1 \skip 1 \skip 1 \skip 1
      }
    >>
  >>
  \layout { }
}
