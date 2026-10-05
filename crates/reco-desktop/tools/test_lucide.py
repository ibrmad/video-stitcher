"""Tests for lucide_icons.py (standard library only):
/usr/bin/python3 crates/reco-desktop/tools/test_lucide.py"""
import unittest

import lucide_icons

PLAY = """<!-- @license lucide-static v1.52.0 - ISC -->
<svg
  class="lucide lucide-play"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z" />
</svg>
"""


class Convert(unittest.TestCase):
    def test_paints_in_black_for_the_widget_to_tint(self):
        out = lucide_icons.convert(PLAY, filled=False)
        self.assertNotIn("currentColor", out)
        self.assertIn('stroke="#000"', out)

    def test_pins_the_whole_box(self):
        out = lucide_icons.convert(PLAY, filled=False)
        self.assertIn(lucide_icons.PIN, out)
        # The pin comes first, so nothing is drawn under it.
        self.assertLess(out.index(lucide_icons.PIN), out.index("<path"))

    def test_keeps_the_licence_line_and_the_shape(self):
        out = lucide_icons.convert(PLAY, filled=False)
        self.assertIn("@license lucide-static v1.52.0 - ISC", out)
        self.assertIn('d="M5 5a2 2 0 0 1 3.008-1.728', out)
        self.assertIn('fill="none"', out)

    def test_a_filled_icon_fills_its_shapes(self):
        out = lucide_icons.convert(PLAY, filled=True)
        self.assertIn('<path fill="#000" d=', out)
        # The pin stays empty.
        self.assertIn(lucide_icons.PIN, out)


if __name__ == "__main__":
    unittest.main()
