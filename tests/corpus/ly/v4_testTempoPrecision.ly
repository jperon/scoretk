\version "2.24.0"
\language "english"

#(set-global-staff-size 19.91442529)
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
  paper-width = 210.05800\mm
  paper-height = 296.92600\mm
  left-margin = 15.00000\mm
  right-margin = 15.00000\mm
  top-margin = 15.00000\mm
  bottom-margin = 15.00000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  \tempo "♩ = 88" 4 = 88 R1 |
  \tempo "Andante" 4 = 92 R1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." midiInstrument = "acoustic grand" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
