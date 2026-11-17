\version "2.24.0"
\language "english"

\header {
  title = "Title"
  composer = "Composer"
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
