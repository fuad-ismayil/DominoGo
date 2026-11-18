"""Black-box shared-host acceptance test. Uses an isolated temporary database.

Build first: cargo build --manifest-path rust/Cargo.toml -p domino-server
Run from the repository root: python unogo/tests/live_host.py
"""
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request


ROOT = Path(__file__).resolve().parents[2]
BINARY = ROOT / "rust/target/debug" / ("domino-server.exe" if os.name == "nt" else "domino-server")


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def api(port, path, token="", body=None, expected=200, method=None):
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = "Bearer " + token
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}{path}",
        data=None if body is None else json.dumps(body).encode(),
        headers=headers,
        method=method or ("GET" if body is None else "POST"),
    )
    try:
        response = urllib.request.urlopen(request, timeout=10)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        data = json.loads(response.read())
        assert response.status == expected, (path, response.status, data)
        return data


def auto_action(view):
    phase = view["phase"]
    if isinstance(phase,dict) and "Swap" in phase:
        return "KeepHand"
    if isinstance(phase,dict) and "TouchFive" in phase:
        return {"TouchFive":{"challenge":phase["TouchFive"]["challenge"]}}
    if phase == "OpeningColor":
        return {"ChooseColor": "Blue"}
    if isinstance(phase, dict) and "Uno" in phase:
        return "CallUno"
    if isinstance(phase, dict) and "Challenge" in phase:
        return "AcceptFour"
    playable = [c for c in view["hand"] if c["id"] in view["playable"]
                and not (c["value"] == "WildFour" and view["four_warning"])]
    if playable:
        card = playable[0]
        color = max(["Red", "Yellow", "Green", "Blue"],
                    key=lambda col: sum(c["color"] == col for c in view["hand"]))
        return {"Play": {"card": card["id"], "color": color if card["color"] is None else None, "uno": True}}
    return "Keep" if isinstance(phase, dict) and "Drawn" in phase else "Draw"


def main():
    assert BINARY.exists(), "Build domino-server first"
    domino, uno = free_port(), free_port()
    while uno == domino:
        uno = free_port()
    with tempfile.TemporaryDirectory(prefix="unogo-host-test-") as directory:
        env = dict(os.environ, DOMINO_PORT=str(domino), UNOGO_PORT=str(uno),
                   DOMINO_DB_PATH=str(Path(directory) / "test.sqlite"))
        env.pop("DOMINO_STATIC_DIR", None)
        env.pop("UNOGO_STATIC_DIR", None)
        log_path = Path(directory) / "server.log"

        def launch():
            log = log_path.open("ab")
            process = subprocess.Popen([str(BINARY)], cwd=directory, env=env,
                                       stdout=log, stderr=log,
                                       creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
            log.close()
            for _ in range(150):
                if process.poll() is not None:
                    raise AssertionError(log_path.read_text())
                try:
                    api(uno, "/api/health")
                    return process
                except (OSError, urllib.error.URLError):
                    time.sleep(0.1)
            process.terminate()
            process.wait(timeout=10)
            raise AssertionError("Host did not become ready")

        process = launch()
        try:
            password = "test-only-passphrase"
            a = api(domino, "/api/auth/register", body={"username": "HostUser", "password": password})
            b = api(uno, "/api/auth/register", body={"username": "FriendUser", "password": password})
            login = api(uno, "/api/auth/login", body={"username": "HostUser", "password": password})
            assert a["user"]["id"] == login["user"]["id"]
            assert api(domino, "/api/auth/me", login["token"])["user"]["id"] == a["user"]["id"]
            api(uno, "/api/friends/request", a["token"], {"username": "FriendUser"})
            pending = api(domino, "/api/friends", b["token"])["incoming"][0]["id"]
            api(domino, "/api/friends/accept", b["token"], {"friendshipId": pending})
            assert api(uno, "/api/friends", a["token"])["friends"][0]["id"] == b["user"]["id"]
            guest = api(uno, "/api/auth/guest", body={"prefix": "Saved"})
            saved = api(uno, "/api/auth/upgrade", guest["token"], {"newUsername": "SavedGuest", "password": password})
            assert saved["user"]["id"] == guest["user"]["id"]
            assert api(domino, "/api/auth/login", body={"username": "SavedGuest", "password": password})["user"]["id"] == guest["user"]["id"]
            legacy = api(domino, "/api/lobbies", a["token"], {"maxPlayers": 2})
            api(domino, f'/api/lobbies/{legacy["id"]}/add-bot', a["token"], {})
            api(domino, f'/api/lobbies/{legacy["id"]}/start', a["token"], {})
            table = api(uno, "/api/lobbies", a["token"], {"maxPlayers": 2})
            url = f'/api/lobbies/{table["id"]}'
            wrong = api(domino, url + "/join", a["token"], {}, expected=409)
            assert wrong["code"] == "WRONG_GAME"
            api(uno, url + "/join", b["token"], {})
            state = api(uno, url + "/start", a["token"], {})
            match_id = state["matchId"]
            for user in [a, b]:
                state = api(uno, url + "/pause", user["token"], {"matchId": match_id})
            assert state["status"] == "paused"
            hand = state["game"]["hand"]
            process.terminate()
            process.wait(timeout=10)
            process = launch()
            restored = api(uno, url, b["token"])
            assert restored["game"]["hand"] == hand and restored["status"] == "paused"
            api(uno, url + "/resume", a["token"], {"matchId": match_id})
            for _ in range(10000):
                state = api(uno, url, a["token"])
                if state["status"] == "finished":
                    break
                phase = state["game"]["phase"]
                seat = phase["Uno"]["player"] if isinstance(phase, dict) and "Uno" in phase else state["game"]["turn"]
                if isinstance(phase,dict) and "TouchFive" in phase:
                    data=phase["TouchFive"]
                    if not state["game"]["touch_ready"]:
                        time.sleep(.1);continue
                    seat=next(i for i in range(len(state["players"])) if i not in data["taps"])
                token = [a, b][seat]["token"]
                state = api(uno, url, token)
                api(uno, url + "/commands", token, {"matchId": match_id, "sequence": state["acceptedSequence"] + 1,
                                                      "action": auto_action(state["game"])})
            else:
                raise AssertionError("Round did not finish")
            assert len(api(uno, "/api/matches/history", b["token"])) == 1
            assert api(uno, f'/api/stats/{a["user"]["id"]}', a["token"])["gamesPlayed"] == 1
            assert len(api(uno, "/api/leaderboard", a["token"])["players"]) == 2
            # The concurrently running DominoGo table and original login survive UNO play.
            assert api(domino, f'/api/lobbies/{legacy["id"]}/join', a["token"], {})["status"] == "playing"
            api(uno, "/api/auth/logout-others", login["token"], {})
            api(domino, "/api/auth/me", a["token"], expected=401)
            api(uno, url, a["token"], expected=401)
            print("PASS: shared login, guest upgrade, friends, wrong-game routing, private rounds, restart/rejoin, UNO stats, session revocation and concurrent DominoGo table")
        finally:
            process.terminate()
            process.wait(timeout=10)


if __name__ == "__main__":
    main()
