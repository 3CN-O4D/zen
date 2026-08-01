#!/usr/bin/env python3
import requests, json, time, os, sys, shutil, re
from datetime import datetime, timedelta
from pathlib import Path

POLL_SECS = 20
MONITOR_POLL_SECS = 10
MATCH_DURATION_SECS = 180
TARGET = 50
SCRIPT_DIR = Path(__file__).resolve().parent
DATA_DIR = Path(os.environ.get('BETIKA_DATA_DIR') or (SCRIPT_DIR / 'betika_data'))
os.makedirs(DATA_DIR, exist_ok=True)

API_BASE = 'https://api.betika.com/v1/'
MATCHES_API = 'https://virtuals.betika.com/v1/matches'
PLACEBET_API = 'https://live.betika.com/v1/virtual/placebet'

LEAGUES = {
    6: 'English', 7: 'Italian', 22: 'French',
    24: 'German', 26: 'Sakata',
    27: 'England Mode', 28: 'Bundesliga'
}

PICK_LABEL = {'1': 'HOME', 'X': 'DRAW', '2': 'AWAY'}

class BetikaBot:
    def __init__(self, phone, password, dry_run=True):
        self.phone = phone
        self.password = password
        self.dry_run = dry_run
        self.token = None
        self.user_id = None
        self.session = requests.Session()
        self.round = 0
        self.all_rounds = []
        self.bankroll = 0
        self.peak = 0
        self.config = {
            'bets_per_round': 3,
            'stake': 5,
            'min_odds': 1.40,
            'min_confidence': 55,
            'min_edge': -0.10,
            'max_exposure': 0.5,
            'recovery': True,
            'recovery_multiplier': 3.0,
            'auto_stake': True,
            'stake_step': 1.0,
            'dd_stop': True,
            'wait_low_balance': False,
            'max_stake': 10.0,
            'ramp_threshold': 5.0,
            'max_odds': 0.0,
            'away_only': False,
            'no_bets_after': '',
            'low_bal_threshold': 10.0,
            'low_bal_exposure': 0.25,
            'min_stake': 1.0,
            'micro': False,
            'withdraw_amount': 0.0,
            'withdraw_at': 0.0,
        }
        self.start_bankroll = 0
        self.starting_balance = 0.0
        self.base_stake = 5.0
        self.recovery_stake = 0.0
        self.placed_ids = set()
        self.stop = []
        self.cum_wins = 0
        self.cum_losses = 0
        self.cum_loss_amt = 0.0
        self.withdrawn_total = 0.0
        self._fresh_matches = None
        self._tty = sys.stdout.isatty()
        self._ncols = 2
        self._cell_w = 34

    SESSION_FILE = DATA_DIR / 'session.json'

    def save_session(self):
        cookies = self.session.cookies.get_dict()
        data = {'token': self.token, 'user_id': self.user_id, 'cookies': cookies}
        with open(self.SESSION_FILE, 'w') as f:
            json.dump(data, f, indent=2)

    def load_session(self):
        if not self.SESSION_FILE.exists():
            return False
        try:
            with open(self.SESSION_FILE) as f:
                data = json.load(f)
            self.token = data.get('token')
            self.user_id = data.get('user_id')
            self.session.cookies.update(data.get('cookies', {}))
            if self.token:
                self.session.headers['Authorization'] = f'Bearer {self.token}'
                bal, _ = self.get_balance()
                if bal is None:
                    self.warmup()
                    bal, _ = self.get_balance()
                if bal is not None:
                    self.save_session()
                    return True
        except: pass
        return False

    def warmup(self):
        self.session.headers.update({
            'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
            'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
            'Accept-Language': 'en-US,en;q=0.9',
        })
        r = self.session.get('https://www.betika.com/en-ke', timeout=30)
        if r.status_code != 200:
            return False
        time.sleep(5)
        r2 = self.session.get('https://www.betika.com/en-ke/virtuals/', timeout=30)
        time.sleep(3)
        return r2.status_code == 200

    def login(self):
        self.session.headers.update({
            'Origin': 'https://www.betika.com',
            'Referer': 'https://www.betika.com/en-ke/virtuals/',
            'Content-Type': 'application/json',
            'Accept': 'application/json, text/plain, */*',
        })
        r = self.session.post(API_BASE + 'login', json={
            'mobile': self.phone, 'password': self.password,
            'remember': True, 'src': 'MOBILE_WEB',
        }, timeout=30)
        if r.status_code != 200:
            return False, r.text[:100]
        data = r.json()
        self.token = data['token']
        self.user_id = data['data']['user']['id']
        self.session.headers['Authorization'] = f'Bearer {self.token}'
        self.save_session()
        return True, None

    def get_balance(self):
        for attempt in range(3):
            try:
                r = self.session.post(API_BASE + 'balance', json={'token': self.token}, timeout=10)
                if r.status_code == 200:
                    bal = r.json()['data']
                    return bal['balance'], bal['bonus']
                return None, None
            except (requests.exceptions.ConnectionError, requests.exceptions.Timeout):
                if attempt < 2:
                    self.p('    balance fetch failed, retrying...')
                    time.sleep(2 * (attempt + 1))
        return None, None

    def withdraw(self, amount):
        """Direct M-Pesa withdrawal (no Cashia). Fires an STK push the user
        approves on the phone. Returns (ok, message)."""
        body = {'amount': float(amount), 'token': self.token, 'app_name': 'MOBILE_WEB'}
        for attempt in range(3):
            try:
                r = self.session.post(API_BASE + 'withdraw', json=body, timeout=15)
                break
            except (requests.exceptions.ConnectionError, requests.exceptions.Timeout) as e:
                last_err = e
                self.p(f'    withdraw network error, retrying ({attempt + 1}/3)...')
                time.sleep(3 * (attempt + 1))
        else:
            return False, f'network error: {last_err}'
        try:
            data = r.json()
        except Exception:
            return False, f'bad response ({r.status_code})'
        if r.status_code in (200, 201) and data.get('success'):
            return True, data.get('success', {}).get('message', 'withdrawal initiated')
        err = data.get('error') or {}
        msg = err.get('message') or data.get('message') or str(data)
        return False, msg

    def deposit(self, amount):
        """Direct M-Pesa deposit. Fires an STK push the user approves on the
        phone. Returns (ok, message)."""
        body = {'src': 'MOBILE_WEB', 'amount': float(amount), 'token': self.token}
        for attempt in range(3):
            try:
                r = self.session.post(API_BASE + 'deposit', json=body, timeout=15)
                break
            except (requests.exceptions.ConnectionError, requests.exceptions.Timeout) as e:
                last_err = e
                self.p(f'    deposit network error, retrying ({attempt + 1}/3)...')
                time.sleep(3 * (attempt + 1))
        else:
            return False, f'network error: {last_err}'
        try:
            data = r.json()
        except Exception:
            return False, f'bad response ({r.status_code})'
        if r.status_code in (200, 201) and data.get('success'):
            return True, data.get('success', {}).get('message', 'deposit initiated')
        err = data.get('error') or {}
        msg = err.get('message') or data.get('message') or str(data)
        return False, msg

    def fetch_all_matches(self):
        parsed = {}
        for lid in LEAGUES:
            try:
                r = self.session.get(MATCHES_API, params={'competition_id': lid}, timeout=10)
                if r.status_code == 200:
                    for matches in r.json()['data'].values():
                        for m in matches:
                            mid = m.get('parent_virtual_id', '')
                            if mid and mid not in parsed:
                                parsed[mid] = m
            except:
                pass
        return list(parsed.values())

    @staticmethod
    def compute_probs(odds):
        inv = {k: 1/v for k, v in odds.items()}
        total = sum(inv.values())
        return {k: round(v/total*100, 1) for k, v in inv.items()}

    @staticmethod
    def _match_level_odds(match):
        odds_1x2 = {}
        for display, odd_field, key_field, outcome_id in (
            ('1', 'home_odd', 'home_team', 1),
            ('X', 'neutral_odd', 'draw', 2),
            ('2', 'away_odd', 'away_team', 3),
        ):
            odd = match.get(odd_field)
            if odd is None:
                continue
            odds_1x2[display] = {
                'odd': float(odd),
                'outcome_id': str(outcome_id),
                'sub_type_id': str(match.get('sub_type_id') or 1),
                'special_bet_value': match.get('special_bet_value', ''),
                'odd_key': match.get(key_field),
            }
        return odds_1x2

    def predict(self, match):
        odds_1x2 = {}
        for market in match.get('markets', []):
            if market['name'] == '1X2':
                for odd in market['odds']:
                    odds_1x2[odd['display']] = {
                        'odd': float(odd['odd_value']),
                        'outcome_id': odd['outcome_id'],
                        'sub_type_id': odd['sub_type_id'],
                        'special_bet_value': odd.get('special_bet_value', ''),
                        'odd_key': odd['odd_key'],
                    }
        if len(odds_1x2) != 3:
            odds_1x2 = self._match_level_odds(match)
        if len(odds_1x2) != 3:
            return None
        probs = self.compute_probs({k: v['odd'] for k, v in odds_1x2.items()})
        best = max(probs.items(), key=lambda x: x[1])
        return {
            'home': match['home_team'],
            'away': match['away_team'],
            'league': match.get('competition_name', ''),
            'id': match['parent_virtual_id'],
            'pick': best[0],
            'confidence': best[1],
            'odd_value': odds_1x2[best[0]]['odd'],
            'odds': {k: v['odd'] for k, v in odds_1x2.items()},
            'probs': probs,
            'bet_data': odds_1x2[best[0]],
            'start_time': match.get('start_time', ''),
            'remaining_time': match.get('remaining_time', ''),
            'season': match.get('season', ''),
            'match_day': match.get('match_day', ''),
        }

    def place_bet(self, prediction, stake):
        body = {
            'bet_string': 'MOBILE_WEB',
            'app_name': 'MOBILE_WEB',
            'src': 'MOBILE_WEB',
            'possible_win': round(stake * prediction['odd_value'], 2),
            'profile_id': self.user_id,
            'stake': stake,
            'total_odd': prediction['odd_value'],
            'token': self.token,
            'betslip': [{
                'sub_type_id': prediction['bet_data']['sub_type_id'],
                'special_bet_value': prediction['bet_data']['special_bet_value'],
                'bet_pick': prediction['bet_data']['odd_key'],
                'odd_value': prediction['bet_data']['odd'],
                'outcome_id': prediction['bet_data']['outcome_id'],
                'parent_virtual_id': prediction['id'],
                'parent_match_id': prediction['id'],
                'match_id': prediction['id'],
                'bet_type': 0,
            }],
            'deviceID': '', 'endCustomerIP': '', 'channelID': 'mobile',
            'user_agent': 'Mozilla/5.0',
        }
        if self.dry_run:
            label = PICK_LABEL.get(prediction['pick'], prediction['pick'])
            self.p(f"[DRY] Bet KES {stake} @ {prediction['odd_value']} on {label} ({prediction['confidence']}%)")
            return 'dry_run', ''
        r = None
        last_err = None
        for attempt in range(3):
            try:
                r = self.session.post(PLACEBET_API, json=body, timeout=15)
                break
            except (requests.exceptions.ConnectionError, requests.exceptions.Timeout) as e:
                last_err = e
                self.p(f'    bet placement network error, retrying ({attempt + 1}/3)...')
                time.sleep(3 * (attempt + 1))
        else:
            self.p(f'    bet placement failed after 3 attempts: {last_err}')
            return 'network_error', ''
        if r.status_code in (200, 201):
            bet_id = ''
            try:
                msg = r.json().get('message', '')
                if 'Bet ID' in msg:
                    bet_id = msg.split('Bet ID')[1].split('.')[0].strip()
            except: pass
            return 'success', bet_id
        elif r.status_code == 421:
            msg = r.json().get('message', '')
            self.p(f'    [place_bet 421] {msg}')
            return ('duplicate', '') if 'similar' in msg.lower() else ('insufficient_balance', '')
        else:
            try:
                msg = r.json().get('message', '')
            except: msg = ''
            self.p(f'    [place_bet {r.status_code}] {msg}')
            return 'error', ''

    @staticmethod
    def match_phase(start_time):
        try:
            t0 = datetime.strptime(start_time, '%Y-%m-%d %H:%M:%S')
        except Exception:
            return 'PLAY'
        diff = (datetime.now() - t0).total_seconds()
        if diff < 0:
            return 'PRE'
        return 'PLAY' if diff < MATCH_DURATION_SECS else 'END'

    def match_progress(self, start_time):
        try:
            t0 = datetime.strptime(start_time, '%Y-%m-%d %H:%M:%S')
        except Exception:
            return '--:--', 'E 0:00', 0.0
        est = (t0 + timedelta(seconds=MATCH_DURATION_SECS)).strftime('%H:%M')
        diff = (datetime.now() - t0).total_seconds()
        if diff < 0:
            s = int(-diff)
            return est, f'S {s // 60}:{s % 60:02d}', 0.0
        if diff >= MATCH_DURATION_SECS:
            return est, 'E 0:00', 1.0
        left = int(MATCH_DURATION_SECS - diff)
        return est, f'E {left // 60}:{left % 60:02d}', diff / MATCH_DURATION_SECS

    @staticmethod
    def progress_bar(frac, width):
        width = max(width, 4)
        filled = min(max(int(round(frac * width)), 0), width)
        return '█' * filled + '░' * (width - filled)

    def money_bar(self, width=46):
        scale = max(self.peak, self.start_bankroll, TARGET, 1.0)
        pos = lambda v: min(max(int(round(v / scale * (width - 1))), 0), width - 1)
        chars = ['░'] * width
        cur = pos(self.bankroll)
        for i in range(cur + 1):
            chars[i] = '█'
        for ch, v in (('T', TARGET), ('S', self.start_bankroll), ('P', self.peak)):
            chars[pos(v)] = ch
        chars[pos(self.bankroll)] = 'C'
        return ''.join(chars)

    def _grid_layout(self):
        width = max(shutil.get_terminal_size((80, 24)).columns, 60)
        ncols = 3 if width >= 126 else 2
        cell_w = (width - 16) // 3 if ncols == 3 else (width - 12) // 2
        cell_w = min(max(cell_w, 24), 42)
        return ncols, cell_w

    def bet_cell(self, b, idx):
        est, clock, frac = self.match_progress(b.get('start_time'))
        label = PICK_LABEL.get(b['pick'], b['pick'])
        result = b.get('result', 'PLACE')
        result_str = {'PLACE': 'PLACED', 'MONITOR': '● ACTIVE', 'WON': '✓ WON', 'LOST': '✗ LOST'}.get(result, result)
        if result == 'WON':
            result_str += f' +{b["stake"] * b["odd_value"]:.2f}'
        lg = b.get('league', '')[:7]
        return [
            f'#{idx} {lg:<7} {clock}  ~{est}',
            f'{b["home"][:11]:11s} vs {b["away"][:11]:11s}',
            f'{label:<4} @{b["odd_value"]:.2f} c{b.get("confidence", 0):.0f}% st{b["stake"]:.1f}',
            f'[{self.progress_bar(frac, self._cell_w - 2)}]',
            f'{result_str}',
        ]

    def render_grid(self, cells):
        ncols, cell_w = self._grid_layout()
        pad = lambda s: s[:cell_w].ljust(cell_w)
        maxlines = max(len(c) for c in cells)
        seg = '─' * (cell_w + 2)
        lines = ['  ┌' + '┬'.join(seg for _ in range(ncols)) + '┐']
        for i in range(0, len(cells), ncols):
            row = [cells[j] for j in range(i, min(i + ncols, len(cells)))]
            for li in range(maxlines):
                parts = [pad(c[li] if li < len(c) else '') for c in row]
                lines.append('  │ ' + ' │ '.join(parts) + ' │')
        lines.append('  └' + '┴'.join(seg for _ in range(ncols)) + '┘')
        return lines

    def print_bets_table(self, bets, phase):
        label_phase = {'PLACE': 'PLACING BETS', 'MONITOR': 'MONITORING', 'DONE': 'ROUND COMPLETE'}.get(phase, phase)
        self._ncols, self._cell_w = self._grid_layout()
        cells = [self.bet_cell(b, i) for i, b in enumerate(bets, 1)]
        print(f'  ═══ {label_phase} ({len(bets)} bet{"s" if len(bets) != 1 else ""}) ═══')
        for line in self.render_grid(cells):
            print(line)

    def print_summary(self, bets, bal):
        wins = sum(1 for b in bets if b.get('result') == 'WON')
        losses = sum(1 for b in bets if b.get('result') == 'LOST')
        active = sum(1 for b in bets if b.get('result') in ('MONITOR', None))
        total_stake = sum(b['stake'] for b in bets)
        staked = sum(b['stake'] for b in bets if b.get('result') in ('WON', 'LOST'))
        returns = sum(b['stake'] * b['odd_value'] for b in bets if b.get('result') == 'WON')
        pnl = returns - staked
        self.p(f'  Bets: {len(bets)} | Won: {wins} | Lost: {losses} | Active: {active}')
        self.p(f'  Staked: KES {total_stake} | Returns: KES {returns:.2f} | P&L: KES {pnl:+.2f}')
        self.p(f'  Balance: KES {bal:.2f} | Bankroll: KES {self.bankroll:.2f}')

    def get_bet_statuses(self):
        try:
            body = {
                'page': '1', 'limit': '200', 'period': 'MONTH',
                'product': 'VIRTUAL', 'profile_id': self.user_id, 'token': self.token,
            }
            r = self.session.post(API_BASE + 'uo/bethistory', json=body, timeout=10)
            if r.status_code != 200:
                return {}
            statuses = {}
            for b in r.json().get('bets', []):
                st = (b.get('betStatus') or {}).get('text', '')
                if st:
                    statuses[str(b['bet_id'])] = st.upper()
            return statuses
        except:
            return {}

    def active_bet_match_ids(self):
        try:
            body = {
                'page': '1', 'limit': '200', 'period': 'MONTH',
                'product': 'VIRTUAL', 'profile_id': self.user_id, 'token': self.token,
            }
            r = self.session.post(API_BASE + 'uo/bethistory', json=body, timeout=10)
            if r.status_code != 200:
                return set()
            ids = set()
            for b in r.json().get('bets', []):
                if ((b.get('betStatus') or {}).get('text', '')).upper() == 'OPEN':
                    mid = (b.get('bet_message') or '').split('#')[0]
                    if mid.isdigit():
                        ids.add(mid)
            return ids
        except:
            return set()

    def upcoming_matches(self, matches, n=5):
        now = datetime.now()
        out = []
        for m in matches:
            st = m.get('start_time', '')
            try:
                t0 = datetime.strptime(st, '%Y-%m-%d %H:%M:%S')
            except Exception:
                continue
            diff = int((t0 - now).total_seconds())
            if 0 <= diff <= 300:
                lg = (m.get('competition_name') or '').replace('Virtual Football ', '')[:10]
                out.append((diff, f'{lg} in {diff // 60}:{diff % 60:02d}'))
        out.sort()
        return [x[1] for x in out[:n]]

    def log_bet_data(self, b):
        try:
            rec = {
                'bet_id': b.get('bet_id'), 'match_id': b.get('id'),
                'placed_at': b.get('placed_at'), 'settled_at': b.get('settled_at'),
                'league': b.get('league'), 'home': b.get('home'), 'away': b.get('away'),
                'start_time': b.get('start_time'), 'remaining_at_place': b.get('remaining_time'),
                'season': b.get('season'), 'match_day': b.get('match_day'),
                'pick': b.get('pick'), 'pick_odd': b.get('odd_value'),
                'odds_1x2': b.get('odds_1x2'),
                'confidence': b.get('confidence'), 'stake': b.get('stake'),
                'result': b.get('result'), 'payout': b.get('payout'),
                'balance_after': b.get('balance'),
            }
            with open(DATA_DIR / 'bet_data.jsonl', 'a') as f:
                f.write(json.dumps(rec) + '\n')
        except: pass

    def monitor_lines(self, bets, pending):
        settled = len(bets) - len(pending)
        self._ncols, self._cell_w = self._grid_layout()
        cells = [self.bet_cell(b, i) for i, b in enumerate(bets, 1)]
        out = [
            f'  ━━━━━ ROUND {self.round} — MONITORING ({settled}/{len(bets)} settled) ━━━━━',
            f'  Balance: KES {self.bankroll:.2f}  Start: {self.start_bankroll:.2f}  Peak: {self.peak:.2f}  Target: {TARGET:.0f}',
            f'  [{self.money_bar()}]',
            '    S=start  P=peak  C=current  T=target',
            '',
        ]
        out += self.render_grid(cells)
        if pending:
            out += ['', f'  ⏳ Waiting on {len(pending)} bet(s)... polls every {POLL_SECS}s']
        return out

    def draw_monitor(self, bets, pending, bal, force=False):
        if self._tty:
            print('\033[2J\033[H', end='')
            print('\n'.join(self.monitor_lines(bets, pending)), flush=True)
        elif force:
            print('\n'.join(self.monitor_lines(bets, pending)), flush=True)
        else:
            clocks = '  '.join(f'#{i} {self.match_progress(b.get("start_time"))[1]}' for i, b in enumerate(bets, 1))
            self.p(f'  ⏳ R{self.round} Bal KES {self.bankroll:.2f} [{self.money_bar(30)}] {clocks}')

    def monitor_bets(self, bets):
        pending = {b['bet_id']: b for b in bets}
        polls = 0
        self.draw_monitor(bets, pending, self.bankroll, force=True)
        while pending:
            time.sleep(MONITOR_POLL_SECS)
            polls += 1
            bal, _ = self.get_balance()
            if bal is not None:
                self.bankroll = bal
                self.peak = max(self.peak, bal)
            if polls % 2 == 0:
                self._fresh_matches = self.fetch_all_matches()
            statuses = self.get_bet_statuses()
            redraw = False
            for bid, b in list(pending.items()):
                st = statuses.get(str(bid), '')
                if st in ('WON', 'LOST'):
                    b['result'] = st
                    b['settled_at'] = datetime.now().isoformat()
                    b['balance'] = bal
                    b['payout'] = round(b['stake'] * b['odd_value'], 2) if st == 'WON' else 0.0
                    self.log_bet_data(b)
                    del pending[bid]
                    redraw = True
            for b in bets:
                if b.get('result') in ('PLACE', 'MONITOR'):
                    b['result'] = 'MONITOR'
                    if self.match_phase(b.get('start_time')) in ('PLAY', 'END'):
                        redraw = True
            self.draw_monitor(bets, pending, bal, force=redraw)
            if not pending:
                break

    @staticmethod
    def ev(prediction):
        prob = prediction['confidence'] / 100.0
        return prob * prediction['odd_value'] - 1

    def recovery_deficit(self):
        if not self.config['recovery']:
            return 0.0
        return max(0.0, self.start_bankroll - self.bankroll)

    @staticmethod
    def _start_secs(pred):
        st = pred.get('start_time', '')
        try:
            return (datetime.strptime(st, '%Y-%m-%d %H:%M:%S') - datetime.now()).total_seconds()
        except Exception:
            return float('inf')

    def set_stop(self, spec, profit=None):
        self.stop = []
        if profit is not None:
            self.stop.append(('profit', float(profit)))
        if not spec:
            return
        for part in str(spec).split(','):
            part = part.strip().lower()
            if not part:
                continue
            m = re.match(r'([a-z]+)(\d+(?:\.\d+)?)$', part)
            if not m:
                print(f'  [ignore] bad --stop spec: {part}')
                continue
            key, val = m.group(1), float(m.group(2))
            if key in ('wins', 'win'):
                self.stop.append(('wins', val))
            elif key in ('losses', 'lost', 'loses'):
                self.stop.append(('losses', val))
            elif key == 'loss':
                self.stop.append(('loss', val))
            elif key == 'bal':
                self.stop.append(('bal', val))
            elif key == 'profit':
                self.stop.append(('profit', val))
            else:
                print(f'  [ignore] unknown --stop key: {key}')

    def check_stop(self):
        profit = self.bankroll - self.start_bankroll
        for kind, val in self.stop:
            if kind == 'wins' and self.cum_wins >= val:
                return f'won {self.cum_wins} bets (target {val:.0f})'
            if kind == 'losses' and self.cum_losses >= val:
                return f'lost {self.cum_losses} bets (target {val:.0f})'
            if kind == 'loss' and self.cum_loss_amt >= val:
                return f'lost KES {self.cum_loss_amt:.2f} (target {val:.0f})'
            if kind == 'bal' and self.bankroll >= val:
                return f'balance KES {self.bankroll:.2f} >= {val:.0f}'
            if kind == 'profit' and profit >= val:
                return f'profit KES {profit:.2f} >= {val:.0f}'
        return None

    def _exposure(self):
        if self.bankroll > 0 and self.bankroll < self.config['low_bal_threshold']:
            return self.config['low_bal_exposure']
        return self.config['max_exposure']

    def compute_stake(self):
        base = self.config['stake']
        max_stake = self.config['max_stake']
        budget = self.bankroll * self._exposure() if self.bankroll > 0 else max_stake
        cap = min(max_stake, budget)
        min_stake = self.config['min_stake']
        if cap < min_stake and self.bankroll >= min_stake:
            cap = min(min_stake, self.bankroll)
        deficit = self.recovery_deficit()
        if deficit > 0:
            base = self.base_stake
            self.recovery_stake = min(base, cap)
            return self.recovery_stake
        self.recovery_stake = 0.0
        return min(base, cap)

    def select_picks(self, matches, stake=None, max_bets=None):
        cfg = self.config
        stake = stake if stake is not None else cfg['stake']
        stake = min(stake, cfg['max_stake'])
        max_bets = max_bets if max_bets is not None else cfg['bets_per_round']
        preds = [self.predict(m) for m in matches]
        preds = [p for p in preds if p is not None]
        max_odds = cfg['max_odds']
        min_conf = cfg['min_confidence']
        if self.recovery_deficit() > 0:
            min_conf = min(min_conf + 8, 90)
        candidates = [p for p in preds
                      if p['odd_value'] >= cfg['min_odds']
                      and (max_odds <= 0 or p['odd_value'] <= max_odds)
                      and (not cfg['away_only'] or p['pick'] == '2')
                      and p['confidence'] >= min_conf
                      and self.ev(p) >= cfg['min_edge']
                      and p['id'] not in self.placed_ids]
        if self.recovery_deficit() > 0:
            candidates.sort(key=lambda x: (self._start_secs(x), x['odd_value']))
        else:
            candidates.sort(key=lambda x: (self._start_secs(x), -self.ev(x)))
        budget = self.bankroll * self._exposure()
        if self.bankroll >= self.config['min_stake']:
            budget = max(budget, self.config['min_stake'])
        picks = []
        for p in candidates:
            if (len(picks) + 1) * stake <= budget + 1e-9:
                picks.append(p)
            if len(picks) >= max_bets:
                break
        return picks

    def decide_next(self, bets, bal):
        wins = sum(1 for b in bets if b.get('result') == 'WON')
        losses = sum(1 for b in bets if b.get('result') == 'LOST')
        total = wins + losses
        if total == 0:
            return 'continue', 'No settled bets'
        win_rate = wins / total
        staked = sum(b['stake'] for b in bets if b.get('result') in ('WON', 'LOST'))
        returns = sum(b['stake'] * b['odd_value'] for b in bets if b.get('result') == 'WON')
        pnl = returns - staked
        roi = pnl / staked if staked else 0
        drawdown = (self.peak - bal) / self.peak if self.peak else 0
        deficit = self.recovery_deficit()

        self.p(f'\n  ═══ ROUND {self.round} ANALYSIS ═══')
        self.p(f'  Win rate: {wins}/{total} ({win_rate*100:.1f}%)')
        self.p(f'  P&L: KES {pnl:+.2f} ({roi*100:+.1f}% ROI)')
        self.p(f'  Bankroll: KES {bal:.2f} (peak: KES {self.peak:.2f}, DD: {drawdown*100:.1f}%)')
        if deficit > 0:
            self.p(f'  Recovery: KES {deficit:.2f} deficit pending → stake KES {self.recovery_stake:.2f} next round')

        if drawdown > 0.3 and self.config['dd_stop']:
            return 'stop', f'Drawdown {drawdown*100:.0f}% > 30% — stopping'
        if pnl <= -10 and not self.config['recovery']:
            return 'stop', f'Lost KES {abs(pnl):.0f} this round — too many losses'
        profit = bal - self.start_bankroll
        ramp_threshold = self.config['ramp_threshold']

        if deficit > 0:
            if self.config['stake'] > self.base_stake:
                old_stake = self.config['stake']
                self.config['stake'] = max(old_stake - self.config['stake_step'], self.base_stake)
                return 'adjust', f'RECOVERING — KES {deficit:.2f} below start → stake {old_stake:.1f} → KES {self.config["stake"]:.1f}'
            return 'continue', f'RECOVERING — KES {deficit:.2f} below start, holding stake KES {self.config["stake"]}, NO increases'

        if profit < ramp_threshold:
            if self.bankroll < 2 * self.config['stake'] and self.config['stake'] > 1.0:
                old_stake = self.config['stake']
                self.config['stake'] = max(old_stake - self.config['stake_step'], 1.0)
                return 'adjust', f'Balance KES {self.bankroll:.2f} < 2× stake → stake {old_stake:.1f} → KES {self.config["stake"]:.1f}'
            return 'continue', f'BREAK-EVEN (profit KES {profit:.2f} < {ramp_threshold:.0f}) — holding stake KES {self.config["stake"]}, NO increase'

        if self.config['auto_stake'] and self.bankroll >= 3 * self.config['stake']:
            old_stake = self.config['stake']
            self.config['stake'] = min(old_stake + self.config['stake_step'], self.config['max_stake'])
            return 'continue', f'Profit KES {profit:.2f} ≥ {ramp_threshold:.0f}! Stake KES {old_stake} → KES {self.config["stake"]}'
        return 'continue', f'Profitable (KES {profit:.2f}) but holding stake KES {self.config["stake"]} (low bankroll)'

    def run(self):
        self.print_header()
        if not self.warmup():
            self.p("Failed to warmup session"); return
        if self.load_session():
            bal, bonus = self.get_balance()
            self.p(f'  Loaded saved session (user {self.user_id})')
        else:
            self.p('  No valid saved session, logging in...')
            ok, err = self.login()
            if not ok:
                self.p(f"Login failed: {err}"); return
            bal, bonus = self.get_balance()
        self.bankroll = bal if bal else 0
        self.start_bankroll = self.starting_balance if self.starting_balance else self.bankroll
        self.peak = self.start_bankroll
        try:
            with open(DATA_DIR / 'state.json') as f:
                st = json.load(f)
            saved_peak = st.get('peak', 0) or 0
            if saved_peak > self.peak:
                self.peak = saved_peak
        except:
            pass
        self.p(f'  Logged in as {self.user_id} | Balance: KES {bal:.2f} (bonus: KES {bonus})')
        self.p(f'  Strategy: {self.config["bets_per_round"]} bets × KES {self.config["stake"]}, odds ≥ {self.config["min_odds"]}'
              + (f'–{self.config["max_odds"]:g}' if self.config['max_odds'] > 0 else '')
              + (', AWAY only' if self.config['away_only'] else '')
              + (f', pause after {self.config["no_bets_after"]}' if self.config['no_bets_after'] else '')
              + (f', recovery x{self.config["recovery_multiplier"]:.0f}' if self.config['recovery'] else ''))
        print()

        while True:
            self.round += 1
            bal, _ = self.get_balance()
            self.bankroll = bal if bal else 0
            self.peak = max(self.peak, self.bankroll)

            stop_reason = self.check_stop()
            if stop_reason:
                self.p(f'\n  ═══ STOP TARGET HIT: {stop_reason} ═══')
                self.p(f'  Balance: KES {self.bankroll:.2f} (start: KES {self.start_bankroll:.2f}, peak: KES {self.peak:.2f})')
                self.save_state([])
                break

            if self.bankroll < self.config['stake']:
                self.p(f'\n  ❌ Balance KES {self.bankroll:.2f} too low to continue. Need ≥ KES {self.config["stake"]}.')
                if not self.config['wait_low_balance']:
                    break
                self.p(f'  Waiting for top-up... checking every {POLL_SECS}s (Ctrl-C to stop)')
                time.sleep(POLL_SECS)
                self.round -= 1
                continue

            cutoff = self.config['no_bets_after']
            if cutoff:
                hh, mm = map(int, cutoff.split(':'))
                now = datetime.now()
                if (now.hour, now.minute) >= (hh, mm):
                    nxt = (now + timedelta(days=1)).replace(hour=0, minute=0, second=0, microsecond=0)
                    secs = int((nxt - now).total_seconds())
                    self.p(f'\n  🛑 Past daily cutoff {cutoff} — pausing {secs//3600}h{(secs%3600)//60}m until midnight')
                    while datetime.now() < nxt:
                        time.sleep(min(300, (nxt - datetime.now()).total_seconds()))
                    self.round -= 1
                    continue

            round_stake = self.compute_stake()
            recovering = self.recovery_deficit() > 0
            max_bets = 1 if (recovering or self.config['micro']) else self.config['bets_per_round']

            self.p(f'  ━━━━━━━━━━━━━━━━━━━━━━━━━ ROUND {self.round} ━━━━━━━━━━━━━━━━━━━━━━━━━━━')
            self.p(f'  Balance: KES {self.bankroll:.2f} | Stake: KES {round_stake:.2f} | Odds ≥ {self.config["min_odds"]}'
                   + (f' | RECOVERY: KES {self.recovery_deficit():.2f}' if recovering else ''))
            print()

            matches = self._fresh_matches if self._fresh_matches else self.fetch_all_matches()
            self._fresh_matches = None
            self.placed_ids |= self.active_bet_match_ids()
            picks = self.select_picks(matches, stake=round_stake, max_bets=max_bets)
            if not picks:
                self.p(f'  No qualifying matches (odds ≥ {self.config["min_odds"]}). Waiting for next matches...')
                nxt = self.upcoming_matches(matches)
                if nxt:
                    self.p('  Next starts: ' + ' | '.join(nxt))
                time.sleep(POLL_SECS)
                self.round -= 1
                continue

            self.p(f'  Top picks ({len(picks)}):')
            for p in picks:
                label = PICK_LABEL.get(p['pick'], p['pick'])
                self.p(f'    {p["home"]:18s} vs {p["away"]:18s} [{p["league"]:12s}] {label} @ {p["odd_value"]:.2f} ({p["confidence"]}%) EV {self.ev(p):+.3f}')
            print()

            if self.dry_run:
                time.sleep(POLL_SECS)
                continue

            bets = []
            for p in picks:
                result, bid = self.place_bet(p, round_stake)
                if result == 'success':
                    self.placed_ids.add(p['id'])
                    bets.append({
                        'id': p['id'], 'home': p['home'], 'away': p['away'],
                        'league': p['league'], 'pick': p['pick'],
                        'odd_value': p['odd_value'], 'confidence': p['confidence'],
                        'bet_id': bid, 'stake': round_stake,
                        'start_time': p.get('start_time', ''),
                        'remaining_time': p.get('remaining_time', ''),
                        'odds_1x2': p.get('odds', {}),
                        'season': p.get('season', ''), 'match_day': p.get('match_day', ''),
                        'result': 'PLACE', 'placed_at': datetime.now().isoformat(),
                    })
                    self.p(f'  ✓ Bet #{bid}: {p["home"]} vs {p["away"]} @ {p["odd_value"]:.2f}')
                elif result == 'insufficient_balance':
                    self.p(f'  ✗ Insufficient balance after {len(bets)} bets placed')
                    break
                else:
                    if result == 'duplicate':
                        self.placed_ids.add(p['id'])
                    self.p(f'  ✗ Failed to place bet on {p["home"]} vs {p["away"]}: {result}')
            print()

            if not bets:
                time.sleep(POLL_SECS)
                continue

            bal, _ = self.get_balance()
            if bal is not None:
                self.bankroll = bal
                self.peak = max(self.peak, bal)
            self.p(f'  💰 Balance after stake: KES {self.bankroll:.2f}')
            print()

            self.print_bets_table(bets, 'PLACE')
            print()

            self.monitor_bets(bets)

            bal, _ = self.get_balance()
            self.bankroll = bal if bal else 0
            self.peak = max(self.peak, self.bankroll)
            self.print_bets_table(bets, 'DONE')
            self.print_summary(bets, bal)
            self.all_rounds.append({'round': self.round, 'bets': bets, 'balance': bal})
            self.cum_wins += sum(1 for b in bets if b.get('result') == 'WON')
            self.cum_losses += sum(1 for b in bets if b.get('result') == 'LOST')
            self.cum_loss_amt += sum(b['stake'] for b in bets if b.get('result') == 'LOST')

            wd = self.config['withdraw_amount']
            if wd > 0 and bal and bal >= self.config['withdraw_at']:
                ok, msg = self.withdraw(wd)
                if ok:
                    self.p(f'  💸 WITHDREW KES {wd:g} to M-Pesa — approve the STK push on your phone!')
                    self.p(f'     {msg}')
                    self.start_bankroll = max(bal - wd, 0)
                    self.peak = max(self.peak, self.start_bankroll)
                    self.withdrawn_total += wd
                    self.p(f'  New recovery baseline: KES {self.start_bankroll:.2f} (total withdrawn: KES {self.withdrawn_total:.2f})')
                else:
                    self.p(f'  ⚠️  Withdraw KES {wd:g} FAILED: {msg}')

            decision, reason = self.decide_next(bets, bal)
            self.p(f'  Decision: {decision.upper()} — {reason}')
            print()

            if decision == 'stop':
                self.p(f'  ═══ SESSION OVER ═══')
                break

            if decision == 'adjust':
                self.p(f'  Adjusting: {reason}')

            self.save_state(bets)

    @staticmethod
    def ts():
        return datetime.now().strftime('%Y-%m-%d %H:%M:%S')

    def p(self, *a):
        print(f'[{self.ts()}]', *a, flush=True)

    def print_header(self):
        print('┌─────────────────────────────────────────────────────────────┐')
        print('│              BETIKA VIRTUAL BETTING BOT v2                 │')
        print(f'│  Mode: {"LIVE" if not self.dry_run else "DRY RUN":31s} │')
        print('└─────────────────────────────────────────────────────────────┘')
        self.p('CMD: python3 ' + Path(sys.argv[0]).name + ' ' + ' '.join(sys.argv[1:]))
        cfg = self.config
        self.p(f'config: bets={cfg["bets_per_round"]} stake={cfg["stake"]} max_stake={cfg["max_stake"]} '
               f'min_odds={cfg["min_odds"]} max_odds={cfg["max_odds"]} min_conf={cfg["min_confidence"]} '
               f'min_edge={cfg["min_edge"]} max_exposure={cfg["max_exposure"]} low_bal<{cfg["low_bal_threshold"]}'
               f'@{cfg["low_bal_exposure"]} away_only={cfg["away_only"]} no_bets_after={cfg["no_bets_after"] or "off"} '
               f'micro={cfg["micro"]} min_stake={cfg["min_stake"]} '
               f'withdraw={cfg["withdraw_amount"]:g}@{cfg["withdraw_at"]:g} '
               f'recovery={cfg["recovery"]} auto_stake={cfg["auto_stake"]} '
               f'dd_stop={cfg["dd_stop"]} wait_low_balance={cfg["wait_low_balance"]}')
        if self.stop:
            self.p('stop targets: ' + ', '.join(f'{k}{v:g}' for k, v in self.stop))

    def save_state(self, bets):
        state = {
            'timestamp': datetime.now().isoformat(),
            'round': self.round,
            'balance': self.bankroll,
            'peak': self.peak,
            'config': self.config,
            'rounds': len(self.all_rounds),
        }
        with open(DATA_DIR / 'state.json', 'w') as f:
            json.dump(state, f, indent=2, default=str)
        with open(DATA_DIR / 'bets.json', 'w') as f:
            json.dump(self.all_rounds, f, indent=2, default=str)


if __name__ == '__main__':
    import argparse

    def _epilog():
        return f'''
═══════════════════════════ QUICK START ═══════════════════════════

Dry run first (no real money):
  python3 betika_bot.py

Live once you trust it:
  python3 betika_bot.py --live --stake 2 --bets 1 --min-odds 1.50

Recommended safe grind (small bankroll):
  python3 betika_bot.py --live --micro --no-bets-after 23:00 \\
      --no-dd-stop --stop "profit10,loss15"

═══════════════════════════ EXAMPLES ═══════════════════════════

  python3 betika_bot.py --live --stake 2 --bets 1 --min-odds 1.50 \\
      --max-stake 5 --auto-stake --stake-step 1.0   live, small grind
  python3 betika_bot.py --live --no-dd-stop          grind indefinitely
  python3 betika_bot.py --live --min-odds 1.50 \\
      --max-stake 5 --no-recovery                    conservative flat
  python3 betika_bot.py --live --min-odds 1.50 --max-stake 5 \\
      --stop bal50 --profit 39                 stop at bal 50 or profit 39
  python3 betika_bot.py --live --min-odds 1.50 \\
      --stop "losses5,loss20"                  stop after 5 losses or -KES20
  python3 betika_bot.py --live --micro --no-bets-after 23:00 \\
      --no-dd-stop --stop "profit10,loss15"    micro grind for a tiny bankroll
  python3 betika_bot.py --live --away-only \\
      --min-odds 1.50 --max-odds 1.70          AWAY picks in the 1.5-1.7 band
  python3 betika_bot.py --live --stake 5 --bets 2 --min-odds 1.50 \\
      --withdraw 50 --withdraw-at 75           auto-withdraw 50 at balance 75
      (Betika minimum withdrawal is KES 50; fires an M-Pesa STK push you
       approve on your phone)

═══════════════════════════ HOW IT PICKS ═══════════════════════════

The bot fetches Betika Virtuals matches and computes implied probabilities
from the 1X2 odds (after removing the bookmaker margin). It picks the side
with the highest confidence, then applies a stack of filters:

  1. Odds band     --min-odds / --max-odds  (hard floor + optional ceiling)
  2. Side bias     --away-only              (bet AWAY (2) only)
  3. Confidence    --min-confidence         (55% default)
  4. EV filter     --min-edge               (skip negative-expectation picks)
  5. No duplicates already-placed bets are skipped

Matches are taken soonest-starting first (in-play before pre-match), so the
bot never idles on a match 8 minutes out while another starts now.

Picks are scored and the highest-EV ones within the round budget are chosen.

═══════════════════════════ STAKING & RISK ═══════════════════════════

Per-bet stake is capped by THREE limits (the strictest wins):
  - base stake    --stake
  - hard cap      --max-stake
  - bankroll      balance × --max-exposure (0.5 default: risk ≤ 50%)

Tiny bankrolls: when balance drops below --low-bal-threshold (10 KES), the
exposure cap falls to --low-bal-exposure (0.25) so a single round cannot
wipe the account. Stakes never drop below --min-stake (1.0, Betika floor).

Auto-staking (--auto-stake, default ON):
  - raises stake by --stake-step ONLY after profitable rounds
  - requires profit ≥ --ramp-threshold (5.0) above the start balance
  - requires bankroll ≥ 3× stake
  - never raises while a recovery deficit is pending

═══════════════════════════ RECOVERY ═══════════════════════════

Recovery (--recovery, default ON) engages whenever the current balance is
BELOW the recovery baseline. While recovering the bot:
  - forces 1 bet per round at the base stake (no martingale multiplier)
  - sorts picks by soonest start then lowest odds
  - never increases stake or odds

Baseline: defaults to the balance when the bot launched. Use
--starting-balance N to pin it to a fixed figure so a restart mid-loss
still counts as "recovering" toward your real starting bankroll.

═══ STOP TARGETS (--stop, comma-separated, checked every round) ═══

  bal50      stop when balance reaches 50
  profit39   stop when profit reaches 39
  wins10     stop after 10 winning bets
  losses5    stop after 5 losing bets
  loss20     stop after losing 20 KES in total
  --profit N is shorthand for --stop profit<N>

═══ OTHER SAFETY ═══

  --withdraw N            auto-withdraw N KES to M-Pesa whenever balance
                          reaches --withdraw-at (default trigger: N + 25).
                          Uses Betika's direct M-Pesa withdrawal (NOT
                          Cashia) — an STK push fires that you approve on
                          your phone with your M-Pesa PIN. After a
                          withdrawal the recovery baseline resets to the
                          remaining balance so the bot keeps grinding.
                          Betika minimum withdrawal is KES 50.
  --no-bets-after HH:MM   pause betting for the rest of the day (the 23:00+
                          window showed the worst win rate) and resume at
                          midnight. Handy when running overnight.
  --no-dd-stop            keep grinding even after a >30% drawdown from peak
                          (default stops the session to protect the account)
  --wait-low-balance      instead of exiting when balance < stake, keep
                          polling until a top-up lands (handy with --micro)

═══════════════════════════ DATA & SESSION ═══════════════════════════

  Session (token + cookies) : {DATA_DIR}/session.json
  Live state (peak, config) : {DATA_DIR}/state.json
  Round history             : {DATA_DIR}/bets.json
  Append-only bet log       : {DATA_DIR}/bet_data.jsonl
  Console log               : {DATA_DIR}/bot.log  (nohup ... &)

Set BETIKA_DATA_DIR to override the data directory.

The bot runs with a saved session by default; --phone/--password are only
used as a login fallback if no valid session exists.
'''

    parser = argparse.ArgumentParser(
        prog='betika_bot.py',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        description=(
            'Betika Virtuals Betting Bot — adaptive staking with EV filter, '
            'recovery, and drawdown protection. Runs with a saved session '
            '(token) by default; login is used only as a fallback.'
        ),
        epilog=_epilog(),
    )
    mode = parser.add_argument_group('Mode')
    mode.add_argument('--live', action='store_true',
                      help='Place REAL bets (default is a dry run)')
    mode.add_argument('--balance', dest='check_balance', action='store_true',
                      help='check balance using the saved session and exit')
    mode.add_argument('--deposit', dest='deposit', type=float, default=0.0,
                      help='fire a direct M-Pesa deposit STK push for this amount '
                           '(KES) and exit. Approve with your M-Pesa PIN on the phone.')

    strat = parser.add_argument_group('Strategy')
    strat.add_argument('--stake', type=float, default=5.0,
                       help='base stake per bet (default 5.0)')
    strat.add_argument('--bets', type=int, default=3,
                       help='bets per round (default 3)')
    strat.add_argument('--min-odds', type=float, default=1.40,
                       help='only bet odds >= this (default 1.40)')
    strat.add_argument('--min-edge', type=float, default=-0.10,
                       help='minimum EV edge to take a pick (default -0.10)')
    strat.add_argument('--min-confidence', dest='min_confidence', type=float, default=55.0,
                       help='minimum confidence (0-100) to take a pick (default 55)')
    strat.add_argument('--confidence', dest='confidence', type=float, default=0.0,
                       help='alias for --min-confidence: only bet picks above this confidence %%')
    strat.add_argument('--max-odds', type=float, default=0.0,
                       help='upper odds bound for the profitable band (0 = no bound) (default 0)')
    strat.add_argument('--away-only', dest='away_only', action='store_true', default=False,
                       help='only bet AWAY picks (2) — AWAY showed +20%% ROI in testing')
    strat.add_argument('--no-bets-after', dest='no_bets_after', default='',
                       help='pause betting after HH:MM daily (e.g. 23:00); resumes at midnight')
    strat.add_argument('--micro', dest='micro', action='store_true', default=False,
                       help='micro-stake mode: stake KES 1, 1 bet/round, min-odds 1.50 (grind a tiny bankroll)')
    strat.add_argument('--max-exposure', type=float, default=0.5,
                       help='max fraction of balance exposed (per bet AND per round) (default 0.5)')
    strat.add_argument('--low-bal-threshold', type=float, default=10.0,
                       help='below this balance, cap exposure at --low-bal-exposure (default 10.0)')
    strat.add_argument('--low-bal-exposure', type=float, default=0.25,
                       help='exposure cap for tiny bankrolls (default 0.25)')
    strat.add_argument('--min-stake', type=float, default=1.0,
                       help='floor on any single bet stake (Betika minimum 1.0) (default 1.0)')
    strat.add_argument('--starting-balance', dest='starting_balance', type=float, default=0.0,
                       help='override the recovery baseline (start bankroll) instead of '
                            'using the balance at launch (default 0 = use current balance)')

    rec = parser.add_argument_group('Recovery')
    rec.add_argument('--recovery', dest='recovery', action='store_true', default=None,
                     help='enable loss recovery (default)')
    rec.add_argument('--no-recovery', dest='recovery', action='store_false',
                     help='disable loss recovery')
    rec.add_argument('--recovery-multiplier', type=float, default=3.0,
                     help='stake multiplier after a loss (default 3.0)')

    stake = parser.add_argument_group('Staking')
    stake.add_argument('--auto-stake', dest='auto_stake', action='store_true', default=None,
                       help='auto-size stakes to balance (default)')
    stake.add_argument('--no-auto-stake', dest='auto_stake', action='store_false',
                       help='use a flat --stake every bet')
    stake.add_argument('--stake-step', type=float, default=1.0,
                       help='stake increment step for auto-stake (default 1.0)')
    stake.add_argument('--max-stake', type=float, default=10.0,
                       help='hard cap on any single bet stake (default 10.0)')
    stake.add_argument('--ramp-threshold', type=float, default=5.0,
                       help='min profit (KES) above start balance before auto-stake '
                            'may raise stakes (default 5.0)')

    safety = parser.add_argument_group('Safety')
    safety.add_argument('--no-dd-stop', dest='dd_stop', action='store_false', default=None,
                        help='disable the drawdown stop and keep grinding')
    safety.add_argument('--wait-low-balance', dest='wait_low_balance', action='store_true',
                        default=False,
                        help='keep polling (instead of exiting) until balance >= stake')
    safety.add_argument('--stop', default='',
                        help='stop when a target is hit. Forms: bal50, profit39, '
                             'wins10, losses20, loss30 (comma-separated allowed)')
    safety.add_argument('--profit', type=float, default=None,
                        help='shorthand for --stop profit<N>')
    safety.add_argument('--withdraw', dest='withdraw', type=float, default=0.0,
                        help='auto-withdraw this amount to M-Pesa (STK push) each time '
                             'balance >= --withdraw-at (default 0 = off)')
    safety.add_argument('--withdraw-at', dest='withdraw_at', type=float, default=0.0,
                        help='balance that triggers --withdraw (default: withdraw amount '
                             '+ 25, e.g. --withdraw 50 fires at 75)')

    sess = parser.add_argument_group('Login fallback (only if no saved session)')
    sess.add_argument('--phone', default='254726498682',
                      help='Betika phone number')
    sess.add_argument('--password', default='34266775',
                      help='Betika password')
    args = parser.parse_args()
    bot = BetikaBot(phone=args.phone, password=args.password, dry_run=not args.live)
    if args.stake: bot.config['stake'] = args.stake
    bot.base_stake = bot.config['stake']
    if args.bets: bot.config['bets_per_round'] = args.bets
    if args.min_odds: bot.config['min_odds'] = args.min_odds
    bot.config['min_edge'] = args.min_edge
    bot.config['min_confidence'] = args.confidence or args.min_confidence
    bot.config['max_exposure'] = args.max_exposure
    bot.config['max_odds'] = args.max_odds
    bot.config['away_only'] = args.away_only
    bot.config['no_bets_after'] = args.no_bets_after
    bot.config['low_bal_threshold'] = args.low_bal_threshold
    bot.config['low_bal_exposure'] = args.low_bal_exposure
    bot.config['min_stake'] = args.min_stake
    bot.config['micro'] = args.micro
    bot.config['withdraw_amount'] = args.withdraw
    bot.config['withdraw_at'] = args.withdraw_at or (args.withdraw + 25 if args.withdraw else 0)
    if args.starting_balance:
        bot.starting_balance = args.starting_balance
    if args.micro:
        bot.config['stake'] = 1.0
        bot.config['max_stake'] = 1.0
        bot.config['min_odds'] = 1.50
        bot.config['bets_per_round'] = 1
    if args.recovery is not None:
        bot.config['recovery'] = args.recovery
    bot.config['recovery_multiplier'] = args.recovery_multiplier
    bot.config['stake_step'] = args.stake_step
    bot.config['max_stake'] = args.max_stake
    if args.auto_stake is not None:
        bot.config['auto_stake'] = args.auto_stake
    if args.dd_stop is not None:
        bot.config['dd_stop'] = args.dd_stop
    bot.config['wait_low_balance'] = args.wait_low_balance
    bot.set_stop(args.stop, args.profit)
    if args.check_balance:
        bot.print_header()
        if not bot.warmup():
            print('  Failed to warmup session'); sys.exit(1)
        if bot.load_session():
            bal, bonus = bot.get_balance()
        else:
            print('  No valid saved session — use --phone/--password or run once to save one')
            sys.exit(1)
        if bal is None:
            print('  Could not fetch balance'); sys.exit(1)
        print(f'  Balance: KES {bal:.2f}  (bonus: KES {bonus:.2f})')
        sys.exit(0)
    if args.deposit:
        bot.print_header()
        if not bot.warmup():
            print('  Failed to warmup session'); sys.exit(1)
        if not bot.load_session():
            print('  No valid saved session — use --phone/--password or run once to save one')
            sys.exit(1)
        ok, msg = bot.deposit(args.deposit)
        if ok:
            print(f'  ✅ Deposit STK push for KES {args.deposit:g} sent — approve with your M-Pesa PIN')
        else:
            print(f'  ⚠️  Deposit failed: {msg}')
            sys.exit(1)
        sys.exit(0)
    try:
        bot.run()
    except KeyboardInterrupt:
        print("\nStopped.", flush=True)
