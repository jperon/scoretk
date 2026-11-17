\version "2.24.0"
\language "english"

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  \once \override Stem.direction = #DOWN ef'4~ ^\markup { "Ebm(add9)/F" }
  \once \override Stem.direction = #DOWN ef'4. r8
  \once \override Stem.direction = #UP af4 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" shortInstrumentName = "Vo." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
