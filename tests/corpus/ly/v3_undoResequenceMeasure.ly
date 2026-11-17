\version "2.24.0"
\language "english"

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  c'1 |
  c'1 |
  c'1 |
  c'1 |
  \break
  \mark \markup { "5" } c'1 |
  c'1 |
  c'1 |
  c'1 |
  \break
  \mark \markup { "QQ" } c'1 |
  c'1 |
  c'1 |
  c'1 |
  \break
  \mark \markup { "Foo" } c'1 |
  c'1 |
  c'1 |
  c'1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Flute" shortInstrumentName = "Fl." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
