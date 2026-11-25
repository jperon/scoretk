\version "2.24.0"
\language "english"

#(set-global-staff-size 17.80004409)
\header {
  title = \markup { \fontsize #24.00000 "Layout test" }
  subtitle = "Just an artificial score to test whether all elements are laid out"
  composer = \markup { \fontsize #12.00000 "Composer" }
  poet = \markup { \fontsize #12.00000 "Lyricist" }
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  subsubtitle = "Layout test"
  instrument = "Just an artificial score to test whether all elements are laid out"
  piece = "Composer"
  dedication = "Lyricist"
  tagline = "The only part"
  scoretkCreditA = "Layout test"
  scoretkCreditB = "Just an artificial score to test whether all elements are laid out"
  scoretkCreditC = "Composer"
  scoretkCreditD = "Lyricist"
  scoretkCreditE = "The only part"
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
  \clef treble \key e \major \time 4/4
  \tempo "Allegro" 4 = 144 cs4 \=3( \< \glissando ^\markup { "System Text" }
  ds4 e4 fs4 |
  \set Staff.midiMinimumVolume = #0.755906
  \set Staff.midiMaximumVolume = #0.755906
  \tuplet 3/2 {
  cs8 _\f \=3) \! ^\markup { "Staff Text" } ds8 e8
  }
  gs2.~ |
  gs4 \< _\markup { "Expression" } gs2 a8 b8 |
  \clef mezzosoprano
  \acciaccatura { g8 } fss1 \! ^\markup { "Change Instr." } \fermata \bar "||"
  a4 b4 cs'4:8 r4 |
  \clef varbaritone
  R1 |
  \break
  \key f \major \time 12/8
  \mark \markup { "A" } <c, e, g,>4-1-3-5 c,4 \glissando g,4 r4 r2 |
  \mark \markup { "B" } <c, e, g,>4-1-3-5 ^\markup { "A" } c,4 \glissando g,4
  r4 r2 \bar "|."
}

staffTwoVoiceOne = \fixed c' {
  \clef treble \key e \major \time 4/4
  \grace { fs32 } e4 \sustainOn r4 r2 |
  cs'4 \sustainOff e'4 fs'4 e'4 |
  cs'4 e'4 e2 |
  R1 \bar "||"
  R1 |
  R1 |
  \break
  \key f \major \time 12/8
  \ottava 1 a4 \ottava 0 bf4 c'4 d'4 e'4 f'4 |
  <a c' e'>1 ^\markup { "C" } ^\markup { "D" } r2 \bar "|."
}

\score {
  <<
    \new PianoStaff \with { instrumentName = "Piano" } <<
      \new Staff = "staffOne" <<
        \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
      >>
      \new Staff = "staffTwo" <<
        \new Voice = "staffTwoVoiceOne" { \staffTwoVoiceOne }
        \new Lyrics \lyricsto "staffTwoVoiceOne" {
          \skip 1 Ly -- rics __ \skip 1 ly \skip 1
          rics -- ly \skip 1 \skip 1 \skip 1 \skip 1
          \skip 1 \skip 1 \skip 1
        }
      >>
    >>
  >>
  \layout { }
}
