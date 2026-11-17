\version "2.24.0"
\language "english"

\header {
  title = "Wedge 4"
  subtitle = "MuseScore testfile"
  composer = "Leon Vinken"
}

staffOneVoiceOne = \fixed c' {
  \voiceOne
  \clef treble \time 4/4
  r2 g2 \< |
  r2 \! g2 \bar "|."
}

staffOneVoiceTwo = \fixed c' {
  \voiceTwo
  R1 |
  s4 s4 s4 \< s4 \! \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" shortInstrumentName = "Vo." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
      \new Voice = "staffOneVoiceTwo" { \staffOneVoiceTwo }
    >>
  >>
  \layout { }
}
