\version "2.24.0"
\language "english"

\paper {
  paper-width = 210.058\mm
  paper-height = 296.926\mm
  left-margin = 15.000\mm
  right-margin = 15.000\mm
  top-margin = 15.000\mm
  bottom-margin = 15.000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  \once \stemDown c'8 \once \stemDown c'8 \once \stemDown c'8
  \once \stemDown c'8 r2 |
  g8 g8 g8 g8 r2 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 \bar "|."
}

staffTwoVoiceOne = \fixed c' {
  \clef bass \time 4/4
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
  R1 |
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
