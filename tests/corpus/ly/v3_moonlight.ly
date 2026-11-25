\version "2.24.0"
\language "english"

#(set-global-staff-size 17.80004409)
\header {
  title = "Sonata No. 14, &quot;Moonlight&quot;"
  subtitle = "Quasi una fantasia"
  composer = "Ludwig van Beethoven"
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  scoretkCreditA = "Sonata No. 14, &quot;Moonlight&quot;"
  scoretkCreditB = "Ludwig van Beethoven"
  scoretkCreditC = "Quasi una fantasia"
}

\paper {
  paper-width = 210.058\mm
  paper-height = 296.926\mm
  left-margin = 10.000\mm
  right-margin = 10.000\mm
  top-margin = 10.000\mm
  bottom-margin = 20.000\mm
}

staffOneVoiceOne = \fixed c' {
  \voiceOne
  \clef treble \key e \major \time 4/4
  \tempo "Adagio sostenuto" 4 = 54
  \tuplet 3/2 {
    gs,8 ^\markup { "Si deve suonare tutto questo pezzo delicatissimamente e senza sordini" }
  cs8 e8
  }
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 \=1( cs8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 cs8 e8 \=1)
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 \=2( d8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 d8 fs8 \=2)
  }
  \revert TupletNumber.stencil
  |
  \break
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 \=3( bs,8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 ds?8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs,8 bs,8 ds8 \=3)
  }
  \revert TupletNumber.stencil
  |
  \once \override Rest.transparent = ##t r4
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 \=4( cs8 e8 \=4)
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 \=5( cs8 e8 \=5)
  }
  \revert TupletNumber.stencil
  \set Staff.midiMinimumVolume = #0.259843
  \set Staff.midiMaximumVolume = #0.259843
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 _\pp \=6( cs8 e8 \=6)
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 cs8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 cs8 fs8
  }
  \revert TupletNumber.stencil
  |
  \break
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 b,8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 b,8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 b,8 ds8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 b,8 ds8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 b,8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 b,8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 b,8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 b,8 e8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  g,8 b,8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  g,8 b,8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  g,8 b,8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  g,8 b,8 e8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  g,8 b,8 f8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  g,8 b,8 f8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  g,8 b,8 f8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  g,8 b,8 f8
  }
  \revert TupletNumber.stencil
  |
  \break
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  g,8 c8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  g,8 b,8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  g,8 cs8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs,?8 cs8 e8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs,8 b,8 d8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs,8 b,8 d8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  g,8 b,8 cs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e,8 b,8 cs8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs,8 b,8 d8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs,8 b,8 d8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs,8 as,8 cs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs,8 as,8 cs8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 d8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 d8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  |
  \break
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 \< e8 g8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 e8 g8 \!
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 \> e8 g8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 e8 g8 \!
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 ds?8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 \< e8 g8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 e8 g8 \!
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 \> e8 g8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 e8 g8 \!
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 ds?8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 d8 es8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 d8 es8
  }
  \revert TupletNumber.stencil
  |
  \break
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 cs8 gs?8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 cs8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 cs8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 cs8 fs8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  g,8 b,8 d8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  g,8 b,8 d8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs,8 a,8 ds8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs,8 a,8 ds8
  }
  \revert TupletNumber.stencil
  |
  \clef treble
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs,8 fs,8 a,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs,8 fs,8 a,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs,8 fs,8 gs,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs,8 es,8 gs,8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs,8 a,8 cs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 cs8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 fs8 a8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 gs8 b8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 gs8 b8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 gs8 b8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 gs8 b8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 ^\markup { "cresc." } fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  bs,8 fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 fs8 a8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds8 fs8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds8 fs8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds8 fs8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds8 fs8 gs8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e8 gs8 cs'8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e8 gs8 cs'8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds8 ^\markup { "decresc." } fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 e8 as8
  }
  \revert TupletNumber.stencil
  |
  \break
  \set Staff.midiMinimumVolume = #0.385827
  \set Staff.midiMaximumVolume = #0.385827
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  bs8 _\p \< bs,8 ds8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs8 bs,8 \! ds8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a8 \> bs,8 ds8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs8 bs,8 ds8 \!
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  r8 bs,8 \< ds8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 bs,8 ds8 \!
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 \> bs,8 ds8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs,8 bs,8 ds8 \!
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e,8 e8 \=26( \< gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs'8 e8 gs8 \!
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e'8 \> e8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs'8 \=26) e8 gs8 \!
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  r8 e,8 gs,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 \< e,8 gs,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e8 e,8 \! \> gs,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 e,8 gs,8 \!
  }
  \revert TupletNumber.stencil
  |
  \break
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds,8 \=30( a,8 fs,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  bs,8 a,8 ds8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  bs,8 fs8 ds8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a8 fs8 bs8 \=30)
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e,8 \=31( cs8 gs,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e8 cs8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e8 cs'8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e'8 cs'8 gs8 \=31)
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 \=32( fss8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  as8 fss8 cs'8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  as8 e'8 cs'8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fss'8 e'8 as'8 \=32)
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs?8 \=33( bs8 a?8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds'?8 bs8 fs'8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds'8 a'8 fs'8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  bs'8 a'8 ds''?8 \=33)
  }
  \revert TupletNumber.stencil
  |
  \break
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  bs'8 \=34( fs'8 a'8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds'8 fs'8 bs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds'8 a8 bs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs8 a8 ds8
  }
  \revert TupletNumber.stencil
  |
  \clef bass
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs8 bs,8 ds8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 bs,?8 fs,8 \=34)
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 \=35( ds,8 fs,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs,8 fs,8 a,8 \=35)
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  bs,,8 fs,8 gs,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 gs,8 fs,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds,8 fs,8 a,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs,8 fs,8 a,8
  }
  \revert TupletNumber.stencil
  |
  \break
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  bs,,8 fs,8 gs,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 gs,8 fs,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds,8 fs,8 a,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs,8 fs,8 a,8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  bs,,8 ^\markup { "decresc." } fs,8 gs,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 gs,8 fs,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs,8 e,8 cs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs,8 e,8 cs8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds,8 a,8 cs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds,8 a,8 cs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds,8 gs,8 bs,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds,8 fs,8 bs,8
  }
  \revert TupletNumber.stencil
  |
  \break
  \clef treble
  \set Staff.midiMinimumVolume = #0.259843
  \set Staff.midiMaximumVolume = #0.259843
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e,8 _\pp gs,8 cs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \revert TupletNumber.stencil
  \set Staff.midiMinimumVolume = #0.259843
  \set Staff.midiMaximumVolume = #0.259843
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 _\pp cs8 e8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 cs8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 cs8 fs8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 b,8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 b,8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 b,8 ds8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 b,8 ds8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 b,8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 e8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 e8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 e8 gs8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 fs8 a8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 ^\markup { "cresc." } e8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 e8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  bs,8 fs8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 e8 gs8
  }
  \revert TupletNumber.stencil
  |
  \set Staff.midiMinimumVolume = #0.385827
  \set Staff.midiMaximumVolume = #0.385827
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds8 _\p fs8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds8 fs8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e8 gs8 cs'8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e8 gs8 cs'8
  }
  \revert TupletNumber.stencil
  |
  \break
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  d8 fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  d8 fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  bs,8 fs8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  bs,8 fs8 gs8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 e8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 e8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 es8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 es8 gs8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 \< fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 fs8 a8 \!
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 \> fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 fs8 a8 \!
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 es8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 es8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 es8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 es8 gs8
  }
  \revert TupletNumber.stencil
  |
  \break
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 \< fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 fs8 a8 \!
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 \> fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 fs8 a8 \!
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 es8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 es8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 fs8 a8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,?8 fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  b,8 e?8 gs8
  }
  \revert TupletNumber.stencil
  |
  \break
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 e8 gs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs,8 ^\markup { "cresc." } cs8 ds8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs,8 cs8 ds8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 ds8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 cs8 ds8
  }
  \revert TupletNumber.stencil
  |
  \set Staff.midiMinimumVolume = #0.385827
  \set Staff.midiMaximumVolume = #0.385827
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 _\p cs8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs,8 bs,8 ds8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  fs,8 bs,8 ds8
  }
  \revert TupletNumber.stencil
  |
  \set Staff.midiMinimumVolume = #0.259843
  \set Staff.midiMaximumVolume = #0.259843
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e,8 _\pp gs,8 cs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 cs8 e8
  }
  \revert TupletNumber.stencil
  |
  \break
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 ds8 fs8
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 \=50( \< e8 cs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs8 e8 cs'8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs8 e'8 cs'8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs'8 e'8 cs'8 \!
  }
  \revert TupletNumber.stencil
  |
  \clef bass
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  bs8 \> ds'8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  bs8 fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds8 fs8 a,8~
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 gs,8 fs,8 \=50) \!
  }
  \revert TupletNumber.stencil
  |
  \clef treble
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  <e, cs>8 \=53( \< e8 cs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs8 e8 cs'8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs8 e'8 cs'8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs'8 e'8 cs'8 \!
  }
  \revert TupletNumber.stencil
  |
  \break
  \clef bass
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  bs8 \> ds'8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  bs8 fs8 a8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  ds8 fs8 a,8~
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  a,8 gs,8 fs,8 \=53) \!
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  <e, cs>8 \=54( gs,8 cs8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e8 cs8 gs,8 \=54)
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  r8 e,8 \=55( gs,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  cs8 gs,8 e,8 \=55)
  }
  \revert TupletNumber.stencil
  |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  r8 cs,8 \=56( e,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,8 e,8 cs,8 \=56)
  }
  \revert TupletNumber.stencil
  r2 |
  r2 \set Staff.midiMinimumVolume = #0.259843
  \set Staff.midiMaximumVolume = #0.259843 <e, gs, cs>2 _\pp |
  <e, gs, cs>1 \fermata \bar "|."
}

staffOneVoiceTwo = \fixed c' {
  \voiceTwo
  s1 |
  s1 |
  s1 |
  s1 |
  \once \override Rest.transparent = ##t r2 r4 gs8. gs16 |
  \once \stemUp gs2. gs8. gs16 |
  \once \stemUp gs2 \=7( \once \stemUp a2 |
  \once \stemUp gs2 \once \stemUp fs4 \once \stemUp b4 \=7) |
  \once \stemUp e4 r4 r2 |
  r2 r4 g8. g16 |
  \once \stemUp g2. g8. g16 |
  \once \stemUp g2. \=8( \once \stemUp fs?4 \=8) |
  \once \stemUp fs2 \=9( \once \stemUp g4 \once \stemUp e4 \=9) |
  \once \stemUp fs2 \once \stemUp fs2 |
  \once \stemUp b,4 r4 r4 \once \stemUp b4 \=12( |
  \once \stemUp c'2. \once \stemUp as4 \=12) |
  \once \stemUp b2. \once \stemUp b4 \=15( |
  \once \stemUp c'2. \once \stemUp as4 \=15) |
  \once \stemUp b2 \once \stemUp b2 |
  \once \stemUp b2 \once \stemUp a?2 |
  \once \stemUp g2 \=16( \once \stemUp fs2 \=16) |
  \once \stemUp cs2 \once \stemUp cs4 \once \stemUp cs4 |
  r2 r4 cs'8. cs'16 |
  \once \stemUp cs'2. cs'8. cs'16 |
  \once \stemUp cs'2 \=17( \once \stemUp bs4 \once \stemUp cs'4 \=17) |
  \once \stemUp ds'2. \once \stemUp ds'4 |
  \once \stemUp e'2 \=18( \once \stemUp ds'4 \once \stemUp cs'4 \=18) |
  s4 \once \stemUp gs4 \=21( \once \stemUp a4 \once \stemUp fs4 \=21) |
  s4 \once \stemUp gs,4 \=24( \once \stemUp a,4 \once \stemUp fs,4 \=24) |
  s4 \once \stemUp cs'4 \once \stemUp e'4 \once \stemUp cs'4 |
  s4 \once \stemUp cs4 \once \stemUp e4 \once \stemUp cs4 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  \once \override Rest.transparent = ##t r2. cs,4 |
  bs,,2 ds,4 \=36( cs,4 \=36) |
  bs,,2 d,4 \=37( cs,4 \=37) |
  bs,,2 s2 |
  s1 |
  s2 s4 \once \stemUp gs8. gs16 |
  \once \stemUp gs2. \once \stemUp gs8. gs16 |
  \once \stemUp gs2 \=38( \once \stemUp a2 |
  \once \stemUp gs2 \once \stemUp fs4 \once \stemUp b4 \=38) |
  \once \stemUp e4 r4 r4 \once \stemUp b8. b16 |
  \once \stemUp b2. \once \stemUp b8. b16 |
  \once \stemUp b2 \=39( \once \stemUp bs4 \once \stemUp cs'4 \=39) |
  \once \stemUp ds'2 \=40( \once \stemUp e'2 |
  \once \stemUp d'2 \once \stemUp bs2 \=40) |
  \once \stemUp cs'2. \once \stemUp cs'4 \=43( |
  \once \stemUp d'2. \once \stemUp bs4 \=43) |
  \once \stemUp cs'2. \once \stemUp cs'4 \=46( |
  \once \stemUp d'2. \once \stemUp bs4 \=46) |
  \once \stemUp cs'2 \once \stemUp cs'2 |
  \once \stemUp b?2. \once \stemUp b4 |
  \once \stemUp a4 \once \stemUp a4 \once \stemUp gs4 \once \stemUp gs4 |
  \once \stemUp fs2 \=47( \once \stemUp gs4 \once \stemUp a4 \=47) |
  \once \stemUp gs2 \once \stemUp gs2 |
  \once \stemUp cs4 s4 s2 |
  s1 |
  s1 |
  \once \override Rest.transparent = ##t r2. \once \stemUp bs,4 |
  s1 |
  \once \override Rest.transparent = ##t r2. \once \stemUp bs,4 |
  s1 |
  s1 |
  s1 |
  s1 \bar "|."
}

staffTwoVoiceOne = \fixed c' {
  \voiceOne
  \clef bass \key e \major \time 4/4
  <cs,, cs,>1 |
  <b,,, b,,>1 |
  <a,,, a,,>2 <fs,,, fs,,>2 |
  \break
  <gs,,, gs,,>2 <gs,,, gs,,>2 |
  <cs,, gs,, cs,>1 |
  <bs,,, gs,, bs,,>1 |
  <cs,, cs,>2 \=58( \=59( <fs,,, fs,,>2 \=58) \=59) |
  \break
  <b,,, b,,>2 <b,,, b,,>2 |
  <e,, e,>1 |
  <e,, e,>1 |
  <d,, d,>1 |
  \break
  <c,, c,>4 \=60( <b,,, b,,>4 <as,,, as,,>2 \=60) |
  \once \stemDown <b,,, b,,>2 \=61( \once \stemDown e,,4
  \once \stemDown g,,4 \=61) |
  \once \stemDown fs,,2 \once \stemDown <fs,,, fs,,>2 |
  <b,,,~ b,,~>1 |
  \break
  <b,,, b,,>4 \=62( \once \stemDown <e,, e,>4 \once \stemDown <g,, g,>4
  \once \stemDown <e,, e,>4 \=62) |
  <b,,,~ b,,~>1 |
  <b,,, b,,>4 \=63( \once \stemDown <e,, e,>4 \once \stemDown <g,, g,>4
  \once \stemDown <e,, e,>4 \=63) |
  <b,,, b,,>2 <gs,,,? gs,,?>2 |
  \break
  <es,,, es,,>2 <fs,,, fs,,>2 |
  \once \stemDown <b,,, b,,>2 \once \stemDown <bs,,, bs,,>2 |
  \once \stemDown cs,,2 \once \stemDown cs,,2 |
  <fs,,, cs,, fs,,>1 |
  <es,, cs, es,>1 |
  <fs,, fs,>2 \=64( <ds,, ds,>4 <cs,, cs,>4 \=64) |
  <bs,,, gs,, bs,,>2. <bs,,, gs,, bs,,>4 |
  <cs,, gs,, cs,>2 <fs,,, fs,,>4 \=65( <fss,,, fss,,>4 \=65) |
  \break
  <gs,,, gs,,>1 |
  <gs,,, gs,,>1 |
  <gs,,, gs,,>1 |
  <gs,,, gs,,>1 |
  \break
  <gs,,, gs,,>1 |
  <gs,,, gs,,>1 |
  <gs,,, gs,,>1 |
  <gs,,,~ gs,,~>1 |
  \break
  <gs,,,~ gs,,~>1 |
  <gs,,, gs,,>1 |
  <gs,,, gs,,>1 |
  \break
  <gs,,, gs,,>1 |
  <gs,,, gs,,>2 \=66( <a,,, a,,>2 |
  <fs,,, fs,,>2 <gs,,, gs,,>2 \=66) |
  \break
  <cs,, gs,, cs,>1 |
  <bs,,, gs,, bs,,>1 |
  <cs,, cs,>2 \=67( <fs,,, fs,,>2 \=67) |
  <b,,, b,,>2 <b,,, b,,>2 |
  <e,, e,>1 |
  <ds,, ds,>1 |
  <e,, e,>2 <ds,, ds,>4 <cs,, cs,>4 |
  <bs,,, gs,, bs,,>2 \=68( <cs,, gs,, cs,>2 \=68) |
  \break
  <fs,,, fs,,>2 \=69( <gs,,, gs,,>2 \=69) |
  <cs,,~ cs,~>1 |
  <cs,, cs,>4 \=70( \once \stemDown <fs,, fs,>4 \once \stemDown <a,, a,>4
  \once \stemDown <fs,, fs,>4 \=70) |
  <cs,,~ cs,~>1 |
  \break
  <cs,, cs,>4 \=71( \once \stemDown <fs,, fs,>4 \once \stemDown <a,, a,>4
  \once \stemDown <fs,, fs,>4 \=71) |
  <cs,, cs,>2 <fs,,, fs,,>2 \=72( \=73( |
  <ds,, ds,>2. \=72) \=73) <e,, e,>4 |
  \break
  <cs,, cs,>4 \=74( <ds,, ds,>4 <bs,,, bs,,>4 <cs,, cs,>4 \=74) |
  \once \stemDown <a,,, a,,>2 \=75( \once \stemDown <gs,,, gs,,>4
  \once \stemDown <fs,,, fs,,>4 \=75) |
  \once \stemDown <gs,,, gs,,>2 \once \stemDown <gs,,, gs,,>2 |
  cs,,1 |
  \break
  bs,,,1 |
  cs,,1 |
  gs,,,1 |
  cs,,1 |
  \break
  gs,,,1 |
  cs,,1~ |
  cs,,1 |
  \set Staff.midiMinimumVolume = #0.259843
  \set Staff.midiMaximumVolume = #0.259843 cs,,2 _\pp <cs,, gs,, cs,>2 |
  <cs,, gs,, cs,>1 \fermata \bar "|."
}

staffTwoVoiceTwo = \fixed c' {
  \voiceTwo
  s1 |
  s1 |
  s1 |
  s1 |
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e,8 \=57( gs,8 cs8 \=57)
  }
  \revert TupletNumber.stencil
  s4 s2 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  s1 |
  \once \stemUp gs,,2. \once \stemUp gs,,8. gs,,16 |
  \once \stemUp gs,,2. \once \stemUp gs,,8. gs,,16 |
  \once \stemUp gs,,2. \once \stemUp gs,,8. gs,,16 |
  \once \stemUp gs,,2. \once \stemUp gs,,8. gs,,16 |
  \once \stemUp gs,,2. \once \stemUp gs,,8. gs,,16 |
  \once \stemUp gs,,2. gs,,8. gs,,16 |
  \once \stemUp gs,,2 \once \stemUp cs,2 |
  \once \stemUp gs,,2
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  gs,,8 \=76( cs,8 gs,,8
  }
  \revert TupletNumber.stencil
  \override TupletNumber.stencil = ##f
  \tuplet 3/2 {
  e,,8 gs,,8 e,,8 \=76)
  }
  \revert TupletNumber.stencil
  |
  s1 |
  s1 \bar "|."
}

\score {
  <<
    \new PianoStaff \with { instrumentName = "Piano" } <<
      \new Staff = "staffOne" <<
        { \staffOneVoiceOne }
        \\
        { \staffOneVoiceTwo }
      >>
      \new Staff = "staffTwo" <<
        { \staffTwoVoiceOne }
        \\
        { \staffTwoVoiceTwo }
      >>
    >>
  >>
  \layout { }
}
