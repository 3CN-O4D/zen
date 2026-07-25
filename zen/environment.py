import os
import json
try:
    from .color import color
except ImportError:
    color = type('_', (), {'red': str, 'yellow': str, 'green': str})()

_ERROR_COLORS = True

def _strip(s):
    global _ERROR_COLORS
    try:
        _ERROR_COLORS = _ERROR_COLORS and bool(os.environ.get('TERM')) and os.isatty(2)
    except Exception:
        _ERROR_COLORS = False
    if not _ERROR_COLORS:
        try:
            import re
            s = re.sub(r'\033\[[0-9;]*m', '', s)
        except Exception:
            pass
    return s

def format_error(file_path, error, source_lines):
    """Format an error like Python traceback.

    Args:
        file_path: Path to the source file
        error: Exception object (LexerError, ParseError, or ZenError)
        source_lines: List of source code lines (0-indexed)

    Returns:
        Formatted error string
    """
    line = getattr(error, 'line', None)
    col = getattr(error, 'col', None)
    message = getattr(error, 'message', str(error))

    if line is None:
        # Try to get from token or node
        token = getattr(error, 'token', None)
        node = getattr(error, 'node', None)
        if token:
            line = getattr(token, 'line', None)
            col = getattr(token, 'col', None)
        elif node:
            line = getattr(node, 'line', None)
            col = getattr(node, 'col', None)

    parts = []
    parts.append(color.red('Traceback (most recent call last):'))
    if line is not None:
        parts.append(f'  File "{color.bright_cyan(file_path)}", line {color.yellow(str(line))}')
        if source_lines and 0 <= line - 1 < len(source_lines):
            src_line = source_lines[line - 1].rstrip('\n').rstrip('\r')
            parts.append(f'    {src_line}')
            if col is not None and col > 0:
                pointer = ' ' * (col - 1) + '^'
                parts.append(f'    {color.red(pointer)}')
    error_type = type(error).__name__
    parts.append(f'{color.red(error_type)}: {message}')
    return '\n'.join(parts)

class ZenReturn(Exception):
    def __init__(self, value):
        self.value = value

class ZenBreak(Exception):
    pass

class ZenContinue(Exception):
    pass

class ZenError(Exception):
    def __init__(self, message, node=None):
        self.message = message
        self.node = node
        super().__init__(message)

class ZenBrowserError(ZenError):
    pass

class ZenFileError(ZenError):
    pass


class Environment:
    def __init__(self, parent=None):
        self.vars = {}
        self.parent = parent
        self._locked = set()

    def lock(self, name):
        self._locked.add(name)

    def is_locked(self, name):
        if name in self._locked:
            return True
        if self.parent is not None:
            return self.parent.is_locked(name)
        return False

    def define(self, name, value):
        if self.is_locked(name):
            raise ZenError(f"Cannot redefine builtin '{name}'")
        self.vars[name] = value

    def get(self, name):
        if name in self.vars:
            return self.vars[name]
        if self.parent is not None:
            return self.parent.get(name)
        raise ZenError(f"Undefined variable: {name}")

    def has(self, name):
        if name in self.vars:
            return True
        if self.parent is not None:
            return self.parent.has(name)
        return False

    def set(self, name, value):
        if name in self.vars:
            if name in self._locked or (self.parent and self.parent.is_locked(name)):
                raise ZenError(f"Cannot redefine builtin '{name}'")
            self.vars[name] = value
            return
        if self.parent is not None:
            self.parent.set(name, value)
            return
        raise ZenError(f"Undefined variable: {name}")

    def child(self):
        return Environment(parent=self)

    def __repr__(self):
        return f"Environment({list(self.vars.keys())})"


class ZenMethod:
    def __init__(self, name, method):
        self._name = name
        self._method = method

    def __call__(self, *args, **kwargs):
        return self._method(*args, **kwargs)

    def __repr__(self):
        return f"<method {self._name}>"


class ZenElement:
    def __init__(self, element):
        self._locator = element

    @property
    def text(self):
        return self._locator.text

    @property
    def html(self):
        return self._locator.html

    @property
    def exists(self):
        try:
            return self._locator.states.is_alive
        except Exception:
            return False

    @property
    def tag(self):
        return self._locator.tag

    def attr(self, name):
        return self._locator.attr(name)

    @property
    def url(self):
        return self._locator.attr('href')

    @property
    def src(self):
        return self._locator.attr('src')

    def click(self):
        self._locator.click()
        return self

    def fill(self, value):
        self._locator.input(str(value))
        return self

    def check(self):
        self._locator.check()
        return self

    def uncheck(self):
        self._locator.check(uncheck=True)
        return self

    def select(self, value):
        try:
            self._locator.select.by_text(str(value))
        except Exception:
            try:
                self._locator.select.by_value(str(value))
            except Exception:
                self._locator.select.by_index(int(value))
        return self

    def find(self, selector):
        try:
            inner = self._locator.ele(selector)
            if inner is None:
                return None
            return ZenElement(inner)
        except Exception:
            return None

    def find_all(self, selector):
        try:
            inner = self._locator.eles(selector)
            if not inner:
                return ZenList([])
            return ZenList([ZenElement(el) for el in inner])
        except Exception:
            return ZenList([])

    def screenshot(self, path):
        self._locator.get_screenshot(path=str(path))
        return True

    def hover(self):
        self._locator.hover()

    @property
    def is_visible(self):
        try:
            return self._locator.states.is_displayed
        except Exception:
            return False

    @property
    def is_enabled(self):
        try:
            return self._locator.states.is_enabled
        except Exception:
            return False

    @property
    def is_checked(self):
        try:
            return self._locator.states.is_checked
        except Exception:
            return False

    def _eval_js(self, js):
        try:
            return self._locator.run_js(js)
        except Exception:
            return None

    def _download_url(self):
        src = self.attr('src')
        if src:
            return src
        href = self.attr('href')
        if href:
            return href
        try:
            src = self._locator.run_js('() => (this.querySelector("source") || {}).src || undefined')
            if src:
                return src
        except Exception:
            pass
        return None

    def play(self):
        self._eval_js('() => { try { this.muted = true; const p = this.play(); if (p && p.catch) p.catch(() => {}); } catch(e) {} }')

    def pause(self):
        self._eval_js('() => { try { this.pause(); } catch(e) {} }')

    def download(self, path):
        import requests
        url = self._download_url()
        if not url:
            raise ZenError("Element has no src or href to download")
        if os.path.isdir(path):
            filename = url.split('/')[-1].split('?')[0]
            path = os.path.join(path, filename)
        try:
            dirname = os.path.dirname(path)
            if dirname and not os.path.exists(dirname):
                os.makedirs(dirname, exist_ok=True)
            resp = requests.get(url)
            with open(path, 'wb') as f:
                f.write(resp.content)
        except Exception as e:
            raise ZenError(f"Download failed: {e}")

    @property
    def duration(self):
        return self._eval_js('() => this.duration')

    @property
    def paused(self):
        return self._eval_js('() => this.paused')

    @property
    def ended(self):
        return self._eval_js('() => this.ended')

    @property
    def muted(self):
        return self._eval_js('() => this.muted')

    @muted.setter
    def muted(self, value):
        v = json.dumps(value)
        self._eval_js(f'() => this.muted = {v}')

    @property
    def loop(self):
        return self._eval_js('() => this.loop')

    @loop.setter
    def loop(self, value):
        v = json.dumps(value)
        self._eval_js(f'() => this.loop = {v}')

    @property
    def volume(self):
        return self._eval_js('() => this.volume')

    @volume.setter
    def volume(self, value):
        v = json.dumps(value)
        self._eval_js(f'() => this.volume = {v}')

    @property
    def current_time(self):
        return self._eval_js('() => this.currentTime')

    @current_time.setter
    def current_time(self, value):
        v = json.dumps(value)
        self._eval_js(f'() => this.currentTime = {v}')

    def __repr__(self):
        return f"<ZenElement: {self._locator}>"


class ZenRegexMatch:
    def __init__(self, match):
        self._m = match
        self.match = match.group()
        self.start = match.start()
        self.end = match.end()

    def group(self, n=0):
        return self._m.group(n)

    def groups(self):
        return list(self._m.groups())

    def __repr__(self):
        return f'<RegexMatch: {self.match!r}>'


class HttpResponse:
    def __init__(self, status, body, headers, raw=None):
        self.status = status
        self.body = body
        self.headers = dict(headers)
        self.ok = 200 <= status < 400
        self._raw = raw if raw is not None else body.encode('utf-8')

    @property
    def bytes(self):
        return self._raw

    def json(self):
        return json.loads(self.body)

    def __repr__(self):
        return f'<Response [{self.status}]>'


class ZenSelector:
    def __init__(self, selector_type, value):
        self.selector_type = selector_type
        self.value = value

    def __repr__(self):
        return f"<ZenSelector: {self.selector_type}={self.value}>"


class ZenList:
    def __init__(self, elements):
        self._elements = elements

    def __len__(self):
        return len(self._elements)

    def __getitem__(self, index):
        return self._elements[index]

    def __iter__(self):
        return iter(self._elements)

    @property
    def first(self):
        return self._elements[0] if self._elements else None

    def nth(self, n):
        return self._elements[n] if n < len(self._elements) else None

    @property
    def texts(self):
        return [e.text for e in self._elements if e is not None]

    @property
    def htmls(self):
        return [e.html for e in self._elements if e is not None]

    @property
    def count(self):
        return len(self._elements)

    def attr(self, name):
        return [e.attr(name) for e in self._elements if e is not None]

    def attrs(self, name):
        return [e.attr(name) for e in self._elements if e is not None]

    @property
    def tags(self):
        return [e.tag for e in self._elements if e is not None]

    def each(self, callback):
        results = []
        for i, e in enumerate(self._elements):
            results.append(callback(e, i))
        return results

    @property
    def len(self):
        return len(self._elements)

    def sorted(self):
        return sorted(self._elements, key=lambda e: str(e.text) if e is not None else '')

    def __repr__(self):
        return f"<ZenList: {len(self._elements)} elements>"


def _bound_method(instance, fn):
    def bound(*args, **kwargs):
        return fn(instance, *args, **kwargs)
    if hasattr(fn, '_is_zen_func'):
        bound._is_zen_func = True
    return bound


class ZenClass:
    def __init__(self, name, methods, parent=None, interpreter=None):
        self._name = name
        self._methods = dict(methods)
        self._parent = parent
        self._interpreter = interpreter

    def _resolve(self, name):
        if name in self._methods:
            return self._methods[name]
        if self._parent is not None:
            if isinstance(self._parent, ZenClass):
                return self._parent._resolve(name)
            if isinstance(self._parent, dict):
                return self._parent.get(name)
        return None

    def __call__(self, *args):
        instance = ZenInstance({'__class__': self}, self, self._interpreter)
        init = self._resolve('__init__')
        if init is not None:
            if self._interpreter:
                fn = lambda inst, *a: self._interpreter._call_func(init, inst, *a)
                fn(instance, *args)
            else:
                init(instance, *args)
        return instance

    def __repr__(self):
        return f"<class {self._name}>"


class ZenInstance:
    def __init__(self, data, klass, interpreter=None):
        self.__data = data
        self.__klass = klass
        self.__interp = interpreter

    def __getattr__(self, name):
        if name in ('__data', '__klass', '__interp', '_ZenInstance__data',
                     '_ZenInstance__klass', '_ZenInstance__interp'):
            return object.__getattribute__(self, name)
        d = object.__getattribute__(self, '_ZenInstance__data')
        if name in d:
            return d[name]
        k = object.__getattribute__(self, '_ZenInstance__klass')
        method = k._resolve(name) if hasattr(k, '_resolve') else None
        if method is not None:
            interp = object.__getattribute__(self, '_ZenInstance__interp')
            if interp:
                from_node = method if hasattr(method, 'params') else None
                if from_node:
                    return _bound_method(self, lambda inst, *a: interp._call_func(from_node, inst, *a))
                return _bound_method(self, lambda inst, *a: method(inst, *a))
            return _bound_method(self, method)
        raise AttributeError(name)

    def __setattr__(self, name, value):
        if name in ('__data', '__klass', '__interp', '_ZenInstance__data',
                     '_ZenInstance__klass', '_ZenInstance__interp'):
            object.__setattr__(self, name, value)
        else:
            d = object.__getattribute__(self, '_ZenInstance__data')
            d[name] = value

    def __repr__(self):
        return f"<instance of {self.__klass._name if hasattr(self.__klass, '_name') else '?'}>"


class PageModule:
    def __init__(self, browser):
        self._browser = browser

    @property
    def html(self):
        return self._browser.page_html()

    @property
    def text(self):
        return self._browser.page_text_markers()

    @property
    def links(self):
        return self._browser.page_links()

    @property
    def images(self):
        return self._browser.page_images()

    @property
    def forms(self):
        return self._browser.page_forms()

    @property
    def inputs(self):
        return self._browser.page_inputs()

    @property
    def buttons(self):
        return self._browser.page_buttons()

    @property
    def title(self):
        return self._browser.title()

    @property
    def url(self):
        return self._browser.url()

    @property
    def source(self):
        return self._browser.page_html()

    def __repr__(self):
        return '<page module>'


class ConfigModule:
    def __init__(self, config_dict, sync_fn=None):
        object.__setattr__(self, '_data', config_dict)
        object.__setattr__(self, '_sync', sync_fn)

    def __getattr__(self, name):
        if name.startswith('_'):
            raise AttributeError(name)
        d = object.__getattribute__(self, '_data')
        if name in d:
            return d[name]
        raise AttributeError(f"Config has no key '{name}'")

    def __setattr__(self, name, value):
        if name.startswith('_'):
            object.__setattr__(self, name, value)
        else:
            d = object.__getattribute__(self, '_data')
            d[name] = value
            sync = object.__getattribute__(self, '_sync')
            if sync:
                sync()

    def get(self, key, default=None):
        return self._data.get(key, default)

    def set(self, key, value):
        self._data[str(key)] = value
        sync = object.__getattribute__(self, '_sync')
        if sync:
            sync()
        return value

    def _keys(self):
        return list(self._data.keys())

    def __repr__(self):
        return f'<config: {dict(self._data)}>'
