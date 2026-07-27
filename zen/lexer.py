import re
from collections import deque

class Token:
    __slots__ = ('type', 'value', 'line', 'col')
    def __init__(self, type, value, line, col):
        self.type = type
        self.value = value
        self.line = line
        self.col = col

    def __repr__(self):
        return f"Token({self.type}, {self.value!r}, L{self.line}:{self.col})"

KEYWORDS = {
    'let', 'go', 'fill', 'with', 'click', 'wait', 'for', 'in',
    'if', 'elif', 'else', 'while', 'function', 'def', 'return', 'print', 'input',
    'into', 'scroll', 'to', 'by', 'shot', 'full', 'refresh',
    'back', 'forward', 'execute', 'download', 'and', 'or', 'not',
    'true', 'false', 'null', 'try', 'catch', 'top', 'bottom',
    'break', 'continue', 'include', 'import', 'require', 'is', 'finally',
    'class', 'extends', 'new', 'self', 'switch', 'case', 'default', 'as',
    'load', 'use',
}

TOKEN_SPEC = [
    ('WS',       r'[ \t\r]+'),
    ('NEWLINE',  r'\n'),
    ('IDENT',    r'[a-zA-Z_][a-zA-Z0-9_]*'),
    ('NUMBER',   r'\d+(?:\.\d+)?'),
    ('STRING',   r'"""(?:(?!""")[\s\S])*"""|\'\'\'(?:(?!\'\'\')[\s\S])*\'\'\'|"[^"\\]*(?:\\.[^"\\]*)*"|\'[^\'\\]*(?:\\.[^\'\\]*)*\''),
    ('COMMENT',  r'//[^\n]*'),
    ('HASH_COMMENT', r'#[^\n]*'),
    ('BLOCK_COMMENT', r'/\*[\s\S]*?\*/'),
    ('ELLIPSIS', r'\.\.\.'),
    ('DOT_DOT',  r'\.\.'),
    ('SAFE_DOT', r'\?\.'),
    ('INC',      r'\+\+'),
    ('DEC',      r'--'),
    ('POW',      r'\*\*'),
    ('PLUS_ASSIGN',  r'\+\='),
    ('MINUS_ASSIGN', r'-='),
    ('STAR_ASSIGN',  r'\*='),
    ('SLASH_ASSIGN', r'/='),
    ('MOD_ASSIGN',   r'%='),
    ('PIPE_PIPE',  r'\|\|'),
    ('AMPERSAND_AMPERSAND', r'&&'),
    ('EQ',       r'=='),
    ('NEQ',      r'!='),
    ('LE',       r'<='),
    ('GE',       r'>='),
    ('RARROW',   r'->'),
    ('LPAREN',   r'\('),
    ('RPAREN',   r'\)'),
    ('LBRACE',   r'\{'),
    ('RBRACE',   r'\}'),
    ('LBRACKET', r'\['),
    ('RBRACKET', r'\]'),
    ('SEMICOLON', r';'),
    ('COMMA',    r','),
    ('AT',       r'@'),
    ('DOT',      r'\.'),
    ('COLON',    r':'),
    ('ASSIGN',   r'='),
    ('LT',       r'<'),
    ('GT',       r'>'),
    ('PLUS',     r'\+'),
    ('MINUS',    r'-'),
    ('STAR',     r'\*'),
    ('SLASH',    r'/'),
    ('MOD',      r'%'),
    ('BANG',     r'!'),
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
            if e == 'x' and i + 3 < len(s):
                result.append(chr(int(s[i+2:i+4], 16)))
                i += 4
            elif e == 'u' and i + 5 < len(s):
                result.append(chr(int(s[i+2:i+6], 16)))
                i += 6
            elif e == 'U' and i + 9 < len(s):
                result.append(chr(int(s[i+2:i+10], 16)))
                i += 10
            else:
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
        self.tokens = deque()
        self.pos = 0
        self.line = 1
        self.col = 1
        self._tokenize()

    def _tokenize(self):
        line = self.line
        column = self.col
        while self.pos < len(self.text):
            match = TOKEN_RE.match(self.text, self.pos)
            if not match:
                char = self.text[self.pos]
                raise LexerError(f"Unexpected character: {char!r}", self.line, self.col)

            kind = match.lastgroup
            value = match.group()
            line, column = self.line, self.col

            self._update_position(value)

            if kind in ('WS', 'COMMENT', 'HASH_COMMENT', 'BLOCK_COMMENT'):
                continue

            if kind == 'STRING':
                if value.startswith('"""') or value.startswith("'''"):
                    inner = value[3:-3]
                else:
                    inner = value[1:-1]
                s = _process_escapes(inner)
                self.tokens.append(Token('STRING', s, line, column))
                continue

            if kind == 'IDENT' and value in KEYWORDS:
                if value == 'true':
                    self.tokens.append(Token('BOOL', True, line, column))
                elif value == 'false':
                    self.tokens.append(Token('BOOL', False, line, column))
                elif value == 'null':
                    self.tokens.append(Token('NULL', None, line, column))
                else:
                    self.tokens.append(Token(value.upper(), value, line, column))
                continue

            self.tokens.append(Token(kind, value, line, column))

        self.tokens.append(Token('EOF', '', line, column))

    def _update_position(self, text):
        nl = text.count('\n')
        if nl:
            self.line += nl
            self.col = len(text) - text.rfind('\n')
        else:
            self.col += len(text)
        self.pos += len(text)

    def peek(self):
        return self.tokens[0] if self.tokens else Token('EOF', '', self.line, self.col)

    def next(self):
        return self.tokens.popleft() if self.tokens else Token('EOF', '', self.line, self.col)

    def expect(self, *types):
        token = self.next()
        if token.type not in types:
            expected = '/'.join(types)
            raise LexerError(
                f"Expected {expected}, got {token.type}({token.value!r})",
                token.line, token.col)
        return token
