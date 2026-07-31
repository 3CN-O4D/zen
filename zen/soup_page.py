"""SoupPage — browserless HTTP + BeautifulSoup backend for Zen.
Duck-types DrissionPage's SessionPage enough to work with the Browser class.
"""

import re as _re
from urllib.parse import urljoin, urlencode
import requests
from bs4 import BeautifulSoup


# ── Helpers ────────────────────────────────────────────────────────────────

_DEFAULT_UA = (
    'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 '
    '(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36'
)


def _first_text(node):
    """Get the first text string inside a tag (like DrissionPage .text)."""
    return node.get_text(strip=False)


def _inner_html(tag):
    """HTML *inside* the tag (not including the tag itself)."""
    return ''.join(str(c) for c in tag.children)


def _tag_or_none(result):
    return result if result and result.name else None


# ── Element states (mimics DrissionPage's element.states.*) ────────────────

class SoupElementStates:
    def __init__(self, tag):
        self._tag = tag

    @property
    def is_alive(self):
        return True

    @property
    def is_displayed(self):
        return True

    @property
    def is_enabled(self):
        return not self._tag.get('disabled')

    @property
    def is_checked(self):
        return self._tag.get('checked') is not None


# ── Element ───────────────────────────────────────────────────────────────

class SoupElement:
    """Wraps a bs4 Tag to look like a DrissionPage element."""

    def __init__(self, tag, page=None):
        self._tag = tag
        self._page = page

    # ── properties ────────────────────────────────────────────────────────

    @property
    def text(self):
        return _first_text(self._tag)

    @property
    def html(self):
        return _inner_html(self._tag)

    @property
    def tag(self):
        return self._tag.name

    @property
    def attrs(self):
        return dict(self._tag.attrs)

    def attr(self, name):
        val = self._tag.get(name)
        if isinstance(val, list):
            return ' '.join(val)
        return val

    # ── states ────────────────────────────────────────────────────────────

    @property
    def states(self):
        return SoupElementStates(self._tag)

    # ── navigation / interaction ──────────────────────────────────────────

    def click(self):
        if self._page is None:
            return
        # Link → follow href
        if self._tag.name == 'a':
            href = self._tag.get('href')
            if href:
                self._page.get(urljoin(self._page.url, href))
            return
        # Submit button → find & submit parent form
        if self._tag.name in ('button', 'input') and self._tag.get('type') in ('submit', 'image'):
            form = self._tag.find_parent('form')
            if form:
                self._submit_form(form, clicked=self._tag)
            return
        # Any element inside a link → follow parent link
        parent_a = self._tag.find_parent('a')
        if parent_a is not None:
            href = parent_a.get('href')
            if href:
                self._page.get(urljoin(self._page.url, href))
            return

    def _submit_form(self, form, clicked=None):
        action = form.get('action', '')
        method = form.get('method', 'get').lower()
        url = urljoin(self._page.url, action)

        data = {}
        for el in form.find_all(['input', 'textarea', 'select']):
            name = el.get('name')
            if not name:
                continue
            if el.get('disabled'):
                continue
            t = (el.get('type') or '').lower()
            if t in ('reset', 'file'):
                continue
            if t in ('submit', 'image') and el is not clicked:
                continue  # skip other submit buttons
            if t == 'checkbox':
                if el.get('checked'):
                    data[name] = el.get('value', 'on')
            elif t == 'radio':
                if el.get('checked'):
                    data[name] = el.get('value', 'on')
            elif el.name == 'select':
                sel = el.find('option', selected=True)
                if sel:
                    data[name] = sel.get('value') or (sel.string or '')
                else:
                    first = el.find('option')
                    if first:
                        data[name] = first.get('value') or (first.string or '')
            else:
                data[name] = el.get('value', '')

        # Include clicked button's value
        if clicked is not None:
            name = clicked.get('name')
            if name and name not in data:
                data[name] = clicked.get('value', '')

        if method == 'post':
            resp = self._page._session.post(url, data=data)
        else:
            resp = self._page._session.get(url, params=data)

        self._page._update(resp)

    # ── form-filling ──────────────────────────────────────────────────────

    def input(self, value):
        if self._tag.name in ('input', 'textarea'):
            # For checkboxes/radios, set checked
            t = (self._tag.get('type') or '').lower()
            if t in ('checkbox', 'radio'):
                self._tag['checked'] = True
                self._tag['value'] = str(value)
            else:
                self._tag['value'] = str(value)
        elif self._tag.name == 'select':
            self._tag['value'] = str(value)
            # Also mark the matching <option> as selected
            for opt in self._tag.find_all('option'):
                val = opt.get('value') or (opt.string or '')
                if val == str(value) or (opt.string or '') == str(value):
                    opt['selected'] = True
                else:
                    del opt['selected']

    def check(self, uncheck=False):
        if self._tag.name == 'input' and (self._tag.get('type') or '').lower() in ('checkbox', 'radio'):
            if uncheck:
                del self._tag['checked']
            else:
                self._tag['checked'] = True

    @property
    def select(self):
        return _SoupSelectProxy(self._tag)

    # ── child search ──────────────────────────────────────────────────────

    def ele(self, selector, timeout=0):
        try:
            el = self._tag.select_one(selector)
            return SoupElement(_tag_or_none(el), self._page) if el else None
        except Exception:
            return None

    def eles(self, selector, timeout=0):
        try:
            els = self._tag.select(selector)
            return [SoupElement(el, self._page) for el in els]
        except Exception:
            return []

    # ── unsupported ───────────────────────────────────────────────────────

    def run_js(self, js):
        raise NotImplementedError("JavaScript execution not available in soup mode")

    def get_screenshot(self, path, full_page=False):
        raise NotImplementedError("Screenshots not available in soup mode")

    def hover(self):
        pass  # no-op

    def __repr__(self):
        txt = (self.text or '')[:60]
        return f'<SoupElement {self._tag.name}: {txt!r}>'


class _SoupSelectProxy:
    """Mimics DrissionPage's element.select sub-object."""

    def __init__(self, tag):
        self._tag = tag

    def by_text(self, text):
        for opt in self._tag.find_all('option'):
            if (opt.string or '') == str(text):
                opt['selected'] = True
                return

    def by_value(self, value):
        for opt in self._tag.find_all('option'):
            if opt.get('value') == str(value):
                opt['selected'] = True
                return

    def by_index(self, index):
        opts = self._tag.find_all('option')
        idx = int(index)
        if 0 <= idx < len(opts):
            opts[idx]['selected'] = True


# ── Page ──────────────────────────────────────────────────────────────────

class SoupPage:
    """HTTP + BeautifulSoup page, duck-typing DrissionPage's SessionPage."""

    def __init__(self):
        self._session = requests.Session()
        self._session.headers.update({'User-Agent': _DEFAULT_UA})
        self._soup = None
        self._url = ''
        self._history = []

    # ── navigation ────────────────────────────────────────────────────────

    def get(self, url):
        url = str(url)
        resp = self._session.get(url, timeout=30)
        resp.raise_for_status()
        self._update(resp)
        return self

    def _update(self, resp):
        self._url = resp.url
        self._history.append(resp.url)
        self._soup = BeautifulSoup(resp.text, 'lxml')

    # ── properties ────────────────────────────────────────────────────────

    @property
    def title(self):
        if self._soup and self._soup.title and self._soup.title.string:
            return self._soup.title.string.strip()
        return ''

    @property
    def url(self):
        return self._url

    @property
    def html(self):
        return str(self._soup) if self._soup else ''

    # ── element finding (DrissionPage API) ───────────────────────────────

    def ele(self, selector, timeout=0):
        if self._soup is None:
            return None
        selector = str(selector)
        # text: prefix → text search
        if selector.startswith('text:'):
            text = selector[5:]
            tag = self._soup.find(string=_re.compile(_re.escape(text), _re.I))
            return SoupElement(tag.parent, self) if tag and tag.parent else None
        if selector.startswith('text='):
            text = selector[5:]
            tag = self._soup.find(string=text)
            return SoupElement(tag.parent, self) if tag and tag.parent else None
        # c: prefix → CSS selector
        if selector.startswith('c:'):
            selector = selector[2:]
        # @attr=value
        if selector.startswith('@'):
            m = _re.match(r'^@(\w+)=["\']?(.+?)["\']?$', selector)
            if m:
                attr, val = m.group(1), m.group(2)
                tag = self._soup.find(attrs={attr: val})
                return SoupElement(_tag_or_none(tag), self) if tag else None
        # CSS selector
        try:
            el = self._soup.select_one(selector)
            return SoupElement(_tag_or_none(el), self) if el else None
        except Exception:
            return None

    def eles(self, selector, timeout=0):
        if self._soup is None:
            return []
        selector = str(selector)
        if selector.startswith('text:'):
            text = selector[5:]
            tags = self._soup.find_all(string=_re.compile(_re.escape(text), _re.I))
            return [SoupElement(t.parent, self) for t in tags if t and t.parent]
        if selector.startswith('text='):
            text = selector[5:]
            tags = self._soup.find_all(string=text)
            return [SoupElement(t.parent, self) for t in tags if t and t.parent]
        if selector.startswith('c:'):
            selector = selector[2:]
        if selector.startswith('@'):
            m = _re.match(r'^@(\w+)=["\']?(.+?)["\']?$', selector)
            if m:
                attr, val = m.group(1), m.group(2)
                tags = self._soup.find_all(attrs={attr: val})
                return [SoupElement(t, self) for t in tags]
        try:
            els = self._soup.select(selector)
            return [SoupElement(el, self) for el in els]
        except Exception:
            return []

    # ── page-level operations ─────────────────────────────────────────────

    def cookies(self):
        return {k: v for k, v in self._session.cookies.items()}

    def quit(self):
        self._session.close()

    def close(self):
        self._session.close()

    def refresh(self):
        if self._url:
            self.get(self._url)

    # ── sub-objects that DrissionPage exposes ─────────────────────────────

    class _set:
        """Duck-types page.set for headers etc."""
        def __init__(self, outer):
            self._outer = outer

        def headers(self, headers):
            self._outer._session.headers.update(headers)

    @property
    def set(self):
        return self._set(self)

    # ── unsupported DrissionPage APIs (raise clear errors) ────────────────

    def run_js(self, js, *args):
        raise NotImplementedError("JavaScript execution requires a browser")

    def get_screenshot(self, path, full_page=False):
        raise NotImplementedError("Screenshots require a browser")

    @property
    def wait(self):
        raise NotImplementedError("Wait operations require a browser")

    @property
    def scroll(self):
        raise NotImplementedError("Scroll requires a browser")

    def back(self):
        if len(self._history) >= 2:
            self._history.pop()
            prev = self._history.pop()
            self.get(prev)

    def forward(self):
        raise NotImplementedError("Forward navigation not supported in soup mode")
