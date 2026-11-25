\version "2.24.0"
\language "english"

\header {
  title = "selection-filter-dynamics"
  workTitle = "selection-filter-dynamics"
}

\paper {
  paper-width = 210.058\mm
  paper-height = 296.926\mm
  left-margin = 10.000\mm
  right-margin = 10.000\mm
  top-margin = 10.000\mm
  bottom-margin = 20.000\mm
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
