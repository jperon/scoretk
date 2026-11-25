\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = "Title"
  workTitle = "Title"
  composer = "Composer"
}

\paper {
  paper-width = 210.000\mm
  paper-height = 297.000\mm
  left-margin = 10.000\mm
  right-margin = 15.000\mm
  top-margin = 10.000\mm
  bottom-margin = 20.000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  R1 |
  R1*3 |
  R1*4 |
  R1*4 |
  R1*4 |
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
