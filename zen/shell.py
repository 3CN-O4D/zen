import sys
import os
import warnings
import asyncio
from .lexer import Lexer, LexerError
from .parser import Parser, ParseError
from .interpreter import Interpreter, ZenError, _VOID
from .environment import ZenElement, ZenList, ZenMethod, ZenRegexMatch
from .browser import Browser
from . import __version__

warnings.filterwarnings('ignore', message='coroutine.*was never awaited')

try:
    import readline
    _HAS_READLINE = True
    _ZEN_HISTFILE = os.path.expanduser('~/.z_history')
    try:
        readline.read_history_file(_ZEN_HISTFILE)
    except (FileNotFoundError, OSError):
        pass
    readline.set_history_length(1000)
except ImportError:
    _HAS_READLINE = False

try:
    from prompt_toolkit import PromptSession
    from prompt_toolkit.history import FileHistory
    from prompt_toolkit.auto_suggest import AutoSuggestFromHistory
    from prompt_toolkit.completion import Completer, Completion
    from prompt_toolkit.styles import Style
    HAS_PT = True
except ImportError:
    HAS_PT = False


_ZEN_KEYWORDS = [
    'let', 'go', 'fill', 'with', 'click', 'wait', 'for', 'in',
    'if', 'else', 'while', 'function', 'def', 'return', 'print', 'input',
    'into', 'scroll', 'to', 'by', 'shot', 'full', 'refresh',
    'back', 'forward', 'execute', 'download', 'and', 'or', 'not',
    'true', 'false', 'null', 'try', 'catch', 'top', 'bottom',
    'break', 'continue', 'include',
    'first', 'nth', 'text', 'texts', 'attr', 'attrs',
]

_ZEN_BUILTINS = [
    'print', 'type', 'len', 'str', 'int', 'float', 'range', 'interval',
    'abs', 'min', 'max', 'round',
    'assert', 'assert_eq', 'assertEq',
    'go', 'fill', 'click', 'check', 'uncheck', 'select', 'text', 'texts', 'attr', 'attrs',
    'wait', 'wait_for', 'waitFor', 'refresh', 'back', 'forward',
    'shot', 'execute', 'url', 'title', 'find', 'find_all', 'findAll',
    'first', 'nth', 'download', 'css', 'by_text', 'byText',
    'input', 'input_str', 'inputStr', 'sleep', 'search',
    'find_by_text', 'findByText', 'find_by_url', 'findByUrl',
    'read_file', 'readFile', 'write_file', 'writeFile',
    'append_file', 'appendFile', 'file_exists', 'fileExists',
    'list_dir', 'listDir', 'mkdir', 'remove_file', 'removeFile',
    'rename_file', 'renameFile', 'copy_file', 'copyFile',
    'move_file', 'moveFile', 'path_join', 'pathJoin',
    'basename', 'dirname', 'cwd', 'pwd', 'cd', 'chdir',
    'read_binary', 'readBinary', 'write_binary', 'writeBinary',
    'rmdir', 'remove_dir', 'removeDir',
    'glob', 'file_size', 'fileSize', 'file_mtime', 'fileMtime',
    'is_file', 'isFile', 'is_dir', 'isDir',
    'exec', 'sh', 'system',
    'history',
    'scroll_to', 'scrollTo', 'page_html', 'pageHtml',
    'page_text', 'pageText', 'page_links', 'pageLinks',
    'page_images', 'pageImages', 'page_forms', 'pageForms',
    'page_inputs', 'pageInputs', 'page_buttons', 'pageButtons',
    'prompt', 'confirm',
    'json_parse', 'jsonParse', 'json_encode', 'jsonEncode',
    'csv_read', 'csvRead', 'csv_write', 'csvWrite',
    'csv_parse', 'csvParse', 'csv_encode', 'csvEncode',
    'user_agent', 'userAgent', 'set_user_agent', 'setUserAgent',
    'set_headers', 'setHeaders', 'headers',
]

_ZEN_SPECIALS = ['_url', '__url', '___url', '_time', '_date', '_dir', '_version', '_', '_timeout', '_page_html', '_page_text', '_page_links', '_page_images', '_page_urls', '_page_forms', '_page_inputs', '_page_buttons', 'user_agent', 'headers']

_ZEN_BUILTIN_HELP = {
    'go': 'go "url" — Navigate to a URL',
    'find': 'find("css_selector") → ZenElement — Find first matching element',
    'find_all': 'find_all("css_selector") → ZenList — Find all matching elements',
    'findAll': 'findAll("css_selector") → ZenList — Alias for find_all',
    'click': 'click("css_selector") — Click first matching element',
    'fill': 'fill("css_selector", "value") — Fill input field',
    'check': 'check("css_selector") — Check a checkbox or radio button',
    'uncheck': 'uncheck("css_selector") — Uncheck a checkbox',
    'select': 'select("css_selector", "value") — Select option from dropdown',
    'wait_for': 'wait_for("css_selector") — Wait for element to appear',
    'wait': 'wait(ms) — Wait for given milliseconds',
    'sleep': 'sleep(seconds) — Sleep for given seconds',
    'print': 'print(value) — Print to console',
    'title': 'title() → str — Get page title',
    'url': 'url() → str — Get current URL',
    'shot': 'shot("filename.png") — Take screenshot',
    'refresh': 'refresh() — Reload current page',
    'back': 'back() — Go back in history',
    'forward': 'forward() — Go forward in history',
    'execute': 'execute("js_code") → any — Run JavaScript in page',
    'scroll_to': 'scroll_to(x, y) or scroll_to("bottom") — Scroll page',
    'page_html': 'page_html() → str — Get full page HTML',
    'page_text': 'page_text() → str — Get visible text with media markers',
    'page_links': 'page_links() → list — Get all links on page',
    'page_images': 'page_images() → list — Get all image URLs',
    'page_forms': 'page_forms() → list — Get all forms on page',
    'history': 'history() → list — Get navigation history',
    'read_file': 'read_file("path") → str — Read file contents',
    'write_file': 'write_file("path", "content") — Write to file',
    'append_file': 'append_file("path", "content") — Append to file',
    'file_exists': 'file_exists("path") → bool — Check if file exists',
    'list_dir': 'list_dir("path") → list — List directory contents',
    'mkdir': 'mkdir("path") — Create directory',
    'remove_file': 'remove_file("path") — Delete file',
    'copy_file': 'copy_file("src", "dst") — Copy file',
    'move_file': 'move_file("src", "dst") — Move/rename file',
    'rename_file': 'rename_file("old", "new") — Rename file',
    'path_join': 'path_join(a, b) → str — Join path components',
    'type': 'type(value) → str — Get type name of value',
    'len': 'len(value) → int — Get length of string, list, or dict',
    'str': 'str(value) → str — Convert value to string',
    'int': 'int(value) → int — Convert value to integer',
    'float': 'float(value) → float — Convert value to float',
    'input': 'input("prompt", type?) → any — Read input, optionally convert to type (int, float, bool, list, dict)',
    'input_str': 'input_str("prompt", type?) → any — Alias for input',
    'prompt': 'prompt("msg") → str — Prompt for user input',
    'confirm': 'confirm("msg") → bool — Ask yes/no confirmation',
    'download': 'download("url", "path") — Download file from URL',
    'search': 'search("text") → ZenList — Find elements by text (CSS, /regex/, text=..., url=...)',
    'find_first': 'find_first("css_selector") → ZenElement — Alias for find',
    'find_by_text': 'find_by_text("visible text", exact=False) → ZenElement — Find element by visible text content',
    'find_by_url': 'find_by_url("url", partial=True) → ZenElement — Find link by href URL',
    'json_parse': 'json_parse("json_str") → any — Parse JSON string',
    'json_encode': 'json_encode(value) → str — Encode value to JSON string',
    'csv_read': 'csv_read("path") → list — Read CSV file into list of rows',
    'csv_write': 'csv_write("path", rows, headers=None) — Write rows to CSV file',
    'csv_parse': 'csv_parse("csv_text") → list — Parse CSV text into list of rows',
    'csv_encode': 'csv_encode(rows, headers=None) → str — Encode rows as CSV text',
    'break': 'break — Exit the current loop',
    'continue': 'continue — Skip to next iteration of the current loop',
    '_page_forms': '_page_forms — List of all forms on the current page',
    'user_agent': 'user_agent() → str — Get current browser user-agent string',
    'userAgent': 'userAgent() → str — Alias for user_agent',
    'set_user_agent': 'set_user_agent("ua_string") — Override navigator.userAgent',
    'setUserAgent': 'setUserAgent("ua_string") — Alias for set_user_agent',
    'set_headers': 'set_headers({"Header": "value"}) — Set extra HTTP headers for all requests',
    'setHeaders': 'setHeaders({"Header": "value"}) — Alias for set_headers',
    'headers': 'headers() → dict — Get currently set extra HTTP headers',
    'range': 'range(end) → list — Numbers from 0 to end-1; range(start, end, step)',
    'interval': 'interval(start, end, step=1) → list — Numbers from start to end-1',
    'abs': 'abs(v) → number — Absolute value',
    'min': 'min(a, b, ...) → number — Minimum of values',
    'max': 'max(a, b, ...) → number — Maximum of values',
    'round': 'round(v, ndigits=0) → number — Round to given precision',
    'assert': 'assert(cond, msg?) — Raise error if cond is falsy',
    'assert_eq': 'assert_eq(a, b, msg?) — Raise error if a != b',
    'assertEq': 'assertEq(a, b, msg?) — Alias for assert_eq',
}


if HAS_PT:
    class ZenCompleter(Completer):
        def get_completions(self, document, complete_event):
            word = document.get_word_before_cursor()
            if not word:
                return
            word_lower = word.lower()
            candidates = _ZEN_KEYWORDS + _ZEN_BUILTINS + _ZEN_SPECIALS
            for c in candidates:
                if c.startswith(word_lower) or c.startswith('_' + word_lower):
                    yield Completion(c, start_position=-len(word))


if HAS_PT:
    _SHELL_STYLE = Style.from_dict({
        'prompt': 'ansicyan bold',
    })


def _show_error_context(code, line, col, message):
    lines = code.split('\n')
    start = max(0, line - 2)
    end = min(len(lines), line + 1)
    for i in range(start, end):
        prefix = '>>>' if i == line - 1 else '   '
        print(f'  {prefix} {lines[i]}')
    print(f'      {" " * (col - 1)}^')
    print(f'  \033[1;31mError: {message}\033[0m')

def _show_error_context_for_token(code, token, message):
    if token:
        _show_error_context(code, token.line, token.col, message)
    else:
        print(f'  \033[1;31mError: {message}\033[0m')


def _format_result(val):
    if val is None:
        return 'null'
    if isinstance(val, bool):
        return 'true' if val else 'false'
    if isinstance(val, str):
        return val
    if isinstance(val, (int, float)):
        return str(val)
    if isinstance(val, (list, tuple)):
        return '[' + ', '.join(_format_result(v) for v in val) + ']'
    if isinstance(val, dict):
        return '{' + ', '.join(f'{k}: {_format_result(v)}' for k, v in val.items()) + '}'
    if isinstance(val, ZenElement):
        try:
            t = val.tag
            s = val.text[:60].replace('\n', ' ')
            return f'<{t}: {s}>'
        except Exception:
            return f'<Element>'
    if isinstance(val, ZenList):
        return f'<{val.count} elements>'
    if isinstance(val, ZenMethod):
        return f'<method {val._name}>'
    if isinstance(val, ZenRegexMatch):
        return repr(val)
    return str(val)

class Shell:

    def __init__(self, headless=True, browser_path=None, connect_port=None, mode='browser'):
        self.browser = Browser(headless=headless, browser_path=browser_path,
                               connect_port=connect_port, mode=mode)
        self.interpreter = None
        self.history = []
        self._pt_session = None
        self._use_pt = HAS_PT
        self._last_cmd_time = 0
        self._busy = False
        self._buf = []
        self._prompt_prefix = ''

    def _build_prompt(self, for_readline=False):
        page_count = len(self.browser.url_history) if self.browser and hasattr(self.browser, 'url_history') else 0
        title = ''
        if self.browser and hasattr(self.browser, 'current_url'):
            try:
                if self.browser.current_url and self.browser.current_url != 'about:blank':
                    t = self.browser.title()
                    if t:
                        title = ' ' + t[:40]
            except Exception:
                pass

        if for_readline:
            cyan = '\001\033[1;36m\002'
            reset = '\001\033[0m\002'
            dim = '\001\033[2m\002'
            yellow = '\001\033[1;33m\002'
        else:
            cyan = '\033[1;36m'
            reset = '\033[0m'
            dim = '\033[2m'
            yellow = '\033[1;33m'

        if self._buf:
            return f'\033[33m...\033[0m '
        if page_count > 0 and title:
            return f'{cyan}zen{dim}[{page_count}]{reset}{dim}{title}{reset} {cyan}❯{reset} '
        elif page_count > 0:
            return f'{cyan}zen{dim}[{page_count}]{reset} {cyan}❯{reset} '
        else:
            return f'{cyan}zen ❯{reset} '

    def start(self):
        banner = f'\033[1;36mZen v{__version__} — Browser Automation Shell\033[0m'
        print(banner)
        if not HAS_PT:
            print('\033[1;33mTip:\033[0m install \033[1;36mprompt-toolkit\033[0m for auto-completion, syntax highlighting, and more (\033[1;33mpip install prompt-toolkit\033[0m)')
        print('Type \033[1;33m.help\033[0m for commands, \033[1;33m.exit\033[0m to quit')
        print()

        self.browser.start()
        self.interpreter = Interpreter(self.browser)

        if self._use_pt:
            try:
                hist_file = os.path.expanduser('~/.z_history')
                self._pt_session = PromptSession(
                    history=FileHistory(hist_file) if os.access(os.path.dirname(hist_file), os.W_OK) else None,
                    auto_suggest=AutoSuggestFromHistory(),
                    completer=ZenCompleter(),
                    style=_SHELL_STYLE,
                )
            except Exception:
                self._use_pt = False

        self._run()

    def _run(self):
        while True:
            try:
                if self._use_pt:
                    try:
                        line = self._pt_session.prompt(self._build_prompt(for_readline=False)).strip()
                    except RuntimeError as e:
                        if 'asyncio.run() cannot be called' in str(e):
                            self._use_pt = False
                            line = input(self._build_prompt(for_readline=True)).strip()
                        else:
                            raise
                else:
                    line = input(self._build_prompt(for_readline=True)).strip()
            except EOFError:
                print()
                break
            except KeyboardInterrupt:
                self._buf = []
                self._prompt_prefix = ''
                print()
                continue

            if not line:
                if self._buf:
                    full_code = '\n'.join(self._buf)
                    self._buf = []
                    self._prompt_prefix = ''
                    self.history.append(full_code)
                    try:
                        self._execute(full_code)
                    except KeyboardInterrupt:
                        print('\n[Interrupted]')
                continue

            if line.startswith('.'):
                if self._buf:
                    self._buf = []
                    self._prompt_prefix = ''
                self._handle_dot_command(line)
                continue

            self._buf.append(line)
            code = '\n'.join(self._buf)
            if self._is_complete(code):
                full_code = '\n'.join(self._buf)
                self._buf = []
                self._prompt_prefix = ''
                self.history.append(full_code)
                try:
                    self._execute(full_code)
                except KeyboardInterrupt:
                    print('\n[Interrupted]')
            else:
                self._prompt_prefix = '... '

    def _handle_dot_command(self, line):
        cmd = line[1:].strip().split()
        if not cmd:
            return

        if cmd[0] in ('exit', 'quit', 'q'):
            print('Goodbye!')
            sys.exit(0)

        elif cmd[0] == 'help':
            if len(cmd) > 1:
                expr = ' '.join(cmd[1:])
                self._show_help(expr)
            else:
                self._show_general_help()
            return

        elif cmd[0] == 'clear':
            print('\033[2J\033[H', end='')
            return

        elif cmd[0] == 'url':
            try:
                print(self.browser.current_url)
            except Exception as e:
                print(f'Error: {e}')
            return

        elif cmd[0] == 'title':
            try:
                from .builtins import _execute_builtin
                print(self.browser.title())
            except Exception as e:
                print(f'Error: {e}')
            return

        elif cmd[0] == 'vars':
            if self.interpreter:
                env = self.interpreter.current_env
                builtin_names = set(_ZEN_BUILTIN_HELP.keys()) | {
                    'by_text', 'css', 'first', 'nth',
                    'find_by_text', 'find_by_url',
                }
                shown = False
                for k in sorted(env.vars):
                    if k.startswith('_') or k in builtin_names:
                        continue
                    v = env.vars[k]
                    print(f'  {k} = {_format_result(v)}')
                    shown = True
                if not shown:
                    print('  (no user variables)')
                print()
                print('  Special vars: _url, __url, ___url, _time, _date, _dir, _version, _timeout, _page_html, _page_text, _page_links, _page_images, _page_urls, _page_forms')
            return

        elif cmd[0] == 'history':
            urls = self.browser.url_history
            if urls:
                for i, u in enumerate(urls):
                    marker = ' <-- current' if i == len(urls) - 1 else ''
                    print(f'  [{i}] {u}{marker}')
            else:
                print('  (no history)')
            return

        elif cmd[0] == 'run':
            if len(cmd) < 2:
                print('Usage: .run <filename.z>')
                return
            path = ' '.join(cmd[1:])
            path = os.path.expanduser(path)
            if not os.path.exists(path):
                print(f'File not found: {path}')
                return
            try:
                from .utils import read_file
                code = read_file(path)
                self._execute(code)
            except KeyboardInterrupt:
                print('\n[Interrupted]')
            except Exception as e:
                print(f'Error: {e}')
            return

        elif cmd[0] == 'shot':
            if len(cmd) < 2:
                print('Usage: .shot <filename.png>')
                return
            try:
                self.browser.shot(cmd[1])
                print(f'Screenshot saved: {cmd[1]}')
            except Exception as e:
                print(f'Error: {e}')
            return

        elif cmd[0] == 'type':
            expr = ' '.join(cmd[1:])
            if expr:
                self._execute(f'print type({expr})')
            return

        elif cmd[0] == 'dir':
            print(os.getcwd())
            return

    def _show_general_help(self):
        print('\033[1;33mZen — Browser Automation Language\033[0m')
        print()
        print('\033[1;36mData Types\033[0m')
        print('  \033[33mNumber\033[0m  42, 3.14, -7')
        print('  \033[33mString\033[0m  "hello", \'world\'')
        print('  \033[33mBoolean\033[0m true, false')
        print('  \033[33mNull\033[0m    null')
        print('  \033[33mList\033[0m    [1, 2, 3]')
        print('  \033[33mDict\033[0m    {"a": 1}')
        print()
        print('\033[1;36mOperators\033[0m')
        print('  \033[33mArithmetic:\033[0m  +  -  *  /  %  **')
        print('  \033[33mComparison:\033[0m  ==  !=  <  >  <=  >=')
        print('  \033[33mMembership:\033[0m  in  not in')
        print('  \033[33mLogical:\033[0m     and  or  not')
        print()
        print('\033[1;36mControl Flow\033[0m')
        print('  \033[33mif\033[0m cond { } \033[33melse\033[0m { }')
        print('  \033[33mwhile\033[0m cond { }')
        print('  \033[33mfor\033[0m x in iterable { }')
        print('  \033[33mfunction\033[0m name(params) { }')
        print('  \033[33mreturn\033[0m value')
        print('  \033[33mbreak\033[0m — exit current loop')
        print('  \033[33mcontinue\033[0m — skip to next iteration')
        print('  \033[33mtry\033[0m { } \033[33mcatch\033[0m err { }   (optional error var)')
        print('  \033[33mtry\033[0m { } \033[33mcatch\033[0m { } \033[33mfinally\033[0m { }')
        print('  \033[33minclude "file.z"\033[0m  — Include and run another Zen file')
        print()
        print('\033[1;36mSelectors\033[0m')
        print('  \033[33mCSS:\033[0m       find("div.class"), find_all("a[href]")')
        print('  \033[33mText:\033[0m      find_by_text("Click Here")   (visible text)')
        print('  \033[33mURL:\033[0m       find_by_url("example.com")  (link href)')
        print('  \033[33mRegex:\033[0m     click "/submit|save/i"')
        print('  \033[33mText sel:\033[0m  click by_text("Login")')
        print()
        print('\033[1;36mModules\033[0m')
        print('  \033[33mre.matches(pattern, str)\033[0m  → bool  — Full match test')
        print('  \033[33mre.search(pattern, str)\033[0m    → ZenRegexMatch — First match with .group(), .groups(), .start, .end, .match')
        print('  \033[33mre.findall(pattern, str)\033[0m   → list  — All non-overlapping matches')
        print('  \033[33mre.split(pattern, str)\033[0m     → list  — Split by regex')
        print('  \033[33mre.sub(pattern, repl, str)\033[0m → str   — Replace matches')
        print()
        print('  \033[33m\033[1mhttp module:\033[0m')
        print('  \033[33mhttp.get(url)\033[0m         → Response  — HTTP GET')
        print('  \033[33mhttp.post(url, data)\033[0m   → Response  — HTTP POST')
        print('  \033[33mhttp.put(url, data)\033[0m    → Response  — HTTP PUT')
        print('  \033[33mhttp.del(url)\033[0m          → Response  — HTTP DELETE')
        print('  \033[33mhttp.head(url)\033[0m         → Response  — HTTP HEAD')
        print('  \033[33mhttp.patch(url, data)\033[0m  → Response  — HTTP PATCH')
        print('  Response methods: .status, .body, .headers, .json(), .ok')
        print()
        print('  \033[33m\033[1mcookies module:\033[0m')
        print('  \033[33mcookies.all()\033[0m    → list  — All cookies as [{name,value}]')
        print('  \033[33mcookies.get(name)\033[0m → str  — Get cookie value')
        print('  \033[33mcookies.set(n,v,path)\033[0m → bool — Set cookie')
        print('  \033[33mcookies.clear()\033[0m  → bool  — Clear all cookies')
        print()
        print('  \033[33m\033[1mstorage module:\033[0m')
        print('  \033[33mstorage.get(key)\033[0m  → str   — localStorage.getItem')
        print('  \033[33mstorage.set(k,v)\033[0m  → bool  — localStorage.setItem')
        print('  \033[33mstorage.remove(k)\033[0m → bool  — localStorage.removeItem')
        print('  \033[33mstorage.clear()\033[0m   → bool  — localStorage.clear')
        print('  \033[33mstorage.all()\033[0m     → list  — All items as [{key,value}]')
        print()
        print('  \033[33mnet.online()\033[0m   → bool — Check if browser is online')
        print('  \033[33mnet.cookies()\033[0m  → str  — Get document.cookie')
        print('  \033[33mnet.url()\033[0m      → str  — Get current URL')
        print('  \033[33mcsv.read("path")\033[0m        → list  — Read CSV file')
        print('  \033[33mcsv.write("path", rows)\033[0m → bool  — Write CSV file')
        print('  \033[33mcsv.parse("text")\033[0m       → list  — Parse CSV string')
        print('  \033[33mcsv.encode(rows)\033[0m        → str   — Encode to CSV string')
        print('  Flat aliases: csv_read, csv_write, csv_parse, csv_encode')
        print()
        print('  \033[33m\033[1mfs module (filesystem):\033[0m')
        print('  \033[33mfs.list(path)\033[0m      → list  — List directory')
        print('  \033[33mfs.read(path)\033[0m      → str   — Read text file')
        print('  \033[33mfs.write(path, c)\033[0m  → bool  — Write text file')
        print('  \033[33mfs.exists(path)\033[0m    → bool  — Check existence')
        print('  \033[33mfs.is_file(path)\033[0m   → bool  — Is regular file?')
        print('  \033[33mfs.is_dir(path)\033[0m    → bool  — Is directory?')
        print('  \033[33mfs.size(path)\033[0m      → int   — File size in bytes')
        print('  \033[33mfs.mtime(path)\033[0m     → float — Last modified time')
        print('  \033[33mfs.mkdir(path)\033[0m     → bool  — Create directory')
        print('  \033[33mfs.remove(path)\033[0m    → bool  — Remove file')
        print('  \033[33mfs.rmdir(path)\033[0m     → bool  — Remove empty dir')
        print('  \033[33mfs.rmtree(path)\033[0m    → bool  — Remove dir recursively')
        print('  \033[33mfs.copy(src, dst)\033[0m  → bool  — Copy file')
        print('  \033[33mfs.move(src, dst)\033[0m  → bool  — Move/rename file')
        print('  \033[33mfs.glob(pattern)\033[0m   → list  — Glob file matching')
        print('  \033[33mfs.join(a, b...)\033[0m   → str   — Join path parts')
        print('  \033[33mfs.cwd()\033[0m           → str   — Current directory')
        print('  \033[33mfs.cd(path)\033[0m        → bool  — Change directory')
        print('  \033[33mfs.exec(cmd)\033[0m       → dict  — Run shell command ({returncode, stdout, stderr})')
        print('  \033[33mfs.basename(path)\033[0m  → str   — Last path component')
        print('  \033[33mfs.dirname(path)\033[0m   → str   — Parent directory')
        print()
        print('  \033[33m\033[1mpage module:\033[0m')
        print('  \033[33mpage.html\033[0m      → str   — Full page HTML')
        print('  \033[33mpage.text\033[0m      → str   — Visible page text')
        print('  \033[33mpage.links\033[0m     → list  — All link URLs')
        print('  \033[33mpage.images\033[0m    → list  — All image URLs')
        print('  \033[33mpage.forms\033[0m     → list  — All forms with inputs')
        print('  \033[33mpage.inputs\033[0m    → list  — All input/select/textarea fields')
        print('  \033[33mpage.buttons\033[0m   → list  — All buttons and clickable elements')
        print('  \033[33mpage.title\033[0m     → str   — Page title')
        print('  \033[33mpage.url\033[0m       → str   — Current URL')
        print('  \033[33mpage.source\033[0m    → str   — Alias for page.html')
        print()
        print('  \033[33m\033[1mUser agent & headers:\033[0m')
        print('  \033[33muser_agent()\033[0m       → str   — Get browser user-agent')
        print('  \033[33mset_user_agent("...")\033[0m — Override navigator.userAgent')
        print('  \033[33mset_headers({...})\033[0m  — Set extra HTTP headers for all requests')
        print('  \033[33mheaders()\033[0m          → dict  — Currently set extra headers')
        print()
        print('  \033[33m\033[1mSequence & math builtins:\033[0m')
        print('  \033[33mrange(end)\033[0m         → list  — 0..end-1')
        print('  \033[33mrange(start, end)\033[0m   → list  — start..end-1')
        print('  \033[33mrange(start, end, step)\033[0m → list')
        print('  \033[33minterval(start, end)\033[0m → list  — alias for range')
        print('  \033[33minterval(start, end, step)\033[0m → list')
        print('  \033[33mabs(v)\033[0m              → n     — Absolute value')
        print('  \033[33mmin(a, b...)\033[0m        → n     — Minimum')
        print('  \033[33mmax(a, b...)\033[0m        → n     — Maximum')
        print('  \033[33mround(v, n?)\033[0m        → n     — Round')
        print('  \033[33massert(cond, msg?)\033[0m  — Raise error if falsy')
        print('  \033[33massert_eq(a, b, msg?)\033[0m — Raise error if a != b')
        print()
        print('  \033[33m\033[1mAlso flat builtins:\033[0m')
        print('  \033[33mread_file\033[0m, \033[33mwrite_file\033[0m, \033[33mappend_file\033[0m, \033[33mfile_exists\033[0m, \033[33mlist_dir\033[0m,')
        print('  \033[33mmkdir\033[0m, \033[33mremove_file\033[0m, \033[33mcopy_file\033[0m, \033[33mmove_file\033[0m, \033[33mrename_file\033[0m,')
        print('  \033[33mpath_join\033[0m, \033[33mcwd\033[0m, \033[33mcd\033[0m, \033[33mglob\033[0m, \033[33mexec\033[0m, \033[33msh\033[0m, \033[33msystem\033[0m')
        print()
        print('\033[1;36mBrowser Builtins\033[0m')
        for k in sorted(_ZEN_BUILTIN_HELP):
            v = _ZEN_BUILTIN_HELP[k]
            print(f'  \033[33m{k}\033[0m  {v}')
        print()
        print('\033[1;36mSpecial Variables\033[0m')
        print('  \033[33m_url\033[0m         Current page URL')
        print('  \033[33m__url\033[0m        Previous page URL')
        print('  \033[33m___url\033[0m       Page before that')
        print('  \033[33m_time\033[0m        Current time (HH:MM:SS)')
        print('  \033[33m_date\033[0m        Current date (YYYY-MM-DD)')
        print('  \033[33m_dir\033[0m         Working directory')
        print('  \033[33m_version\033[0m     Zen version')
        print('  \033[33m_\033[0m            Last expression result')
        print('  \033[33m_timeout\033[0m     Default timeout (ms, "3s", "1.5m")')
        print('  \033[33m_page_html\033[0m   Raw page HTML')
        print('  \033[33m_page_text\033[0m   Visible text with {[media]} markers')
        print('  \033[33m_page_links\033[0m  All link URLs on page')
        print('  \033[33m_page_images\033[0m All image URLs on page')
        print('  \033[33m_page_urls\033[0m   All visited URLs this session')
        print('  \033[33m_page_forms\033[0m   All forms on page')
        print('  \033[33m_page_inputs\033[0m  All input/select/textarea fields')
        print('  \033[33m_page_buttons\033[0m All buttons and clickable elements')
        print()
        print('\033[1;36mElement Methods (ZenElement)\033[0m')
        print('  \033[33m.text\033[0m            \033[33m.html\033[0m            \033[33m.exists\033[0m            \033[33m.tag\033[0m')
        print('  \033[33m.attr("href")\033[0m    \033[33m.click()\033[0m         \033[33m.fill("val")\033[0m')
        print('  \033[33m.check()\033[0m         \033[33m.uncheck()\033[0m       \033[33m.select("opt")\033[0m    \033[33m.hover()\033[0m')
        print('  \033[33m.screenshot("path")\033[0m')
        print('  \033[33m.find("sel")\033[0m     \033[33m.find_all("sel")\033[0m')
        print('  \033[33m.play()\033[0m          \033[33m.pause()\033[0m         \033[33m.download("path")\033[0m')
        print('  \033[33m.muted\033[0m           \033[33m.volume\033[0m          \033[33m.current_time\033[0m')
        print('  \033[33m.duration\033[0m        \033[33m.paused\033[0m          \033[33m.ended\033[0m           \033[33m.loop\033[0m')
        print()
        print('\033[1;36mList Methods (ZenList)\033[0m')
        print('  \033[33m.texts\033[0m  \033[33m.htmls\033[0m  \033[33m.tags\033[0m  \033[33m.count\033[0m  \033[33m.len\033[0m  \033[33m.first\033[0m')
        print('  \033[33m.nth(n)\033[0m  \033[33m.attr(n)\033[0m  \033[33m.attrs(n)\033[0m  \033[33m.each(fn)\033[0m  \033[33m.sorted()\033[0m')
        print()
        print('\033[1;36mString / List / Dict / Number Methods\033[0m')
        print('  \033[33mString:\033[0m  .upper()  .lower()  .split()  .join()  .replace()  .strip()')
        print('               .startswith()  .endswith()  .find()  .len  .format()')
        print('  \033[33mList:\033[0m    .append()  .pop()  .sort()  .reverse()  .clear()  .len  .sorted()')
        print('               .push()  .shift()  .unshift()  .includes()  .indexOf()  .join()')
        print('  \033[33mDict:\033[0m    .keys()  .values()  .items()  .get()  .put()  .len  .clear()')
        print('  \033[33mNumber:\033[0m  .times(fn)  .str()  .float()  .type')
        print('  \033[33mAny:\033[0m     .str()  .int()  .float()  .bool()  .type')
        print()
        print('\033[1;36mSlice Syntax\033[0m')
        print('  \033[33mlist[start:end]\033[0m  \033[33mlist[start:]\033[0m  \033[33mlist[:end]\033[0m  \033[33mlist[::step]\033[0m')
        print()
        print('\033[1;36mShell Commands\033[0m')
        print('  \033[33m.exit\033[0m / \033[33m.quit\033[0m   Exit the shell')
        print('  \033[33m.help\033[0m [expr]    Show this help or expression details')
        print('  \033[33m.clear\033[0m          Clear screen')
        print('  \033[33m.url\033[0m            Show current URL')
        print('  \033[33m.title\033[0m           Show page title')
        print('  \033[33m.vars\033[0m           Show user variables')
        print('  \033[33m.history\033[0m         Show URL navigation history')
        print('  \033[33m.run\033[0m <file>      Run a .z script file')
        print('  \033[33m.shot\033[0m <file>     Take page screenshot')
        print('  \033[33m.type\033[0m <expr>     Show type of expression')
        print('  \033[33m.dir\033[0m             Show current directory')
        print()
        print('For detailed help on a specific function or expression, type \033[33m.help <expr>\033[0m')
        print('  e.g., \033[33m.help find\033[0m, \033[33m.help find_all\033[0m, \033[33m.help _url\033[0m, \033[33m.help \"hello\"\033[0m')

    def _show_help(self, expr):
        _NO_VALUE = object()
        val = _NO_VALUE
        eval_ok = False
        doc = _ZEN_BUILTIN_HELP.get(expr)

        dot_cmds = {
            '.exit': 'Exit the shell',
            '.quit': 'Exit the shell',
            '.help': 'Show this help or expression details',
            '.clear': 'Clear screen',
            '.url': 'Show current URL',
            '.title': 'Show page title',
            '.vars': 'Show user variables',
            '.history': 'Show URL navigation history',
            '.run': 'Run a .z script file',
            '.shot': 'Take page screenshot',
            '.type': 'Show type of expression',
            '.dir': 'Show current directory',
        }

        if expr in dot_cmds:
            print(f'\033[1;33m{expr}\033[0m')
            print()
            print(f'  \033[1;36mShell Command: {dot_cmds[expr]}\033[0m')
            print()
            return

        if expr == 'keywords':
            print('\033[1;33mAll help topics\033[0m')
            print()
            for k in sorted(_ZEN_BUILTIN_HELP):
                print(f'  \033[33m{k}\033[0m')
            for k in sorted(dot_cmds):
                print(f'  \033[33m.{k}\033[0m')
            for k in ['include', 'csv', 're', 'net', 'http', 'cookies', 'storage', 'search']:
                print(f'  \033[33m{k}\033[0m')
            print()
            return

        if expr == 'include':
            print('\033[1;33minclude\033[0m')
            print()
            print('  \033[1;36minclude "file.z"\033[0m — Include and run another Zen file')
            print()
            print('  All functions, variables, and assignments in the included file')
            print('  become available in the current scope.')
            print()
            return

        if expr == 'csv':
            print('\033[1;33mcsv\033[0m')
            print()
            print('  \033[1;36mModule for CSV processing\033[0m')
            print('  \033[33mcsv.read("path")\033[0m       → list — Read CSV file')
            print('  \033[33mcsv.write("path", rows)\033[0m  → bool — Write CSV file')
            print('  \033[33mcsv.parse("text")\033[0m      → list — Parse CSV string')
            print('  \033[33mcsv.encode(rows)\033[0m       → str — Encode to CSV string')
            print('  Flat aliases: csv_read, csv_write, csv_parse, csv_encode')
            print()
            return

        if expr == 're':
            print('\033[1;33mre\033[0m')
            print()
            print('  \033[1;36mModule for regular expressions\033[0m')
            print('  \033[33mre.matches(pattern, str)\033[0m  → bool — Test if whole string matches')
            print('  \033[33mre.search(pattern, str)\033[0m    → ZenRegexMatch — First match')
            print('  \033[33mre.findall(pattern, str)\033[0m   → list — All matches')
            print('  \033[33mre.split(pattern, str)\033[0m     → list — Split by pattern')
            print('  \033[33mre.sub(pattern, repl, str)\033[0m → str — Replace matches')
            print()
            print('  \033[1;36mZenRegexMatch methods:\033[0m')
            print('    \033[33m.match\033[0m   — Full matched text (str)')
            print('    \033[33m.start\033[0m   — Start position (int)')
            print('    \033[33m.end\033[0m     — End position (int)')
            print('    \033[33m.group(n)\033[0m — Capture group n (n=0 for full match)')
            print('    \033[33m.groups()\033[0m — List of all capture groups')
            print()
            return

        if expr == 'net':
            print('\033[1;33mnet\033[0m')
            print()
            print('  \033[1;36mModule for browser network info\033[0m')
            print('  \033[33mnet.online()\033[0m   → bool — Is browser online?')
            print('  \033[33mnet.cookies()\033[0m  → str — Get document.cookie')
            print('  \033[33mnet.url()\033[0m      → str — Get current URL')
            print()
            return

        if expr == 'search':
            print('\033[1;33msearch\033[0m')
            print()
            print('  \033[1;36mFind elements by flexible query\033[0m')
            print('  \033[33msearch("text")\033[0m       → Find by visible text (e.g., search("Login"))')
            print('  \033[33msearch("div.class")\033[0m  → Find by CSS selector')
            print('  \033[33msearch("/pattern/")\033[0m  → Find by regex text match')
            print('  \033[33msearch("text=...")\033[0m   → Find by exact text')
            print('  \033[33msearch("url=...")\033[0m    → Find by link URL')
            print('  Returns a ZenList of matching elements.')
            print()
            return

        try:
            lexer = Lexer(expr)
            parser = Parser(lexer)
            node = parser.parse()
            if node.statements:
                if self.interpreter:
                    val = self.interpreter.interpret(node)
                    eval_ok = True
        except Exception:
            pass

        print(f'\033[1;33m{expr}\033[0m')
        print()

        if doc:
            print(f'  \033[1;36mBuiltin: {doc}\033[0m')
            print()

        if eval_ok and val is not _VOID and val is not None:
            t = type(val).__name__
            if isinstance(val, ZenMethod):
                t = 'method'
            print(f'  Type: \033[1;36m{t}\033[0m')
            print(f'  Value: \033[1;32m{_format_result(val)}\033[0m')

            if isinstance(val, str):
                print()
                print('  String methods:')
                for m in ['upper()', 'lower()', 'split(sep)', 'join(list)',
                          'replace(old, new)', 'strip()',
                          'startswith(s)', 'endswith(s)', 'len']:
                    print(f'    .{m}')

            elif isinstance(val, (list, tuple)):
                print()
                print('  List methods:')
                for m in ['append(x)', 'pop()', 'sort()', 'reverse()',
                          'clear()', 'len']:
                    print(f'    .{m}')

            elif isinstance(val, dict):
                print()
                print('  Dict methods:')
                for m in ['keys()', 'values()', 'items()', 'get(key)',
                          'len']:
                    print(f'    .{m}')

            elif isinstance(val, ZenMethod) and val._name in _ZEN_BUILTIN_HELP:
                pass

            elif hasattr(val, '__dict__') or hasattr(type(val), '__dict__'):
                print()
                print('  Attributes & methods:')
                for name in sorted(dir(val)):
                    if name.startswith('_'):
                        continue
                    attr = getattr(type(val), name, None)
                    if attr is None:
                        continue
                    if callable(attr):
                        print(f'    .{name}()')
                    else:
                        print(f'    .{name}')

        elif eval_ok and val is None:
            print('  \033[1;33mnull\033[0m — The null value')
        else:
            print('  \033[1;33m(no value or could not evaluate)\033[0m')

        print()
        if eval_ok and val is not _VOID and val is not None:
            print('  Examples:')
            if isinstance(val, str):
                print(f'    {expr}.upper()')
                print(f'    {expr}.len')
                print(f'    {expr}.split(",")')
            elif isinstance(val, (int, float)):
                print(f'    {expr} + 1')
                print(f'    let x = {expr}')
        elif not doc and not eval_ok:
            lower = expr.lower()
            matches = {k: v for k, v in _ZEN_BUILTIN_HELP.items() if lower in k.lower() or lower in v.lower()}
            if matches:
                print(f'  \033[1;36mMatching builtins for "{expr}":\033[0m')
                print()
                for k in sorted(matches):
                    print(f'  \033[33m{k}\033[0m — {matches[k]}')
            else:
                print('  \033[1;33m(no matching help topic)\033[0m')
            print()
            return
        elif doc and not eval_ok:
            print('  Examples:')
            print(f'    {doc}')
        elif eval_ok and val is None:
            print('  Use \033[1;36mnull\033[0m for the null value')

    def _is_complete(self, code):
        stripped = code.strip()
        if not stripped:
            return True
        try:
            lexer = Lexer(code)
            parser = Parser(lexer)
            parser.parse()
            return True
        except (LexerError, ParseError) as e:
            msg = str(e)
            if 'Expected RBRACE' in msg or 'Expected RPAREN' in msg or 'Expected RBRACKET' in msg:
                return False
            if 'EOF' in msg and ('LBRACE' in msg or 'LPAREN' in msg or 'LBRACKET' in msg):
                return False
            return True
        except Exception:
            return True

    def _execute(self, code):
        try:
            lexer = Lexer(code)
            parser = Parser(lexer)
            program = parser.parse()
            result = self.interpreter.interpret(program)
            if result is not _VOID:
                formatted = _format_result(result)
                if formatted:
                    print(formatted)
        except LexerError as e:
            print(f'\033[1;31mLexer Error: {e.message}\033[0m')
            _show_error_context(code, e.line, e.col, e.message)
        except ParseError as e:
            msg = e.message
            tok = e.token if hasattr(e, 'token') else None
            if tok and tok.type in ('ELSE', 'WHILE', 'FOR', 'IF', 'ELIF'):
                import re
                for kw in ['if', 'while', 'for', 'else', 'function', 'return',
                           'try', 'catch', 'print', 'break', 'continue']:
                    p = re.compile(re.escape(kw) + r'[a-zA-Z_]')
                    for m in p.finditer(code):
                        start = max(0, m.start() - 4)
                        ctx = code[start:m.end() + 4]
                        msg += "\n  \033[1;33mHint: '" + ctx.strip() + "' — keywords need spaces (e.g., '" + kw + " ...' not '" + m.group() + "')\033[0m"
                        break
                    if '\n' in msg:
                        break
            print(f'\033[1;31mParse Error: {msg}\033[0m')
            if tok:
                _show_error_context(code, tok.line, tok.col, msg)
        except ZenError as e:
            msg = e.message
            if msg == 'Undefined variable: help':
                msg = "Did you mean '.help'? Shell commands start with '.'"
            elif msg == 'Undefined variable: none':
                msg = "Undefined variable: none. Did you mean 'null'?"
            print(f'\033[1;31mRuntime Error: {msg}\033[0m')
            if e.node and hasattr(e.node, 'line'):
                _show_error_context(code, e.node.line, e.node.col, msg)
        except Exception as e:
            err_type = type(e).__name__
            err_msg = str(e).split('\n')[0]
            print(f'\033[1;31m{err_type}: {err_msg}\033[0m')

    def stop(self):
        self.browser.stop()
        if _HAS_READLINE:
            try:
                readline.write_history_file(_ZEN_HISTFILE)
            except (FileNotFoundError, OSError):
                pass
