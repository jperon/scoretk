\version "2.24.0"
\language "english"

\header {
  title = "testlines"
}

\paper {
  paper-width = 210.058\mm
  paper-height = 296.926\mm
  left-margin = 15.000\mm
  right-margin = 15.000\mm
  top-margin = 15.000\mm
  bottom-margin = 15.000\mm
}

staffOneVoiceOne = \fixed c' {
  \repeat volta 2 {
    \clef treble \time 4/4
    e1 \< ^\markup { "Staff/1" } |
    g1 |
    a1 \! |
    f1 |
    g1 ^\markup { "System" } |
    d1 |
  }
  \alternative {
    {
      g1 |
    }
    {
      b1 \bar "|."
    }
  }
}

staffTwoVoiceOne = \fixed c' {
  \repeat volta 2 {
    \clef treble \transposition bf \key d \major \time 4/4
    b1 |
    e1 |
    g1 \> ^\markup { "Staff/2" } |
    e1 |
    fs1 \! |
    b1 |
  }
  \alternative {
    {
      fs1 |
    }
    {
      g1 \bar "|."
    }
  }
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Flute" shortInstrumentName = "Fl." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
    \new Staff = "staffTwo" \with { instrumentName = "B♭ Clarinet" shortInstrumentName = "B♭ Cl." } <<
      \new Voice = "staffTwoVoiceOne" { \staffTwoVoiceOne }
    >>
  >>
  \layout { }
}
