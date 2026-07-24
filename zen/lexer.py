import re

class Token:
    def __init__(self, type, value, line, col):
        self.type = type
        self.value = value
        self.line = line
        self.col = col

    def __repr__(self):
        return f"Token({self.type}, {self.value!r}, L{self.line}:{self.col})"

KEYWORDS = {
    'let', 'go', 'fill', 'with', 'click', 'wait', 'for', 'in',
    'if', 'else', 'while', 'function', 'def', 'return', 'print', 'input',
    'into', 'scroll', 'to', 'by', 'shot', 'full', 'refresh',
    'back', 'forward', 'execute', 'download', 'and', 'or', 'not',
    'true', 'false', 'null', 'try', 'catch', 'top', 'bottom',
    'break', 'continue', 'include', 'import', 'require', 'is', 'finally',
}

TOKEN_SPEC = [
    ('NUMBER',   r'-?\d+(?:\.\d+)?'),
    ('STRING',   r'"[^"\\]*(?:\\.[^"\\]*)*"|\'[^\'\\]*(?:\\.[^\'\\]*)*\''),
    ('COMMENT',  r'//[^\n]*'),
    ('HASH_COMMENT', r'#[^\n]*'),
    ('BLOCK_COMMENT', r'/\*[\s\S]*?\*/'),
    ('LPAREN',   r'\('),
    ('RPAREN',   r'\)'),
    ('LBRACE',   r'\{'),
    ('RBRACE',   r'\}'),
    ('LBRACKET', r'\['),
    ('RBRACKET', r'\]'),
    ('SEMICOLON', r';'),
    ('COMMA',    r','),
    ('DOT',      r'\.'),
    ('COLON',    r':'),
    ('PIPE_PIPE',  r'\|\|'),
    ('AMPERSAND_AMPERSAND', r'&&'),
    ('EQ',       r'=='),
    ('NEQ',      r'!='),
    ('LE',       r'<='),
    ('GE',       r'>='),
    ('POW',      r'\*\*'),
    ('ASSIGN',   r'='),
    ('LT',       r'<'),
    ('GT',       r'>'),
    ('PLUS',     r'\+'),
    ('MINUS',    r'-'),
    ('STAR',     r'\*'),
    ('SLASH',    r'/'),
    ('MOD',      r'%'),
    ('BANG',     r'!'),
    ('IDENT',    r'[a-zA-Z_][a-zA-Z0-9_]*'),
    ('NEWLINE',  r'\n'),
    ('WS',       r'[ \t\r]+'),
]

TOKEN_RE = re.compile('|'.join(f'(?P<{name}>{pattern})' for name, pattern in TOKEN_SPEC))

_ESCAPE_MAP = {
    'n': '\n',
    't': '\t',
    'r': '\r',
    '\\': '\\',
    '"': '"',
    "'": "'",
    '0': '\0',
}

def _process_escapes(s):
    result = []
    i = 0
    while i < len(s):
        if s[i] == '\\' and i + 1 < len(s):
            e = s[i + 1]
            result.append(_ESCAPE_MAP.get(e, '\\' + e))
            i += 2
        else:
            result.append(s[i])
            i += 1
    return ''.join(result)


class LexerError(Exception):
    def __init__(self, message, line, col):
        self.message = message
        self.line = line
        self.col = col
        super().__init__(f"LexerError at {line}:{col}: {message}")

class Lexer:
    def __init__(self, text):
        self.text = text
        self.tokens = []
        self.pos = 0
        self.line = 1
        self.col = 1
        self._tokenize()

    def _tokenize(self):
        while self.pos < len(self.text):
            match = TOKEN_RE.match(self.text, self.pos)
            if not match:
                char = self.text[self.pos]
                raise LexerError(f"Unexpected character: {char!r}", self.line, self.col)

            kind = match.lastgroup
            value = match.group()
            column = self.col

            self._update_position(value)

            if kind in ('WS', 'COMMENT', 'HASH_COMMENT', 'BLOCK_COMMENT'):
                continue

            if kind == 'STRING':
                inner = value[1:-1]
                s = _process_escapes(inner)
                self.tokens.append(Token('STRING', s, self.line, column))
                continue

            if kind == 'IDENT' and value in KEYWORDS:
                if value == 'true':
                    self.tokens.append(Token('BOOL', True, self.line, column))
                elif value == 'false':
                    self.tokens.append(Token('BOOL', False, self.line, column))
                elif value == 'null':
                    self.tokens.append(Token('NULL', None, self.line, column))
                else:
                    self.tokens.append(Token(value.upper(), value, self.line, column))
                continue

            self.tokens.append(Token(kind, value, self.line, column))

        self.tokens.append(Token('EOF', '', self.line, self.col))

    def _update_position(self, text):
        for ch in text:
            if ch == '\n':
                self.line += 1
                self.col = 1
            else:
                self.col += 1
        self.pos += len(text)

    def peek(self):
        return self.tokens[0] if self.tokens else Token('EOF', '', self.line, self.col)

    def next(self):
        return self.tokens.pop(0) if self.tokens else Token('EOF', '', self.line, self.col)

    def expect(self, *types):
        token = self.next()
        if token.type not in types:
            expected = '/'.join(types)
            raise LexerError(
                f"Expected {expected}, got {token.type}({token.value!r})",
                token.line, token.col)
        return token
