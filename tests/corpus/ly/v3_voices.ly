\version "2.24.0"
\language "english"

\header {
  title = "voices"
}

staffOneVoiceOne = \fixed c' {
  \voiceOne
  \clef treble \key af \major \time 4/4
  R1 _\pp |
  R1 |
  R1 ^\markup { "Eb7" } |
  \time 2/4
  R2 |
  R2 |
  \key g \major
  R2 |
  \key ef \major \time 4/4
  R1 \bar "|."
}

staffOneVoiceTwo = \fixed c' {
  \voiceTwo
  af4 bf4 c'4 r4 |
  ef4 ef4 ef4-2 ef4-> |
  ef4 ef4 ef4 ef4 |
  ef4 \=2( ef4 |
  ef4 ef4 \=2) |
  ef4~ ef4 |
  ef4 r2 r4 \bar "|."
}

staffOneVoiceThree = \fixed c' {
  \voiceThree
  ef'4 f'4 g'4 af'4 |
  R1 |
  ef'4 ef'4 ef'4 ef'4 |
  R2 |
  R2 |
  R2 |
  R1 \bar "|."
}

staffTwoVoiceOne = \fixed c' {
  \clef bass \time 4/4
  R1 |
  R1 |
  g,4 a,4 b,4 r4 |
  \time 2/4
  R2 |
  R2 |
  \key g \major
  R2 |
  \key ef \major \time 4/4
  R1 \bar "|."
}

staffThreeVoiceOne = \fixed c' {
  \voiceOne
  \clef bass \time 4/4
  R1 |
  R1 |
  g,2 g,2 |
  \time 2/4
  b,2 |
  R2 |
  \key g \major
  R2 |
  \key ef \major \time 4/4
  R1 \bar "|."
}

staffThreeVoiceTwo = \fixed c' {
  \voiceTwo
  R1 |
  R1 |
  R1 |
  R2 |
  R2 |
  R2 |
  R1 \bar "|."
}

staffThreeVoiceThree = \fixed c' {
  \voiceThree
  R1 |
  R1 |
  R1 |
  R2 |
  R2 |
  R2 |
  R1 \bar "|."
}

staffThreeVoiceFour = \fixed c' {
  \voiceFour
  c,4 d,4 e,4 r4 |
  g,,4 g,,4 g,,4-2 g,,4-> |
  g,,4 g,,4 g,,4 g,,4 |
  g,,4 \=3( g,,4 |
  g,,4 g,,4 \=3) |
  g,,4~ g,,4 |
  g,,4 r2 r4 \bar "|."
}

\score {
  <<
    \new PianoStaff \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Staff = "staffOne" <<
        { \staffOneVoiceOne }
        \\
        { \staffOneVoiceTwo }
        \\
        { \staffOneVoiceThree }
      >>
      \new Staff = "staffTwo" <<
        \new Voice = "staffTwoVoiceOne" { \staffTwoVoiceOne }
      >>
    >>
    \new Staff = "staffThree" \with { instrumentName = "Trombone" shortInstrumentName = "Tbn." } <<
      { \staffThreeVoiceOne }
      \\
      { \staffThreeVoiceTwo }
      \\
      { \staffThreeVoiceThree }
      \\
      { \staffThreeVoiceFour }
    >>
  >>
  \layout { }
}
