\version "2.24.0"
\language "english"

\header {
  title = "Title"
  workTitle = "Title"
  composer = "Composer"
}

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
  R1 |
  \section \key g \major
  R1 |
  \section \key d \major
  R1 |
  \section \key a \major
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
