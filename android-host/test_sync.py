import copy
import contextlib
import sqlite3
import tempfile
import unittest
from pathlib import Path
from sync_pc import choose, merge, snapshot, materialize


def fixture():
    def table(columns, keys, rows):
        return {"columns": columns.split(), "keys": keys.split(), "rows": {tuple(r[k] for k in keys.split()): r for r in rows}}
    return {
        "users": table("id username", "id", [{"id": "u", "username": "player"}]),
        "dated_player_stats": table("match_id user_id wins losses total_score", "match_id user_id", []),
        "player_stats": table("user_id wins losses total_score games_played", "user_id", [{"user_id": "u", "wins": 5, "losses": 2, "total_score": 500, "games_played": 7}]),
        "leaderboard_match_completions": table("match_id completed_at_ms", "match_id", []),
        "completed_match_players": table("match_id user_id hidden", "match_id user_id", [{"match_id": "old", "user_id": "u", "hidden": 0}]),
    }


def add_match(data, match, win, score):
    data["dated_player_stats"]["rows"][(match, "u")] = dict(match_id=match, user_id="u", wins=win, losses=1-win, total_score=score)
    data["leaderboard_match_completions"]["rows"][(match,)] = dict(match_id=match, completed_at_ms=1000)
    r = data["player_stats"]["rows"][("u",)]
    r["wins"] += win; r["losses"] += 1-win; r["total_score"] += score; r["games_played"] += 1


class SyncTests(unittest.TestCase):
    def uno(self):
        data=fixture()
        specs={"uno_tables":("id host_id capacity status match_id state_json saved_at", "id"),"uno_seats":("table_id seat user_id name","table_id seat"),"uno_results":("match_id table_id completed_at summary_json","match_id"),"uno_result_players":("match_id user_id won hidden","match_id user_id"),"uno_invitations":("id table_id sender_id receiver_id status","id"),"uno_reactions":("id table_id user_id emoji created_at","id")}
        for name,(columns,keys) in specs.items():data[name]=dict(columns=columns.split(),keys=keys.split(),rows={})
        data["uno_tables"]["rows"][("UNO1234",)]=dict(id="UNO1234",host_id="u",capacity=4,status="playing",match_id="round",state_json="pc-hand",saved_at=1000)
        data["uno_seats"]["rows"][("UNO1234",0)]=dict(table_id="UNO1234",seat=0,user_id="u",name="player")
        return data

    def test_uno_room_and_membership_are_chosen_together(self):
        base=self.uno();pc=copy.deepcopy(base);phone=copy.deepcopy(base)
        phone["uno_tables"]["rows"][("UNO1234",)].update(saved_at=3000,state_json="phone-hand")
        pc["uno_tables"]["rows"][("UNO1234",)].update(saved_at=2000,state_json="pc-new-hand")
        pc["uno_seats"]["rows"][("UNO1234",1)]=dict(table_id="UNO1234",seat=1,user_id=None,name="PC bot")
        phone["uno_seats"]["rows"][("UNO1234",2)]=dict(table_id="UNO1234",seat=2,user_id=None,name="Phone bot")
        result=merge(base,pc,phone)
        self.assertEqual(result["uno_tables"]["rows"][("UNO1234",)]["state_json"],"phone-hand")
        self.assertIn(("UNO1234",2),result["uno_seats"]["rows"])
        self.assertNotIn(("UNO1234",1),result["uno_seats"]["rows"])

    def test_uno_schema_can_extend_an_old_common_baseline(self):
        base=fixture();pc=self.uno();phone=copy.deepcopy(pc)
        result=merge(base,pc,phone)
        self.assertEqual(result["uno_tables"]["rows"],pc["uno_tables"]["rows"])
        self.assertEqual(result["users"]["rows"],base["users"]["rows"])

    def test_uno_results_are_unique_hidden_per_user_and_do_not_change_domino_totals(self):
        base=self.uno();pc=copy.deepcopy(base);phone=copy.deepcopy(base)
        for peer in (pc,phone):
            peer["uno_results"]["rows"][("result",)]=dict(match_id="result",table_id="UNO1234",completed_at=1000,summary_json="{}")
            peer["uno_result_players"]["rows"][("result","u")]=dict(match_id="result",user_id="u",won=1,hidden=0)
        phone["uno_result_players"]["rows"][("result","u")]["hidden"]=1
        result=merge(base,pc,phone)
        self.assertEqual(len(result["uno_result_players"]["rows"]),1)
        self.assertEqual(result["uno_result_players"]["rows"][("result","u")]["hidden"],1)
        self.assertEqual(result["player_stats"]["rows"],base["player_stats"]["rows"])

    def test_active_uno_account_conflict_stops_sync_instead_of_damaging_seat_order(self):
        base=self.uno();pc=copy.deepcopy(base);phone=copy.deepcopy(base)
        phone["users"]["rows"]={}
        with self.assertRaisesRegex(RuntimeError,"participant was deleted"):merge(base,pc,phone)

    def test_reaction_uses_upgrade_baseline_and_merge_offline_clicks_once(self):
        base=fixture();pc=copy.deepcopy(base);phone=copy.deepcopy(base)
        spec=dict(columns=["id","user_id","emoji"],keys=["id"],rows={})
        for data in (pc,phone):data["reaction_uses"]=copy.deepcopy(spec)
        for i in range(2):
            pc["reaction_uses"]["rows"][(f"pc-{i}",)]=dict(id=f"pc-{i}",user_id="u",emoji="😭")
            phone["reaction_uses"]["rows"][(f"phone-{i}",)]=dict(id=f"phone-{i}",user_id="u",emoji="😭")
        combined=merge(base,pc,phone)
        self.assertEqual(len(combined["reaction_uses"]["rows"]),4)
        self.assertEqual(merge(base,combined,combined),combined)

    def test_newer_match_wins(self):
        old = dict(updated_at="2026-11-01 12:00:00", state="old")
        pc = dict(updated_at="2026-11-03 12:00:00", state="pc")
        phone = dict(updated_at="2026-11-04 12:00:00", state="phone")
        self.assertEqual(choose(old, pc, phone), phone)
        self.assertEqual(choose(old, phone, pc), phone)

    def test_deletion_does_not_resurrect(self):
        row = dict(id="deleted")
        self.assertIsNone(choose(row, None, row))
        self.assertIsNone(choose(row, row, None))

    def test_independent_matches_add_without_losing_legacy_totals(self):
        base = fixture(); pc = copy.deepcopy(base); phone = copy.deepcopy(base)
        add_match(pc, "pcmatch", 1, 90); add_match(phone, "phonematch", 0, 70)
        merged = merge(base, pc, phone)
        self.assertEqual(merged["player_stats"]["rows"][("u",)], dict(user_id="u", wins=6, losses=3, total_score=660, games_played=9))
        self.assertEqual(merge(merged, merged, merged), merged)

    def test_same_completed_match_counted_once(self):
        base = fixture(); pc = copy.deepcopy(base); phone = copy.deepcopy(base)
        add_match(pc, "same", 1, 90); add_match(phone, "same", 1, 90)
        merged = merge(base, pc, phone)
        self.assertEqual(merged["player_stats"]["rows"][("u",)]["games_played"], 8)

    def test_hidden_history_propagates(self):
        base = fixture(); pc = copy.deepcopy(base); phone = copy.deepcopy(base)
        phone["completed_match_players"]["rows"][("old", "u")]["hidden"] = 1
        merged = merge(base, pc, phone)
        self.assertEqual(merged["completed_match_players"]["rows"][("old", "u")]["hidden"], 1)

    def test_integer_ids_allocated_offline_do_not_overwrite(self):
        base = fixture()
        base["friendships"] = dict(columns=["id", "sender_id", "receiver_id"], keys=["id"], rows={})
        pc = copy.deepcopy(base); phone = copy.deepcopy(base)
        pc["friendships"]["rows"][(1,)] = dict(id=1, sender_id="u", receiver_id="a")
        phone["friendships"]["rows"][(1,)] = dict(id=1, sender_id="u", receiver_id="b")
        self.assertEqual(len(merge(base, pc, phone)["friendships"]["rows"]), 2)

    def test_parent_deletion_cascades_in_validated_output(self):
        with tempfile.TemporaryDirectory() as temp:
            source, output = Path(temp)/"source.db", Path(temp)/"output.db"
            with contextlib.closing(sqlite3.connect(source)) as con:
                con.executescript("CREATE TABLE users(id TEXT PRIMARY KEY); CREATE TABLE child(id TEXT PRIMARY KEY, user_id TEXT REFERENCES users(id)); INSERT INTO users VALUES('u'); INSERT INTO child VALUES('c','u');")
            data = snapshot(source); data["users"]["rows"] = {}
            materialize(source, output, data)
            self.assertEqual(snapshot(output)["child"]["rows"], {})

    def test_turn_timer_column_upgrades_old_baseline_and_keeps_choices(self):
        base = fixture()
        base["lobbies"] = {"columns": ["id", "draw_rule"], "keys": ["id"], "rows": {("T1",): {"id": "T1", "draw_rule": "anytime"}}}
        pc = copy.deepcopy(base); phone = copy.deepcopy(base)
        for data in (pc, phone):
            data["lobbies"]["columns"].append("turn_timer")
            data["lobbies"]["rows"][("T1",)]["turn_timer"] = 1
        phone["lobbies"]["rows"][("T2",)] = {"id": "T2", "draw_rule": "anytime", "turn_timer": 0}
        result = merge(base, pc, phone)
        self.assertEqual(result["lobbies"]["rows"][("T1",)]["turn_timer"], 1)
        self.assertEqual(result["lobbies"]["rows"][("T2",)]["turn_timer"], 0)
        self.assertEqual(result["lobbies"]["columns"], ["id", "draw_rule", "turn_timer"])

    def test_turn_timer_on_only_one_host_still_refuses_sync(self):
        base = fixture()
        base["lobbies"] = {"columns": ["id"], "keys": ["id"], "rows": {}}
        pc = copy.deepcopy(base); phone = copy.deepcopy(base)
        pc["lobbies"]["columns"].append("turn_timer")
        with self.assertRaisesRegex(RuntimeError, "Schema differs for lobbies"):
            merge(base, pc, phone)

    def test_schema_mismatch_refuses_sync(self):
        base = fixture(); phone = copy.deepcopy(base); phone["newtable"] = {}
        with self.assertRaisesRegex(RuntimeError, "schemas differ"):
            merge(base, base, phone)


if __name__ == "__main__":
    unittest.main()
