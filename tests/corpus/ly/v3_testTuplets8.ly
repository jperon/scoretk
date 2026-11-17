\version "2.24.0"
\language "english"

\header {
  title = "Tuplets 8"
  subtitle = "MuseScore testfile"
  composer = "Leon Vinken"
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 2/4
  \tuplet 3/2 {
    c'4 b4
    \tuplet 5/4 {
    a16 g16 f16 e16 d16
    }
  }
  |
  \tuplet 3/2 {
    c'4
    \tuplet 5/4 {
    b16 a16 g16 f16 e16
    }
  d4
  }
  |
  \tuplet 3/2 {
    \tuplet 5/4 {
    c'16 b16 a16 g16 f16
    }
  e4 d4
  }
  \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" shortInstrumentName = "Vo." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
