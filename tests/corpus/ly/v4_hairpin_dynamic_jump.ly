\version "2.24.0"
\language "english"

\paper {
  paper-width = 210.05800\mm
  paper-height = 296.92600\mm
  left-margin = 15.00000\mm
  right-margin = 15.00000\mm
  top-margin = 15.00000\mm
  bottom-margin = 15.00000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \key c \major \time 4/4
  \set Staff.midiMinimumVolume = #0.755906
  \set Staff.midiMaximumVolume = #0.755906 \tempo "♩ = 120" 4 = 120
  g,4 _\f \< a,4 c4 d4 |
  \set Staff.midiMinimumVolume = #0.629921
  \set Staff.midiMaximumVolume = #0.629921 c1 _\mf \! \bar "||"
  \set Staff.midiMinimumVolume = #0.755906
  \set Staff.midiMaximumVolume = #0.755906 g,4 _\f \> a,4 c4 d4 |
  \set Staff.midiMinimumVolume = #0.881890
  \set Staff.midiMaximumVolume = #0.881890 c1 _\ff \! \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Clean Guitar" shortInstrumentName = "el.guit." midiInstrument = "electric guitar (clean)" midiExpression = #0.795276 } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
