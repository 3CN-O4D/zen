import re
from . import nodes as zen_ast
from .environment import ZenError

class CompileError(Exception):
    pass

_BINOP = {
    'PLUS': '+', 'MINUS': '-', 'STAR': '*', 'SLASH': '/',
    'MOD': '%', 'POW': '**',
}
_CMPOP = {
    'EQ': '==', 'NEQ': '!=', 'LT': '<', 'GT': '>',
    'LE': '<=', 'GE': '>=', 'IS': 'is', 'IS_NOT': 'is not',
    'IN': 'in', 'NOT_IN': 'not in',
}

def _op_str(op):
    if op in _BINOP:
        return _BINOP[op]
    if op in _CMPOP:
        return _CMPOP[op]
    return None

def _indent(code, level=1):
    indent = '    ' * level
    return '\n'.join(indent + line if line.strip() else line for line in code.split('\n'))

def _is_simple_expr(node):
    t = type(node).__name__
    if t in ('Literal', 'Variable', 'ListLiteral', 'DictLiteral'):
        return True
    if t == 'BinaryOp':
        return _is_simple_expr(node.left) and _is_simple_expr(node.right)
    if t == 'UnaryOp':
        return _is_simple_expr(node.operand)
    if t == 'Range':
        return _is_simple_expr(node.start) and _is_simple_expr(node.end)
    if t == 'Ternary':
        return _is_simple_expr(node.cond) and _is_simple_expr(node.then_val) and _is_simple_expr(node.else_val)
    return False

def _is_compileable(node):
    """Check if a node (and its children) can be compiled to plain Python."""
    t = type(node).__name__
    hard = {'Class', 'New', 'Include', 'TryCatch', 'Function', 'InterpolatedString',
            'SafeMember', 'With', 'Switch', 'Member', 'Call', 'Spread'}
    if t in hard:
        # Some hard nodes we can handle via __env__ fallback
        return True
    if t == 'Program':
        return all(_is_compileable(s) for s in node.statements)
    if t == 'Block':
        return all(_is_compileable(s) for s in node.statements)
    if t == 'If':
        return _is_compileable(node.condition) and _is_compileable(node.then_branch) and \
               (node.else_branch is None or _is_compileable(node.else_branch))
    if t == 'For':
        return _is_compileable(node.iterable) and _is_compileable(node.body)
    if t == 'While':
        return _is_compileable(node.condition) and _is_compileable(node.body)
    return True

class LocalVar:
    def __init__(self, name, is_initialized=False):
        self.name = name
        self.is_initialized = is_initialized

class Scope:
    def __init__(self, parent=None, is_func=False):
        self.parent = parent
        self.vars = {}  # name -> LocalVar
        self.is_func = is_func
    
    def define(self, name):
        self.vars[name] = LocalVar(name, True)
    
    def is_local(self, name):
        if name in self.vars:
            return True
        if self.parent and not self.parent.is_func:
            return self.parent.is_local(name)
        return False
    
    def is_func_scope(self):
        if self.is_func:
            return True
        if self.parent:
            return self.parent.is_func_scope()
        return False

class ZenCompiler:
    def __init__(self):
        self._cache = {}

    def compile(self, node, name='<zen>'):
        cache_key = id(node)
        cached = self._cache.get(cache_key)
        if cached is not None:
            return cached
        scope = Scope()
        source = self._gen(node, scope)
        if source is None:
            raise CompileError("Cannot compile node")
        py_code = compile(source, name, 'exec')
        result = (py_code, source)
        self._cache[cache_key] = result
        return result

    def _var(self, name, scope):
        if scope.is_local(name) or name in ('true', 'false', 'null', '_'):
            return name
        return f"__env__['{name}']"

    def _gen(self, node, scope):
        t = type(node).__name__

        if t == 'Program':
            parts = []
            for stmt in node.statements:
                code = self._gen(stmt, scope)
                if code is None:
                    return None
                parts.append(code)
            if parts:
                st = node.statements[-1]
                st_t = type(st).__name__
                if st_t in ('Variable', 'Literal', 'BinaryOp', 'UnaryOp', 'Ternary',
                            'Call', 'Index', 'Member', 'ListLiteral', 'DictLiteral',
                            'Range', 'InterpolatedString', 'SafeMember', 'Slice'):
                    parts[-1] = f"__result__ = {parts[-1]}"
            return '\n'.join(parts)

        if t == 'Block':
            child = Scope(scope)
            parts = []
            for stmt in node.statements:
                code = self._gen(stmt, child)
                if code is None:
                    return None
                parts.append(code)
            if not parts:
                return ''
            return '\n'.join(parts)

        if t == 'Let':
            val = self._expr(node.value, scope)
            if val is None:
                return None
            scope.define(node.name)
            return f"{node.name} = {val}"

        if t == 'Assign':
            return self._gen_assign(node, scope)

        if t == 'Variable':
            name = node.name
            if name == 'true':
                return 'True'
            if name == 'false':
                return 'False'
            if name == 'null':
                return 'None'
            if name == '_':
                return "__env__._last_result"
            if name.startswith('_') and name in ('_url', '_time', '_date', '_dir', '_version',
                                                  '_timeout', '__url', '___url'):
                return f"__env__.__special__('{name}')"
            if scope.is_local(name):
                return name
            return f"__env__['{name}']"

        if t == 'Literal':
            v = node.value
            if isinstance(v, str):
                return repr(v)
            if isinstance(v, bool):
                return 'True' if v else 'False'
            if v is None:
                return 'None'
            return repr(v)

        if t == 'BinaryOp':
            left = self._expr(node.left, scope)
            right = self._expr(node.right, scope)
            if left is None or right is None:
                return None
            op = _op_str(node.op)
            if op:
                if node.op in ('AND', 'OR'):
                    return f"({left} {op} {right})"
                return f"({left} {op} {right})"
            return f"__env__.__binop__('{node.op}', {left}, {right})"

        if t == 'UnaryOp':
            op = node.op
            operand = self._expr(node.operand, scope)
            if operand is None:
                return None
            if op == '-':
                return f"(-{operand})"
            if op in ('not', '!'):
                return f"(not {operand})"
            return f"__env__.__unaryop__('{op}', {operand})"

        if t == 'Range':
            start = self._expr(node.start, scope)
            end = self._expr(node.end, scope)
            if start is None or end is None:
                return None
            step = self._expr(node.step, scope) if node.step is not None else None
            if step:
                return f"list(range({start}, {end} + 1, {step}))"
            # Auto-detect direction: range(start, end+1, 1) or range(start, end-1, -1)
            return f"list(range({start}, ({end} + 1) if {start} <= {end} else ({end} - 1), 1 if {start} <= {end} else -1))"

        if t == 'ListLiteral':
            elts = []
            for e in node.elements:
                c = self._expr(e, scope)
                if c is None:
                    return None
                elts.append(c)
            return f"[{', '.join(elts)}]"

        if t == 'DictLiteral':
            pairs = []
            for k, v in node.pairs:
                kc = self._expr(k, scope)
                vc = self._expr(v, scope)
                if kc is None or vc is None:
                    return None
                pairs.append(f"{kc}: {vc}")
            return f"{{{', '.join(pairs)}}}"

        if t == 'Call':
            return self._gen_call(node, scope)

        if t == 'Member':
            return self._gen_member(node, scope)

        if t == 'Index':
            obj = self._expr(node.obj, scope)
            index = self._expr(node.index, scope)
            if obj is None or index is None:
                return None
            return f"{obj}[{index}]"

        if t == 'Ternary':
            cond = self._expr(node.cond, scope)
            then_val = self._expr(node.then_val, scope)
            else_val = self._expr(node.else_val, scope)
            if cond is None or then_val is None or else_val is None:
                return None
            return f"({then_val} if {cond} else {else_val})"

        if t == 'InterpolatedString':
            parts = []
            for part in node.parts:
                if isinstance(part, str):
                    parts.append(repr(part))
                else:
                    c = self._expr(part, scope)
                    if c is None:
                        return None
                    parts.append(f"str({c})")
            if len(parts) <= 1:
                return parts[0] if parts else "''"
            return f"({'+'.join(parts)})"

        if t == 'Print':
            values = ', '.join(self._expr(v, scope) for v in node.values)
            return f"__env__.__print__({values})"

        if t == 'If':
            return self._gen_if(node, scope)

        if t == 'For':
            return self._gen_for(node, scope)

        if t == 'While':
            return self._gen_while(node, scope)

        if t == 'Function':
            return self._gen_func(node, scope)

        if t == 'Return':
            if node.value:
                val = self._expr(node.value, scope)
                if val is None:
                    return None
                return f"raise __Return({val})"
            return "raise __Return(None)"

        if t == 'Break':
            return "break"
        if t == 'Continue':
            return "continue"

        if t == 'Switch':
            return self._gen_switch(node, scope)

        if t == 'With':
            return self._gen_with(node, scope)

        if t == 'Go':
            url = self._expr(node.url, scope)
            return f"__env__.__browser__.go({url})" if url else None

        if t == 'Click':
            if node.target:
                target = self._expr(node.target, scope)
                return f"__env__.__browser__.click({target})"
            return ""

        if t == 'Fill':
            sel = self._expr(node.selector, scope)
            val = self._expr(node.value, scope)
            return f"__env__.__browser__.fill({sel}, {val})"

        if t == 'Wait':
            dur = self._expr(node.duration, scope)
            return f"__env__.__wait__({dur})"

        if t == 'Shot':
            path = self._expr(node.path, scope)
            return f"__env__.__browser__.shot({path}, full={'True' if node.full else 'False'})"

        if t == 'Scroll':
            args = []
            if node.direction:
                d = self._expr(node.direction, scope)
                args.append(f"direction={d}")
            if node.x:
                x = self._expr(node.x, scope)
                args.append(f"x={x}")
            if node.y:
                y = self._expr(node.y, scope)
                args.append(f"y={y}")
            return f"__env__.__browser__.scroll({', '.join(args)})"

        if t == 'Execute':
            code = self._expr(node.code, scope)
            return f"__env__.__browser__.execute({code})"

        if t == 'Download':
            url = self._expr(node.url, scope)
            path = self._expr(node.path, scope)
            return f"__env__.__browser__.download({url}, {path})"

        if t == 'Input':
            prompt = self._expr(node.prompt, scope) if node.prompt else "''"
            target = self._expr(node.target, scope) if node.target else 'None'
            return f"__env__.__input__({prompt}, {target})"

        if t == 'Refresh':
            return "__env__.__browser__.refresh()"
        if t == 'Back':
            return "__env__.__browser__.back()"
        if t == 'Forward':
            return "__env__.__browser__.forward()"
        if t == 'WaitFor':
            sel = self._expr(node.selector, scope)
            return f"__env__.__browser__.wait_for({sel})"

        if t == 'SafeMember':
            obj = self._expr(node.obj, scope)
            name = node.name
            return f"__env__.__safe_getattr__({obj}, '{name}')"

        if t == 'Class':
            raise CompileError("Class")
        if t == 'New':
            raise CompileError("New")
        if t == 'Include':
            raise CompileError("Include")
        if t == 'TryCatch':
            raise CompileError("TryCatch")
        if t == 'Spread':
            c = self._expr(node.expr, scope)
            return f"*{c}"

        raise CompileError(f"Cannot compile {t}")

    def _expr(self, node, scope):
        if node is None:
            return 'None'
        result = self._gen(node, scope)
        if result is None:
            return None
        return result.replace('\n', ' ')

    def _gen_assign(self, node, scope):
        value = self._expr(node.value, scope)
        if value is None:
            return None
        target = node.target
        if isinstance(target, zen_ast.Variable):
            name = target.name
            if name == '_':
                return value
            if not scope.is_local(name):
                return f"__env__['{name}'] = {value}"
            if name not in scope.vars:
                scope.define(name)
            return f"{name} = {value}"
        if isinstance(target, zen_ast.ListLiteral):
            targets = []
            for t in target.elements:
                if isinstance(t, zen_ast.Variable):
                    if not scope.is_local(t.name):
                        targets.append(f"__env__['{t.name}']")
                    else:
                        targets.append(t.name)
                else:
                    c = self._expr(t, scope)
                    if c is None:
                        return None
                    targets.append(c)
            return f"{', '.join(targets)} = {value}"
        if isinstance(target, zen_ast.Member):
            obj = self._expr(target.obj, scope)
            return f"__env__.__set_prop__({obj}, '{target.name}', {value})"
        if isinstance(target, zen_ast.Index):
            obj = self._expr(target.obj, scope)
            index = self._expr(target.index, scope)
            return f"{obj}[{index}] = {value}"
        return f"__env__['{target.name}'] = {value}"

    def _gen_if(self, node, scope):
        cond = self._expr(node.condition, scope)
        if cond is None:
            return None
        then_code = self._gen(node.then_branch, scope)
        if then_code is None:
            return None
        result = f"if {cond}:\n{_indent(then_code, 1)}"
        if node.else_branch:
            else_code = self._gen(node.else_branch, scope)
            if else_code is None:
                return None
            result += f"\nelse:\n{_indent(else_code, 1)}"
        return result

    def _gen_for(self, node, scope):
        var = node.var_name
        iterable = self._expr(node.iterable, scope)
        if iterable is None:
            return None
        scope.define(var)
        body = self._gen(node.body, scope)
        if body is None:
            return None
        return f"for {var} in {iterable}:\n{_indent(body, 1)}"

    def _gen_while(self, node, scope):
        cond = self._expr(node.condition, scope)
        if cond is None:
            return None
        body = self._gen(node.body, scope)
        if body is None:
            return None
        return f"while {cond}:\n{_indent(body, 1)}"

    def _gen_func(self, node, scope):
        params = ', '.join(node.params)
        child = Scope(scope, is_func=True)
        for p in node.params:
            child.define(p)
        body = self._gen(node.body, child)
        if body is None:
            return None
        name = node.name or f'_anon_{id(node)}'
        body_indent = _indent(body, 1)
        source = f"def {name}({params}):\n{body_indent}"
        if node.name:
            source += f"\n__env__.define('{node.name}', {name})"
        return source

    def _gen_switch(self, node, scope):
        val = self._expr(node.expr, scope)
        if val is None:
            return None
        lines = []
        for i, (case_val_node, body) in enumerate(node.cases):
            case_val = self._expr(case_val_node, scope)
            body_code = self._gen(body, scope)
            if case_val is None or body_code is None:
                return None
            # Capture case body result
            blines = body_code.split('\n')
            if blines and blines[-1].strip():
                blines[-1] = f"__result__ = {blines[-1].strip()}"
                body_code = '\n'.join(blines)
            kw = 'if' if i == 0 else 'elif'
            lines.append(f"{kw} {val} == {case_val}:\n{_indent(body_code, 1)}")
        if node.default_body:
            default_code = self._gen(node.default_body, scope)
            if default_code is None:
                return None
            blines = default_code.split('\n')
            if blines and blines[-1].strip():
                blines[-1] = f"__result__ = {blines[-1].strip()}"
                default_code = '\n'.join(blines)
            lines.append(f"else:\n{_indent(default_code, 1)}")
        # Initialize __result__ to None for unmatched cases
        result = f"__result__ = None\n" + '\n'.join(lines)
        return result

    def _gen_with(self, node, scope):
        val = self._expr(node.expr, scope)
        name = node.name
        if val is None:
            return None
        scope.define(name)
        body = self._gen(node.body, scope)
        if body is None:
            return None
        # With returns the body's value - capture with __result__
        lines = body.split('\n')
        if lines and lines[-1].strip():
            lines[-1] = f"__result__ = {lines[-1].strip()}"
            body = '\n'.join(lines)
        return f"{name} = {val}\n{body}"

    def _gen_call(self, node, scope):
        callee = node.callee
        # Handle method calls like x.y() -> converted to __env__._getattr call
        if isinstance(callee, zen_ast.Variable):
            name = callee.name
            if name in ('print', 'println'):
                args = ', '.join(self._expr(a, scope) for a in node.args)
                return f"__env__.__print__({args})" if args else "__env__.__print__()"
            fn = name if scope.is_local(name) else f"__env__['{name}']"
        elif isinstance(callee, zen_ast.Member):
            obj = self._expr(callee.obj, scope)
            method = callee.name
            args = ', '.join(self._expr(a, scope) for a in node.args)
            if obj is None:
                return None
            # Check for known method patterns
            if isinstance(callee.obj, zen_ast.Variable) and not scope.is_local(callee.obj.name):
                return f"__env__._getattr({obj}, '{method}')({args})"
            return f"{obj}.{method}({args})"
        else:
            c = self._expr(callee, scope)
            if c is None:
                return None
            fn = c
        args = ', '.join(self._expr(a, scope) for a in node.args)
        if node.kwargs:
            kwargs = ', '.join(f"{k}={self._expr(v, scope)}" for k, v in node.kwargs)
            args = f"{args}, {kwargs}" if args else kwargs
        return f"{fn}({args})"

    def _gen_member(self, node, scope):
        obj = self._expr(node.obj, scope)
        name = node.name
        if obj is None:
            return None
        return f"__env__._getattr({obj}, '{name}')"
