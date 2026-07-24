from .lexer import Lexer, LexerError
from . import nodes as ast

class ParseError(Exception):
    def __init__(self, message, token):
        self.message = message
        self.token = token
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

    def _advance(self):
        tok = self.current
        self._pos += 1
        self.current = self._all_tokens[self._pos] if self._pos < len(self._all_tokens) else self._all_tokens[-1]
        return tok

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
            elif self._check('RBRACE'):
                return True
            return False
        finally:
            self._pos = saved_pos
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
        return ast.Program(statements)

    def _statement(self):
        if self._check('LET'):
            return self._parse_let()
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
        elif self._check('INCLUDE') or self._check('IMPORT') or self._check('REQUIRE'):
            return self._parse_include()
        elif self._check('TRY'):
            return self._parse_try()
        elif self._check('LBRACE'):
            if self._looks_like_dict():
                return self._parse_expression_statement()
            return self._parse_block()
        else:
            return self._parse_expression_statement()

    def _parse_let(self):
        self._advance()
        name = self._expect('IDENT')
        self._expect('ASSIGN')
        value = self._parse_expression()
        return ast.Let(name.value, value)

    def _parse_go(self):
        self._advance()
        url = self._parse_expression()
        return ast.Go(url)

    def _parse_fill(self):
        self._advance()
        if self._check('LPAREN'):
            return self._parse_call_chain(ast.Variable('fill'))
        selector = self._parse_expression()
        self._expect('WITH')
        value = self._parse_expression()
        return ast.Fill(selector, value)

    def _parse_click(self):
        self._advance()
        if self._check('LPAREN'):
            return self._parse_call_chain(ast.Variable('click'))
        if self._check('EOF', 'RBRACE', 'RBRACKET', 'RPAREN', 'NEWLINE', 'SEMICOLON'):
            return ast.Click(None)
        target = self._parse_expression()
        return ast.Click(target)

    def _parse_wait(self):
        self._advance()
        if self._check('LPAREN'):
            return self._parse_call_chain(ast.Variable('wait'))
        if self._match('FOR'):
            selector = self._parse_expression()
            return ast.WaitFor(selector)
        return ast.Wait(self._parse_expression())

    def _parse_refresh(self):
        self._advance()
        return ast.Refresh()

    def _parse_back(self):
        self._advance()
        return ast.Back()

    def _parse_forward(self):
        self._advance()
        return ast.Forward()

    def _parse_shot(self):
        self._advance()
        path = self._parse_expression()
        full = self._match('FULL') is not None
        return ast.Shot(path, full)

    def _parse_scroll(self):
        self._advance()
        if self._check('TO'):
            self._advance()
            if self._check('TOP'):
                self._advance()
                return ast.Scroll(direction='top')
            elif self._check('BOTTOM'):
                self._advance()
                return ast.Scroll(direction='bottom')
            raise ParseError("Expected 'top' or 'bottom' after 'scroll to'", self.current)
        elif self._check('BY'):
            self._advance()
            x = self._parse_expression()
            self._expect('COMMA')
            y = self._parse_expression()
            return ast.Scroll(direction='by', x=x, y=y)
        raise ParseError("Expected 'to' or 'by' after 'scroll'", self.current)

    def _parse_execute(self):
        self._advance()
        code = self._parse_expression()
        return ast.Execute(code)

    def _parse_download(self):
        self._advance()
        url = self._parse_expression()
        self._expect('TO')
        path = self._parse_expression()
        return ast.Download(url, path)

    def _parse_print(self):
        self._advance()
        values = [self._parse_expression()]
        while self._match('COMMA'):
            values.append(self._parse_expression())
        return ast.Print(values)

    def _parse_input(self):
        self._advance()
        prompt = self._parse_expression()
        self._expect('INTO')
        target = self._expect('IDENT')
        return ast.Input(prompt, target.value)

    def _parse_if(self):
        self._advance()
        condition = self._parse_expression()
        then_branch = self._parse_block()
        else_branch = None
        self._skip_newlines()
        if self._match('ELSE'):
            self._skip_newlines()
            if self._check('IF'):
                elseif = self._parse_if()
                else_branch = ast.Block([elseif])
            else:
                else_branch = self._parse_block()
        return ast.If(condition, then_branch, else_branch)

    def _parse_for(self):
        self._advance()
        var_name = self._expect('IDENT')
        self._expect('IN')
        iterable = self._parse_expression()
        body = self._parse_block()
        return ast.For(var_name.value, iterable, body)

    def _parse_while(self):
        self._advance()
        condition = self._parse_expression()
        body = self._parse_block()
        return ast.While(condition, body)

    def _parse_params(self):
        self._expect('LPAREN')
        params = []
        defaults = {}
        if not self._check('RPAREN'):
            name = self._expect('IDENT').value
            if self._match('ASSIGN'):
                defaults[name] = self._parse_expression()
            params.append(name)
            while self._match('COMMA'):
                name = self._expect('IDENT').value
                if self._match('ASSIGN'):
                    defaults[name] = self._parse_expression()
                params.append(name)
        self._expect('RPAREN')
        return params, defaults

    def _parse_function(self):
        self._advance()
        name = self._expect('IDENT')
        params, defaults = self._parse_params()
        body = self._parse_block()
        return ast.Function(name.value, params, body, defaults)

    def _parse_return(self):
        self._advance()
        if self._check('EOF', 'RBRACE', 'SEMICOLON', 'NEWLINE'):
            return ast.Return(None)
        value = self._parse_expression()
        return ast.Return(value)

    def _parse_break(self):
        self._advance()
        return ast.Break()

    def _parse_continue(self):
        self._advance()
        return ast.Continue()

    def _parse_include(self):
        self._advance()
        path = self._parse_expression()
        return ast.Include(path)

    def _parse_try(self):
        self._advance()
        try_body = self._parse_block()
        self._skip_newlines()
        self._expect('CATCH')
        err_var = None
        if self._check('IDENT'):
            err_var = self._advance().value
        self._skip_newlines()
        catch_body = self._parse_block()
        self._skip_newlines()
        finally_body = None
        if self._check('FINALLY'):
            self._advance()
            self._skip_newlines()
            finally_body = self._parse_block()
        return ast.TryCatch(try_body, catch_body, err_var, finally_body)

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
        return ast.Block(statements)

    def _parse_expression_statement(self):
        expr = self._parse_expression()
        if self._match('ASSIGN'):
            if not isinstance(expr, (ast.Variable, ast.Member, ast.Index)):
                raise ParseError("Invalid assignment target", self.current)
            value = self._parse_expression()
            return ast.Assign(expr, value)
        return expr

    def _parse_expression(self):
        return self._parse_or()

    def _parse_or(self):
        left = self._parse_and()
        while self._match('OR') or self._match('PIPE_PIPE'):
            right = self._parse_and()
            left = ast.BinaryOp(left, 'or', right)
        return left

    def _parse_and(self):
        left = self._parse_not()
        while self._match('AND') or self._match('AMPERSAND_AMPERSAND'):
            right = self._parse_not()
            left = ast.BinaryOp(left, 'and', right)
        return left

    def _parse_not(self):
        if self._match('NOT'):
            return ast.UnaryOp('not', self._parse_not())
        return self._parse_comparison()

    def _parse_comparison(self):
        left = self._parse_addition()
        op_token = self._match('EQ', 'NEQ', 'LT', 'GT', 'LE', 'GE', 'IN', 'IS')
        if op_token:
            right = self._parse_addition()
            return ast.BinaryOp(left, op_token.type, right)
        if self._check('NOT') and self.lexer.peek().type == 'IN':
            self._advance()
            self._advance()
            right = self._parse_addition()
            return ast.UnaryOp('not', ast.BinaryOp(left, 'IN', right))
        return left

    def _parse_addition(self):
        left = self._parse_term()
        while True:
            op = self._match('PLUS', 'MINUS')
            if not op:
                break
            right = self._parse_term()
            left = ast.BinaryOp(left, op.type, right)
        return left

    def _parse_term(self):
        left = self._parse_unary()
        while True:
            op = self._match('STAR', 'SLASH', 'MOD')
            if not op:
                break
            right = self._parse_unary()
            left = ast.BinaryOp(left, op.type, right)
        return left

    def _parse_unary(self):
        if self._match('MINUS'):
            return ast.UnaryOp('-', self._parse_unary())
        if self._match('BANG'):
            return ast.UnaryOp('!', self._parse_unary())
        return self._parse_pow()

    def _parse_pow(self):
        left = self._parse_call_chain()
        if self._match('POW'):
            right = self._parse_pow()
            left = ast.BinaryOp(left, 'POW', right)
        return left

    def _parse_call_chain(self, left=None):
        if left is None:
            left = self._parse_atom()
        while True:
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
                        self._parse_one_call_arg(args, kwargs)
                        self._skip_newlines()
                self._expect('RPAREN')
                left = ast.Call(left, args, kwargs)
            elif self._check('DOT'):
                self._advance()
                name = self._parse_member_name()
                left = ast.Member(left, name)
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
                    left = ast.Slice(left, start, end, step)
                else:
                    start = self._parse_expression()
                    self._skip_newlines()
                    if self._match('COLON'):
                        end = self._parse_expression() if not self._check('RBRACKET') and not self._check('COLON') else None
                        step = None
                        if self._match('COLON'):
                            step = self._parse_expression() if not self._check('RBRACKET') else None
                        self._expect('RBRACKET')
                        left = ast.Slice(left, start, end, step)
                    else:
                        self._expect('RBRACKET')
                        left = ast.Index(left, start)
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
        'BREAK', 'CONTINUE', 'IF', 'ELSE', 'WHILE', 'FUNCTION',
        'AND', 'OR', 'NOT', 'TRY', 'CATCH', 'LET', 'IN', 'INTO',
        'TO', 'BY', 'FULL', 'TOP', 'BOTTOM', 'INCLUDE', 'IMPORT', 'REQUIRE',
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
        if self._check('NUMBER'):
            tok = self._advance()
            val = float(tok.value) if '.' in tok.value else int(tok.value)
            return ast.Literal(val)
        if self._check('STRING'):
            tok = self._advance()
            return ast.Literal(tok.value)
        if self._check('BOOL'):
            tok = self._advance()
            return ast.Literal(tok.value)
        if self._check('NULL'):
            self._advance()
            return ast.Literal(None)
        if self._check('FUNCTION'):
            return self._parse_anonymous_function()
        if self._check('INCLUDE') or self._check('IMPORT') or self._check('REQUIRE'):
            return self._parse_include()
        if self._check('IDENT') or self.current.type in self.FUNC_KEYWORDS:
            tok = self._advance()
            name = tok.value if tok.type == 'IDENT' else tok.type.lower()
            return ast.Variable(name)
        if self._check('LBRACKET'):
            return self._parse_list()
        if self._check('LBRACE'):
            return self._parse_dict()
        if self._check('LPAREN'):
            self._advance()
            self._skip_newlines()
            if self._check('RPAREN'):
                self._advance()
                return ast.ListLiteral([])
            exprs = [self._parse_expression()]
            while self._match('COMMA'):
                self._skip_newlines()
                exprs.append(self._parse_expression())
            self._skip_newlines()
            self._expect('RPAREN')
            if len(exprs) == 1:
                return exprs[0]
            return ast.ListLiteral(exprs)
        raise ParseError(
            f"Unexpected token: {self.current.type}({self.current.value!r})",
            self.current)

    def _parse_anonymous_function(self):
        self._advance()
        self._skip_newlines()
        params, defaults = self._parse_params()
        body = self._parse_block()
        return ast.Function(None, params, body, defaults)

    def _parse_list(self):
        self._advance()
        self._skip_newlines()
        elements = []
        if not self._check('RBRACKET'):
            elements.append(self._parse_expression())
            self._skip_newlines()
            while self._match('COMMA'):
                self._skip_newlines()
                elements.append(self._parse_expression())
                self._skip_newlines()
        self._expect('RBRACKET')
        return ast.ListLiteral(elements)

    def _parse_dict(self):
        self._advance()
        self._skip_newlines()
        pairs = []
        if not self._check('RBRACE'):
            key = self._parse_expression()
            self._skip_newlines()
            self._expect('COLON')
            self._skip_newlines()
            value = self._parse_expression()
            pairs.append((key, value))
            self._skip_newlines()
            while self._match('COMMA'):
                self._skip_newlines()
                key = self._parse_expression()
                self._skip_newlines()
                self._expect('COLON')
                self._skip_newlines()
                value = self._parse_expression()
                pairs.append((key, value))
                self._skip_newlines()
        self._expect('RBRACE')
        return ast.DictLiteral(pairs)
