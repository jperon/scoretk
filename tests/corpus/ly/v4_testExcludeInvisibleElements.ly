\version "2.24.0"
\language "english"

\header {
  title = "Invisible elements test"
  workTitle = "Invisible elements test"
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  c4 \fermata c4 \fermata d4-. d4 |
  c4 ^\markup { "Visible text" } c4 c4 c4 |
  \tempo 4 = 80 d4 d4 d4 d4 |
  c4\reverseturn c4\turn c4 c4 |
  \break
  e4 _\p \< e4 e4 e4 _\f \! |
  e4 \< e4 e4 e4 \! |
  c4 \sustainOn \=5( d4 e4 f4 \=5) |
  c4 \sustainOff \sustainOn \=6( d4 e4 f4 \=6) |
  \ottava 1 f4 \sustainOff f4 f4 f4 |
  \ottava 0 f4 f4 f4 f4 |
  \break
  c4 \glissando a4 c4 \glissando a4 |
  <c e g>4 <c e g>4 r4 a4~-4 |
  a4 r4 r4 a4~ |
  a4 r4 r4 a4~-4 |
  a4 r4 r2 |
  \break
  \key e \major \time 3/4
  R2. |
  \key af \major \time 4/4
  \once \override Rest.transparent = ##t r1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
