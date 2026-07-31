from .lexer import Lexer, LexerError
from . import nodes as ast

class ParseError(Exception):
    def __init__(self, message, token):
        self.message = message
        self.token = token
        self.line = token.line
        self.col = token.col
        super().__init__(f"ParseError at {token.line}:{token.col}: {message}")

class Parser:
    def __init__(self, lexer):
        self.lexer = lexer
        self._all_tokens = []
        tok = lexer.next()
        while tok.type != 'EOF':
            self._all_tokens.append(tok)
            tok = lexer.next()
        self._all_tokens.append(tok)  # EOF
        self._pos = 0
        self.current = self._all_tokens[0]
        self._prev_line = self.current.line if self._all_tokens else 1
        self._prev_col = self.current.col if self._all_tokens else 1

    def _advance(self):
        tok = self.current
        self._prev_line = tok.line
        self._prev_col = tok.col
        self._pos += 1
        self.current = self._all_tokens[self._pos] if self._pos < len(self._all_tokens) else self._all_tokens[-1]
        return tok
    
    def _node(self, cls, *args, **kwargs):
        node = cls(*args, **kwargs)
        node.line = self._prev_line
        node.col = self._prev_col
        return node

    def _check(self, *types):
        return self.current.type in types

    def _match(self, *types):
        if self.current.type in types:
            return self._advance()
        return None

    def _expect(self, *types):
        if self.current.type in types:
            return self._advance()
        expected = '/'.join(types)
        raise ParseError(
            f"Expected {expected}, got {self.current.type}({self.current.value!r})",
            self.current)

    def _peek_token(self, offset=1):
        idx = self._pos + offset
        if idx < len(self._all_tokens):
            return self._all_tokens[idx]
        return None

    def _looks_like_dict(self):
        saved_pos = self._pos
        try:
            if not self._check('LBRACE'):
                return False
            peek = self._advance()
            self._skip_newlines()
            if self._check('STRING'):
                tok2 = self._peek_token(1)
                if tok2 and tok2.type == 'COLON':
                    return True
            elif self._check('ELLIPSIS'):
                return True
            elif self._check('RBRACE'):
                return True
            return False
        finally:
            self._pos = saved_pos
            self.current = self._all_tokens[self._pos]

    def _looks_like_with(self):
        saved = self._pos
        try:
            self._advance()
            self._skip_newlines()
            i = 1
            while True:
                tok = self._peek_token(i)
                if tok is None or tok.type in ('EOF', 'SEMICOLON', 'RBRACE', 'RBRACKET', 'RPAREN'):
                    return False
                if tok.type == 'LBRACE':
                    return True
                if tok.type in ('LPAREN', 'LBRACKET'):
                    close = 'RPAREN' if tok.type == 'LPAREN' else 'RBRACKET'
                    depth = 1
                    i += 1
                    while depth > 0:
                        t2 = self._peek_token(i)
                        if t2 is None: return False
                        if t2.type == tok.type: depth += 1
                        if t2.type == close: depth -= 1
                        i += 1
                    continue
                i += 1
        finally:
            self._pos = saved
            self.current = self._all_tokens[self._pos]

    def _skip_newlines(self):
        while self._check('NEWLINE', 'SEMICOLON'):
            self._advance()

    def _consume_semicolon(self):
        if self._check('SEMICOLON'):
            self._advance()

    def parse(self):
        statements = []
        self._skip_newlines()
        while not self._check('EOF'):
            stmt = self._statement()
            if stmt is not None:
                statements.append(stmt)
            self._consume_semicolon()
            self._skip_newlines()
        return self._node(ast.Program, statements)

    def _statement(self):
        if self._check('LET'):
            return self._parse_let()
        elif self._check('CONST'):
            return self._parse_const()
        elif self._check('GO'):
            return self._parse_go()
        elif self._check('FILL'):
            return self._parse_fill()
        elif self._check('CLICK'):
            return self._parse_click()
        elif self._check('WAIT'):
            return self._parse_wait()
        elif self._check('REFRESH'):
            return self._parse_refresh()
        elif self._check('BACK'):
            return self._parse_back()
        elif self._check('FORWARD'):
            return self._parse_forward()
        elif self._check('SHOT'):
            return self._parse_shot()
        elif self._check('SCROLL'):
            return self._parse_scroll()
        elif self._check('EXECUTE'):
            return self._parse_execute()
        elif self._check('DOWNLOAD'):
            return self._parse_download()
        elif self._check('PRINT'):
            return self._parse_print()
        elif self._check('INPUT'):
            return self._parse_input()
        elif self._check('IF'):
            return self._parse_if()
        elif self._check('FOR'):
            return self._parse_for()
        elif self._check('WHILE'):
            return self._parse_while()
        elif self._check('FUNCTION') or self._check('DEF'):
            return self._parse_function()
        elif self._check('RETURN'):
            return self._parse_return()
        elif self._check('BREAK'):
            return self._parse_break()
        elif self._check('CONTINUE'):
            return self._parse_continue()
        elif self._check('CLASS'):
            if self._peek_token(1) and self._peek_token(1).type == 'LBRACE':
                return self._parse_expression_statement()
            return self._parse_class()
        elif self._check('SWITCH'):
            return self._parse_switch()
        elif self._check('WITH') and self._looks_like_with():
            return self._parse_with()
        elif self._check('INCLUDE') or self._check('IMPORT') or self._check('REQUIRE'):
            return self._parse_include()
        elif self._check('LOAD') or self._check('USE'):
            return self._parse_load()
        elif self._check('TRY'):
            return self._parse_try()
        elif self._check('THROW') or self._check('RAISE'):
            return self._parse_throw()
        elif self._check('ASSERT'):
            return self._parse_assert()
        elif self._check('LBRACE'):
            if self._looks_like_dict():
                return self._parse_expression_statement()
            return self._parse_block()
        else:
            return self._parse_expression_statement()

    def _parse_let(self):
        self._advance()
        if self._check('LBRACKET'):
            return self._parse_destructure(ast.Let)
        if self._check('LBRACE'):
            return self._parse_destructure(ast.Let)
        name = self._expect('IDENT')
        self._expect('ASSIGN')
        value = self._parse_expression()
        return self._node(ast.Let, name.value, value)

    def _parse_const(self):
        self._advance()
        if self._check('LBRACKET'):
            return self._parse_destructure(ast.Const)
        if self._check('LBRACE'):
            return self._parse_destructure(ast.Const)
        name = self._expect('IDENT')
        self._expect('ASSIGN')
        value = self._parse_expression()
        return self._node(ast.Const, name.value, value)

    def _parse_destructure(self, cls):
        if self._check('LBRACKET'):
            self._advance()
            targets = []
            if not self._check('RBRACKET'):
                targets.append(self._expect('IDENT').value)
                while self._match('COMMA'):
                    if self._check('RBRACKET'):
                        break
                    targets.append(self._expect('IDENT').value)
            self._expect('RBRACKET')
        else:
            self._advance()
            targets = []
            if not self._check('RBRACE'):
                targets.append(self._expect('IDENT').value)
                while self._match('COMMA'):
                    if self._check('RBRACE'):
                        break
                    targets.append(self._expect('IDENT').value)
            self._expect('RBRACE')
        self._expect('ASSIGN')
        value = self._parse_expression()
        target_nodes = [self._node(ast.Variable, t) for t in targets]
        list_target = self._node(ast.ListLiteral, target_nodes)
        return self._node(ast.Assign, list_target, value)

    def _parse_go(self):
        self._advance()
        url = self._parse_expression()
        return self._node(ast.Go, url)

    def _parse_fill(self):
        self._advance()
        if self._check('LPAREN'):
            return self._parse_call_chain(self._node(ast.Variable, 'fill'))
        selector = self._parse_expression()
        self._expect('WITH')
        value = self._parse_expression()
        return self._node(ast.Fill, selector, value)

    def _parse_click(self):
        self._advance()
        if self._check('LPAREN'):
            return self._parse_call_chain(self._node(ast.Variable, 'click'))
        if self._check('EOF', 'RBRACE', 'RBRACKET', 'RPAREN', 'NEWLINE', 'SEMICOLON'):
            return self._node(ast.Click, None)
        target = self._parse_expression()
        return self._node(ast.Click, target)

    def _parse_wait(self):
        self._advance()
        if self._check('LPAREN'):
            return self._parse_call_chain(self._node(ast.Variable, 'wait'))
        if self._match('FOR'):
            selector = self._parse_expression()
            return self._node(ast.WaitFor, selector)
        expr = self._parse_expression()
        return self._node(ast.Wait, expr)

    def _parse_refresh(self):
        self._advance()
        return self._node(ast.Refresh)

    def _parse_back(self):
        self._advance()
        return self._node(ast.Back)

    def _parse_forward(self):
        self._advance()
        return self._node(ast.Forward)

    def _parse_shot(self):
        self._advance()
        path = self._parse_expression()
        full = self._match('FULL') is not None
        return self._node(ast.Shot, path, full)

    def _parse_scroll(self):
        self._advance()
        if self._check('TO'):
            self._advance()
            if self._check('TOP'):
                self._advance()
                return self._node(ast.Scroll, direction='top')
            elif self._check('BOTTOM'):
                self._advance()
                return self._node(ast.Scroll, direction='bottom')
            raise ParseError("Expected 'top' or 'bottom' after 'scroll to'", self.current)
        elif self._check('BY'):
            self._advance()
            x = self._parse_expression()
            self._expect('COMMA')
            y = self._parse_expression()
            return self._node(ast.Scroll, direction='by', x=x, y=y)
        raise ParseError("Expected 'to' or 'by' after 'scroll'", self.current)

    def _parse_execute(self):
        self._advance()
        code = self._parse_expression()
        return self._node(ast.Execute, code)

    def _parse_download(self):
        self._advance()
        url = self._parse_expression()
        self._expect('TO')
        path = self._parse_expression()
        return self._node(ast.Download, url, path)

    def _parse_print(self):
        self._advance()
        values = [self._parse_expression()]
        while self._match('COMMA'):
            values.append(self._parse_expression())
        return self._node(ast.Print, values)

    def _parse_input(self):
        self._advance()
        prompt = self._parse_expression()
        self._expect('INTO')
        target = self._expect('IDENT')
        return self._node(ast.Input, prompt, target.value)

    def _parse_class(self):
        self._advance()
        name = None
        if self._check('IDENT') and not self._check('LBRACE'):
            name = self._expect('IDENT').value
        parent = None
        if self._match('EXTENDS'):
            parent = self._parse_expression()
        body_stmts = self._parse_block().statements
        body = {}
        for stmt in body_stmts:
            if isinstance(stmt, ast.Assign) and isinstance(stmt.target, (ast.Variable, ast.Member)):
                key = stmt.target.name if isinstance(stmt.target, ast.Variable) else stmt.target.name
                body[key] = stmt.value
            elif isinstance(stmt, ast.Function):
                body[stmt.name] = stmt
            elif isinstance(stmt, ast.Let):
                body[stmt.name] = stmt.value
            else:
                body[f'_stmt_{id(stmt)}'] = stmt
        return self._node(ast.Class, name, body, parent)

    def _parse_interpolated(self, value):
        parts = []
        i = 0
        while i < len(value):
            has_dollar = (i + 1 < len(value) and value[i] == '$' and value[i + 1] == '{')
            if has_dollar:
                j = i + 2
                depth = 1
                while j < len(value) and depth > 0:
                    if value[j] == '{': depth += 1
                    elif value[j] == '}': depth -= 1
                    if depth > 0: j += 1
                if depth == 0 and j > i + 2:
                    expr_str = value[i + 2:j]
                    parts.append((True, expr_str))
                    i = j + 1
                    continue
                parts.append((False, value[i:i + 1]))
                i += 1
                continue
            if value[i] == '{' and i + 1 < len(value):
                j = i + 1
                depth = 1
                while j < len(value) and depth > 0:
                    if value[j] == '{': depth += 1
                    elif value[j] == '}': depth -= 1
                    if depth > 0: j += 1
                if depth == 0 and j > i + 1:
                    inner = value[i+1:j]
                    if inner.isidentifier():
                        parts.append((True, inner))
                        i = j + 1
                        continue
            next_brace = -1
            for k in range(i, len(value)):
                if value[k] == '$' and k + 1 < len(value) and value[k + 1] == '{':
                    next_brace = k
                    break
                if value[k] == '{':
                    next_brace = k
                    break
            if next_brace == -1:
                parts.append((False, value[i:]))
                break
            if next_brace > i:
                parts.append((False, value[i:next_brace]))
            if next_brace == i:
                parts.append((False, value[i]))
                i += 1
                continue
            i = next_brace
        if len(parts) == 1 and not parts[0][0]:
            return self._node(ast.Literal, parts[0][1])
        return self._node(ast.InterpolatedString, parts)

    def _parse_switch(self):
        self._advance()
        expr = self._parse_expression()
        self._skip_newlines()
        self._expect('LBRACE')
        cases = []
        default_body = None
        while not self._check('RBRACE') and not self._check('EOF'):
            self._skip_newlines()
            if self._match('CASE'):
                self._skip_newlines()
                case_val = self._parse_expression()
                self._skip_newlines()
                body = self._parse_block()
                cases.append((case_val, body))
            elif self._match('DEFAULT'):
                self._skip_newlines()
                default_body = self._parse_block()
            else:
                raise ParseError("Expected 'case' or 'default' in switch", self.current)
            self._skip_newlines()
        self._expect('RBRACE')
        return self._node(ast.Switch, expr, cases, default_body)

    def _parse_with(self):
        self._advance()
        expr = self._parse_expression()
        name = None
        if self._match('AS'):
            name = self._expect('IDENT').value
        body = self._parse_block()
        return self._node(ast.With, expr, name, body)

    def _parse_if(self):
        self._advance()
        return self._parse_if_tail()

    def _parse_if_tail(self):
        condition = self._parse_expression()
        then_branch = self._parse_block()
        else_branch = None
        self._skip_newlines()
        if self._match('ELSE'):
            self._skip_newlines()
            if self._check('IF'):
                self._advance()
                else_branch = self._node(ast.Block, [self._parse_if_tail()])
            else:
                else_branch = self._parse_block()
        elif self._match('ELIF'):
            else_branch = self._node(ast.Block, [self._parse_if_tail()])
        return self._node(ast.If, condition, then_branch, else_branch)

    def _parse_for(self):
        self._advance()
        var_name = self._expect('IDENT')
        self._expect('IN')
        iterable = self._parse_expression()
        body = self._parse_block()
        return self._node(ast.For, var_name.value, iterable, body)

    def _parse_while(self):
        self._advance()
        condition = self._parse_expression()
        body = self._parse_block()
        return self._node(ast.While, condition, body)

    def _parse_params(self):
        self._expect('LPAREN')
        params = []
        defaults = {}
        if not self._check('RPAREN'):
            name = self._parse_param_name()
            if self._match('ASSIGN'):
                defaults[name] = self._parse_expression()
            params.append(name)
            while self._match('COMMA'):
                name = self._parse_param_name()
                if self._match('ASSIGN'):
                    defaults[name] = self._parse_expression()
                params.append(name)
        self._expect('RPAREN')
        return params, defaults

    def _parse_param_name(self):
        if self._check('IDENT'):
            return self._expect('IDENT').value
        if self._check('SELF'):
            self._advance()
            return 'self'
        raise ParseError("Expected parameter name", self.current)

    def _parse_function(self):
        self._advance()
        name = self._expect('IDENT')
        params, defaults = self._parse_params()
        body = self._parse_block()
        return self._node(ast.Function, name.value, params, body, defaults)

    def _parse_return(self):
        self._advance()
        if self._check('EOF', 'RBRACE', 'SEMICOLON', 'NEWLINE'):
            return self._node(ast.Return, None)
        value = self._parse_expression()
        return self._node(ast.Return, value)

    def _parse_break(self):
        self._advance()
        return self._node(ast.Break)

    def _parse_continue(self):
        self._advance()
        return self._node(ast.Continue)

    def _parse_throw(self):
        self._advance()
        if self._check('EOF', 'RBRACE', 'SEMICOLON', 'NEWLINE'):
            return self._node(ast.Throw, None)
        value = self._parse_expression()
        return self._node(ast.Throw, value)

    def _parse_assert(self):
        self._advance()
        condition = self._parse_expression()
        message = None
        if self._match('COMMA'):
            message = self._parse_expression()
        return self._node(ast.Assert, condition, message)

    def _parse_include(self):
        self._advance()
        path = self._parse_expression()
        return self._node(ast.Include, path)

    def _parse_load(self):
        self._advance()
        name = self._expect('IDENT').value
        path = self._node(ast.Literal, name)
        return self._node(ast.Include, path, merge=True)

    def _parse_try(self):
        self._advance()
        try_body = self._parse_block()
        self._skip_newlines()
        self._expect('CATCH')
        err_var = None
        catch_type = None
        if self._check('IDENT'):
            name = self._advance().value
            if self._match('AS'):
                err_var = self._expect('IDENT').value
                catch_type = name
            else:
                err_var = name
        self._skip_newlines()
        catch_body = self._parse_block()
        self._skip_newlines()
        finally_body = None
        if self._check('FINALLY'):
            self._advance()
            self._skip_newlines()
            finally_body = self._parse_block()
        return self._node(ast.TryCatch, try_body, catch_body, err_var, finally_body, catch_type=catch_type)

    def _parse_block(self):
        self._skip_newlines()
        self._expect('LBRACE')
        statements = []
        self._skip_newlines()
        while not self._check('RBRACE') and not self._check('EOF'):
            stmt = self._statement()
            if stmt is not None:
                statements.append(stmt)
            self._consume_semicolon()
            self._skip_newlines()
        self._expect('RBRACE')
        return self._node(ast.Block, statements)

    def _parse_expression_statement(self):
        expr = self._parse_expression()

        # Tuple unpacking: expr, expr, ... = rhs
        if self._check('COMMA'):
            targets = [expr]
            while self._match('COMMA'):
                self._skip_newlines()
                if self._check('ASSIGN'):
                    break
                targets.append(self._parse_expression())
                self._skip_newlines()
            if self._match('ASSIGN'):
                for t in targets:
                    if not isinstance(t, (ast.Variable, ast.Member, ast.Index)):
                        raise ParseError("Invalid unpacking target", self.current)
                self._skip_newlines()
                rhs_values = [self._parse_expression()]
                while self._match('COMMA'):
                    self._skip_newlines()
                    rhs_values.append(self._parse_expression())
                    self._skip_newlines()
                value_node = rhs_values[0] if len(rhs_values) == 1 else self._node(ast.ListLiteral, rhs_values)
                return self._node(ast.Assign, self._node(ast.ListLiteral, targets), value_node)
            raise ParseError("Unexpected comma in expression statement", self.current)

        # Compound assignment (desugar to simple assign)
        compound_ops = {
            'PLUS_ASSIGN': 'PLUS',
            'MINUS_ASSIGN': 'MINUS',
            'STAR_ASSIGN': 'STAR',
            'SLASH_ASSIGN': 'SLASH',
            'MOD_ASSIGN': 'MOD',
            'NULLISH_ASSIGN': '??',
        }
        op_token = self._match(*compound_ops.keys())
        if op_token:
            if not isinstance(expr, (ast.Variable, ast.Member, ast.Index)):
                raise ParseError("Invalid compound assignment target", self.current)
            self._skip_newlines()
            right = self._parse_expression()
            value = self._node(ast.BinaryOp, expr, compound_ops[op_token.type], right)
            return self._node(ast.Assign, expr, value)

        if self._match('ASSIGN'):
            if not isinstance(expr, (ast.Variable, ast.Member, ast.Index, ast.ListLiteral)):
                raise ParseError("Invalid assignment target", self.current)
            self._skip_newlines()
            value = self._parse_expression()
            return self._node(ast.Assign, expr, value)
        return expr

    def _parse_expression(self):
        return self._parse_ternary()

    def _parse_ternary(self):
        then_val = self._parse_range()
        if self._match('IF'):
            cond = self._parse_range()
            self._expect('ELSE')
            else_val = self._parse_ternary()
            return self._node(ast.Ternary, then_val, cond, else_val)
        return then_val

    def _parse_range(self):
        left = self._parse_nullish()
        if self._match('TO') or self._match('RARROW'):
            right = self._parse_range()
            step = None
            if self._match('BY') or self._match('AT'):
                step = self._parse_range()
            return self._node(ast.Range, left, right, step)
        if self._match('DOT_DOT'):
            right = self._parse_range()
            step = None
            if self._match('BY') or self._match('AT'):
                step = self._parse_range()
            return self._node(ast.Range, left, right, step, exclusive=True)
        return left

    def _parse_nullish(self):
        left = self._parse_or()
        while self._match('NULLISH_COALESCE'):
            right = self._parse_or()
            left = self._node(ast.BinaryOp, left, '??', right)
        return left

    def _parse_or(self):
        left = self._parse_and()
        while self._match('OR') or self._match('PIPE_PIPE'):
            right = self._parse_and()
            left = self._node(ast.BinaryOp, left, 'or', right)
        return left

    def _parse_and(self):
        left = self._parse_not()
        while self._match('AND') or self._match('AMPERSAND_AMPERSAND'):
            right = self._parse_not()
            left = self._node(ast.BinaryOp, left, 'and', right)
        return left

    def _parse_not(self):
        if self._match('NOT'):
            inner = self._parse_not()
            return self._node(ast.UnaryOp, 'not', inner)
        return self._parse_comparison()

    def _parse_comparison(self):
        left = self._parse_bitwise()
        op_token = self._match('EQ', 'NEQ', 'STRICT_EQ', 'STRICT_NEQ', 'LT', 'GT', 'LE', 'GE', 'IN', 'IS')
        if not op_token:
            if self._check('NOT'):
                peek = self._peek_token(1)
                if peek and peek.type == 'IN':
                    self._advance()
                    self._advance()
                    right = self._parse_bitwise()
                    binop = self._node(ast.BinaryOp, left, 'IN', right)
                    return self._node(ast.UnaryOp, 'not', binop)
            return left
        if op_token.type == 'IS' and self._match('NOT'):
            right = self._parse_bitwise()
            binop = self._node(ast.BinaryOp, left, 'IS', right)
            return self._node(ast.UnaryOp, 'not', binop)
        right = self._parse_bitwise()
        result = self._node(ast.BinaryOp, left, op_token.type, right)
        while True:
            next_op = self._match('EQ', 'NEQ', 'STRICT_EQ', 'STRICT_NEQ', 'LT', 'GT', 'LE', 'GE')
            if not next_op:
                break
            next_right = self._parse_bitwise()
            chain = self._node(ast.BinaryOp, right, next_op.type, next_right)
            result = self._node(ast.BinaryOp, result, 'and', chain)
            right = next_right
        return result

    def _parse_bitwise(self):
        left = self._parse_shift()
        while self._match('AMPERSAND'):
            right = self._parse_shift()
            left = self._node(ast.BinaryOp, left, 'AMPERSAND', right)
        return left

    def _parse_shift(self):
        left = self._parse_xor()
        while True:
            op = self._match('LSHIFT', 'RSHIFT')
            if not op:
                break
            right = self._parse_xor()
            left = self._node(ast.BinaryOp, left, op.type, right)
        return left

    def _parse_xor(self):
        left = self._parse_pipe()
        while self._match('CARET'):
            right = self._parse_pipe()
            left = self._node(ast.BinaryOp, left, 'CARET', right)
        return left

    def _parse_pipe(self):
        left = self._parse_addition()
        while self._match('PIPE'):
            right = self._parse_addition()
            left = self._node(ast.BinaryOp, left, 'PIPE', right)
        return left

    def _parse_addition(self):
        left = self._parse_term()
        while True:
            op = self._match('PLUS', 'MINUS')
            if not op:
                break
            right = self._parse_term()
            left = self._node(ast.BinaryOp, left, op.type, right)
        return left

    def _parse_term(self):
        left = self._parse_unary()
        while True:
            op = self._match('STAR', 'SLASH', 'MOD')
            if not op:
                break
            right = self._parse_unary()
            left = self._node(ast.BinaryOp, left, op.type, right)
        return left

    def _parse_unary(self):
        if self._match('MINUS'):
            inner = self._parse_unary()
            return self._node(ast.UnaryOp, '-', inner)
        if self._match('BANG'):
            inner = self._parse_unary()
            return self._node(ast.UnaryOp, '!', inner)
        if self._match('TYPEOF'):
            inner = self._parse_unary()
            return self._node(ast.UnaryOp, 'typeof', inner)
        if self._match('TILDE'):
            inner = self._parse_unary()
            return self._node(ast.UnaryOp, '~', inner)
        return self._parse_pow()

    def _parse_pow(self):
        left = self._parse_call_chain()
        if self._match('POW'):
            right = self._parse_pow()
            left = self._node(ast.BinaryOp, left, 'POW', right)
        return left

    def _parse_call_chain(self, left=None):
        if left is None:
            left = self._parse_atom()
        while True:
            if self._check('INC', 'DEC'):
                incdec = self._advance()
                if not isinstance(left, (ast.Variable, ast.Member, ast.Index)):
                    raise ParseError("Invalid increment/decrement target", self.current)
                op = 'PLUS' if incdec.type == 'INC' else 'MINUS'
                value = self._node(ast.BinaryOp, left, op, self._node(ast.Literal, 1))
                left = self._node(ast.Assign, left, value)
            if self._check('LPAREN'):
                self._advance()
                self._skip_newlines()
                args = []
                kwargs = []
                if not self._check('RPAREN'):
                    self._parse_one_call_arg(args, kwargs)
                    self._skip_newlines()
                    while self._match('COMMA'):
                        self._skip_newlines()
                        if self._check('RPAREN'):
                            break
                        self._parse_one_call_arg(args, kwargs)
                        self._skip_newlines()
                self._expect('RPAREN')
                left = self._node(ast.Call, left, args, kwargs)
            elif self._match('SAFE_DOT'):
                name = self._parse_member_name()
                left = self._node(ast.SafeMember, left, name)
            elif self._check('DOT'):
                self._advance()
                name = self._parse_member_name()
                left = self._node(ast.Member, left, name)
            elif self._check('LBRACKET'):
                self._advance()
                self._skip_newlines()
                if self._match('COLON'):
                    start = None
                    if self._check('COLON'):
                        end = None
                        self._advance()
                        step = self._parse_expression() if not self._check('RBRACKET') else None
                    elif self._check('RBRACKET'):
                        end = None
                        step = None
                    else:
                        end = self._parse_expression()
                        step = None
                        if self._match('COLON'):
                            step = self._parse_expression() if not self._check('RBRACKET') else None
                    self._expect('RBRACKET')
                    left = self._node(ast.Slice, left, start, end, step)
                else:
                    start = self._parse_expression()
                    self._skip_newlines()
                    if self._match('COLON'):
                        end = self._parse_expression() if not self._check('RBRACKET') and not self._check('COLON') else None
                        step = None
                        if self._match('COLON'):
                            step = self._parse_expression() if not self._check('RBRACKET') else None
                        self._expect('RBRACKET')
                        left = self._node(ast.Slice, left, start, end, step)
                    else:
                        self._expect('RBRACKET')
                        left = self._node(ast.Index, left, start)
            else:
                break
        return left

    def _parse_member_name(self):
        tok = self._advance()
        if tok.type == 'IDENT':
            return tok.value
        if tok.type in self.FUNC_KEYWORDS:
            return tok.type.lower()
        raise ParseError(f"Expected member name, got {tok.type}({tok.value!r})", tok)

    FUNC_KEYWORDS = {
        'PRINT', 'EXECUTE', 'DOWNLOAD', 'SHOT', 'INPUT',
        'WAIT', 'CLICK', 'FILL', 'GO', 'SCROLL', 'REFRESH',
        'BACK', 'FORWARD', 'WITH', 'FOR', 'RETURN', 'WAIT',
        'BREAK', 'CONTINUE', 'IF', 'ELSE', 'ELIF', 'WHILE', 'FUNCTION',
        'AND', 'OR', 'NOT', 'TRY', 'CATCH', 'LET', 'CONST', 'IN', 'INTO',
        'TO', 'BY', 'FULL', 'TOP', 'BOTTOM', 'INCLUDE', 'IMPORT', 'REQUIRE',
        'CLASS', 'EXTENDS', 'NEW', 'SELF', 'SWITCH', 'CASE', 'DEFAULT', 'AS',
        'TYPEOF', 'THROW', 'RAISE', 'ASSERT', 'LAMBDA',
        'LOAD', 'USE',
    }

    def _parse_one_call_arg(self, args, kwargs):
        if self._check('IDENT') and self._peek_token(1) and self._peek_token(1).type == 'ASSIGN':
            name = self._advance().value
            self._advance()
            kwargs.append((name, self._parse_expression()))
        elif self.current.type in self.FUNC_KEYWORDS and self._peek_token(1) and self._peek_token(1).type == 'ASSIGN':
            tok = self._advance()
            self._advance()
            kwargs.append((tok.type.lower(), self._parse_expression()))
        else:
            args.append(self._parse_expression())

    def _parse_atom(self):
        self._skip_newlines()
        if self._match('NEW'):
            class_expr = self._parse_call_chain()
            if isinstance(class_expr, ast.Call):
                args = class_expr.args
                class_expr = class_expr.callee
            else:
                args = []
            return self._node(ast.New, class_expr, args)
        if self._check('NUMBER'):
            tok = self._advance()
            val = float(tok.value) if '.' in tok.value else int(tok.value)
            return self._node(ast.Literal, val)
        if self._check('STRING'):
            tok = self._advance()
            if '{' in tok.value and '}' in tok.value:
                return self._parse_interpolated(tok.value)
            return self._node(ast.Literal, tok.value)
        if self._check('BACKTICK_STRING'):
            tok = self._advance()
            if '{' in tok.value and '}' in tok.value:
                return self._parse_interpolated(tok.value)
            return self._node(ast.Literal, tok.value)
        if self._check('BOOL'):
            tok = self._advance()
            return self._node(ast.Literal, tok.value)
        if self._check('NULL'):
            self._advance()
            return self._node(ast.Literal, None)
        if self._check('CLASS'):
            return self._parse_class()
        if self._check('FUNCTION'):
            return self._parse_anonymous_function()
        if self._check('LAMBDA'):
            return self._parse_lambda()
        if self._check('INCLUDE') or self._check('IMPORT') or self._check('REQUIRE'):
            return self._parse_include()
        if self._check('LOAD') or self._check('USE'):
            return self._parse_load()
        if self._check('IDENT') or self.current.type in self.FUNC_KEYWORDS:
            tok = self._advance()
            name = tok.value if tok.type == 'IDENT' else tok.type.lower()
            return self._node(ast.Variable, name)
        if self._check('LBRACKET'):
            return self._parse_list()
        if self._check('LBRACE'):
            return self._parse_dict()
        if self._check('LPAREN'):
            saved = self._pos
            saved_current = self.current
            self._advance()
            self._skip_newlines()
            params = []
            is_arrow = False
            if self._check('RPAREN'):
                self._advance()
                if self._match('ARROW'):
                    is_arrow = True
            elif self._check('IDENT'):
                params.append(self._advance().value)
                while self._match('COMMA'):
                    self._skip_newlines()
                    params.append(self._expect('IDENT').value)
                if self._check('RPAREN'):
                    self._advance()
                    if self._match('ARROW'):
                        is_arrow = True
            if is_arrow:
                body = self._parse_expression()
                return self._node(ast.ArrowFunction, params, body)
            self._pos = saved
            self.current = saved_current
            self._advance()
            self._skip_newlines()
            if self._check('RPAREN'):
                self._advance()
                return self._node(ast.ListLiteral, [])
            exprs = [self._parse_expression()]
            while self._match('COMMA'):
                self._skip_newlines()
                exprs.append(self._parse_expression())
            self._skip_newlines()
            self._expect('RPAREN')
            if len(exprs) == 1:
                return exprs[0]
            return self._node(ast.ListLiteral, exprs)
        raise ParseError(
            f"Unexpected token: {self.current.type}({self.current.value!r})",
            self.current)

    def _parse_anonymous_function(self):
        self._advance()
        self._skip_newlines()
        params, defaults = self._parse_params()
        body = self._parse_block()
        return self._node(ast.Function, None, params, body, defaults)

    def _parse_lambda(self):
        self._advance()
        params = []
        if self._check('IDENT'):
            params.append(self._advance().value)
            while self._match('COMMA'):
                params.append(self._expect('IDENT').value)
        self._expect('COLON')
        body = self._parse_expression()
        return self._node(ast.ArrowFunction, params, body)

    def _parse_list(self):
        self._advance()
        self._skip_newlines()
        elements = []
        if not self._check('RBRACKET'):
            self._parse_one_list_element(elements)
            if self._match('FOR'):
                first_expr = elements[0]
                var_name = self._expect('IDENT').value
                self._expect('IN')
                iterable = self._parse_range()
                condition = None
                if self._match('IF'):
                    condition = self._parse_range()
                self._expect('RBRACKET')
                return self._node(ast.ListComprehension, first_expr, var_name, iterable, condition)
            self._skip_newlines()
            while self._match('COMMA'):
                self._skip_newlines()
                if self._check('RBRACKET'):
                    break
                self._parse_one_list_element(elements)
                self._skip_newlines()
        self._expect('RBRACKET')
        return self._node(ast.ListLiteral, elements)

    def _parse_one_list_element(self, elements):
        if self._match('ELLIPSIS'):
            elements.append(self._node(ast.Spread, self._parse_expression()))
        else:
            elements.append(self._parse_expression())

    def _parse_dict(self):
        self._advance()
        self._skip_newlines()
        pairs = []
        if not self._check('RBRACE'):
            self._parse_one_dict_pair(pairs)
            self._skip_newlines()
            while self._match('COMMA'):
                self._skip_newlines()
                if self._check('RBRACE'):
                    break
                self._parse_one_dict_pair(pairs)
                self._skip_newlines()
        self._expect('RBRACE')
        return self._node(ast.DictLiteral, pairs)

    def _parse_one_dict_pair(self, pairs):
        if self._match('ELLIPSIS'):
            pairs.append((self._node(ast.Spread, self._parse_expression()), None))
        else:
            if self._check('IDENT') and self._peek_token(1) and self._peek_token(1).type == 'COLON':
                tok = self._advance()
                key = self._node(ast.Literal, tok.value)
            else:
                key = self._parse_expression()
            self._skip_newlines()
            self._expect('COLON')
            self._skip_newlines()
            value = self._parse_expression()
            pairs.append((key, value))
