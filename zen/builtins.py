import time as _time
from collections import OrderedDict
import os
import json as _json
import urllib.request as _urllib
import random as _random
import math as _math
import datetime as _datetime
import socket as _socket
import base64 as _base64
import hashlib as _hashlib
import hmac as _hmac
import threading as _threading
import queue as _queue
from .environment import ZenElement, ZenSelector, ZenRegexMatch, HttpResponse, ZenError, PageModule, ConfigModule
from .browser import get_config, set_config


def _os_popen(cmd, args):
    import subprocess
    proc = subprocess.Popen([cmd] + list(args), stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    return {
        'stdin': proc.stdin,
        'stdout': proc.stdout,
        'stderr': proc.stderr,
        'pid': proc.pid,
        'poll': lambda: proc.poll(),
        'wait': lambda: proc.wait(),
        'kill': lambda: proc.kill(),
    }


def _to_bytes(data):
    if isinstance(data, bytes):
        return data
    if isinstance(data, str):
        return data.encode('utf-8')
    if isinstance(data, list):
        return bytes(data)
    return str(data).encode('utf-8')


def _typed_input(prompt, expected_type=None):
    raw = input(prompt)
    if expected_type is None:
        return raw
    t = str(expected_type).strip().lower()
    try:
        if t in ('str', 'string'):
            return raw
        if t in ('int', 'integer'):
            return int(raw)
        if t in ('float', 'real', 'double'):
            return float(raw)
        if t == 'bool':
            low = raw.strip().lower()
            if low in ('true', 'yes', 'y', '1'):
                return True
            if low in ('false', 'no', 'n', '0'):
                return False
            raise ValueError(f"cannot convert {raw!r} to bool")
        if t == 'list':
            import json as _json
            return _json.loads(raw)
        if t in ('dict', 'map', 'object'):
            import json as _json
            return _json.loads(raw)
        # fallback: try converting via Python's type name
        import builtins as _builtins
        converter = getattr(_builtins, t, None)
        if converter is not None and callable(converter):
            return converter(raw)
        raise ValueError(f"unknown type: {expected_type!r}")
    except Exception as e:
        from .environment import ZenError
        raise ZenError(f"Failed to convert input to {t}: {e}")

def _assert_fn(cond, msg=None):
    from .environment import ZenError
    if not cond:
        raise ZenError(msg or 'Assertion failed')
    return True

def _assert_eq_fn(a, b, msg=None):
    from .environment import ZenError
    if a != b:
        raise ZenError(msg or f'Assertion failed: {a!r} != {b!r}')
    return True

def _resolve_selector(sel, **kwargs):
    if kwargs.get('text'):
        text_val = kwargs['text']
        return f'text={"".join(text_val) if isinstance(text_val, list) else text_val}'
    if kwargs.get('url'):
        return f'css=[href="{kwargs["url"]}"]'
    if isinstance(sel, str) and sel.strip():
        s = sel.strip()
        if _looks_like_plain_text(s):
            return f'text={s}'
        return s
    if isinstance(sel, ZenElement):
        return sel
    return sel

def _looks_like_plain_text(s):
    if ' ' not in s:
        return False
    for ch in ('#', '.', ':', '>', '+', '~', '[', ']', '@', '*'):
        if ch in s:
            return False
    return True

def _resolve_find(browser, mode, *args, **kwargs):
    sel = args[0] if args else None
    resolved = _resolve_selector(sel, **kwargs)
    exact = kwargs.get('exact', False)
    if isinstance(resolved, str) and resolved.startswith('text='):
        text_val = resolved[5:]
        if exact:
            resolved = f'text={text_val}'
        if mode == 'all':
            return browser.find(resolved)
        elif mode == 'nth':
            n = int(kwargs.get('n', 0))
            return browser.find_nth(resolved, n)
        return browser.find_first(resolved)
    if mode == 'all':
        return browser.find(str(resolved))
    elif mode == 'nth':
        n = int(kwargs.get('n', 0))
        return browser.find_nth(str(resolved), n)
    return browser.find_first(str(resolved))

def _smart_click(browser, *args, **kwargs):
    if not args and not kwargs:
        browser.click(None)
        return True
    sel = args[0] if args else None
    if hasattr(sel, '_locator'):
        sel.click()
        return True
    resolved = _resolve_selector(sel, **kwargs)
    browser.click(str(resolved))
    return True

def _smart_fill(browser, *args, **kwargs):
    if kwargs.get('with'):
        val = kwargs['with']
        sel = args[0] if args else None
        resolved = _resolve_selector(sel, **kwargs)
        browser.fill(str(resolved), str(val))
        return True
    elif len(args) >= 2:
        browser.fill(str(args[0]), str(args[1]))
        return True
    elif kwargs:
        for k, v in kwargs.items():
            if k != 'text' and k != 'exact':
                sel = _resolve_selector(args[0] if args else None, **kwargs)
                browser.fill(str(sel), str(v))
                return True
    raise ZenError('fill(selector, value) requires a selector and a value')

def _smart_wait(browser, *args, **kwargs):
    sel = args[0] if args else None
    resolved = _resolve_selector(sel, **kwargs)
    return browser.wait_for(str(resolved))

def _smart_check(browser, *args):
    if args and hasattr(args[0], '_locator'):
        return args[0].check()
    sel = str(args[0]) if args else None
    return browser.find_first(sel).check()

def _smart_uncheck(browser, *args):
    if args and hasattr(args[0], '_locator'):
        return args[0].uncheck()
    sel = str(args[0]) if args else None
    return browser.find_first(sel).uncheck()

def _smart_select(browser, sel, val):
    if hasattr(sel, '_locator'):
        return sel.select(val)
    return browser.find_first(str(sel)).select(str(val))

def _aes_encrypt(key, data, iv=None):
    from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
    from cryptography.hazmat.primitives import padding
    key = _to_bytes(key)
    data = _to_bytes(data)
    if iv is not None:
        iv = _to_bytes(iv)
    else:
        iv = os.urandom(16)
    key = key[:32].ljust(32, b'\0')
    padder = padding.PKCS7(128).padder()
    padded = padder.update(data) + padder.finalize()
    cipher = Cipher(algorithms.AES(key), modes.CBC(iv))
    encryptor = cipher.encryptor()
    ct = encryptor.update(padded) + encryptor.finalize()
    return (iv + ct).hex()

def _aes_decrypt(key, data, iv=None):
    from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
    from cryptography.hazmat.primitives import padding
    key = _to_bytes(key)
    key = key[:32].ljust(32, b'\0')
    raw = bytes.fromhex(str(data))
    if iv is not None:
        iv = _to_bytes(iv)
        ct = raw
    else:
        iv = raw[:16]
        ct = raw[16:]
    cipher = Cipher(algorithms.AES(key), modes.CBC(iv))
    decryptor = cipher.decryptor()
    padded = decryptor.update(ct) + decryptor.finalize()
    unpadder = padding.PKCS7(128).unpadder()
    return (unpadder.update(padded) + unpadder.finalize()).decode('utf-8')

def _build_cryptography_module():
    mod = {}
    try:
        from cryptography.fernet import Fernet
        mod['fernet'] = {
            'generate_key': lambda: Fernet.generate_key().decode(),
            'encrypt': lambda key, data: Fernet(str(key).encode() if isinstance(key, str) else key).encrypt(_to_bytes(data)).decode(),
            'decrypt': lambda key, token: Fernet(str(key).encode() if isinstance(key, str) else key).decrypt(str(token).encode() if isinstance(token, str) else token).decode(),
        }
    except ImportError:
        mod['fernet'] = lambda: _raise_err("cryptography package not installed: pip install cryptography")
    return mod

def _raise_err(msg):
    raise ZenError(msg)

def _json_loads_file(path):
    with open(os.path.expanduser(str(path)), 'r', encoding='utf-8') as f:
        return _json.load(f)

def _json_save_file(path, val):
    path = os.path.expanduser(str(path))
    d = os.path.dirname(path)
    if d and not os.path.exists(d):
        os.makedirs(d, exist_ok=True)
    with open(path, 'w', encoding='utf-8') as f:
        _json.dump(val, f, ensure_ascii=False, indent=2)
    return True

def _statistics_mean(data):
    import statistics
    return statistics.mean(data)

def _statistics_median(data):
    import statistics
    return statistics.median(data)

def _statistics_mode(data):
    import statistics
    return statistics.mode(data)

def _statistics_stdev(data):
    import statistics
    return statistics.stdev(data)

def _statistics_variance(data):
    import statistics
    return statistics.variance(data)

def _build_decimal(val):
    from decimal import Decimal
    return Decimal(str(val))

def _decimal_getcontext():
    import decimal
    ctx = decimal.getcontext()
    return {
        'prec': ctx.prec,
        'rounding': str(ctx.rounding),
        'Emin': ctx.Emin,
        'Emax': ctx.Emax,
        'capitals': ctx.capitals,
        'clamp': ctx.clamp,
    }

def _decimal_setcontext(ctx):
    import decimal
    dc = decimal.getcontext()
    if isinstance(ctx, dict):
        dc.prec = int(ctx.get('prec', dc.prec))
        dc.Emin = int(ctx.get('Emin', dc.Emin))
        dc.Emax = int(ctx.get('Emax', dc.Emax))
    return True

def _decimal_localcontext(ctx=None, **kw):
    import decimal
    return decimal.localcontext(ctx, **kw) if ctx else decimal.localcontext()

def _emoji_show(emoji_map, query=None):
    lines = []
    for name in sorted(emoji_map.keys()):
        if query is None or str(query).lower() in name.lower():
            lines.append(f'{emoji_map[name]}  {name}')
    for line in lines:
        print(line)
    print(f'\n{len(lines)} emoji(s) shown')

class _Categories:
    def __init__(self, data, cat_map):
        object.__setattr__(self, '__data', data)
        object.__setattr__(self, '__cat_map', cat_map)
        object.__setattr__(self, '__names', list(cat_map.keys()))

    def __getattr__(self, name):
        if name.startswith('_'):
            raise AttributeError(name)
        d = object.__getattribute__(self, '__data')
        m = object.__getattribute__(self, '__cat_map')
        if name in m:
            return {k: d[k] for k in m[name] if k in d}
        raise AttributeError(name)

    def __getitem__(self, idx):
        m = object.__getattribute__(self, '__cat_map')
        n = object.__getattribute__(self, '__names')
        name = n[idx]
        d = object.__getattribute__(self, '__data')
        return {k: d[k] for k in m[name] if k in d}

    def __len__(self):
        return len(object.__getattribute__(self, '__names'))

    def __iter__(self):
        return iter(object.__getattribute__(self, '__names'))

    def __repr__(self):
        return f'Categories({", ".join(object.__getattribute__(self, "__names"))})'


class _EmojiModule:
    def __init__(self, data, by_name, by_code, codes, names_list, search_fn, show_fn, cat):
        object.__setattr__(self, '__data', data)
        object.__setattr__(self, '__by_name', by_name)
        object.__setattr__(self, '__by_code', by_code)
        object.__setattr__(self, '__codes', codes)
        object.__setattr__(self, '__names_list', names_list)
        object.__setattr__(self, '__search', search_fn)
        object.__setattr__(self, '__show', show_fn)
        object.__setattr__(self, '__cat', cat)

    def __getitem__(self, idx):
        data = object.__getattribute__(self, '__data')
        if isinstance(idx, str):
            return data.get(idx)
        if not isinstance(idx, int):
            raise TypeError(f'emoji index must be an integer or string, not {type(idx).__name__}')
        names = object.__getattribute__(self, '__names_list')
        if 1 <= idx <= len(names):
            return data[names[idx - 1]]
        raise IndexError(f'emoji index {idx} out of range (1-{len(names)})')

    def __getattr__(self, name):
        if name.startswith('_'):
            raise AttributeError(name)
        if name == 'cat':
            return object.__getattribute__(self, '__cat')
        if name == 'categories':
            return list(object.__getattribute__(self, '__cat'))
        data = object.__getattribute__(self, '__data')
        if name in data:
            return data[name]
        if name == 'by_name':
            return object.__getattribute__(self, '__by_name')
        if name == 'by_code':
            return object.__getattribute__(self, '__by_code')
        if name == 'codes':
            return object.__getattribute__(self, '__codes')
        if name == 'names':
            return lambda: list(object.__getattribute__(self, '__names_list'))
        if name == 'search':
            return object.__getattribute__(self, '__search')
        if name == 'show':
            return object.__getattribute__(self, '__show')
        if name == 'show_all':
            return lambda: object.__getattribute__(self, '__show')(None)
        matches = {k: v for k, v in data.items() if name in k}
        return matches

    def __setattr__(self, name, val):
        raise TypeError('Emoji module is read-only')

def _build_emoji_module():
    _EMOJI_MAP = {
        # Smileys & Emotion
        'grin': '\U0001F600', 'smiley': '\U0001F603', 'smile': '\U0001F604',
        'sweat_smile': '\U0001F605', 'laughing': '\U0001F606', 'joy': '\U0001F602',
        'rofl': '\U0001F923', 'happy': '\U0001F601', 'wink': '\U0001F609',
        'blush': '\U0001F60A', 'innocent': '\U0001F607', 'heart_eyes': '\U0001F60D',
        'kissing_heart': '\U0001F618', 'kissing': '\U0001F617', 'yum': '\U0001F60B',
        'stuck_out_tongue': '\U0001F61B', 'stuck_out_tongue_wink': '\U0001F61C',
        'stuck_out_tongue_closed_eyes': '\U0001F61D', 'money_mouth': '\U0001F911',
        'hug': '\U0001F917', 'smirk': '\U0001F60F', 'no_mouth': '\U0001F636',
        'neutral': '\U0001F610', 'expressionless': '\U0001F611',
        'unamused': '\U0001F612', 'rolling_eyes': '\U0001F644',
        'thinking': '\U0001F914', 'flushed': '\U0001F633',
        'disappointed': '\U0001F61E', 'worried': '\U0001F61F',
        'angry': '\U0001F620', 'rage': '\U0001F621',
        'pensive': '\U0001F614', 'confused': '\U0001F615',
        'slight_frown': '\U0001F641', 'frowning': '\U0001F626',
        'persevere': '\U0001F623', 'confounded': '\U0001F616',
        'tired': '\U0001F62B', 'weary': '\U0001F629',
        'triumph': '\U0001F624', 'sob': '\U0001F62D', 'cry': '\U0001F622',
        'scream': '\U0001F631', 'fearful': '\U0001F628', 'cold_sweat': '\U0001F630',
        'sleepy': '\U0001F62A', 'sleeping': '\U0001F634', 'dizzy': '\U0001F635',
        'astonished': '\U0001F632', 'zipper_mouth': '\U0001F910',
        'mask': '\U0001F637', 'thermometer': '\U0001F912',
        'sick': '\U0001F922', 'nauseated': '\U0001F922',
        'sneeze': '\U0001F927', 'clown': '\U0001F921',
        'poop': '\U0001F4A9', 'shit': '\U0001F4A9',
        'skull': '\U0001F480', 'alien': '\U0001F47D',
        'robot': '\U0001F916', 'ghost': '\U0001F47B',
        'angel': '\U0001F47C', 'devil': '\U0001F47F',
        'imp': '\U0001F47F', 'ogre': '\U0001F479', 'goblin': '\U0001F47A',

        # Heart symbols
        'heart': '\u2764\uFE0F', 'red_heart': '\u2764\uFE0F',
        'orange_heart': '\U0001F9E1', 'yellow_heart': '\U0001F49B',
        'green_heart': '\U0001F49A', 'blue_heart': '\U0001F499',
        'purple_heart': '\U0001F49C', 'black_heart': '\U0001F5A4',
        'broken_heart': '\U0001F494', 'heart_exclamation': '\u2763\uFE0F',
        'two_hearts': '\U0001F495', 'revolving_hearts': '\U0001F49E',
        'heartbeat': '\U0001F493', 'heartpulse': '\U0001F497',
        'sparkling_heart': '\U0001F496', 'cupid': '\U0001F498',
        'gift_heart': '\U0001F49D', 'love_letter': '\U0001F48C',

        # Gestures & People
        'wave': '\U0001F44B', 'raised_hand': '\u270B',
        'ok_hand': '\U0001F44C', 'thumbsup': '\U0001F44D',
        'thumbsdown': '\U0001F44E', 'clap': '\U0001F44F',
        'open_hands': '\U0001F450', 'pray': '\U0001F64F',
        'handshake': '\U0001F91D', 'muscle': '\U0001F4AA',
        'point_up': '\u261D\uFE0F', 'point_down': '\U0001F447',
        'point_left': '\U0001F448', 'point_right': '\U0001F449',
        'fist': '\u270A', 'facepunch': '\U0001F44A',
        'middle_finger': '\U0001F595', 'fingers_crossed': '\U0001F91E',
        'v': '\u270C\uFE0F', 'peace': '\u270C\uFE0F',
        'crossed_fingers': '\U0001F91E', 'call_me': '\U0001F919',
        'writing_hand': '\u270D\uFE0F', 'nail_care': '\U0001F485',
        'selfie': '\U0001F933', 'flexed_biceps': '\U0001F4AA',

        # Body
        'eyes': '\U0001F440', 'eye': '\U0001F441\uFE0F',
        'ear': '\U0001F442', 'nose': '\U0001F443',
        'mouth': '\U0001F444', 'tongue': '\U0001F445',
        'lips': '\U0001F48B', 'kiss': '\U0001F48B',
        'bone': '\U0001F9B4', 'anatomy': '\U0001FAC0',

        # People
        'person': '\U0001F9D1', 'man': '\U0001F468', 'woman': '\U0001F469',
        'girl': '\U0001F467', 'boy': '\U0001F466', 'baby': '\U0001F476',
        'old_man': '\U0001F474', 'old_woman': '\U0001F475',
        'person_blond_hair': '\U0001F471', 'person_red_hair': '\U0001F9D1\u200D\U0001F9B0',
        'person_curly_hair': '\U0001F9D1\u200D\U0001F9B1',
        'person_white_hair': '\U0001F9D1\u200D\U0001F9B3',
        'person_bald': '\U0001F9D1\u200D\U0001F9B2',
        'person_beard': '\U0001F9D4',
        'woman_with_headscarf': '\U0001F9D5',
        'person_in_tuxedo': '\U0001F935', 'bride_with_veil': '\U0001F470',
        'pregnant_woman': '\U0001F930', 'breastfeeding': '\U0001F931',

        # Fantasy
        'fairy': '\U0001F9DA', 'vampire': '\U0001F9DB',
        'merperson': '\U0001F9DC', 'elf': '\U0001F9DD',
        'genie': '\U0001F9DE', 'zombie': '\U0001F9DF',

        # Activity
        'dance': '\U0001F483', 'dancer': '\U0001F483',
        'man_dancing': '\U0001F57A', 'person_walking': '\U0001F6B6',
        'person_running': '\U0001F3C3', 'standing': '\U0001F9CD',
        'kneeling': '\U0001F9CE', 'person_with_probing_cane': '\U0001F9D1\u200D\U0001F9AF',
        'person_in_motorized_wheelchair': '\U0001F9D1\u200D\U0001F9BC',

        # Clothing
        'glasses': '\U0001F453', 'sunglasses': '\U0001F576\uFE0F',
        'necktie': '\U0001F454', 'shirt': '\U0001F455',
        'jeans': '\U0001F456', 'dress': '\U0001F457',
        'bikini': '\U0001F459', 'kimono': '\U0001F458',
        'sari': '\U0001F97B', 'lab_coat': '\U0001F97C',
        'graduation_cap': '\U0001F393', 'crown': '\U0001F451',
        'hat': '\U0001F452', 'tophat': '\U0001F3A9',
        'military_helmet': '\U0001FA96', 'helmet': '\U0001FA96',

        # Animals
        'dog': '\U0001F436', 'cat': '\U0001F431', 'mouse': '\U0001F42D',
        'hamster': '\U0001F439', 'rabbit': '\U0001F430', 'fox': '\U0001F98A',
        'bear': '\U0001F43B', 'panda': '\U0001F43C', 'koala': '\U0001F428',
        'tiger': '\U0001F42F', 'lion': '\U0001F981', 'cow': '\U0001F42E',
        'pig': '\U0001F437', 'frog': '\U0001F438', 'monkey': '\U0001F435',
        'monkey_face': '\U0001F412', 'chicken': '\U0001F414', 'penguin': '\U0001F427',
        'bird': '\U0001F426', 'eagle': '\U0001F985', 'duck': '\U0001F986',
        'owl': '\U0001F989', 'bat': '\U0001F987', 'wolf': '\U0001F43A',
        'horse': '\U0001F434', 'unicorn': '\U0001F984', 'zebra': '\U0001F993',
        'deer': '\U0001F98C', 'cow_face': '\U0001F404',
        'snake': '\U0001F40D', 'dragon': '\U0001F409', 'dragon_face': '\U0001F432',
        'dinosaur': '\U0001F996', 'whale': '\U0001F433', 'dolphin': '\U0001F42C',
        'fish': '\U0001F41F', 'tropical_fish': '\U0001F420', 'blowfish': '\U0001F421',
        'shark': '\U0001F988', 'octopus': '\U0001F419', 'shell': '\U0001F41A',
        'snail': '\U0001F40C', 'butterfly': '\U0001F98B', 'bug': '\U0001F41B',
        'ant': '\U0001F41C', 'bee': '\U0001F41D', 'ladybug': '\U0001F41E',
        'cricket': '\U0001F997', 'spider': '\U0001F577\uFE0F', 'scorpion': '\U0001F982',
        'mosquito': '\U0001F99F', 'microbe': '\U0001F9A0',

        # Plants
        'seedling': '\U0001F331', 'tree': '\U0001F333', 'palm_tree': '\U0001F334',
        'cactus': '\U0001F335', 'flower': '\U0001F337', 'rose': '\U0001F339',
        'wilted_rose': '\U0001F940', 'hibiscus': '\U0001F33A', 'cherry_blossom': '\U0001F338',
        'blossom': '\U0001F33C', 'sunflower': '\U0001F33B', 'bouquet': '\U0001F490',
        'mushroom': '\U0001F344', 'leaf': '\U0001F342', 'maple_leaf': '\U0001F341',
        'four_leaf_clover': '\U0001F340', 'shamrock': '\u2618\uFE0F',

        # Food & Drink
        'apple': '\U0001F34E', 'green_apple': '\U0001F34F',
        'banana': '\U0001F34C', 'orange': '\U0001F34A', 'lemon': '\U0001F34B',
        'watermelon': '\U0001F349', 'grapes': '\U0001F347', 'strawberry': '\U0001F353',
        'cherries': '\U0001F352', 'peach': '\U0001F351', 'mango': '\U0001F96D',
        'pineapple': '\U0001F34D', 'coconut': '\U0001F965', 'kiwi': '\U0001F95D',
        'tomato': '\U0001F345', 'eggplant': '\U0001F346', 'avocado': '\U0001F951',
        'broccoli': '\U0001F966', 'corn': '\U0001F33D', 'bread': '\U0001F35E',
        'croissant': '\U0001F950', 'pizza': '\U0001F355', 'hamburger': '\U0001F354',
        'fries': '\U0001F35F', 'hotdog': '\U0001F32D', 'sandwich': '\U0001F96A',
        'taco': '\U0001F32E', 'burrito': '\U0001F32F', 'rice': '\U0001F35A',
        'ramen': '\U0001F35C', 'spaghetti': '\U0001F35D', 'sushi': '\U0001F363',
        'dumpling': '\U0001F95F', 'cooking': '\U0001F373', 'egg': '\U0001F95A',
        'pancakes': '\U0001F95E', 'waffle': '\U0001F9C7', 'bacon': '\U0001F953',
        'donut': '\U0001F369', 'cake': '\U0001F370', 'cupcake': '\U0001F9C1',
        'cookie': '\U0001F36A', 'chocolate': '\U0001F36B', 'candy': '\U0001F36C',
        'ice_cream': '\U0001F368', 'coffee': '\u2615', 'tea': '\U0001F375',
        'beer': '\U0001F37A', 'wine': '\U0001F377', 'cocktail': '\U0001F378',
        'soda': '\U0001F964', 'juice': '\U0001F9C3', 'milk': '\U0001F95B',
        'honey': '\U0001F36F', 'popcorn': '\U0001F37F', 'champagne': '\U0001F37E',

        # Travel & Places
        'earth': '\U0001F30E', 'globe': '\U0001F30D', 'moon': '\U0001F319',
        'sun': '\u2600\uFE0F', 'star': '\u2B50', 'shooting_star': '\U0001F320',
        'comet': '\u2604\uFE0F', 'cloud': '\u2601\uFE0F', 'rainbow': '\U0001F308',
        'rain': '\U0001F327\uFE0F', 'thunder': '\U0001F329\uFE0F', 'snow': '\U0001F328\uFE0F',
        'fire': '\U0001F525', 'tornado': '\U0001F32A\uFE0F', 'wind': '\U0001F32C\uFE0F',
        'house': '\U0001F3E0', 'hut': '\U0001F3E1', 'office': '\U0001F3E2',
        'post_office': '\U0001F3E3', 'hospital': '\U0001F3E5', 'bank': '\U0001F3E6',
        'school': '\U0001F3EB', 'castle': '\U0001F3F0', 'church': '\u26EA',
        'mosque': '\U0001F54C', 'synagogue': '\U0001F54D', 'temple': '\U0001F54B',
        'japan': '\U0001F5FE', 'mountain': '\U0001F3D4\uFE0F', 'volcano': '\U0001F30B',
        'beach': '\U0001F3D6\uFE0F', 'desert': '\U0001F3DC\uFE0F',
        'island': '\U0001F3DD\uFE0F', 'park': '\U0001F3DE\uFE0F',
        'car': '\U0001F697', 'taxi': '\U0001F695', 'bus': '\U0001F68C',
        'train': '\U0001F68B', 'airplane': '\u2708\uFE0F', 'helicopter': '\U0001F681',
        'rocket': '\U0001F680', 'satellite': '\U0001F6F0\uFE0F',
        'ship': '\U0001F6A2', 'boat': '\u26F5', 'anchor': '\u2693',
        'bicycle': '\U0001F6B2', 'motorcycle': '\U0001F3CD\uFE0F',
        'fuel_pump': '\u26FD', 'traffic_light': '\U0001F6A6',
        'railway_track': '\U0001F6E4\uFE0F',

        # Objects
        'phone': '\U0001F4F1', 'mobile': '\U0001F4F1',
        'computer': '\U0001F4BB', 'keyboard': '\u2328\uFE0F',
        'printer': '\U0001F5A8\uFE0F', 'mouse_computer': '\U0001F5B1\uFE0F',
        'tv': '\U0001F4FA', 'radio': '\U0001F4FB', 'speaker': '\U0001F50A',
        'headphone': '\U0001F50A', 'camera': '\U0001F4F7', 'video_camera': '\U0001F4F9',
        'movie_camera': '\U0001F3A5', 'film': '\U0001F39E\uFE0F',
        'projector': '\U0001F4FD\uFE0F', 'book': '\U0001F4D6',
        'books': '\U0001F4DA', 'notebook': '\U0001F4D3', 'newspaper': '\U0001F4F0',
        'money_bag': '\U0001F4B0', 'dollar': '\U0001F4B5', 'euro': '\U0001F4B6',
        'pound': '\U0001F4B7', 'yen': '\U0001F4B4', 'credit_card': '\U0001F4B3',
        'chart': '\U0001F4CA', 'email': '\u2709\uFE0F', 'inbox': '\U0001F4E5',
        'outbox': '\U0001F4E4', 'package': '\U0001F4E6', 'door': '\U0001F6AA',
        'bed': '\U0001F6CF\uFE0F', 'toilet': '\U0001F6BD', 'shower': '\U0001F6BF',
        'bathtub': '\U0001F6C1', 'key': '\U0001F511', 'lock': '\U0001F512',
        'unlock': '\U0001F513', 'bell': '\U0001F514', 'bulb': '\U0001F4A1',
        'flashlight': '\U0001F526', 'wrench': '\U0001F527', 'hammer': '\U0001F528',
        'pick': '\u26CF\uFE0F', 'nut_bolt': '\U0001F529', 'gear': '\u2699\uFE0F',
        'alarm': '\u23F0', 'clock': '\U0001F570\uFE0F', 'hourglass': '\u231B',
        'microscope': '\U0001F52C', 'telescope': '\U0001F52D',
        'syringe': '\U0001F489', 'pill': '\U0001F48A',
        'adhesive_bandage': '\U0001FA79', 'stethoscope': '\U0001FA7A',
        'broom': '\U0001F9F9', 'basket': '\U0001F9FA', 'toilet_paper': '\U0001F9FB',
        'soap': '\U0001F9FC', 'sponge': '\U0001F9FD', 'fire_extinguisher': '\U0001F9EF',
        'shopping_cart': '\U0001F6D2',

        # Symbols
        '100': '\U0001F4AF', 'hundred': '\U0001F4AF',
        '1234': '\U0001F522', 'abc': '\U0001F524',
        'ab': '\U0001F18E', 'cl': '\U0001F191', 'cool': '\U0001F192',
        'free': '\U0001F193', 'id': '\U0001F194', 'new': '\U0001F195',
        'ng': '\U0001F196', 'ok': '\U0001F197', 'sos': '\U0001F198',
        'up': '\U0001F199', 'vs': '\U0001F19A',
        'check': '\u2705', 'cross': '\u274C', 'x': '\u274C',
        'exclamation': '\u2757', 'question': '\u2753',
        'white_check_mark': '\u2705', 'heart_exclamation': '\u2755',
        'warning': '\u26A0\uFE0F', 'no_entry': '\u26D4',
        'prohibited': '\U0001F6AB', 'radioactive': '\u2622\uFE0F',
        'biohazard': '\u2623\uFE0F', 'skull_crossbones': '\u2620\uFE0F',
        'infinity': '\u267E\uFE0F', 'recycling': '\u267B\uFE0F',
        'fleur_de_lis': '\u269C\uFE0F', 'trident': '\U0001F531',
        'name_badge': '\U0001F4DB', 'beginner': '\U0001F530',
        'tm': '\u2122\uFE0F', 'copyright': '\u00A9\uFE0F', 'registered': '\u00AE\uFE0F',
        'at': '\U0001F51F', 'hash': '#\uFE0F\u20E3', 'keycap_star': '*\uFE0F\u20E3',
        'zero': '0\uFE0F\u20E3', 'one': '1\uFE0F\u20E3', 'two': '2\uFE0F\u20E3',
        'three': '3\uFE0F\u20E3', 'four': '4\uFE0F\u20E3', 'five': '5\uFE0F\u20E3',
        'six': '6\uFE0F\u20E3', 'seven': '7\uFE0F\u20E3', 'eight': '8\uFE0F\u20E3',
        'nine': '9\uFE0F\u20E3', 'ten': '\U0001F51F',

        # Arrows
        'right_arrow': '\u27A1\uFE0F', 'left_arrow': '\u2B05\uFE0F',
        'up_arrow': '\u2B06\uFE0F', 'down_arrow': '\u2B07\uFE0F',
        'back': '\U0001F519', 'end': '\U0001F51A', 'soon': '\U0001F51C',
        'top': '\U0001F51D',

        # Zodiac
        'aries': '\u2648\uFE0F', 'taurus': '\u2649\uFE0F', 'gemini': '\u264A\uFE0F',
        'cancer': '\u264B\uFE0F', 'leo': '\u264C\uFE0F', 'virgo': '\u264D\uFE0F',
        'libra': '\u264E\uFE0F', 'scorpius': '\u264F\uFE0F', 'sagittarius': '\u2650\uFE0F',
        'capricorn': '\u2651\uFE0F', 'aquarius': '\u2652\uFE0F', 'pisces': '\u2653\uFE0F',
        'ophiuchus': '\u26CE',

        # Activities & Sports
        'soccer': '\u26BD', 'basketball': '\U0001F3C0', 'football': '\U0001F3C8',
        'baseball': '\u26BE', 'tennis': '\U0001F3BE', 'volleyball': '\U0001F3D0',
        'golf': '\u26F3', 'swim': '\U0001F3CA', 'surf': '\U0001F3C4',
        'ski': '\U0001F3BF', 'skate': '\U0001F6F9', 'weight_lift': '\U0001F3CB\uFE0F',
        'basket': '\U0001F3C0', 'trophy': '\U0001F3C6', 'medal': '\U0001F3C5',
        'guitar': '\U0001F3B8', 'drum': '\U0001F941', 'trumpet': '\U0001F3BA',
        'violin': '\U0001F3BB', 'saxophone': '\U0001F3B7', 'microphone': '\U0001F3A4',
        'musical_note': '\U0001F3B5', 'notes': '\U0001F3B6',
        'game': '\U0001F3AE', 'dice': '\U0001F3B2', 'chess': '\U0001FAE0',
        'art': '\U0001F3A8', 'palette': '\U0001F3A8', 'camera_with_flash': '\U0001F4F8',

        # Celebration
        'party': '\U0001F389', 'confetti': '\U0001F38A', 'balloon': '\U0001F388',
        'ribbon': '\U0001F380', 'gift': '\U0001F381', 'birthday': '\U0001F382',
        'fireworks': '\U0001F386', 'sparkler': '\U0001F387', 'sparkles': '\u2728',
        'glitter': '\u2728', 'tada': '\U0001F389', 'christmas_tree': '\U0001F384',
        'santa': '\U0001F385', 'mrs_claus': '\U0001F936',

        # Nature
        'snowflake': '\u2744\uFE0F', 'snowman': '\u26C4',
        'sunrise': '\U0001F305', 'sunset': '\U0001F307',
        'night': '\U0001F303', 'city': '\U0001F306',
        'bridge': '\U0001F309', 'camping': '\U0001F3D5\uFE0F',
        'tent': '\u26FA', 'railway': '\U0001F3D4\uFE0F',

        # Time
        'watch': '\u231A', 'stopwatch': '\u23F1\uFE0F', 'timer': '\u23F2\uFE0F',

        # Misc
        'bio': '\U0001F9EC', 'dna': '\U0001F9EC', 'test_tube': '\U0001F9EA',
        'petri_dish': '\U0001F9EB', 'mag': '\U0001F50D', 'mag_right': '\U0001F50E',
        'tools': '\U0001F6E0\uFE0F', 'alembic': '\u2697\uFE0F',

        # Flags (common)
        'checkered_flag': '\U0001F3C1', 'black_flag': '\U0001F3F4',
        'white_flag': '\U0001F3F3\uFE0F', 'rainbow_flag': '\U0001F3F3\uFE0F\u200D\U0001F308',
        'pirate_flag': '\U0001F3F4\u200D\u2620\uFE0F',

        # Country flags (common)
        'flag_us': '\U0001F1FA\U0001F1F8', 'flag_uk': '\U0001F1EC\U0001F1E7',
        'flag_fr': '\U0001F1EB\U0001F1F7', 'flag_de': '\U0001F1E9\U0001F1EA',
        'flag_it': '\U0001F1EE\U0001F1F9', 'flag_es': '\U0001F1EA\U0001F1F8',
        'flag_jp': '\U0001F1EF\U0001F1F5', 'flag_cn': '\U0001F1E8\U0001F1F3',
        'flag_ru': '\U0001F1F7\U0001F1FA', 'flag_br': '\U0001F1E7\U0001F1F7',
        'flag_in': '\U0001F1EE\U0001F1F3', 'flag_au': '\U0001F1E6\U0001F1FA',
        'flag_ca': '\U0001F1E8\U0001F1E6', 'flag_mx': '\U0001F1F2\U0001F1FD',
        'flag_kr': '\U0001F1F0\U0001F1F7', 'flag_il': '\U0001F1EE\U0001F1F1',
        'flag_ng': '\U0001F1F3\U0001F1EC', 'flag_za': '\U0001F1FF\U0001F1E6',
        'flag_ar': '\U0001F1E6\U0001F1F7', 'flag_ke': '\U0001F1F0\U0001F1EA',
        'flag_eg': '\U0001F1EA\U0001F1EC', 'flag_gh': '\U0001F1EC\U0001F1ED',
        'flag_gr': '\U0001F1EC\U0001F1F7', 'flag_ie': '\U0001F1EE\U0001F1EA',
        'flag_no': '\U0001F1F3\U0001F1F4', 'flag_se': '\U0001F1F8\U0001F1EA',
        'flag_fi': '\U0001F1EB\U0001F1EE', 'flag_dk': '\U0001F1E9\U0001F1F0',
        'flag_nl': '\U0001F1F3\U0001F1F1', 'flag_be': '\U0001F1E7\U0001F1EA',
        'flag_ch': '\U0001F1E8\U0001F1ED', 'flag_at': '\U0001F1E6\U0001F1F9',
        'flag_pl': '\U0001F1F5\U0001F1F1', 'flag_ua': '\U0001F1FA\U0001F1E6',
        'flag_tr': '\U0001F1F9\U0001F1F7', 'flag_sa': '\U0001F1F8\U0001F1E6',
        'flag_ae': '\U0001F1E6\U0001F1EA', 'flag_ph': '\U0001F1F5\U0001F1ED',
        'flag_vn': '\U0001F1FB\U0001F1F3', 'flag_th': '\U0001F1F9\U0001F1ED',
        'flag_id': '\U0001F1EE\U0001F1E9', 'flag_my': '\U0001F1F2\U0001F1FE',
        'flag_sg': '\U0001F1F8\U0001F1EC', 'flag_nz': '\U0001F1F3\U0001F1FF',
        'flag_pt': '\U0001F1F5\U0001F1F9',
    }

    _TEXT_EMOTICONS = {
        ':)': '\U0001F642', ':(': '\U0001F641', ';)': '\U0001F609',
        ':D': '\U0001F604', 'XD': '\U0001F606', 'xD': '\U0001F606',
        ':P': '\U0001F61B', ':p': '\U0001F61B', ';P': '\U0001F61C', ';p': '\U0001F61C',
        ':O': '\U0001F62E', ':o': '\U0001F62E', 'O:': '\U0001F62E',
        '>:(': '\U0001F620', '>:-(': '\U0001F621', ':S': '\U0001F615', ':s': '\U0001F615',
        ':\'(': '\U0001F622', ':\')': '\U0001F602', ':\'D': '\U0001F602',
        'B)': '\U0001F60E', '8)': '\U0001F60E',
        ':|': '\U0001F610', ':/': '\U0001F615', ':\\': '\U0001F615',
        '<3': '\u2764\uFE0F', '</3': '\U0001F494',
        '^_^': '\U0001F60A', '^_~': '\U0001F609',
        '-_-': '\U0001F611', 'o_O': '\U0001F632', 'O_o': '\U0001F632',
        ':*': '\U0001F618', ':+1': '\U0001F44D', ':-1': '\U0001F44E',
        ':wave': '\U0001F44B', ':clap': '\U0001F44F', ':ok': '\U0001F44C',
        ':fire': '\U0001F525', ':100': '\U0001F4AF',
    }

    _CODE_MAP = {}
    for cp in range(0x1F600, 0x1F650):
        _CODE_MAP[f'{cp:x}'] = chr(cp)
    for cp in range(0x1F300, 0x1F5FF):
        _CODE_MAP[f'{cp:x}'] = chr(cp)
    for cp in range(0x1F400, 0x1F5FF):
        _CODE_MAP[f'{cp:x}'] = chr(cp)
    for cp in range(0x1F650, 0x1F680):
        _CODE_MAP[f'{cp:x}'] = chr(cp)
    for cp in range(0x1F680, 0x1F6D0):
        _CODE_MAP[f'{cp:x}'] = chr(cp)
    for cp in range(0x1F900, 0x1FA00):
        _CODE_MAP[f'{cp:x}'] = chr(cp)
    for cp in [0x231A, 0x231B, 0x23F0, 0x23F3, 0x2600, 0x2601, 0x2614, 0x2615,
               0x2648, 0x2649, 0x264A, 0x264B, 0x264C, 0x264D, 0x264E, 0x264F,
               0x2650, 0x2651, 0x2652, 0x2653, 0x267B, 0x267E, 0x2693, 0x2699,
               0x26A0, 0x26A1, 0x26BD, 0x26BE, 0x26C4, 0x26CE, 0x26D4, 0x26EA,
               0x26F3, 0x26F5, 0x26FD, 0x2702, 0x2705, 0x2708, 0x2709, 0x270A,
               0x270B, 0x270C, 0x270D, 0x270F, 0x2712, 0x2714, 0x2716, 0x271D,
               0x2721, 0x2728, 0x2733, 0x2734, 0x2744, 0x2747, 0x274C, 0x274E,
               0x2753, 0x2755, 0x2757, 0x2763, 0x2764, 0x2795, 0x2796, 0x2797,
               0x27A1, 0x27B0, 0x27BF, 0x2934, 0x2935, 0x2B05, 0x2B06, 0x2B07,
               0x2B50, 0x2B55, 0x3030, 0x303D, 0x3297, 0x3299, 0x00A9, 0x00AE,
               0x203C, 0x2049, 0x2122, 0x2139, 0x2194, 0x2195, 0x2196, 0x2197,
               0x2198, 0x2199, 0x21A9, 0x21AA, 0x2328, 0x23CF, 0x23E9, 0x23EA,
               0x23EB, 0x23EC, 0x23ED, 0x23EE, 0x23EF, 0x23F1, 0x23F2, 0x23F8,
               0x23F9, 0x23FA, 0x24C2, 0x25AA, 0x25AB, 0x25B6, 0x25C0, 0x25FB,
               0x25FC, 0x25FD, 0x25FE, 0x2602, 0x2603, 0x2604, 0x260E, 0x2611,
               0x2618, 0x261D, 0x2620, 0x2622, 0x2623, 0x2626, 0x262A, 0x262E,
               0x262F, 0x2638, 0x2639, 0x263A, 0x2640, 0x2642, 0x265F, 0x2660,
               0x2663, 0x2665, 0x2666, 0x2668, 0x267E, 0x267F, 0x2692, 0x2694,
               0x2695, 0x2696, 0x2697, 0x269B, 0x269C]:
        _CODE_MAP[f'{cp:x}'] = chr(cp)

    def _by_name(name):
        return _EMOJI_MAP.get(str(name).lower().replace(' ', '_'), '')

    def _by_code(code):
        return _CODE_MAP.get(str(code).lower().replace('U+', '').replace('0x', '').replace(' ', ''), '')

    def _codes(text):
        s = str(text)
        for pattern, emoji in sorted(_TEXT_EMOTICONS.items(), key=lambda x: -len(x[0])):
            s = s.replace(pattern, emoji)
        return s

    _names_list = sorted(_EMOJI_MAP.keys())

    def _search(q):
        q = str(q).lower()
        results = {}
        for name, emoji in _EMOJI_MAP.items():
            if q in name.lower():
                results[name] = emoji
        return results

    _CATEGORIES = OrderedDict()
    _CATEGORIES['happy'] = ['grin','smiley','smile','sweat_smile','laughing','joy','rofl','happy','wink','blush','innocent','heart_eyes','kissing_heart','kissing','yum','stuck_out_tongue','stuck_out_tongue_wink','stuck_out_tongue_closed_eyes','money_mouth','hug','smirk','relaxed']
    _CATEGORIES['sad'] = ['disappointed','worried','pensive','confused','persevere','confounded','tired','weary','triumph','sob','cry','scream','fearful','cold_sweat','sleepy','sleeping','dizzy','astonished','zipper_mouth','mask','thermometer','sick','nauseated','sneeze']
    _CATEGORIES['angry'] = ['angry','rage','no_mouth','neutral','expressionless','unamused','rolling_eyes','thinking','flushed','slight_frown','frowning']
    _CATEGORIES['faces'] = list(set(_CATEGORIES['happy'] + _CATEGORIES['sad'] + _CATEGORIES['angry'] + ['clown','poop','shit','skull','alien','robot','ghost','angel','devil','imp','ogre','goblin']))
    _CATEGORIES['heart'] = ['heart','red_heart','orange_heart','yellow_heart','green_heart','blue_heart','purple_heart','black_heart','broken_heart','heart_exclamation','two_hearts','revolving_hearts','heartbeat','heartpulse','sparkling_heart','cupid','gift_heart','love_letter']
    _CATEGORIES['hands'] = ['wave','raised_hand','ok_hand','thumbsup','thumbsdown','clap','open_hands','pray','handshake','muscle','point_up','point_down','point_left','point_right','fist','facepunch','middle_finger','fingers_crossed','v','peace','crossed_fingers','call_me','writing_hand','nail_care','selfie','flexed_biceps']
    _CATEGORIES['people'] = ['person','man','woman','girl','boy','baby','old_man','old_woman','person_blond_hair','person_red_hair','person_curly_hair','person_white_hair','person_bald','person_beard','woman_with_headscarf','person_in_tuxedo','bride_with_veil','pregnant_woman','breastfeeding','fairy','vampire','merperson','elf','genie','zombie','dance','dancer','man_dancing','person_walking','person_running','standing','kneeling','person_with_probing_cane','person_in_motorized_wheelchair','eyes','eye','ear','nose','mouth','tongue','lips','bone','anatomy','glasses','sunglasses','necktie','shirt','jeans','dress','bikini','kimono','sari','lab_coat','graduation_cap','crown','hat','tophat','military_helmet','helmet']
    _CATEGORIES['animals'] = ['dog','cat','mouse','hamster','rabbit','fox','bear','panda','koala','tiger','lion','cow','pig','frog','monkey','monkey_face','chicken','penguin','bird','eagle','duck','owl','bat','wolf','horse','unicorn','zebra','deer','cow_face','snake','dragon','dragon_face','dinosaur','whale','dolphin','fish','tropical_fish','blowfish','shark','octopus','shell','snail','butterfly','bug','ant','bee','ladybug','cricket','spider','scorpion','mosquito','microbe']
    _CATEGORIES['food'] = ['apple','green_apple','banana','orange','lemon','watermelon','grapes','strawberry','cherries','peach','mango','pineapple','coconut','kiwi','tomato','eggplant','avocado','broccoli','corn','bread','croissant','pizza','hamburger','fries','hotdog','sandwich','taco','burrito','rice','ramen','spaghetti','sushi','dumpling','cooking','egg','pancakes','waffle','bacon','donut','cake','cupcake','cookie','chocolate','candy','ice_cream','coffee','tea','beer','wine','cocktail','soda','juice','milk','honey','popcorn','champagne']
    _CATEGORIES['nature'] = ['seedling','tree','palm_tree','cactus','flower','rose','wilted_rose','hibiscus','cherry_blossom','blossom','sunflower','bouquet','mushroom','leaf','maple_leaf','four_leaf_clover','shamrock','earth','globe','moon','sun','star','shooting_star','comet','cloud','rainbow','rain','thunder','snow','fire','tornado','wind']
    _CATEGORIES['objects'] = ['phone','mobile','computer','keyboard','printer','mouse_computer','tv','radio','speaker','headphone','camera','video_camera','movie_camera','film','projector','book','books','notebook','newspaper','money_bag','dollar','euro','pound','yen','credit_card','chart','email','inbox','outbox','package','door','bed','toilet','shower','bathtub','key','lock','unlock','bell','bulb','flashlight','wrench','hammer','pick','nut_bolt','gear','alarm','clock','hourglass','microscope','telescope','syringe','pill','adhesive_bandage','stethoscope','broom','basket','toilet_paper','soap','sponge','fire_extinguisher','shopping_cart']
    _CATEGORIES['symbols'] = ['100','hundred','1234','abc','ab','cl','cool','free','id','new','ng','ok','sos','up','vs','check','cross','x','exclamation','question','white_check_mark','warning','no_entry','prohibited','radioactive','biohazard','skull_crossbones','infinity','recycling','fleur_de_lis','trident','name_badge','beginner','tm','copyright','registered','hash','keycap_star','zero','one','two','three','four','five','six','seven','eight','nine','ten','right_arrow','left_arrow','up_arrow','down_arrow','back','end','soon','top','aries','taurus','gemini','cancer','leo','virgo','libra','scorpius','sagittarius','capricorn','aquarius','pisces','ophiuchus']
    _CATEGORIES['flags'] = ['checkered_flag','black_flag','white_flag','rainbow_flag','pirate_flag','flag_us','flag_uk','flag_fr','flag_de','flag_it','flag_es','flag_jp','flag_cn','flag_ru','flag_br','flag_in','flag_au','flag_ca','flag_mx','flag_kr','flag_il','flag_ng','flag_za','flag_ar','flag_ke','flag_eg','flag_gh','flag_gr','flag_ie','flag_no','flag_se','flag_fi','flag_dk','flag_nl','flag_be','flag_ch','flag_at','flag_pl','flag_ua','flag_tr','flag_sa','flag_ae','flag_ph','flag_vn','flag_th','flag_id','flag_my','flag_sg','flag_nz','flag_pt']
    _CATEGORIES['activity'] = ['soccer','basketball','football','baseball','tennis','volleyball','golf','swim','surf','ski','skate','weight_lift','trophy','medal','guitar','drum','trumpet','violin','saxophone','microphone','musical_note','notes','game','dice','chess','art','palette','camera_with_flash']
    _CATEGORIES['travel'] = ['house','hut','office','post_office','hospital','bank','school','castle','church','mosque','synagogue','temple','japan','mountain','volcano','beach','desert','island','park','car','taxi','bus','train','airplane','helicopter','rocket','satellite','ship','boat','anchor','bicycle','motorcycle','fuel_pump','traffic_light','railway_track']
    _CATEGORIES['celebration'] = ['party','confetti','balloon','ribbon','gift','birthday','fireworks','sparkler','sparkles','glitter','tada','christmas_tree','santa','mrs_claus','snowflake','snowman','sunrise','sunset','night','city','bridge','camping','tent','watch','stopwatch','timer','bio','dna','test_tube','petri_dish','mag','mag_right','tools','alembic']

    _emoji_cat = _Categories(_EMOJI_MAP, _CATEGORIES)

    return _EmojiModule(
        _EMOJI_MAP, _by_name, _by_code, _codes, _names_list, _search,
        lambda q=None: _emoji_show(_EMOJI_MAP, q), _emoji_cat,
    )


class _ZenThread:
    def __init__(self, fn):
        object.__setattr__(self, '_t', _threading.Thread(target=fn))
    def start(self):
        object.__getattribute__(self, '_t').start()
    def join(self, timeout=None):
        object.__getattribute__(self, '_t').join(timeout=float(timeout) if timeout is not None else None)
    def is_alive(self):
        return object.__getattribute__(self, '_t').is_alive()
    @property
    def name(self):
        return object.__getattribute__(self, '_t').name
    @property
    def daemon(self):
        return object.__getattribute__(self, '_t').daemon
    @daemon.setter
    def daemon(self, val):
        object.__getattribute__(self, '_t').daemon = bool(val)

def _thread_start(fn):
    t = _threading.Thread(target=fn, daemon=True)
    t.start()
    return {'name': t.name, 'ident': t.ident, 'daemon': t.daemon}

def register_builtins(env, browser):
    cfg = get_config()

    def _sync_config():
        for k in cfg:
            if k not in cfg:
                continue
        set_config('browser_path', cfg.get('browser_path'))
        set_config('browser_type', cfg.get('browser_type'))
        set_config('headless', cfg.get('headless'))
        set_config('timeout', cfg.get('timeout'))
        set_config('ele_timeout', cfg.get('ele_timeout'))

    env.define('config', ConfigModule(cfg, _sync_config))

    env.define('type', lambda v: type(v).__name__)
    env.define('len', lambda v: len(v) if hasattr(v, '__len__') else len(str(v)))
    env.define('str', lambda v: str(v))
    env.define('int', lambda v: int(v))
    env.define('float', lambda v: float(v))
    env.define('bool', lambda v: bool(v))
    env.define('list', lambda v: list(v))
    env.define('exit', lambda code=0: os._exit(int(code)))

    env.define('assert', lambda cond, msg=None: _assert_fn(cond, msg))
    env.define('assert_eq', lambda a, b, msg=None: _assert_eq_fn(a, b, msg))
    env.define('assertEq', lambda a, b, msg=None: _assert_eq_fn(a, b, msg))

    env.define('range', lambda start, end=None, step=1: list(range(start, end, step) if end is not None else range(start)))
    env.define('interval', lambda start, end, step=1: list(range(start, end, step)))
    env.define('enumerate', lambda iterable, start=0: list(enumerate(iterable, int(start))))
    env.define('zip', lambda *iterables: [list(group) for group in zip(*iterables)])
    env.define('map', lambda fn, iterable: list(map(fn, iterable)))
    env.define('filter', lambda fn, iterable: list(filter(fn, iterable)))
    env.define('reduce', lambda fn, iterable, initial=None: __import__('functools').reduce(fn, iterable, initial) if initial is not None else __import__('functools').reduce(fn, iterable))
    env.define('abs', lambda v: abs(v))
    env.define('min', lambda *args: min(args))
    env.define('max', lambda *args: max(args))
    env.define('round', lambda v, ndigits=0: round(v, int(ndigits)))
    env.define('trunc', lambda v, ndigits=0: int(v * 10**int(ndigits)) / 10**int(ndigits) if ndigits else int(v))
    env.define('go', lambda url: browser.go(str(url)) or True)
    env.define('fill', lambda *args, **kwargs: _smart_fill(browser, *args, **kwargs))
    env.define('click', lambda *args, **kwargs: _smart_click(browser, *args, **kwargs))
    env.define('check', lambda *args: _smart_check(browser, *args) if args else None)
    env.define('uncheck', lambda *args: _smart_uncheck(browser, *args) if args else None)
    env.define('select', lambda sel, val: _smart_select(browser, sel, val))
    env.define('text', lambda sel: browser.text(sel))
    env.define('texts', lambda sel: browser.texts(sel))
    env.define('attr', lambda sel, name: browser.attr(sel, name))
    env.define('attrs', lambda sel, name: browser.attrs(sel, name))
    env.define('wait', lambda ms: browser.wait(_parse_duration(ms)))
    env.define('wait_for', lambda *args, **kwargs: _smart_wait(browser, *args, **kwargs))
    env.define('waitFor', lambda *args, **kwargs: _smart_wait(browser, *args, **kwargs))
    env.define('wait_for_network', lambda: browser.wait_for_network())
    env.define('waitForNetwork', lambda: browser.wait_for_network())
    env.define('refresh', lambda: browser.refresh())
    env.define('back', lambda: browser.back())
    env.define('forward', lambda: browser.forward())
    env.define('shot', lambda path, full=False: browser.shot(str(path), bool(full)))
    env.define('execute', lambda code: browser.execute(str(code)))
    env.define('js', lambda code: browser.execute(str(code)))
    env.define('uri', lambda: browser.url())
    env.define('title', lambda: browser.title())
    env.define('user_agent', lambda: browser.user_agent)
    env.define('userAgent', lambda: browser.user_agent)
    env.define('set_user_agent', lambda ua: browser.set_user_agent(str(ua)))
    env.define('setUserAgent', lambda ua: browser.set_user_agent(str(ua)))
    env.define('set_headers', lambda headers: browser.set_headers(dict(headers)))
    env.define('setHeaders', lambda headers: browser.set_headers(dict(headers)))
    env.define('headers', lambda: browser.headers)

    env.define('find', lambda *args, **kwargs: _resolve_find(browser, 'first', *args, **kwargs))
    env.define('find_all', lambda *args, **kwargs: _resolve_find(browser, 'all', *args, **kwargs))
    env.define('findAll', lambda *args, **kwargs: _resolve_find(browser, 'all', *args, **kwargs))
    env.define('first', lambda *args, **kwargs: _resolve_find(browser, 'first', *args, **kwargs))
    env.define('nth', lambda sel, n, **kwargs: _resolve_find(browser, 'nth', sel, n=int(n), **kwargs))

    env.define('download', lambda url, path: browser.download(str(url), str(path)))

    env.define('css', lambda sel: ZenSelector('css', sel))
    env.define('by_text', lambda text: ZenSelector('text', text))
    env.define('byText', lambda text: ZenSelector('text', text))

    env.define('input', lambda prompt, expected_type=None: _typed_input(str(prompt), expected_type))
    env.define('input_str', lambda prompt, expected_type=None: _typed_input(str(prompt), expected_type))
    env.define('inputStr', lambda prompt, expected_type=None: _typed_input(str(prompt), expected_type))
    env.define('prompt', lambda msg="": input(str(msg)))
    env.define('confirm', lambda msg="": input(str(msg) + " (y/n): ").strip().lower() in ('y', 'yes'))

    env.define('scroll_to', lambda y: browser.execute(f'window.scrollTo(0, {y})'))
    env.define('scrollTo', lambda y: browser.execute(f'window.scrollTo(0, {y})'))

    env.define('sleep', lambda secs: _time.sleep(float(secs)))

    env.define('read_file', lambda path: _read_file(str(path)))
    env.define('readFile', lambda path: _read_file(str(path)))
    env.define('write_file', lambda path, content: _write_file(str(path), str(content)))
    env.define('writeFile', lambda path, content: _write_file(str(path), str(content)))
    env.define('append_file', lambda path, content: _append_file(str(path), str(content)))
    env.define('appendFile', lambda path, content: _append_file(str(path), str(content)))
    env.define('file_exists', lambda path: os.path.exists(str(path)))
    env.define('fileExists', lambda path: os.path.exists(str(path)))
    env.define('list_dir', lambda path='.': os.listdir(str(path)))
    env.define('listDir', lambda path='.': os.listdir(str(path)))
    env.define('mkdir', lambda path: os.makedirs(str(path), exist_ok=True))
    env.define('remove_file', lambda path: os.remove(str(path)))
    env.define('removeFile', lambda path: os.remove(str(path)))
    env.define('copy_file', lambda src, dst: __import__('shutil').copy2(str(src), str(dst)))
    env.define('copyFile', lambda src, dst: __import__('shutil').copy2(str(src), str(dst)))
    env.define('move_file', lambda src, dst: __import__('shutil').move(str(src), str(dst)))
    env.define('moveFile', lambda src, dst: __import__('shutil').move(str(src), str(dst)))
    env.define('rename_file', lambda src, dst: os.rename(str(src), str(dst)))
    env.define('renameFile', lambda src, dst: os.rename(str(src), str(dst)))
    env.define('path_join', lambda *parts: os.path.join(*[str(p) for p in parts]))
    env.define('pathJoin', lambda *parts: os.path.join(*[str(p) for p in parts]))

    env.define('basename', lambda path: os.path.basename(str(path)))
    env.define('dirname', lambda path: os.path.dirname(str(path)))
    env.define('cwd', lambda: os.getcwd())
    env.define('pwd', lambda: os.getcwd())
    env.define('cd', lambda path: os.chdir(str(path)))
    env.define('chdir', lambda path: os.chdir(str(path)))

    env.define('read_binary', lambda path: _read_binary(str(path)))
    env.define('readBinary', lambda path: _read_binary(str(path)))
    env.define('write_binary', lambda path, data: _write_binary(str(path), data))
    env.define('writeBinary', lambda path, data: _write_binary(str(path), data))
    env.define('base64_decode', lambda s: _base64.b64decode(str(s)))
    env.define('b64decode', lambda s: _base64.b64decode(str(s)))

    env.define('rmdir', lambda path: os.rmdir(str(path)))
    env.define('remove_dir', lambda path: os.rmdir(str(path)))
    env.define('removeDir', lambda path: os.rmdir(str(path)))

    env.define('glob', lambda pattern: __import__('glob').glob(str(pattern)))
    env.define('file_size', lambda path: os.path.getsize(str(path)))
    env.define('fileSize', lambda path: os.path.getsize(str(path)))
    env.define('file_mtime', lambda path: os.path.getmtime(str(path)))
    env.define('fileMtime', lambda path: os.path.getmtime(str(path)))
    env.define('is_file', lambda path: os.path.isfile(str(path)))
    env.define('isFile', lambda path: os.path.isfile(str(path)))
    env.define('is_dir', lambda path: os.path.isdir(str(path)))
    env.define('isDir', lambda path: os.path.isdir(str(path)))

    env.define('exec', lambda cmd: _exec_cmd(str(cmd)))
    env.define('sh', lambda cmd: _exec_cmd(str(cmd)))
    env.define('system', lambda cmd: _exec_cmd(str(cmd)))

    # File object
    from .environment import ZenFile
    env.define('file', lambda path, mode='r': ZenFile(str(path), str(mode)))

    # Bytes object
    from .environment import ZenBytes
    env.define('bytes', lambda data='': ZenBytes(data))

    env.define('python', lambda code, **kwargs: _exec_python(str(code), kwargs.get('globals')))
    env.define('lua', lambda code: _exec_lua(str(code)))

    env.define('fs', {
        'list': lambda path='.': os.listdir(str(path)),
        'read': lambda path: _read_file(str(path)),
        'write': lambda path, content: _write_file(str(path), str(content)),
        'append': lambda path, content: _append_file(str(path), str(content)),
        'read_binary': lambda path: _read_binary(str(path)),
        'readBinary': lambda path: _read_binary(str(path)),
        'write_binary': lambda path, data: _write_binary(str(path), data),
        'writeBinary': lambda path, data: _write_binary(str(path), data),
        'exists': lambda path: os.path.exists(str(path)),
        'is_file': lambda path: os.path.isfile(str(path)),
        'isFile': lambda path: os.path.isfile(str(path)),
        'is_dir': lambda path: os.path.isdir(str(path)),
        'isDir': lambda path: os.path.isdir(str(path)),
        'size': lambda path: os.path.getsize(str(path)),
        'mtime': lambda path: os.path.getmtime(str(path)),
        'mkdir': lambda path: os.makedirs(str(path), exist_ok=True),
        'mkdirs': lambda path: os.makedirs(str(path), exist_ok=True),
        'remove': lambda path: os.remove(str(path)),
        'rmdir': lambda path: os.rmdir(str(path)),
        'rmtree': lambda path: __import__('shutil').rmtree(str(path)),
        'copy': lambda src, dst: __import__('shutil').copy2(str(src), str(dst)),
        'move': lambda src, dst: __import__('shutil').move(str(src), str(dst)),
        'rename': lambda src, dst: os.rename(str(src), str(dst)),
        'glob': lambda pattern: __import__('glob').glob(str(pattern)),
        'join': lambda *parts: os.path.join(*[str(p) for p in parts]),
        'basename': lambda path: os.path.basename(str(path)),
        'dirname': lambda path: os.path.dirname(str(path)),
        'cwd': lambda: os.getcwd(),
        'cd': lambda path: os.chdir(str(path)),
        'exec': lambda cmd: _exec_cmd(str(cmd)),
        'sh': lambda cmd: _exec_cmd(str(cmd)),
    })

    env.define('history', lambda: browser.url_history)

    env.define('search', lambda *args, **kwargs: browser.search(*args, **kwargs))
    env.define('find_by_text', lambda text, exact=False: browser.find_by_text(text, exact))
    env.define('findByText', lambda text, exact=False: browser.find_by_text(text, exact))
    env.define('find_by_url', lambda url, partial=True: browser.find_by_url(url, partial))
    env.define('findByUrl', lambda url, partial=True: browser.find_by_url(url, partial))

    env.define('page_html', lambda: browser.page_html())
    env.define('pageHtml', lambda: browser.page_html())
    env.define('page_text', lambda: browser.page_text_markers())
    env.define('pageText', lambda: browser.page_text_markers())
    env.define('page_links', lambda: browser.page_links())
    env.define('pageLinks', lambda: browser.page_links())
    env.define('page_images', lambda: browser.page_images())
    env.define('pageImages', lambda: browser.page_images())
    env.define('page_forms', lambda: browser.page_forms())
    env.define('pageForms', lambda: browser.page_forms())

    env.define('csv', {
        'read': lambda path: _csv_read(path),
        'write': lambda path, rows, headers=None: _csv_write(path, rows, headers),
        'parse': lambda text: _csv_parse(text),
        'encode': lambda rows, headers=None: _csv_encode(rows, headers),
    })
    env.define('csv_read', lambda path: _csv_read(path))
    env.define('csvRead', lambda path: _csv_read(path))
    env.define('csv_write', lambda path, rows, headers=None: _csv_write(path, rows, headers))
    env.define('csvWrite', lambda path, rows, headers=None: _csv_write(path, rows, headers))
    env.define('csv_parse', lambda text: _csv_parse(text))
    env.define('csvParse', lambda text: _csv_parse(text))
    env.define('csv_encode', lambda rows, headers=None: _csv_encode(rows, headers))
    env.define('csvEncode', lambda rows, headers=None: _csv_encode(rows, headers))
    env.define('json_parse', lambda text: _json_parse(text))
    env.define('jsonParse', lambda text: _json_parse(text))
    env.define('json_encode', lambda val: _json_encode(val))
    env.define('jsonEncode', lambda val: _json_encode(val))

    env.define('re', {
        'matches': lambda pattern, string: bool(__import__('re').fullmatch(str(pattern), str(string))),
        'search': lambda pattern, string: _re_search(pattern, string),
        'findall': lambda pattern, string: __import__('re').findall(str(pattern), str(string)),
        'split': lambda pattern, string: __import__('re').split(str(pattern), str(string)),
        'sub': lambda pattern, repl, string: __import__('re').sub(str(pattern), str(repl), str(string)),
    })

    env.define('http', {
        'get': lambda url, opts=None, **kw: _http_request('GET', str(url), **{**(opts or {}), **kw}),
        'post': lambda url, data=None, json=None, opts=None, **kw: _http_request('POST', str(url), data, json, **{**(opts or {}), **kw}),
        'put': lambda url, data=None, json=None, opts=None, **kw: _http_request('PUT', str(url), data, json, **{**(opts or {}), **kw}),
        'del': lambda url, opts=None, **kw: _http_request('DELETE', str(url), **{**(opts or {}), **kw}),
        'head': lambda url, opts=None, **kw: _http_request('HEAD', str(url), **{**(opts or {}), **kw}),
        'patch': lambda url, data=None, json=None, opts=None, **kw: _http_request('PATCH', str(url), data, json, **{**(opts or {}), **kw}),
    })

    if browser is not None:
        env.define('net', {
            'online': lambda: bool(browser.execute('navigator.onLine')),
            'cookies': lambda: browser.execute('document.cookie'),
            'uri': lambda: browser.url(),
        })
        env.define('cookies', {
            'all': lambda: browser.execute('document.cookie.split("; ").filter(Boolean).map(c => { let [n,...v] = c.split("="); return {name:n.trim(), value:v.join("=")} })'),
            'get': lambda name: browser.execute(f'document.cookie.split("; ").find(c => c.startsWith("{name}="))?.split("=").slice(1).join("=") || null'),
            'set': lambda name, value, path='/': browser.execute(f'document.cookie = "{name}={value}; path={path}"'),
            'clear': lambda: browser.execute('document.cookie.split("; ").forEach(c => { let n = c.split("=")[0]; document.cookie = n + "=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/" }) || true'),
        })
        env.define('storage', {
            'get': lambda key: browser.execute(f'localStorage.getItem("{key}")'),
            'set': lambda key, value: browser.execute(f'localStorage.setItem("{key}", "{value}") || true'),
            'remove': lambda key: browser.execute(f'localStorage.removeItem("{key}") || true'),
            'clear': lambda: browser.execute('localStorage.clear() || true'),
            'all': lambda: browser.execute('Object.entries(localStorage).map(([k,v]) => ({key:k, value:v}))'),
        })

        env.define('page', PageModule(browser))

        env.define('popup', _PopupModule(browser))

    env.define('random', {
        'random': lambda: _random.random(),
        'randint': lambda a, b: _random.randint(int(a), int(b)),
        'randrange': lambda start, stop=None, step=1: _random.randrange(int(start), int(stop), int(step)) if stop is not None else _random.randrange(int(start)),
        'choice': lambda seq: _random.choice(seq),
        'choices': lambda seq, k=1: [_random.choice(seq) for _ in range(int(k))],
        'sample': lambda seq, k: _random.sample(seq, int(k)),
        'shuffle': lambda seq: _random.sample(seq, len(seq)),
        'uniform': lambda a, b: _random.uniform(float(a), float(b)),
        'hex': lambda k=16: ''.join(_random.choices('0123456789abcdef', k=int(k))),
        'seed': lambda n=None: _random.seed(n),
    })

    env.define('math', {
        'pi': _math.pi,
        'e': _math.e,
        'inf': _math.inf,
        'nan': _math.nan,
        'floor': lambda x: _math.floor(x),
        'ceil': lambda x: _math.ceil(x),
        'trunc': lambda x: _math.trunc(x),
        'sqrt': lambda x: _math.sqrt(x),
        'abs': lambda x: abs(x),
        'pow': lambda x, y: _math.pow(x, y),
        'exp': lambda x: _math.exp(x),
        'log': lambda x, base=_math.e: _math.log(x, base),
        'log2': lambda x: _math.log2(x),
        'log10': lambda x: _math.log10(x),
        'sin': lambda x: _math.sin(x),
        'cos': lambda x: _math.cos(x),
        'tan': lambda x: _math.tan(x),
        'asin': lambda x: _math.asin(x),
        'acos': lambda x: _math.acos(x),
        'atan': lambda x: _math.atan(x),
        'atan2': lambda y, x: _math.atan2(y, x),
        'degrees': lambda x: _math.degrees(x),
        'radians': lambda x: _math.radians(x),
        'hypot': lambda *args: _math.hypot(*args),
        'isnan': lambda x: _math.isnan(x),
        'isfinite': lambda x: _math.isfinite(x),
        'isinf': lambda x: _math.isinf(x),
        'copysign': lambda x, y: _math.copysign(x, y),
        'gcd': lambda a, b: _math.gcd(int(a), int(b)),
        'lcm': lambda a, b: _math.lcm(int(a), int(b)),
        'factorial': lambda x: _math.factorial(int(x)),
        'comb': lambda n, k: _math.comb(int(n), int(k)),
        'perm': lambda n, k=None: _math.perm(int(n), int(k)) if k is not None else _math.perm(int(n)),
        'remainder': lambda x, y: _math.remainder(x, y),
        'fsum': lambda iterable: _math.fsum(iterable),
        'prod': lambda iterable, start=1: _math.prod(iterable, start=int(start)),
        'modf': lambda x: list(_math.modf(x)),
        'frexp': lambda x: list(_math.frexp(x)),
        'ldexp': lambda x, exp: _math.ldexp(x, int(exp)),
        'round': lambda x, ndigits=0: round(x, int(ndigits)),
    })

    env.define('time', {
        'now': lambda: _datetime.datetime.now().isoformat(),
        'unix': lambda: _time.time(),
        'utc': lambda: _datetime.datetime.now(_datetime.timezone.utc).isoformat(),
        'date': lambda: _datetime.date.today().isoformat(),
        'format': lambda fmt=None: _datetime.datetime.now().strftime(str(fmt)) if fmt else _datetime.datetime.now().isoformat(),
        'parse': lambda s, fmt: _datetime.datetime.strptime(str(s), str(fmt)).isoformat(),
        'sleep': lambda secs: _time.sleep(float(secs)),
        'wait': lambda ms: _time.sleep(float(ms) / 1000),
        'year': lambda: _datetime.datetime.now().year,
        'month': lambda: _datetime.datetime.now().month,
        'day': lambda: _datetime.datetime.now().day,
        'hour': lambda: _datetime.datetime.now().hour,
        'minute': lambda: _datetime.datetime.now().minute,
        'second': lambda: _datetime.datetime.now().second,
        'weekday': lambda: _datetime.datetime.now().weekday(),
        'timestamp': lambda: _time.time(),
    })

    env.define('os', {
        'env': lambda key, default=None: os.environ.get(str(key), default),
        'exit': lambda code=0: os._exit(int(code)),
        'platform': lambda: os.uname().sysname if hasattr(os, 'uname') else os.name,
        'hostname': lambda: _socket.gethostname(),
        'pid': lambda: os.getpid(),
        'cwd': lambda: os.getcwd(),
        'chdir': lambda path: os.chdir(str(path)),
        'name': os.name,
        'sep': os.sep,
        'linesep': os.linesep,
        'cpu_count': lambda: os.cpu_count(),
        'getenv': lambda key, default=None: os.environ.get(str(key), default),
        'setenv': lambda key, val: os.environ.__setitem__(str(key), str(val)),
        'unsetenv': lambda key: os.environ.pop(str(key), None),
        'system': lambda cmd: os.system(str(cmd)),
        'popen': lambda cmd, *args: _os_popen(cmd, args),
    })

    _COLOR_NAMES = {
        'black': 0, 'red': 1, 'green': 2, 'yellow': 3,
        'blue': 4, 'magenta': 5, 'cyan': 6, 'white': 7,
        'bright_black': 8, 'bright_red': 9, 'bright_green': 10,
        'bright_yellow': 11, 'bright_blue': 12, 'bright_magenta': 13,
        'bright_cyan': 14, 'bright_white': 15,
    }

    def _color_fn(code):
        return lambda text=None: f'\033[{code}m{text}\033[0m' if text is not None else f'\033[{code}m'

    color_mod = {
        'rgb': lambda r, g, b, text=None: f'\033[38;2;{int(r)};{int(g)};{int(b)}m{text or ""}\033[0m',
        'bg_rgb': lambda r, g, b, text=None: f'\033[48;2;{int(r)};{int(g)};{int(b)}m{text or ""}\033[0m',
        'hex': lambda h, text=None: _hex_color(str(h), text),
        'strip': lambda text: __import__('re').sub(r'\033\[[0-9;]*m', '', str(text)),
        'reset': '\033[0m',
        'bold': lambda text=None: f'\033[1m{text}\033[0m' if text is not None else '\033[1m',
        'dim': lambda text=None: f'\033[2m{text}\033[0m' if text is not None else '\033[2m',
        'italic': lambda text=None: f'\033[3m{text}\033[0m' if text is not None else '\033[3m',
        'underline': lambda text=None: f'\033[4m{text}\033[0m' if text is not None else '\033[4m',
        'blink': lambda text=None: f'\033[5m{text}\033[0m' if text is not None else '\033[5m',
        'reverse': lambda text=None: f'\033[7m{text}\033[0m' if text is not None else '\033[7m',
        'hidden': lambda text=None: f'\033[8m{text}\033[0m' if text is not None else '\033[8m',
        'strike': lambda text=None: f'\033[9m{text}\033[0m' if text is not None else '\033[9m',
    }

    def _hex_color(h, text):
        h = h.lstrip('#')
        if len(h) == 3:
            h = ''.join(c * 2 for c in h)
        r, g, b = int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)
        return f'\033[38;2;{r};{g};{b}m{text or ""}\033[0m'

    for name, code in _COLOR_NAMES.items():
        fg = 30 + code if code < 8 else 82 + code
        color_mod[name] = _color_fn(fg)
        color_mod[f'bg_{name}'] = _color_fn(40 + code if code < 8 else 92 + code)

    env.define('color', color_mod)

    # --- base64 module ---
    env.define('base64', {
        'encode': lambda data: _base64.b64encode(_to_bytes(data)).decode(),
        'decode': lambda data: _base64.b64decode(str(data)).decode(),
        'url_encode': lambda data: _base64.urlsafe_b64encode(_to_bytes(data)).decode(),
        'url_decode': lambda data: _base64.urlsafe_b64decode(str(data)).decode(),
    })

    # --- base32 module ---
    env.define('base32', {
        'encode': lambda data: _base64.b32encode(_to_bytes(data)).decode(),
        'decode': lambda data: _base64.b32decode(str(data)).decode(),
    })

    # --- crypto module ---
    env.define('crypto', {
        'sha256': lambda data: _hashlib.sha256(_to_bytes(data)).hexdigest(),
        'sha1': lambda data: _hashlib.sha1(_to_bytes(data)).hexdigest(),
        'md5': lambda data: _hashlib.md5(_to_bytes(data)).hexdigest(),
        'sha512': lambda data: _hashlib.sha512(_to_bytes(data)).hexdigest(),
        'sha224': lambda data: _hashlib.sha224(_to_bytes(data)).hexdigest(),
        'sha384': lambda data: _hashlib.sha384(_to_bytes(data)).hexdigest(),
        'sha3_256': lambda data: _hashlib.sha3_256(_to_bytes(data)).hexdigest(),
        'sha3_512': lambda data: _hashlib.sha3_512(_to_bytes(data)).hexdigest(),
        'blake2b': lambda data: _hashlib.blake2b(_to_bytes(data)).hexdigest(),
        'blake2s': lambda data: _hashlib.blake2s(_to_bytes(data)).hexdigest(),
        'hmac_sha256': lambda key, data: _hmac.new(_to_bytes(key), _to_bytes(data), 'sha256').hexdigest(),
        'hmac_sha1': lambda key, data: _hmac.new(_to_bytes(key), _to_bytes(data), 'sha1').hexdigest(),
        'hmac_md5': lambda key, data: _hmac.new(_to_bytes(key), _to_bytes(data), 'md5').hexdigest(),
        'random_bytes': lambda n: _base64.b16encode(os.urandom(int(n))).decode().lower(),
        'random_hex': lambda n: _base64.b16encode(os.urandom(int(n))).decode().lower(),
        'pbkdf2': lambda password, salt, iterations=100000, dklen=32: _hashlib.pbkdf2_hmac('sha256', _to_bytes(password), _to_bytes(salt), int(iterations), int(dklen)).hex(),
        'aes_encrypt': lambda key, data, iv=None: _aes_encrypt(key, data, iv),
        'aes_decrypt': lambda key, data, iv=None: _aes_decrypt(key, data, iv),
    })

    # --- cryptography module (Fernet, etc.) ---
    env.define('cryptography', _build_cryptography_module())

    # --- datetime module ---
    env.define('datetime', {
        'now': lambda: _datetime.datetime.now().isoformat(),
        'utcnow': lambda: _datetime.datetime.now(_datetime.timezone.utc).isoformat(),
        'today': lambda: _datetime.date.today().isoformat(),
        'unix': lambda: _time.time(),
        'from_unix': lambda ts: _datetime.datetime.fromtimestamp(float(ts), tz=_datetime.timezone.utc).isoformat(),
        'parse': lambda s, fmt: _datetime.datetime.strptime(str(s), str(fmt)).isoformat(),
        'format': lambda dt, fmt: _datetime.datetime.fromisoformat(str(dt)).strftime(str(fmt)) if isinstance(dt, str) else dt.strftime(str(fmt)),
        'year': lambda: _datetime.datetime.now().year,
        'month': lambda: _datetime.datetime.now().month,
        'day': lambda: _datetime.datetime.now().day,
        'hour': lambda: _datetime.datetime.now().hour,
        'minute': lambda: _datetime.datetime.now().minute,
        'second': lambda: _datetime.datetime.now().second,
        'weekday': lambda: _datetime.datetime.now().weekday(),
        'add_days': lambda d, n: _datetime.datetime.fromisoformat(str(d)) if isinstance(d, str) else d,
        'MONDAY': 0, 'TUESDAY': 1, 'WEDNESDAY': 2, 'THURSDAY': 3,
        'FRIDAY': 4, 'SATURDAY': 5, 'SUNDAY': 6,
    })

    # --- uuid module ---
    env.define('uuid', {
        'uuid4': lambda: str(__import__('uuid').uuid4()),
        'uuid1': lambda: str(__import__('uuid').uuid1()),
        'uuid3': lambda ns, name: str(__import__('uuid').uuid3(__import__('uuid').NAMESPACE_DNS if ns == 'dns' else __import__('uuid').NAMESPACE_URL if ns == 'url' else __import__('uuid').NAMESPACE_OID if ns == 'oid' else __import__('uuid').NAMESPACE_X500 if ns == 'x500' else ns, str(name))),
        'uuid5': lambda ns, name: str(__import__('uuid').uuid5(__import__('uuid').NAMESPACE_DNS if ns == 'dns' else __import__('uuid').NAMESPACE_URL if ns == 'url' else __import__('uuid').NAMESPACE_OID if ns == 'oid' else __import__('uuid').NAMESPACE_X500 if ns == 'x500' else ns, str(name))),
        'NAMESPACE_DNS': 'dns',
        'NAMESPACE_URL': 'url',
        'NAMESPACE_OID': 'oid',
        'NAMESPACE_X500': 'x500',
    })

    # --- json module ---
    env.define('json', {
        'parse': lambda text: _json.loads(str(text)),
        'encode': lambda val, pretty=False: _json.dumps(val, indent=2 if pretty else None, ensure_ascii=False) if pretty else _json.dumps(val, ensure_ascii=False, separators=(',', ':')),
        'load': lambda path: _json_loads_file(str(path)),
        'save': lambda path, val: _json_save_file(str(path), val),
    })

    # --- statistics module ---
    env.define('statistics', {
        'mean': lambda data: _statistics_mean(list(data)),
        'median': lambda data: _statistics_median(list(data)),
        'mode': lambda data: _statistics_mode(list(data)),
        'stdev': lambda data: _statistics_stdev(list(data)),
        'variance': lambda data: _statistics_variance(list(data)),
        'min': lambda *args: min(args),
        'max': lambda *args: max(args),
        'sum': lambda data: sum(list(data)),
    })

    # --- decimal module ---
    env.define('decimal', {
        'Decimal': lambda val: _build_decimal(str(val)),
        'getcontext': lambda: _decimal_getcontext(),
        'setcontext': lambda ctx: _decimal_setcontext(ctx),
        'localcontext': lambda ctx=None, **kw: _decimal_localcontext(ctx, **kw),
        'ROUND_HALF_UP': 'ROUND_HALF_UP',
        'ROUND_HALF_EVEN': 'ROUND_HALF_EVEN',
        'ROUND_DOWN': 'ROUND_DOWN',
        'ROUND_UP': 'ROUND_UP',
        'ROUND_CEILING': 'ROUND_CEILING',
        'ROUND_FLOOR': 'ROUND_FLOOR',
        'ROUND_HALF_DOWN': 'ROUND_HALF_DOWN',
        'ROUND_05UP': 'ROUND_05UP',
    })

    # --- threading module ---
    env.define('threading', {
        'start': _thread_start,
        'Thread': lambda fn: _ZenThread(fn),
        'current': lambda: _threading.current_thread().name,
        'active': lambda: _threading.active_count(),
        'list': lambda: [{'name': t.name, 'ident': t.ident, 'daemon': t.daemon} for t in _threading.enumerate()],
        'Lock': lambda: _threading.Lock(),
        'RLock': lambda: _threading.RLock(),
        'Event': lambda: _threading.Event(),
        'Condition': lambda lock=None: _threading.Condition(lock) if lock else _threading.Condition(),
        'Semaphore': lambda n=1: _threading.Semaphore(int(n) if n else 1),
        'Barrier': lambda n: _threading.Barrier(int(n)),
        'Queue': lambda maxsize=0: _queue.Queue(maxsize=int(maxsize)),
        'sleep': lambda secs: _time.sleep(float(secs)),
    })

    # --- emoji module ---
    env.define('emoji', _build_emoji_module())

    try:
        from .wa_module import _build_wa_module
        env.define('wa', _build_wa_module())
    except Exception:
        pass

    try:
        from .cookies_module import _build_cookies_module
        env.define('cookies', _build_cookies_module())
    except Exception:
        pass


def _csv_read(path):
    import csv
    path = os.path.expanduser(str(path))
    with open(path, 'r', encoding='utf-8') as f:
        return list(csv.reader(f))

def _csv_write(path, rows, headers=None):
    import csv
    path = os.path.expanduser(str(path))
    d = os.path.dirname(path)
    if d and not os.path.exists(d):
        os.makedirs(d, exist_ok=True)
    with open(path, 'w', newline='') as f:
        w = csv.writer(f)
        if headers:
            w.writerow(headers)
        w.writerows(rows)
    return True

def _csv_parse(text):
    import csv, io
    return list(csv.reader(io.StringIO(str(text))))

def _csv_encode(rows, headers=None):
    import csv, io
    buf = io.StringIO()
    w = csv.writer(buf)
    if headers:
        w.writerow(headers)
    w.writerows(rows)
    return buf.getvalue()

def _re_search(pattern, string):
    import re
    m = re.search(str(pattern), str(string))
    if not m:
        return None
    return ZenRegexMatch(m)


def _json_parse(text):
    import json
    return json.loads(str(text))

def _json_encode(val):
    import json
    return json.dumps(val, ensure_ascii=False, separators=(',', ':'))


def _read_file(path):
    with open(os.path.expanduser(path), 'r', encoding='utf-8') as f:
        return f.read()


def _write_file(path, content):
    path = os.path.expanduser(path)
    d = os.path.dirname(path)
    if d and not os.path.exists(d):
        os.makedirs(d, exist_ok=True)
    with open(path, 'w', encoding='utf-8') as f:
        f.write(content)
    return True


def _append_file(path, content):
    path = os.path.expanduser(path)
    with open(path, 'a') as f:
        f.write(content)
    return True


def _read_binary(path):
    with open(path, 'rb') as f:
        return f.read()

def _write_binary(path, data):
    if isinstance(data, str):
        data = data.encode('utf-8')
    elif isinstance(data, list):
        data = bytes(data)
    with open(path, 'wb') as f:
        f.write(data)
    return True

def _exec_cmd(cmd):
    import subprocess as _sp
    result = _sp.run(cmd, shell=True, capture_output=True, text=True)
    return {
        'returncode': result.returncode,
        'stdout': result.stdout,
        'stderr': result.stderr,
    }


def _exec_python(code, globals_dict=None):
    import sys
    import io
    old_stdout = sys.stdout
    sys.stdout = io.StringIO()
    try:
        g = globals_dict if globals_dict else {}
        exec(code, g)
        result = sys.stdout.getvalue()
        return result if result else None
    finally:
        sys.stdout = old_stdout

def _exec_lua(code):
    import subprocess as _sp
    try:
        import lupa
        from lupa import LuaRuntime
        lua = LuaRuntime(unpack_returned_tuples=True)
        result = lua.execute(code)
        return result
    except ImportError:
        try:
            result = _sp.run(['lua', '-e', code], capture_output=True, text=True, timeout=10)
            if result.returncode == 0:
                return result.stdout.strip() if result.stdout.strip() else None
            raise ZenError(f"Lua error: {result.stderr}")
        except FileNotFoundError:
            raise ZenError("Lua not available — install 'lua' binary or 'lupa' Python package")

def _http_request(method, url, data=None, json=None, headers=None, timeout=30):
    if json is not None:
        data = _json.dumps(json).encode('utf-8')
        if headers is None:
            headers = {}
        if 'Content-Type' not in headers:
            headers['Content-Type'] = 'application/json'
    elif data is not None:
        data = str(data).encode('utf-8')
    req = _urllib.Request(url, data=data, method=method)
    if headers:
        for k, v in headers.items():
            req.add_header(str(k), str(v))
    try:
        resp = _urllib.urlopen(req, timeout=int(timeout))
        raw = resp.read()
        body = raw.decode('utf-8', errors='replace')
        return HttpResponse(resp.status, body, resp.getheaders(), raw=raw)
    except _urllib.HTTPError as e:
        raw = e.read()
        body = raw.decode('utf-8', errors='replace')
        return HttpResponse(e.code, body, e.headers, raw=raw)
    except Exception as e:
        raise ZenError(f'HTTP {method} {url}: {e}')


def _parse_duration(dur):
    if isinstance(dur, (int, float)):
        return int(dur)
    s = str(dur).strip().lower()
    if s.endswith('m'):
        return float(s[:-1]) * 60000
    if s.endswith('ms'):
        return float(s[:-2])
    if s.endswith('s'):
        return float(s[:-1]) * 1000
    return float(s)


class _PopupModule:
    """Comprehensive popup handler for any web framework.

    Detects and interacts with:
      - SweetAlert2       (.swal2-popup)
      - SweetAlert1       (.sweet-alert)
      - Bootstrap modal   (.modal.show / .in)
      - jQuery UI dialog  (.ui-dialog)
      - Material UI       (.MuiDialog-root)
      - Ant Design        (.ant-modal)
      - Element UI        (.el-dialog)
      - Vuetify           (.v-dialog)
      - Native browser    (alert/confirm/prompt via DrissionPage)
      - Generic overlays  (any fixed/absolute element with high z-index + backdrop)
    """

    POPUP_SELECTORS = [
        '.swal2-popup',
        '.sweet-alert',
        '.modal.show, .modal.fade.in, .modal[style*="display: block"]',
        '.ui-dialog',
        '.MuiDialog-root',
        '.ant-modal',
        '.el-dialog',
        '.v-dialog',
    ]

    BUTTON_SELECTORS = [
        '.swal2-confirm',
        '.swal2-cancel',
        '.swal2-deny',
        '.ui-dialog .ui-dialog-buttonpane button',
        '.modal.show .modal-footer button, .modal.fade.in .modal-footer button',
        '.MuiDialog-root .MuiDialogActions-root button',
        '.ant-modal .ant-modal-footer button',
        '.el-dialog .el-dialog__footer button',
        '.v-dialog .v-card-actions button',
    ]

    def __init__(self, browser):
        self._browser = browser

    def _js(self, code):
        """Execute JS returning raw value."""
        return self._browser.execute(code)

    def _js_obj(self, code):
        """Execute JS that returns a simple object (serialised via JSON)."""
        wrapped = f"JSON.stringify((function(){{{code}}})())"
        raw = self._browser.execute(wrapped)
        if isinstance(raw, str):
            import json as _json
            try:
                return _json.loads(raw)
            except Exception:
                return None
        return raw

    # ── Generic popup scan ────────────────────────────────────

    def _known_popup_selectors(self):
        """Return CSS selector matching any known framework popup."""
        return ', '.join(self.POPUP_SELECTORS)

    def _heuristic_scan(self):
        """Find any visible popup-like element via heuristics (last resort)."""
        return self._js_obj("""
            var all = document.querySelectorAll('body > *, div, section, aside');
            var candidates = [];
            var maxZ = 0;
            var topEl = null;

            for (var i = 0; i < all.length; i++) {
                var el = all[i];
                if (el.offsetParent === null && window.getComputedStyle(el).display === 'none') continue;
                var cs = window.getComputedStyle(el);
                if (cs.position !== 'fixed' && cs.position !== 'absolute') continue;
                var z = parseInt(cs.zIndex);
                if (isNaN(z)) z = 0;
                if (z < 100) continue;
                if (z > maxZ) { maxZ = z; topEl = el; }
            }

            if (!topEl) return null;

            return {
                element: 'heuristic',
                title: (topEl.querySelector('h1, h2, h3, h4, .title, .modal-title, .ui-dialog-title') || {}).innerText || '',
                content: (topEl.querySelector('p, .content, .modal-body, .message, .ui-dialog-content') || {}).innerText || '',
                buttons: Array.from(topEl.querySelectorAll('button, a.btn')).map(function(b) { return b.innerText.trim(); }).filter(Boolean),
                visible: true
            };
        """)

    def _detect_any_popup(self):
        """Return info dict for whatever popup is visible, or None."""
        # 1 — Known framework selectors
        found = self._js("(function(){ var s='" + self._known_popup_selectors().replace("'", "\\'") + "'; return document.querySelector(s) !== null ? document.querySelector(s).className : null; })()")
        if found:
            return self._extract_popup_info(found)

        # 2 — Native browser dialogs (alert/confirm/prompt)
        try:
            alert_exists = self._browser._drission.wait.alert_exists(timeout=0.1)
            if alert_exists:
                return {'type': 'native', 'visible': True}
        except Exception:
            pass

        # 3 — Heuristic: any fixed/absolute element with z-index ≥ 100
        heur = self._heuristic_scan()
        if heur and heur.get('buttons') or heur.get('title') or heur.get('content'):
            heur['type'] = 'heuristic'
            return heur

        return None

    def _extract_popup_info(self, class_name):
        """Given a class match, extract full popup info."""
        cls = str(class_name)

        # ── SweetAlert2 ────────────────────────────────────────
        if 'swal2' in cls:
            v = self._vis_fn()
            return {
                'type': 'sweetalert2',
                'title': self._js("document.querySelector('.swal2-title') ? document.querySelector('.swal2-title').innerText.trim() : ''"),
                'content': self._js("document.querySelector('.swal2-html-container') ? document.querySelector('.swal2-html-container').innerText.trim() : ''"),
                'icon': self._js("document.querySelector('.swal2-icon') ? document.querySelector('.swal2-icon').className : ''"),
                'prompt': self._js("document.querySelector('.swal2-input') !== null && document.querySelector('.swal2-input').offsetParent !== null"),
                'prompt_value': self._js("document.querySelector('.swal2-input') ? document.querySelector('.swal2-input').value : ''"),
                'prompt_placeholder': self._js("document.querySelector('.swal2-input') ? document.querySelector('.swal2-input').placeholder : ''"),
                'has_confirm': self._js(v + "(document.querySelector('.swal2-confirm'))"),
                'has_cancel': self._js(v + "(document.querySelector('.swal2-cancel'))"),
                'has_deny': self._js(v + "(document.querySelector('.swal2-deny'))"),
                'has_close': self._js(v + "(document.querySelector('.swal2-close'))"),
                'confirm_text': self._js("document.querySelector('.swal2-confirm') ? document.querySelector('.swal2-confirm').innerText.trim() : 'OK'"),
                'cancel_text': self._js("document.querySelector('.swal2-cancel') ? document.querySelector('.swal2-cancel').innerText.trim() : 'Cancel'"),
                'deny_text': self._js("document.querySelector('.swal2-deny') ? document.querySelector('.swal2-deny').innerText.trim() : 'No'"),
                'visible': True,
            }

        # ── Bootstrap modal ─────────────────────────────────────
        if 'modal' in cls:
            return {
                'type': 'bootstrap_modal',
                'title': self._js("document.querySelector('.modal.show .modal-title, .modal.fade.in .modal-title') ? document.querySelector('.modal.show .modal-title, .modal.fade.in .modal-title').innerText.trim() : ''"),
                'content': self._js("document.querySelector('.modal.show .modal-body, .modal.fade.in .modal-body') ? document.querySelector('.modal.show .modal-body, .modal.fade.in .modal-body').innerText.trim() : ''"),
                'has_close': self._js("document.querySelector('.modal.show .close, .modal.fade.in .close') !== null"),
                'buttons': self._js("Array.from(document.querySelectorAll('.modal.show .modal-footer button, .modal.fade.in .modal-footer button')).map(function(b){return b.innerText.trim()})"),
                'visible': True,
            }

        # ── jQuery UI Dialog ────────────────────────────────────
        if 'ui-dialog' in cls:
            return {
                'type': 'jquery_ui',
                'title': self._js("document.querySelector('.ui-dialog .ui-dialog-title') ? document.querySelector('.ui-dialog .ui-dialog-title').innerText.trim() : ''"),
                'content': self._js("document.querySelector('.ui-dialog .ui-dialog-content') ? document.querySelector('.ui-dialog .ui-dialog-content').innerText.trim() : ''"),
                'buttons': self._js("Array.from(document.querySelectorAll('.ui-dialog .ui-dialog-buttonpane button')).map(function(b){return b.innerText.trim()})"),
                'visible': True,
            }

        # ── Material UI ─────────────────────────────────────────
        if 'MuiDialog' in cls:
            return {
                'type': 'material_ui',
                'title': self._js("document.querySelector('.MuiDialog-root .MuiDialogTitle-root') ? document.querySelector('.MuiDialog-root .MuiDialogTitle-root').innerText.trim() : ''"),
                'content': self._js("document.querySelector('.MuiDialog-root .MuiDialogContent-root') ? document.querySelector('.MuiDialog-root .MuiDialogContent-root').innerText.trim() : ''"),
                'buttons': self._js("Array.from(document.querySelectorAll('.MuiDialog-root .MuiDialogActions-root button')).map(function(b){return b.innerText.trim()})"),
                'visible': True,
            }

        # ── Ant Design ──────────────────────────────────────────
        if 'ant-modal' in cls:
            return {
                'type': 'antd',
                'title': self._js("document.querySelector('.ant-modal .ant-modal-title') ? document.querySelector('.ant-modal .ant-modal-title').innerText.trim() : ''"),
                'content': self._js("document.querySelector('.ant-modal .ant-modal-body') ? document.querySelector('.ant-modal .ant-modal-body').innerText.trim() : ''"),
                'buttons': self._js("Array.from(document.querySelectorAll('.ant-modal .ant-modal-footer button')).map(function(b){return b.innerText.trim()})"),
                'visible': True,
            }

        # ── Element UI ───────────────────────────────────────────
        if 'el-dialog' in cls:
            return {
                'type': 'element_ui',
                'title': self._js("document.querySelector('.el-dialog .el-dialog__title') ? document.querySelector('.el-dialog .el-dialog__title').innerText.trim() : ''"),
                'content': self._js("document.querySelector('.el-dialog .el-dialog__body') ? document.querySelector('.el-dialog .el-dialog__body').innerText.trim() : ''"),
                'buttons': self._js("Array.from(document.querySelectorAll('.el-dialog .el-dialog__footer button')).map(function(b){return b.innerText.trim()})"),
                'visible': True,
            }

        # ── Vuetify ──────────────────────────────────────────────
        if 'v-dialog' in cls:
            return {
                'type': 'vuetify',
                'title': self._js("document.querySelector('.v-dialog .v-card-title') ? document.querySelector('.v-dialog .v-card-title').innerText.trim() : ''"),
                'content': self._js("document.querySelector('.v-dialog .v-card-text') ? document.querySelector('.v-dialog .v-card-text').innerText.trim() : ''"),
                'buttons': self._js("Array.from(document.querySelectorAll('.v-dialog .v-card-actions button')).map(function(b){return b.innerText.trim()})"),
                'visible': True,
            }

        # ── Unknown / fallback ────────────────────────────────────
        return {'type': 'unknown', 'visible': True}

    def _vis_fn(self):
        return "(function(el){ return el && el.offsetParent !== null && window.getComputedStyle(el).display !== 'none' && el.offsetHeight > 0 })"

    # ── Public core API ──────────────────────────────────────────

    def is_open(self):
        """Returns True if any popup is currently visible."""
        return self._js("""
            (function(){
                var s = '.swal2-popup,.sweet-alert,.modal.show,.modal.fade.in,.modal[style*=\"display: block\"],.ui-dialog,.MuiDialog-root,.ant-modal,.el-dialog,.v-dialog';
                if (document.querySelector(s)) return true;
                var all = document.querySelectorAll('body > *');
                for (var i = 0; i < all.length; i++) {
                    var el = all[i];
                    if (el.offsetParent === null) continue;
                    var cs = window.getComputedStyle(el);
                    if (cs.position === 'fixed' || cs.position === 'absolute') {
                        var z = parseInt(cs.zIndex);
                        if (!isNaN(z) && z >= 100) return true;
                    }
                }
                return false;
            })()
        """)

    def info(self):
        """Return full info dict for the current popup, or None."""
        return self._detect_any_popup()

    def alert(self, info=None):
        """Print the popup in a nice ASCII box in the terminal."""
        if info is None:
            info = self._detect_any_popup()
        if info is None:
            print("No popup currently visible")
            return

        t = info.get('type', 'unknown')
        if t in ('sweetalert2',):
            self._render_swal2(info)
        elif t == 'native':
            self._render_native()
        else:
            self._render_generic(info)

    def _render_swal2(self, info):
        width = max(50, min(len(info.get('content', '')) + 10, 70))
        lines = ['+' + '-' * width + '+']
        lines.append('|' + ' ' * width + '|')

        title = info.get('title', '')
        if title:
            t = '⚠ ' + title if 'warning' in info.get('icon', '') else title
            pad = (width - len(t)) // 2
            lines.append('|' + ' ' * pad + t + ' ' * (width - pad - len(t)) + '|')
            lines.append('|' + ' ' * width + '|')

        content = info.get('content', '')
        for line in self._word_wrap(content, width - 4):
            lines.append('|' + '  ' + line + ' ' * (width - 2 - len(line)) + '|')
        lines.append('|' + ' ' * width + '|')

        if info.get('prompt'):
            ph = info.get('prompt_placeholder') or info.get('prompt_value') or '...'
            inp = '[ ' + ph + ' ]'
            pad = (width - len(inp)) // 2
            lines.append('|' + ' ' * pad + inp + ' ' * (width - pad - len(inp)) + '|')
            lines.append('|' + ' ' * width + '|')

        btn_parts = []
        if info.get('has_confirm'):  btn_parts.append('[' + info.get('confirm_text', 'OK') + ']')
        if info.get('has_cancel'):   btn_parts.append('[' + info.get('cancel_text', 'Cancel') + ']')
        if info.get('has_deny'):     btn_parts.append('[' + info.get('deny_text', 'No') + ']')
        if btn_parts:
            btn_line = ' '.join(btn_parts)
            pad = (width - len(btn_line)) // 2
            lines.append('|' + ' ' * pad + btn_line + ' ' * (width - pad - len(btn_line)) + '|')

        lines.append('|' + ' ' * width + '|')
        lines.append('+' + '-' * width + '+')
        for l in lines:
            print(l)
        if info.get('prompt'):
            print(">> Popup has an input field — use popup.fill('text') to type")

    def _render_native(self):
        print('+' + '-' * 50 + '+')
        print('|' + ' ' * 50 + '|')
        print('|' + ' ' * 14 + 'NATIVE BROWSER DIALOG' + ' ' * 14 + '|')
        print('|' + ' ' * 50 + '|')
        print('|' + ' ' * 8 + 'Use popup.accept() or popup.dismiss()' + ' ' * 7 + '|')
        print('|' + ' ' * 50 + '|')
        print('+' + '-' * 50 + '+')

    def _render_generic(self, info):
        title = info.get('title', 'POPUP')
        content = info.get('content', '')
        buttons = info.get('buttons', [])
        width = max(50, min(max(len(content), len(title)) + 10, 72))

        lines = ['+' + '-' * width + '+']
        lines.append('|' + ' ' * width + '|')
        pad = (width - len(title)) // 2
        lines.append('|' + ' ' * pad + title + ' ' * (width - pad - len(title)) + '|')
        lines.append('|' + ' ' * width + '|')
        for line in self._word_wrap(content, width - 4):
            lines.append('|' + '  ' + line + ' ' * (width - 2 - len(line)) + '|')
        lines.append('|' + ' ' * width + '|')
        if buttons:
            btn_line = '  '.join('[' + b + ']' for b in buttons)
            if len(btn_line) <= width:
                pad = (width - len(btn_line)) // 2
                lines.append('|' + ' ' * pad + btn_line + ' ' * (width - pad - len(btn_line)) + '|')
            else:
                for b in buttons:
                    item = '[' + b + ']'
                    pad = (width - len(item)) // 2
                    lines.append('|' + ' ' * pad + item + ' ' * (width - pad - len(item)) + '|')
        lines.append('|' + ' ' * width + '|')
        lines.append('+' + '-' * width + '+')
        for l in lines:
            print(l)

    def _word_wrap(self, text, max_width):
        words = text.split(' ')
        lines = []
        line = ''
        for word in words:
            test = (line + ' ' + word).strip()
            if len(test) > max_width:
                if line:
                    lines.append(line)
                line = word
            else:
                line = test
        if line:
            lines.append(line)
        return lines

    def _wait(self, seconds=0.5):
        _time.sleep(seconds)

    # ── Actions ──────────────────────────────────────────────────

    def _click_selector(self, selector):
        """Click the first element matching a CSS selector."""
        q = selector.replace("'", "\\'")
        self._js("document.querySelector('" + q + "')?.click()")
        self._wait()

    def click_ok(self):
        """Click the primary confirm/OK button (SweetAlert2)."""
        self._click_selector('.swal2-confirm')

    def click_cancel(self):
        """Click the cancel button (SweetAlert2)."""
        self._click_selector('.swal2-cancel')

    def click_deny(self):
        """Click the deny/no button (SweetAlert2)."""
        self._click_selector('.swal2-deny')

    def click(self, text=None):
        """Click any button by text, or the primary action button."""
        if text:
            safe = text.replace("'", "\\'")
            self._js("Array.from(document.querySelectorAll('button')).find(function(b){ return b.innerText.includes('" + safe + "') })?.click()")
        else:
            # Try most common primary buttons
            self._js("""
                (function(){
                    var s = '.swal2-confirm,.modal.show .modal-footer .btn-primary,.modal.fade.in .modal-footer .btn-primary,.MuiDialogActions-root button:first-child,.ant-modal-footer button:first-child,.el-dialog__footer .el-button--primary,.v-card-actions button:first-child,.ui-dialog-buttonpane button:first-child';
                    var el = document.querySelector(s);
                    if (el) { el.click(); return true; }
                    var btns = Array.from(document.querySelectorAll('button'));
                    var primary = btns.find(function(b){ return b.innerText.toLowerCase().includes('ok') || b.innerText.toLowerCase().includes('confirm') || b.innerText.toLowerCase().includes('yes') });
                    if (primary) { primary.click(); return true; }
                    if (btns.length > 0) { btns[0].click(); return true; }
                    return false;
                })()
            """)
        self._wait()

    def fill(self, text):
        """Type text into a SweetAlert2 prompt input."""
        safe = text.replace("'", "\\'").replace('"', '\\"')
        self._js("""
            (function(){
                var inp = document.querySelector('.swal2-input');
                if (!inp) return false;
                inp.value = '""" + safe + """';
                inp.dispatchEvent(new Event('input', {bubbles:true}));
                inp.dispatchEvent(new Event('change', {bubbles:true}));
                return true;
            })()
        """)
        self._wait(0.2)

    def dismiss(self):
        """Dismiss popup — close button if available, else confirm."""
        self._js("""
            (function(){
                var close = document.querySelector('.swal2-close') || document.querySelector('.modal .close') || document.querySelector('.ui-dialog .ui-dialog-titlebar-close');
                if (close) { close.click(); return true; }
                var confirm = document.querySelector('.swal2-confirm');
                if (confirm) { confirm.click(); return true; }
                var btns = Array.from(document.querySelectorAll('button'));
                if (btns.length) { btns[btns.length - 1].click(); return true; }
                return false;
            })()
        """)
        self._wait()

    def close(self):
        """Close popup via its close/X button only."""
        self._js("""
            (function(){
                var close = document.querySelector('.swal2-close') || document.querySelector('.modal .close') || document.querySelector('.ui-dialog .ui-dialog-titlebar-close') || document.querySelector('.ant-modal-close') || document.querySelector('.el-dialog__headerbtn');
                if (close) { close.click(); return true; }
                return false;
            })()
        """)
        self._wait()

    # ── Native dialog handling (alert/confirm/prompt) ─────────

    def accept(self, prompt_text=None):
        """Accept/confirm a native browser dialog (alert/confirm/prompt)."""
        try:
            args = {'accept': True}
            if prompt_text is not None:
                args['prompt_text'] = str(prompt_text)
            self._browser._drission.handle_alert(**args)
            return True
        except Exception:
            # Fallback: try clicking known OK buttons
            self.click_ok()
            return False

    def reject(self):
        """Dismiss/cancel a native browser dialog."""
        try:
            self._browser._drission.handle_alert(accept=False)
            return True
        except Exception:
            return False

    # ── Blocking ────────────────────────────────────────────────

    def block(self):
        """Override native dialogs + SweetAlert2 to suppress popups."""
        self._js("""
            (function(){
                if (window.__popup_blocked) return;
                window.__popup_blocked = true;

                // Native dialogs
                window.__native_alert = window.alert;
                window.__native_confirm = window.confirm;
                window.__native_prompt = window.prompt;
                window.alert = function(){};
                window.confirm = function(){ return true; };
                window.prompt = function(){ return ''; };

                // SweetAlert2
                if (window.Swal) {
                    window.__origSwal = window.Swal;
                    window.Swal = function(){ return Promise.resolve({isConfirmed:false,isDenied:false,isDismissed:true}); };
                }
                if (window.swal) {
                    window.__origSwal2 = window.swal;
                    window.swal = function(){ return Promise.resolve({isConfirmed:false,isDenied:false,isDismissed:true}); };
                }
            })()
        """)
        print("Popups blocked: native dialogs + SweetAlert2 intercepted")

    def unblock(self):
        """Restore original popup behaviour (reloads the page)."""
        self._js("""
            (function(){
                if (window.__native_alert) window.alert = window.__native_alert;
                if (window.__native_confirm) window.confirm = window.__native_confirm;
                if (window.__native_prompt) window.prompt = window.__native_prompt;
                if (window.__origSwal) window.Swal = window.__origSwal;
                if (window.__origSwal2) window.swal = window.__origSwal2;
                window.__popup_blocked = false;
            })()
        """)
        print("Popups unblocked")

    # ── Convenience aliases ──────────────────────────────────────

    def confirm(self):
        self.click_ok()

    def cancel(self):
        self.click_cancel()

    def deny(self):
        self.click_deny()

    def watch(self):
        return self._detect_any_popup()

    def wait_and_watch(self, seconds=2):
        _time.sleep(float(seconds))
        return self._detect_any_popup()

    def is_error(self):
        info = self._detect_any_popup()
        if info is None:
            return False
        if info.get('type') == 'sweetalert2':
            t = info.get('title', '').lower()
            c = info.get('content', '').lower()
            icon = info.get('icon', '').lower()
            return any(kw in icon for kw in ['warning', 'error']) or any(kw in t for kw in ['warning', 'error']) or any(kw in c for kw in ['wrong', 'fail', 'invalid', 'error'])
        return False

    def is_success(self):
        info = self._detect_any_popup()
        if info is None:
            return False
        if info.get('type') == 'sweetalert2':
            t = info.get('title', '').lower()
            c = info.get('content', '').lower()
            return any(kw in t for kw in ['success', 'done']) or any(kw in c for kw in ['success', 'saved', 'welcome'])
        return False
