#!/usr/bin/env python3
"""
Betika Virtuals Scraper + Predictor
- Scrapes matches and results via REST API every N seconds
- Saves all data to /tmp/betika_data/
- Generates predictions
- Runs forever
"""
import requests, json, time, os, sys
from datetime import datetime
from pathlib import Path

POLL_SECS = 30
DATA_DIR = Path(os.environ.get('BETIKA_DATA_DIR', '/tmp/betika_data'))
os.makedirs(DATA_DIR, exist_ok=True)

API = 'https://virtuals.betika.com/v1/matches'

# All active leagues
LEAGUES = {
    6: 'Betika English League',
    7: 'Virtual Football Italian League',
    22: 'Virtual Football French League',
    24: 'Virtual Football German League',
    26: 'Betika Sakata League',
    27: 'Virtual Football League Mode England',
    28: 'Betika Bundesliga League',
}

session = requests.Session()
session.headers.update({
    'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36',
    'Origin': 'https://www.betika.com',
    'Referer': 'https://www.betika.com/en-ke/virtuals/matches/26',
})

def fetch_matches(league_id, status=None):
    params = {'competition_id': league_id}
    if status:
        params['status'] = status
    r = session.get(API, params=params, timeout=15)
    return r.json()['data']

def flatten(match_list):
    flat = []
    for comp_id, matches in match_list.items():
        for m in matches:
            flat.append(m)
    return flat

def parse_match(m):
    entry = {
        'id': m['parent_virtual_id'],
        'home': m['home_team'],
        'away': m['away_team'],
        'start': m['start_time'],
        'matchday': m.get('match_day', ''),
        'competition': m.get('competition_name', ''),
        'league_id': m.get('competition_id', ''),
        'remaining': m.get('remaining_time', ''),
        'outcome_id': m.get('outcome_id', ''),
        'scraped_at': datetime.now().isoformat(),
        'markets': {},
    }
    for market in m.get('markets', []):
        name = market['name']
        selections = {}
        for odd in market.get('odds', []):
            selections[odd['display']] = {
                'odd': float(odd['odd_value']),
                'outcome_id': odd['outcome_id'],
                'key': odd['odd_key'],
            }
        entry['markets'][name] = selections
    return entry

def outcome_to_result(outcome_id, markets):
    for display, data in markets.get('1X2', {}).items():
        if data['outcome_id'] == outcome_id:
            return display
    return f'outcome_{outcome_id}'

def compute_implied_probs(odds_dict):
    if not odds_dict:
        return {}
    inv = {}
    for k, v in odds_dict.items():
        try:
            inv[k] = 1.0 / v
        except:
            inv[k] = 0
    total = sum(inv.values())
    if total <= 0:
        return {k: 0 for k in inv}
    return {k: round(v / total * 100, 1) for k, v in inv.items()}

def predict_match(match):
    odds_1x2 = {k: v['odd'] for k, v in match['markets'].get('1X2', {}).items()}
    if not odds_1x2 or len(odds_1x2) != 3:
        return None
    probs = compute_implied_probs(odds_1x2)
    outcomes = sorted(probs.items(), key=lambda x: -x[1])
    best = outcomes[0]
    return {
        'home': match['home'],
        'away': match['away'],
        'matchday': match['matchday'],
        'league': match['competition'],
        'id': match['id'],
        'pick': best[0],
        'confidence': best[1],
        'probs': probs,
        'odds': odds_1x2,
        'timestamp': match['scraped_at'],
    }

def main():
    print(f"=== Betika Virtuals Scraper ===", flush=True)
    print(f"Leagues: {len(LEAGUES)}", flush=True)
    print(f"Data: {DATA_DIR}/", flush=True)

    all_matches = []

    hist_file = DATA_DIR / 'history.json'
    if hist_file.exists():
        try:
            with open(hist_file) as f: all_matches = json.load(f)
            print(f"Loaded {len(all_matches)} historical matches", flush=True)
        except:
            pass

    existing_ids = {m['id'] for m in all_matches}
    iteration = 0

    while True:
        iteration += 1
        now = datetime.now().isoformat()
        print(f"\n[{now}] #{iteration}", flush=True)

        try:
            parsed = {}
            for lid in LEAGUES:
                raw = fetch_matches(lid)
                for m in flatten(raw):
                    mid = m.get('parent_virtual_id', '')
                    if mid and mid not in parsed:
                        parsed[mid] = parse_match(m)

            all_current = list(parsed.values())
            n_upcoming = sum(1 for m in all_current if not m['outcome_id'] or not m['remaining'].startswith('-'))
            n_finished = sum(1 for m in all_current if m['outcome_id'] and m['remaining'].startswith('-'))

            new_ids = 0
            for m in all_current:
                if m['id'] not in existing_ids:
                    all_matches.append(m)
                    existing_ids.add(m['id'])
                    new_ids += 1

            print(f"  Total: {len(parsed)} ({n_upcoming} upcoming, {n_finished} finished, {new_ids} new)", flush=True)
            print(f"  History: {len(all_matches)} unique matches", flush=True)

            with open(hist_file, 'w') as f:
                json.dump(all_matches, f, indent=2, default=str)

            # Predict on matches without outcome_id
            preds = [predict_match(m) for m in all_current if not m['outcome_id']]
            preds = [p for p in preds if p is not None]
            preds.sort(key=lambda x: -x['confidence'])

            # Historical accuracy
            if len(all_matches) >= 5:
                correct = 0
                total_tracked = 0
                for m in all_matches:
                    if m['outcome_id'] and m['markets'].get('1X2'):
                        odds_1x2 = {k: v['odd'] for k, v in m['markets']['1X2'].items()}
                        if len(odds_1x2) == 3:
                            probs = compute_implied_probs(odds_1x2)
                            predicted = max(probs, key=probs.get)
                            actual = outcome_to_result(m['outcome_id'], m['markets'])
                            if predicted == actual:
                                correct += 1
                            total_tracked += 1

                if total_tracked > 0:
                    accuracy = round(correct / total_tracked * 100, 1)
                    print(f"\n  📊 Historical accuracy: {correct}/{total_tracked} ({accuracy}%)", flush=True)

            # Top predictions
            if preds:
                print(f"\n  🔮 Predictions: {len(preds)}", flush=True)
                for p in preds[:5]:
                    line = f"    ▶ {p['home']} vs {p['away']}: {p['pick']} ({p['confidence']}%)"
                    if p.get('league'):
                        line += f" [{p['league']}]"
                    print(line, flush=True)

            # Save predictions
            with open(DATA_DIR / 'predictions.json', 'w') as f:
                json.dump(preds, f, indent=2, default=str)

            state = {
                'timestamp': now,
                'iteration': iteration,
                'upcoming': n_upcoming,
                'completed': n_finished,
                'history_total': len(all_matches),
                'predictions': len(preds),
            }
            with open(DATA_DIR / 'state.json', 'w') as f:
                json.dump(state, f, indent=2)

        except Exception as e:
            print(f"  ❌ Error: {e}", flush=True)
            import traceback
            traceback.print_exc()

        print(f"\n  Sleeping {POLL_SECS}s...", flush=True)
        time.sleep(POLL_SECS)

if __name__ == '__main__':
    try:
        main()
    except KeyboardInterrupt:
        print("\nStopped.", flush=True)
