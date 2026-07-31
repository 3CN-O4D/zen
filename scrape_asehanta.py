#!/usr/bin/env python3
import json
import re
import time

from selenium import webdriver
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.common.action_chains import ActionChains


def init_driver():
    service = Service('/usr/bin/chromedriver')
    opts = webdriver.ChromeOptions()
    opts.add_argument('--headless=new')
    opts.add_argument('--no-sandbox')
    opts.add_argument('--disable-gpu')
    opts.add_argument('--disable-dev-shm-usage')
    opts.add_argument('--window-size=1920,1080')
    opts.add_experimental_option('excludeSwitches', ['enable-logging'])
    opts.binary_location = '/usr/bin/chromium'
    opts.set_capability('pageLoadStrategy', 'normal')
    driver = webdriver.Chrome(service=service, options=opts)
    driver.implicitly_wait(0)
    return driver


_RENT_RE = re.compile(r'ksh\s*[\d,]+', re.I)
_FEES_RE = re.compile(r'fees?\s*:\s*ksh\s*[\d,]+', re.I)
_TITLE_SKIP = re.compile(r'more\s*photos|click\s*here|listed\s*by', re.I)


def classify_texts(texts):
    out = {}
    for t in texts:
        s = t.strip()
        if not s or _TITLE_SKIP.search(s):
            continue
        if _FEES_RE.search(s):
            out['fees'] = s
        elif _RENT_RE.search(s):
            out['rent'] = s
        elif s.startswith('✅') or 'available' in s.lower():
            out['availability'] = s
        elif 'title' not in out and len(s) > 10:
            out['title'] = s
    if 'title' not in out:
        for t in texts:
            s = t.strip()
            if len(s) > 10 and not _TITLE_SKIP.search(s):
                out['title'] = s
                break
    return out


def js(driver, script, *args):
    try:
        return driver.execute_script(script, *args)
    except Exception:
        return None


def scrape():
    driver = init_driver()
    try:
        driver.get('https://asehanta.com')
        time.sleep(8)

        cards_json = js(driver, """
            var cards = document.querySelectorAll('.bubble-element.group-item');
            var result = [];
            for (var i = 0; i < cards.length; i++) {
                var c = cards[i];
                var entry = {};
                var img = c.querySelector('.bubble-element.Image img');
                entry.image_url = img ? img.src : '';
                var texts = c.querySelectorAll('.bubble-element.Text');
                entry.texts = [];
                for (var j = 0; j < texts.length; j++) {
                    var t = texts[j].textContent.trim();
                    if (t) entry.texts.push(t);
                }
                var btn = c.querySelector('.bubble-element.Button');
                entry.availability_btn = btn ? btn.textContent.trim() : '';
                result.push(entry);
            }
            return JSON.stringify(result);
        """)

        listings = json.loads(cards_json) if cards_json and cards_json != 'null' else []

        for item in listings:
            raw = item.pop('texts', [])
            item['_raw_texts'] = raw
            item.update(classify_texts(raw))
            if not item.get('availability') and item.get('availability_btn', '').startswith('✅'):
                item['availability'] = item['availability_btn']
            item.pop('availability_btn', None)

        for idx in range(len(listings)):
            try:
                _extract_popup(driver, idx, listings[idx])
            except Exception:
                pass
        return listings
    finally:
        driver.quit()


def _extract_popup(driver, idx, item):
    trigger_el = js(driver, """
        var card = document.querySelectorAll('.bubble-element.group-item')[arguments[0]];
        if (!card) return null;
        var all = card.querySelectorAll('*');
        for (var i = 0; i < all.length; i++) {
            if (all[i].textContent.indexOf('Click Here') >= 0) {
                return all[i];
            }
        }
        return null;
    """, idx)
    if not trigger_el:
        return
    try:
        ActionChains(driver).move_to_element(trigger_el).click(trigger_el).perform()
    except Exception:
        return
    time.sleep(4)

    popup = js(driver, """
        var p = document.querySelector('.bubble-element.Popup');
        if (!p) return 'no popup';
        var rg = p.querySelector('.bubble-element.RepeatingGroup');
        if (rg) { rg.scrollTop = rg.scrollHeight; }
        setTimeout(function() {
            if (rg) { rg.scrollTop = 0; }
            setTimeout(function() {
                if (rg) { rg.scrollTop = rg.scrollHeight; }
            }, 300);
        }, 300);
        var allImgs = rg ? rg.querySelectorAll('img') : [];
        var realImgs = [];
        for (var i = 0; i < allImgs.length; i++) {
            if (allImgs[i].src && !allImgs[i].src.startsWith('data:')) {
                realImgs.push(allImgs[i].src);
            }
        }
        return {
            images: realImgs,
            iframes: Array.from(p.querySelectorAll('iframe'))
                .map(function(i) { return i.src; })
                .filter(function(s) { return s; })
        };
    """)
    if isinstance(popup, dict):
        if popup.get('images'):
            item['popup_images'] = popup['images']
        if popup.get('iframes') and 'video_urls' not in item:
            item['video_urls'] = popup['iframes']

    js(driver, "document.querySelector('.bubble-element.Popup')?.remove();")
    time.sleep(0.5)


if __name__ == '__main__':
    data = scrape()
    output = json.dumps(data, indent=2, default=str, ensure_ascii=False)
    print(output)
    with open('/home/bard/Desktop/zen/asehanta_old_data.json', 'w') as f:
        f.write(output)
