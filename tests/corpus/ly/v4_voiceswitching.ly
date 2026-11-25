\version "2.24.0"
\language "english"

\paper {
  paper-width = 210.058\mm
  paper-height = 296.926\mm
  left-margin = 15.000\mm
  right-margin = 15.000\mm
  top-margin = 15.000\mm
  bottom-margin = 15.000\mm
}

staffOneVoiceOne = \fixed c' {
  \voiceOne
  \clef treble \key c \major \time 4/4
  c'2 ^\markup { "This test goes through each measure, moves note input to the segment/voice of the &quot;plus&quot; notehead, and  switches voices. After performing this action, the note input is expected to move to the segment/voice of the &quot;cross&quot; notehead." } ^\markup { "No tuplets:" }
  c'2 |
  c'2 <\tweak style #'cross f c'>2 |
  \break
  \tuplet 3/2 {
    \once \stemUp c'2 ^\markup { "Tuplets in voice 1:" } \once \stemUp c'2
  \once \stemUp c'2
  }
  |
  \once \stemUp c'2
  \tuplet 3/2 {
    \once \stemUp \tweak style #'cross c'4
    \tuplet 3/2 {
    \once \stemUp c'8 \once \stemUp c'8 \once \stemUp c'8
    }
  \once \stemUp c'4
  }
  |
  \break
  f4 ^\markup { "Tuplets in voice 2:" } f2 f4 |
  f2 f2 |
  \break
  \tuplet 3/2 {
    \tweak style #'cross e'2 ^\markup { "Tuplets in voices 1 and 2:" }
    \tuplet 3/2 {
    e'4 e'4 e'4
    }
  e'2
  }
  |
  e'2
  \tuplet 3/2 {
    e'4
    \tuplet 3/2 {
    e'8 e'8 e'8
    }
  e'4
  }
  \bar "|."
}

staffOneVoiceTwo = \fixed c' {
  \voiceTwo
  f2 \tweak style #'cross f2 |
  s1 |
  \tweak style #'cross f4 f2 f4 |
  f4 f2 f4 |
  \once \stemUp c'2
  \tuplet 3/2 {
  \once \stemUp \tweak style #'cross c'4 \once \stemUp c'4 \once \stemUp c'4
  }
  |
  \tuplet 3/2 {
    \once \stemUp \tweak style #'cross c'2
    \tuplet 3/2 {
      \once \stemUp c'4
      \tuplet 3/2 {
      \once \stemUp c'8 \once \stemUp c'8 \once \stemUp c'8
      }
    \once \stemUp c'4
    }
  \once \stemUp c'2
  }
  |
  \tuplet 4/4 {
    a4
    \tuplet 4/4 {
    a8 a8 a8 a8
    }
  a4
  }
  |
  a4
  \tuplet 4/4 {
    a8 a8
    \tuplet 4/4 {
    \tweak style #'cross a16 a16 a16 a16
    }
  }
  a4 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      { \staffOneVoiceOne }
      \\
      { \staffOneVoiceTwo }
    >>
  >>
  \layout { }
}
