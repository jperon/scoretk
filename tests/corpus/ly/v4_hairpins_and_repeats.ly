\version "2.24.0"
\language "english"

\header {
  title = \markup { \bold "Idee in Noten no.8" }
  workTitle = "Idee in Noten no.8"
  composer = "Yannis A. Paul"
}

\paper {
  paper-width = 210.05800\mm
  paper-height = 296.92600\mm
  left-margin = 15.00000\mm
  right-margin = 15.00000\mm
  top-margin = 15.00000\mm
  bottom-margin = 15.00000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  \set Staff.midiMinimumVolume = #0.385827
  \set Staff.midiMaximumVolume = #0.385827 f4 _\p \< f4 f4 f4 |
  \repeat volta 2 {
    \set Staff.midiMinimumVolume = #0.755906
    \set Staff.midiMaximumVolume = #0.755906 f4 _\f \! \< f4 f4 f4 \! |
  }
  \set Staff.midiMinimumVolume = #0.125984
  \set Staff.midiMaximumVolume = #0.125984 g4 _\ppp \< g4 g4
  \set Staff.midiMinimumVolume = #0.385827
  \set Staff.midiMaximumVolume = #0.385827 g4 _\p \! \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Klavier" shortInstrumentName = "Flgl." midiInstrument = "acoustic grand" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
