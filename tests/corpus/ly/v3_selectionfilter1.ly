\version "2.24.0"
\language "english"

\header {
  title = "selection-filter-dynamics"
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  c'4 _\f d'4 \< e'4 f'4 \! |
  R1 |
  R1 |
  R1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Violin" shortInstrumentName = "Vln." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
