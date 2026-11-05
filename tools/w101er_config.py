"""The build's paths from config.toml (round 35, R-BUILD): every tool asks here before using its default.

The lookup order for each setting is:
1. The environment variable.
2. config.toml at the repo root, or the file named by W101ER_CONFIG.
3. The tool's own default, which is the author's layout, so a PC without a config keeps working.

| Setting (environment variable) | config.toml key | What it is |
| --- | --- | --- |
| ER_GAME | game | ELDEN RING\\Game, unpacked with UXM |
| W101_INSTALL | wizard101 | the Wizard101 install (the folder holding Data\\GameData) |
| W101ER_ROOT | work_root | where the build writes everything (reference\\..., created by build.py) |
| SMITHBOX | smithbox | the Smithbox folder (for its paramdefs) |
| MODENGINE | modengine | the Mod Engine 2 folder (its config_eldenring.toml is written) |
| BLENDER | blender | blender.exe |
"""
import os
from pathlib import Path

KEYS = {"ER_GAME": "game", "W101_INSTALL": "wizard101", "W101ER_ROOT": "work_root", "SMITHBOX": "smithbox",
        "MODENGINE": "modengine", "BLENDER": "blender"}
CONFIG = Path(os.environ.get("W101ER_CONFIG", Path(__file__).resolve().parent.parent / "config.toml"))
_cache = None


def load(path=CONFIG):
    """{key: value} of the [paths] table (or top level) of config.toml; {} when it is missing."""
    path = Path(path)
    if not path.exists():
        return {}
    try:
        import tomllib
        data = tomllib.loads(path.read_text(encoding="utf-8"))
        return {k: str(v) for k, v in {**data, **data.get("paths", {})}.items() if not isinstance(v, dict)}
    except ImportError:  # Python < 3.11: key = 'value' or "value" lines only
        out = {}
        for line in path.read_text(encoding="utf-8").splitlines():
            line = line.split("#", 1)[0].strip()
            if "=" in line and not line.startswith("["):
                key, value = (x.strip() for x in line.split("=", 1))
                quote = value[:1]
                value = value[1:-1] if quote in "'\"" and value.endswith(quote) else value
                out[key] = value if quote == "'" else value.replace("\\\\", "\\")
        return out


def setting(name, default=None):
    """The environment variable `name`, else its config.toml key, else `default`."""
    global _cache
    if name in os.environ:
        return os.environ[name]
    if _cache is None:
        _cache = load()
    value = _cache.get(KEYS.get(name, name.lower()))
    return value if value else default
