\version "2.24.0"
\language "english"

\header {
  title = "Full range of noteheads"
  workTitle = "Full range of noteheads"
  composer = "Klaus Rettinghaus"
}

\paper {
  paper-width = 210.058\mm
  paper-height = 296.926\mm
  left-margin = 15.000\mm
  right-margin = 15.000\mm
  top-margin = 15.000\mm
  bottom-margin = 15.000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 2/4
  g4 \tweak style #'cross g4 |
  g4 \tweak style #'xcircle g4 |
  g4 \tweak style #'do g4 |
  \tweak style #'do g4 \tweak style #'slash g4 |
  \tweak style #'slash g4 \tweak style #'harmonic-mixed g4 |
  \tweak style #'harmonic-mixed g4 \tweak style #'xcircle g4 |
  \tweak style #'xcircle g4 \tweak style #'do g4 |
  g4 \tweak style #'slash g4 |
  \tweak style #'harmonic-mixed g4 \tweak style #'sol g4 |
  \tweak style #'la g4 \tweak style #'fa g4 |
  \tweak style #'mi g4 \tweak style #'do g4 |
  \tweak style #'re g4 \tweak style #'ti g4 |
  \tweak style #'cross g4 \tweak style #'cross g4 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
