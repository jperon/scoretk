\version "2.24.0"
\language "english"

\header {
  title = "Naamloze partituur"
  workTitle = "Naamloze partituur"
  subtitle = "Ondertitel"
  composer = "Componist / arrangeur"
}

staffOneVoiceOne = \fixed c' {
  \key c \major \time 4/4
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
