\version "2.24.0"
\language "english"

\header {
  title = "Idee in Noten no.8"
  workTitle = "Idee in Noten no.8"
  composer = "Yannis A. Paul"
}

staffOneVoiceOne = \fixed c' {
  \time 4/4
  f4 _\p \< f4 f4 f4 |
  \repeat volta 2 {
    f4 _\f \! \< f4 f4 f4 \! |
  }
  g4 _\ppp \< g4 g4 g4 _\p \! \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Klavier" shortInstrumentName = "Flgl." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
