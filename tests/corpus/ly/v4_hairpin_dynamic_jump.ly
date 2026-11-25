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
  \clef treble \key c \major \time 4/4
  \tempo 4 = 120 g,4 _\f \< a,4 c4 d4 |
  c1 _\mf \! \bar "||"
  g,4 _\f \> a,4 c4 d4 |
  c1 _\ff \! \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Clean Guitar" shortInstrumentName = "el.guit." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
