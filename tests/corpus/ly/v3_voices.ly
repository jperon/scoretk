\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = \markup { \bold \fontsize #24.00000 "voices" }
  workTitle = "voices"
  composer = \markup { \fontsize #12.00000 "" }
  poet = \markup { \fontsize #12.00000 "" }
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  subsubtitle = "voices"
  scoretkCreditA = "voices"
}

\paper {
  paper-width = 210.05800\mm
  paper-height = 296.92600\mm
  left-margin = 10.00000\mm
  right-margin = 10.00000\mm
  top-margin = 10.00000\mm
  bottom-margin = 20.00000\mm
}

staffOneVoiceOne = \fixed c' {
  \voiceOne
  \clef treble \key af \major \time 4/4
  \set Staff.midiMinimumVolume = #0.259843
  \set Staff.midiMaximumVolume = #0.259843 R1 _\pp |
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
  s2 |
  s2 |
  s2 |
  s1 \bar "|."
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
  s1 |
  s1 |
  s1 |
  s2 |
  s2 |
  s2 |
  s1 \bar "|."
}

staffThreeVoiceThree = \fixed c' {
  \voiceThree
  s1 |
  s1 |
  s1 |
  s2 |
  s2 |
  s2 |
  s1 \bar "|."
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
