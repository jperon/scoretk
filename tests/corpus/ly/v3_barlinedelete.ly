\version "2.24.0"
\language "english"

\header {
  title = "Title"
  workTitle = "Title"
  composer = "Composer"
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  \tempo "Tempo text" 4 = 120 c'4 e'4 g'4 e'4 \bar "||"
  c'4 g4 c'4 e'4 |
  \mark \markup { "1" } g'4 r4 r2 |
  R1 |
  \break
  R1 \bar "||"
  R1 ^\markup { "Staff text" } |
  R1 \fermata |
  R1 ^\markup { "System text" } |
  \time 3/4
  R2. |
  R2. \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
