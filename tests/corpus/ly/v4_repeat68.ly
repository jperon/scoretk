\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = \markup { \bold "Repeat Test" }
  workTitle = "Repeat Test"
  subtitle = "Volta skipping entire score"
  subsubtitle = "repeat68"
  instrument = "Volta skipping entire score"
  scoretkCreditA = "repeat68"
  scoretkCreditB = "Volta skipping entire score"
  scoretkTboxA = "0|no playback"
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
  \clef treble \time 4/4
  \tempo "♩ = 400" 4 = 400 e1 ^\markup { "no playback" } \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
