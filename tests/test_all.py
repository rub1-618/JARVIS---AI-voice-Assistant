import sys, os, aifc, random, string, uuid
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
sys.path.append(os.path.join(os.path.dirname(__file__), "../web/server"))
from web.server.db import Base, engine

email = f"test{random.randint(1000,9999)}@test.com"
key = str(uuid.uuid4())
hwid = str(uuid.uuid4())
name = ''.join(random.choices(string.ascii_lowercase, k=8))
description = "test description"
downloads = random.randint(0, 1000)
code = "test code"
author_id = random.randint(1, 9999)
is_verified = False
email_val = f"test_val{random.randint(1000,9999)}@test.com"


def setup_module():
    Base.metadata.create_all(bind=engine)


# ── _strip_markdown ────────────────────────────────────────────────────────────

from main import _strip_markdown

def test_strip_bold():
    text = "**bold** text"
    assert _strip_markdown(text) == "bold text"

def test_strip_big_bold():
    text = "***big bold*** text"
    assert _strip_markdown(text) == "big bold text"

def test_strip_bullet():
    text = "- bullet text"
    assert _strip_markdown(text) == "bullet text"

def test_strip_strike():
    text = "~~strike~~ text"
    assert _strip_markdown(text) == "strike text"

def test_strip_link():
    text = "[text](https://url.com)"
    assert _strip_markdown(text) == "text"

def test_strip_header():
    text = "## header text"
    assert _strip_markdown(text) == "header text"

def test_strip_under():
    text = "_under_ text"
    assert _strip_markdown(text) == "under text"

def test_strip_code():
    text = "`code` text"
    assert _strip_markdown(text) == "text"

def test_strip_blank():
    text = ""
    assert _strip_markdown(text) == ""

def test_strip_no_markdown():
    text = "text"
    assert _strip_markdown(text) == "text"

# ── recognize_cmd ──────────────────────────────────────────────────────────────

from main import recognize_cmd
def test_recognize_cmd():
    result = ""
    assert recognize_cmd(result) == {"cmd": "", "percent": 0}

# ── FastAPI ────────────────────────────────────────────────────────────────────

from fastapi.testclient import TestClient
from web.server.main import app

client = TestClient(app)

def test_get_root():
    r = client.get("/")
    assert r.json() == {"status": "ok"}

def test_post_register():
    r = client.post("/register", json={"email": email, "password": "123"})
    assert "key" in r.json()
    assert r.json()["plan"] == "free"

def test_post_login():
    r = client.post("/login", json={"email": email, "password": "123"})
    assert "key" in r.json()
    assert r.json()["plan"] == "free"
 
def test_post_login_wrong_password():
    r = client.post("/login", json={"email": email, "password": "wrongpass"})
    assert r.status_code == 401

def test_get_validate_invalid_key():
    r = client.get("/validate", params={"key": key, "hwid": hwid})
    assert r.status_code == 404

def test_get_validate():
    reg = client.post("/register", json={"email": email_val, "password": "123"})
    r = client.get("/validate", params={"key": reg.json()["key"], "hwid": hwid})
    assert r.status_code == 200

def test_get_plugins():
    r = client.get("/plugins")
    assert isinstance(r.json(), list)

def test_post_plugins():
    r = client.post("/plugins", json={
                                      "name": name, 
                                      "description": description, 
                                      "code": code, 
                                      "author_id": author_id, 
                                      "is_verified": is_verified
                                      })
    assert r.status_code == 200

# ── OPTS ────────────────────────────────────────────────────────────────────

def test_recognize_cmd_commands():

    result = recognize_cmd("стоп")
    assert result["cmd"] == "stop"

    result = recognize_cmd("котра година")
    assert result["cmd"] == "ctime"

    result = recognize_cmd("статистика")
    assert result["cmd"] == "stats"

    result = recognize_cmd("wake up daddy's home")
    assert result["cmd"] == "wakeup"

    result = recognize_cmd("сховай все крім")
    assert result["cmd"] == "window"

    result = recognize_cmd("надрукуй")
    assert result["cmd"] == "dictation"

    result = recognize_cmd("affirmative")
    assert result["cmd"] == "confirm_yes"
    
    result = recognize_cmd("negative")
    assert result["cmd"] == "confirm_no"

    result = recognize_cmd("analyze screen")
    assert result["cmd"] == "screen"

    result = recognize_cmd("запусти плагін")
    assert result["cmd"] == "plugin"

    result = recognize_cmd("створи плагін")
    assert result["cmd"] == "plugin_create"

    result = recognize_cmd("відкоти плагін")
    assert result["cmd"] == "plugin_rollback"

    result = recognize_cmd("ollama mode")
    assert result["cmd"] == "ai_mode_ollama"

    result = recognize_cmd("gemini mode")
    assert result["cmd"] == "ai_mode_gemini"

    result = recognize_cmd("overlay")
    assert result["cmd"] == "overlay"

    # result = recognize_cmd("turn off overlay")
    # assert result["cmd"] == "overlay_hide"

    # result = recognize_cmd("перемісти оверлей")
    # assert result["cmd"] == "overlay_move"

    result = recognize_cmd("toggle music")
    assert result["cmd"] == "music_toggle_play_pause"

    result = recognize_cmd("next")
    assert result["cmd"] == "music_next"

    result = recognize_cmd("prev")
    assert result["cmd"] == "music_prev"

    result = recognize_cmd("music info")
    assert result["cmd"] == "music_info"

    result = recognize_cmd("прочитай файл")
    assert result["cmd"] == "file_read"

    result = recognize_cmd("створи файл")
    assert result["cmd"] == "file_write"

    result = recognize_cmd("додай до файлу")
    assert result["cmd"] == "file_append"

    result = recognize_cmd("що в папці")
    assert result["cmd"] == "file_list"

    result = recognize_cmd("видали файл")
    assert result["cmd"] == "file_delete"

    result = recognize_cmd("зміни назву файлу")
    assert result["cmd"] == "file_rename"

def test_recognize_cmd_fuzzy():

    result = recognize_cmd("стопп")
    assert result["percent"] >= 60
    assert result["cmd"] == "stop"

    result = recognize_cmd("котраа годиинна")
    assert result["percent"] >= 60
    assert result["cmd"] == "ctime"

def test_recognise_cmd_fuzzy_rubish():
    result = recognize_cmd("фдлоы р ав длфрм")
    assert not result["percent"] >= 60

    result = recognize_cmd("ліоварп дфлам")
    assert not result["percent"] >= 60

    result = recognize_cmd("ядажлоп мофу кщжшшп")
    assert not result["percent"] >= 60

    # ── other shi ────────────────────────────────────────────────────────────────────
    # test_save_settings / test_load_settings — сохранить и прочитать
    # test_strip_markdown для ***bold***, ~~strike~~
    # test_recognize_cmd для новых команд: file_read, file_write, music_toggle
    # test_get_plugins_empty — пустой список когда нет плагинов

from main import save_settings, load_settings

def test_save_settings():
    save_settings({"ai_mode": "ollama", "gemini_key": "test_key", "theme": {"accent": "#e94560", "bg": "#0d0d1a", "secondary": "#556080"}})
    result = load_settings()
    assert result["ai_mode"] == "ollama"

def test_load_settings():
    result = load_settings()
    assert "ai_mode" in result
    assert "theme" in result
    assert "gemini_key" in result

    #── ask_ai ────────────────────────────────────────────────────────────────────

from main import ask_ai

def test_ask_ai():
    text = ""
    assert isinstance(ask_ai(text), str)

    #── LANGUAGES ────────────────────────────────────────────────────────────────────

from main import split_by_language

def test_split_by_language_en():
    text = "hello"
    assert split_by_language(text) == [("en", "hello")]

def test_split_by_language_uk():
    text = "ъуъ"
    assert split_by_language(text) == [("ru", "ъуъ")]

def test_split_by_language_ru():
    text = "привіт"
    assert split_by_language(text) == [("uk", "привіт")]

    #── PARSE_REM ────────────────────────────────────────────────────────────────────

    # def parse_reminder(text):
    #     match = re.search(r'через (\d+) (хвилин|секунд|годин|минут|секунд|часов|minutes|seconds|hours)', text)
    #     if match:
    #         n = int(match.group(1))
    #         unit = match.group(2)
    #         units = {"хвилин": 60, "годин": 3600, "секунд": 1, "минут": 60, "часов": 3600, "minutes": 60, "hours": 3600, "seconds": 1}
    #         seconds = n * units.get(unit, 60)
    #         return seconds
    #     else:
    #         return None

from main import parse_reminder

def test_parse_reminder():
    assert parse_reminder("через 5 хвилин зателефонувати") == 300
    assert parse_reminder("пргипш плдитиукідлолуррію") == None