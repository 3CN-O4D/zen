import threading
import json
import urllib.request
import urllib.parse
import websocket
import time
import subprocess
import tempfile
import os


def _http(path, port=9222):
    return json.loads(urllib.request.urlopen(f'http://127.0.0.1:{port}{path}').read())


def _connect_ws(port=9222):
    info = _http('/json/version', port)
    return websocket.create_connection(info['webSocketDebuggerUrl'], timeout=10)


def _send(ws, method, params=None, session_id=None):
    global _ws_msg_id
    if not hasattr(_send, 'counter'):
        _send.counter = 0
    _send.counter += 1
    mid = _send.counter
    msg = {'id': mid, 'method': method}
    if params: msg['params'] = params
    if session_id: msg['sessionId'] = session_id
    ws.send(json.dumps(msg))
    return mid


def _recv(ws, target_id, timeout=30):
    end = time.time() + timeout
    while time.time() < end:
        ws.settimeout(max(0.5, end - time.time()))
        try:
            raw = ws.recv()
        except websocket.WebSocketTimeoutException:
            continue
        msg = json.loads(raw)
        if msg.get('id') == target_id:
            return msg.get('result')
    raise TimeoutError('CDP command timed out')


def _extract_cookies_cdp(port=9222, domain=None):
    ws = _connect_ws(port)
    tabs = _http('/json', port)
    target = None
    for t in tabs:
        if t.get('type') == 'page' and t.get('url', '').startswith('http'):
            target = t
            break
    if not target and tabs:
        for t in tabs:
            if t.get('type') == 'page':
                target = t
                break
    if not target:
        mid = _send(ws, 'Target.createTarget', {'url': 'about:blank'})
        result = _recv(ws, mid)
        target_id = result['targetId']
        mid = _send(ws, 'Target.attachToTarget', {'targetId': target_id, 'flatten': True})
        result = _recv(ws, mid)
        session_id = result['sessionId']
        mid = _send(ws, 'Network.enable', session_id=session_id)
        _recv(ws, mid)
        mid = _send(ws, 'Network.getAllCookies', session_id=session_id)
        result = _recv(ws, mid)
        cookies = result.get('cookies', []) if result else []
        ws.close()
        return [c for c in cookies if not domain or domain in c.get('domain', '')]
    target_id = target['id']
    mid = _send(ws, 'Target.attachToTarget', {'targetId': target_id, 'flatten': True})
    result = _recv(ws, mid)
    session_id = result['sessionId']
    mid = _send(ws, 'Network.enable', session_id=session_id)
    _recv(ws, mid)
    mid = _send(ws, 'Network.getAllCookies', session_id=session_id)
    result = _recv(ws, mid)
    cookies = result.get('cookies', []) if result else []
    ws.close()
    return [c for c in cookies if not domain or domain in c.get('domain', '')]


def _extract_cookies_file(profile_dir, domain=None):
    db_path = os.path.join(profile_dir, 'Cookies')
    if not os.path.exists(db_path):
        profile_dir = os.path.join(profile_dir, 'Default')
        db_path = os.path.join(profile_dir, 'Cookies')
    if not os.path.exists(db_path):
        raise FileNotFoundError(f'Cookies database not found in {profile_dir}')
    try:
        import sqlite3
    except ImportError:
        raise RuntimeError('sqlite3 module required for file-based extraction')
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()
    query = 'SELECT host_key, name, value, path, expires_utc, is_secure, is_httponly, samesite, has_expires, creation_utc, last_access_utc FROM cookies'
    if domain:
        query += f' WHERE host_key LIKE "%{domain}%"'
    cur.execute(query)
    cookies = []
    for row in cur.fetchall():
        cookies.append({
            'name': row['name'],
            'value': row['value'],
            'domain': row['host_key'],
            'path': row['path'],
            'secure': bool(row['is_secure']),
            'httponly': bool(row['is_httponly']),
            'samesite': row['samesite'],
            'expires': row['expires_utc'],
        })
    conn.close()
    return cookies


def extract(domain=None):
    try:
        return _extract_cookies_cdp(domain=domain)
    except Exception as e:
        raise RuntimeError(
            f'CDP extraction failed: {e}\n'
            '  Make sure Chromium is running with --remote-debugging-port=9222 --remote-allow-origins=*\n'
            '  Or use cookies.from_path("/path/to/chromium/profile") instead.'
        )


def from_path(profile_or_bin, domain=None):
    if os.path.isfile(profile_or_bin):
        bin_path = profile_or_bin
        port = _find_free_port()
        proc = subprocess.Popen(
            [bin_path, f'--remote-debugging-port={port}', '--remote-allow-origins=*',
             '--headless=new', '--no-first-run', '--user-data-dir=' + tempfile.mkdtemp()],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        try:
            for _ in range(30):
                time.sleep(0.5)
                try:
                    info = _http('/json/version', port)
                    break
                except Exception:
                    continue
            else:
                raise TimeoutError('Browser did not start')
            ws = _connect_ws(port)
            mid = _send(ws, 'Target.createTarget', {'url': 'about:blank'})
            result = _recv(ws, mid)
            target_id = result['targetId']
            mid = _send(ws, 'Target.attachToTarget', {'targetId': target_id, 'flatten': True})
            result = _recv(ws, mid)
            session_id = result['sessionId']
            mid = _send(ws, 'Network.enable', session_id=session_id)
            _recv(ws, mid)
            mid = _send(ws, 'Network.getAllCookies', session_id=session_id)
            result = _recv(ws, mid)
            cookies = result.get('cookies', []) if result else []
            ws.close()
            return [c for c in cookies if not domain or domain in c.get('domain', '')]
        finally:
            try:
                proc.kill()
            except Exception:
                pass
    elif os.path.isdir(profile_or_bin):
        return _extract_cookies_file(profile_or_bin, domain)
    else:
        raise FileNotFoundError(f'Path not found: {profile_or_bin}')


def _find_free_port():
    import socket
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(('', 0))
        return s.getsockname()[1]


def _build_cookies_module():
    return {
        'extract': lambda domain=None: extract(domain),
        'from_path': lambda path, domain=None: from_path(path, domain),
    }
