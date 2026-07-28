import pytest
from zen.lexer import Lexer
from zen.parser import Parser, ParseError
from zen.interpreter import Interpreter, ZenError

def test_lexer_numbers():
    l = Lexer("123 .456 0.67")
    vals = [t.value for t in l.tokens if t.type == 'NUMBER']
    assert vals == ['123', '.456', '0.67']

def test_soft_keyword_with():
    code = "with x { print x }"
    l = Lexer(code)
    p = Parser(l)
    ast = p.parse()
    assert ast is not None

def test_prose_fallback():
    code = "this language has a weakness with dot"
    l = Lexer(code)
    p = Parser(l)
    ast = p.parse()
    interp = Interpreter(browser=None)
    with pytest.raises(ZenError):
        interp.interpret(ast)

def test_interpreter_basic():
    code = "let x = 10 + 20\nprint x"
    l = Lexer(code)
    p = Parser(l)
    interp = Interpreter(browser=None)
    ast = p.parse()
    interp.interpret(ast)



