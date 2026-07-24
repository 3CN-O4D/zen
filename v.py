import requests
import random
import time
import sys

URL = "https://cube-community-api.vercel.app/api/v1/peoples-choice/vote"
APPLICANT_ID = "seed-0"+input("seed: ")
TRACK = input("track : ")#"Agriculture & Food"
DELAY = 0.2

HEADERS = {
    "Host": "cube-community-api.vercel.app",
    "Sec-Ch-Ua-Platform": '"Linux"',
    "Accept-Language": "en-US,en;q=0.9",
    "Accept": "application/json, text/plain, */*",
    "Sec-Ch-Ua": '"Not-A.Brand";v="24", "Chromium";v="146"',
    "Content-Type": "application/json",
    "Sec-Ch-Ua-Mobile": "?0",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
    "Origin": "https://the-cube.co.ke",
    "Sec-Fetch-Site": "cross-site",
    "Sec-Fetch-Mode": "cors",
    "Sec-Fetch-Dest": "empty",
    "Referer": "https://the-cube.co.ke/",
    "Accept-Encoding": "gzip, deflate, br",
    "Priority": "u=1, i",
}


def random_fingerprint():
    return "".join(random.choices("0123456789abcdef", k=64))


def banner():
    print("\033[1;36m╔══════════════════════════════════════════════╗")
    print("║     \033[1;33mZen Vote Bot  —  Peoples Choice\033[1;36m        ║")
    print("╠══════════════════════════════════════════════╣")
    print(f"║  Applicant: \033[1;33m{APPLICANT_ID}\033[1;36m")
    print(f"║  Track:     \033[1;33m{TRACK}\033[1;36m")
    print("║  Mode:      \033[1;33mUnlimited (Ctrl+C to stop)\033[1;36m")
    print("╚══════════════════════════════════════════════╝")
    print()


def results(success, failed, count):
    print()
    print("\033[1;36m╔══════════════════════════════════════════════╗")
    print("║                    \033[1;33mRESULTS\033[1;36m                    ║")
    print("╠══════════════════════════════════════════════╣")
    print(f"║  Votes cast:       \033[1;33m{count}\033[1;36m")
    print(f"║  Successful:       \033[1;32m{success}\033[1;36m")
    print(f"║  Failed:           \033[1;31m{failed}\033[1;36m")
    print("╚══════════════════════════════════════════════╝\033[0m")


def main():
    banner()
    print("\033[1;33mPress Ctrl+C to stop\033[0m")
    print()

    success = 0
    failed = 0
    count = 0

    try:
        while True:
            count += 1
            fp = random_fingerprint()
            body = {
                "applicantId": APPLICANT_ID,
                "track": TRACK,
                "fingerprint": fp,
            }

            try:
                resp = requests.post(URL, json=body, headers=HEADERS, timeout=15)
                if resp.ok:
                    data = resp.json()
                    msg = data.get("message") or resp.text
                    success += 1
                    print(f"\033[1;36m[{count}]\033[0m \033[1;32m✓\033[0m {msg}")
                else:
                    failed += 1
                    print(f"\033[1;36m[{count}]\033[0m \033[1;31m✗\033[0m HTTP {resp.status_code} — {resp.text}")
            except requests.RequestException as e:
                failed += 1
                print(f"\033[1;36m[{count}]\033[0m \033[1;31m✗\033[0m {e}")

            time.sleep(DELAY)
    except KeyboardInterrupt:
        print("\n\033[1;33mAborted.\033[0m")
        results(success, failed, count)
        sys.exit(0)


if __name__ == "__main__":
    main()
