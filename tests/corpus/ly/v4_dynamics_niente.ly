\version "2.24.0"
\language "english"

\header {
  title = \markup { \bold "Naamloze partituur" }
  workTitle = "Naamloze partituur"
  subtitle = "Ondertitel"
  composer = "Componist / arrangeur"
  subsubtitle = "Naamloze partituur"
  instrument = "Ondertitel"
  piece = "Componist / arrangeur"
  scoretkCreditA = "Naamloze partituur"
  scoretkCreditB = "Ondertitel"
  scoretkCreditC = "Componist / arrangeur"
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
  \clef treble \key c \major \time 4/4
  \set Staff.midiMinimumVolume = #0.755906
  \set Staff.midiMaximumVolume = #0.755906 d'1~ _\f \> |
  \set Staff.midiMinimumVolume = #0.385827
  \set Staff.midiMaximumVolume = #0.385827 d'1 _\n \! |
  \set Staff.midiMinimumVolume = #0.755906
  \set Staff.midiMaximumVolume = #0.755906 d'1~ _\f \> |
  d'1 \! |
  \set Staff.midiMinimumVolume = #0.385827
  \set Staff.midiMaximumVolume = #0.385827 d'1~ _\n \< |
  \set Staff.midiMinimumVolume = #0.755906
  \set Staff.midiMaximumVolume = #0.755906 d'1 _\f \! |
  d'1~ \< |
  \set Staff.midiMinimumVolume = #0.755906
  \set Staff.midiMaximumVolume = #0.755906 d'1 _\f \! \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Viool" shortInstrumentName = "Vl." midiInstrument = "violin" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
