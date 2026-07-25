import sys
import os
import traceback
import argparse

from .lexer import Lexer, LexerError
from .parser import Parser, ParseError
from .interpreter import Interpreter, ZenError
from .browser import Browser, set_config
from .shell import Shell
from .utils import read_file, resolve_path
from .color import color
from .environment import ZenBrowserError, format_error


def _add_browser_args(sp):
    sp.add_argument('--headful', action='store_true', help='Show browser window')
    sp.add_argument('--no-headless', action='store_true', help='Show browser window')
    sp.add_argument('--browser-path', default=None,
                    help='Path to Chromium browser executable (e.g. /usr/bin/brave)')
    sp.add_argument('--connect', nargs='?', const=9222, type=int, default=None,
                    help='Connect to running browser on port (default 9222)')
    sp.add_argument('--http', action='store_true', help='HTTP-only mode (no browser, uses requests)')


def main():
    argv = sys.argv[1:]
    if argv and not argv[0].startswith('-') and argv[0] not in ('shell', 'run', 'open', 'shot', 'scrape'):
        if os.path.isfile(argv[0]) or argv[0].endswith('.z'):
            argv.insert(0, 'run')
    elif argv and '-e' in argv:
        ei = argv.index('-e')
        if ei == 0 or argv[ei - 1] not in ('shell', 'run', 'open', 'shot', 'scrape'):
            argv.insert(0, 'run')
    sys.argv = [sys.argv[0]] + argv

    parser = argparse.ArgumentParser(
        prog='zen',
        description='Zen - Browser Automation Language',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  zen run script.z              Run a Zen script
  zen shell                     Start interactive shell
  zen open https://example.com  Open a URL and print title
  zen shot https://example.com  Take a screenshot
  zen script.z --connect        Run script attached to your browser
  zen script.z --http           Run script with HTTP only (no browser)
        """)

    parser.add_argument('--version', action='store_true', help='Show version')
    parser.add_argument('--debug', action='store_true', help='Show full debug tracebacks')

    sub = parser.add_subparsers(dest='command', help='Command')

    shell_parser = sub.add_parser('shell', help='Start interactive shell')
    _add_browser_args(shell_parser)

    run_parser = sub.add_parser('run', help='Run a .z script file')
    run_parser.add_argument('file', nargs='?', help='Path to .z script')
    run_parser.add_argument('-e', '--eval', action='append', dest='eval_stmts',
                            help='Inline Zen code (can be used multiple times)')
    _add_browser_args(run_parser)

    open_parser = sub.add_parser('open', help='Open a URL')
    open_parser.add_argument('url', help='URL to open')
    open_parser.add_argument('--html', action='store_true', help='Print rendered HTML')
    _add_browser_args(open_parser)

    shot_parser = sub.add_parser('shot', help='Take a screenshot')
    shot_parser.add_argument('url', help='URL to screenshot')
    shot_parser.add_argument('-o', '--output', default='screenshot.png', help='Output file')
    _add_browser_args(shot_parser)

    scrape_parser = sub.add_parser('scrape', help='Scrape text from URL')
    scrape_parser.add_argument('url', help='URL to scrape')
    scrape_parser.add_argument('-s', '--selector', required=True, help='CSS selector')
    _add_browser_args(scrape_parser)

    script_extra = []
    args, unknown = parser.parse_known_args()

    if args.debug:
        os.environ['ZEN_DEBUG'] = '1'

    headless = not (getattr(args, 'headful', False) or getattr(args, 'no_headless', False))
    set_config('headless', headless)

    if getattr(args, 'browser_path', None):
        set_config('browser_path', args.browser_path)

    if getattr(args, 'command', None) == 'run':
        script_extra = unknown
    elif not args.command and unknown:
        first = unknown[0]
        if os.path.isfile(first) or first.endswith('.z'):
            args.command = 'run'
            args.file = first
            script_extra = unknown[1:]
        elif not first.startswith('-'):
            args.command = 'run'
            args.file = first
            script_extra = unknown[1:]

    if args.version:
        from . import __version__
        print(f"zen {__version__}")
        return

    browser_mode = 'browser'
    if getattr(args, 'http', False):
        browser_mode = 'http'
    elif getattr(args, 'connect', None) is not None:
        browser_mode = 'connect'

    src_path = getattr(args, 'file', None)
    src_lines = None
    eval_stmts = getattr(args, 'eval_stmts', None)
    if eval_stmts:
        src_path = '<eval>'
    elif src_path and os.path.exists(src_path):
        try:
            src_lines = read_file(src_path).splitlines(True)
        except Exception:
            pass

    try:
        _dispatch(args, unknown, script_extra, headless, browser_mode)
    except ZenBrowserError as e:
        print(color.red(e.message))
        if args.debug or 'ZEN_DEBUG' in os.environ:
            traceback.print_exc()
        sys.exit(1)
    except (LexerError, ParseError) as e:
        if src_lines:
            print(format_error(src_path, e, src_lines))
        else:
            print(color.red(f"Error: {e}"))
        sys.exit(1)
    except ZenError as e:
        if src_lines:
            print(format_error(src_path, e, src_lines))
        else:
            print(color.red(f"Error: {e.message}"))
        sys.exit(1)
    except KeyboardInterrupt:
        pass
    except Exception as e:
        print(color.red(f"Unexpected error: {e}"))
        if args.debug or 'ZEN_DEBUG' in os.environ:
            traceback.print_exc()
        sys.exit(1)


def _dispatch(args, unknown, script_extra, headless, browser_mode):
    if args.command == 'shell' or args.command is None:
        shell = Shell(headless=headless, browser_path=getattr(args, 'browser_path', None),
                      connect_port=getattr(args, 'connect', None), mode=browser_mode)
        try:
            shell.start()
        finally:
            shell.stop()
        return

    if args.command == 'run':
        eval_stmts = getattr(args, 'eval_stmts', None)
        if eval_stmts:
            code = '\n'.join(eval_stmts)
            src_path = '<eval>'
            src_lines = code.splitlines(True)
        else:
            path = resolve_path(args.file)
            if not os.path.exists(path):
                print(color.red(f"File not found: {path}"))
                sys.exit(1)
            code = read_file(path)
            src_path = path
        browser = Browser(
            headless=headless,
            browser_path=getattr(args, 'browser_path', None),
            connect_port=getattr(args, 'connect', None),
            mode=browser_mode,
        )
        try:
            interpreter = Interpreter(browser, script_args=script_extra)
            lexer = Lexer(code)
            parser = Parser(lexer)
            program = parser.parse()
            interpreter.interpret(program)
        finally:
            browser.stop()
        return

    if args.command == 'open':
        browser = Browser(
            headless=headless,
            browser_path=getattr(args, 'browser_path', None),
            connect_port=getattr(args, 'connect', None),
            mode=browser_mode,
        )
        try:
            browser.go(args.url)
            print(f"Title: {browser.title()}")
            print(f"URL: {browser.url()}")
            if args.html:
                print("---")
                print(browser.page_html())
        finally:
            browser.stop()
        return

    if args.command == 'shot':
        browser = Browser(
            headless=headless,
            browser_path=getattr(args, 'browser_path', None),
            connect_port=getattr(args, 'connect', None),
            mode=browser_mode,
        )
        try:
            browser.go(args.url)
            browser.shot(args.output)
            print(color.green(f"Screenshot saved: {args.output}"))
        finally:
            browser.stop()
        return

    if args.command == 'scrape':
        browser = Browser(
            headless=headless,
            browser_path=getattr(args, 'browser_path', None),
            connect_port=getattr(args, 'connect', None),
            mode=browser_mode,
        )
        try:
            browser.go(args.url)
            texts = browser.texts(args.selector)
            for t in texts:
                print(t)
        finally:
            browser.stop()
        return


if __name__ == '__main__':
    main()
