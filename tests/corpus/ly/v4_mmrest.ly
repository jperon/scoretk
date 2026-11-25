\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = \markup { \bold "Title" }
  workTitle = "Title"
  composer = "Composer"
  subsubtitle = "Title"
  instrument = "Composer"
  scoretkCreditA = "Title"
  scoretkCreditB = "Composer"
}

\paper {
  paper-width = 209.99983\mm
  paper-height = 296.99966\mm
  left-margin = 10.00001\mm
  right-margin = 15.00000\mm
  top-margin = 10.00001\mm
  bottom-margin = 20.00004\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  R1 |
  R1 ^\markup { "VII" } |
  R1 |
  R1 |
  \break
  R1 |
  R1 |
  R1 |
  R1 |
  \break
  R1 |
  R1 |
  R1 |
  R1 |
  \break
  R1 |
  R1 |
  R1 |
  R1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
