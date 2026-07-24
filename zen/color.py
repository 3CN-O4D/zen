import os
import sys
import re

_TERMUX = 'TERMUX_VERSION' in os.environ


class Color:
    def __init__(self):
        self._enabled = sys.stdout.isatty() and 'NO_COLOR' not in os.environ

    def enable(self, val=True):
        self._enabled = val

    @property
    def enabled(self):
        return self._enabled

    def _fmt(self, code, text):
        if text is None:
            return f'\033[{code}m' if self._enabled else ''
        if not self._enabled:
            return str(text)
        return f'\033[{code}m{text}\033[0m'

    def _fmt_pair(self, code1, code2, text):
        if text is None:
            return f'\033[{code1};{code2}m' if self._enabled else ''
        if not self._enabled:
            return str(text)
        return f'\033[{code1};{code2}m{text}\033[0m'

    def prompt(self, text):
        if not self._enabled:
            return str(text)
        if _TERMUX:
            return text
        return f'\001\033[0m\002{text}\001\033[0m\002'

    def reset(self, text=None):
        return self._fmt(0, text)

    def bold(self, text=None):
        return self._fmt(1, text)

    def dim(self, text=None):
        return self._fmt(2, text)

    def italic(self, text=None):
        return self._fmt(3, text)

    def underline(self, text=None):
        return self._fmt(4, text)

    def blink(self, text=None):
        return self._fmt(5, text)

    def reverse(self, text=None):
        return self._fmt(7, text)

    def hidden(self, text=None):
        return self._fmt(8, text)

    def strike(self, text=None):
        return self._fmt(9, text)

    def black(self, text=None):
        return self._fmt(30, text)

    def red(self, text=None):
        return self._fmt(31, text)

    def green(self, text=None):
        return self._fmt(32, text)

    def yellow(self, text=None):
        return self._fmt(33, text)

    def blue(self, text=None):
        return self._fmt(34, text)

    def magenta(self, text=None):
        return self._fmt(35, text)

    def cyan(self, text=None):
        return self._fmt(36, text)

    def white(self, text=None):
        return self._fmt(37, text)

    def bright_black(self, text=None):
        return self._fmt(90, text)

    def bright_red(self, text=None):
        return self._fmt(91, text)

    def bright_green(self, text=None):
        return self._fmt(92, text)

    def bright_yellow(self, text=None):
        return self._fmt(93, text)

    def bright_blue(self, text=None):
        return self._fmt(94, text)

    def bright_magenta(self, text=None):
        return self._fmt(95, text)

    def bright_cyan(self, text=None):
        return self._fmt(96, text)

    def bright_white(self, text=None):
        return self._fmt(97, text)

    def rgb(self, r, g, b, text=None):
        if not self._enabled:
            return str(text) if text is not None else ''
        if text is not None:
            return f'\033[38;2;{int(r)};{int(g)};{int(b)}m{text}\033[0m'
        return f'\033[38;2;{int(r)};{int(g)};{int(b)}m'

    def hex(self, h, text=None):
        h = str(h).lstrip('#')
        if len(h) == 3:
            h = ''.join(c * 2 for c in h)
        r, g, b = int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)
        return self.rgb(r, g, b, text)

    def strip(self, text):
        return re.sub(r'\033\[[0-9;]*m', '', str(text))


color = Color()
