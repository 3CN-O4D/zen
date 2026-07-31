#!/usr/bin/env python3
import requests, json, time, os, sys
from datetime import datetime
from pathlib import Path

POLL_SECS = 20
MATCH_DURATION_SECS = 180
DATA_DIR = Path(os.environ.get('BETIKA_DATA_DIR', '/tmp/betika_data'))
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
            'max_exposure': 0.8,
            'recovery': True,
            'recovery_multiplier': 3.0,
            'auto_stake': True,
            'stake_step': 1.0,
            'dd_stop': True,
        }
        self.start_bankroll = 0
        self.recovery_stake = 0.0
        self.placed_ids = set()

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
                if bal is not None:
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
                    print('    balance fetch failed, retrying...', flush=True)
                    time.sleep(2 * (attempt + 1))
        return None, None

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
            print(f"[DRY] Bet KES {stake} @ {prediction['odd_value']} on {label} ({prediction['confidence']}%)")
            return 'dry_run', ''
        r = None
        last_err = None
        for attempt in range(3):
            try:
                r = self.session.post(PLACEBET_API, json=body, timeout=15)
                break
            except (requests.exceptions.ConnectionError, requests.exceptions.Timeout) as e:
                last_err = e
                print(f'    bet placement network error, retrying ({attempt + 1}/3)...', flush=True)
                time.sleep(3 * (attempt + 1))
        else:
            print(f'    bet placement failed after 3 attempts: {last_err}', flush=True)
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
            print(f'    [place_bet 421] {msg}', flush=True)
            return ('duplicate', '') if 'similar' in msg.lower() else ('insufficient_balance', '')
        else:
            try:
                msg = r.json().get('message', '')
            except: msg = ''
            print(f'    [place_bet {r.status_code}] {msg}', flush=True)
            return 'error', ''

    @staticmethod
    def clock_str(b):
        st = b.get('start_time', '')
        try:
            t0 = datetime.strptime(st, '%Y-%m-%d %H:%M:%S')
        except Exception:
            return ''
        diff = int((t0 - datetime.now()).total_seconds())
        if diff > 0:
            return f'S {diff // 60}:{diff % 60:02d}'
        elapsed = -diff
        left = MATCH_DURATION_SECS - elapsed
        if left > 0:
            return f'E {left // 60}:{left % 60:02d}'
        return 'E 0:00'

    def print_bets_table(self, bets, phase):
        cfg = self.config
        label_phase = {'PLACE': 'PLACING BETS', 'MONITOR': 'MONITORING', 'DONE': 'ROUND COMPLETE'}.get(phase, phase)
        widths = (5, 32, 13, 8, 8, 8, 9, 7, 8, 11)
        def box(l, m, r):
            return '  ' + l + m.join('─' * w for w in widths) + r
        print(box('┌', '┬', '┐'))
        print('  │ ' + f'{"#":^3} │ {"Match":^30} │ {"League":^11} │ {"Pick":^6} │ {"Odds":^6} │ {"Conf%":^6} │ {"Stake":^7} │ {"Start":^5} │ {"Clock":^6} │ {"Result":^9} │')
        print(box('├', '┼', '┤'))
        for i, b in enumerate(bets, 1):
            label = PICK_LABEL.get(b['pick'], b['pick'])
            league = b.get('league', '')[:11]
            result = b.get('result', phase)
            result_str = {'PLACE': 'PLACED', 'MONITOR': '● ACTIVE', 'WON': '✓ WON', 'LOST': '✗ LOST'}.get(result, result)
            home = b['home'][:13]
            away = b['away'][:13]
            conf = b.get('confidence', 0)
            stake = b.get('stake', 0)
            start = (b.get('start_time') or '')[11:16]
            clock = self.clock_str(b)
            print(f'  │ {i:^3d} │ {home:13s} vs {away:13s} │ {league:11s} │ {label:6s} │ {b["odd_value"]:6.2f} │ {conf:5.1f}% │ {stake:7.2f} │ {start:5s} │ {clock:6s} │ {result_str:9s} │')
        print(box('└', '┴', '┘'))

    def print_summary(self, bets, bal):
        wins = sum(1 for b in bets if b.get('result') == 'WON')
        losses = sum(1 for b in bets if b.get('result') == 'LOST')
        active = sum(1 for b in bets if b.get('result') in ('MONITOR', None))
        total_stake = sum(b['stake'] for b in bets)
        staked = sum(b['stake'] for b in bets if b.get('result') in ('WON', 'LOST'))
        returns = sum(b['stake'] * b['odd_value'] for b in bets if b.get('result') == 'WON')
        pnl = returns - staked
        print(f'  Bets: {len(bets)} | Won: {wins} | Lost: {losses} | Active: {active}')
        print(f'  Staked: KES {total_stake} | Returns: KES {returns:.2f} | P&L: KES {pnl:+.2f}')
        print(f'  Balance: KES {bal:.2f} | Bankroll: KES {self.bankroll:.2f}')

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

    def monitor_bets(self, bets):
        pending = {b['bet_id']: b for b in bets}
        polls = 0
        last_settled = 0
        while pending:
            time.sleep(POLL_SECS)
            polls += 1
            bal, _ = self.get_balance()
            statuses = self.get_bet_statuses()
            for bid, b in list(pending.items()):
                st = statuses.get(str(bid), '')
                if st in ('WON', 'LOST'):
                    b['result'] = st
                    b['settled_at'] = datetime.now().isoformat()
                    b['balance'] = bal
                    b['payout'] = round(b['stake'] * b['odd_value'], 2) if st == 'WON' else 0.0
                    self.log_bet_data(b)
                    del pending[bid]
            settled = len(bets) - len(pending)
            if settled != last_settled:
                self.print_bets_table(bets, 'MONITOR')
                print(f'  Settled: {settled}/{len(bets)} | Balance: KES {bal}', flush=True)
                if settled > 0:
                    print('  ' + '-' * 70, flush=True)
                last_settled = settled
            elif pending and polls % 15 == 0:
                print(f'  ⏳ Waiting on {len(pending)} bet(s) to settle... Balance: KES {bal}', flush=True)

    @staticmethod
    def ev(prediction):
        prob = prediction['confidence'] / 100.0
        return prob * prediction['odd_value'] - 1

    def recovery_deficit(self):
        if not self.config['recovery']:
            return 0.0
        return max(0.0, self.start_bankroll - self.bankroll)

    def compute_stake(self):
        base = self.config['stake']
        deficit = self.recovery_deficit()
        if deficit <= 0:
            self.recovery_stake = 0.0
            return base
        odd = max(self.config['min_odds'], 1.01)
        recover_stake = base + deficit / (odd - 1)
        cap = base * self.config['recovery_multiplier']
        budget = self.bankroll * self.config['max_exposure']
        stake = min(recover_stake, cap, budget)
        self.recovery_stake = stake
        return max(base, stake)

    def select_picks(self, matches, stake=None, max_bets=None):
        cfg = self.config
        stake = stake if stake is not None else cfg['stake']
        max_bets = max_bets if max_bets is not None else cfg['bets_per_round']
        preds = [self.predict(m) for m in matches]
        preds = [p for p in preds if p is not None]
        candidates = [p for p in preds
                      if p['odd_value'] >= cfg['min_odds']
                      and p['confidence'] >= cfg['min_confidence']
                      and self.ev(p) >= cfg['min_edge']
                      and p['id'] not in self.placed_ids]
        candidates.sort(key=lambda x: -self.ev(x))
        budget = self.bankroll * cfg['max_exposure']
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

        print(f'\n  ═══ ROUND {self.round} ANALYSIS ═══')
        print(f'  Win rate: {wins}/{total} ({win_rate*100:.1f}%)')
        print(f'  P&L: KES {pnl:+.2f} ({roi*100:+.1f}% ROI)')
        print(f'  Bankroll: KES {bal:.2f} (peak: KES {self.peak:.2f}, DD: {drawdown*100:.1f}%)')
        if deficit > 0:
            print(f'  Recovery: KES {deficit:.2f} deficit pending → stake KES {self.recovery_stake:.2f} next round')

        if drawdown > 0.3 and self.config['dd_stop']:
            return 'stop', f'Drawdown {drawdown*100:.0f}% > 30% — stopping'
        if pnl <= -10 and not self.config['recovery']:
            return 'stop', f'Lost KES {abs(pnl):.0f} this round — too many losses'
        if pnl > 0:
            if self.config['auto_stake']:
                old_stake = self.config['stake']
                self.config['stake'] = min(self.config['stake'] + self.config['stake_step'], 10)
                return 'continue', f'Profitable! Stake KES {old_stake} → KES {self.config["stake"]}'
            return 'continue', f'Profitable! Maintaining stake KES {self.config["stake"]}'
        if win_rate < 0.5 and total >= 3:
            self.config['min_odds'] = min(self.config['min_odds'] + 0.10, 2.00)
            self.config['bets_per_round'] = max(self.config['bets_per_round'] - 1, 1)
            return 'adjust', f'WR {win_rate*100:.0f}% too low → odds≥{self.config["min_odds"]:.2f}, bets={self.config["bets_per_round"]}'
        return 'continue', f'Break-even, maintaining strategy'

    def run(self):
        self.print_header()
        if self.load_session():
            bal, bonus = self.get_balance()
            print(f'  Loaded saved session (user {self.user_id})')
        else:
            print('  No valid saved session, logging in...')
            if not self.warmup():
                print("Failed to warmup session"); return
            ok, err = self.login()
            if not ok:
                print(f"Login failed: {err}"); return
            bal, bonus = self.get_balance()
        self.bankroll = bal if bal else 0
        self.start_bankroll = self.bankroll
        self.peak = self.bankroll
        try:
            with open(DATA_DIR / 'state.json') as f:
                st = json.load(f)
            saved_peak = st.get('peak', 0) or 0
            if saved_peak > self.peak:
                self.peak = saved_peak
        except:
            pass
        print(f'  Logged in as {self.user_id} | Balance: KES {bal:.2f} (bonus: KES {bonus})')
        print(f'  Strategy: {self.config["bets_per_round"]} bets × KES {self.config["stake"]}, odds ≥ {self.config["min_odds"]}'
              + (f', recovery x{self.config["recovery_multiplier"]:.0f}' if self.config['recovery'] else ''))
        print()

        while True:
            self.round += 1
            bal, _ = self.get_balance()
            self.bankroll = bal if bal else 0
            self.peak = max(self.peak, self.bankroll)

            if self.bankroll < self.config['stake']:
                print(f'\n  ❌ Balance KES {self.bankroll:.2f} too low to continue. Need ≥ KES {self.config["stake"]}.')
                break

            round_stake = self.compute_stake()
            recovering = self.recovery_deficit() > 0
            max_bets = 1 if recovering else self.config['bets_per_round']

            print(f'  ━━━━━━━━━━━━━━━━━━━━━━━━━ ROUND {self.round} ━━━━━━━━━━━━━━━━━━━━━━━━━━━')
            print(f'  Balance: KES {self.bankroll:.2f} | Stake: KES {round_stake:.2f} | Odds ≥ {self.config["min_odds"]}'
                  + (f' | RECOVERY: KES {self.recovery_deficit():.2f}' if recovering else ''))
            print()

            matches = self.fetch_all_matches()
            self.placed_ids |= self.active_bet_match_ids()
            picks = self.select_picks(matches, stake=round_stake, max_bets=max_bets)
            if not picks:
                print(f'  No qualifying matches (odds ≥ {self.config["min_odds"]}). Waiting...')
                time.sleep(POLL_SECS)
                self.round -= 1
                continue

            print(f'  Top picks ({len(picks)}):')
            for p in picks:
                label = PICK_LABEL.get(p['pick'], p['pick'])
                print(f'    {p["home"]:18s} vs {p["away"]:18s} [{p["league"]:12s}] {label} @ {p["odd_value"]:.2f} ({p["confidence"]}%) EV {self.ev(p):+.3f}')
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
                    print(f'  ✓ Bet #{bid}: {p["home"]} vs {p["away"]} @ {p["odd_value"]:.2f}')
                elif result == 'insufficient_balance':
                    print(f'  ✗ Insufficient balance after {len(bets)} bets placed')
                    break
                else:
                    if result == 'duplicate':
                        self.placed_ids.add(p['id'])
                    print(f'  ✗ Failed to place bet on {p["home"]} vs {p["away"]}: {result}')
            print()

            if not bets:
                time.sleep(POLL_SECS)
                continue

            self.print_bets_table(bets, 'PLACE')
            print()

            self.monitor_bets(bets)

            bal, _ = self.get_balance()
            self.bankroll = bal if bal else 0
            self.peak = max(self.peak, self.bankroll)
            self.print_bets_table(bets, 'DONE')
            self.print_summary(bets, bal)
            self.all_rounds.append({'round': self.round, 'bets': bets, 'balance': bal})

            if bal and bal >= 50:
                print(f'  🏆 TARGET HIT! Balance KES {bal:.2f} ≥ KES 50')
                print(f'  >>> WITHDRAW KES 20 <<<')
                print(f'  Withdraw via Betika website or app, then continue with remaining balance.\n')

            decision, reason = self.decide_next(bets, bal)
            print(f'  Decision: {decision.upper()} — {reason}')
            print()

            if decision == 'stop':
                print(f'  ═══ SESSION OVER ═══')
                break

            if decision == 'adjust':
                print(f'  Adjusting: {reason}')

            self.save_state(bets)

    def print_header(self):
        print('┌─────────────────────────────────────────────────────────────┐')
        print('│              BETIKA VIRTUAL BETTING BOT v2                 │')
        print(f'│  Mode: {"LIVE" if not self.dry_run else "DRY RUN":31s} │')
        print('└─────────────────────────────────────────────────────────────┘')

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
    parser = argparse.ArgumentParser()
    parser.add_argument('--phone', default='254726498682')
    parser.add_argument('--password', default='34266775')
    parser.add_argument('--stake', type=float, default=5.0)
    parser.add_argument('--bets', type=int, default=3)
    parser.add_argument('--min-odds', type=float, default=1.40)
    parser.add_argument('--min-edge', type=float, default=-0.10)
    parser.add_argument('--max-exposure', type=float, default=0.8)
    parser.add_argument('--recovery', dest='recovery', action='store_true', default=None)
    parser.add_argument('--no-recovery', dest='recovery', action='store_false')
    parser.add_argument('--recovery-multiplier', type=float, default=3.0)
    parser.add_argument('--stake-step', type=float, default=1.0)
    parser.add_argument('--auto-stake', dest='auto_stake', action='store_true', default=None)
    parser.add_argument('--no-auto-stake', dest='auto_stake', action='store_false')
    parser.add_argument('--no-dd-stop', dest='dd_stop', action='store_false', default=None)
    parser.add_argument('--live', action='store_true')
    args = parser.parse_args()
    bot = BetikaBot(phone=args.phone, password=args.password, dry_run=not args.live)
    if args.stake: bot.config['stake'] = args.stake
    if args.bets: bot.config['bets_per_round'] = args.bets
    if args.min_odds: bot.config['min_odds'] = args.min_odds
    bot.config['min_edge'] = args.min_edge
    bot.config['max_exposure'] = args.max_exposure
    if args.recovery is not None:
        bot.config['recovery'] = args.recovery
    bot.config['recovery_multiplier'] = args.recovery_multiplier
    bot.config['stake_step'] = args.stake_step
    if args.auto_stake is not None:
        bot.config['auto_stake'] = args.auto_stake
    if args.dd_stop is not None:
        bot.config['dd_stop'] = args.dd_stop
    try:
        bot.run()
    except KeyboardInterrupt:
        print("\nStopped.", flush=True)
