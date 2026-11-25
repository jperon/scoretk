\version "2.24.0"
\language "english"

\header {
  title = "Untitled score"
  workTitle = "Untitled score"
  subtitle = "Subtitle"
  composer = "Composer / arranger"
  scoretkCreditA = "Untitled score"
  scoretkCreditB = "Subtitle"
  scoretkCreditC = "Composer / arranger"
}

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
  \set Staff.midiMinimumVolume = #0.385827
  \set Staff.midiMaximumVolume = #0.385827 d'4 _\p d'4 d'4 d'4 |
  d'4 \< d'4 d'4 d'4 |
  \set Staff.midiMinimumVolume = #0.755906
  \set Staff.midiMaximumVolume = #0.755906 d'4 _\f \! d'4 d'4 d'4 |
  d'4 \< d'4 d'4 d'4 |
  \set Staff.midiMinimumVolume = #0.259843
  \set Staff.midiMaximumVolume = #0.259843 d'4 _\pp \! d'4 d'4 d'4 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Violin" shortInstrumentName = "Vln." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
