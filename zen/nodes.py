class Node:
    def __init__(self, **kwargs):
        for k, v in kwargs.items():
            setattr(self, k, v)

class Program(Node):
    def __init__(self, statements):
        self.statements = statements

class Let(Node):
    def __init__(self, name, value):
        self.name = name
        self.value = value

class Assign(Node):
    def __init__(self, target, value):
        self.target = target
        self.value = value

class Go(Node):
    def __init__(self, url):
        self.url = url

class Fill(Node):
    def __init__(self, selector, value):
        self.selector = selector
        self.value = value

class Click(Node):
    def __init__(self, target=None):
        self.target = target

class Wait(Node):
    def __init__(self, duration):
        self.duration = duration

class WaitFor(Node):
    def __init__(self, selector):
        self.selector = selector

class Refresh(Node):
    pass

class Back(Node):
    pass

class Forward(Node):
    pass

class Shot(Node):
    def __init__(self, path, full=False):
        self.path = path
        self.full = full

class Scroll(Node):
    def __init__(self, direction=None, x=None, y=None):
        self.direction = direction
        self.x = x
        self.y = y

class Execute(Node):
    def __init__(self, code):
        self.code = code

class Download(Node):
    def __init__(self, url, path):
        self.url = url
        self.path = path

class Print(Node):
    def __init__(self, values):
        self.values = values if isinstance(values, list) else [values]

class Input(Node):
    def __init__(self, prompt, target):
        self.prompt = prompt
        self.target = target

class If(Node):
    def __init__(self, condition, then_branch, else_branch=None):
        self.condition = condition
        self.then_branch = then_branch
        self.else_branch = else_branch

class For(Node):
    def __init__(self, var_name, iterable, body):
        self.var_name = var_name
        self.iterable = iterable
        self.body = body

class While(Node):
    def __init__(self, condition, body):
        self.condition = condition
        self.body = body

class Function(Node):
    def __init__(self, name, params, body, defaults=None):
        self.name = name
        self.params = params
        self.body = body
        self.defaults = defaults or {}

class Return(Node):
    def __init__(self, value=None):
        self.value = value

class Break(Node):
    pass

class Continue(Node):
    pass

class TryCatch(Node):
    def __init__(self, try_body, catch_body, err_var=None, finally_body=None, catch_type=None):
        self.try_body = try_body
        self.catch_body = catch_body
        self.err_var = err_var
        self.finally_body = finally_body
        self.catch_type = catch_type

class Block(Node):
    def __init__(self, statements):
        self.statements = statements

class BinaryOp(Node):
    def __init__(self, left, op, right):
        self.left = left
        self.op = op
        self.right = right

class UnaryOp(Node):
    def __init__(self, op, operand):
        self.op = op
        self.operand = operand

class Call(Node):
    def __init__(self, callee, args, kwargs=None):
        self.callee = callee
        self.args = args
        self.kwargs = kwargs or []

class Index(Node):
    def __init__(self, obj, index):
        self.obj = obj
        self.index = index

class Member(Node):
    def __init__(self, obj, name):
        self.obj = obj
        self.name = name

class Slice(Node):
    def __init__(self, obj, start=None, end=None, step=None):
        self.obj = obj
        self.start = start
        self.end = end
        self.step = step

class Include(Node):
    def __init__(self, path, merge=False):
        self.path = path
        self.merge = merge

class Literal(Node):
    def __init__(self, value):
        self.value = value

class ListLiteral(Node):
    def __init__(self, elements):
        self.elements = elements

class DictLiteral(Node):
    def __init__(self, pairs):
        self.pairs = pairs

class Variable(Node):
    def __init__(self, name):
        self.name = name

class Class(Node):
    def __init__(self, name, body, parent=None):
        self.name = name
        self.body = body
        self.parent = parent

class Ternary(Node):
    def __init__(self, then_val, cond, else_val):
        self.then_val = then_val
        self.cond = cond
        self.else_val = else_val

class InterpolatedString(Node):
    def __init__(self, parts):
        self.parts = parts

class Spread(Node):
    def __init__(self, expr):
        self.expr = expr

class SafeMember(Node):
    def __init__(self, obj, name):
        self.obj = obj
        self.name = name

class Switch(Node):
    def __init__(self, expr, cases, default_body=None):
        self.expr = expr
        self.cases = cases
        self.default_body = default_body

class With(Node):
    def __init__(self, expr, name, body):
        self.expr = expr
        self.name = name
        self.body = body

class Range(Node):
    def __init__(self, start, end, step=None, exclusive=False):
        self.start = start
        self.end = end
        self.step = step
        self.exclusive = exclusive

class New(Node):
    def __init__(self, class_expr, args=None):
        self.class_expr = class_expr
        self.args = args or []
