"""Paired, offline, three-way SQLite sync over an authorized USB/wireless ADB connection.

The common baseline records deletions. Live engines are never rewritten. Snapshots
and recovery copies stay in the user's private state directory, outside the repo.
"""
from __future__ import annotations
import argparse
import contextlib
import csv
import ctypes
import datetime as dt
import io
import json
import os
from pathlib import Path
import re
import shutil
import sqlite3
import subprocess
import time
import tempfile
import uuid

PACKAGE = "games.dominogo.host"
ROOT = Path(__file__).resolve().parent.parent
STATE = Path.home() / ".codex" / "domino-phone-sync"
COUNTERS = ("wins", "losses", "total_score", "games_played")


def quote(name):
    return '"' + name.replace('"', '""') + '"'


def snapshot(path):
    with contextlib.closing(sqlite3.connect(path)) as con:
        con.row_factory = sqlite3.Row
        result = {}
        for (table,) in con.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"):
            columns = list(con.execute(f"PRAGMA table_info({quote(table)})"))
            keys = [r[1] for r in sorted(columns, key=lambda r: r[5]) if r[5]]
            if not keys:
                raise RuntimeError(f"Cannot sync table without primary key: {table}")
            rows = [dict(r) for r in con.execute(f"SELECT * FROM {quote(table)}")]
            result[table] = {"columns": [r[1] for r in columns], "keys": keys,
                             "rows": {tuple(row[k] for k in keys): row for row in rows}}
        return result


def stamp(row):
    if row is None:
        return 0.0
    for field in ("saved_at", "updated_at", "completed_at_ms", "completed_at", "last_active_at", "created_at", "hidden_at"):
        value = row.get(field)
        if value is None:
            continue
        try:
            if field.endswith("_ms") or field in ("saved_at","completed_at"):
                return float(value) / 1000
            parsed = dt.datetime.fromisoformat(str(value).replace("Z", "+00:00"))
            if parsed.tzinfo is None:
                parsed = parsed.replace(tzinfo=dt.timezone.utc)
            return parsed.timestamp()
        except (ValueError, TypeError):
            pass
    return 0.0


def choose(base, pc, phone):
    if pc == phone:
        return pc
    if pc == base:
        return phone
    if phone == base:
        return pc
    # A deletion since the common baseline must not be resurrected by another device.
    if pc is None or phone is None:
        return None
    # History deletion is per account and remains hidden on both devices.
    if "hidden" in pc and "hidden" in phone:
        return dict(pc if stamp(pc) >= stamp(phone) else phone, hidden=max(pc["hidden"], phone["hidden"]))
    return pc if stamp(pc) >= stamp(phone) else phone


def legacy_stats(data, user):
    row = data["player_stats"]["rows"].get((user,), {})
    values = {k: row.get(k, 0) for k in COUNTERS}
    for r in data["dated_player_stats"]["rows"].values():
        if r["user_id"] == user:
            for k in COUNTERS:
                values[k] -= 1 if k == "games_played" else r[k]
    return values


def merge(base, pc, phone):
    # This additive table did not exist in older common baselines. Its empty
    # history is unambiguous once both current databases have the new schema.
    for table in ("reaction_uses","uno_tables","uno_seats","uno_results","uno_result_players","uno_invitations","uno_reactions"):
        if table not in base and table in pc and table in phone:
            base = dict(base, **{table:dict(pc[table], rows={})})
    if set(base) != set(pc) or set(pc) != set(phone):
        raise RuntimeError("Database schemas differ. Update both apps before syncing.")
    merged = {}
    for table, spec in pc.items():
        if spec["columns"] != phone[table]["columns"] or spec["columns"] != base[table]["columns"]:
            raise RuntimeError(f"Schema differs for {table}; update both apps first")
        b, p, a = [data[table]["rows"] for data in (base, pc, phone)]
        result = {}
        # Integer IDs can be allocated independently offline. Preserve both new records.
        next_id = max([k[0] for k in set(p) | set(a) if len(k) == 1 and isinstance(k[0], int)] + [0]) + 1
        for key in sorted(set(b) | set(p) | set(a), key=repr):
            bv, pv, av = b.get(key), p.get(key), a.get(key)
            if table in ("friendships", "game_invitations") and bv is None and pv is not None and av is not None and pv != av:
                natural = ("sender_id", "receiver_id") if table == "friendships" else ("lobby_id", "sender_id", "receiver_id", "created_at")
                if any(pv[k] != av[k] for k in natural):
                    result[key] = pv
                    result[(next_id,)] = dict(av, id=next_id)
                    next_id += 1
                    continue
            value = choose(bv, pv, av)
            if value is not None:
                result[key] = dict(value)
        merged[table] = dict(spec, rows=result)
    if "uno_tables" in merged:
        # A room and its ordered membership are one saved game. Never combine
        # one host's private hands with independently merged seats from another.
        coherent_seats={}
        for key, room in merged["uno_tables"]["rows"].items():
            source=pc if pc["uno_tables"]["rows"].get(key)==room else phone
            for seat_key,seat in source["uno_seats"]["rows"].items():
                if seat["table_id"]==room["id"]:
                    coherent_seats[seat_key]=dict(seat)
            if room["status"] in ("playing","paused"):
                participants=[s["user_id"] for s in coherent_seats.values() if s["table_id"]==room["id"] and s["user_id"] is not None]
                if any((user,) not in merged["users"]["rows"] for user in participants):
                    raise RuntimeError("A saved UNO participant was deleted on the other host; review recovery copies before syncing")
        merged["uno_seats"]["rows"]=coherent_seats
        merged["uno_reactions"]["rows"]={} # public three-second bubbles are transient
    # Match status belongs to the chosen version of the actual saved game.
    for key, game in merged.get("game_states", {}).get("rows", {}).items():
        lobby = merged["lobbies"]["rows"].get(key)
        try:
            status = json.loads(game["state_json"]).get("status")
            if lobby and status in ("waiting", "playing", "stopped", "finished"):
                lobby["status"] = status
        except (ValueError, TypeError):
            pass
    # Rebuild totals from the union of match results, preserving pre-tracking totals.
    # A match completed on both copies counts once, not twice.
    if "dated_player_stats" in merged:
        totals = {}
        for (user,), _ in merged["users"]["rows"].items():
            b, p, a = [legacy_stats(d, user) for d in (base, pc, phone)]
            values = p if a == b else a if p == b else p if p == a else {k: p[k] + a[k] - b[k] for k in COUNTERS}
            values = dict(values)
            records = [r for r in merged["dated_player_stats"]["rows"].values() if r["user_id"] == user]
            for r in records:
                for k in COUNTERS:
                    values[k] += 1 if k == "games_played" else r[k]
            if records or any(values.values()) or any((user,) in d["player_stats"]["rows"] for d in (pc, phone)):
                totals[(user,)] = dict(user_id=user, **values)
        merged["player_stats"]["rows"] = totals
    return merged


def backup(source, output):
    output.parent.mkdir(parents=True, exist_ok=True)
    with contextlib.closing(sqlite3.connect(source, timeout=10)) as src, contextlib.closing(sqlite3.connect(output, timeout=10)) as dst:
        src.backup(dst)


def ensure_reaction_schema(database):
    # The PC executable may not have run yet after an app update. Apply only
    # this additive migration under the existing offline sync lock.
    with contextlib.closing(sqlite3.connect(database)) as con:
        con.executescript("""CREATE TABLE IF NOT EXISTS reaction_uses (
            id TEXT PRIMARY KEY, user_id TEXT NOT NULL, emoji TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE);
            CREATE INDEX IF NOT EXISTS reaction_uses_by_user ON reaction_uses (user_id, emoji);""")
        con.executescript((ROOT/"unogo/server/schema.sql").read_text(encoding="utf8"))


def materialize(template, output, data):
    backup(template, output)
    with contextlib.closing(sqlite3.connect(output)) as con:
        con.execute("PRAGMA foreign_keys=OFF")
        con.execute("BEGIN IMMEDIATE")
        for table in data:
            con.execute(f"DELETE FROM {quote(table)}")
        for table, spec in data.items():
            cols = spec["columns"]
            sql = f"INSERT INTO {quote(table)} ({','.join(map(quote, cols))}) VALUES ({','.join('?' for _ in cols)})"
            con.executemany(sql, [[r[c] for c in cols] for r in spec["rows"].values()])
        # Parent deletions also delete their dependent records, matching ON DELETE CASCADE.
        for _ in range(len(data)):
            violations = list(con.execute("PRAGMA foreign_key_check"))
            if not violations:
                break
            for table, rowid, parent, constraint in violations:
                con.execute(f"DELETE FROM {quote(table)} WHERE rowid=?", (rowid,))
        if list(con.execute("PRAGMA foreign_key_check")):
            raise RuntimeError("Merged database has broken references")
        if con.execute("PRAGMA integrity_check").fetchone()[0] != "ok":
            raise RuntimeError("Merged database failed integrity check")
        con.commit()


class Phone:
    def __init__(self, serial, adb="adb"):
        self.serial, self.adb, self.pairing_id = serial, adb, serial

    def resolve_transport(self):
        state = subprocess.run([self.adb, "-s", self.serial, "get-state"], capture_output=True, timeout=10)
        if state.returncode == 0 and state.stdout.strip() == b"device":
            return
        # Wireless ADB uses a different address/transport name. Match the explicitly
        # paired physical device rather than starting a new database pairing.
        devices = subprocess.run([self.adb, "devices"], capture_output=True, text=True, timeout=10)
        for line in devices.stdout.splitlines()[1:]:
            parts = line.split()
            if len(parts) < 2 or parts[1] != "device":
                continue
            identity = subprocess.run([self.adb, "-s", parts[0], "shell", "getprop", "ro.serialno"], capture_output=True, text=True, timeout=10)
            if identity.returncode == 0 and identity.stdout.strip() == self.pairing_id:
                self.serial = parts[0]
                return
        raise RuntimeError("Paired phone is not connected")

    def run(self, *args, data=None, check=True):
        result = subprocess.run([self.adb, "-s", self.serial, *args], input=data, capture_output=True, timeout=30)
        if check and result.returncode:
            raise RuntimeError(result.stderr.decode(errors="replace").strip() or "ADB operation failed")
        return result.stdout

    def read(self, name):
        return self.run("exec-out", "run-as", PACKAGE, "cat", "files/" + name)

    def write(self, name, data):
        if name not in ("sync-incoming.sqlite", "sync-summary.json", "hosting-config.json"):
            raise ValueError("Unexpected file")
        self.run("shell", "run-as", PACKAGE, "mkdir", "-p", "files")
        remote = "/data/local/tmp/domino-sync-" + uuid.uuid4().hex
        with tempfile.TemporaryDirectory() as temp:
            local = Path(temp) / "payload"
            local.write_bytes(data)
            try:
                self.run("push", str(local), remote)
                self.run("shell", "run-as", PACKAGE, "cp", remote, "files/" + name)
            finally:
                self.run("shell", "rm", "-f", remote, check=False)

    def operation(self, operation):
        request = uuid.uuid4().hex
        self.run("shell", "am", "broadcast", "--include-stopped-packages", "-n", PACKAGE + "/.SyncReceiver", "-a", PACKAGE + ".SYNC", "--es", "operation", operation, "--es", "request", request)
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            try:
                response = json.loads(self.read("sync-result.json"))
                if response.get("request") == request:
                    if not response["ok"]:
                        raise RuntimeError(response["message"])
                    return
            except (json.JSONDecodeError, UnicodeDecodeError):
                pass
            except RuntimeError as e:
                if "No such file" not in str(e):
                    raise
            time.sleep(.2)
        raise RuntimeError("Phone sync did not respond")

    def status(self, message, ok=True):
        self.write("sync-summary.json", json.dumps({"message": message, "ok": ok, "updated": int(time.time() * 1000)}).encode())


def pc_running():
    if os.name != "nt":
        return False
    result = subprocess.run(["tasklist", "/FI", "IMAGENAME eq domino-server.exe", "/FO", "CSV", "/NH"], capture_output=True, text=True, timeout=10)
    return any(r and r[0].lower() == "domino-server.exe" for r in csv.reader(io.StringIO(result.stdout)))


@contextlib.contextmanager
def sync_lock():
    path = str(ROOT / ".domino-sync.lock")
    if os.name == "nt":
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.CreateFileW.argtypes = [ctypes.c_wchar_p, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_void_p, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_void_p]
        kernel.CreateFileW.restype = ctypes.c_void_p
        handle = kernel.CreateFileW(path, 0xC0000000, 0, None, 4, 0x80, None)
        if handle == ctypes.c_void_p(-1).value:
            raise RuntimeError("PC server is running or another sync is in progress")
        try:
            yield
        finally:
            kernel.CloseHandle.argtypes = [ctypes.c_void_p]
            kernel.CloseHandle(handle)
    else:
        with open(path, "a"):
            yield


def sync_once(phone, database=ROOT / "database.sqlite", state_root=STATE):
    device_dir = state_root / re.sub(r"[^a-zA-Z0-9_.-]", "_", phone.pairing_id)
    device_dir.mkdir(parents=True, exist_ok=True)
    base_path = device_dir / "baseline.sqlite"
    pc_path, phone_path, merged_path = [device_dir / n for n in ("pc.sqlite", "phone.sqlite", "merged.sqlite")]
    with sync_lock():
        if pc_running():
            raise RuntimeError("Waiting for the PC server to stop before syncing")
        ensure_reaction_schema(database)
        backup(database, pc_path)
        pc = snapshot(pc_path)
        if not base_path.exists():
            # A new pairing seeds only a truly absent phone DB. Never replace existing phone data.
            check = phone.run("shell", f"run-as {PACKAGE} sh -c 'if test -e files/database.sqlite; then echo exists; fi'", check=False)
            if b"exists" in check:
                raise RuntimeError("Phone already has data without a common baseline. Restore the original pairing state; automatic replacement is blocked.")
            phone.write("sync-incoming.sqlite", pc_path.read_bytes())
            phone.operation("seed")
            shutil.copy2(pc_path, base_path)
            phone.status("PC database copied to this phone")
            return "Seeded phone from the PC database"
        phone.operation("export")
        phone_path.write_bytes(phone.read("sync-before.sqlite"))
        before = snapshot(base_path)
        mobile = snapshot(phone_path)
        target = merge(before, pc, mobile)
        materialize(pc_path, merged_path, target)
        target = snapshot(merged_path)
        if target == pc and target == mobile:
            phone.status("Database is in sync with PC")
            # Advance baseline after no-op too, so propagated deletions remain authoritative.
            shutil.copy2(merged_path, base_path)
            return "Already in sync"
        recovery = device_dir / "backups" / dt.datetime.now().strftime("%Y%m%d-%H%M%S-%f")
        recovery.mkdir(parents=True)
        shutil.copy2(pc_path, recovery / "pc.sqlite")
        shutil.copy2(phone_path, recovery / "phone.sqlite")
        shutil.copy2(base_path, recovery / "baseline.sqlite")
        # Keep the Windows write lock while updating both peers. No PC writes can race the commit.
        with contextlib.closing(sqlite3.connect(database, timeout=10)) as current:
            current.execute("BEGIN IMMEDIATE")
            if snapshot(database) != pc:
                raise RuntimeError("PC database changed during sync; retry")
            if target != mobile:
                phone.write("sync-incoming.sqlite", merged_path.read_bytes())
                phone.operation("import")
            if pc_running():
                raise RuntimeError("PC server started during sync; stop it and retry")
            if target != pc:
                for table in target:
                    current.execute(f"DELETE FROM {quote(table)}")
                for table, spec in target.items():
                    columns = spec["columns"]
                    sql = f"INSERT INTO {quote(table)} ({','.join(map(quote, columns))}) VALUES ({','.join('?' for _ in columns)})"
                    current.executemany(sql, [[r[c] for c in columns] for r in spec["rows"].values()])
                if list(current.execute("PRAGMA foreign_key_check")):
                    raise RuntimeError("PC database update has broken references; changes rolled back")
            current.commit()
        shutil.copy2(merged_path, base_path)
        phone.status("Database synced with PC")
        return "Synced PC and phone databases"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serial", required=True, help="The explicitly paired ADB device serial")
    parser.add_argument("--watch", action="store_true")
    parser.add_argument("--adb", default="adb")
    args = parser.parse_args()
    phone = Phone(args.serial, args.adb)
    last_message = ""
    while True:
        try:
            phone.resolve_transport()
            message = sync_once(phone)
        except (RuntimeError, sqlite3.Error, OSError, subprocess.SubprocessError) as e:
            message = str(e)
            if "offline" not in message.lower() and "not found" not in message.lower() and "not connected" not in message.lower():
                try:
                    phone.status(message, False)
                except Exception:
                    pass
            if not args.watch:
                raise SystemExit(message)
        if message != last_message:
            print(dt.datetime.now().isoformat(timespec="seconds"), message, flush=True)
            last_message = message
        if not args.watch:
            return
        time.sleep(15)


if __name__ == "__main__":
    main()
