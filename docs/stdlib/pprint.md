# pprint — pretty-printing

Pretty-printing in the spirit of Python's `pprint`. Nested lists and dicts
render across lines; dict keys sort for stable output; strings render quoted.

```zen
import pprint

pprint.pp({"b": [1, 2], "a": 1})
# {'a': 1, 'b': [1, 2]}
```

## Functions

| Function | Description |
|----------|-------------|
| `pformat(v)` | Renders a value to a string (short collections stay inline; longer ones expand). |
| `pp(v)` | Prints `pformat(v)`. Named `pp` because a same-named member would shadow the module on import. |

## Example

```zen
import pprint

print pprint.pformat("hi")            # 'hi'
print pprint.pformat([1, 2])          # [1, 2]
print pprint.pformat({"b": 1, "a": 2})  # {'a': 2, 'b': 1}
```
