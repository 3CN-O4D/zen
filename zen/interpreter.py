import os, time as _time_mod
from . import nodes as ast
from .environment import Environment, ZenReturn, ZenBreak, ZenContinue, ZenError, ZenElement, ZenList, ZenMethod, ConfigModule, ZenClass, ZenInstance
from .builtins import register_builtins, _parse_duration
from .color import color

_VOID = object()

_SPECIAL_VARS = {
    '_url', '__url', '___url',
    '_time', '_date', '_dir',
    '_version', '_', '_timeout',
    '_page_html', '_page_text',
    '_page_links', '_page_images',
    '_page_urls', '_page_forms',
    '_page_inputs', '_page_buttons',
}

class Interpreter:
    def __init__(self, browser, script_args=None):
        self.browser = browser
        self.global_env = Environment()
        register_builtins(self.global_env, browser)
        for name in list(self.global_env.vars):
            self.global_env.lock(name)
        self.current_env = self.global_env
        self._last_result = None
        self._load_default_libs()
        if script_args is not None:
            self.global_env.define('args', list(script_args))
            self.global_env.lock('args')

    def _load_default_libs(self):
        lib_dir = os.path.join(os.path.dirname(__file__), 'lib')
        if not os.path.isdir(lib_dir):
            return
        auto = {'str.z', 'dict.z', 'browser.z'}
        from .lexer import Lexer
        from .parser import Parser
        for fname in sorted(os.listdir(lib_dir)):
            if fname not in auto:
                continue
            path = os.path.join(lib_dir, fname)
            try:
                with open(path) as f:
                    code = f.read()
                lexer = Lexer(code)
                parser = Parser(lexer)
                program = parser.parse()
                old_env = self.current_env
                self.current_env = self.current_env.child()
                try:
                    self.interpret(program)
                    mod = {}
                    for k, v in self.current_env.vars.items():
                        if not k.startswith('_') or k in ('_', '_version'):
                            mod[k] = v
                            if not getattr(old_env, 'is_locked', lambda x: False)(k):
                                old_env.define(k, v)
                finally:
                    self.current_env = old_env
                mod_name = fname.replace('.z', '')
                if not getattr(old_env, 'is_locked', lambda x: False)(mod_name):
                    old_env.define(mod_name, mod)
            except Exception as e:
                raise ZenError(f"Failed to load lib/{fname}: {e}")

    def _resolve_special(self, name):
        if name == '_url':
            return self.browser.current_url
        if name == '__url':
            u = self.browser.previous_url
            return u if u else ''
        if name == '___url':
            u = self.browser.older_url
            return u if u else ''
        if name == '_time':
            return _time_mod.strftime('%H:%M:%S')
        if name == '_date':
            return _time_mod.strftime('%Y-%m-%d')
        if name == '_dir':
            return os.getcwd()
        if name == '_version':
            return '0.1.0'
        if name == '_':
            return self._last_result
        if name == '_timeout':
            return self.browser.timeout_ms
        if name == '_page_html':
            return self.browser.page_html()
        if name == '_page_text':
            return self.browser.page_text_markers()
        if name == '_page_links':
            return self.browser.page_links()
        if name == '_page_images':
            return self.browser.page_images()
        if name == '_page_urls':
            return list(self.browser.url_history)
        if name == '_page_forms':
            return self.browser.page_forms()
        if name == '_page_inputs':
            return self.browser.page_inputs()
        if name == '_page_buttons':
            return self.browser.page_buttons()
        raise ZenError(f"Unknown special variable: {name}")

    def interpret(self, node):
        result = self._eval(node)
        if result is not _VOID:
            self._last_result = result
        return result

    def _call_func(self, func_node, instance, *args):
        old_env = self.current_env
        self.current_env = self.current_env.child()
        self.current_env.define('self', instance)
        arg_idx = 0
        for param in func_node.params:
            if param == 'self':
                continue
            if arg_idx < len(args):
                self.current_env.define(param, args[arg_idx])
                arg_idx += 1
            elif param in func_node.defaults:
                self.current_env.define(param, self._eval(func_node.defaults[param]))
            else:
                raise ZenError(f"Missing argument: {param}")
        try:
            try:
                self.current_env.lock('self')
            except Exception:
                pass
            try:
                result = self._eval(func_node.body)
                return result
            except ZenReturn as ret:
                return ret.value
        finally:
            self.current_env = old_env

    def _eval(self, node):
        try:
            return self._eval_inner(node)
        except ZenError as e:
            if e.node is None:
                e.node = node
            raise
        except ZenReturn:
            raise
        except ZenBreak:
            raise
        except ZenContinue:
            raise
        except Exception as e:
            raise ZenError(str(e), node)

    def _assign_single(self, target, value):
        if isinstance(target, ast.Variable):
            if target.name == '_':
                return value
            if target.name == '_timeout':
                self.browser.timeout_ms = _parse_duration(value)
                return value
            if self.current_env.has(target.name):
                self.current_env.set(target.name, value)
            else:
                self.current_env.define(target.name, value)
        elif isinstance(target, ast.Member):
            obj = self._eval(target.obj)
            if isinstance(obj, (ZenElement, ConfigModule, ZenInstance)):
                setattr(obj, target.name, value)
            else:
                raise ZenError(f"Cannot set property on {type(obj).__name__}")
        elif isinstance(target, ast.Index):
            obj = self._eval(target.obj)
            index = self._eval(target.index)
            obj[index] = value
        return value

    def _eval_inner(self, node):
        if isinstance(node, ast.Program):
            result = _VOID
            for stmt in node.statements:
                result = self._eval(stmt)
            return result

        elif isinstance(node, ast.Block):
            old_env = self.current_env
            self.current_env = self.current_env.child()
            try:
                result = _VOID
                for stmt in node.statements:
                    result = self._eval(stmt)
                return result
            finally:
                self.current_env = old_env

        elif isinstance(node, ast.Let):
            value = self._eval(node.value)
            self.current_env.define(node.name, value)
            return value

        elif isinstance(node, ast.Assign):
            value = self._eval(node.value)
            if isinstance(node.target, ast.ListLiteral):
                targets = node.target.elements
                if isinstance(value, (list, tuple)):
                    if len(targets) != len(value):
                        raise ZenError(
                            f"Cannot unpack {len(value)} values into {len(targets)} targets")
                    for t, v in zip(targets, value):
                        self._assign_single(t, v)
                    return value
                if hasattr(value, '__iter__'):
                    vals = list(value)
                    if len(targets) != len(vals):
                        raise ZenError(
                            f"Cannot unpack {len(vals)} values into {len(targets)} targets")
                    for t, v in zip(targets, vals):
                        self._assign_single(t, v)
                    return value
                raise ZenError(f"Cannot unpack {type(value).__name__}")
            return self._assign_single(node.target, value)

        elif isinstance(node, ast.Go):
            try:
                url = str(self._eval(node.url))
            except ZenError as e:
                msg = e.message
                if 'Undefined variable' in msg:
                    msg += "\n  " + color.yellow("Hint: URLs need quotes. Try: go \"https://example.com\"")
                raise ZenError(msg)
            self.browser.go(url)
            return _VOID

        elif isinstance(node, ast.Fill):
            selector = self._eval(node.selector)
            value = str(self._eval(node.value))
            self.browser.fill(selector, value)
            return _VOID

        elif isinstance(node, ast.Click):
            if node.target is None:
                return _VOID
            target = self._eval(node.target)
            self.browser.click(target)
            return _VOID

        elif isinstance(node, ast.Wait):
            dur = self._eval(node.duration)
            if isinstance(dur, (int, float)) and dur <= 60:
                dur = str(int(dur)) + 's'
            ms = _parse_duration(dur)
            self.browser.wait(ms)
            return _VOID

        elif isinstance(node, ast.WaitFor):
            selector = self._eval(node.selector)
            self.browser.wait_for(selector)
            return _VOID

        elif isinstance(node, ast.Refresh):
            self.browser.refresh()
            return _VOID

        elif isinstance(node, ast.Back):
            self.browser.back()
            return _VOID

        elif isinstance(node, ast.Forward):
            self.browser.forward()
            return _VOID

        elif isinstance(node, ast.Shot):
            path = str(self._eval(node.path))
            self.browser.shot(path, full=node.full)
            return _VOID

        elif isinstance(node, ast.Scroll):
            self.browser.scroll(direction=node.direction, x=node.x, y=node.y)
            return _VOID

        elif isinstance(node, ast.Execute):
            code = str(self._eval(node.code))
            return self.browser.execute(code)

        elif isinstance(node, ast.Download):
            url = str(self._eval(node.url))
            path = str(self._eval(node.path))
            self.browser.download(url, path)
            return _VOID

        elif isinstance(node, ast.Print):
            parts = []
            for v in node.values:
                val = self._eval(v)
                if isinstance(val, list):
                    for item in val:
                        parts.append(str(_format_value(item)))
                else:
                    parts.append(str(_format_value(val)))
            if parts:
                print(' '.join(parts), flush=True)
            else:
                print(flush=True)
            return _VOID

        elif isinstance(node, ast.Input):
            prompt = str(self._eval(node.prompt))
            value = input(prompt)
            self.current_env.define(node.target, value)
            return value

        elif isinstance(node, ast.If):
            cond = self._eval(node.condition)
            if _is_truthy(cond):
                return self._eval(node.then_branch)
            elif node.else_branch:
                return self._eval(node.else_branch)
            return _VOID

        elif isinstance(node, ast.Switch):
            val = self._eval(node.expr)
            matched = False
            for case_val_node, body in node.cases:
                case_val = self._eval(case_val_node)
                if case_val == val:
                    matched = True
                    return self._eval(body)
            if not matched and node.default_body:
                return self._eval(node.default_body)
            return _VOID

        elif isinstance(node, ast.With):
            val = self._eval(node.expr)
            old_env = self.current_env
            self.current_env = self.current_env.child()
            try:
                if node.name:
                    self.current_env.define(node.name, val)
                return self._eval(node.body)
            finally:
                self.current_env = old_env

        elif isinstance(node, ast.For):
            iterable = self._eval(node.iterable)
            result = _VOID
            if hasattr(iterable, '__iter__'):
                for item in iterable:
                    old_env = self.current_env
                    self.current_env = self.current_env.child()
                    try:
                        self.current_env.define(node.var_name, item)
                        result = self._eval(node.body)
                    except ZenBreak:
                        break
                    except ZenContinue:
                        continue
                    finally:
                        self.current_env = old_env
            return result

        elif isinstance(node, ast.While):
            result = _VOID
            while _is_truthy(self._eval(node.condition)):
                old_env = self.current_env
                self.current_env = self.current_env.child()
                try:
                    result = self._eval(node.body)
                except ZenBreak:
                    break
                except ZenContinue:
                    continue
                finally:
                    self.current_env = old_env
            return result

        elif isinstance(node, ast.Function):
            closure_env = self.current_env
            def fn(*args, **kwargs):
                old_env = self.current_env
                self.current_env = closure_env.child()
                try:
                    if len(args) > len(node.params):
                        raise ZenError(
                            f"Function takes {len(node.params)} positional argument(s)"
                            f" but {len(args)} were given")
                    for i, param in enumerate(node.params):
                        if i < len(args):
                            self.current_env.define(param, args[i])
                        elif param in kwargs:
                            self.current_env.define(param, kwargs.pop(param))
                        elif param in node.defaults:
                            old = self.current_env
                            self.current_env = closure_env
                            try:
                                self.current_env.define(param, self._eval(node.defaults[param]))
                            finally:
                                self.current_env = old
                        else:
                            raise ZenError(f"Missing required argument: {param}")
                    if kwargs:
                        raise ZenError(f"Unexpected keyword argument: {list(kwargs.keys())[0]}")
                    try:
                        result = self._eval(node.body)
                        return result
                    except ZenReturn as ret:
                        return ret.value
                finally:
                    self.current_env = old_env
            if node.name is not None:
                self.current_env.define(node.name, fn)
            return fn

        elif isinstance(node, ast.Return):
            if node.value is not None:
                raise ZenReturn(self._eval(node.value))
            raise ZenReturn(None)

        elif isinstance(node, ast.Break):
            raise ZenBreak()

        elif isinstance(node, ast.Continue):
            raise ZenContinue()

        elif isinstance(node, ast.TryCatch):
            old_env = self.current_env
            self.current_env = self.current_env.child()
            result = None
            try:
                try:
                    result = self._eval(node.try_body)
                except ZenBreak:
                    raise
                except ZenContinue:
                    raise
                except Exception as e:
                    err_name = node.err_var or 'error'
                    self.current_env.define(err_name, str(e))
                    result = self._eval(node.catch_body)
            finally:
                if node.finally_body:
                    self._eval(node.finally_body)
                self.current_env = old_env
            return result

        elif isinstance(node, ast.BinaryOp):
            left = self._eval(node.left)

            if node.op == 'or':
                if _is_truthy(left):
                    return left
                right = self._eval(node.right)
                return right
            elif node.op == 'and':
                if not _is_truthy(left):
                    return left
                right = self._eval(node.right)
                return right

            right = self._eval(node.right) if hasattr(node, 'right') else None
            if node.op == 'PLUS':
                if isinstance(left, str) or isinstance(right, str):
                    return str(left) + str(right)
                return left + right
            if node.op == 'MINUS':
                return left - right
            if node.op == 'STAR':
                return left * right
            if node.op == 'SLASH':
                return left / right
            if node.op == 'MOD':
                return left % right
            if node.op == 'POW':
                return left ** right
            if node.op == 'IS':
                return left is right
            if node.op in ('EQ', '=='):
                return left == right
            if node.op in ('NEQ', '!='):
                return left != right
            if node.op == 'LT':
                return left < right
            if node.op == 'GT':
                return left > right
            if node.op == 'LE':
                return left <= right
            if node.op == 'GE':
                return left >= right
            if node.op == 'IN':
                if isinstance(right, (list, tuple)):
                    return left in right
                if isinstance(right, str):
                    return str(left) in right
                if isinstance(right, dict):
                    return left in right
                if hasattr(right, '__contains__'):
                    return left in right
                return False
            raise ZenError(f"Unknown operator: {node.op}")

        elif isinstance(node, ast.UnaryOp):
            operand = self._eval(node.operand)
            if node.op == '-':
                return -operand
            elif node.op == '!':
                return not _is_truthy(operand)
            elif node.op == 'not':
                return not _is_truthy(operand)
            raise ZenError(f"Unknown unary operator: {node.op}")

        elif isinstance(node, ast.Call):
            callee = self._eval(node.callee)
            args = [self._eval(arg) for arg in node.args]
            kwargs = {k: self._eval(v) for k, v in node.kwargs}
            if callable(callee):
                try:
                    result = callee(*args, **kwargs)
                except TypeError:
                    if kwargs:
                        raise ZenError(f"Function does not accept keyword arguments")
                    result = callee(*args)
                if isinstance(result, (dict_keys, dict_values, dict_items)):
                    return list(result)
                return result
            if isinstance(node.callee, ast.Member):
                return callee
            raise ZenError(f"Not callable: {callee}")

        elif isinstance(node, ast.SafeMember):
            obj = self._eval(node.obj)
            if obj is None:
                return None
            member = ast.Member(node.obj, node.name)
            member.line = getattr(node, 'line', 0)
            member.col = getattr(node, 'col', 0)
            return self._eval_inner(member)

        elif isinstance(node, ast.Member):
            obj = self._eval(node.obj)

            if isinstance(obj, ZenMethod):
                obj = obj()

            # --- universal (available on all types) ---
            if node.name == 'str':
                return ZenMethod('str', lambda: str(obj))
            if node.name == 'int':
                return ZenMethod('int', lambda: int(obj))
            if node.name == 'float':
                return ZenMethod('float', lambda: float(obj))
            if node.name == 'bool':
                return ZenMethod('bool', lambda: bool(obj))
            if node.name == 'type':
                return type(obj).__name__

            # --- ZenInstance ---
            if isinstance(obj, ZenInstance):
                try:
                    return getattr(obj, node.name)
                except AttributeError:
                    raise ZenError(f"Instance has no attribute '{node.name}'", node)

            # --- ZenElement ---
            if isinstance(obj, ZenElement):
                if node.name in ('text', 'html', 'exists', 'tag',
                                 'duration', 'paused', 'ended',
                                 'muted', 'loop', 'volume',
                                 'current_time', 'currentTime',
                                 'is_visible', 'isVisible',
                                 'is_enabled', 'isEnabled',
                                 'is_checked', 'isChecked',
                                 'url', 'src'):
                    name = {
                        'currentTime': 'current_time', 'isVisible': 'is_visible',
                        'isEnabled': 'is_enabled', 'isChecked': 'is_checked',
                    }.get(node.name, node.name)
                    return getattr(obj, name)
                if node.name in ('attr', 'click', 'fill',
                                 'check', 'uncheck', 'select',
                                 'find', 'find_all', 'findAll',
                                 'play', 'pause', 'download',
                                 'screenshot', 'hover'):
                    name = {
                        'findAll': 'find_all',
                    }.get(node.name, node.name)
                    return ZenMethod(node.name, getattr(obj, name))
                raise ZenError(f"ZenElement has no attribute '{node.name}'", node)

            # --- ZenList ---
            if isinstance(obj, ZenList):
                if node.name in ('first', 'texts', 'htmls', 'count', 'tags', 'len'):
                    return getattr(obj, node.name)
                if node.name in ('to_list', 'toList'):
                    return list(obj)
                if node.name in ('nth', 'attr', 'attrs', 'each', 'sorted'):
                    return ZenMethod(node.name, getattr(obj, node.name))
                raise ZenError(f"ZenList has no attribute '{node.name}'", node)

            # --- dict ---
            if isinstance(obj, dict):
                if node.name in ('len', 'count'):
                    return len(obj)
                if node.name in ('is_empty', 'isEmpty'):
                    return len(obj) == 0
                if node.name in obj:
                    val = obj.get(node.name)
                    if callable(val) and hasattr(val, '__code__') and val.__code__.co_argcount == 0 and not isinstance(val, type):
                        try:
                            return val()
                        except Exception:
                            return val
                    return val
                if node.name in ('keys', 'values'):
                    return ZenMethod(node.name, lambda: list(getattr(obj, node.name)()))
                if node.name == 'items':
                    return ZenMethod('items', lambda: [list(kv) for kv in obj.items()])
                if node.name == 'get':
                    return ZenMethod('get', lambda key, default=None: obj.get(key, default))
                if node.name == 'has':
                    return ZenMethod('has', lambda key: key in obj)
                if node.name == 'put':
                    return ZenMethod('put', lambda key, val: obj.update({key: val}) or obj)
                if hasattr(type(obj), node.name):
                    attr = getattr(type(obj), node.name)
                    if isinstance(attr, property):
                        return attr.__get__(obj, type(obj))
                    if callable(attr):
                        return ZenMethod(node.name, attr.__get__(obj, type(obj)))
                return None

            # --- str ---
            if isinstance(obj, str):
                if node.name in ('len', 'count'):
                    return len(obj)
                if node.name in ('is_empty', 'isEmpty'):
                    return len(obj) == 0
                if node.name in ('upper', 'to_upper', 'toUpper'):
                    name = node.name
                    return ZenMethod(name, lambda: obj.upper())
                if node.name in ('upper', 'to_upper', 'toUpper'):
                    return ZenMethod(node.name, lambda: obj.upper())
                if node.name in ('lower', 'to_lower', 'toLower'):
                    return ZenMethod(node.name, lambda: obj.lower())
                if node.name == 'capitalize':
                    return ZenMethod('capitalize', lambda: obj.capitalize())
                if node.name in ('title_case', 'titleCase'):
                    return ZenMethod(node.name, lambda: obj.title())
                if node.name in ('trim', 'strip'):
                    return ZenMethod(node.name, lambda: obj.strip())
                if node.name in ('trim_left', 'trimLeft'):
                    return ZenMethod(node.name, lambda: obj.lstrip())
                if node.name in ('trim_right', 'trimRight'):
                    return ZenMethod(node.name, lambda: obj.rstrip())
                if node.name == 'contains':
                    return ZenMethod('contains', lambda sub: sub in obj)
                if node.name in ('starts_with', 'startsWith'):
                    return ZenMethod(node.name, lambda prefix: obj.startswith(prefix))
                if node.name in ('ends_with', 'endsWith'):
                    return ZenMethod(node.name, lambda suffix: obj.endswith(suffix))
                if node.name == 'split':
                    return ZenMethod('split', lambda sep=None: obj.split(sep) if sep else obj.split())
                if node.name == 'replace':
                    return ZenMethod('replace', lambda old, new: obj.replace(old, new))
                if node.name in ('replace_all', 'replaceAll'):
                    return ZenMethod(node.name, lambda old, new: obj.replace(old, new))
                if node.name == 'repeat':
                    return ZenMethod('repeat', lambda n: obj * int(n))
                if node.name in ('substring', 'slice'):
                    return ZenMethod(node.name,
                        lambda start, end=None: obj[int(start):int(end)] if end is not None else obj[int(start):])
                if node.name in ('char_at', 'charAt'):
                    return ZenMethod(node.name, lambda i: obj[int(i)])
                if node.name in ('to_list', 'toList'):
                    return ZenMethod(node.name, lambda: list(obj))
                if node.name in ('index_of', 'indexOf'):
                    return ZenMethod(node.name, lambda sub, start=0: obj.index(sub, int(start)) if sub in obj[int(start):] else -1)
                if node.name in ('last_index_of', 'lastIndexOf'):
                    return ZenMethod(node.name, lambda sub: obj.rindex(sub) if sub in obj else -1)
                if node.name == 'matches':
                    return ZenMethod('matches', lambda pattern: bool(__import__('re').search(pattern, obj)))
                if node.name in ('pad_left', 'padLeft'):
                    return ZenMethod(node.name, lambda width, char=' ': obj.rjust(int(width), str(char)))
                if node.name in ('pad_right', 'padRight'):
                    return ZenMethod(node.name, lambda width, char=' ': obj.ljust(int(width), str(char)))
                if node.name == 'format':
                    return ZenMethod('format', lambda *fargs, **fkwargs: obj.format(*fargs, **fkwargs))
                if node.name == 'lines':
                    return ZenMethod('lines', lambda: obj.splitlines())
                if node.name == 'join':
                    return ZenMethod('join', lambda iterable: obj.join(str(x) for x in iterable))
                raise ZenError(f"String has no attribute '{node.name}'", node)

            # --- list ---
            if isinstance(obj, list):
                if node.name in ('len', 'count'):
                    return len(obj)
                if node.name in ('is_empty', 'isEmpty'):
                    return len(obj) == 0
                if node.name == 'first':
                    return obj[0] if obj else None
                if node.name == 'last':
                    return obj[-1] if obj else None
                if node.name == 'sum':
                    return sum(obj)
                if node.name == 'min':
                    return min(obj)
                if node.name == 'max':
                    return max(obj)
                if node.name == 'contains':
                    return ZenMethod('contains', lambda val: val in obj)
                if node.name == 'map':
                    return ZenMethod('map', lambda fn: [fn(x) for x in obj])
                if node.name == 'filter':
                    return ZenMethod('filter', lambda fn: [x for x in obj if fn(x)])
                if node.name == 'reduce':
                    return ZenMethod('reduce',
                        lambda fn, initial=None: __import__('functools').reduce(fn, obj, initial) if initial is not None else __import__('functools').reduce(fn, obj))
                if node.name == 'each':
                    return ZenMethod('each', lambda fn: [fn(x) for x in obj])
                if node.name == 'sort':
                    return ZenMethod('sort', lambda key=None, reverse=False: sorted(obj, key=key) if key else sorted(obj, reverse=reverse))
                if node.name == 'reverse':
                    return ZenMethod('reverse', lambda: list(reversed(obj)))
                if node.name == 'shuffle':
                    return ZenMethod('shuffle', lambda: __import__('random').sample(obj, len(obj)))
                if node.name == 'unique':
                    return ZenMethod('unique', lambda: list(dict.fromkeys(obj)))
                if node.name == 'flatten':
                    return ZenMethod('flatten', lambda: [item for sublist in obj for item in (sublist if isinstance(sublist, list) else [sublist])])
                if node.name in ('group_by', 'groupBy'):
                    return ZenMethod(node.name, lambda fn: __import__('collections').defaultdict(list, {k: list(g) for k, g in __import__('itertools').groupby(sorted(obj, key=fn), fn)}))
                if node.name == 'join':
                    return ZenMethod('join', lambda sep='': sep.join(str(x) for x in obj))
                if node.name == 'append':
                    return ZenMethod('append', lambda x: obj.append(x))
                if node.name == 'pop':
                    return ZenMethod('pop', lambda: obj.pop())
                if node.name == 'push':
                    return ZenMethod('push', lambda x: obj.append(x))
                if node.name == 'shift':
                    return ZenMethod('shift', lambda: obj.pop(0))
                if node.name == 'unshift':
                    return ZenMethod('unshift', lambda x: obj.insert(0, x))
                if node.name == 'includes':
                    return ZenMethod('includes', lambda val: val in obj)
                if node.name == 'indexOf':
                    return ZenMethod('indexOf', lambda val: obj.index(val) if val in obj else -1)
                raise ZenError(f"List has no attribute '{node.name}'", node)

            # --- int / float (number methods) ---
            if isinstance(obj, (int, float)):
                if node.name == 'times':
                    return ZenMethod('times', lambda fn: [fn(i) for i in range(int(obj))])
                raise ZenError(f"Number has no attribute '{node.name}'", node)

            # --- generic object (fallback) ---
            if obj is None:
                raise ZenError(f"null has no attribute '{node.name}'", node)
            if node.name == 'len' and hasattr(obj, '__len__'):
                return len(obj)
            if hasattr(obj, node.name):
                attr = getattr(obj, node.name)
                if callable(attr):
                    return ZenMethod(node.name, attr)
                return attr
            raise ZenError(f"Type {type(obj).__name__} has no attribute '{node.name}'", node)

        elif isinstance(node, ast.Slice):
            obj = self._eval(node.obj)
            start = self._eval(node.start) if node.start is not None else None
            end = self._eval(node.end) if node.end is not None else None
            step = self._eval(node.step) if node.step is not None else None
            if isinstance(obj, (str, list)):
                return obj[slice(start, end, step)]
            if isinstance(obj, dict):
                keys = list(obj.keys())[slice(start, end, step)]
                return {k: obj[k] for k in keys}
            raise ZenError(f"Type {type(obj).__name__} does not support slicing", node)

        elif isinstance(node, ast.Index):
            obj = self._eval(node.obj)
            index = self._eval(node.index)
            if isinstance(obj, list):
                return obj[index]
            if isinstance(obj, dict):
                return obj.get(index)
            if isinstance(obj, str):
                return obj[index]
            if isinstance(obj, ZenList):
                return obj.nth(int(index))
            return obj[int(index)]

        elif isinstance(node, ast.Include):
            path = str(self._eval(node.path))
            if not path.endswith('.z'):
                path = path + '.z'
            sep_path = path.replace('.', '/')
            lib_dir = os.path.join(os.path.dirname(__file__), 'lib')
            candidates = [
                os.path.join(lib_dir, sep_path),
                os.path.join(lib_dir, path),
                path,
                sep_path,
            ]
            resolved = None
            for c in candidates:
                if os.path.isfile(c):
                    resolved = c
                    break
            if resolved is None:
                libs = sorted(f for f in os.listdir(lib_dir) if f.endswith('.z'))
                hint = ''
                clean = path.rstrip('.z') + '.z'
                if clean in libs:
                    hint = f' (did you mean include "{clean}"?)'
                else:
                    hint = f' (available: {", ".join(libs)})'
                raise ZenError(f"Include file not found: {path}{hint}")
            with open(resolved) as f:
                code = f.read()
            from .lexer import Lexer
            from .parser import Parser
            lexer = Lexer(code)
            parser = Parser(lexer)
            program = parser.parse()
            old_env = self.current_env
            self.current_env = self.current_env.child()
            try:
                result = self.interpret(program)
                module = {}
                for k, v in self.current_env.vars.items():
                    if not k.startswith('_') or k in ('_', '_version'):
                        module[k] = v
                return module
            finally:
                self.current_env = old_env

        elif isinstance(node, ast.Ternary):
            cond = self._eval(node.cond)
            return self._eval(node.then_val) if _is_truthy(cond) else self._eval(node.else_val)

        elif isinstance(node, ast.InterpolatedString):
            parts = []
            for is_expr, val in node.parts:
                if is_expr:
                    parts.append(str(self.current_env.get(val)))
                else:
                    parts.append(val)
            return ''.join(parts)

        elif isinstance(node, ast.Literal):
            if node.value is None:
                return None
            return node.value

        elif isinstance(node, ast.Variable):
            if node.name in _SPECIAL_VARS:
                return self._resolve_special(node.name)
            return self.current_env.get(node.name)

        elif isinstance(node, ast.ListLiteral):
            result = []
            for e in node.elements:
                if isinstance(e, ast.Spread):
                    val = self._eval(e.expr)
                    if isinstance(val, list):
                        result.extend(val)
                    else:
                        result.append(val)
                else:
                    result.append(self._eval(e))
            return result

        elif isinstance(node, ast.Range):
            start = int(self._eval(node.start))
            end = int(self._eval(node.end))
            step = int(self._eval(node.step)) if node.step is not None else 1
            end_adj = end + 1 if step > 0 else end - 1
            return list(range(start, end_adj, step))

        elif isinstance(node, ast.DictLiteral):
            result = {}
            for k, v in node.pairs:
                if isinstance(k, ast.Spread):
                    val = self._eval(k.expr)
                    if isinstance(val, dict):
                        result.update(val)
                    else:
                        raise ZenError(f"Cannot spread non-dict in dict literal")
                else:
                    result[str(self._eval(k))] = self._eval(v)
            return result

        elif isinstance(node, ast.Class):
            methods = {}
            for name, val_node in node.body.items():
                if isinstance(val_node, ast.Function):
                    methods[name] = val_node
                elif isinstance(val_node, ast.Literal):
                    methods[name] = val_node.value
                else:
                    methods[name] = self._eval(val_node)
            parent_val = None
            if node.parent is not None:
                parent_val = self._eval(node.parent)
                if not isinstance(parent_val, (ZenClass, dict)):
                    raise ZenError("Parent class must be a class", node)
            klass = ZenClass(node.name, methods, parent_val, self)
            self.current_env.define(node.name, klass)
            return klass

        elif isinstance(node, ast.New):
            klass = self._eval(node.class_expr)
            if not isinstance(klass, (ZenClass, type)):
                raise ZenError(f"Called 'new' on non-class: {klass}", node)
            args = [self._eval(a) for a in node.args]
            return klass(*args)

        else:
            raise ZenError(f"Unknown AST node: {type(node).__name__}")


def _is_truthy(val):
    if val is None:
        return False
    if isinstance(val, bool):
        return val
    if isinstance(val, (int, float)):
        return val != 0
    if isinstance(val, str):
        return len(val) > 0
    if isinstance(val, (list, dict)):
        return len(val) > 0
    return True


def _format_value(val):
    if isinstance(val, str):
        return val
    if isinstance(val, (int, float, bool)):
        return str(val)
    if val is None:
        return 'null'
    if isinstance(val, (list, tuple)):
        return '[' + ', '.join(_format_value(v) for v in val) + ']'
    if isinstance(val, dict):
        return '{' + ', '.join(f'{k}: {_format_value(v)}' for k, v in val.items()) + '}'
    if isinstance(val, (dict_keys, dict_values, dict_items)):
        return _format_value(list(val))
    if isinstance(val, ZenList):
        return f'<{val.count} elements>'
    return str(val)

dict_keys = type({}.keys())
dict_values = type({}.values())
dict_items = type({}.items())
