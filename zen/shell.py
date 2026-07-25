import sys
import os
import warnings
import asyncio
from .lexer import Lexer, LexerError
from .parser import Parser, ParseError
from .interpreter import Interpreter, ZenError, _VOID
from .environment import ZenElement, ZenList, ZenMethod, ZenRegexMatch
from .browser import Browser
from .color import color
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
    'if', 'elif', 'else', 'while', 'function', 'def', 'return', 'print', 'input',
    'into', 'scroll', 'to', 'by', 'shot', 'full', 'refresh',
    'back', 'forward', 'execute', 'download', 'and', 'or', 'not',
    'true', 'false', 'null', 'try', 'catch', 'top', 'bottom',
    'break', 'continue', 'include',
    'switch', 'case', 'default', 'with', 'as',
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
    'switch': 'switch expr { case val { } default { } } — Multi-branch value match',
    'case': 'case val { } — Branch within a switch statement',
    'default': 'default { } — Fallback branch in switch',
    'with': 'with expr as name { } — Temporary block scope',
    'elif': 'elif cond { } — Else-if chaining (alternative: else if)',
    'class': 'class Name { } or class { } — Define or create class as expression',
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
    print(color.red(f'  Error: {message}'))

def _show_error_context_for_token(code, token, message):
    if token:
        _show_error_context(code, token.line, token.col, message)
    else:
        print(color.red(f'  Error: {message}'))


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

        c = color
        if self._buf:
            yellow_dots = c.yellow('...') if c.enabled else '...'
            return f'{yellow_dots} '
        no_browser = ''
        if self.browser and not self.browser.has_browser:
            no_browser = c.yellow(' (no browser)')
        if page_count > 0 and title:
            parts = f'{c.bright_cyan("zen")}{no_browser}{c.dim(f"[{page_count}]")}{c.dim(title)} {c.bright_cyan("❯")} '
        elif page_count > 0:
            parts = f'{c.bright_cyan("zen")}{no_browser}{c.dim(f"[{page_count}]")} {c.bright_cyan("❯")} '
        else:
            parts = f'{c.bright_cyan("zen")}{no_browser} {c.bright_cyan("❯")} '
        if for_readline and 'TERMUX_VERSION' not in os.environ:
            return f'\001{c.reset()}\002{parts}\001{c.reset()}\002'
        return parts

    def start(self):
        c = color
        print(c.bright_cyan(f'Zen v{__version__} — Browser Automation Shell'))
        if not HAS_PT:
            print(f'{c.yellow("Tip:")} install {c.bright_cyan("prompt-toolkit")} for auto-completion, syntax highlighting, and more ({c.yellow("pip install prompt-toolkit")})')
        print(f'Type {c.yellow(".help")} for commands, {c.yellow(".exit")} to quit')

        self.browser.start()
        if not self.browser.has_browser and self.browser._no_browser:
            print(c.yellow(f'\n! {self.browser._no_browser.split(chr(10))[0]}'))
            print(c.yellow('! Browser-dependent commands (go, click, fill, ...) will not work.'))
        print()

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
        c = color
        Y = c.yellow
        C = c.bright_cyan
        R = c.reset
        print(C('Zen — Browser Automation Language'))
        print()
        print(C('Data Types'))
        print(f'  {Y("Number")}  42, 3.14, -7')
        print(f'  {Y("String")}  "hello", \'world\'')
        print(f'  {Y("Boolean")} true, false')
        print(f'  {Y("Null")}    null')
        print(f'  {Y("List")}    [1, 2, 3]')
        print(f'  {Y("Dict")}    {{"a": 1}}')
        print()
        print(C('Operators'))
        print(f'  {Y("Arithmetic:")}     +  -  *  /  %  **')
        print(f'  {Y("Compound:")}       +=  -=  *=  /=  %=')
        print(f'  {Y("Incr/Decr:")}      x++  x--')
        print(f'  {Y("Comparison:")}     ==  !=  <  >  <=  >=')
        print(f'  {Y("Chained:")}        a < b < c')
        print(f'  {Y("Identity:")}       is  is not')
        print(f'  {Y("Membership:")}     in  not in')
        print(f'  {Y("Logical:")}        and  or  not')
        print(f'  {Y("Ternary:")}        val if cond else val')
        print(f'  {Y("Range:")}          x -> y  (also x .. y, x to y)')
        print(f'  {Y("Range step:")}     x -> y by n  (also x -> y @ n)')
        print(f'  {Y("Spread:")}         [...xs]  {{...ds}}  (unpack into list/dict)')
        print(f'  {Y("Safe nav:")}       obj?.prop  (null if obj is null)')
        print()
        print(C('Control Flow'))
        print(f'  {Y("if")} cond {{ }} {Y("elif")} cond {{ }} {Y("else")} {{ }}')
        print(f'  {Y("switch")} expr {{ {Y("case")} val {{ }} {Y("default")} {{ }} }}')
        print(f'  {Y("while")} cond {{ }}')
        print(f'  {Y("for")} x in iterable {{ }}')
        print(f'  {Y("function")} name(params) {{ }}')
        print(f'  {Y("with")} expr {Y("as")} name {{ }}  — temporary block scope')
        print(f'  {Y("return")} value')
        print(f'  {Y("break")} — exit current loop')
        print(f'  {Y("continue")} — skip to next iteration')
        print(f'  {Y("try")} {{ }} {Y("catch")} err {{ }}   (optional error var)')
        print(f'  {Y("try")} {{ }} {Y("catch")} {{ }} {Y("finally")} {{ }}')
        print(f'  {Y("include \"file.z\"")}  — Include and run another Zen file')
        print()
        print(C('Selectors'))
        print(f'  {Y("CSS:")}       find("div.class"), find_all("a[href]")')
        print(f'  {Y("Text:")}      find_by_text("Click Here")   (visible text)')
        print(f'  {Y("URL:")}       find_by_url("example.com")  (link href)')
        print(f'  {Y("Regex:")}     click "/submit|save/i"')
        print(f'  {Y("Text sel:")}  click by_text("Login")')
        print()
        print(C('Modules'))
        print(f'  {Y("re.matches(pattern, str)")}  → bool  — Full match test')
        print(f'  {Y("re.search(pattern, str)")}    → ZenRegexMatch — First match with .group(), .groups(), .start, .end, .match')
        print(f'  {Y("re.findall(pattern, str)")}   → list  — All non-overlapping matches')
        print(f'  {Y("re.split(pattern, str)")}     → list  — Split by regex')
        print(f'  {Y("re.sub(pattern, repl, str)")} → str   — Replace matches')
        print()
        print(f'  {c.bold(Y("http module:"))}')
        print(f'  {Y("http.get(url)")}         → Response  — HTTP GET')
        print(f'  {Y("http.post(url, data)")}   → Response  — HTTP POST')
        print(f'  {Y("http.put(url, data)")}    → Response  — HTTP PUT')
        print(f'  {Y("http.del(url)")}          → Response  — HTTP DELETE')
        print(f'  {Y("http.head(url)")}         → Response  — HTTP HEAD')
        print(f'  {Y("http.patch(url, data)")}  → Response  — HTTP PATCH')
        print('  Response methods: .status, .body, .headers, .json(), .ok')
        print()
        print(f'  {c.bold(Y("cookies module:"))}')
        print(f'  {Y("cookies.all()")}    → list  — All cookies as [{{name,value}}]')
        print(f'  {Y("cookies.get(name)")} → str  — Get cookie value')
        print(f'  {Y("cookies.set(n,v,path)")} → bool — Set cookie')
        print(f'  {Y("cookies.clear()")}  → bool  — Clear all cookies')
        print()
        print(f'  {c.bold(Y("storage module:"))}')
        print(f'  {Y("storage.get(key)")}  → str   — localStorage.getItem')
        print(f'  {Y("storage.set(k,v)")}  → bool  — localStorage.setItem')
        print(f'  {Y("storage.remove(k)")} → bool  — localStorage.removeItem')
        print(f'  {Y("storage.clear()")}   → bool  — localStorage.clear')
        print(f'  {Y("storage.all()")}     → list  — All items as [{{key,value}}]')
        print()
        print(f'  {Y("net.online()")}   → bool — Check if browser is online')
        print(f'  {Y("net.cookies()")}  → str  — Get document.cookie')
        print(f'  {Y("net.url()")}      → str  — Get current URL')
        print(f'  {Y("csv.read(\"path\")")}        → list  — Read CSV file')
        print(f'  {Y("csv.write(\"path\", rows)")} → bool  — Write CSV file')
        print(f'  {Y("csv.parse(\"text\")")}       → list  — Parse CSV string')
        print(f'  {Y("csv.encode(rows)")}        → str   — Encode to CSV string')
        print('  Flat aliases: csv_read, csv_write, csv_parse, csv_encode')
        print()
        print(f'  {c.bold(Y("fs module (filesystem):"))}')
        print(f'  {Y("fs.list(path)")}      → list  — List directory')
        print(f'  {Y("fs.read(path)")}      → str   — Read text file')
        print(f'  {Y("fs.write(path, c)")}  → bool  — Write text file')
        print(f'  {Y("fs.exists(path)")}    → bool  — Check existence')
        print(f'  {Y("fs.is_file(path)")}   → bool  — Is regular file?')
        print(f'  {Y("fs.is_dir(path)")}    → bool  — Is directory?')
        print(f'  {Y("fs.size(path)")}      → int   — File size in bytes')
        print(f'  {Y("fs.mtime(path)")}     → float — Last modified time')
        print(f'  {Y("fs.mkdir(path)")}     → bool  — Create directory')
        print(f'  {Y("fs.remove(path)")}    → bool  — Remove file')
        print(f'  {Y("fs.rmdir(path)")}     → bool  — Remove empty dir')
        print(f'  {Y("fs.rmtree(path)")}    → bool  — Remove dir recursively')
        print(f'  {Y("fs.copy(src, dst)")}  → bool  — Copy file')
        print(f'  {Y("fs.move(src, dst)")}  → bool  — Move/rename file')
        print(f'  {Y("fs.glob(pattern)")}   → list  — Glob file matching')
        print(f'  {Y("fs.join(a, b...)")}   → str   — Join path parts')
        print(f'  {Y("fs.cwd()")}           → str   — Current directory')
        print(f'  {Y("fs.cd(path)")}        → bool  — Change directory')
        print(f'  {Y("fs.exec(cmd)")}       → dict  — Run shell command ({{returncode, stdout, stderr}})')
        print(f'  {Y("fs.basename(path)")}  → str   — Last path component')
        print(f'  {Y("fs.dirname(path)")}   → str   — Parent directory')
        print()
        print(f'  {c.bold(Y("page module:"))}')
        print(f'  {Y("page.html")}      → str   — Full page HTML')
        print(f'  {Y("page.text")}      → str   — Visible page text')
        print(f'  {Y("page.links")}     → list  — All link URLs')
        print(f'  {Y("page.images")}    → list  — All image URLs')
        print(f'  {Y("page.forms")}     → list  — All forms with inputs')
        print(f'  {Y("page.inputs")}    → list  — All input/select/textarea fields')
        print(f'  {Y("page.buttons")}   → list  — All buttons and clickable elements')
        print(f'  {Y("page.title")}     → str   — Page title')
        print(f'  {Y("page.url")}       → str   — Current URL')
        print(f'  {Y("page.source")}    → str   — Alias for page.html')
        print()
        print(f'  {c.bold(Y("User agent & headers:"))}')
        print(f'  {Y("user_agent()")}       → str   — Get browser user-agent')
        print(f'  {Y("set_user_agent(\"...\")")} — Override navigator.userAgent')
        print(f'  {Y("set_headers({...})")}  — Set extra HTTP headers for all requests')
        print(f'  {Y("headers()")}          → dict  — Currently set extra headers')
        print()
        print(f'  {c.bold(Y("Sequence & math builtins:"))}')
        print(f'  {Y("range(end)")}         → list  — 0..end-1')
        print(f'  {Y("range(start, end)")}   → list  — start..end-1')
        print(f'  {Y("range(start, end, step)")} → list')
        print(f'  {Y("interval(start, end)")} → list  — alias for range')
        print(f'  {Y("interval(start, end, step)")} → list')
        print(f'  {Y("abs(v)")}              → n     — Absolute value')
        print(f'  {Y("min(a, b...)")}        → n     — Minimum')
        print(f'  {Y("max(a, b...)")}        → n     — Maximum')
        print(f'  {Y("round(v, n?)")}        → n     — Round')
        print(f'  {Y("assert(cond, msg?)")}  — Raise error if falsy')
        print(f'  {Y("assert_eq(a, b, msg?)")} — Raise error if a != b')
        print()
        print(f'  {c.bold(Y("Also flat builtins:"))}')
        print(f'  {Y("read_file")}, {Y("write_file")}, {Y("append_file")}, {Y("file_exists")}, {Y("list_dir")},')
        print(f'  {Y("mkdir")}, {Y("remove_file")}, {Y("copy_file")}, {Y("move_file")}, {Y("rename_file")},')
        print(f'  {Y("path_join")}, {Y("cwd")}, {Y("cd")}, {Y("glob")}, {Y("exec")}, {Y("sh")}, {Y("system")}')
        print()
        print(C('Classes'))
        print(f'  {Y("class Name { }")}          — define a class')
        print(f'  {Y("class Name extends P { }")} — inheritance')
        print(f'  {Y("let cls = class { }")}      — class as expression')
        print(f'  {Y("new ClassName(args)")}      — instantiate')
        print(f'  {Y("self")}                     — instance reference in methods')
        print()
        print(C('String Features'))
        print(f'  {Y("Interpolation:")} "hello {name}" — embed variable')
        print(f'  {Y("Escapes:")} \\n \\t \\r \\\\ \\" \\\' \\0')
        print(f'  {Y("Hex/Unicode:")} \\xNN \\uNNNN \\UNNNNNNNN')
        print(f'  {Y("Triple quotes:")} """multi\\nline""" ')
        print()
        print(C('Browser Builtins'))
        for k in sorted(_ZEN_BUILTIN_HELP):
            v = _ZEN_BUILTIN_HELP[k]
            print(f'  {Y(k)}  {v}')
        print()
        print(C('Special Variables'))
        print(f'  {Y("_url")}         Current page URL')
        print(f'  {Y("__url")}        Previous page URL')
        print(f'  {Y("___url")}       Page before that')
        print(f'  {Y("_time")}        Current time (HH:MM:SS)')
        print(f'  {Y("_date")}        Current date (YYYY-MM-DD)')
        print(f'  {Y("_dir")}         Working directory')
        print(f'  {Y("_version")}     Zen version')
        print(f'  {Y("_")}            Last expression result')
        print(f'  {Y("_timeout")}     Default timeout (ms, "3s", "1.5m")')
        print(f'  {Y("_page_html")}   Raw page HTML')
        print(f'  {Y("_page_text")}   Visible text with {{[media]}} markers')
        print(f'  {Y("_page_links")}  All link URLs on page')
        print(f'  {Y("_page_images")} All image URLs on page')
        print(f'  {Y("_page_urls")}   All visited URLs this session')
        print(f'  {Y("_page_forms")}   All forms on page')
        print(f'  {Y("_page_inputs")}  All input/select/textarea fields')
        print(f'  {Y("_page_buttons")} All buttons and clickable elements')
        print()
        print(C('Variables & Assignment'))
        print(f'  {Y("let x = val")}      — declare variable')
        print(f'  {Y("x = val")}          — assign/reassign')
        print(f'  {Y("x += val")}         — compound assignment')
        print(f'  {Y("x++")}              — increment')
        print(f'  {Y("a, b = 1, 2")}      — tuple unpacking')
        print(f'  {Y("a, _ = [1, 2]")}    — _ throwaway value')
        print()
        print(C('Element Methods (ZenElement)'))
        print(f'  {Y(".text")}            {Y(".html")}            {Y(".exists")}            {Y(".tag")}')
        print(f'  {Y(".attr(\"href\")")}    {Y(".click()")}         {Y(".fill(\"val\")")}')
        print(f'  {Y(".check()")}         {Y(".uncheck()")}       {Y(".select(\"opt\")")}    {Y(".hover()")}')
        print(f'  {Y(".screenshot(\"path\")")}')
        print(f'  {Y(".find(\"sel\")")}     {Y(".find_all(\"sel\")")}')
        print(f'  {Y(".play()")}          {Y(".pause()")}         {Y(".download(\"path\")")}')
        print(f'  {Y(".muted")}           {Y(".volume")}          {Y(".current_time")}')
        print(f'  {Y(".duration")}        {Y(".paused")}          {Y(".ended")}           {Y(".loop")}')
        print()
        print(C('List Methods (ZenList)'))
        print(f'  {Y(".texts")}  {Y(".htmls")}  {Y(".tags")}  {Y(".count")}  {Y(".len")}  {Y(".first")}')
        print(f'  {Y(".nth(n)")}  {Y(".attr(n)")}  {Y(".attrs(n)")}  {Y(".each(fn)")}  {Y(".sorted()")}')
        print()
        print(C('String / List / Dict / Number Methods'))
        print(f'  {Y("String:")}  .upper()  .lower()  .split()  .join()  .replace()  .strip()')
        print('               .startswith()  .endswith()  .find()  .len  .count  .format()')
        print(f'  {Y("List:")}    .append()  .pop()  .sort()  .reverse()  .clear()  .len  .count  .sorted()')
        print('               .push()  .shift()  .unshift()  .includes()  .indexOf()  .join()')
        print(f'  {Y("Dict:")}    .keys()  .values()  .items()  .get()  .put()  .len  .count  .clear()')
        print(f'  {Y("Number:")}  .times(fn)  .str()  .float()  .type')
        print(f'  {Y("Any:")}     .str()  .int()  .float()  .bool()  .type')
        print(f'  {Y("Interp:")}  "hello {{name}}" — embed variable in string')
        print()
        print(C('Slice Syntax'))
        print(f'  {Y("list[start:end]")}  {Y("list[start:]")}  {Y("list[:end]")}  {Y("list[::step]")}')
        print()
        print(C('Shell Commands'))
        print(f'  {Y(".exit")} / {Y(".quit")}   Exit the shell')
        print(f'  {Y(".help")} [expr]    Show this help or expression details')
        print(f'  {Y(".clear")}          Clear screen')
        print(f'  {Y(".url")}            Show current URL')
        print(f'  {Y(".title")}           Show page title')
        print(f'  {Y(".vars")}           Show user variables')
        print(f'  {Y(".history")}         Show URL navigation history')
        print(f'  {Y(".run")} <file>      Run a .z script file')
        print(f'  {Y(".shot")} <file>     Take page screenshot')
        print(f'  {Y(".type")} <expr>     Show type of expression')
        print(f'  {Y(".dir")}             Show current directory')
        print()
        print(f'For detailed help on a specific function or expression, type {Y(".help <expr>")}')
        print(f'  e.g., {Y(".help find")}, {Y(".help find_all")}, {Y(".help _url")}, {Y(".help \"hello\"")}')

    def _show_help(self, expr):
        c = color
        Y = c.yellow
        C = c.bright_cyan
        G = c.green
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
            print(Y(expr))
            print()
            print(f'  {C(f"Shell Command: {dot_cmds[expr]}")}')
            print()
            return

        if expr == 'keywords':
            print(Y('All help topics'))
            print()
            for k in sorted(_ZEN_BUILTIN_HELP):
                print(f'  {Y(k)}')
            for k in sorted(dot_cmds):
                print(f'  {Y(f".{k}")}')
            for k in ['include', 'csv', 're', 'net', 'http', 'cookies', 'storage', 'search']:
                print(f'  {Y(k)}')
            print()
            return

        if expr == 'include':
            print(Y('include'))
            print()
            print(f'  {C("include \"file.z\"")} — Include and run another Zen file')
            print()
            print('  All functions, variables, and assignments in the included file')
            print('  become available in the current scope.')
            print()
            return

        if expr == 'csv':
            print(Y('csv'))
            print()
            print(f'  {C("Module for CSV processing")}')
            print(f'  {Y("csv.read(\"path\")")}       → list — Read CSV file')
            print(f'  {Y("csv.write(\"path\", rows)")}  → bool — Write CSV file')
            print(f'  {Y("csv.parse(\"text\")")}      → list — Parse CSV string')
            print(f'  {Y("csv.encode(rows)")}       → str — Encode to CSV string')
            print('  Flat aliases: csv_read, csv_write, csv_parse, csv_encode')
            print()
            return

        if expr == 're':
            print(Y('re'))
            print()
            print(f'  {C("Module for regular expressions")}')
            print(f'  {Y("re.matches(pattern, str)")}  → bool — Test if whole string matches')
            print(f'  {Y("re.search(pattern, str)")}    → ZenRegexMatch — First match')
            print(f'  {Y("re.findall(pattern, str)")}   → list — All matches')
            print(f'  {Y("re.split(pattern, str)")}     → list — Split by pattern')
            print(f'  {Y("re.sub(pattern, repl, str)")} → str — Replace matches')
            print()
            print(f'  {C("ZenRegexMatch methods:")}')
            print(f'    {Y(".match")}   — Full matched text (str)')
            print(f'    {Y(".start")}   — Start position (int)')
            print(f'    {Y(".end")}     — End position (int)')
            print(f'    {Y(".group(n)")} — Capture group n (n=0 for full match)')
            print(f'    {Y(".groups()")} — List of all capture groups')
            print()
            return

        if expr == 'net':
            print(Y('net'))
            print()
            print(f'  {C("Module for browser network info")}')
            print(f'  {Y("net.online()")}   → bool — Is browser online?')
            print(f'  {Y("net.cookies()")}  → str — Get document.cookie')
            print(f'  {Y("net.url()")}      → str — Get current URL')
            print()
            return

        if expr == 'search':
            print(Y('search'))
            print()
            print(f'  {C("Find elements by flexible query")}')
            print(f'  {Y("search(\"text\")")}       → Find by visible text (e.g., search("Login"))')
            print(f'  {Y("search(\"div.class\")")}  → Find by CSS selector')
            print(f'  {Y("search(\"/pattern/\")")}  → Find by regex text match')
            print(f'  {Y("search(\"text=...\")")}   → Find by exact text')
            print(f'  {Y("search(\"url=...\")")}    → Find by link URL')
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

        print(Y(expr))
        print()

        if doc:
            print(f'  {C(f"Builtin: {doc}")}')
            print()

        if eval_ok and val is not _VOID and val is not None:
            t = type(val).__name__
            if isinstance(val, ZenMethod):
                t = 'method'
            print(f'  Type: {C(t)}')
            print(f'  Value: {G(_format_result(val))}')

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
            print(f'  {Y("null")} — The null value')
        else:
            print(f'  {Y("(no value or could not evaluate)")}')

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
                print(f'  {C(f"Matching builtins for \"{expr}\":")}')
                print()
                for k in sorted(matches):
                    print(f'  {Y(k)} — {matches[k]}')
            else:
                print(f'  {Y("(no matching help topic)")}')
            print()
            return
        elif doc and not eval_ok:
            print('  Examples:')
            print(f'    {doc}')
        elif eval_ok and val is None:
            print(f'  Use {C("null")} for the null value')

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
            print(color.red(f'Lexer Error: {e.message}'))
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
                        msg += "\n  " + color.yellow("Hint: '" + ctx.strip() + "' — keywords need spaces (e.g., '" + kw + " ...' not '" + m.group() + "')")
                        break
                    if '\n' in msg:
                        break
            print(color.red(f'Parse Error: {msg}'))
            if tok:
                _show_error_context(code, tok.line, tok.col, msg)
        except ZenError as e:
            msg = e.message
            if msg == 'Undefined variable: help':
                msg = "Did you mean '.help'? Shell commands start with '.'"
            elif msg == 'Undefined variable: none':
                msg = "Undefined variable: none. Did you mean 'null'?"
            print(color.red(f'Runtime Error: {msg}'))
            if e.node and hasattr(e.node, 'line'):
                _show_error_context(code, e.node.line, e.node.col, msg)
        except Exception as e:
            err_type = type(e).__name__
            err_msg = str(e).split('\n')[0]
            print(color.red(f'{err_type}: {err_msg}'))

    def stop(self):
        self.browser.stop()
        if _HAS_READLINE:
            try:
                readline.write_history_file(_ZEN_HISTFILE)
            except (FileNotFoundError, OSError):
                pass
