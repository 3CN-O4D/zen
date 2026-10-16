# codec — text transforms

CTF/recon text transforms: `rot13`, `rot47`, repeating-key `xor`, JWT
decoding (structure only — signatures are never verified here), and stepwise
`pipe` chains. Pure offline, pure Zen.

```zen
import codec

print codec.rot13("hello")              # uryyb
print codec.pipe(data, ["b64d", "rot13"])
```

## Functions

| Function | Description |
|----------|-------------|
| `rot13(s)` | ROT-13 letters; other characters untouched. |
| `rot47(s)` | ROT-47 over printable ASCII; other characters untouched. |
| `xor(data, key)` | Repeating-key XOR; `key` is a string or a single byte. |
| `jwt_decode(token)` | `{header_text, payload_text, signature, header, payload}`; `header`/`payload` are parsed dicts or `null`. Throws unless the token has three segments. |
| `pipe(data, steps)` | Applies steps in order: `"rot13"`, `"rot47"`, `"b64e"`, `"b64d"`, `"hex"`, `"unhex"`. Unknown steps throw. |

Only test systems you own or are authorized to audit.
