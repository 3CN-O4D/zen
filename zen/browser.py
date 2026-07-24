import os
import re as _re
import time as _time
from .environment import ZenElement, ZenSelector, ZenList, ZenRegexMatch, ZenError

_URL_RE = _re.compile(r'^[a-zA-Z][a-zA-Z0-9+\-.]*://')

_config = {
    'browser_path': None,
    'headless': True,
    'timeout': 30000,
}

def get_config():
    return _config

def set_config(key, value):
    if key in _config:
        _config[key] = value

def _ensure_scheme(url):
    s = url.strip()
    if not _URL_RE.match(s):
        s = 'https://' + s
    return s

import platform as _platform

_BROWSER_PATHS = {
    'linux': [
        '/usr/bin/chromium',
        '/usr/bin/chromium-browser',
        '/usr/bin/google-chrome',
        '/usr/bin/google-chrome-stable',
        '/usr/bin/brave-browser',
        '/usr/bin/brave',
        '/usr/bin/edge',
        '/usr/bin/msedge',
        '/usr/bin/vivaldi',
        '/usr/bin/opera',
        '/snap/bin/chromium',
        '/snap/bin/google-chrome',
        '/data/data/com.termux/files/usr/bin/chromium-browser',
        '/data/data/com.termux/files/usr/bin/chromium',
        '/data/data/com.termux/files/usr/bin/google-chrome',
        '/data/data/com.termux/files/usr/bin/brave',
    ],
    'darwin': [
        '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
        '/Applications/Chromium.app/Contents/MacOS/Chromium',
        '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser',
        '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
        '/Applications/Vivaldi.app/Contents/MacOS/Vivaldi',
        '/Applications/Opera.app/Contents/MacOS/Opera',
        os.path.expanduser('~/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'),
        os.path.expanduser('~/Applications/Chromium.app/Contents/MacOS/Chromium'),
        os.path.expanduser('~/Applications/Brave Browser.app/Contents/MacOS/Brave Browser'),
    ],
    'windows': [
        os.path.expandvars(r'%PROGRAMFILES%\Google\Chrome\Application\chrome.exe'),
        os.path.expandvars(r'%PROGRAMFILES(X86)%\Google\Chrome\Application\chrome.exe'),
        os.path.expandvars(r'%LOCALAPPDATA%\Google\Chrome\Application\chrome.exe'),
        os.path.expandvars(r'%PROGRAMFILES%\Chromium\Application\chrome.exe'),
        os.path.expandvars(r'%PROGRAMFILES%\BraveSoftware\Brave-Browser\Application\brave.exe'),
        os.path.expandvars(r'%LOCALAPPDATA%\BraveSoftware\Brave-Browser\Application\brave.exe'),
        os.path.expandvars(r'%PROGRAMFILES%\Microsoft\Edge\Application\msedge.exe'),
        os.path.expandvars(r'%PROGRAMFILES(X86)%\Microsoft\Edge\Application\msedge.exe'),
        os.path.expandvars(r'%PROGRAMFILES%\Vivaldi\Application\vivaldi.exe'),
        os.path.expandvars(r'%PROGRAMFILES%\Opera\launcher.exe'),
    ],
}

def _find_browser_path():
    env_vars = ['ZEN_BROWSER_PATH', 'CHROME_PATH', 'CHROMIUM_PATH', 'BROWSER']
    for var in env_vars:
        val = os.environ.get(var)
        if val and os.path.isfile(val) and os.access(val, os.X_OK):
            return val
    system = _platform.system().lower()
    if system == 'linux':
        paths = _BROWSER_PATHS.get('linux', [])
        for path in paths:
            if os.path.isfile(path) and os.access(path, os.X_OK):
                return path
    elif system == 'darwin':
        for path in _BROWSER_PATHS.get('darwin', []):
            if os.path.isfile(path) and os.access(path, os.X_OK):
                return path
    elif system == 'windows':
        for path in _BROWSER_PATHS.get('windows', []):
            expanded = os.path.expandvars(path)
            if os.path.isfile(expanded) and os.access(expanded, os.X_OK):
                return expanded
    from shutil import which
    names = ['chromium-browser', 'chromium', 'google-chrome', 'google-chrome-stable',
             'brave-browser', 'brave', 'vivaldi', 'msedge', 'opera',
             'chrome', 'google chrome', 'microsoft-edge']
    for name in names:
        found = which(name)
        if found:
            return found
    return None

class Browser:
    def __init__(self, headless=True, browser_path=None, connect_port=None, mode='browser'):
        self._drission = None
        self._headless = headless
        self._browser_path = browser_path
        self._connect_port = connect_port
        self._mode = mode
        self._url_history = []
        self._timeout_ms = 30000

    @property
    def timeout_ms(self):
        return self._timeout_ms

    @timeout_ms.setter
    def timeout_ms(self, value):
        self._timeout_ms = int(value)

    def _record_url(self):
        try:
            url = self.page.url
            if not self._url_history or self._url_history[-1] != url:
                self._url_history.append(url)
        except Exception:
            pass

    @property
    def url_history(self):
        return list(self._url_history)

    @property
    def current_url(self):
        try:
            return self.page.url
        except Exception:
            return 'about:blank'

    @property
    def previous_url(self):
        if len(self._url_history) >= 2:
            return self._url_history[-2]
        return None

    @property
    def older_url(self):
        if len(self._url_history) >= 3:
            return self._url_history[-3]
        return None

    @property
    def page(self):
        if self._drission is None:
            self.start()
        return self._drission

    @property
    def user_agent(self):
        return self.page.run_js('navigator.userAgent')

    def set_user_agent(self, ua):
        self.page.run_js(f'''
            Object.defineProperty(navigator, "userAgent", {{
                get: () => "{ua}",
                configurable: true
            }})
        ''')
        return True

    def set_headers(self, headers):
        if self._mode == 'http':
            self.page.set.headers(headers)
        else:
            for k, v in headers.items():
                self.page.run_js(f'''
                    (function() {{
                        if (!window.__zen_headers) {{
                            window.__zen_headers = {{}};
                            const orig = XMLHttpRequest.prototype.setRequestHeader;
                            XMLHttpRequest.prototype.setRequestHeader = function(key, val) {{
                                if (window.__zen_headers[key]) {{
                                    val = window.__zen_headers[key];
                                }}
                                orig.call(this, key, val);
                            }};
                        }}
                        window.__zen_headers["{k}"] = "{v}";
                    }})()
                ''')
        self._extra_headers = dict(headers)
        return True

    @property
    def headers(self):
        return dict(getattr(self, '_extra_headers', {}))

    def start(self):
        if self._drission is not None:
            return self
        if self._mode == 'http':
            from DrissionPage import SessionPage
            self._drission = SessionPage()
        elif self._mode == 'connect':
            from DrissionPage import ChromiumPage, ChromiumOptions
            co = ChromiumOptions()
            addr = f'127.0.0.1:{self._connect_port}' if self._connect_port else '127.0.0.1:9222'
            co.set_address(addr)
            co.existing_only(True)
            self._drission = ChromiumPage(addr_or_opts=co)
        else:
            from DrissionPage import ChromiumPage, ChromiumOptions
            co = ChromiumOptions()
            co.headless(self._headless)
            bp = self._browser_path or get_config().get('browser_path') or _find_browser_path()
            if bp:
                co.set_browser_path(bp)
            self._drission = ChromiumPage(addr_or_opts=co)
        return self

    def stop(self):
        if self._drission:
            try:
                self.page.quit()
            except Exception:
                pass
        self._drission = None

    def __enter__(self):
        return self.start()

    def __exit__(self, *args):
        self.stop()

    def _safe(self, fn, context=''):
        try:
            return fn()
        except Exception as e:
            msg = str(e)
            if context:
                raise ZenError(f'{context}: {msg}')
            raise ZenError(msg)

    def go(self, url):
        url = _ensure_scheme(str(url))
        self._safe(lambda: self.page.get(url), f'navigating to {url}')
        self._record_url()

    def url(self):
        return self._safe(lambda: self.page.url, 'getting URL')

    def title(self):
        return self._safe(lambda: self.page.title, 'getting title')

    def _resolve_locator(self, selector):
        if isinstance(selector, ZenElement):
            return selector._locator
        if isinstance(selector, ZenSelector):
            if selector.selector_type == 'css':
                return f'c:{selector.value}'
            if selector.selector_type == 'text':
                return f'text:{selector.value}'
            return str(selector.value)
        if isinstance(selector, str):
            if selector.startswith('/') and selector.endswith('/'):
                pattern = selector[1:-1]
                return f'text:{pattern}'
            return selector
        return str(selector)

    def _find_locator(self, selector):
        if isinstance(selector, ZenRegexMatch):
            return f'text:{selector.match}'
        return self._resolve_locator(selector)

    def _find_ele(self, loc):
        if not isinstance(loc, str):
            return loc
        try:
            return self.page.ele(loc)
        except Exception:
            return None

    def _find_eles(self, loc):
        if isinstance(loc, str):
            try:
                return self.page.eles(loc)
            except Exception:
                return []
        return [loc]

    def find(self, selector):
        loc = self._find_locator(selector)
        eles = self._find_eles(loc)
        if not eles:
            return None
        return ZenList([ZenElement(el) for el in eles])

    def find_first(self, selector):
        loc = self._find_locator(selector)
        if isinstance(loc, str):
            ele = self._find_ele(loc)
            if ele is None:
                return None
            return ZenElement(ele)
        return ZenElement(loc)

    def find_nth(self, selector, n):
        loc = self._find_locator(selector)
        eles = self._find_eles(loc)
        if n < len(eles):
            return ZenElement(eles[n])
        return None

    def click(self, target):
        if isinstance(target, ZenElement):
            self._safe(lambda: target.click(), 'clicking element')
        elif isinstance(target, ZenList):
            first = target.first()
            if first:
                self._safe(lambda: first.click(), 'clicking')
        else:
            loc = self._find_locator(target)
            ele = self._find_ele(loc)
            if ele is not None:
                self._safe(lambda: ele.click(), f'clicking "{target}"')

    def fill(self, selector, value):
        if isinstance(selector, ZenElement):
            self._safe(lambda: selector.fill(str(value)), 'filling element')
        else:
            loc = self._find_locator(selector)
            ele = self._find_ele(loc)
            if ele is not None:
                self._safe(lambda: ele.input(str(value)), f'filling "{selector}"')

    def text(self, selector):
        ele = self.find_first(selector)
        if ele is None:
            return ''
        return self._safe(lambda: ele.text, 'getting text')

    def texts(self, selector):
        loc = self._find_locator(selector)
        eles = self._find_eles(loc)
        return self._safe(lambda: [e.text for e in eles], 'getting texts')

    def attr(self, selector, name):
        ele = self.find_first(selector)
        if ele is None:
            return None
        return self._safe(lambda: ele.attr(name), f'getting attr "{name}"')

    def attrs(self, selector, name):
        loc = self._find_locator(selector)
        eles = self._find_eles(loc)
        return self._safe(lambda: [e.attr(name) for e in eles], f'getting attrs "{name}"')

    def wait(self, ms):
        _time.sleep(int(ms) / 1000.0)

    def wait_for(self, selector):
        loc = self._find_locator(selector)
        if isinstance(loc, str):
            self._safe(lambda: self.page.wait.eles_loaded(loc), f'waiting for "{selector}"')

    def wait_for_network(self):
        self._safe(lambda: self.page.wait.doc_loaded(), 'waiting for network idle')

    def refresh(self):
        self._safe(lambda: self.page.refresh(), 'refreshing')
        self._record_url()

    def back(self):
        self._safe(lambda: self.page.back(), 'going back')
        self._record_url()

    def forward(self):
        self._safe(lambda: self.page.forward(), 'going forward')
        self._record_url()

    def shot(self, path, full=False):
        dirname = os.path.dirname(str(path))
        if dirname and not os.path.exists(dirname):
            os.makedirs(dirname, exist_ok=True)
        self._safe(lambda: self.page.get_screenshot(path=str(path), full_page=bool(full)),
                   'taking screenshot')

    def scroll(self, direction=None, x=None, y=None):
        if direction == 'top':
            self._safe(lambda: self.page.scroll.to_top(), 'scrolling')
        elif direction == 'bottom':
            self._safe(lambda: self.page.scroll.to_bottom(), 'scrolling')
        elif direction == 'by':
            self._safe(lambda: self.page.run_js(f'window.scrollBy({x}, {y})'), 'scrolling')

    def execute(self, code):
        return self._safe(lambda: self.page.run_js(code), 'executing JS')

    def page_html(self):
        return self._safe(lambda: self.page.html, 'getting page HTML')

    def page_text_markers(self):
        js = """
        () => {
            function extract(root) {
                let parts = [];
                for (let node of root.childNodes) {
                    if (node.nodeType === 3) {
                        let t = node.textContent.trim();
                        if (t) parts.push(t);
                    } else if (node.nodeType === 1) {
                        let tag = node.tagName.toLowerCase();
                        if (tag === 'br') {
                            parts.push('\\n');
                        } else if (tag === 'img') {
                            parts.push('{[image]}');
                        } else if (tag === 'video') {
                            parts.push('{[video]}');
                        } else if (tag === 'audio') {
                            parts.push('{[audio]}');
                        } else if (tag === 'iframe') {
                            parts.push('{[iframe]}');
                        } else if (tag === 'script' || tag === 'style') {
                            continue;
                        } else if (tag === 'p' || tag === 'div' || tag === 'h1' || tag === 'h2' || tag === 'h3' || tag === 'h4' || tag === 'h5' || tag === 'h6' || tag === 'li') {
                            let inner = extract(node).trim();
                            if (inner) parts.push(inner + '\\n');
                        } else {
                            parts.push(extract(node));
                        }
                    }
                }
                return parts.join(' ').replace(/ +/g, ' ').replace(/\\n /g, '\\n');
            }
            return extract(document.body).trim();
        }
        """
        return self._safe(lambda: self.page.run_js(js), 'extracting page text')

    def page_links(self):
        return self._safe(
            lambda: self.page.run_js('() => [...document.querySelectorAll("a[href]")].map(a => a.href)'),
            'getting page links')

    def page_images(self):
        return self._safe(
            lambda: self.page.run_js('() => [...document.querySelectorAll("img[src]")].map(img => img.src)'),
            'getting page images')

    def page_forms(self):
        return self._safe(
            lambda: self.page.run_js('() => [...document.querySelectorAll("form")].map(f => ({action: f.action || "", method: f.method || "get", id: f.id || "", inputs: [...f.querySelectorAll("input, select, textarea")].map(i => ({name: i.name || "", type: i.type || "text", id: i.id || "", placeholder: i.placeholder || ""}))}))'),
            'getting page forms')

    def page_inputs(self):
        return self._safe(
            lambda: self.page.run_js('() => [...document.querySelectorAll("input, select, textarea")].map(i => ({name: i.name || "", type: i.type || "text", id: i.id || "", placeholder: i.placeholder || "", value: i.value || "", disabled: i.disabled, readonly: i.readOnly, required: i.required, checked: i.checked || false, tag: i.tagName.toLowerCase()}))'),
            'getting page inputs')

    def page_buttons(self):
        return self._safe(
            lambda: self.page.run_js('() => [...document.querySelectorAll("button, input[type=submit], input[type=button], input[type=reset], a.btn, [role=button]")].map(b => ({tag: b.tagName.toLowerCase(), type: b.type || "", id: b.id || "", text: (b.textContent || b.value || "").trim(), href: b.href || "", class: b.className || ""}))'),
            'getting page buttons')

    def download(self, url, path):
        import requests
        path = str(path)
        dirname = os.path.dirname(path)
        if dirname and not os.path.exists(dirname):
            os.makedirs(dirname, exist_ok=True)
        resp = self._safe(lambda: requests.get(url, timeout=self._timeout_ms / 1000.0),
                          f'downloading from {url}')
        with open(path, 'wb') as f:
            f.write(resp.content)

    def find_by_text(self, text, exact=False):
        loc = f'text:{text}' if not exact else f'text={text}'
        ele = self._find_ele(loc)
        if ele is None:
            return None
        return ZenElement(ele)

    def find_by_url(self, url, partial=True):
        js = '''
        (args) => {
            let target = args[0];
            let partial = args[1];
            let links = document.querySelectorAll('a[href]');
            for (let el of links) {
                let href = el.getAttribute('href') || '';
                if (partial ? href.includes(target) : href === target) {
                    let path = [];
                    let cur = el;
                    while (cur && cur !== document.body) {
                        let tag = cur.tagName.toLowerCase();
                        let id = cur.id ? '#' + CSS.escape(cur.id) : '';
                        let cls = '';
                        if (cur.classList.length) {
                            cls = '.' + [...cur.classList].map(c => CSS.escape(c)).join('.');
                        }
                        let same = cur.parentElement ? [...cur.parentElement.children].filter(c => c.tagName === cur.tagName) : [];
                        let nth = same.length > 1 ? ':nth-of-type(' + ([...same].indexOf(cur) + 1) + ')' : '';
                        path.unshift(tag + id + cls + nth);
                        cur = cur.parentElement;
                    }
                    return path.join(' > ');
                }
            }
            return null;
        }
        '''
        selector = self._safe(lambda: self.page.run_js(js, [str(url), bool(partial)]),
                              f'finding by URL {url}')
        if not selector:
            return None
        ele = self._find_ele(selector)
        if ele is None:
            return None
        return ZenElement(ele)

    def search(self, *criteria, selector=None):
        if len(criteria) == 0 and selector is None:
            return ZenList([])
        query = str(criteria[0]) if criteria else str(selector)
        q = query.strip()

        if q.startswith(('.', '#', '[')):
            loc = f'c:{q}'
        elif q.startswith('/') and q.endswith('/'):
            pattern = q[1:-1]
            loc = f'text:{pattern}'
        elif q.startswith('text='):
            loc = f'text:{q[5:]}'
        elif q.startswith('url='):
            url_part = q[4:]
            loc = f'c:a[href*="{url_part}"]'
        else:
            loc = f'text:{q}'

        eles = self._find_eles(loc)
        if not eles:
            return ZenList([])
        return ZenList([ZenElement(el) for el in eles])
