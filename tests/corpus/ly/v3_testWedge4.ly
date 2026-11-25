\version "2.24.0"
\language "english"

\header {
  title = "Wedge 4"
  subtitle = "MuseScore testfile"
  composer = "Leon Vinken"
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
  \voiceOne
  \clef treble \time 4/4
  r2 g2 \< |
  r2 \! g2 \bar "|."
}

staffOneVoiceTwo = \fixed c' {
  \voiceTwo
  s1 |
  \once \override Rest.transparent = ##t r4
  \once \override Rest.transparent = ##t r4
  \once \override Rest.transparent = ##t r4 \<
  \once \override Rest.transparent = ##t r4 \! \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" shortInstrumentName = "Vo." } <<
      { \staffOneVoiceOne }
      \\
      { \staffOneVoiceTwo }
    >>
  >>
  \layout { }
}
