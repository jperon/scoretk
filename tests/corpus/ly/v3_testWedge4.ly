\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = \markup { \fontsize #24.00000 "Wedge 4" }
  subtitle = "MuseScore testfile"
  composer = \markup { \fontsize #12.00000 "Leon Vinken" }
  poet = \markup { \fontsize #12.00000 "" }
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  subsubtitle = "Wedge 4"
  instrument = "MuseScore testfile"
  piece = "Leon Vinken"
  scoretkCreditA = "Wedge 4"
  scoretkCreditB = "MuseScore testfile"
  scoretkCreditC = "Leon Vinken"
}

\paper {
  paper-width = 210.05800\mm
  paper-height = 296.92600\mm
  left-margin = 10.00000\mm
  right-margin = 10.00000\mm
  top-margin = 10.00000\mm
  bottom-margin = 20.00000\mm
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
