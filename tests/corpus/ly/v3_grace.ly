\version "2.24.0"
\language "english"

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
  c'2 r2 |
  R1 |
  R1 |
  R1 \bar "|."
}

staffTwoVoiceOne = \fixed c' {
  \clef bass \time 4/4
  R1 |
  R1 |
  R1 |
  R1 \bar "|."
}

\score {
  <<
    \new PianoStaff \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Staff = "staffOne" <<
        \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
      >>
      \new Staff = "staffTwo" <<
        \new Voice = "staffTwoVoiceOne" { \staffTwoVoiceOne }
      >>
    >>
  >>
  \layout { }
}
