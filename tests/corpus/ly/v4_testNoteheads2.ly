\version "2.24.0"
\language "english"

\header {
  title = \markup { \bold "Full range of noteheads" }
  workTitle = "Full range of noteheads"
  composer = "Klaus Rettinghaus"
  subsubtitle = "Full range of noteheads"
  scoretkCreditA = "Full range of noteheads"
}

\paper {
  paper-width = 210.05800\mm
  paper-height = 296.92600\mm
  left-margin = 15.00000\mm
  right-margin = 15.00000\mm
  top-margin = 15.00000\mm
  bottom-margin = 15.00000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 2/4
  g4 \tweak #'style #'cross g4 |
  \tweak #'style #'cross g4 \tweak #'style #'xcircle g4 |
  \tweak #'style #'cross g4 \tweak #'style #'triangle g4 |
  \tweak #'style #'triangle g4 \tweak #'style #'slash g4 |
  \tweak #'style #'slash g4 \tweak #'style #'harmonic g4 |
  \tweak #'style #'harmonic g4 \tweak #'style #'xcircle g4 |
  \tweak #'style #'xcircle g4 \tweak #'style #'triangle g4 |
  \tweak #'style #'cross g4 \tweak #'style #'slash g4 |
  \tweak #'style #'harmonic g4 \tweak #'style #'sol g4 |
  \tweak #'style #'la g4 \tweak #'style #'fa g4 |
  \tweak #'style #'mi g4 \tweak #'style #'do g4 |
  \tweak #'style #'re g4 \tweak #'style #'ti g4 |
  \tweak #'style #'cross g4 \tweak #'style #'cross g4 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
