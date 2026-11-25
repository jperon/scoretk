\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = "Wedge 4"
  subtitle = "MuseScore testfile"
  composer = "Leon Vinken"
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
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
