\version "2.24.0"
\language "english"

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  a4 b4 c'4 d'4 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" shortInstrumentName = "Vo." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
      \new Lyrics \lyricsto "staffOneVoiceOne" {
        A __ \skip 1 \skip 1 \skip 1
      }
    >>
  >>
  \layout { }
}
