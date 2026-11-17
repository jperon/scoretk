\version "2.24.0"
\language "english"

\header {
  title = "Title"
  workTitle = "Title"
  composer = "Composer on page 1"
  poet = "Lyricist on page 1"
  copyright = "Copyright"
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  R1 ^\markup { "Above" } _\markup { "Below" } |
  R1*2 |
  R1 |
  g1 ^\markup { "Staff Text" } _\markup { "Staff Text" } |
  g1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" shortInstrumentName = "Vo." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
