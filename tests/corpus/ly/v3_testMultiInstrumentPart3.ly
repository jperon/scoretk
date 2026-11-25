\version "2.24.0"
\language "english"

\header {
  title = "clarinet scales"
  workTitle = "clarinet scales"
}

\paper {
  paper-width = 215.900\mm
  paper-height = 279.400\mm
  left-margin = 10.000\mm
  right-margin = 10.000\mm
  top-margin = 10.000\mm
  bottom-margin = 20.000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \transposition a \key c \major \time 4/4
  ef4 ^\markup { "A Clarinet" } f4 g4 af4 |
  bf4 c'4 d'4 ef'4 |
  \transposition bf \key c \major
  ef4 ^\markup { "B♭ Clarinet" } f4 g4 af4 |
  bf4 c'4 d'4 ef'4 |
  R1 |
  R1 |
  R1 |
  R1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "A Clarinet" shortInstrumentName = "A Cl." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
