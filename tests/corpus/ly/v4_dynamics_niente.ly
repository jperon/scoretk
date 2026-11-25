\version "2.24.0"
\language "english"

\header {
  title = "Naamloze partituur"
  workTitle = "Naamloze partituur"
  subtitle = "Ondertitel"
  composer = "Componist / arrangeur"
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
  d'1~ _\f \> |
  d'1 _\n \! |
  d'1~ _\f \> |
  d'1 \! |
  d'1~ _\n \< |
  d'1 _\f \! |
  d'1~ \< |
  d'1 _\f \! \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Viool" shortInstrumentName = "Vl." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
