import os
import re as _re
import time as _time
from .environment import ZenElement, ZenSelector, ZenList, ZenRegexMatch, ZenError, ZenBrowserError

_SOUP_AVAILABLE = False
try:
    from .soup_page import SoupPage
    _SOUP_AVAILABLE = True
except ImportError:
    pass

_URL_RE = _re.compile(r'^[a-zA-Z][a-zA-Z0-9+\-.]*://')

_config = {
    'browser_path': None,
    'headless': True,
    'timeout': 30000,
    'ele_timeout': 1,
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
        self._no_browser = None

    @property
    def has_browser(self):
        if self._no_browser or self._mode == 'none':
            return False
        if self._mode in ('http', 'soup'):
            return True
        return self._drission is not None

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
        if self._no_browser:
            raise ZenBrowserError(self._no_browser)
        if self._drission is None:
            self.start()
        if self._drission is None:
            msg = self._no_browser or 'No browser available.'
            raise ZenBrowserError(msg)
        return self._drission

    @property
    def user_agent(self):
        if self._mode == 'soup':
            return self.page._session.headers.get('User-Agent', '')
        return self.page.run_js('navigator.userAgent')

    def set_user_agent(self, ua):
        if self._mode == 'soup':
            self.page._session.headers.update({'User-Agent': str(ua)})
            return True
        self.page.run_js(f'''
            Object.defineProperty(navigator, "userAgent", {{
                get: () => "{ua}",
                configurable: true
            }})
        ''')
        return True

    def set_headers(self, headers):
        if self._mode in ('http', 'soup'):
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
        if self._mode == 'soup':
            if not _SOUP_AVAILABLE:
                self._no_browser = (
                    "SoupPage not available. Install dependencies:\n"
                    "  pip install requests beautifulsoup4 lxml\n")
                return self
            self._drission = SoupPage()
        elif self._mode == 'http':
            from DrissionPage import SessionPage
            self._drission = SessionPage()
        elif self._mode == 'connect':
            from DrissionPage import ChromiumPage, ChromiumOptions
            co = ChromiumOptions()
            addr = f'127.0.0.1:{self._connect_port}' if self._connect_port else '127.0.0.1:9222'
            co.set_address(addr)
            co.existing_only(True)
            try:
                self._drission = ChromiumPage(addr_or_opts=co)
            except (FileNotFoundError, OSError):
                self._no_browser = (
                    "Cannot connect to browser.\n"
                    "  Make sure Chrome is running with --remote-debugging-port=9222\n"
                    "  Or use:  zen script.z --http  (HTTP-only mode)")
            except Exception as exc:
                msg = str(exc).split('\n')[0][:120]
                self._no_browser = (
                    f"Cannot connect to browser: {msg}\n"
                    f"  Make sure Chrome is running with --remote-debugging-port=9222 --remote-allow-origins=*\n")
        elif self._mode == 'none':
            pass
        else:
            from DrissionPage import ChromiumPage, ChromiumOptions
            co = ChromiumOptions()
            co.headless(self._headless)
            bp = self._browser_path or get_config().get('browser_path') or _find_browser_path()
            if bp:
                co.set_browser_path(bp)
            try:
                self._drission = ChromiumPage(addr_or_opts=co)
            except (FileNotFoundError, OSError):
                msg = (
                    "No Chromium-based browser found.\n"
                    "  Install one:   pkg install chromium          (Termux)\n"
                    "                 apt install chromium-browser  (Debian/Ubuntu)\n"
                    "                 brew install --cask chromium  (macOS)\n"
                    "  Or use:        zen script.z --http           (HTTP-only, no browser)\n"
                )
                if bp:
                    msg += f"  Path tried:  {bp}\n"
                self._no_browser = msg
            except Exception as exc:
                msg = str(exc).split('\n')[0][:120]
                self._no_browser = (
                    f"Cannot start browser: {msg}\n"
                    f"  Use zen --no-browser for language-only mode.\n"
                )
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
            return self.page.ele(loc, timeout=_config['ele_timeout'])
        except Exception:
            return None

    def _find_eles(self, loc):
        if isinstance(loc, str):
            try:
                return self.page.eles(loc, timeout=_config['ele_timeout'])
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
        if self._mode == 'soup':
            return  # no-op, page is already loaded
        loc = self._find_locator(selector)
        if isinstance(loc, str):
            self._safe(lambda: self.page.wait.eles_loaded(loc), f'waiting for "{selector}"')

    def wait_for_network(self):
        if self._mode == 'soup':
            return  # no-op
        self._safe(lambda: self.page.wait.doc_loaded(), 'waiting for network idle')

    def refresh(self):
        if self._mode == 'soup':
            self._safe(lambda: self.page.refresh(), 'refreshing')
            self._record_url()
            return
        self._safe(lambda: self.page.refresh(), 'refreshing')
        self._record_url()

    def back(self):
        if self._mode == 'soup':
            self._safe(lambda: self.page.back(), 'going back')
            self._record_url()
            return
        self._safe(lambda: self.page.back(), 'going back')
        self._record_url()

    def forward(self):
        if self._mode == 'soup':
            raise ZenError("Forward not supported in soup mode")
        self._safe(lambda: self.page.forward(), 'going forward')
        self._record_url()

    def shot(self, path, full=False):
        if self._mode == 'soup':
            raise ZenError("Screenshots require a browser. Use --headful or --headless.")
        dirname = os.path.dirname(str(path))
        if dirname and not os.path.exists(dirname):
            os.makedirs(dirname, exist_ok=True)
        self._safe(lambda: self.page.get_screenshot(path=str(path), full_page=bool(full)),
                   'taking screenshot')

    def scroll(self, direction=None, x=None, y=None):
        if self._mode == 'soup':
            return  # no-op
        if direction == 'top':
            self._safe(lambda: self.page.scroll.to_top(), 'scrolling')
        elif direction == 'bottom':
            self._safe(lambda: self.page.scroll.to_bottom(), 'scrolling')
        elif direction == 'by':
            self._safe(lambda: self.page.run_js(f'window.scrollBy({x}, {y})'), 'scrolling')

    def execute(self, code):
        if self._mode == 'soup':
            raise ZenError("execute() / js() requires a browser. Use --headful or --headless.")
        code = str(code).strip()
        if code.startswith('return ') or code.startswith('return\n'):
            return self._safe(lambda: self.page.run_js(code), 'executing JS')
        stmt_keywords = ('var ', 'let ', 'const ', 'if ', 'for ', 'while ', 'function ',
                         'switch ', 'try ', 'throw ')
        if any(code.startswith(kw) for kw in stmt_keywords):
            code = 'return (function(){ ' + code + ' })()'
        else:
            code = 'return ' + code
        return self._safe(lambda: self.page.run_js(code), 'executing JS')

    def page_html(self):
        return self._safe(lambda: self.page.html, 'getting page HTML')

    def page_text_markers(self):
        if self._mode == 'soup':
            return self._safe(lambda: self._soup_text_markers(), 'extracting page text')
        js = """
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
        return (document.body ? extract(document.body).trim() : '');
        """
        return self._safe(lambda: self.page.run_js(js), 'extracting page text')

    def _soup_text_markers(self):
        from bs4 import Comment, NavigableString
        parts = []
        def walk(tag, block=False):
            for child in tag.children:
                if isinstance(child, Comment):
                    continue
                if isinstance(child, NavigableString):
                    t = str(child).strip()
                    if t:
                        parts.append(t)
                elif child.name in ('br',):
                    parts.append('\n')
                elif child.name in ('img',):
                    parts.append('{[image]}')
                elif child.name in ('video',):
                    parts.append('{[video]}')
                elif child.name in ('audio',):
                    parts.append('{[audio]}')
                elif child.name in ('iframe',):
                    parts.append('{[iframe]}')
                elif child.name in ('script', 'style'):
                    continue
                elif child.name in ('p', 'div', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'li'):
                    walk(child, block=True)
                else:
                    walk(child)
            if block and parts:
                last = parts[-1]
                if isinstance(last, str) and not last.endswith('\n'):
                    parts.append('\n')
        if self.page._soup and self.page._soup.body:
            walk(self.page._soup.body)
        return ' '.join(str(p) for p in parts).replace(' \n', '\n').strip()

    def page_links(self):
        if self._mode == 'soup':
            return self._safe(lambda: _soup_links(self.page), 'getting page links')
        return self._safe(
            lambda: self.page.run_js('return [...document.querySelectorAll("a[href]")].map(a => a.href)'),
            'getting page links')

    def page_images(self):
        if self._mode == 'soup':
            return self._safe(lambda: _soup_images(self.page), 'getting page images')
        return self._safe(
            lambda: self.page.run_js('return [...document.querySelectorAll("img[src]")].map(img => img.src)'),
            'getting page images')

    def page_forms(self):
        if self._mode == 'soup':
            return self._safe(lambda: _soup_forms(self.page), 'getting page forms')
        return self._safe(
            lambda: self.page.run_js('return [...document.querySelectorAll("form")].map(f => ({action: f.action || "", method: f.method || "get", id: f.id || "", inputs: [...f.querySelectorAll("input, select, textarea")].map(i => ({name: i.name || "", type: i.type || "text", id: i.id || "", placeholder: i.placeholder || ""}))}))'),
            'getting page forms')

    def page_inputs(self):
        if self._mode == 'soup':
            return self._safe(lambda: _soup_inputs(self.page), 'getting page inputs')
        return self._safe(
            lambda: self.page.run_js('return [...document.querySelectorAll("input, select, textarea")].map(i => ({name: i.name || "", type: i.type || "text", id: i.id || "", placeholder: i.placeholder || "", value: i.value || "", disabled: i.disabled, readonly: i.readOnly, required: i.required, checked: i.checked || false, tag: i.tagName.toLowerCase()}))'),
            'getting page inputs')

    def page_buttons(self):
        if self._mode == 'soup':
            return self._safe(lambda: _soup_buttons(self.page), 'getting page buttons')
        return self._safe(
            lambda: self.page.run_js('return [...document.querySelectorAll("button, input[type=submit], input[type=button], input[type=reset], a.btn, [role=button]")].map(b => ({tag: b.tagName.toLowerCase(), type: b.type || "", id: b.id || "", text: (b.textContent || b.value || "").trim(), href: b.href || "", class: b.className || ""}))'),
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
        if self._mode == 'soup':
            page = self.page
            if not page._soup:
                return None
            target = str(url)
            for a in page._soup.find_all('a', href=True):
                href = a.get('href', '')
                if (partial and target in href) or (not partial and href == target):
                    return ZenElement(page.ele('a[href="' + href + '"]'))
            return None
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


# ── soup-mode helpers ─────────────────────────────────────────────────

def _soup_links(page):
    if not page._soup:
        return []
    return [a.get('href', '') for a in page._soup.find_all('a', href=True)]

def _soup_images(page):
    if not page._soup:
        return []
    return [img.get('src', '') for img in page._soup.find_all('img', src=True)]

def _soup_forms(page):
    if not page._soup:
        return []
    result = []
    for f in page._soup.find_all('form'):
        inputs = []
        for i in f.find_all(['input', 'select', 'textarea']):
            inputs.append({
                'name': i.get('name', ''),
                'type': i.get('type', 'text'),
                'id': i.get('id', ''),
                'placeholder': i.get('placeholder', ''),
            })
        result.append({
            'action': f.get('action', ''),
            'method': f.get('method', 'get'),
            'id': f.get('id', ''),
            'inputs': inputs,
        })
    return result

def _soup_inputs(page):
    if not page._soup:
        return []
    result = []
    for i in page._soup.find_all(['input', 'select', 'textarea']):
        result.append({
            'name': i.get('name', ''),
            'type': i.get('type', 'text'),
            'id': i.get('id', ''),
            'placeholder': i.get('placeholder', ''),
            'value': i.get('value', ''),
            'disabled': i.get('disabled') is not None,
            'readonly': i.get('readonly') is not None,
            'required': i.get('required') is not None,
            'checked': i.get('checked') is not None,
            'tag': i.name,
        })
    return result

def _soup_buttons(page):
    if not page._soup:
        return []
    result = []
    for b in page._soup.find_all(['button', 'a']):
        role = b.get('role', '')
        if b.name == 'button' or b.get('type') in ('submit', 'button') or role == 'button' or b.name == 'a':
            result.append({
                'tag': b.name,
                'type': b.get('type', ''),
                'id': b.get('id', ''),
                'text': (b.get_text(strip=True) or b.get('value', '')),
                'href': b.get('href', ''),
                'class': ' '.join(b.get('class', [])),
            })
    return result
