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
  g4 \tweak #'style #'cross g4 ^\markup { "scoretk-head-cross" } |
  \tweak #'style #'cross g4 ^\markup { "scoretk-head-plus" }
  \tweak #'style #'xcircle g4 ^\markup { "scoretk-head-xcircle" } |
  \tweak #'style #'cross g4 ^\markup { "scoretk-head-withx" }
  \tweak #'style #'triangle g4 ^\markup { "scoretk-head-triangle-up" } |
  \tweak #'style #'triangle g4 ^\markup { "scoretk-head-triangle-down" }
  \tweak #'style #'slash g4 ^\markup { "scoretk-head-slashed1" } |
  \tweak #'style #'slash g4 ^\markup { "scoretk-head-slashed2" }
  \tweak #'style #'harmonic g4 ^\markup { "scoretk-head-diamond" } |
  \tweak #'style #'harmonic g4 ^\markup { "scoretk-head-diamond-old" }
  \tweak #'style #'xcircle g4 ^\markup { "scoretk-head-circled" } |
  \tweak #'style #'xcircle g4 ^\markup { "scoretk-head-circled-large" }
  \tweak #'style #'triangle g4 ^\markup { "scoretk-head-large-arrow" } |
  \tweak #'style #'cross g4 ^\markup { "scoretk-head-brevis-alt" }
  \tweak #'style #'slash g4 ^\markup { "scoretk-head-slash" } |
  \tweak #'style #'harmonic g4 ^\markup { "scoretk-head-large-diamond" }
  \tweak #'style #'sol g4 ^\markup { "scoretk-head-sol" } |
  \tweak #'style #'la g4 ^\markup { "scoretk-head-la" }
  \tweak #'style #'fa g4 ^\markup { "scoretk-head-fa" } |
  \tweak #'style #'mi g4 ^\markup { "scoretk-head-mi" }
  \tweak #'style #'do g4 ^\markup { "scoretk-head-do" } |
  \tweak #'style #'re g4 ^\markup { "scoretk-head-re" }
  \tweak #'style #'ti g4 ^\markup { "scoretk-head-ti" } |
  \tweak #'style #'cross g4 ^\markup { "scoretk-head-heavy-cross" }
  \tweak #'style #'cross g4 ^\markup { "scoretk-head-heavy-cross-hat" }
  \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
