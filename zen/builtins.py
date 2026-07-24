import time as _time
import os
import json as _json
import urllib.request as _urllib
import urllib.parse as _urlparse
import random as _random
import math as _math
import datetime as _datetime
import socket as _socket
from .environment import ZenElement, ZenSelector, ZenList, ZenRegexMatch, HttpResponse, ZenError, PageModule, ConfigModule
from .browser import get_config, set_config


def _typed_input(prompt, expected_type=None):
    raw = input(prompt)
    if expected_type is None:
        return raw
    t = str(expected_type).strip().lower()
    try:
        if t in ('str', 'string'):
            return raw
        if t in ('int', 'integer'):
            return int(raw)
        if t in ('float', 'real', 'double'):
            return float(raw)
        if t == 'bool':
            low = raw.strip().lower()
            if low in ('true', 'yes', 'y', '1'):
                return True
            if low in ('false', 'no', 'n', '0'):
                return False
            raise ValueError(f"cannot convert {raw!r} to bool")
        if t == 'list':
            import json as _json
            return _json.loads(raw)
        if t in ('dict', 'map', 'object'):
            import json as _json
            return _json.loads(raw)
        # fallback: try converting via Python's type name
        import builtins as _builtins
        converter = getattr(_builtins, t, None)
        if converter is not None and callable(converter):
            return converter(raw)
        raise ValueError(f"unknown type: {expected_type!r}")
    except Exception as e:
        from .environment import ZenError
        raise ZenError(f"Failed to convert input to {t}: {e}")

def _assert_fn(cond, msg=None):
    from .environment import ZenError
    if not cond:
        raise ZenError(msg or 'Assertion failed')
    return True

def _assert_eq_fn(a, b, msg=None):
    from .environment import ZenError
    if a != b:
        raise ZenError(msg or f'Assertion failed: {a!r} != {b!r}')
    return True

def _resolve_selector(sel, **kwargs):
    if kwargs.get('text'):
        text_val = kwargs['text']
        return f'text={"".join(text_val) if isinstance(text_val, list) else text_val}'
    if kwargs.get('url'):
        return f'css=[href="{kwargs["url"]}"]'
    if isinstance(sel, str) and sel.strip():
        s = sel.strip()
        if _looks_like_plain_text(s):
            return f'text={s}'
        return s
    if isinstance(sel, ZenElement):
        return sel
    return sel

def _looks_like_plain_text(s):
    if ' ' not in s:
        return False
    for ch in ('#', '.', ':', '>', '+', '~', '[', ']', '@', '*'):
        if ch in s:
            return False
    return True

def _resolve_find(browser, mode, *args, **kwargs):
    sel = args[0] if args else None
    resolved = _resolve_selector(sel, **kwargs)
    exact = kwargs.get('exact', False)
    if isinstance(resolved, str) and resolved.startswith('text='):
        text_val = resolved[5:]
        if exact:
            resolved = f'text={text_val}'
        if mode == 'all':
            return browser.find(resolved)
        elif mode == 'nth':
            n = int(kwargs.get('n', 0))
            return browser.find_nth(resolved, n)
        return browser.find_first(resolved)
    if mode == 'all':
        return browser.find(str(resolved))
    elif mode == 'nth':
        n = int(kwargs.get('n', 0))
        return browser.find_nth(str(resolved), n)
    return browser.find_first(str(resolved))

def _smart_click(browser, *args, **kwargs):
    if not args and not kwargs:
        browser.click(None)
        return True
    sel = args[0] if args else None
    if hasattr(sel, '_locator'):
        sel.click()
        return True
    resolved = _resolve_selector(sel, **kwargs)
    browser.click(str(resolved))
    return True

def _smart_fill(browser, *args, **kwargs):
    if kwargs.get('with'):
        val = kwargs['with']
        sel = args[0] if args else None
        resolved = _resolve_selector(sel, **kwargs)
        browser.fill(str(resolved), str(val))
        return True
    elif len(args) >= 2:
        browser.fill(str(args[0]), str(args[1]))
        return True
    elif kwargs:
        for k, v in kwargs.items():
            if k != 'text' and k != 'exact':
                sel = _resolve_selector(args[0] if args else None, **kwargs)
                browser.fill(str(sel), str(v))
                return True
    raise ZenError('fill(selector, value) requires a selector and a value')

def _smart_wait(browser, *args, **kwargs):
    sel = args[0] if args else None
    resolved = _resolve_selector(sel, **kwargs)
    return browser.wait_for(str(resolved))

def _smart_check(browser, *args):
    if args and hasattr(args[0], '_locator'):
        return args[0].check()
    sel = str(args[0]) if args else None
    return browser.find_first(sel).check()

def _smart_uncheck(browser, *args):
    if args and hasattr(args[0], '_locator'):
        return args[0].uncheck()
    sel = str(args[0]) if args else None
    return browser.find_first(sel).uncheck()

def _smart_select(browser, sel, val):
    if hasattr(sel, '_locator'):
        return sel.select(val)
    return browser.find_first(str(sel)).select(str(val))

def register_builtins(env, browser):
    cfg = get_config()

    def _sync_config():
        for k in cfg:
            if k not in cfg:
                continue
        set_config('browser_path', cfg.get('browser_path'))
        set_config('browser_type', cfg.get('browser_type'))
        set_config('headless', cfg.get('headless'))
        set_config('timeout', cfg.get('timeout'))

    env.define('config', ConfigModule(cfg, _sync_config))

    env.define('type', lambda v: type(v).__name__)
    env.define('len', lambda v: len(v) if hasattr(v, '__len__') else len(str(v)))
    env.define('str', lambda v: str(v))
    env.define('int', lambda v: int(v))
    env.define('float', lambda v: float(v))
    env.define('bool', lambda v: bool(v))
    env.define('list', lambda v: list(v))

    env.define('assert', lambda cond, msg=None: _assert_fn(cond, msg))
    env.define('assert_eq', lambda a, b, msg=None: _assert_eq_fn(a, b, msg))
    env.define('assertEq', lambda a, b, msg=None: _assert_eq_fn(a, b, msg))

    env.define('range', lambda start, end=None, step=1: list(range(start, end) if end is not None else range(start)))
    env.define('interval', lambda start, end, step=1: list(range(start, end, step)))
    env.define('abs', lambda v: abs(v))
    env.define('min', lambda *args: min(args))
    env.define('max', lambda *args: max(args))
    env.define('round', lambda v, ndigits=0: round(v, int(ndigits)))
    env.define('go', lambda url: browser.go(str(url)) or True)
    env.define('fill', lambda *args, **kwargs: _smart_fill(browser, *args, **kwargs))
    env.define('click', lambda *args, **kwargs: _smart_click(browser, *args, **kwargs))
    env.define('check', lambda *args: _smart_check(browser, *args) if args else None)
    env.define('uncheck', lambda *args: _smart_uncheck(browser, *args) if args else None)
    env.define('select', lambda sel, val: _smart_select(browser, sel, val))
    env.define('text', lambda sel: browser.text(sel))
    env.define('texts', lambda sel: browser.texts(sel))
    env.define('attr', lambda sel, name: browser.attr(sel, name))
    env.define('attrs', lambda sel, name: browser.attrs(sel, name))
    env.define('wait', lambda ms: browser.wait(_parse_duration(ms)))
    env.define('wait_for', lambda *args, **kwargs: _smart_wait(browser, *args, **kwargs))
    env.define('waitFor', lambda *args, **kwargs: _smart_wait(browser, *args, **kwargs))
    env.define('wait_for_network', lambda: browser.wait_for_network())
    env.define('waitForNetwork', lambda: browser.wait_for_network())
    env.define('refresh', lambda: browser.refresh())
    env.define('back', lambda: browser.back())
    env.define('forward', lambda: browser.forward())
    env.define('shot', lambda path, full=False: browser.shot(str(path), bool(full)))
    env.define('execute', lambda code: browser.execute(str(code)))
    env.define('uri', lambda: browser.url())
    env.define('title', lambda: browser.title())
    env.define('user_agent', lambda: browser.user_agent)
    env.define('userAgent', lambda: browser.user_agent)
    env.define('set_user_agent', lambda ua: browser.set_user_agent(str(ua)))
    env.define('setUserAgent', lambda ua: browser.set_user_agent(str(ua)))
    env.define('set_headers', lambda headers: browser.set_headers(dict(headers)))
    env.define('setHeaders', lambda headers: browser.set_headers(dict(headers)))
    env.define('headers', lambda: browser.headers)

    env.define('find', lambda *args, **kwargs: _resolve_find(browser, 'first', *args, **kwargs))
    env.define('find_all', lambda *args, **kwargs: _resolve_find(browser, 'all', *args, **kwargs))
    env.define('findAll', lambda *args, **kwargs: _resolve_find(browser, 'all', *args, **kwargs))
    env.define('first', lambda *args, **kwargs: _resolve_find(browser, 'first', *args, **kwargs))
    env.define('nth', lambda sel, n, **kwargs: _resolve_find(browser, 'nth', sel, n=int(n), **kwargs))

    env.define('download', lambda url, path: browser.download(str(url), str(path)))

    env.define('css', lambda sel: ZenSelector('css', sel))
    env.define('by_text', lambda text: ZenSelector('text', text))
    env.define('byText', lambda text: ZenSelector('text', text))

    env.define('input', lambda prompt, expected_type=None: _typed_input(str(prompt), expected_type))
    env.define('input_str', lambda prompt, expected_type=None: _typed_input(str(prompt), expected_type))
    env.define('inputStr', lambda prompt, expected_type=None: _typed_input(str(prompt), expected_type))
    env.define('prompt', lambda msg="": input(str(msg)))
    env.define('confirm', lambda msg="": input(str(msg) + " (y/n): ").strip().lower() in ('y', 'yes'))

    env.define('scroll_to', lambda y: browser.execute(f'window.scrollTo(0, {y})'))
    env.define('scrollTo', lambda y: browser.execute(f'window.scrollTo(0, {y})'))

    env.define('sleep', lambda secs: _time.sleep(float(secs)))

    env.define('read_file', lambda path: _read_file(str(path)))
    env.define('readFile', lambda path: _read_file(str(path)))
    env.define('write_file', lambda path, content: _write_file(str(path), str(content)))
    env.define('writeFile', lambda path, content: _write_file(str(path), str(content)))
    env.define('append_file', lambda path, content: _append_file(str(path), str(content)))
    env.define('appendFile', lambda path, content: _append_file(str(path), str(content)))
    env.define('file_exists', lambda path: os.path.exists(str(path)))
    env.define('fileExists', lambda path: os.path.exists(str(path)))
    env.define('list_dir', lambda path='.': os.listdir(str(path)))
    env.define('listDir', lambda path='.': os.listdir(str(path)))
    env.define('mkdir', lambda path: os.makedirs(str(path), exist_ok=True))
    env.define('remove_file', lambda path: os.remove(str(path)))
    env.define('removeFile', lambda path: os.remove(str(path)))
    env.define('copy_file', lambda src, dst: __import__('shutil').copy2(str(src), str(dst)))
    env.define('copyFile', lambda src, dst: __import__('shutil').copy2(str(src), str(dst)))
    env.define('move_file', lambda src, dst: __import__('shutil').move(str(src), str(dst)))
    env.define('moveFile', lambda src, dst: __import__('shutil').move(str(src), str(dst)))
    env.define('rename_file', lambda src, dst: os.rename(str(src), str(dst)))
    env.define('renameFile', lambda src, dst: os.rename(str(src), str(dst)))
    env.define('path_join', lambda *parts: os.path.join(*[str(p) for p in parts]))
    env.define('pathJoin', lambda *parts: os.path.join(*[str(p) for p in parts]))

    env.define('basename', lambda path: os.path.basename(str(path)))
    env.define('dirname', lambda path: os.path.dirname(str(path)))
    env.define('cwd', lambda: os.getcwd())
    env.define('pwd', lambda: os.getcwd())
    env.define('cd', lambda path: os.chdir(str(path)))
    env.define('chdir', lambda path: os.chdir(str(path)))

    env.define('read_binary', lambda path: _read_binary(str(path)))
    env.define('readBinary', lambda path: _read_binary(str(path)))
    env.define('write_binary', lambda path, data: _write_binary(str(path), data))
    env.define('writeBinary', lambda path, data: _write_binary(str(path), data))

    env.define('rmdir', lambda path: os.rmdir(str(path)))
    env.define('remove_dir', lambda path: os.rmdir(str(path)))
    env.define('removeDir', lambda path: os.rmdir(str(path)))

    env.define('glob', lambda pattern: __import__('glob').glob(str(pattern)))
    env.define('file_size', lambda path: os.path.getsize(str(path)))
    env.define('fileSize', lambda path: os.path.getsize(str(path)))
    env.define('file_mtime', lambda path: os.path.getmtime(str(path)))
    env.define('fileMtime', lambda path: os.path.getmtime(str(path)))
    env.define('is_file', lambda path: os.path.isfile(str(path)))
    env.define('isFile', lambda path: os.path.isfile(str(path)))
    env.define('is_dir', lambda path: os.path.isdir(str(path)))
    env.define('isDir', lambda path: os.path.isdir(str(path)))

    env.define('exec', lambda cmd: _exec_cmd(str(cmd)))
    env.define('sh', lambda cmd: _exec_cmd(str(cmd)))
    env.define('system', lambda cmd: _exec_cmd(str(cmd)))

    env.define('python', lambda code, **kwargs: _exec_python(str(code), kwargs.get('globals')))
    env.define('lua', lambda code: _exec_lua(str(code)))

    env.define('fs', {
        'list': lambda path='.': os.listdir(str(path)),
        'read': lambda path: _read_file(str(path)),
        'write': lambda path, content: _write_file(str(path), str(content)),
        'append': lambda path, content: _append_file(str(path), str(content)),
        'read_binary': lambda path: _read_binary(str(path)),
        'readBinary': lambda path: _read_binary(str(path)),
        'write_binary': lambda path, data: _write_binary(str(path), data),
        'writeBinary': lambda path, data: _write_binary(str(path), data),
        'exists': lambda path: os.path.exists(str(path)),
        'is_file': lambda path: os.path.isfile(str(path)),
        'isFile': lambda path: os.path.isfile(str(path)),
        'is_dir': lambda path: os.path.isdir(str(path)),
        'isDir': lambda path: os.path.isdir(str(path)),
        'size': lambda path: os.path.getsize(str(path)),
        'mtime': lambda path: os.path.getmtime(str(path)),
        'mkdir': lambda path: os.makedirs(str(path), exist_ok=True),
        'mkdirs': lambda path: os.makedirs(str(path), exist_ok=True),
        'remove': lambda path: os.remove(str(path)),
        'rmdir': lambda path: os.rmdir(str(path)),
        'rmtree': lambda path: __import__('shutil').rmtree(str(path)),
        'copy': lambda src, dst: __import__('shutil').copy2(str(src), str(dst)),
        'move': lambda src, dst: __import__('shutil').move(str(src), str(dst)),
        'rename': lambda src, dst: os.rename(str(src), str(dst)),
        'glob': lambda pattern: __import__('glob').glob(str(pattern)),
        'join': lambda *parts: os.path.join(*[str(p) for p in parts]),
        'basename': lambda path: os.path.basename(str(path)),
        'dirname': lambda path: os.path.dirname(str(path)),
        'cwd': lambda: os.getcwd(),
        'cd': lambda path: os.chdir(str(path)),
        'exec': lambda cmd: _exec_cmd(str(cmd)),
        'sh': lambda cmd: _exec_cmd(str(cmd)),
    })

    env.define('history', lambda: browser.url_history)

    env.define('search', lambda *args, **kwargs: browser.search(*args, **kwargs))
    env.define('find_by_text', lambda text, exact=False: browser.find_by_text(text, exact))
    env.define('findByText', lambda text, exact=False: browser.find_by_text(text, exact))
    env.define('find_by_url', lambda url, partial=True: browser.find_by_url(url, partial))
    env.define('findByUrl', lambda url, partial=True: browser.find_by_url(url, partial))

    env.define('page_html', lambda: browser.page_html())
    env.define('pageHtml', lambda: browser.page_html())
    env.define('page_text', lambda: browser.page_text_markers())
    env.define('pageText', lambda: browser.page_text_markers())
    env.define('page_links', lambda: browser.page_links())
    env.define('pageLinks', lambda: browser.page_links())
    env.define('page_images', lambda: browser.page_images())
    env.define('pageImages', lambda: browser.page_images())
    env.define('page_forms', lambda: browser.page_forms())
    env.define('pageForms', lambda: browser.page_forms())

    env.define('csv', {
        'read': lambda path: _csv_read(path),
        'write': lambda path, rows, headers=None: _csv_write(path, rows, headers),
        'parse': lambda text: _csv_parse(text),
        'encode': lambda rows, headers=None: _csv_encode(rows, headers),
    })
    env.define('csv_read', lambda path: _csv_read(path))
    env.define('csvRead', lambda path: _csv_read(path))
    env.define('csv_write', lambda path, rows, headers=None: _csv_write(path, rows, headers))
    env.define('csvWrite', lambda path, rows, headers=None: _csv_write(path, rows, headers))
    env.define('csv_parse', lambda text: _csv_parse(text))
    env.define('csvParse', lambda text: _csv_parse(text))
    env.define('csv_encode', lambda rows, headers=None: _csv_encode(rows, headers))
    env.define('csvEncode', lambda rows, headers=None: _csv_encode(rows, headers))
    env.define('json_parse', lambda text: _json_parse(text))
    env.define('jsonParse', lambda text: _json_parse(text))
    env.define('json_encode', lambda val: _json_encode(val))
    env.define('jsonEncode', lambda val: _json_encode(val))

    env.define('re', {
        'matches': lambda pattern, string: bool(__import__('re').fullmatch(str(pattern), str(string))),
        'search': lambda pattern, string: _re_search(pattern, string),
        'findall': lambda pattern, string: __import__('re').findall(str(pattern), str(string)),
        'split': lambda pattern, string: __import__('re').split(str(pattern), str(string)),
        'sub': lambda pattern, repl, string: __import__('re').sub(str(pattern), str(repl), str(string)),
    })

    env.define('http', {
        'get': lambda url, **kw: _http_request('GET', str(url), **kw),
        'post': lambda url, data=None, json=None, **kw: _http_request('POST', str(url), data, json, **kw),
        'put': lambda url, data=None, json=None, **kw: _http_request('PUT', str(url), data, json, **kw),
        'del': lambda url, **kw: _http_request('DELETE', str(url), **kw),
        'head': lambda url, **kw: _http_request('HEAD', str(url), **kw),
        'patch': lambda url, data=None, json=None, **kw: _http_request('PATCH', str(url), data, json, **kw),
    })

    if browser is not None:
        env.define('net', {
            'online': lambda: bool(browser.execute('navigator.onLine')),
            'cookies': lambda: browser.execute('document.cookie'),
            'uri': lambda: browser.url(),
        })
        env.define('cookies', {
            'all': lambda: browser.execute('() => document.cookie.split("; ").filter(Boolean).map(c => { let [n,...v] = c.split("="); return {name:n.trim(), value:v.join("=")} })'),
            'get': lambda name: browser.execute(f'() => document.cookie.split("; ").find(c => c.startsWith("{name}="))?.split("=").slice(1).join("=") || null'),
            'set': lambda name, value, path='/': browser.execute(f'() => {{ document.cookie = "{name}={value}; path={path}"; return true }}'),
            'clear': lambda: browser.execute('() => { document.cookie.split("; ").forEach(c => { let n = c.split("=")[0]; document.cookie = n + "=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/" }); return true }'),
        })
        env.define('storage', {
            'get': lambda key: browser.execute(f'() => localStorage.getItem("{key}")'),
            'set': lambda key, value: browser.execute(f'() => {{ localStorage.setItem("{key}", "{value}"); return true }}'),
            'remove': lambda key: browser.execute(f'() => {{ localStorage.removeItem("{key}"); return true }}'),
            'clear': lambda: browser.execute('() => { localStorage.clear(); return true }'),
            'all': lambda: browser.execute('() => Object.entries(localStorage).map(([k,v]) => ({key:k, value:v}))'),
        })

        env.define('page', PageModule(browser))

    env.define('random', {
        'random': lambda: _random.random(),
        'randint': lambda a, b: _random.randint(int(a), int(b)),
        'randrange': lambda start, stop=None, step=1: _random.randrange(int(start), int(stop), int(step)) if stop is not None else _random.randrange(int(start)),
        'choice': lambda seq: _random.choice(seq),
        'choices': lambda seq, k=1: [_random.choice(seq) for _ in range(int(k))],
        'sample': lambda seq, k: _random.sample(seq, int(k)),
        'shuffle': lambda seq: _random.sample(seq, len(seq)),
        'uniform': lambda a, b: _random.uniform(float(a), float(b)),
        'hex': lambda k=16: ''.join(_random.choices('0123456789abcdef', k=int(k))),
        'seed': lambda n=None: _random.seed(n),
    })

    env.define('math', {
        'pi': _math.pi,
        'e': _math.e,
        'inf': _math.inf,
        'nan': _math.nan,
        'floor': lambda x: _math.floor(x),
        'ceil': lambda x: _math.ceil(x),
        'trunc': lambda x: _math.trunc(x),
        'sqrt': lambda x: _math.sqrt(x),
        'abs': lambda x: abs(x),
        'pow': lambda x, y: _math.pow(x, y),
        'exp': lambda x: _math.exp(x),
        'log': lambda x, base=_math.e: _math.log(x, base),
        'log2': lambda x: _math.log2(x),
        'log10': lambda x: _math.log10(x),
        'sin': lambda x: _math.sin(x),
        'cos': lambda x: _math.cos(x),
        'tan': lambda x: _math.tan(x),
        'asin': lambda x: _math.asin(x),
        'acos': lambda x: _math.acos(x),
        'atan': lambda x: _math.atan(x),
        'atan2': lambda y, x: _math.atan2(y, x),
        'degrees': lambda x: _math.degrees(x),
        'radians': lambda x: _math.radians(x),
        'hypot': lambda *args: _math.hypot(*args),
        'isnan': lambda x: _math.isnan(x),
        'isfinite': lambda x: _math.isfinite(x),
        'isinf': lambda x: _math.isinf(x),
        'copysign': lambda x, y: _math.copysign(x, y),
        'gcd': lambda a, b: _math.gcd(int(a), int(b)),
        'lcm': lambda a, b: _math.lcm(int(a), int(b)),
        'factorial': lambda x: _math.factorial(int(x)),
        'comb': lambda n, k: _math.comb(int(n), int(k)),
        'perm': lambda n, k=None: _math.perm(int(n), int(k)) if k is not None else _math.perm(int(n)),
        'remainder': lambda x, y: _math.remainder(x, y),
        'fsum': lambda iterable: _math.fsum(iterable),
        'prod': lambda iterable, start=1: _math.prod(iterable, start=int(start)),
        'modf': lambda x: list(_math.modf(x)),
        'frexp': lambda x: list(_math.frexp(x)),
        'ldexp': lambda x, exp: _math.ldexp(x, int(exp)),
        'round': lambda x, ndigits=0: round(x, int(ndigits)),
    })

    env.define('time', {
        'now': lambda: _datetime.datetime.now().isoformat(),
        'unix': lambda: _time.time(),
        'utc': lambda: _datetime.datetime.now(_datetime.timezone.utc).isoformat(),
        'date': lambda: _datetime.date.today().isoformat(),
        'format': lambda fmt=None: _datetime.datetime.now().strftime(str(fmt)) if fmt else _datetime.datetime.now().isoformat(),
        'parse': lambda s, fmt: _datetime.datetime.strptime(str(s), str(fmt)).isoformat(),
        'sleep': lambda secs: _time.sleep(float(secs)),
        'wait': lambda ms: _time.sleep(float(ms) / 1000),
        'year': lambda: _datetime.datetime.now().year,
        'month': lambda: _datetime.datetime.now().month,
        'day': lambda: _datetime.datetime.now().day,
        'hour': lambda: _datetime.datetime.now().hour,
        'minute': lambda: _datetime.datetime.now().minute,
        'second': lambda: _datetime.datetime.now().second,
        'weekday': lambda: _datetime.datetime.now().weekday(),
        'timestamp': lambda: _time.time(),
    })

    env.define('os', {
        'env': lambda key, default=None: os.environ.get(str(key), default),
        'exit': lambda code=0: os._exit(int(code)),
        'platform': lambda: os.uname().sysname if hasattr(os, 'uname') else os.name,
        'hostname': lambda: _socket.gethostname(),
        'pid': lambda: os.getpid(),
        'cwd': lambda: os.getcwd(),
        'chdir': lambda path: os.chdir(str(path)),
        'name': os.name,
        'sep': os.sep,
        'linesep': os.linesep,
        'cpu_count': lambda: os.cpu_count(),
        'getenv': lambda key, default=None: os.environ.get(str(key), default),
        'setenv': lambda key, val: os.environ.__setitem__(str(key), str(val)),
        'unsetenv': lambda key: os.environ.pop(str(key), None),
        'system': lambda cmd: os.system(str(cmd)),
    })

    _COLOR_NAMES = {
        'black': 0, 'red': 1, 'green': 2, 'yellow': 3,
        'blue': 4, 'magenta': 5, 'cyan': 6, 'white': 7,
        'bright_black': 8, 'bright_red': 9, 'bright_green': 10,
        'bright_yellow': 11, 'bright_blue': 12, 'bright_magenta': 13,
        'bright_cyan': 14, 'bright_white': 15,
    }

    def _color_fn(code):
        return lambda text=None: f'\033[{code}m{text}\033[0m' if text is not None else f'\033[{code}m'

    color_mod = {
        'rgb': lambda r, g, b, text=None: f'\033[38;2;{int(r)};{int(g)};{int(b)}m{text or ""}\033[0m',
        'bg_rgb': lambda r, g, b, text=None: f'\033[48;2;{int(r)};{int(g)};{int(b)}m{text or ""}\033[0m',
        'hex': lambda h, text=None: _hex_color(str(h), text),
        'strip': lambda text: __import__('re').sub(r'\033\[[0-9;]*m', '', str(text)),
        'reset': '\033[0m',
        'bold': lambda text=None: f'\033[1m{text}\033[0m' if text is not None else '\033[1m',
        'dim': lambda text=None: f'\033[2m{text}\033[0m' if text is not None else '\033[2m',
        'italic': lambda text=None: f'\033[3m{text}\033[0m' if text is not None else '\033[3m',
        'underline': lambda text=None: f'\033[4m{text}\033[0m' if text is not None else '\033[4m',
        'blink': lambda text=None: f'\033[5m{text}\033[0m' if text is not None else '\033[5m',
        'reverse': lambda text=None: f'\033[7m{text}\033[0m' if text is not None else '\033[7m',
        'hidden': lambda text=None: f'\033[8m{text}\033[0m' if text is not None else '\033[8m',
        'strike': lambda text=None: f'\033[9m{text}\033[0m' if text is not None else '\033[9m',
    }

    def _hex_color(h, text):
        h = h.lstrip('#')
        if len(h) == 3:
            h = ''.join(c * 2 for c in h)
        r, g, b = int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)
        return f'\033[38;2;{r};{g};{b}m{text or ""}\033[0m'

    for name, code in _COLOR_NAMES.items():
        fg = 30 + code if code < 8 else 82 + code
        color_mod[name] = _color_fn(fg)
        color_mod[f'bg_{name}'] = _color_fn(40 + code if code < 8 else 92 + code)

    env.define('color', color_mod)


def _csv_read(path):
    import csv
    with open(os.path.expanduser(str(path)), 'r') as f:
        return list(csv.reader(f))

def _csv_write(path, rows, headers=None):
    import csv
    path = os.path.expanduser(str(path))
    d = os.path.dirname(path)
    if d and not os.path.exists(d):
        os.makedirs(d, exist_ok=True)
    with open(path, 'w', newline='') as f:
        w = csv.writer(f)
        if headers:
            w.writerow(headers)
        w.writerows(rows)
    return True

def _csv_parse(text):
    import csv, io
    return list(csv.reader(io.StringIO(str(text))))

def _csv_encode(rows, headers=None):
    import csv, io
    buf = io.StringIO()
    w = csv.writer(buf)
    if headers:
        w.writerow(headers)
    w.writerows(rows)
    return buf.getvalue()

def _re_search(pattern, string):
    import re
    m = re.search(str(pattern), str(string))
    if not m:
        return None
    return ZenRegexMatch(m)


def _json_parse(text):
    import json
    return json.loads(str(text))

def _json_encode(val):
    import json
    return json.dumps(val, ensure_ascii=False, separators=(',', ':'))


def _read_file(path):
    with open(os.path.expanduser(path), 'r') as f:
        return f.read()


def _write_file(path, content):
    path = os.path.expanduser(path)
    d = os.path.dirname(path)
    if d and not os.path.exists(d):
        os.makedirs(d, exist_ok=True)
    with open(path, 'w') as f:
        f.write(content)
    return True


def _append_file(path, content):
    path = os.path.expanduser(path)
    with open(path, 'a') as f:
        f.write(content)
    return True


def _read_binary(path):
    with open(path, 'rb') as f:
        return f.read()

def _write_binary(path, data):
    if isinstance(data, str):
        data = data.encode('utf-8')
    elif isinstance(data, list):
        data = bytes(data)
    with open(path, 'wb') as f:
        f.write(data)
    return True

def _exec_cmd(cmd):
    import subprocess as _sp
    result = _sp.run(cmd, shell=True, capture_output=True, text=True)
    return {
        'returncode': result.returncode,
        'stdout': result.stdout,
        'stderr': result.stderr,
    }


def _exec_python(code, globals_dict=None):
    import sys
    import io
    old_stdout = sys.stdout
    sys.stdout = io.StringIO()
    try:
        g = globals_dict if globals_dict else {}
        exec(code, g)
        result = sys.stdout.getvalue()
        return result if result else None
    finally:
        sys.stdout = old_stdout

def _exec_lua(code):
    import subprocess as _sp
    try:
        import lupa
        from lupa import LuaRuntime
        lua = LuaRuntime(unpack_returned_tuples=True)
        result = lua.execute(code)
        return result
    except ImportError:
        try:
            result = _sp.run(['lua', '-e', code], capture_output=True, text=True, timeout=10)
            if result.returncode == 0:
                return result.stdout.strip() if result.stdout.strip() else None
            raise ZenError(f"Lua error: {result.stderr}")
        except FileNotFoundError:
            raise ZenError("Lua not available — install 'lua' binary or 'lupa' Python package")

def _http_request(method, url, data=None, json=None, headers=None, timeout=30):
    if json is not None:
        data = _json.dumps(json).encode('utf-8')
        if headers is None:
            headers = {}
        if 'Content-Type' not in headers:
            headers['Content-Type'] = 'application/json'
    elif data is not None:
        data = str(data).encode('utf-8')
    req = _urllib.Request(url, data=data, method=method)
    if headers:
        for k, v in headers.items():
            req.add_header(str(k), str(v))
    try:
        resp = _urllib.urlopen(req, timeout=int(timeout))
        body = resp.read().decode('utf-8', errors='replace')
        return HttpResponse(resp.status, body, resp.getheaders())
    except _urllib.HTTPError as e:
        body = e.read().decode('utf-8', errors='replace')
        return HttpResponse(e.code, body, e.headers)
    except Exception as e:
        raise ZenError(f'HTTP {method} {url}: {e}')


def _parse_duration(dur):
    if isinstance(dur, (int, float)):
        return int(dur)
    s = str(dur).strip().lower()
    if s.endswith('m'):
        return float(s[:-1]) * 60000
    if s.endswith('ms'):
        return float(s[:-2])
    if s.endswith('s'):
        return float(s[:-1]) * 1000
    return float(s)
